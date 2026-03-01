use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::{body::Incoming, Method, Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio_rustls::TlsConnector;
use tracing::{error, info, warn};

use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::replay_db::{
    KeyValueItem, ReplayBody, ReplayGroup, ReplayHistory, ReplayRequest, ReplayRequestSummary,
    RuleConfig, MAX_CONCURRENT_REPLAYS, MAX_HISTORY, MAX_REQUESTS,
};
use crate::state::SharedAdminState;
use crate::traffic::{MatchedRule, RequestTiming, TrafficRecord};

static REPLAY_SEMAPHORE: once_cell::sync::Lazy<Arc<Semaphore>> =
    once_cell::sync::Lazy::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_REPLAYS)));

static REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExecuteRequest {
    pub request: ReplayRequestData,
    pub rule_config: RuleConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRequestData {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayExecuteResponse {
    pub traffic_id: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub duration_ms: u64,
    pub applied_rules: Vec<MatchedRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn handle_replay(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/replay/execute" {
        match method {
            Method::POST => execute_replay(req, state).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/groups" || path == "/api/replay/groups/" {
        match method {
            Method::GET => list_groups(state).await,
            Method::POST => create_group(req, state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(id) = path.strip_prefix("/api/replay/groups/") {
        match method {
            Method::GET => get_group(state, id).await,
            Method::PUT => update_group(req, state, id).await,
            Method::DELETE => delete_group(state, id).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/requests" || path == "/api/replay/requests/" {
        match method {
            Method::GET => list_requests(req, state).await,
            Method::POST => create_request(req, state).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/requests/count" {
        match method {
            Method::GET => count_requests(state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(rest) = path.strip_prefix("/api/replay/requests/") {
        if let Some(id) = rest.strip_suffix("/move") {
            match method {
                Method::PUT => move_request(req, state, id).await,
                _ => method_not_allowed(),
            }
        } else {
            match method {
                Method::GET => get_request(state, rest).await,
                Method::PUT => update_request(req, state, rest).await,
                Method::DELETE => delete_request(state, rest).await,
                _ => method_not_allowed(),
            }
        }
    } else if path == "/api/replay/history" || path == "/api/replay/history/" {
        match method {
            Method::GET => list_history(req, state).await,
            Method::DELETE => clear_history(req, state).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/history/count" {
        match method {
            Method::GET => count_history(req, state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(id) = path.strip_prefix("/api/replay/history/") {
        match method {
            Method::DELETE => delete_history(state, id).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/stats" {
        match method {
            Method::GET => get_stats(state).await,
            _ => method_not_allowed(),
        }
    } else {
        error_response(StatusCode::NOT_FOUND, "Not Found")
    }
}

async fn execute_replay(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let execute_req: ReplayExecuteRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    let permit = match REPLAY_SEMAPHORE.clone().try_acquire_owned() {
        Ok(p) => p,
        Err(_) => {
            return error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "Too many concurrent replay requests",
            )
        }
    };

    let result = execute_replay_inner(&state, execute_req).await;
    drop(permit);

    match result {
        Ok(response) => {
            #[derive(Serialize)]
            struct ExecuteResult {
                success: bool,
                data: ReplayExecuteResponse,
            }
            json_response(&ExecuteResult {
                success: true,
                data: response,
            })
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn execute_replay_inner(
    state: &SharedAdminState,
    request: ReplayExecuteRequest,
) -> Result<ReplayExecuteResponse, String> {
    let start_time = Instant::now();
    let replay_id = format!("replay-{}", REPLAY_SEQUENCE.fetch_add(1, Ordering::SeqCst));

    let url = &request.request.url;
    let method = &request.request.method;

    info!(
        replay_id = %replay_id,
        method = %method,
        url = %url,
        "[REPLAY] Starting replay request"
    );

    let uri: Uri = url.parse().map_err(|e| format!("Invalid URL: {}", e))?;

    let is_https = uri.scheme_str() == Some("https");
    let host = uri
        .host()
        .ok_or_else(|| "Missing host".to_string())?
        .to_string();
    let port = uri.port_u16().unwrap_or(if is_https { 443 } else { 80 });
    let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

    let mut timing = RequestTiming::default();
    let dns_start = Instant::now();

    let connect_addr = format!("{}:{}", host, port);
    timing.dns_ms = Some(dns_start.elapsed().as_millis() as u64);

    let connect_start = Instant::now();
    let tcp_stream = TcpStream::connect(&connect_addr)
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", connect_addr, e))?;
    timing.connect_ms = Some(connect_start.elapsed().as_millis() as u64);

    let (status, response_headers, response_body, tls_ms) = if is_https {
        let tls_start = Instant::now();
        let (s, h, b) = send_https_request(
            tcp_stream,
            &host,
            method,
            path,
            &request.request.headers,
            request.request.body.as_deref(),
        )
        .await?;
        (s, h, b, Some(tls_start.elapsed().as_millis() as u64))
    } else {
        let (s, h, b) = send_http_request(
            tcp_stream,
            &host,
            method,
            path,
            &request.request.headers,
            request.request.body.as_deref(),
        )
        .await?;
        (s, h, b, None)
    };
    timing.tls_ms = tls_ms;

    let duration_ms = start_time.elapsed().as_millis() as u64;
    timing.total_ms = duration_ms;

    let applied_rules: Vec<MatchedRule> = vec![];

    let traffic_id = record_traffic(
        state,
        &replay_id,
        &request,
        status,
        &response_headers,
        response_body.as_deref(),
        duration_ms,
        &applied_rules,
        &timing,
    );

    if let Some(request_id) = &request.request_id {
        record_history(
            state,
            request_id,
            &traffic_id,
            method,
            url,
            status,
            duration_ms,
            &request.rule_config,
        );
    }

    info!(
        replay_id = %replay_id,
        traffic_id = %traffic_id,
        status = status,
        duration_ms = duration_ms,
        "[REPLAY] Completed replay request"
    );

    Ok(ReplayExecuteResponse {
        traffic_id,
        status,
        headers: response_headers,
        body: response_body,
        duration_ms,
        applied_rules,
        error: None,
    })
}

async fn send_http_request(
    stream: TcpStream,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<(u16, Vec<(String, String)>, Option<String>), String> {
    let io = TokioIo::new(stream);

    let (mut sender, conn) = ClientBuilder::new()
        .handshake(io)
        .await
        .map_err(|e| format!("HTTP handshake failed: {}", e))?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            error!(error = %e, "[REPLAY] HTTP connection error");
        }
    });

    let mut req_builder = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("Host", host);

    for (key, value) in headers {
        let key_lower = key.to_lowercase();
        if key_lower == "host" || key_lower == "content-length" {
            continue;
        }
        req_builder = req_builder.header(key, value);
    }

    let body_bytes = body.map(|b| Bytes::from(b.to_string())).unwrap_or_default();
    if !body_bytes.is_empty() {
        req_builder = req_builder.header("Content-Length", body_bytes.len().to_string());
    }

    let request = req_builder
        .body(http_body_util::Full::new(body_bytes))
        .map_err(|e| format!("Failed to build request: {}", e))?;

    let response = sender
        .send_request(request)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    parse_response(response).await
}

async fn send_https_request(
    stream: TcpStream,
    host: &str,
    method: &str,
    path: &str,
    headers: &[(String, String)],
    body: Option<&str>,
) -> Result<(u16, Vec<(String, String)>, Option<String>), String> {
    let tls_config = get_tls_client_config();
    let connector = TlsConnector::from(Arc::new(tls_config));

    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| format!("Invalid server name: {}", e))?;

    let tls_stream = connector
        .connect(server_name, stream)
        .await
        .map_err(|e| format!("TLS handshake failed: {}", e))?;

    let io = TokioIo::new(tls_stream);

    let (mut sender, conn) = ClientBuilder::new()
        .handshake(io)
        .await
        .map_err(|e| format!("HTTPS handshake failed: {}", e))?;

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            error!(error = %e, "[REPLAY] HTTPS connection error");
        }
    });

    let mut req_builder = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("Host", host);

    for (key, value) in headers {
        let key_lower = key.to_lowercase();
        if key_lower == "host" || key_lower == "content-length" {
            continue;
        }
        req_builder = req_builder.header(key, value);
    }

    let body_bytes = body.map(|b| Bytes::from(b.to_string())).unwrap_or_default();
    if !body_bytes.is_empty() {
        req_builder = req_builder.header("Content-Length", body_bytes.len().to_string());
    }

    let request = req_builder
        .body(http_body_util::Full::new(body_bytes))
        .map_err(|e| format!("Failed to build request: {}", e))?;

    let response = sender
        .send_request(request)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    parse_response(response).await
}

async fn parse_response<B>(
    response: hyper::Response<B>,
) -> Result<(u16, Vec<(String, String)>, Option<String>), String>
where
    B: hyper::body::Body,
    B::Error: std::fmt::Display,
{
    let status = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?
        .to_bytes();

    let body = if body_bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&body_bytes).to_string())
    };

    Ok((status, headers, body))
}

#[allow(clippy::too_many_arguments)]
fn record_traffic(
    state: &SharedAdminState,
    replay_id: &str,
    request: &ReplayExecuteRequest,
    status: u16,
    response_headers: &[(String, String)],
    response_body: Option<&str>,
    duration_ms: u64,
    applied_rules: &[MatchedRule],
    timing: &RequestTiming,
) -> String {
    let traffic_id = format!("{}-{}", replay_id, uuid::Uuid::new_v4());
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    let url = &request.request.url;
    let uri: Uri = url.parse().unwrap_or_default();
    let host = uri.host().unwrap_or("unknown").to_string();
    let path = uri.path().to_string();
    let is_https = uri.scheme_str() == Some("https");

    let content_type = response_headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "content-type")
        .map(|(_, v)| v.clone());

    let request_content_type = request
        .request
        .headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "content-type")
        .map(|(_, v)| v.clone());

    let request_size = request.request.body.as_ref().map(|b| b.len()).unwrap_or(0);
    let response_size = response_body.map(|b| b.len()).unwrap_or(0);

    let record = TrafficRecord {
        id: traffic_id.clone(),
        sequence: 0,
        timestamp,
        host,
        method: request.request.method.clone(),
        url: url.clone(),
        path,
        status,
        protocol: if is_https { "https" } else { "http" }.to_string(),
        content_type,
        request_content_type,
        request_size,
        response_size,
        duration_ms,
        client_ip: "127.0.0.1".to_string(),
        client_app: Some("Bifrost Replay".to_string()),
        client_pid: None,
        client_path: None,
        is_tunnel: false,
        is_websocket: false,
        is_sse: false,
        is_h3: false,
        has_rule_hit: !applied_rules.is_empty(),
        is_replay: true,
        frame_count: 0,
        last_frame_id: 0,
        timing: Some(timing.clone()),
        request_headers: Some(request.request.headers.clone()),
        response_headers: Some(response_headers.to_vec()),
        matched_rules: if applied_rules.is_empty() {
            None
        } else {
            Some(applied_rules.to_vec())
        },
        socket_status: None,
        request_body_ref: None,
        response_body_ref: None,
        actual_url: None,
        actual_host: None,
        original_request_headers: None,
        actual_response_headers: None,
        error_message: None,
        req_script_results: None,
        res_script_results: None,
    };

    if let Some(ref traffic_db) = state.traffic_db_store {
        traffic_db.record(record);
    } else if let Some(ref async_writer) = state.async_traffic_writer {
        async_writer.record(record);
    }

    if let Some(body) = request.request.body.as_ref() {
        if let Some(ref body_store) = state.body_store {
            let _ = body_store.read().store(&traffic_id, "req", body.as_bytes());
        }
    }

    if let Some(body) = response_body {
        if let Some(ref body_store) = state.body_store {
            let _ = body_store.read().store(&traffic_id, "res", body.as_bytes());
        }
    }

    traffic_id
}

#[allow(clippy::too_many_arguments)]
fn record_history(
    state: &SharedAdminState,
    request_id: &str,
    traffic_id: &str,
    method: &str,
    url: &str,
    status: u16,
    duration_ms: u64,
    rule_config: &RuleConfig,
) {
    if let Some(ref replay_db) = state.replay_db_store {
        let history = ReplayHistory::new(
            Some(request_id.to_string()),
            traffic_id.to_string(),
            method.to_string(),
            url.to_string(),
            status,
            duration_ms,
            Some(rule_config.clone()),
        );
        if let Err(e) = replay_db.create_history(&history) {
            warn!(error = %e, "[REPLAY] Failed to record history");
        }
    }
}

fn get_tls_client_config() -> rustls::ClientConfig {
    use rustls::{ClientConfig, RootCertStore};

    let mut root_store = RootCertStore::empty();
    let certs = rustls_native_certs::load_native_certs();
    for cert in certs.certs {
        let _ = root_store.add(cert);
    }

    ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

async fn list_groups(state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let groups = store.list_groups();

    #[derive(Serialize)]
    struct GroupsResponse {
        groups: Vec<ReplayGroup>,
    }
    json_response(&GroupsResponse { groups })
}

#[derive(Deserialize)]
struct CreateGroupRequest {
    name: String,
    parent_id: Option<String>,
}

async fn create_group(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let create_req: CreateGroupRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    let now = chrono::Utc::now().timestamp_millis() as u64;
    let group = ReplayGroup {
        id: uuid::Uuid::new_v4().to_string(),
        name: create_req.name,
        parent_id: create_req.parent_id,
        sort_order: 0,
        created_at: now,
        updated_at: now,
    };

    if let Err(e) = store.create_group(&group) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to create group: {}", e),
        );
    }

    json_response(&group)
}

async fn get_group(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    match store.get_group(id) {
        Some(group) => json_response(&group),
        None => error_response(StatusCode::NOT_FOUND, "Group not found"),
    }
}

#[derive(Deserialize)]
struct UpdateGroupRequest {
    name: Option<String>,
    parent_id: Option<String>,
    sort_order: Option<i32>,
}

async fn update_group(
    req: Request<Incoming>,
    state: SharedAdminState,
    id: &str,
) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let mut group = match store.get_group(id) {
        Some(g) => g,
        None => return error_response(StatusCode::NOT_FOUND, "Group not found"),
    };

    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let update_req: UpdateGroupRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    if let Some(name) = update_req.name {
        group.name = name;
    }
    if let Some(parent_id) = update_req.parent_id {
        group.parent_id = Some(parent_id);
    }
    if let Some(sort_order) = update_req.sort_order {
        group.sort_order = sort_order;
    }
    group.updated_at = chrono::Utc::now().timestamp_millis() as u64;

    if let Err(e) = store.update_group(&group) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update group: {}", e),
        );
    }

    json_response(&group)
}

async fn delete_group(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    if let Err(e) = store.delete_group(id) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete group: {}", e),
        );
    }

    success_response("Group deleted")
}

async fn list_requests(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let query = req.uri().query().unwrap_or("");
    let params: std::collections::HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let saved_only = params.get("saved").map(|v| v == "true");
    let group_id = params.get("group_id").map(|s| s.as_str());
    let limit = params.get("limit").and_then(|v| v.parse().ok());
    let offset = params.get("offset").and_then(|v| v.parse().ok());

    let requests = store.list_requests(saved_only, group_id, limit, offset);
    let total = store.count_requests();

    #[derive(Serialize)]
    struct RequestsResponse {
        requests: Vec<ReplayRequestSummary>,
        total: usize,
        max_requests: usize,
    }
    json_response(&RequestsResponse {
        requests,
        total,
        max_requests: MAX_REQUESTS,
    })
}

async fn count_requests(state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let count = store.count_requests();

    #[derive(Serialize)]
    struct CountResponse {
        count: usize,
        max_requests: usize,
    }
    json_response(&CountResponse {
        count,
        max_requests: MAX_REQUESTS,
    })
}

#[derive(Deserialize)]
struct CreateRequestRequest {
    #[serde(default)]
    group_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<KeyValueItem>,
    #[serde(default)]
    body: Option<ReplayBody>,
    #[serde(default)]
    is_saved: bool,
}

async fn create_request(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let count = store.count_requests();
    if count >= MAX_REQUESTS {
        return error_response(
            StatusCode::CONFLICT,
            &format!(
                "Maximum request limit ({}) reached. Please delete some requests first.",
                MAX_REQUESTS
            ),
        );
    }

    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let create_req: CreateRequestRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    let now = chrono::Utc::now().timestamp_millis() as u64;
    let request = ReplayRequest {
        id: uuid::Uuid::new_v4().to_string(),
        group_id: create_req.group_id,
        name: create_req.name,
        method: create_req.method,
        url: create_req.url,
        headers: create_req.headers,
        body: create_req.body,
        is_saved: create_req.is_saved,
        sort_order: 0,
        created_at: now,
        updated_at: now,
    };

    if let Err(e) = store.create_request(&request) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to create request: {}", e),
        );
    }

    json_response(&request)
}

async fn get_request(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    match store.get_request(id) {
        Some(request) => json_response(&request),
        None => error_response(StatusCode::NOT_FOUND, "Request not found"),
    }
}

#[derive(Deserialize)]
struct UpdateRequestRequest {
    #[serde(default)]
    group_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    headers: Option<Vec<KeyValueItem>>,
    #[serde(default)]
    body: Option<ReplayBody>,
    #[serde(default)]
    is_saved: Option<bool>,
    #[serde(default)]
    sort_order: Option<i32>,
}

async fn update_request(
    req: Request<Incoming>,
    state: SharedAdminState,
    id: &str,
) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let mut request = match store.get_request(id) {
        Some(r) => r,
        None => return error_response(StatusCode::NOT_FOUND, "Request not found"),
    };

    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let update_req: UpdateRequestRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    if update_req.group_id.is_some() {
        request.group_id = update_req.group_id;
    }
    if let Some(name) = update_req.name {
        request.name = Some(name);
    }
    if let Some(method) = update_req.method {
        request.method = method;
    }
    if let Some(url) = update_req.url {
        request.url = url;
    }
    if let Some(headers) = update_req.headers {
        request.headers = headers;
    }
    if update_req.body.is_some() {
        request.body = update_req.body;
    }
    if let Some(is_saved) = update_req.is_saved {
        request.is_saved = is_saved;
    }
    if let Some(sort_order) = update_req.sort_order {
        request.sort_order = sort_order;
    }
    request.updated_at = chrono::Utc::now().timestamp_millis() as u64;

    if let Err(e) = store.update_request(&request) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update request: {}", e),
        );
    }

    json_response(&request)
}

async fn delete_request(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    if let Err(e) = store.delete_request(id) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete request: {}", e),
        );
    }

    success_response("Request deleted")
}

#[derive(Deserialize)]
struct MoveRequestRequest {
    group_id: Option<String>,
}

async fn move_request(
    req: Request<Incoming>,
    state: SharedAdminState,
    id: &str,
) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let move_req: MoveRequestRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    if let Err(e) = store.move_request_to_group(id, move_req.group_id.as_deref()) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to move request: {}", e),
        );
    }

    success_response("Request moved")
}

async fn list_history(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let query = req.uri().query().unwrap_or("");
    let params: std::collections::HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let request_id = params.get("request_id").map(|s| s.as_str());
    let limit = params.get("limit").and_then(|v| v.parse().ok());
    let offset = params.get("offset").and_then(|v| v.parse().ok());

    let history = store.list_history(request_id, limit, offset);
    let total = store.count_history(request_id);

    #[derive(Serialize)]
    struct HistoryResponse {
        history: Vec<ReplayHistory>,
        total: usize,
        max_history: usize,
    }
    json_response(&HistoryResponse {
        history,
        total,
        max_history: MAX_HISTORY,
    })
}

async fn count_history(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let query = req.uri().query().unwrap_or("");
    let params: std::collections::HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let request_id = params.get("request_id").map(|s| s.as_str());
    let count = store.count_history(request_id);

    #[derive(Serialize)]
    struct CountResponse {
        count: usize,
        max_history: usize,
    }
    json_response(&CountResponse {
        count,
        max_history: MAX_HISTORY,
    })
}

async fn delete_history(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    if let Err(e) = store.delete_history(id) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete history: {}", e),
        );
    }

    success_response("History deleted")
}

async fn clear_history(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let query = req.uri().query().unwrap_or("");
    let params: std::collections::HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let request_id = params.get("request_id").map(|s| s.as_str());

    match store.clear_history(request_id) {
        Ok(deleted) => {
            #[derive(Serialize)]
            struct ClearResponse {
                success: bool,
                deleted: usize,
            }
            json_response(&ClearResponse {
                success: true,
                deleted,
            })
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to clear history: {}", e),
        ),
    }
}

async fn get_stats(state: SharedAdminState) -> Response<BoxBody> {
    let store = match &state.replay_db_store {
        Some(s) => s,
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Replay store not available",
            )
        }
    };

    let stats = store.stats();
    json_response(&stats)
}

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use base64::Engine;
use bytes::Bytes;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::{body::Incoming, upgrade, Method, Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio_rustls::TlsConnector;

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, WebSocketStream};
use tracing::{error, info, warn};

use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::push::SharedPushManager;
use crate::replay_db::{
    KeyValueItem, ReplayBody, ReplayGroup, ReplayHistory, ReplayRequest, ReplayRequestSummary,
    RequestType, RuleConfig, RuleMode, MAX_CONCURRENT_REPLAYS, MAX_HISTORY, MAX_REQUESTS,
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
pub struct ReplayStreamRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub type_: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub type_: String,
    pub data: String,
    pub timestamp: u64,
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
    push_manager: Option<SharedPushManager>,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/replay/execute" {
        match method {
            Method::POST => execute_replay(req, state, push_manager).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/execute/stream" {
        match method {
            Method::POST => execute_replay_stream(req, state, push_manager).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/execute/ws" {
        match method {
            Method::GET => execute_replay_websocket(req, state, push_manager).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/groups" || path == "/api/replay/groups/" {
        match method {
            Method::GET => list_groups(state).await,
            Method::POST => create_group(req, state, push_manager).await,
            _ => method_not_allowed(),
        }
    } else if let Some(id) = path.strip_prefix("/api/replay/groups/") {
        match method {
            Method::GET => get_group(state, id).await,
            Method::PUT => update_group(req, state, id).await,
            Method::DELETE => delete_group(state, push_manager, id).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/replay/requests" || path == "/api/replay/requests/" {
        match method {
            Method::GET => list_requests(req, state).await,
            Method::POST => create_request(req, state, push_manager).await,
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
                Method::PUT => update_request(req, state, push_manager, rest).await,
                Method::DELETE => delete_request(state, push_manager, rest).await,
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

async fn execute_replay(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
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

    let result = execute_replay_inner(&state, &push_manager, execute_req).await;
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
    push_manager: &Option<SharedPushManager>,
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
            push_manager,
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

    let content_encoding = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))
        .map(|(_, v)| v.clone());

    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?
        .to_bytes();

    let body = if body_bytes.is_empty() {
        None
    } else {
        let decompressed = decompress_body(&body_bytes, content_encoding.as_deref());
        Some(String::from_utf8_lossy(&decompressed).to_string())
    };

    Ok((status, headers, body))
}

fn decompress_body(data: &[u8], content_encoding: Option<&str>) -> Bytes {
    let encoding = match content_encoding {
        Some(e) => e.to_lowercase(),
        None => return Bytes::copy_from_slice(data),
    };

    let result = match encoding.as_str() {
        "gzip" => decompress_gzip(data),
        "deflate" => decompress_deflate(data),
        "br" => decompress_brotli(data),
        "zstd" => decompress_zstd(data),
        _ => return Bytes::copy_from_slice(data),
    };

    match result {
        Ok(decompressed) => Bytes::from(decompressed),
        Err(e) => {
            tracing::debug!(
                encoding = %encoding,
                error = %e,
                "[REPLAY] Failed to decompress body"
            );
            Bytes::copy_from_slice(data)
        }
    }
}

fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_deflate(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    if let Ok(result) = decompress_zlib(data) {
        return Ok(result);
    }
    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_brotli(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decompressed = Vec::new();
    brotli::BrotliDecompress(&mut std::io::Cursor::new(data), &mut decompressed)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    Ok(decompressed)
}

fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    zstd::stream::decode_all(std::io::Cursor::new(data))
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

    let request_body_ref = if let Some(body) = request.request.body.as_ref() {
        if let Some(ref body_store) = state.body_store {
            body_store.read().store(&traffic_id, "req", body.as_bytes())
        } else {
            None
        }
    } else {
        None
    };

    let response_body_ref = if let Some(body) = response_body {
        if let Some(ref body_store) = state.body_store {
            body_store.read().store(&traffic_id, "res", body.as_bytes())
        } else {
            None
        }
    } else {
        None
    };

    let mut record = record;
    record.request_body_ref = request_body_ref;
    record.response_body_ref = response_body_ref;

    if let Some(ref traffic_db) = state.traffic_db_store {
        traffic_db.record(record);
    } else if let Some(ref async_writer) = state.async_traffic_writer {
        async_writer.record(record);
    }

    traffic_id
}

#[allow(clippy::too_many_arguments)]
fn record_history(
    state: &SharedAdminState,
    push_manager: &Option<SharedPushManager>,
    request_id: &str,
    traffic_id: &str,
    method: &str,
    url: &str,
    status: u16,
    duration_ms: u64,
    rule_config: &RuleConfig,
) {
    if let Some(ref replay_db) = state.replay_db_store {
        let request = replay_db.get_request(request_id);
        let is_saved = request.as_ref().map(|r| r.is_saved).unwrap_or(false);

        info!(
            request_id = %request_id,
            is_saved = %is_saved,
            request_exists = %request.is_some(),
            "[REPLAY] Recording history"
        );

        let history = ReplayHistory::new(
            Some(request_id.to_string()),
            traffic_id.to_string(),
            method.to_string(),
            url.to_string(),
            status,
            duration_ms,
            Some(rule_config.clone()),
        );

        if is_saved {
            info!(
                history_id = %history.id,
                "[REPLAY] Saving history to database"
            );
            if let Err(e) = replay_db.create_history(&history) {
                warn!(error = %e, "[REPLAY] Failed to record history");
            } else {
                info!(
                    history_id = %history.id,
                    "[REPLAY] History saved successfully"
                );
                if let Some(pm) = push_manager {
                    pm.broadcast_replay_history_updated(
                        "history_created",
                        request_id,
                        Some(&history.id),
                    );
                }
            }
        } else {
            info!(
                history_id = %history.id,
                "[REPLAY] Request not saved, only broadcasting history event"
            );
            if let Some(pm) = push_manager {
                pm.broadcast_replay_history_updated(
                    "history_created",
                    request_id,
                    Some(&history.id),
                );
            }
        }
    } else {
        warn!("[REPLAY] replay_db_store is None, cannot record history");
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

async fn create_group(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
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

    if let Some(pm) = push_manager {
        pm.broadcast_replay_request_updated("group_created", None, Some(&group.id));
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

async fn delete_group(
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
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

    if let Err(e) = store.delete_group(id) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete group: {}", e),
        );
    }

    if let Some(pm) = push_manager {
        pm.broadcast_replay_request_updated("group_deleted", None, Some(id));
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
    #[serde(default)]
    request_type: RequestType,
    method: String,
    url: String,
    #[serde(default)]
    headers: Vec<KeyValueItem>,
    #[serde(default)]
    body: Option<ReplayBody>,
    #[serde(default)]
    is_saved: bool,
}

async fn create_request(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
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
        request_type: create_req.request_type,
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

    if let Some(pm) = push_manager {
        pm.broadcast_replay_request_updated(
            "request_created",
            Some(&request.id),
            request.group_id.as_deref(),
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
    request_type: Option<RequestType>,
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
    push_manager: Option<SharedPushManager>,
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
    if let Some(request_type) = update_req.request_type {
        request.request_type = request_type;
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

    if let Some(pm) = push_manager {
        pm.broadcast_replay_request_updated(
            "request_updated",
            Some(&request.id),
            request.group_id.as_deref(),
        );
    }

    json_response(&request)
}

async fn delete_request(
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
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

    if let Err(e) = store.delete_request(id) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to delete request: {}", e),
        );
    }

    if let Some(pm) = push_manager {
        pm.broadcast_replay_request_updated("request_deleted", Some(id), None);
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

async fn execute_replay_stream(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let stream_req: ReplayStreamRequest = match serde_json::from_slice(&body) {
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

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let replay_id = format!("replay-{}", REPLAY_SEQUENCE.fetch_add(1, Ordering::SeqCst));
    let url = stream_req.url.clone();
    let headers = stream_req.headers.clone();
    let request_id = stream_req.request_id.clone();

    tokio::spawn(async move {
        let result = execute_sse_stream(
            &state,
            &push_manager,
            &replay_id,
            &url,
            &headers,
            request_id,
            tx,
        )
        .await;
        drop(permit);
        if let Err(e) = result {
            error!(error = %e, replay_id = %replay_id, "Failed to execute SSE stream");
        }
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(|event| {
        let mut sse_event = String::from("event: message\n");
        sse_event.push_str(&format!("data: {}\n", event));
        sse_event.push('\n');
        Ok::<_, hyper::Error>(hyper::body::Frame::data(Bytes::from(sse_event)))
    });

    let body = http_body_util::StreamBody::new(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(BodyExt::boxed(body))
        .unwrap()
}

async fn execute_sse_stream(
    state: &SharedAdminState,
    push_manager: &Option<SharedPushManager>,
    replay_id: &str,
    url: &str,
    headers: &[(String, String)],
    request_id: Option<String>,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    info!(replay_id = %replay_id, url = %url, "Starting SSE stream");

    let traffic_id = record_traffic_for_stream(state, replay_id, url, headers, true);

    if let Some(ref req_id) = request_id {
        record_history(
            state,
            push_manager,
            req_id,
            &traffic_id,
            "GET",
            url,
            200,
            0,
            &RuleConfig {
                mode: RuleMode::Enabled,
                selected_rules: vec![],
            },
        );
    }

    // Use reqwest to handle SSE stream
    let client = reqwest::Client::new();
    let mut req_builder = client.get(url);

    for (key, value) in headers {
        req_builder = req_builder.header(key, value);
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("Request failed with status: {}", status));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut current_event = StreamEvent {
        type_: "message".to_string(),
        data: String::new(),
        id: None,
    };

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("Failed to read chunk: {}", e))?;
        buffer.extend_from_slice(&chunk);

        while buffer.contains(&b'\n') {
            if let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let line = buffer.drain(..=pos).collect::<Vec<_>>();
                let line_str = String::from_utf8_lossy(&line[..line.len() - 1]).to_string();

                if line_str.is_empty() {
                    if !current_event.data.is_empty() {
                        let event_json = serde_json::to_string(&current_event).unwrap();
                        let _ = tx.send(event_json);
                        record_sse_event(state, replay_id, &traffic_id, &current_event);
                    }
                    current_event = StreamEvent {
                        type_: "message".to_string(),
                        data: String::new(),
                        id: None,
                    };
                } else if let Some(event_type) = line_str.strip_prefix("event:") {
                    current_event.type_ = event_type.trim().to_string();
                } else if let Some(data) = line_str.strip_prefix("data:") {
                    if !current_event.data.is_empty() {
                        current_event.data.push('\n');
                    }
                    current_event.data.push_str(data.trim());
                } else if let Some(id) = line_str.strip_prefix("id:") {
                    current_event.id = Some(id.trim().to_string());
                }
            }
        }
    }

    // Send any remaining event
    if !current_event.data.is_empty() {
        let event_json = serde_json::to_string(&current_event).unwrap();
        tx.send(event_json).unwrap();
        record_sse_event(state, replay_id, &traffic_id, &current_event);
    }

    info!(replay_id = %replay_id, traffic_id = %traffic_id, "SSE stream completed");
    Ok(())
}

async fn execute_replay_websocket(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    use hyper::upgrade;
    use tokio_tungstenite::WebSocketStream;

    let upgrade_header = req
        .headers()
        .get("Upgrade")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !upgrade_header.eq_ignore_ascii_case("websocket") {
        return error_response(StatusCode::BAD_REQUEST, "Invalid upgrade header");
    }

    let ws_key = match req.headers().get("Sec-WebSocket-Key") {
        Some(key) => key.to_str().unwrap_or("").to_string(),
        None => {
            return error_response(StatusCode::BAD_REQUEST, "Missing Sec-WebSocket-Key header");
        }
    };

    let query = req.uri().query().unwrap_or("");
    let params: std::collections::HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let url = match params.get("url") {
        Some(url) => url.clone(),
        None => return error_response(StatusCode::BAD_REQUEST, "Missing url parameter"),
    };

    let request_id = params.get("request_id").cloned();
    let replay_id = format!("replay-{}", REPLAY_SEQUENCE.fetch_add(1, Ordering::SeqCst));

    info!(replay_id = %replay_id, url = %url, "Starting WebSocket proxy");

    let headers: Vec<(String, String)> = vec![];
    let traffic_id = record_traffic_for_stream(&state, &replay_id, &url, &headers, false);

    if let Some(ref req_id) = request_id {
        record_history(
            &state,
            &push_manager,
            req_id,
            &traffic_id,
            "GET",
            &url,
            200,
            0,
            &RuleConfig {
                mode: RuleMode::Enabled,
                selected_rules: vec![],
            },
        );
    }

    let accept_key = generate_accept_key(&ws_key);

    tokio::spawn(async move {
        let upgraded = match upgrade::on(req).await {
            Ok(u) => u,
            Err(e) => {
                error!(error = %e, "WebSocket upgrade failed");
                return;
            }
        };

        let client_ws = WebSocketStream::from_raw_socket(
            hyper_util::rt::TokioIo::new(upgraded),
            tokio_tungstenite::tungstenite::protocol::Role::Server,
            None,
        )
        .await;

        match connect_async(url).await {
            Ok((server_ws, _)) => {
                proxy_websocket(
                    client_ws,
                    server_ws,
                    &replay_id,
                    &traffic_id,
                    request_id,
                    &state,
                )
                .await;
            }
            Err(e) => {
                error!(error = %e, replay_id = %replay_id, "Failed to connect to target WebSocket");
                let (mut sender, _) = client_ws.split();
                let error_msg = Message::Text(
                    format!("Error: Failed to connect to target WebSocket: {}", e).into(),
                );
                let _ = sender.send(error_msg).await;
            }
        }

        info!(replay_id = %replay_id, traffic_id = %traffic_id, "WebSocket proxy closed");
    });

    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Accept", accept_key)
        .header("Access-Control-Allow-Origin", "*")
        .body(BoxBody::default())
        .unwrap()
}

fn generate_accept_key(key: &str) -> String {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use sha1::{Digest, Sha1};

    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    BASE64.encode(hasher.finalize())
}

async fn proxy_websocket(
    client_ws: WebSocketStream<hyper_util::rt::TokioIo<upgrade::Upgraded>>,
    server_ws: WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    replay_id: &str,
    traffic_id: &str,
    _request_id: Option<String>,
    state: &SharedAdminState,
) {
    let (mut client_tx, mut client_rx) = client_ws.split();
    let (mut server_tx, mut server_rx) = server_ws.split();

    let replay_id_clone = replay_id.to_string();
    let traffic_id_clone = traffic_id.to_string();
    let state_clone = state.clone();

    let client_to_server = tokio::spawn(async move {
        while let Some(Ok(msg)) = client_rx.next().await {
            match msg {
                Message::Text(text) => {
                    info!(replay_id = %replay_id_clone, "Client -> Server: {}", text);
                    // Record message
                    record_websocket_message(
                        &state_clone,
                        &replay_id_clone,
                        &traffic_id_clone,
                        &None,
                        "send",
                        &text,
                    );
                    if let Err(e) = server_tx.send(Message::Text(text)).await {
                        error!(error = %e, replay_id = %replay_id_clone, "Failed to send to server");
                        break;
                    }
                }
                Message::Binary(data) => {
                    info!(replay_id = %replay_id_clone, "Client -> Server: [Binary data]");
                    // Record message
                    record_websocket_message(
                        &state_clone,
                        &replay_id_clone,
                        &traffic_id_clone,
                        &None,
                        "send_binary",
                        &base64::engine::general_purpose::STANDARD.encode(&data),
                    );
                    if let Err(e) = server_tx.send(Message::Binary(data)).await {
                        error!(error = %e, replay_id = %replay_id_clone, "Failed to send to server");
                        break;
                    }
                }
                Message::Ping(data) => {
                    if let Err(_e) = server_tx.send(Message::Ping(data)).await {
                        break;
                    }
                }
                Message::Pong(data) => {
                    if let Err(_e) = server_tx.send(Message::Pong(data)).await {
                        break;
                    }
                }
                Message::Close(_) => {
                    let _ = server_tx.send(Message::Close(None)).await;
                    break;
                }
                Message::Frame(_) => {}
            }
        }
    });

    let replay_id_clone2 = replay_id.to_string();
    let traffic_id_clone2 = traffic_id.to_string();
    let state_clone2 = state.clone();

    let server_to_client = tokio::spawn(async move {
        while let Some(Ok(msg)) = server_rx.next().await {
            match msg {
                Message::Text(text) => {
                    info!(replay_id = %replay_id_clone2, "Server -> Client: {}", text);
                    // Record message
                    record_websocket_message(
                        &state_clone2,
                        &replay_id_clone2,
                        &traffic_id_clone2,
                        &None,
                        "receive",
                        &text,
                    );
                    if let Err(e) = client_tx.send(Message::Text(text)).await {
                        error!(error = %e, replay_id = %replay_id_clone2, "Failed to send to client");
                        break;
                    }
                }
                Message::Binary(data) => {
                    info!(replay_id = %replay_id_clone2, "Server -> Client: [Binary data]");
                    // Record message
                    record_websocket_message(
                        &state_clone2,
                        &replay_id_clone2,
                        &traffic_id_clone2,
                        &None,
                        "receive_binary",
                        &base64::engine::general_purpose::STANDARD.encode(&data),
                    );
                    if let Err(e) = client_tx.send(Message::Binary(data)).await {
                        error!(error = %e, replay_id = %replay_id_clone2, "Failed to send to client");
                        break;
                    }
                }
                Message::Ping(data) => {
                    if let Err(_e) = client_tx.send(Message::Ping(data)).await {
                        break;
                    }
                }
                Message::Pong(data) => {
                    if let Err(_e) = client_tx.send(Message::Pong(data)).await {
                        break;
                    }
                }
                Message::Close(_) => {
                    let _ = client_tx.send(Message::Close(None)).await;
                    break;
                }
                Message::Frame(_) => {}
            }
        }
    });

    tokio::select! {
        _ = client_to_server => {}
        _ = server_to_client => {}
    }

    info!(replay_id = %replay_id, traffic_id = %traffic_id, "WebSocket proxy closed");
}

fn record_traffic_for_stream(
    state: &SharedAdminState,
    replay_id: &str,
    url: &str,
    headers: &[(String, String)],
    is_sse: bool,
) -> String {
    let traffic_id = format!("{}-{}", replay_id, uuid::Uuid::new_v4());
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    let uri: Uri = url.parse().unwrap_or_default();
    let host = uri.host().unwrap_or("unknown").to_string();
    let path = uri.path().to_string();
    let is_https = uri.scheme_str() == Some("https");

    let request_content_type = headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "content-type")
        .map(|(_, v)| v.clone());

    let record = TrafficRecord {
        id: traffic_id.clone(),
        sequence: 0,
        timestamp,
        host,
        method: "GET".to_string(),
        url: url.to_string(),
        path,
        status: 200,
        protocol: if is_https { "https" } else { "http" }.to_string(),
        content_type: None,
        request_content_type,
        request_size: 0,
        response_size: 0,
        duration_ms: 0,
        client_ip: "127.0.0.1".to_string(),
        client_app: Some("Bifrost Replay".to_string()),
        client_pid: None,
        client_path: None,
        is_tunnel: false,
        is_websocket: !is_sse,
        is_sse,
        is_h3: false,
        has_rule_hit: false,
        is_replay: true,
        frame_count: 0,
        last_frame_id: 0,
        timing: None,
        request_headers: Some(headers.to_vec()),
        response_headers: None,
        matched_rules: None,
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

    traffic_id
}

fn record_websocket_message(
    state: &SharedAdminState,
    replay_id: &str,
    traffic_id: &str,
    _request_id: &Option<String>,
    direction: &str,
    data: &str,
) {
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    let message = WebSocketMessage {
        type_: direction.to_string(),
        data: data.to_string(),
        timestamp,
    };

    // Store message in body store
    if let Some(ref body_store) = state.body_store {
        let message_json = serde_json::to_string(&message).unwrap();
        let _ = body_store.read().store(
            traffic_id,
            &format!("ws_{}_{}", direction, timestamp),
            message_json.as_bytes(),
        );
    }

    info!(replay_id = %replay_id, traffic_id = %traffic_id, direction = %direction, timestamp = %timestamp, "Recorded WebSocket message");
}

fn record_sse_event(
    state: &SharedAdminState,
    replay_id: &str,
    traffic_id: &str,
    event: &StreamEvent,
) {
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    // Store event in body store
    if let Some(ref body_store) = state.body_store {
        let event_json = serde_json::to_string(event).unwrap();
        let _ = body_store.read().store(
            traffic_id,
            &format!("sse_{}_{}", event.type_, timestamp),
            event_json.as_bytes(),
        );
    }

    info!(replay_id = %replay_id, traffic_id = %traffic_id, event_type = %event.type_, "Recorded SSE event");
}

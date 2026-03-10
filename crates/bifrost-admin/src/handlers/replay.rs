use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use base64::Engine;
use bifrost_core::{parse_rules, RequestContext, Rule, RulesResolver, ValueStore};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use hyper::{body::Incoming, upgrade, Method, Request, Response, StatusCode, Uri};
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio_rustls::TlsConnector;

use tokio_tungstenite::{tungstenite::protocol::Message, WebSocketStream};
use tracing::{debug, error, info, warn};

use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::push::SharedPushManager;
use crate::replay_db::{
    KeyValueItem, ReplayBody, ReplayGroup, ReplayHistory, ReplayRequest, ReplayRequestSummary,
    RequestType, RuleConfig, RuleMode, MAX_CONCURRENT_REPLAYS, MAX_HISTORY, MAX_REQUESTS,
};
use crate::request_rules::{apply_all_request_rules, build_applied_rules, AppliedRequest};
use crate::state::SharedAdminState;
use crate::traffic::{MatchedRule, TrafficRecord};

static REPLAY_SEMAPHORE: once_cell::sync::Lazy<Arc<Semaphore>> =
    once_cell::sync::Lazy::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT_REPLAYS)));

static REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn get_header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn decode_replay_body(headers: &[(String, String)], body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }

    let encoding = get_header_value(headers, "content-encoding")
        .unwrap_or("")
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();

    let decoded = match encoding.as_str() {
        "" => body.to_vec(),
        "gzip" => {
            let mut d = flate2::read::GzDecoder::new(std::io::Cursor::new(body));
            let mut out = Vec::new();
            d.read_to_end(&mut out).ok()?;
            out
        }
        "deflate" => {
            let mut d = flate2::read::ZlibDecoder::new(std::io::Cursor::new(body));
            let mut out = Vec::new();
            d.read_to_end(&mut out).ok()?;
            out
        }
        "br" => {
            let mut d = brotli::Decompressor::new(std::io::Cursor::new(body), 4096);
            let mut out = Vec::new();
            d.read_to_end(&mut out).ok()?;
            out
        }
        "zstd" => zstd::stream::decode_all(std::io::Cursor::new(body)).ok()?,
        _ => body.to_vec(),
    };

    Some(String::from_utf8_lossy(&decoded).to_string())
}

#[cfg(test)]
mod replay_body_decode_tests {
    use super::decode_replay_body;

    #[test]
    fn decode_gzip_response_body() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let raw = br#"{"ok":true,"msg":"hello"}"#;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(raw).unwrap();
        let gz = enc.finish().unwrap();

        let headers = vec![("content-encoding".to_string(), "gzip".to_string())];
        let decoded = decode_replay_body(&headers, &gz).unwrap();
        assert_eq!(decoded, String::from_utf8_lossy(raw));
    }
}

fn default_method() -> String {
    "GET".to_string()
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

pub async fn handle_replay(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/replay/execute" || path == "/api/replay/execute/unified" {
        match method {
            Method::POST => execute_replay_unified(req, state, push_manager).await,
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

#[derive(Debug, Clone, Deserialize)]
struct UnifiedReplayRequest {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rule_config: Option<RuleConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
}

async fn execute_replay_unified(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid body: {}", e)),
    };

    let unified_req: UnifiedReplayRequest = match serde_json::from_slice(&body) {
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

    let replay_id = format!("replay-{}", REPLAY_SEQUENCE.fetch_add(1, Ordering::SeqCst));
    let rule_config = unified_req.rule_config.clone().unwrap_or(RuleConfig {
        mode: RuleMode::Enabled,
        selected_rules: vec![],
        custom_rules: None,
    });

    let (resolved_rules, matched_rules, applied_request) = resolve_and_apply_rules(
        &state,
        &rule_config,
        &unified_req.url,
        &unified_req.method,
        &unified_req.headers,
        unified_req.body.as_ref().map(|s| s.as_bytes()),
    );

    info!(
        replay_id = %replay_id,
        original_url = %unified_req.url,
        applied_url = %applied_request.url,
        rules_count = matched_rules.len(),
        "[UNIFIED_REPLAY] Applied request rules"
    );

    let applied_url_lower = applied_request.url.to_lowercase();
    if applied_url_lower.starts_with("ws://") || applied_url_lower.starts_with("wss://") {
        drop(permit);
        return error_response(
            StatusCode::BAD_REQUEST,
            "WebSocket URLs are not supported via HTTP endpoint. Use the WebSocket endpoint (/api/replay/execute/ws) instead.",
        );
    }

    let unsafe_ssl = state.runtime_config.read().await.unsafe_ssl;
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(unsafe_ssl)
        .build()
        .unwrap_or_default();

    let mut req_builder = match applied_request.method.to_uppercase().as_str() {
        "POST" => client.post(&applied_request.url),
        "PUT" => client.put(&applied_request.url),
        "PATCH" => client.patch(&applied_request.url),
        "DELETE" => client.delete(&applied_request.url),
        "HEAD" => client.head(&applied_request.url),
        "OPTIONS" => client.request(reqwest::Method::OPTIONS, &applied_request.url),
        _ => client.get(&applied_request.url),
    };

    for (key, value) in &applied_request.headers {
        req_builder = req_builder.header(key, value);
    }

    if let Some(ref body) = applied_request.body {
        req_builder = req_builder.body(body.clone());
    }

    let start_time = std::time::Instant::now();
    // NOTE: timeout_ms 只用于“连接建立/首包(headers)获取”的超时控制。
    // 不能用于整个请求生命周期，否则 SSE 这类长连接会在超时后被错误断开。
    let timeout_ms = unified_req
        .timeout_ms
        .unwrap_or(crate::replay_executor::DEFAULT_TIMEOUT_MS);
    let send_future = req_builder.send();
    let response = match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        send_future,
    )
    .await
    {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            drop(permit);
            let error_msg = format!("Request failed: {}", e);
            error!(replay_id = %replay_id, error = %error_msg, "[UNIFIED_REPLAY] Request failed");
            return error_response(StatusCode::BAD_GATEWAY, &error_msg);
        }
        Err(_) => {
            drop(permit);
            let error_msg = format!("Request timeout after {}ms", timeout_ms);
            error!(replay_id = %replay_id, error = %error_msg, "[UNIFIED_REPLAY] Request timed out");
            return error_response(StatusCode::GATEWAY_TIMEOUT, &error_msg);
        }
    };

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let is_sse = content_type.contains("text/event-stream");

    if is_sse {
        info!(replay_id = %replay_id, "[UNIFIED_REPLAY] Detected SSE response, switching to streaming mode");

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        #[derive(Serialize)]
        struct ConnectionEvent {
            type_: String,
            traffic_id: String,
            url: String,
            applied_url: String,
            applied_rules: Vec<MatchedRule>,
        }

        let traffic_id =
            record_traffic_for_stream(&state, &replay_id, &applied_request, &matched_rules, true);

        let conn_event = ConnectionEvent {
            type_: "connection".to_string(),
            traffic_id: traffic_id.clone(),
            url: unified_req.url.clone(),
            applied_url: applied_request.url.clone(),
            applied_rules: matched_rules.clone(),
        };
        let _ = tx.send(serde_json::to_string(&conn_event).unwrap());

        if let Some(ref req_id) = unified_req.request_id {
            record_history(
                &state,
                &push_manager,
                req_id,
                &traffic_id,
                &applied_request.method,
                &applied_request.url,
                200,
                0,
                &rule_config,
            );
        }

        let state_clone = state.clone();
        let replay_id_clone = replay_id.clone();
        let traffic_id_clone = traffic_id.clone();

        tokio::spawn(async move {
            let result = process_sse_response(
                &state_clone,
                &replay_id_clone,
                &traffic_id_clone,
                response,
                tx,
            )
            .await;
            drop(permit);
            if let Err(e) = result {
                error!(error = %e, replay_id = %replay_id_clone, "[UNIFIED_REPLAY] SSE stream processing failed");
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
    } else {
        let status = response.status().as_u16();
        let response_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        let response_body = match response.bytes().await {
            Ok(b) => decode_replay_body(&response_headers, &b),
            Err(_) => None,
        };

        let (status, response_headers, response_body) =
            apply_response_rules(&resolved_rules, status, response_headers, response_body);

        let duration_ms = start_time.elapsed().as_millis() as u64;
        drop(permit);

        let traffic_id = record_traffic_for_unified(
            &state,
            &replay_id,
            &unified_req,
            &applied_request,
            status,
            &response_headers,
            response_body.as_deref(),
            duration_ms,
            &matched_rules,
        );

        if let Some(ref req_id) = unified_req.request_id {
            record_history(
                &state,
                &push_manager,
                req_id,
                &traffic_id,
                &unified_req.method,
                &unified_req.url,
                status,
                duration_ms,
                &rule_config,
            );
        }

        info!(
            replay_id = %replay_id,
            traffic_id = %traffic_id,
            status = status,
            duration_ms = duration_ms,
            "[UNIFIED_REPLAY] Request completed"
        );

        #[derive(Serialize)]
        struct ExecuteResult {
            success: bool,
            data: UnifiedReplayResponse,
        }

        #[derive(Serialize)]
        struct UnifiedReplayResponse {
            traffic_id: String,
            status: u16,
            headers: Vec<(String, String)>,
            body: Option<String>,
            duration_ms: u64,
            applied_rules: Vec<MatchedRule>,
        }

        json_response(&ExecuteResult {
            success: true,
            data: UnifiedReplayResponse {
                traffic_id,
                status,
                headers: response_headers,
                body: response_body,
                duration_ms,
                applied_rules: matched_rules,
            },
        })
    }
}

async fn process_sse_response(
    state: &SharedAdminState,
    replay_id: &str,
    traffic_id: &str,
    response: reqwest::Response,
    tx: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<(), String> {
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut current_event = StreamEvent {
        type_: "message".to_string(),
        data: String::new(),
        id: None,
    };

    while let Some(chunk_result) = stream.next().await {
        if tx.is_closed() {
            info!(replay_id = %replay_id, traffic_id = %traffic_id, "[SSE] Client disconnected, stopping stream processing");
            return Ok(());
        }

        let chunk = chunk_result.map_err(|e| format!("Failed to read chunk: {}", e))?;
        buffer.extend_from_slice(&chunk);

        while buffer.contains(&b'\n') {
            if let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let line = buffer.drain(..=pos).collect::<Vec<_>>();
                let line_str = String::from_utf8_lossy(&line[..line.len() - 1]).to_string();

                if line_str.is_empty() {
                    if !current_event.data.is_empty() {
                        let event_json = serde_json::to_string(&current_event).unwrap();
                        if tx.send(event_json).is_err() {
                            info!(replay_id = %replay_id, traffic_id = %traffic_id, "[SSE] Client disconnected, stopping stream processing");
                            return Ok(());
                        }
                        record_sse_event(state, replay_id, traffic_id, &current_event);
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

    if !current_event.data.is_empty() && !tx.is_closed() {
        let event_json = serde_json::to_string(&current_event).unwrap();
        let _ = tx.send(event_json);
        record_sse_event(state, replay_id, traffic_id, &current_event);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn record_traffic_for_unified(
    state: &SharedAdminState,
    replay_id: &str,
    unified_req: &UnifiedReplayRequest,
    applied_request: &AppliedRequest,
    status: u16,
    response_headers: &[(String, String)],
    response_body: Option<&str>,
    duration_ms: u64,
    matched_rules: &[MatchedRule],
) -> String {
    let traffic_id = format!("{}-{}", replay_id, uuid::Uuid::new_v4());
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    let uri: Uri = applied_request.url.parse().unwrap_or_default();
    let host = uri.host().unwrap_or("unknown").to_string();
    let path = uri.path().to_string();
    let scheme = uri.scheme_str().unwrap_or("http");

    let request_content_type = applied_request
        .headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "content-type")
        .map(|(_, v)| v.clone());

    let response_content_type = response_headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "content-type")
        .map(|(_, v)| v.clone());

    let request_body_ref = if let Some(ref body) = unified_req.body {
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

    let record = TrafficRecord {
        id: traffic_id.clone(),
        sequence: 0,
        timestamp,
        host,
        method: applied_request.method.clone(),
        url: applied_request.url.clone(),
        path,
        status,
        protocol: scheme.to_string(),
        content_type: response_content_type,
        request_content_type,
        request_size: unified_req.body.as_ref().map(|b| b.len()).unwrap_or(0),
        response_size: response_body.map(|b| b.len()).unwrap_or(0),
        duration_ms,
        client_ip: "127.0.0.1".to_string(),
        client_app: Some("Bifrost Replay".to_string()),
        client_pid: None,
        client_path: None,
        is_tunnel: false,
        is_websocket: false,
        is_sse: false,
        is_h3: false,
        has_rule_hit: !matched_rules.is_empty(),
        is_replay: true,
        frame_count: 0,
        last_frame_id: 0,
        timing: None,
        request_headers: Some(applied_request.headers.clone()),
        response_headers: Some(response_headers.to_vec()),
        matched_rules: if matched_rules.is_empty() {
            None
        } else {
            Some(matched_rules.to_vec())
        },
        socket_status: None,
        request_body_ref,
        response_body_ref,
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
        source: crate::replay_db::RequestSource::Internal,
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

    let upgrade_headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .filter(|(k, _)| {
            let name = k.as_str().to_lowercase();
            !matches!(
                name.as_str(),
                "upgrade"
                    | "connection"
                    | "sec-websocket-key"
                    | "sec-websocket-version"
                    | "sec-websocket-extensions"
                    | "sec-websocket-protocol"
                    | "host"
            )
        })
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let query = req.uri().query().unwrap_or("");
    let params: HashMap<_, _> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();

    let url = match params.get("url") {
        Some(url) => url.clone(),
        None => return error_response(StatusCode::BAD_REQUEST, "Missing url parameter"),
    };

    let request_id = params.get("request_id").cloned();

    let rule_config = if let Some(rule_config_str) = params.get("rule_config") {
        serde_json::from_str(rule_config_str).unwrap_or(RuleConfig {
            mode: RuleMode::None,
            selected_rules: vec![],
            custom_rules: None,
        })
    } else {
        RuleConfig {
            mode: RuleMode::None,
            selected_rules: vec![],
            custom_rules: None,
        }
    };

    let replay_id = format!("replay-{}", REPLAY_SEQUENCE.fetch_add(1, Ordering::SeqCst));

    info!(replay_id = %replay_id, url = %url, "Starting WebSocket proxy");

    let (_resolved_rules, matched_rules, applied_request) =
        resolve_and_apply_rules(&state, &rule_config, &url, "GET", &upgrade_headers, None);

    info!(
        replay_id = %replay_id,
        original_url = %url,
        applied_url = %applied_request.url,
        rules_count = matched_rules.len(),
        "[WS_REPLAY] Applied request rules"
    );

    let traffic_id =
        record_traffic_for_stream(&state, &replay_id, &applied_request, &matched_rules, false);

    if let Some(ref req_id) = request_id {
        record_history(
            &state,
            &push_manager,
            req_id,
            &traffic_id,
            "GET",
            &applied_request.url,
            200,
            0,
            &rule_config,
        );
    }

    let accept_key = generate_accept_key(&ws_key);
    let applied_url = applied_request.url.clone();

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

        match connect_websocket(&applied_url, &applied_request.headers).await {
            Ok(connection) => match connection {
                WebSocketConnection::Plain(server_ws) => {
                    proxy_websocket(
                        client_ws,
                        *server_ws,
                        &replay_id,
                        &traffic_id,
                        request_id,
                        &state,
                    )
                    .await;
                }
                WebSocketConnection::Tls(server_ws) => {
                    proxy_websocket(
                        client_ws,
                        *server_ws,
                        &replay_id,
                        &traffic_id,
                        request_id,
                        &state,
                    )
                    .await;
                }
            },
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

enum WebSocketConnection {
    Plain(Box<WebSocketStream<TcpStream>>),
    Tls(Box<WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>>),
}

async fn connect_websocket(
    url: &str,
    headers: &[(String, String)],
) -> Result<WebSocketConnection, String> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};

    let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

    let is_secure = match parsed_url.scheme() {
        "wss" | "https" => true,
        "ws" | "http" => false,
        scheme => return Err(format!("Unsupported scheme: {}", scheme)),
    };

    // tungstenite 的 client handshake 期望 ws/wss scheme；兼容用户传入 http/https
    let ws_url = if parsed_url.scheme() == "http" || parsed_url.scheme() == "https" {
        let mut new = parsed_url.clone();
        let scheme = if is_secure { "wss" } else { "ws" };
        new.set_scheme(scheme)
            .map_err(|_| format!("Failed to set scheme to {}", scheme))?;
        new
    } else {
        parsed_url.clone()
    };

    let host = parsed_url
        .host_str()
        .ok_or_else(|| "Missing host".to_string())?;
    let port = parsed_url
        .port()
        .unwrap_or(if is_secure { 443 } else { 80 });
    let addr = format!("{}:{}", host, port);

    debug!(url = %ws_url.as_str(), host = %host, port = %port, is_secure = %is_secure, "[WS_REPLAY] Connecting to WebSocket");

    let tcp_stream = TcpStream::connect(&addr)
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", addr, e))?;

    if is_secure {
        let tls_config = get_ws_tls_client_config();
        let connector = TlsConnector::from(Arc::new(tls_config));

        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| format!("Invalid server name: {}", e))?;

        let tls_stream = connector
            .connect(server_name, tcp_stream)
            .await
            .map_err(|e| format!("TLS handshake failed: {}", e))?;

        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|e| format!("WebSocket request build failed: {}", e))?;
        for (k, v) in headers {
            let name = k.to_lowercase();
            if matches!(
                name.as_str(),
                "host"
                    | "upgrade"
                    | "connection"
                    | "sec-websocket-key"
                    | "sec-websocket-version"
                    | "sec-websocket-extensions"
                    | "sec-websocket-protocol"
            ) {
                continue;
            }
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                request.headers_mut().insert(header_name, header_value);
            }
        }

        let (ws_stream, _) = tokio_tungstenite::client_async(request, tls_stream)
            .await
            .map_err(|e| format!("WebSocket handshake failed: {}", e))?;

        Ok(WebSocketConnection::Tls(Box::new(ws_stream)))
    } else {
        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|e| format!("WebSocket request build failed: {}", e))?;
        for (k, v) in headers {
            let name = k.to_lowercase();
            if matches!(
                name.as_str(),
                "host"
                    | "upgrade"
                    | "connection"
                    | "sec-websocket-key"
                    | "sec-websocket-version"
                    | "sec-websocket-extensions"
                    | "sec-websocket-protocol"
            ) {
                continue;
            }
            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(v),
            ) {
                request.headers_mut().insert(header_name, header_value);
            }
        }

        let (ws_stream, _) = tokio_tungstenite::client_async(request, tcp_stream)
            .await
            .map_err(|e| format!("WebSocket handshake failed: {}", e))?;

        Ok(WebSocketConnection::Plain(Box::new(ws_stream)))
    }
}

fn get_ws_tls_client_config() -> rustls::ClientConfig {
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

async fn proxy_websocket<S>(
    client_ws: WebSocketStream<hyper_util::rt::TokioIo<upgrade::Upgraded>>,
    server_ws: WebSocketStream<S>,
    replay_id: &str,
    traffic_id: &str,
    _request_id: Option<String>,
    state: &SharedAdminState,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
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

fn resolve_and_apply_rules(
    state: &SharedAdminState,
    rule_config: &RuleConfig,
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> (
    bifrost_core::ResolvedRules,
    Vec<MatchedRule>,
    AppliedRequest,
) {
    let (resolved_rules, matched_rules) = match rule_config.mode {
        RuleMode::None => (bifrost_core::ResolvedRules::default(), vec![]),
        RuleMode::Custom => {
            if let Some(ref custom_rules) = rule_config.custom_rules {
                resolve_custom_rules(state, custom_rules, url, method)
            } else {
                (bifrost_core::ResolvedRules::default(), vec![])
            }
        }
        RuleMode::Enabled | RuleMode::Selected => {
            let selected = if rule_config.mode == RuleMode::Selected {
                Some(&rule_config.selected_rules)
            } else {
                None
            };
            resolve_from_storage(state, url, method, selected)
        }
    };

    let rules_to_apply = build_applied_rules(&resolved_rules);

    let applied_request =
        match apply_all_request_rules(url, method, headers, body, &rules_to_apply, true) {
            Ok(req) => req,
            Err(e) => {
                warn!(error = %e, "[REPLAY] Failed to apply rules, using original request");
                AppliedRequest {
                    url: url.to_string(),
                    method: method.to_string(),
                    headers: headers.to_vec(),
                    body: body.map(Bytes::copy_from_slice),
                }
            }
        };

    (resolved_rules, matched_rules, applied_request)
}

fn extract_inline_content(value: &str) -> String {
    if value.starts_with('{') && value.ends_with('}') && value.len() > 1 {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn parse_headers(value: &str) -> Option<Vec<(String, String)>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (content, use_colon) = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        (&trimmed[1..trimmed.len() - 1], true)
    } else {
        (trimmed, trimmed.contains('\n') || trimmed.contains(':'))
    };

    let mut headers = Vec::new();
    let delimiter = if content.contains('\n') { '\n' } else { ',' };
    for part in content.split(delimiter) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let separator = if use_colon { ':' } else { '=' };
        if let Some(pos) = part.find(separator) {
            let key = part[..pos].trim().to_string();
            let val = part[pos + 1..].trim().to_string();
            if !key.is_empty() {
                headers.push((key, val));
            }
        }
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

fn apply_response_rules(
    resolved_rules: &bifrost_core::ResolvedRules,
    status: u16,
    mut headers: Vec<(String, String)>,
    body: Option<String>,
) -> (u16, Vec<(String, String)>, Option<String>) {
    use bifrost_core::Protocol;

    let mut final_status = status;
    let mut final_body = body;

    for rule in &resolved_rules.rules {
        match rule.rule.protocol {
            Protocol::ResHeaders => {
                if let Some(parsed) = parse_headers(&rule.resolved_value) {
                    for (key, value) in parsed {
                        let key_lower = key.to_lowercase();
                        headers.retain(|(k, _)| k.to_lowercase() != key_lower);
                        headers.push((key, value));
                    }
                }
            }
            Protocol::StatusCode | Protocol::ReplaceStatus => {
                if let Ok(code) = rule.resolved_value.parse::<u16>() {
                    final_status = code;
                }
            }
            Protocol::ResBody => {
                let content = extract_inline_content(&rule.resolved_value);
                final_body = Some(content);
            }
            _ => {}
        }
    }

    (final_status, headers, final_body)
}

fn resolve_custom_rules(
    state: &SharedAdminState,
    custom_rules: &str,
    url: &str,
    method: &str,
) -> (bifrost_core::ResolvedRules, Vec<MatchedRule>) {
    let rules = match parse_rules(custom_rules) {
        Ok(r) => r
            .into_iter()
            .enumerate()
            .map(|(i, r)| r.with_source("custom".to_string(), i + 1))
            .collect::<Vec<_>>(),
        Err(e) => {
            warn!(error = %e, "[REPLAY] Failed to parse custom rules");
            return (bifrost_core::ResolvedRules::default(), vec![]);
        }
    };

    if rules.is_empty() {
        return (bifrost_core::ResolvedRules::default(), vec![]);
    }

    let values = load_values(state);
    let resolver = RulesResolver::new(rules).with_values(values);
    let ctx = RequestContext::from_url(url).with_method(method);
    let resolved = resolver.resolve(&ctx);

    let matched: Vec<MatchedRule> = resolved
        .rules
        .iter()
        .map(|r| MatchedRule {
            pattern: r.rule.pattern.clone(),
            protocol: r.rule.protocol.to_str().to_string(),
            value: r.resolved_value.clone(),
            rule_name: r.rule.file.clone(),
            raw: Some(r.rule.raw.clone()),
            line: r.rule.line,
        })
        .collect();

    (resolved, matched)
}

fn resolve_from_storage(
    state: &SharedAdminState,
    url: &str,
    method: &str,
    selected_rules: Option<&Vec<String>>,
) -> (bifrost_core::ResolvedRules, Vec<MatchedRule>) {
    let rules_storage = &state.rules_storage;
    let mut all_rules: Vec<Rule> = vec![];

    let rule_files = match rules_storage.load_all() {
        Ok(files) => files,
        Err(e) => {
            warn!(error = %e, "[REPLAY] Failed to load rules");
            return (bifrost_core::ResolvedRules::default(), vec![]);
        }
    };

    for rule_file in rule_files {
        if !rule_file.enabled {
            continue;
        }

        if let Some(selected) = selected_rules {
            if !selected.contains(&rule_file.name) {
                continue;
            }
        }

        if let Ok(parsed) = parse_rules(&rule_file.content) {
            let rules_with_source: Vec<Rule> = parsed
                .into_iter()
                .enumerate()
                .map(|(i, r)| r.with_source(rule_file.name.clone(), i + 1))
                .collect();
            all_rules.extend(rules_with_source);
        }
    }

    if all_rules.is_empty() {
        return (bifrost_core::ResolvedRules::default(), vec![]);
    }

    let values = load_values(state);
    let resolver = RulesResolver::new(all_rules).with_values(values);
    let ctx = RequestContext::from_url(url).with_method(method);
    let resolved = resolver.resolve(&ctx);

    let matched: Vec<MatchedRule> = resolved
        .rules
        .iter()
        .map(|r| MatchedRule {
            pattern: r.rule.pattern.clone(),
            protocol: r.rule.protocol.to_str().to_string(),
            value: r.resolved_value.clone(),
            rule_name: r.rule.file.clone(),
            raw: Some(r.rule.raw.clone()),
            line: r.rule.line,
        })
        .collect();

    (resolved, matched)
}

fn load_values(state: &SharedAdminState) -> HashMap<String, String> {
    if let Some(ref values_storage) = state.values_storage {
        let guard = values_storage.read();
        return guard.as_hashmap();
    }
    HashMap::new()
}

fn record_traffic_for_stream(
    state: &SharedAdminState,
    replay_id: &str,
    applied_request: &AppliedRequest,
    matched_rules: &[MatchedRule],
    is_sse: bool,
) -> String {
    let traffic_id = format!("{}-{}", replay_id, uuid::Uuid::new_v4());
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    let uri: Uri = applied_request.url.parse().unwrap_or_default();
    let host = uri.host().unwrap_or("unknown").to_string();
    let path = uri.path().to_string();
    let scheme = uri.scheme_str().unwrap_or("http");

    let request_content_type = applied_request
        .headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "content-type")
        .map(|(_, v)| v.clone());

    let request_body_ref = if let Some(ref body) = applied_request.body {
        if let Some(ref body_store) = state.body_store {
            body_store.read().store(&traffic_id, "req", body)
        } else {
            None
        }
    } else {
        None
    };

    let record = TrafficRecord {
        id: traffic_id.clone(),
        sequence: 0,
        timestamp,
        host,
        method: applied_request.method.clone(),
        url: applied_request.url.clone(),
        path,
        status: 200,
        protocol: scheme.to_string(),
        content_type: None,
        request_content_type,
        request_size: applied_request.body.as_ref().map(|b| b.len()).unwrap_or(0),
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
        has_rule_hit: !matched_rules.is_empty(),
        is_replay: true,
        frame_count: 0,
        last_frame_id: 0,
        timing: None,
        request_headers: Some(applied_request.headers.clone()),
        response_headers: None,
        matched_rules: if matched_rules.is_empty() {
            None
        } else {
            Some(matched_rules.to_vec())
        },
        socket_status: None,
        request_body_ref,
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

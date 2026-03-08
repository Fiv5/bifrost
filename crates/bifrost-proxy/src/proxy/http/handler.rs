use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use bifrost_admin::{AdminState, RequestTiming, TrafficRecord, TrafficType};
use bifrost_core::{protocol::Protocol, BifrostError, Result};
use bifrost_script::{MatchedRuleInfo, RequestData, ResponseData, ScriptContext, ScriptType};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::header::HeaderValue;
use hyper::http::response::Parts as ResponseParts;
use hyper::{Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::dns::DnsResolver;

use super::tunnel::get_tls_client_config;
use crate::server::{full_body, with_trailers, BoxBody, ResolvedRules, RulesResolver};
use crate::transform::apply_req_rules;
use crate::transform::apply_res_rules;
use crate::transform::{apply_body_rules, apply_content_injection, Phase};
use crate::transform::{decompress_body, get_content_encoding};
use crate::utils::http_size::{
    calculate_request_size, calculate_response_headers_size, calculate_response_size,
};
use crate::utils::logging::{format_rules_detail, format_rules_summary, RequestContext};
use crate::utils::mock::{generate_mock_response, should_intercept_response};
use crate::utils::tee::{
    create_request_tee_body, create_sse_tee_body, create_tee_body_with_store, store_request_body,
    store_response_body, BodyCaptureHandle,
};
use crate::utils::throttle::wrap_throttled_body;
use crate::utils::url::apply_url_rules;

const STREAMING_CONTENT_TYPES: &[&str] = &[
    "video/x-flv",
    "video/mp4",
    "video/webm",
    "video/ogg",
    "video/mpeg",
    "video/mp2t",
    "application/x-mpegurl",
    "application/vnd.apple.mpegurl",
    "application/dash+xml",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "audio/aac",
    "text/event-stream",
    "application/octet-stream",
];

trait AsyncReadWrite: tokio::io::AsyncRead + tokio::io::AsyncWrite {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite> AsyncReadWrite for T {}

fn get_traffic_type_from_url(url: &str) -> TrafficType {
    if url.starts_with("https://") {
        TrafficType::Https
    } else {
        TrafficType::Http
    }
}

fn get_content_type(res_parts: &ResponseParts) -> String {
    res_parts
        .headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase()
}

fn is_sse_response(res_parts: &ResponseParts) -> bool {
    get_content_type(res_parts).starts_with("text/event-stream")
}

fn is_streaming_response(res_parts: &ResponseParts) -> bool {
    let content_type_lower = get_content_type(res_parts);

    for streaming_type in STREAMING_CONTENT_TYPES {
        if content_type_lower.starts_with(streaming_type) {
            return true;
        }
    }

    let has_content_length = res_parts
        .headers
        .contains_key(hyper::header::CONTENT_LENGTH);
    let is_chunked = res_parts
        .headers
        .get(hyper::header::TRANSFER_ENCODING)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("chunked"))
        .unwrap_or(false);

    if !has_content_length && is_chunked && content_type_lower.contains("video") {
        return true;
    }

    false
}

pub fn needs_body_processing(rules: &ResolvedRules) -> bool {
    rules.res_body.is_some()
        || !rules.res_replace.is_empty()
        || !rules.res_replace_regex.is_empty()
        || rules.res_prepend.is_some()
        || rules.res_append.is_some()
        || rules.res_merge.is_some()
        || rules.html_append.is_some()
        || rules.html_prepend.is_some()
        || rules.html_body.is_some()
        || rules.js_append.is_some()
        || rules.js_prepend.is_some()
        || rules.js_body.is_some()
        || rules.css_append.is_some()
        || rules.css_prepend.is_some()
        || rules.css_body.is_some()
}

pub fn needs_response_override(rules: &ResolvedRules) -> bool {
    rules.res_body.is_some() || rules.status_code.is_some() || rules.replace_status.is_some()
}

enum BodyMode {
    Known(usize),
    Stream,
    StreamWithTrailers,
}

fn is_no_body_response(status: StatusCode, method: &str) -> bool {
    status.is_informational()
        || status == StatusCode::NO_CONTENT
        || status == StatusCode::NOT_MODIFIED
        || method.eq_ignore_ascii_case("HEAD")
}

fn normalize_req_headers(parts: &mut hyper::http::request::Parts, mode: BodyMode) {
    match mode {
        BodyMode::Known(len) => {
            parts.headers.remove(hyper::header::TRANSFER_ENCODING);
            parts.headers.remove(hyper::header::CONTENT_LENGTH);
            parts.headers.insert(
                hyper::header::CONTENT_LENGTH,
                HeaderValue::from_str(&len.to_string()).unwrap(),
            );
        }
        BodyMode::Stream | BodyMode::StreamWithTrailers => {
            parts.headers.remove(hyper::header::TRANSFER_ENCODING);
            parts.headers.remove(hyper::header::CONTENT_LENGTH);
        }
    }
}

fn normalize_res_headers(parts: &mut ResponseParts, mode: BodyMode, method: &str) {
    if is_no_body_response(parts.status, method) {
        parts.headers.remove(hyper::header::TRANSFER_ENCODING);
        parts.headers.remove(hyper::header::CONTENT_LENGTH);
        return;
    }
    match mode {
        BodyMode::Known(len) => {
            parts.headers.remove(hyper::header::TRANSFER_ENCODING);
            parts.headers.remove(hyper::header::CONTENT_LENGTH);
            parts.headers.insert(
                hyper::header::CONTENT_LENGTH,
                HeaderValue::from_str(&len.to_string()).unwrap(),
            );
        }
        BodyMode::Stream | BodyMode::StreamWithTrailers => {
            parts.headers.remove(hyper::header::TRANSFER_ENCODING);
            parts.headers.remove(hyper::header::CONTENT_LENGTH);
        }
    }
}

pub struct ConnectionErrorInfo {
    pub error_type: &'static str,
    pub error_message: String,
    pub host: String,
    pub request_url: String,
}

fn build_error_body(status_code: u16, error_info: &ConnectionErrorInfo) -> Bytes {
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let now = chrono::Local::now();
    let date_str = now.format("%m/%d/%Y, %I:%M:%S %p").to_string();

    Bytes::from(format!(
        "Status: {}\nError: {}\nFrom: Bifrost@{}\nHost: {}\nDate: {}\nURL: {}",
        status_code,
        error_info.error_message,
        hostname,
        error_info.host,
        date_str,
        error_info.request_url,
    ))
}

pub fn build_connection_error_response(
    status_code: u16,
    error_info: &ConnectionErrorInfo,
) -> Response<BoxBody> {
    let hostname = gethostname::gethostname().to_string_lossy().to_string();
    let now = chrono::Local::now();
    let date_str = now.format("%m/%d/%Y, %I:%M:%S %p").to_string();

    let body = format!(
        "Status: {}\nError: {}\nFrom: Bifrost@{}\nHost: {}\nDate: {}\nURL: {}",
        status_code,
        error_info.error_message,
        hostname,
        error_info.host,
        date_str,
        error_info.request_url,
    );

    Response::builder()
        .status(hyper::StatusCode::from_u16(status_code).unwrap_or(hyper::StatusCode::BAD_GATEWAY))
        .header(hyper::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header("X-Bifrost-Error", error_info.error_type)
        .body(full_body(body.into_bytes()))
        .unwrap()
}

pub fn build_overridden_error_response(
    rules: &ResolvedRules,
    default_status: u16,
    error_info: &ConnectionErrorInfo,
) -> Response<BoxBody> {
    let status_code = rules
        .status_code
        .or(rules.replace_status)
        .unwrap_or(default_status);

    let body = if let Some(ref res_body) = rules.res_body {
        res_body.clone()
    } else {
        let hostname = gethostname::gethostname().to_string_lossy().to_string();
        let now = chrono::Local::now();
        let date_str = now.format("%m/%d/%Y, %I:%M:%S %p").to_string();

        let body_str = format!(
            "Status: {}\nError: {}\nFrom: Bifrost@{}\nHost: {}\nDate: {}\nURL: {}",
            status_code,
            error_info.error_message,
            hostname,
            error_info.host,
            date_str,
            error_info.request_url,
        );
        Bytes::from(body_str)
    };

    let mut response = Response::builder()
        .status(hyper::StatusCode::from_u16(status_code).unwrap_or(hyper::StatusCode::BAD_GATEWAY));

    for (name, value) in &rules.res_headers {
        if let (Ok(header_name), Ok(header_value)) = (
            hyper::header::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            response = response.header(header_name, header_value);
        }
    }

    if rules.res_body.is_none() {
        response = response.header(hyper::header::CONTENT_TYPE, "text/plain; charset=utf-8");
        response = response.header("X-Bifrost-Error", error_info.error_type);
    }

    response.body(full_body(body.to_vec())).unwrap()
}

pub fn needs_request_body_processing(rules: &ResolvedRules) -> bool {
    rules.req_body.is_some()
        || rules.req_prepend.is_some()
        || rules.req_append.is_some()
        || !rules.req_replace.is_empty()
        || !rules.req_replace_regex.is_empty()
        || rules.req_merge.is_some()
}

fn build_matched_rules_info(resolved_rules: &ResolvedRules) -> Vec<MatchedRuleInfo> {
    resolved_rules
        .rules
        .iter()
        .map(|r| MatchedRuleInfo {
            pattern: r.pattern.clone(),
            protocol: r.protocol.to_string(),
            value: r.value.clone(),
        })
        .collect()
}

fn headers_to_hashmap(headers: &[(String, String)]) -> HashMap<String, String> {
    headers.iter().cloned().collect()
}

#[allow(clippy::too_many_arguments)]
async fn execute_request_scripts(
    admin_state: &Option<Arc<AdminState>>,
    script_names: &[String],
    ctx: &RequestContext,
    resolved_rules: &ResolvedRules,
    url: &str,
    method: &mut String,
    headers: &mut HashMap<String, String>,
    body: &mut Option<String>,
    values: &HashMap<String, String>,
) -> Vec<bifrost_script::ScriptExecutionResult> {
    if script_names.is_empty() {
        return vec![];
    }

    let state = match admin_state {
        Some(s) => s,
        None => return vec![],
    };

    let manager = match &state.script_manager {
        Some(m) => m,
        None => return vec![],
    };

    let matched_rules = build_matched_rules_info(resolved_rules);
    let (host, path, protocol) = parse_url_parts(url);

    let mut request_data = RequestData {
        url: url.to_string(),
        method: method.clone(),
        host,
        path,
        protocol,
        client_ip: ctx.client_ip.clone(),
        client_app: ctx.client_app.clone(),
        headers: headers.clone(),
        body: body.clone(),
    };

    let script_ctx = ScriptContext {
        request_id: ctx.id_str().to_string(),
        script_name: script_names.first().cloned().unwrap_or_default(),
        script_type: ScriptType::Request,
        values: values.clone(),
        matched_rules,
    };

    let mgr = manager.read().await;
    let results = mgr
        .execute_request_scripts(script_names, &mut request_data, &script_ctx)
        .await;

    if results.iter().any(|r| r.success) {
        *method = request_data.method;
        *headers = request_data.headers;
        *body = request_data.body;
    }

    results
}

#[allow(clippy::too_many_arguments)]
async fn execute_response_scripts(
    admin_state: &Option<Arc<AdminState>>,
    script_names: &[String],
    ctx: &RequestContext,
    resolved_rules: &ResolvedRules,
    request_url: &str,
    request_method: &str,
    request_headers: &HashMap<String, String>,
    status: &mut u16,
    status_text: &mut String,
    headers: &mut HashMap<String, String>,
    body: &mut Option<String>,
    values: &HashMap<String, String>,
) -> Vec<bifrost_script::ScriptExecutionResult> {
    if script_names.is_empty() {
        return vec![];
    }

    let state = match admin_state {
        Some(s) => s,
        None => return vec![],
    };

    let manager = match &state.script_manager {
        Some(m) => m,
        None => return vec![],
    };

    let matched_rules = build_matched_rules_info(resolved_rules);
    let (host, path, protocol) = parse_url_parts(request_url);

    let mut response_data = ResponseData {
        status: *status,
        status_text: status_text.clone(),
        headers: headers.clone(),
        body: body.clone(),
        request: RequestData {
            url: request_url.to_string(),
            method: request_method.to_string(),
            host,
            path,
            protocol,
            client_ip: ctx.client_ip.clone(),
            client_app: ctx.client_app.clone(),
            headers: request_headers.clone(),
            body: None,
        },
    };

    let script_ctx = ScriptContext {
        request_id: ctx.id_str().to_string(),
        script_name: script_names.first().cloned().unwrap_or_default(),
        script_type: ScriptType::Response,
        values: values.clone(),
        matched_rules,
    };

    let mgr = manager.read().await;
    let results = mgr
        .execute_response_scripts(script_names, &mut response_data, &script_ctx)
        .await;

    if results.iter().any(|r| r.success) {
        *status = response_data.status;
        *status_text = response_data.status_text;
        *headers = response_data.headers;
        *body = response_data.body;
    }

    results
}

fn parse_url_parts(url: &str) -> (String, String, String) {
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("").to_string();
        let path = parsed.path().to_string();
        let protocol = parsed.scheme().to_string();
        (host, path, protocol)
    } else {
        ("".to_string(), url.to_string(), "http".to_string())
    }
}

async fn get_values_from_state(admin_state: &Option<Arc<AdminState>>) -> HashMap<String, String> {
    use bifrost_core::ValueStore;
    if let Some(state) = admin_state {
        if let Some(values_storage) = &state.values_storage {
            let storage = values_storage.read();
            return storage.as_hashmap();
        }
    }
    HashMap::new()
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_http_request(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    unsafe_ssl: bool,
    max_body_buffer_size: usize,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Option<Arc<DnsResolver>>,
) -> Result<Response<BoxBody>> {
    if is_websocket_upgrade(&req) {
        return handle_http_websocket(req, rules, ctx, admin_state, unsafe_ssl).await;
    }

    let uri = req.uri().clone();
    let method = req.method().to_string();
    let url = uri.to_string();
    let record_url = if ctx.url.is_empty() {
        url.clone()
    } else {
        ctx.url.clone()
    };
    let start_time = std::time::Instant::now();

    let resolved_rules = rules.resolve(&url, &method);

    let has_rules = !resolved_rules.rules.is_empty()
        || resolved_rules.host.is_some()
        || resolved_rules.proxy.is_some()
        || !resolved_rules.req_headers.is_empty()
        || !resolved_rules.res_headers.is_empty()
        || resolved_rules.status_code.is_some()
        || should_intercept_response(&resolved_rules);

    if verbose_logging {
        if has_rules {
            info!(
                "[{}] [RULES] matched: {}",
                ctx.id_str(),
                format_rules_summary(&resolved_rules)
            );
            debug!(
                "[{}] [RULES] details:\n{}",
                ctx.id_str(),
                format_rules_detail(&resolved_rules)
            );
        } else {
            debug!("[{}] [RULES] none matched", ctx.id_str());
        }
    }

    if let Some(mock_response) =
        generate_mock_response(&resolved_rules, &uri, verbose_logging, ctx).await
    {
        if verbose_logging {
            info!("[{}] [MOCK] returning mock response", ctx.id_str());
        }
        return Ok(mock_response);
    }

    if let Some(ref redirect_url) = resolved_rules.redirect {
        let status = resolved_rules.redirect_status.unwrap_or(302);
        if verbose_logging {
            info!(
                "[{}] [REDIRECT] {} -> {} ({})",
                ctx.id_str(),
                url,
                redirect_url,
                status
            );
        }
        return Ok(build_redirect_response(status, redirect_url));
    }

    if let Some(ref location) = resolved_rules.location_href {
        if verbose_logging {
            info!("[{}] [LOCATION] {} -> {}", ctx.id_str(), url, location);
        }
        return Ok(build_redirect_response(301, location));
    }

    let processed_uri = apply_url_rules(&uri, &resolved_rules, verbose_logging, ctx);

    let original_host = uri.host().unwrap_or("unknown").to_string();
    let is_https = uri.scheme_str() == Some("https") || uri.scheme_str() == Some("wss");
    let default_port = if is_https { 443 } else { 80 };
    let original_port = uri.port_u16().unwrap_or(default_port);
    let (host, port) = extract_host_port(&processed_uri, &resolved_rules, is_https)?;

    if verbose_logging {
        if resolved_rules.host.is_some() {
            info!(
                "[{}] [FORWARD] {}:{} -> {}:{} (redirected by host rule)",
                ctx.id_str(),
                original_host,
                original_port,
                host,
                port
            );
        } else {
            info!("[{}] [FORWARD] {}:{}", ctx.id_str(), host, port);
        }
    } else {
        debug!("Proxying HTTP request to {}:{}", host, port);
    }

    let (mut parts, body) = req.into_parts();

    let original_req_headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let req_content_encoding = get_content_encoding(&original_req_headers);

    apply_req_rules(&mut parts, &resolved_rules, verbose_logging, ctx);

    let mut req_headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let content_length = parts
        .headers
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());

    let needs_req_processing = needs_request_body_processing(&resolved_rules);
    let has_req_body_override = resolved_rules.req_body.is_some();
    let has_req_scripts = !resolved_rules.req_scripts.is_empty();
    let needs_req_body_read = !has_req_body_override && (needs_req_processing || has_req_scripts);
    let req_body_too_large = content_length
        .map(|len| len > max_body_buffer_size)
        .unwrap_or(false);

    let mut skip_req_scripts = false;
    let mut streaming_body: Option<BoxBody> = None;
    let mut req_body_capture: Option<BodyCaptureHandle> = None;
    let (body_bytes, final_body) = if needs_req_body_read {
        if req_body_too_large {
            warn!(
                "[{}] [REQ_BODY] body too large ({} bytes > {} limit), skipping body rules and scripts",
                ctx.id_str(),
                content_length.unwrap(),
                max_body_buffer_size
            );
            skip_req_scripts = true;
            if admin_state.is_some() {
                let (tee_body, capture) =
                    create_request_tee_body(body, admin_state.clone(), ctx.id_str().to_string());
                streaming_body = Some(tee_body.boxed());
                req_body_capture = Some(capture);
            } else {
                streaming_body = Some(body.boxed());
            }
            (Bytes::new(), Bytes::new())
        } else {
            let bytes = body
                .collect()
                .await
                .map_err(|e| BifrostError::Network(format!("Failed to read request body: {}", e)))?
                .to_bytes();
            let req_content_type = parts
                .headers
                .get(hyper::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok());
            let processed = apply_body_rules(
                bytes.clone(),
                &resolved_rules,
                Phase::Request,
                req_content_type,
                verbose_logging,
                ctx,
            );
            (bytes, processed)
        }
    } else if let Some(ref new_body) = resolved_rules.req_body {
        if verbose_logging {
            info!(
                "[{}] [REQ_BODY] replaced: {} bytes -> {} bytes",
                ctx.id_str(),
                content_length.unwrap_or(0),
                new_body.len()
            );
        }
        let mut body = body;
        while let Some(frame) = body.frame().await {
            if frame.is_err() {
                break;
            }
        }
        (Bytes::new(), new_body.clone())
    } else {
        if admin_state.is_some() {
            let (tee_body, capture) =
                create_request_tee_body(body, admin_state.clone(), ctx.id_str().to_string());
            streaming_body = Some(tee_body.boxed());
            req_body_capture = Some(capture);
        } else {
            streaming_body = Some(body.boxed());
        }
        (Bytes::new(), Bytes::new())
    };
    let state_values = get_values_from_state(&admin_state).await;
    let mut values = resolved_rules.values.clone();
    for (k, v) in state_values {
        values.entry(k).or_insert(v);
    }

    let mut script_method = method.clone();
    let mut script_headers = headers_to_hashmap(&req_headers);
    let mut script_body = if has_req_scripts && !skip_req_scripts && !final_body.is_empty() {
        String::from_utf8(final_body.to_vec()).ok()
    } else {
        None
    };

    let req_script_results = execute_request_scripts(
        &admin_state,
        if skip_req_scripts {
            &[]
        } else {
            &resolved_rules.req_scripts
        },
        ctx,
        &resolved_rules,
        &url,
        &mut script_method,
        &mut script_headers,
        &mut script_body,
        &values,
    )
    .await;

    if !req_script_results.is_empty() && req_script_results.iter().any(|r| r.success) {
        if let Ok(new_method) = script_method.parse() {
            parts.method = new_method;
        }

        let mut new_headers = hyper::HeaderMap::new();
        for (key, value) in &script_headers {
            if let (Ok(name), Ok(val)) = (
                hyper::header::HeaderName::from_bytes(key.as_bytes()),
                hyper::header::HeaderValue::from_str(value),
            ) {
                new_headers.insert(name, val);
            }
        }
        parts.headers = new_headers;

    }

    let final_body =
        if !req_script_results.is_empty() && req_script_results.iter().any(|r| r.success) {
            if let Some(ref new_body) = script_body {
                Bytes::from(new_body.clone())
            } else {
                final_body
            }
        } else {
            final_body
        };
    let req_body_mode = if streaming_body.is_some() {
        BodyMode::Stream
    } else {
        BodyMode::Known(final_body.len())
    };
    normalize_req_headers(&mut parts, req_body_mode);
    req_headers = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let request_body_size = if !final_body.is_empty() {
        final_body.len()
    } else {
        content_length.unwrap_or(0)
    };
    let outgoing_body = match streaming_body {
        Some(body) => body,
        None => full_body(final_body.clone()),
    };
    let outgoing_body = wrap_throttled_body(outgoing_body, resolved_rules.req_speed);

    let dns_start = Instant::now();
    let (connect_host, dns_ms, dns_error) = if !resolved_rules.dns_servers.is_empty() {
        if let Some(ref resolver) = dns_resolver {
            if verbose_logging {
                info!(
                    "[{}] [DNS] resolving {} with custom servers: {:?}",
                    ctx.id_str(),
                    host,
                    resolved_rules.dns_servers
                );
            }
            match resolver.resolve(&host, &resolved_rules.dns_servers).await {
                Ok(Some(ip)) => {
                    let elapsed = dns_start.elapsed().as_millis() as u64;
                    if verbose_logging {
                        info!(
                            "[{}] [DNS] resolved {} -> {} ({}ms)",
                            ctx.id_str(),
                            host,
                            ip,
                            elapsed
                        );
                    }
                    (ip.to_string(), Some(elapsed), None)
                }
                Ok(None) => {
                    debug!(
                        "[{}] [DNS] custom DNS returned None, using original host",
                        ctx.id_str()
                    );
                    (host.clone(), None, None)
                }
                Err(e) => {
                    debug!(
                        "[{}] [DNS] custom DNS failed: {}, using original host",
                        ctx.id_str(),
                        e
                    );
                    (host.clone(), None, Some(e.to_string()))
                }
            }
        } else {
            (host.clone(), None, None)
        }
    } else {
        (host.clone(), None, None)
    };

    let connect_start = Instant::now();
    let stream = match TcpStream::connect(format!("{}:{}", connect_host, port)).await {
        Ok(s) => s,
        Err(e) => {
            let (error_type, error_message) = if let Some(ref dns_err) = dns_error {
                (
                    "DNS_LOOKUP_FAILED",
                    format!("DNS Lookup Failed: {}", dns_err),
                )
            } else {
                ("TCP_CONNECTION_FAILED", format!("Connection Failed: {}", e))
            };
            let error_msg = if let Some(ref dns_err) = dns_error {
                format!("DNS lookup failed for {}: {}", host, dns_err)
            } else {
                format!("Failed to connect to {}:{}: {}", connect_host, port, e)
            };
            error!("[{}] {}", ctx.id_str(), error_msg);

            let error_info = ConnectionErrorInfo {
                error_type,
                error_message,
                host: host.clone(),
                request_url: url.clone(),
            };

            let total_ms = start_time.elapsed().as_millis() as u64;
            if let Some(ref state) = admin_state {
                let mut record = TrafficRecord::new(
                    ctx.id_str().to_string(),
                    method.clone(),
                    record_url.clone(),
                );
                record.status = if needs_response_override(&resolved_rules) {
                    resolved_rules
                        .status_code
                        .or(resolved_rules.replace_status)
                        .unwrap_or(502)
                } else {
                    502
                };
                record.duration_ms = total_ms;
                record.host = original_host.clone();
                record.timing = Some(RequestTiming {
                    dns_ms,
                    connect_ms: Some(connect_start.elapsed().as_millis() as u64),
                    tls_ms: None,
                    send_ms: None,
                    wait_ms: None,
                    receive_ms: None,
                    total_ms,
                });
                record.original_request_headers = Some(original_req_headers.clone());
                record.has_rule_hit = has_rules;
                record.matched_rules = crate::utils::build_matched_rules(&resolved_rules);
                record.error_message = Some(error_msg);
                record.request_body_ref = if let Some(ref capture) = req_body_capture {
                    capture.take()
                } else {
                    store_request_body(
                        &admin_state,
                        &ctx.id_str(),
                        &final_body,
                        req_content_encoding.as_deref(),
                    )
                };

                let response_body = if needs_response_override(&resolved_rules) {
                    if let Some(ref res_body) = resolved_rules.res_body {
                        res_body.clone()
                    } else {
                        build_error_body(record.status, &error_info)
                    }
                } else {
                    build_error_body(502, &error_info)
                };
                record.response_body_ref =
                    store_response_body(&admin_state, &ctx.id_str(), &response_body);
                state.record_traffic(record);
            }

            if needs_response_override(&resolved_rules) {
                if verbose_logging {
                    info!(
                        "[{}] [CONN_ERROR] {} failed, applying response override rules",
                        ctx.id_str(),
                        error_type
                    );
                }
                return Ok(build_overridden_error_response(
                    &resolved_rules,
                    502,
                    &error_info,
                ));
            }
            return Ok(build_connection_error_response(502, &error_info));
        }
    };
    let tcp_connect_ms = connect_start.elapsed().as_millis() as u64;

    if let Err(e) = stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY on HTTP connection: {}", e);
    }

    let use_tls = if resolved_rules.ignored.host {
        is_https
    } else {
        match resolved_rules.host_protocol {
            Some(Protocol::Http) | Some(Protocol::Ws) => false,
            Some(Protocol::Https) | Some(Protocol::Wss) => true,
            Some(Protocol::Host) | Some(Protocol::XHost) => port == 443 || port == 8443,
            _ => is_https,
        }
    };

    let build_conn_error_and_record =
        |error_type: &'static str, error_msg: String, err_tls_ms: Option<u64>| {
            let (final_error_type, final_error_message) = if let Some(ref dns_err) = dns_error {
                (
                    "DNS_LOOKUP_FAILED",
                    format!("DNS Lookup Failed: {}", dns_err),
                )
            } else {
                (error_type, error_msg.clone())
            };
            let final_error_msg = if let Some(ref dns_err) = dns_error {
                format!("DNS lookup failed for {}: {}", host, dns_err)
            } else {
                error_msg
            };

            let error_info = ConnectionErrorInfo {
                error_type: final_error_type,
                error_message: final_error_message,
                host: host.clone(),
                request_url: url.clone(),
            };
            let total_ms = start_time.elapsed().as_millis() as u64;
            if let Some(ref state) = admin_state {
                let mut record = TrafficRecord::new(
                    ctx.id_str().to_string(),
                    method.clone(),
                    record_url.clone(),
                );
                record.status = if needs_response_override(&resolved_rules) {
                    resolved_rules
                        .status_code
                        .or(resolved_rules.replace_status)
                        .unwrap_or(502)
                } else {
                    502
                };
                record.duration_ms = total_ms;
                record.host = original_host.clone();
                record.timing = Some(RequestTiming {
                    dns_ms,
                    connect_ms: Some(tcp_connect_ms),
                    tls_ms: err_tls_ms,
                    send_ms: None,
                    wait_ms: None,
                    receive_ms: None,
                    total_ms,
                });
                record.original_request_headers = Some(original_req_headers.clone());
                record.has_rule_hit = has_rules;
                record.matched_rules = crate::utils::build_matched_rules(&resolved_rules);
                record.error_message = Some(final_error_msg);
                record.request_body_ref = if let Some(ref capture) = req_body_capture {
                    capture.take()
                } else {
                    store_request_body(
                        &admin_state,
                        &ctx.id_str(),
                        &final_body,
                        req_content_encoding.as_deref(),
                    )
                };

                let response_body = if needs_response_override(&resolved_rules) {
                    if let Some(ref res_body) = resolved_rules.res_body {
                        res_body.clone()
                    } else {
                        build_error_body(record.status, &error_info)
                    }
                } else {
                    build_error_body(502, &error_info)
                };
                record.response_body_ref =
                    store_response_body(&admin_state, &ctx.id_str(), &response_body);
                state.record_traffic(record);
            }
            if needs_response_override(&resolved_rules) {
                if verbose_logging {
                    info!(
                        "[{}] [CONN_ERROR] {}, applying response override rules",
                        ctx.id_str(),
                        final_error_type
                    );
                }
                build_overridden_error_response(&resolved_rules, 502, &error_info)
            } else {
                build_connection_error_response(502, &error_info)
            }
        };

    let (mut sender, tls_ms) = if use_tls {
        let tls_start = Instant::now();
        let tls_config = get_tls_client_config(unsafe_ssl);
        let connector = TlsConnector::from(tls_config);

        let server_name = match ServerName::try_from(host.clone()) {
            Ok(name) => name,
            Err(_) => {
                let error_msg = format!("Invalid server name for TLS: {}", host);
                error!("[{}] {}", ctx.id_str(), error_msg);
                return Ok(build_conn_error_and_record(
                    "TLS_SERVER_NAME_INVALID",
                    error_msg,
                    None,
                ));
            }
        };

        let tls_stream = match connector.connect(server_name, stream).await {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("TLS handshake failed: {}", e);
                error!("[{}] {}", ctx.id_str(), error_msg);
                let tls_ms = tls_start.elapsed().as_millis() as u64;
                return Ok(build_conn_error_and_record(
                    "TLS_HANDSHAKE_FAILED",
                    error_msg,
                    Some(tls_ms),
                ));
            }
        };
        let tls_elapsed = tls_start.elapsed().as_millis() as u64;

        let io = TokioIo::new(tls_stream);
        let (sender, conn) = match ClientBuilder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake(io)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("HTTP handshake failed: {}", e);
                error!("[{}] {}", ctx.id_str(), error_msg);
                return Ok(build_conn_error_and_record(
                    "HTTP_HANDSHAKE_FAILED",
                    error_msg,
                    Some(tls_elapsed),
                ));
            }
        };

        let req_id_for_conn = ctx.id_str();
        let host_for_conn = host.clone();
        let port_for_conn = port;
        let method_for_conn = method.clone();
        let url_for_conn = url.clone();
        let client_ip_for_conn = ctx.client_ip.clone();
        let client_app_for_conn = ctx.client_app.clone();
        let client_pid_for_conn = ctx.client_pid;
        let client_path_for_conn = ctx.client_path.clone();
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                error!(
                    "[{}] Connection failed to {}:{} method={} url={} client_ip={} client_app={:?} client_pid={:?} client_path={:?} error={:?}",
                    req_id_for_conn,
                    host_for_conn,
                    port_for_conn,
                    method_for_conn,
                    url_for_conn,
                    client_ip_for_conn,
                    client_app_for_conn,
                    client_pid_for_conn,
                    client_path_for_conn,
                    err
                );
            }
        });

        (sender, Some(tls_elapsed))
    } else {
        let io = TokioIo::new(stream);
        let (sender, conn) = match ClientBuilder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake(io)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!("HTTP handshake failed: {}", e);
                error!("[{}] {}", ctx.id_str(), error_msg);
                return Ok(build_conn_error_and_record(
                    "HTTP_HANDSHAKE_FAILED",
                    error_msg,
                    None,
                ));
            }
        };

        let req_id_for_conn = ctx.id_str();
        let host_for_conn = host.clone();
        let port_for_conn = port;
        let method_for_conn = method.clone();
        let url_for_conn = url.clone();
        let client_ip_for_conn = ctx.client_ip.clone();
        let client_app_for_conn = ctx.client_app.clone();
        let client_pid_for_conn = ctx.client_pid;
        let client_path_for_conn = ctx.client_path.clone();
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                error!(
                    "[{}] Connection failed to {}:{} method={} url={} client_ip={} client_app={:?} client_pid={:?} client_path={:?} error={:?}",
                    req_id_for_conn,
                    host_for_conn,
                    port_for_conn,
                    method_for_conn,
                    url_for_conn,
                    client_ip_for_conn,
                    client_app_for_conn,
                    client_pid_for_conn,
                    client_path_for_conn,
                    err
                );
            }
        });

        (sender, None)
    };

    let path = processed_uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let new_uri: Uri = path
        .parse()
        .map_err(|e| BifrostError::Network(format!("Invalid URI: {}", e)))?;

    parts.uri = new_uri;

    if !parts.headers.contains_key(hyper::header::HOST) {
        let host_value = if port == 80 {
            host.clone()
        } else {
            format!("{}:{}", host, port)
        };
        parts
            .headers
            .insert(hyper::header::HOST, host_value.parse().unwrap());
    }

    let outgoing_req = Request::from_parts(parts, outgoing_body);

    let send_start = Instant::now();
    let res = match sender.send_request(outgoing_req).await {
        Ok(r) => r,
        Err(e) => {
            let error_msg = format!("Request failed: {}", e);
            error!("[{}] {}", ctx.id_str(), error_msg);
            return Ok(build_conn_error_and_record(
                "REQUEST_FAILED",
                error_msg,
                tls_ms,
            ));
        }
    };
    let wait_ms = send_start.elapsed().as_millis() as u64;

    let (mut res_parts, res_body) = res.into_parts();

    let original_res_headers: Vec<(String, String)> = res_parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let res_content_encoding = get_content_encoding(&original_res_headers);

    apply_res_rules(&mut res_parts, &resolved_rules, verbose_logging, ctx);

    let res_headers: Vec<(String, String)> = res_parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let needs_processing = needs_body_processing(&resolved_rules);
    let has_res_body_override = resolved_rules.res_body.is_some();
    let needs_res_body_read = needs_processing && !has_res_body_override;

    let is_websocket = res_parts.status == StatusCode::SWITCHING_PROTOCOLS
        && res_parts
            .headers
            .get(hyper::header::UPGRADE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false);

    let res_content_length = res_parts
        .headers
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());

    let res_body_too_large = res_content_length
        .map(|len| len > max_body_buffer_size)
        .unwrap_or(false);

    let skip_body_processing = !needs_processing || (res_body_too_large && needs_res_body_read);

    if needs_res_body_read && res_body_too_large {
        warn!(
            "[{}] [RES_BODY] body too large ({} bytes > {} limit), skipping body rules and streaming forward",
            ctx.id_str(),
            res_content_length.unwrap(),
            max_body_buffer_size
        );
    }

    if skip_body_processing {
        let is_streaming = is_streaming_response(&res_parts);
        let is_sse = is_sse_response(&res_parts);
        let res_body_mode = if resolved_rules.trailers.is_empty() {
            BodyMode::Stream
        } else {
            BodyMode::StreamWithTrailers
        };
        normalize_res_headers(&mut res_parts, res_body_mode, &method);
        let res_headers: Vec<(String, String)> = res_parts
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        if verbose_logging && !res_body_too_large {
            if is_sse {
                info!(
                    "[{}] [SSE] detected SSE response, forwarding with event capture",
                    ctx.id_str()
                );
            } else if is_streaming {
                info!(
                    "[{}] [STREAMING] detected streaming response, forwarding directly with tee",
                    ctx.id_str()
                );
            } else {
                debug!(
                    "[{}] No body processing needed, streaming forward with tee",
                    ctx.id_str()
                );
            }
        }

        let total_ms = start_time.elapsed().as_millis() as u64;
        let record_id = ctx.id_str();
        let traffic_type = get_traffic_type_from_url(&record_url);
        let mut sse_stream_writer: Option<bifrost_admin::BodyStreamWriter> = None;

        if let Some(ref state) = admin_state {
            state
                .metrics_collector
                .add_bytes_sent_by_type(traffic_type, request_body_size as u64);
            state
                .metrics_collector
                .increment_requests_by_type(traffic_type);

            let mut record =
                TrafficRecord::new(record_id.clone(), method.clone(), record_url.clone());
            record.status = res_parts.status.as_u16();
            record.content_type = res_parts
                .headers
                .get(hyper::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            record.request_size =
                calculate_request_size(&method, &record_url, &req_headers, request_body_size);
            record.response_size = 0;
            record.duration_ms = total_ms;
            record.timing = Some(RequestTiming {
                dns_ms,
                connect_ms: Some(tcp_connect_ms),
                tls_ms,
                send_ms: None,
                wait_ms: Some(wait_ms),
                receive_ms: None,
                total_ms,
            });
            record.request_headers = Some(req_headers.clone());
            record.response_headers = Some(original_res_headers.clone());
            if res_headers != original_res_headers {
                record.actual_response_headers = Some(res_headers.clone());
            }
            record.has_rule_hit = has_rules;
            record.matched_rules = crate::utils::build_matched_rules(&resolved_rules);
            record.request_content_type = req_headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                .map(|(_, v)| v.clone());
            record.client_ip = ctx.client_ip.clone();
            record.client_app = ctx.client_app.clone();
            record.client_pid = ctx.client_pid;
            record.client_path = ctx.client_path.clone();

            if is_websocket {
                record.protocol = "ws".to_string();
                record.set_websocket();
                state.connection_monitor.register_connection(&record_id);
            } else if is_sse {
                record.set_sse();
                state.sse_hub.register(&record_id);
            } else if is_streaming {
                record.set_streaming();
                state.connection_monitor.register_connection(&record_id);
            }

            record.request_body_ref = if let Some(ref capture) = req_body_capture {
                capture.take()
            } else {
                store_request_body(
                    &admin_state,
                    &record_id,
                    &body_bytes,
                    req_content_encoding.as_deref(),
                )
            };

            if !req_script_results.is_empty() {
                record.req_script_results = Some(req_script_results.clone());
            }

            if is_sse {
                if let Some(ref body_store) = state.body_store {
                    match body_store.read().start_stream(&record_id, "sse_raw") {
                        Ok(writer) => {
                            record.response_body_ref = Some(writer.body_ref());
                            sse_stream_writer = Some(writer);
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, record_id = %record_id, "failed to start sse raw stream writer");
                        }
                    }
                }
            }

            state.record_traffic(record);
        }

        if is_sse {
            let tee_body = create_sse_tee_body(
                res_body,
                admin_state.clone(),
                record_id,
                Some(traffic_type),
                sse_stream_writer,
                max_body_buffer_size,
            );
            let body = with_trailers(tee_body.boxed(), &resolved_rules);
            return Ok(Response::from_parts(res_parts, body));
        } else {
            let response_headers_size =
                calculate_response_headers_size(res_parts.status.as_u16(), &res_headers);
            let tee_body = create_tee_body_with_store(
                res_body,
                admin_state.clone(),
                record_id,
                Some(max_body_buffer_size),
                res_content_encoding.clone(),
                Some(traffic_type),
                response_headers_size,
            );
            let body = with_trailers(tee_body.boxed(), &resolved_rules);
            return Ok(Response::from_parts(res_parts, body));
        }
    }

    let (res_body_bytes, receive_ms) = if needs_res_body_read {
        let receive_start = Instant::now();
        let res_body_bytes = res_body
            .collect()
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to read response body: {}", e)))?
            .to_bytes();
        let receive_ms = receive_start.elapsed().as_millis() as u64;
        (res_body_bytes, receive_ms)
    } else {
        (Bytes::new(), 0)
    };

    let content_type = res_parts
        .headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let original_res_body_len = res_content_length.unwrap_or(res_body_bytes.len());
    let final_res_body = if let Some(ref new_body) = resolved_rules.res_body {
        if verbose_logging {
            info!(
                "[{}] [RES_BODY] replaced: {} bytes -> {} bytes",
                ctx.id_str(),
                original_res_body_len,
                new_body.len()
            );
        }
        new_body.clone()
    } else {
        let body_processed = apply_body_rules(
            res_body_bytes.clone(),
            &resolved_rules,
            Phase::Response,
            Some(&content_type),
            verbose_logging,
            ctx,
        );

        apply_content_injection(
            body_processed,
            &content_type,
            &resolved_rules,
            verbose_logging,
            ctx,
        )
    };

    let mut res_script_status = res_parts.status.as_u16();
    let mut res_script_status_text = res_parts
        .status
        .canonical_reason()
        .unwrap_or("OK")
        .to_string();
    let mut res_script_headers = headers_to_hashmap(&res_headers);
    let mut res_script_body = String::from_utf8(final_res_body.to_vec()).ok();

    let res_script_results = execute_response_scripts(
        &admin_state,
        &resolved_rules.res_scripts,
        ctx,
        &resolved_rules,
        &url,
        &method,
        &headers_to_hashmap(&req_headers),
        &mut res_script_status,
        &mut res_script_status_text,
        &mut res_script_headers,
        &mut res_script_body,
        &values,
    )
    .await;

    if !res_script_results.is_empty() && res_script_results.iter().any(|r| r.success) {
        if let Ok(new_status) = hyper::StatusCode::from_u16(res_script_status) {
            res_parts.status = new_status;
        }

        let mut new_headers = hyper::HeaderMap::new();
        for (key, value) in &res_script_headers {
            if let (Ok(name), Ok(val)) = (
                hyper::header::HeaderName::from_bytes(key.as_bytes()),
                hyper::header::HeaderValue::from_str(value),
            ) {
                new_headers.insert(name, val);
            }
        }
        res_parts.headers = new_headers;
    }

    let final_res_body =
        if !res_script_results.is_empty() && res_script_results.iter().any(|r| r.success) {
            if let Some(ref new_body) = res_script_body {
                Bytes::from(new_body.clone())
            } else {
                final_res_body
            }
        } else {
            final_res_body
        };
    normalize_res_headers(
        &mut res_parts,
        BodyMode::Known(final_res_body.len()),
        &method,
    );

    let total_ms = start_time.elapsed().as_millis() as u64;

    if let Some(ref state) = admin_state {
        let traffic_type = get_traffic_type_from_url(&record_url);
        state
            .metrics_collector
            .add_bytes_sent_by_type(traffic_type, request_body_size as u64);
        state
            .metrics_collector
            .add_bytes_received_by_type(traffic_type, final_res_body.len() as u64);
        state
            .metrics_collector
            .increment_requests_by_type(traffic_type);

        let mut record = TrafficRecord::new(ctx.id_str(), method.clone(), record_url.clone());
        record.status = res_parts.status.as_u16();
        record.content_type = res_parts
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let res_headers: Vec<(String, String)> = res_parts
            .headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        record.request_size =
            calculate_request_size(&method, &record_url, &req_headers, request_body_size);
        record.response_size = calculate_response_size(
            res_parts.status.as_u16(),
            &res_headers,
            final_res_body.len(),
        );
        record.duration_ms = total_ms;
        record.timing = Some(RequestTiming {
            dns_ms,
            connect_ms: Some(tcp_connect_ms),
            tls_ms,
            send_ms: None,
            wait_ms: Some(wait_ms),
            receive_ms: Some(receive_ms),
            total_ms,
        });
        record.request_headers = Some(req_headers.clone());
        record.response_headers = Some(original_res_headers.clone());
        if res_headers != original_res_headers {
            record.actual_response_headers = Some(res_headers.clone());
        }
        record.original_request_headers = Some(original_req_headers.clone());
        if host != original_host || port != original_port {
            let actual_scheme = if use_tls { "https" } else { "http" };
            let actual_url = if (use_tls && port == 443) || (!use_tls && port == 80) {
                format!(
                    "{}://{}{}",
                    actual_scheme,
                    host,
                    processed_uri
                        .path_and_query()
                        .map(|pq| pq.as_str())
                        .unwrap_or("/")
                )
            } else {
                format!(
                    "{}://{}:{}{}",
                    actual_scheme,
                    host,
                    port,
                    processed_uri
                        .path_and_query()
                        .map(|pq| pq.as_str())
                        .unwrap_or("/")
                )
            };
            record.actual_url = Some(actual_url);
            record.actual_host = Some(host.clone());
        }
        record.has_rule_hit = has_rules;
        record.matched_rules = crate::utils::build_matched_rules(&resolved_rules);
        record.request_content_type = req_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());
        record.client_ip = ctx.client_ip.clone();
        record.client_app = ctx.client_app.clone();
        record.client_pid = ctx.client_pid;
        record.client_path = ctx.client_path.clone();

        if is_websocket {
            record.protocol = "ws".to_string();
            record.set_websocket();
            state.connection_monitor.register_connection(&ctx.id_str());
        }

        let is_sse = is_sse_response(&res_parts);
        if is_sse {
            record.set_sse();
        }

        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();

            let decompressed_req_body =
                decompress_body(&final_body, req_content_encoding.as_deref());
            record.request_body_ref =
                store.store(&ctx.id_str(), "req", decompressed_req_body.as_ref());

            let decompressed_res_body =
                decompress_body(&final_res_body, res_content_encoding.as_deref());
            record.response_body_ref =
                store.store(&ctx.id_str(), "res", decompressed_res_body.as_ref());
        }

        if !req_script_results.is_empty() {
            record.req_script_results = Some(req_script_results.clone());
        }
        if !res_script_results.is_empty() {
            record.res_script_results = Some(res_script_results.clone());
        }

        if is_sse {
            let event_count = parse_and_record_sse_events(&final_res_body);
            let response_size = final_res_body.len();
            record.response_size = response_size;
            record.frame_count = event_count;
            record.last_frame_id = event_count as u64;
            record.socket_status = Some(bifrost_admin::SocketStatus {
                is_open: false,
                send_count: 0,
                receive_count: event_count as u64,
                send_bytes: 0,
                receive_bytes: response_size as u64,
                frame_count: event_count,
                close_code: None,
                close_reason: Some("SSE stream completed".to_string()),
            });
        }

        state.record_traffic(record);
    }

    let body = with_trailers(full_body(final_res_body), &resolved_rules);
    Ok(Response::from_parts(res_parts, body))
}

fn build_redirect_response(status_code: u16, location: &str) -> Response<BoxBody> {
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::FOUND);
    let body = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Redirect</title></head>
<body><a href="{}">Redirecting...</a></body>
</html>"#,
        location
    );

    Response::builder()
        .status(status)
        .header(hyper::header::LOCATION, location)
        .header(hyper::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(full_body(bytes::Bytes::from(body)))
        .unwrap()
}

fn extract_host_port(uri: &Uri, rules: &ResolvedRules, is_https: bool) -> Result<(String, u16)> {
    let default_port = get_default_port(&rules.host_protocol, is_https);

    if !rules.ignored.host {
        if let Some(ref host_rule) = rules.host {
            let host_without_path = host_rule.split('/').next().unwrap_or(host_rule);
            let parts: Vec<&str> = host_without_path.split(':').collect();
            let host = parts[0].to_string();
            let port = if parts.len() > 1 {
                parts[1].parse().unwrap_or(default_port)
            } else {
                default_port
            };
            return Ok((host, port));
        }
    }

    if let Some(ref proxy_rule) = rules.proxy {
        if proxy_rule.starts_with("http://") || proxy_rule.starts_with("https://") {
            if let Ok(url) = Url::parse(proxy_rule) {
                if let Some(host) = url.host_str() {
                    let port = url.port().unwrap_or(default_port);
                    return Ok((host.to_string(), port));
                }
            }
        }
        let host_without_path = proxy_rule.split('/').next().unwrap_or(proxy_rule);
        let parts: Vec<&str> = host_without_path.split(':').collect();
        let host = parts[0].to_string();
        let port = if parts.len() > 1 {
            parts[1].parse().unwrap_or(default_port)
        } else {
            default_port
        };
        return Ok((host, port));
    }

    let host = uri
        .host()
        .ok_or_else(|| BifrostError::Network("Missing host in URI".to_string()))?
        .to_string();

    let port = uri.port_u16().unwrap_or(default_port);

    Ok((host, port))
}

fn get_default_port(host_protocol: &Option<Protocol>, is_https: bool) -> u16 {
    match host_protocol {
        Some(Protocol::Http) | Some(Protocol::Ws) => 80,
        Some(Protocol::Https) | Some(Protocol::Wss) => 443,
        None | Some(Protocol::Host) => {
            if is_https {
                443
            } else {
                80
            }
        }
        _ => 80,
    }
}

async fn handle_http_websocket(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
    unsafe_ssl: bool,
) -> Result<Response<BoxBody>> {
    use super::websocket::websocket_bidirectional_generic_with_capture;
    use crate::server::empty_body;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::rustls::pki_types::ServerName;

    let start_time = Instant::now();
    let uri = req.uri().clone();
    let method = req.method().to_string();

    let forwarded_proto = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|p| p.split(',').next().unwrap_or(p).trim().to_ascii_lowercase());

    let host_header = req
        .headers()
        .get("x-forwarded-host")
        .and_then(|v| v.to_str().ok())
        .map(|h| h.split(',').next().unwrap_or(h).trim().to_string())
        .or_else(|| uri.host().map(|h| h.to_string()))
        .or_else(|| {
            req.headers()
                .get(hyper::header::HOST)
                .and_then(|v| v.to_str().ok())
                .map(|h| h.trim().to_string())
        })
        .ok_or_else(|| BifrostError::Network("Missing host in WebSocket request".to_string()))?;

    let (host, host_port_from_header) = if let Some((h, p)) = host_header.rsplit_once(':') {
        if let Ok(p) = p.parse::<u16>() {
            (h.to_string(), Some(p))
        } else {
            (host_header.clone(), None)
        }
    } else {
        (host_header.clone(), None)
    };

    let is_wss = matches!(uri.scheme_str(), Some("wss" | "https"))
        || matches!(forwarded_proto.as_deref(), Some("wss" | "https"))
        || matches!(uri.port_u16(), Some(443 | 8443))
        || matches!(host_port_from_header, Some(443 | 8443));

    let port = uri
        .port_u16()
        .or(host_port_from_header)
        .unwrap_or(if is_wss { 443 } else { 80 });

    let ws_scheme = if is_wss { "wss" } else { "ws" };
    let ws_url = if let Some(authority) = uri.authority().map(|a| a.as_str()) {
        let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        format!("{}://{}{}", ws_scheme, authority, path)
    } else {
        let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        format!("{}://{}{}", ws_scheme, host_header, path)
    };

    let http_scheme = if is_wss { "https" } else { "http" };
    let http_url = if let Some(authority) = uri.authority().map(|a| a.as_str()) {
        let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        format!("{}://{}{}", http_scheme, authority, path)
    } else {
        let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
        format!("{}://{}{}", http_scheme, host_header, path)
    };

    let mut resolved_rules = rules.resolve(&ws_url, "GET");
    if resolved_rules.rules.is_empty() && resolved_rules.host.is_none() {
        resolved_rules = rules.resolve(&http_url, "GET");
    }
    let has_rules = !resolved_rules.rules.is_empty() || resolved_rules.host.is_some();

    let req_headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let (target_host, target_port) = if let Some(ref host_rule) = resolved_rules.host {
        let parts: Vec<&str> = host_rule.split(':').collect();
        let h = parts[0].to_string();
        let p = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(port);
        (h, p)
    } else {
        (host.to_string(), port)
    };

    debug!(
        "[{}] WebSocket upgrade via HTTP proxy to {}:{}",
        ctx.id_str(),
        target_host,
        target_port
    );

    let connect_start = Instant::now();
    let target_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                target_host, target_port, e
            ))
        })?;
    let tcp_connect_ms = connect_start.elapsed().as_millis() as u64;

    if let Err(e) = target_stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY on WebSocket connection: {}", e);
    }

    let use_tls = match resolved_rules.host_protocol {
        Some(Protocol::Http) | Some(Protocol::Ws) => false,
        Some(Protocol::Https) | Some(Protocol::Wss) => true,
        Some(Protocol::Host) | Some(Protocol::XHost) => target_port == 443 || target_port == 8443,
        _ => is_wss,
    };
    let mut target_stream: Box<dyn AsyncReadWrite + Unpin + Send> = if use_tls {
        let tls_config = super::tunnel::get_tls_client_config(unsafe_ssl);
        let connector = TlsConnector::from(tls_config);

        let server_name = ServerName::try_from(target_host.clone()).map_err(|_| {
            BifrostError::Network(format!("Invalid server name for TLS: {}", target_host))
        })?;

        let tls_stream = connector
            .connect(server_name, target_stream)
            .await
            .map_err(|e| BifrostError::Network(format!("TLS handshake failed: {}", e)))?;

        Box::new(tls_stream)
    } else {
        Box::new(target_stream)
    };

    let upgrade_request = build_http_websocket_handshake(&req, &target_host, target_port)?;
    target_stream
        .write_all(upgrade_request.as_bytes())
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send WS handshake: {}", e)))?;

    let mut response_buf = vec![0u8; 4096];
    let n = target_stream.read(&mut response_buf).await.map_err(|e| {
        BifrostError::Network(format!("Failed to read WS handshake response: {}", e))
    })?;

    let response_str = String::from_utf8_lossy(&response_buf[..n]);
    if !response_str.contains("101") {
        return Err(BifrostError::Network(format!(
            "WebSocket handshake failed: {}",
            response_str
        )));
    }

    let (response_headers, sec_accept) = parse_websocket_response(&response_str);

    let compression_enabled = crate::protocol::extract_sec_websocket_extensions(&response_str)
        .map(|ext| crate::protocol::parse_permessage_deflate(&ext))
        .unwrap_or(false);

    let total_ms = start_time.elapsed().as_millis() as u64;
    let record_id = ctx.id_str();

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_requests_by_type(bifrost_admin::TrafficType::Ws);

        let record_protocol = if use_tls { "wss" } else { "ws" };
        let ws_url = format!(
            "{}://{}:{}{}",
            record_protocol,
            host,
            port,
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
        );

        let mut record = bifrost_admin::TrafficRecord::new(record_id.clone(), method, ws_url);
        record.status = 101;
        record.protocol = record_protocol.to_string();
        record.duration_ms = total_ms;
        record.timing = Some(bifrost_admin::RequestTiming {
            dns_ms: None,
            connect_ms: Some(tcp_connect_ms),
            tls_ms: None,
            send_ms: None,
            wait_ms: Some(total_ms.saturating_sub(tcp_connect_ms)),
            receive_ms: None,
            total_ms,
        });
        record.request_headers = Some(req_headers);
        record.response_headers = Some(response_headers.clone());
        record.has_rule_hit = has_rules;
        record.matched_rules = crate::utils::build_matched_rules(&resolved_rules);
        record.client_ip = ctx.client_ip.clone();
        record.client_app = ctx.client_app.clone();
        record.client_pid = ctx.client_pid;
        record.client_path = ctx.client_path.clone();
        record.set_websocket();

        state.connection_monitor.register_connection(&record_id);
        state.record_traffic(record);
    }

    let record_id_clone = record_id.clone();
    let admin_state_clone = admin_state.clone();
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = websocket_bidirectional_generic_with_capture(
                    upgraded,
                    target_stream,
                    &record_id_clone,
                    admin_state_clone.clone(),
                    compression_enabled,
                )
                .await
                {
                    error!("[{}] WebSocket tunnel error: {}", record_id_clone, e);
                }

                if let Some(ref state) = admin_state_clone {
                    state.connection_monitor.set_connection_closed(
                        &record_id_clone,
                        None,
                        None,
                        state.frame_store.as_ref(),
                        state.ws_payload_store.as_ref(),
                    );
                }
            }
            Err(e) => {
                error!("[{}] WebSocket upgrade error: {}", record_id_clone, e);
            }
        }
    });

    let mut response = Response::builder()
        .status(101)
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::CONNECTION, "Upgrade");

    if let Some(accept) = sec_accept {
        response = response.header("Sec-WebSocket-Accept", accept);
    }

    for (name, value) in response_headers {
        if name.to_lowercase() != "upgrade"
            && name.to_lowercase() != "connection"
            && name.to_lowercase() != "sec-websocket-accept"
        {
            response = response.header(name, value);
        }
    }

    Ok(response.body(empty_body()).unwrap())
}

fn build_http_websocket_handshake(
    req: &Request<Incoming>,
    target_host: &str,
    target_port: u16,
) -> Result<String> {
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let host_header = if target_port == 80 {
        target_host.to_string()
    } else {
        format!("{}:{}", target_host, target_port)
    };

    let ws_key = req
        .headers()
        .get("Sec-WebSocket-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let ws_version = req
        .headers()
        .get("Sec-WebSocket-Version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("13");

    let mut handshake = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: {}\r\n",
        path, host_header, ws_key, ws_version
    );

    if let Some(protocol) = req.headers().get("Sec-WebSocket-Protocol") {
        if let Ok(protocol_str) = protocol.to_str() {
            handshake.push_str(&format!("Sec-WebSocket-Protocol: {}\r\n", protocol_str));
        }
    }

    if let Some(extensions) = req.headers().get("Sec-WebSocket-Extensions") {
        if let Ok(ext_str) = extensions.to_str() {
            handshake.push_str(&format!("Sec-WebSocket-Extensions: {}\r\n", ext_str));
        }
    }

    handshake.push_str("\r\n");
    Ok(handshake)
}

fn parse_websocket_response(response_str: &str) -> (Vec<(String, String)>, Option<String>) {
    let mut headers = Vec::new();
    let mut sec_accept = None;

    for line in response_str.lines().skip(1) {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim().to_string();
            let value = value.trim().to_string();
            if name.to_lowercase() == "sec-websocket-accept" {
                sec_accept = Some(value.clone());
            }
            headers.push((name, value));
        }
    }

    (headers, sec_accept)
}

pub fn is_websocket_upgrade(req: &Request<Incoming>) -> bool {
    let connection = req
        .headers()
        .get(hyper::header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let upgrade = req
        .headers()
        .get(hyper::header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    connection.to_lowercase().contains("upgrade") && upgrade.to_lowercase() == "websocket"
}

pub fn get_request_url(req: &Request<Incoming>) -> String {
    let uri = req.uri();
    if uri.scheme().is_some() {
        uri.to_string()
    } else {
        let host = req
            .headers()
            .get(hyper::header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("localhost");
        format!(
            "http://{}{}",
            host,
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
        )
    }
}

pub fn parse_and_record_sse_events(body: &[u8]) -> usize {
    let body_str = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut current_event = String::new();
    let mut count = 0usize;
    for line in body_str.lines() {
        if line.is_empty() {
            if !current_event.is_empty() {
                current_event.clear();
                count += 1;
            }
        } else {
            if !current_event.is_empty() {
                current_event.push('\n');
            }
            current_event.push_str(line);
        }
    }

    if !current_event.is_empty() {
        count += 1;
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Uri;

    #[test]
    fn test_extract_host_port_from_uri() {
        let uri: Uri = "http://example.com:8080/path".parse().unwrap();
        let rules = ResolvedRules::default();
        let (host, port) = extract_host_port(&uri, &rules, false).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_extract_host_port_default_port() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        let rules = ResolvedRules::default();
        let (host, port) = extract_host_port(&uri, &rules, false).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 80);
    }

    #[test]
    fn test_extract_host_port_with_rule_override() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        let rules = ResolvedRules {
            host: Some("override.com:9000".to_string()),
            ..Default::default()
        };
        let (host, port) = extract_host_port(&uri, &rules, false).unwrap();
        assert_eq!(host, "override.com");
        assert_eq!(port, 9000);
    }

    #[test]
    fn test_extract_host_port_rule_without_port() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        let rules = ResolvedRules {
            host: Some("override.com".to_string()),
            ..Default::default()
        };
        let (host, port) = extract_host_port(&uri, &rules, false).unwrap();
        assert_eq!(host, "override.com");
        assert_eq!(port, 80);
    }

    #[test]
    fn test_extract_host_port_rule_with_path() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        let rules = ResolvedRules {
            host: Some("127.0.0.1:3020/ws".to_string()),
            ..Default::default()
        };
        let (host, port) = extract_host_port(&uri, &rules, false).unwrap();
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 3020);
    }

    #[test]
    fn test_extract_host_port_https_default_port() {
        let uri: Uri = "https://example.com/path".parse().unwrap();
        let rules = ResolvedRules::default();
        let (host, port) = extract_host_port(&uri, &rules, true).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_extract_host_port_https_rule_without_port() {
        let uri: Uri = "https://example.com/path".parse().unwrap();
        let rules = ResolvedRules {
            host: Some("override.com".to_string()),
            ..Default::default()
        };
        let (host, port) = extract_host_port(&uri, &rules, true).unwrap();
        assert_eq!(host, "override.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_extract_host_port_http_protocol_forces_port_80() {
        let uri: Uri = "https://example.com/path".parse().unwrap();
        let rules = ResolvedRules {
            host: Some("override.com".to_string()),
            host_protocol: Some(Protocol::Http),
            ..Default::default()
        };
        let (host, port) = extract_host_port(&uri, &rules, true).unwrap();
        assert_eq!(host, "override.com");
        assert_eq!(port, 80);
    }

    #[test]
    fn test_extract_host_port_https_protocol_forces_port_443() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        let rules = ResolvedRules {
            host: Some("override.com".to_string()),
            host_protocol: Some(Protocol::Https),
            ..Default::default()
        };
        let (host, port) = extract_host_port(&uri, &rules, false).unwrap();
        assert_eq!(host, "override.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_get_default_port() {
        assert_eq!(get_default_port(&None, false), 80);
        assert_eq!(get_default_port(&None, true), 443);
        assert_eq!(get_default_port(&Some(Protocol::Host), false), 80);
        assert_eq!(get_default_port(&Some(Protocol::Host), true), 443);
        assert_eq!(get_default_port(&Some(Protocol::Http), false), 80);
        assert_eq!(get_default_port(&Some(Protocol::Http), true), 80);
        assert_eq!(get_default_port(&Some(Protocol::Https), false), 443);
        assert_eq!(get_default_port(&Some(Protocol::Https), true), 443);
        assert_eq!(get_default_port(&Some(Protocol::Ws), false), 80);
        assert_eq!(get_default_port(&Some(Protocol::Wss), true), 443);
    }
}

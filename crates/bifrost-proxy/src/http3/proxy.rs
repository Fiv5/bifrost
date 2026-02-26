use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use bifrost_admin::{AdminState, TrafficRecord, TrafficType};
use bifrost_core::{BifrostError, Result};
use bytes::{Buf, Bytes};
use h3::quic::BidiStream;
use h3::server::RequestStream;
use hyper::{Request, Response, StatusCode, Uri};
use tracing::{debug, info, warn};

use crate::dns::DnsResolver;
use crate::protocol::ProtocolDetector;
use crate::proxy::http::handler::{needs_body_processing, needs_request_body_processing};
use crate::server::{ProxyConfig, ResolvedRules, RulesResolver};
use crate::utils::http_size::{calculate_request_size, calculate_response_size};
use crate::utils::logging::RequestContext;

use super::Http3Client;

#[allow(clippy::too_many_arguments)]
pub async fn handle_h3_proxy_request<S>(
    req: Request<()>,
    mut stream: RequestStream<S, Bytes>,
    peer_addr: SocketAddr,
    rules: Arc<dyn RulesResolver>,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    _dns_resolver: Arc<DnsResolver>,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let start_time = Instant::now();
    let ctx = RequestContext::new();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let verbose = proxy_config.verbose_logging;

    info!(
        "[{}] HTTP/3 Proxy: {} {} from {}",
        ctx.id_str(),
        method,
        uri,
        peer_addr
    );

    let req_headers_map: HashMap<String, String> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let req_cookies_map: HashMap<String, String> = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|cookies| {
            cookies
                .split(';')
                .filter_map(|c| {
                    let mut parts = c.trim().splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    let resolved_rules = rules.resolve_with_context(
        &uri.to_string(),
        method.as_str(),
        &req_headers_map,
        &req_cookies_map,
    );

    let has_rules = !is_rules_empty(&resolved_rules);

    let headers_vec: Vec<(String, String)> = req_headers_map
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let is_websocket = ProtocolDetector::is_websocket_upgrade(&headers_vec);
    let is_sse = ProtocolDetector::is_sse_request(&headers_vec);

    if verbose {
        if has_rules {
            info!(
                "[{}] Rules matched for HTTP/3 request: host={:?}, headers={}, res_headers={}",
                ctx.id_str(),
                resolved_rules.host,
                resolved_rules.req_headers.len(),
                resolved_rules.res_headers.len()
            );
        }
        if is_websocket {
            info!("[{}] HTTP/3 WebSocket upgrade detected", ctx.id_str());
        }
        if is_sse {
            info!("[{}] HTTP/3 SSE request detected", ctx.id_str());
        }
    }

    let host = extract_host(&uri, &headers, &resolved_rules)?;
    let port = extract_port(&uri, &resolved_rules);

    let mut body_data = Vec::new();
    while let Some(mut data) = stream
        .recv_data()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to receive request body: {}", e)))?
    {
        while data.has_remaining() {
            let chunk = data.chunk();
            body_data.extend_from_slice(chunk);
            data.advance(chunk.len());
        }
    }

    debug!(
        "[{}] Request body received: {} bytes",
        ctx.id_str(),
        body_data.len()
    );

    let modified_body = if needs_request_body_processing(&resolved_rules) && !body_data.is_empty() {
        apply_request_body_rules(&body_data, &resolved_rules)
    } else {
        Bytes::from(body_data.clone())
    };

    let req_headers: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_requests_by_type(TrafficType::H3);

        let mut record = TrafficRecord::new(ctx.id_str(), method.to_string(), uri.to_string());
        record.protocol = "h3".to_string();
        record.host = host.clone();
        record.client_ip = peer_addr.ip().to_string();
        record.request_headers = Some(req_headers.clone());
        record.request_size = calculate_request_size(
            &method.to_string(),
            &uri.to_string(),
            &req_headers,
            body_data.len(),
        );
        record.has_rule_hit = has_rules;
        record.is_websocket = is_websocket;
        record.is_sse = is_sse;
        record.set_h3();

        state.record_traffic(record);
    }

    let target_uri = build_target_uri(&uri, &host, port, &resolved_rules)?;

    let mut req_builder = Request::builder()
        .method(method.clone())
        .uri(target_uri.clone());

    for (key, value) in headers.iter() {
        if !is_hop_by_hop_header(key.as_str()) {
            req_builder = req_builder.header(key, value);
        }
    }

    for (key, value) in &resolved_rules.req_headers {
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    if let Some(ref ua) = resolved_rules.ua {
        req_builder = req_builder.header("user-agent", ua.as_str());
    }

    let outgoing_req = req_builder
        .body(modified_body)
        .map_err(|e| BifrostError::Parse(format!("Failed to build request: {}", e)))?;

    info!(
        "[{}] Forwarding via HTTP/3 to {}:{}{}",
        ctx.id_str(),
        host,
        port,
        if has_rules { " (with rules)" } else { "" }
    );

    let client = Http3Client::new()?;

    let response = match client.request(&host, port, outgoing_req).await {
        Ok(resp) => {
            info!(
                "[{}] HTTP/3 response: {} from {}",
                ctx.id_str(),
                resp.status(),
                host
            );
            resp
        }
        Err(e) => {
            warn!("[{}] HTTP/3 request failed: {}", ctx.id_str(), e);

            send_error_response(
                &mut stream,
                StatusCode::BAD_GATEWAY,
                &format!("HTTP/3 request failed: {}", e),
            )
            .await?;
            return Ok(());
        }
    };

    let status = response.status();
    let response_headers = response.headers().clone();
    let response_body = response.into_body();

    let modified_response_body =
        if needs_body_processing(&resolved_rules) && !response_body.is_empty() {
            apply_response_body_rules(&response_body, &resolved_rules)
        } else {
            response_body
        };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let response_headers_vec: Vec<(String, String)> = response_headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let response_body_len = modified_response_body.len();
    let response_total_size =
        calculate_response_size(status.as_u16(), &response_headers_vec, response_body_len);

    if let Some(ref state) = admin_state {
        let req_id = ctx.id_str();
        state.update_traffic_by_id(&req_id, move |record| {
            record.status = status.as_u16();
            record.response_size = response_total_size;
            record.response_headers = Some(response_headers_vec.clone());
            record.duration_ms = duration_ms;
        });
    }

    let mut h3_response = Response::builder().status(status);

    for (key, value) in response_headers.iter() {
        if !is_hop_by_hop_header(key.as_str()) {
            h3_response = h3_response.header(key, value);
        }
    }

    for (key, value) in &resolved_rules.res_headers {
        h3_response = h3_response.header(key.as_str(), value.as_str());
    }

    let h3_response = h3_response
        .body(())
        .map_err(|e| BifrostError::Parse(format!("Failed to build response: {}", e)))?;

    stream
        .send_response(h3_response)
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send response: {}", e)))?;

    if !modified_response_body.is_empty() {
        stream
            .send_data(modified_response_body)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to send response body: {}", e)))?;
    }

    stream
        .finish()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to finish stream: {}", e)))?;

    info!(
        "[{}] HTTP/3 proxy completed: {} {} -> {} ({}ms)",
        ctx.id_str(),
        method,
        uri,
        status,
        duration_ms
    );

    Ok(())
}

fn extract_host(uri: &Uri, headers: &hyper::HeaderMap, rules: &ResolvedRules) -> Result<String> {
    if let Some(ref host_rule) = rules.host {
        let host = host_rule.split(':').next().unwrap_or(host_rule);
        let host = host.split('/').next().unwrap_or(host);
        return Ok(host.to_string());
    }

    uri.host()
        .map(|h| h.to_string())
        .or_else(|| {
            headers
                .get("host")
                .and_then(|h| h.to_str().ok())
                .map(|h| h.split(':').next().unwrap_or(h).to_string())
        })
        .ok_or_else(|| BifrostError::Parse("Missing host in request".to_string()))
}

fn extract_port(uri: &Uri, rules: &ResolvedRules) -> u16 {
    if let Some(ref host_rule) = rules.host {
        if let Some(port_str) = host_rule.split(':').nth(1) {
            let port_str = port_str.split('/').next().unwrap_or(port_str);
            if let Ok(port) = port_str.parse::<u16>() {
                return port;
            }
        }
    }

    uri.port_u16().unwrap_or(443)
}

fn build_target_uri(original: &Uri, host: &str, port: u16, rules: &ResolvedRules) -> Result<Uri> {
    let path_and_query = if let Some(ref host_rule) = rules.host {
        if let Some(path_start) = host_rule.find('/') {
            let rule_path = &host_rule[path_start..];
            let original_path = original
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/");
            format!("{}{}", rule_path.trim_end_matches('/'), original_path)
        } else {
            original
                .path_and_query()
                .map(|pq| pq.as_str().to_string())
                .unwrap_or_else(|| "/".to_string())
        }
    } else {
        original
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string())
    };

    let scheme = original.scheme_str().unwrap_or("https");

    let uri_str = if port == 443 && scheme == "https" || port == 80 && scheme == "http" {
        format!("{}://{}{}", scheme, host, path_and_query)
    } else {
        format!("{}://{}:{}{}", scheme, host, port, path_and_query)
    };

    uri_str
        .parse()
        .map_err(|e| BifrostError::Parse(format!("Invalid URI: {}", e)))
}

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn is_rules_empty(rules: &ResolvedRules) -> bool {
    rules.host.is_none()
        && rules.req_headers.is_empty()
        && rules.res_headers.is_empty()
        && rules.req_body.is_none()
        && rules.res_body.is_none()
        && rules.ua.is_none()
}

fn apply_request_body_rules(body: &[u8], rules: &ResolvedRules) -> Bytes {
    if let Some(ref new_body) = rules.req_body {
        return new_body.clone();
    }

    let mut result = body.to_vec();

    if let Some(ref prepend) = rules.req_prepend {
        let mut new_body = prepend.to_vec();
        new_body.extend_from_slice(&result);
        result = new_body;
    }

    if let Some(ref append) = rules.req_append {
        result.extend_from_slice(append);
    }

    Bytes::from(result)
}

fn apply_response_body_rules(body: &Bytes, rules: &ResolvedRules) -> Bytes {
    if let Some(ref new_body) = rules.res_body {
        return new_body.clone();
    }

    let mut result = body.to_vec();

    if let Some(ref prepend) = rules.res_prepend {
        let mut new_body = prepend.to_vec();
        new_body.extend_from_slice(&result);
        result = new_body;
    }

    if let Some(ref append) = rules.res_append {
        result.extend_from_slice(append);
    }

    for (search, replace) in &rules.res_replace {
        if let Ok(body_str) = String::from_utf8(result.clone()) {
            result = body_str.replace(search, replace).into_bytes();
        }
    }

    for regex_replace in &rules.res_replace_regex {
        if let Ok(body_str) = String::from_utf8(result.clone()) {
            if regex_replace.global {
                result = regex_replace
                    .pattern
                    .replace_all(&body_str, regex_replace.replacement.as_str())
                    .into_owned()
                    .into_bytes();
            } else {
                result = regex_replace
                    .pattern
                    .replace(&body_str, regex_replace.replacement.as_str())
                    .into_owned()
                    .into_bytes();
            }
        }
    }

    Bytes::from(result)
}

async fn send_error_response<S>(
    stream: &mut RequestStream<S, Bytes>,
    status: StatusCode,
    message: &str,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let response = Response::builder().status(status).body(()).unwrap();

    stream
        .send_response(response)
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send error response: {}", e)))?;

    stream
        .send_data(Bytes::from(message.to_string()))
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send error body: {}", e)))?;

    stream
        .finish()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to finish stream: {}", e)))?;

    Ok(())
}

use std::sync::Arc;
use std::time::Instant;

use bifrost_admin::{AdminState, MatchedRule, RequestTiming, TrafficRecord};
use bifrost_core::{BifrostError, Result};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::{Request, Response, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tracing::{debug, error, info};

use crate::body::{apply_body_rules, apply_content_injection, Phase};
use crate::logging::{format_rules_detail, format_rules_summary, RequestContext};
use crate::mock::{generate_mock_response, should_intercept_response};
use crate::request::apply_req_rules;
use crate::response::apply_res_rules;
use crate::server::{full_body, BoxBody, ResolvedRules, RulesResolver};
use crate::url::apply_url_rules;

pub async fn handle_http_request(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
) -> Result<Response<BoxBody>> {
    let uri = req.uri().clone();
    let method = req.method().to_string();
    let url = uri.to_string();
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

    if resolved_rules.ignored {
        if verbose_logging {
            info!("[{}] [IGNORED] request ignored by rule", ctx.id_str());
        }
        return forward_without_rules(req, admin_state).await;
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
        if verbose_logging {
            info!("[{}] [REDIRECT] {} -> {}", ctx.id_str(), url, redirect_url);
        }
        return Ok(build_redirect_response(302, redirect_url));
    }

    if let Some(ref location) = resolved_rules.location_href {
        if verbose_logging {
            info!("[{}] [LOCATION] {} -> {}", ctx.id_str(), url, location);
        }
        return Ok(build_redirect_response(301, location));
    }

    let processed_uri = apply_url_rules(&uri, &resolved_rules, verbose_logging, ctx);

    let original_host = uri.host().unwrap_or("unknown").to_string();
    let original_port = uri.port_u16().unwrap_or(80);
    let (host, port) = extract_host_port(&processed_uri, &resolved_rules)?;

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

    let req_headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    apply_req_rules(&mut parts, &resolved_rules, verbose_logging, ctx);

    let body_bytes = body
        .collect()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to read request body: {}", e)))?
        .to_bytes();

    let final_body = if let Some(ref new_body) = resolved_rules.req_body {
        if verbose_logging {
            info!(
                "[{}] [REQ_BODY] replaced: {} bytes -> {} bytes",
                ctx.id_str(),
                body_bytes.len(),
                new_body.len()
            );
        }
        new_body.clone()
    } else {
        apply_body_rules(
            body_bytes.clone(),
            &resolved_rules,
            Phase::Request,
            verbose_logging,
            ctx,
        )
    };

    let connect_start = Instant::now();
    let stream = TcpStream::connect(format!("{}:{}", host, port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!("Failed to connect to {}:{}: {}", host, port, e))
        })?;
    let connect_ms = connect_start.elapsed().as_millis() as u64;

    let io = TokioIo::new(stream);

    let (mut sender, conn) = ClientBuilder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .handshake(io)
        .await
        .map_err(|e| BifrostError::Network(format!("Handshake failed: {}", e)))?;

    tokio::spawn(async move {
        if let Err(err) = conn.await {
            error!("Connection failed: {:?}", err);
        }
    });

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

    let outgoing_req = Request::from_parts(parts, full_body(final_body.clone()));

    let send_start = Instant::now();
    let res = sender
        .send_request(outgoing_req)
        .await
        .map_err(|e| BifrostError::Network(format!("Request failed: {}", e)))?;
    let wait_ms = send_start.elapsed().as_millis() as u64;

    let (mut res_parts, res_body) = res.into_parts();

    let res_headers: Vec<(String, String)> = res_parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    apply_res_rules(&mut res_parts, &resolved_rules, verbose_logging, ctx);

    let receive_start = Instant::now();
    let res_body_bytes = res_body
        .collect()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to read response body: {}", e)))?
        .to_bytes();
    let receive_ms = receive_start.elapsed().as_millis() as u64;

    let content_type = res_parts
        .headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let final_res_body = if let Some(ref new_body) = resolved_rules.res_body {
        if verbose_logging {
            info!(
                "[{}] [RES_BODY] replaced: {} bytes -> {} bytes",
                ctx.id_str(),
                res_body_bytes.len(),
                new_body.len()
            );
        }
        new_body.clone()
    } else {
        let body_processed = apply_body_rules(
            res_body_bytes.clone(),
            &resolved_rules,
            Phase::Response,
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

    let total_ms = start_time.elapsed().as_millis() as u64;

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .add_bytes_sent(final_body.len() as u64);
        state
            .metrics_collector
            .add_bytes_received(final_res_body.len() as u64);

        let mut record = TrafficRecord::new(ctx.id_str(), method, url);
        record.status = res_parts.status.as_u16();
        record.content_type = res_parts
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        record.request_size = final_body.len();
        record.response_size = final_res_body.len();
        record.duration_ms = total_ms;
        record.timing = Some(RequestTiming {
            dns_ms: None,
            connect_ms: Some(connect_ms),
            tls_ms: None,
            send_ms: None,
            wait_ms: Some(wait_ms),
            receive_ms: Some(receive_ms),
            total_ms,
        });
        record.request_headers = Some(req_headers);
        record.response_headers = Some(res_headers);
        record.matched_rules = if resolved_rules.rules.is_empty() {
            None
        } else {
            Some(
                resolved_rules
                    .rules
                    .iter()
                    .map(|r| MatchedRule {
                        pattern: r.pattern.clone(),
                        protocol: format!("{:?}", r.protocol),
                        value: r.value.clone(),
                    })
                    .collect(),
            )
        };

        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();
            record.request_body_ref = store.store(&ctx.id_str(), "req", &body_bytes);
            record.response_body_ref = store.store(&ctx.id_str(), "res", &res_body_bytes);
        }

        state.traffic_recorder.record(record);
    }

    Ok(Response::from_parts(res_parts, full_body(final_res_body)))
}

async fn forward_without_rules(
    req: Request<Incoming>,
    admin_state: Option<Arc<AdminState>>,
) -> Result<Response<BoxBody>> {
    let start_time = Instant::now();
    let method = req.method().to_string();
    let uri = req.uri().clone();
    let url = uri.to_string();
    let host = uri
        .host()
        .ok_or_else(|| BifrostError::Network("Missing host in URI".to_string()))?
        .to_string();
    let port = uri.port_u16().unwrap_or(80);

    let connect_start = Instant::now();
    let stream = TcpStream::connect(format!("{}:{}", host, port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!("Failed to connect to {}:{}: {}", host, port, e))
        })?;
    let connect_ms = connect_start.elapsed().as_millis() as u64;

    let io = TokioIo::new(stream);

    let (mut sender, conn) = ClientBuilder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .handshake(io)
        .await
        .map_err(|e| BifrostError::Network(format!("Handshake failed: {}", e)))?;

    tokio::spawn(async move {
        if let Err(err) = conn.await {
            error!("Connection failed: {:?}", err);
        }
    });

    let (mut parts, body) = req.into_parts();

    let req_headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let path = parts
        .uri
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

    let body_bytes = body
        .collect()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to read request body: {}", e)))?
        .to_bytes();

    let outgoing_req = Request::from_parts(parts, full_body(body_bytes.clone()));

    let send_start = Instant::now();
    let res = sender
        .send_request(outgoing_req)
        .await
        .map_err(|e| BifrostError::Network(format!("Request failed: {}", e)))?;
    let wait_ms = send_start.elapsed().as_millis() as u64;

    let (res_parts, res_body) = res.into_parts();

    let res_headers: Vec<(String, String)> = res_parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let receive_start = Instant::now();
    let res_body_bytes = res_body
        .collect()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to read response body: {}", e)))?
        .to_bytes();
    let receive_ms = receive_start.elapsed().as_millis() as u64;

    let total_ms = start_time.elapsed().as_millis() as u64;

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .add_bytes_sent(body_bytes.len() as u64);
        state
            .metrics_collector
            .add_bytes_received(res_body_bytes.len() as u64);

        let record_id = format!(
            "{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let mut record = TrafficRecord::new(record_id, method, url);
        record.status = res_parts.status.as_u16();
        record.content_type = res_parts
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        record.request_size = body_bytes.len();
        record.response_size = res_body_bytes.len();
        record.duration_ms = total_ms;
        record.timing = Some(RequestTiming {
            dns_ms: None,
            connect_ms: Some(connect_ms),
            tls_ms: None,
            send_ms: None,
            wait_ms: Some(wait_ms),
            receive_ms: Some(receive_ms),
            total_ms,
        });
        record.request_headers = Some(req_headers);
        record.response_headers = Some(res_headers);

        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();
            record.request_body_ref = store.store(&record.id, "req", &body_bytes);
            record.response_body_ref = store.store(&record.id, "res", &res_body_bytes);
        }

        state.traffic_recorder.record(record);
    }

    Ok(Response::from_parts(res_parts, full_body(res_body_bytes)))
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

fn extract_host_port(uri: &Uri, rules: &ResolvedRules) -> Result<(String, u16)> {
    if let Some(ref host_rule) = rules.host {
        let parts: Vec<&str> = host_rule.split(':').collect();
        let host = parts[0].to_string();
        let port = if parts.len() > 1 {
            parts[1].parse().unwrap_or(80)
        } else {
            80
        };
        return Ok((host, port));
    }

    let host = uri
        .host()
        .ok_or_else(|| BifrostError::Network("Missing host in URI".to_string()))?
        .to_string();

    let port = uri.port_u16().unwrap_or(80);

    Ok((host, port))
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

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Uri;

    #[test]
    fn test_extract_host_port_from_uri() {
        let uri: Uri = "http://example.com:8080/path".parse().unwrap();
        let rules = ResolvedRules::default();
        let (host, port) = extract_host_port(&uri, &rules).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_extract_host_port_default_port() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        let rules = ResolvedRules::default();
        let (host, port) = extract_host_port(&uri, &rules).unwrap();
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
        let (host, port) = extract_host_port(&uri, &rules).unwrap();
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
        let (host, port) = extract_host_port(&uri, &rules).unwrap();
        assert_eq!(host, "override.com");
        assert_eq!(port, 80);
    }
}

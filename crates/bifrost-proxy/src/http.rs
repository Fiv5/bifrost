use std::sync::Arc;

use bifrost_core::{BifrostError, Result};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::{Request, Response, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tracing::{debug, error, info};

use crate::logging::{format_rules_detail, format_rules_summary, RequestContext};
use crate::request::apply_req_rules;
use crate::response::apply_res_rules;
use crate::server::{full_body, BoxBody, ResolvedRules, RulesResolver};

pub async fn handle_http_request(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Result<Response<BoxBody>> {
    let uri = req.uri().clone();
    let method = req.method().to_string();
    let url = uri.to_string();

    let resolved_rules = rules.resolve(&url, &method);

    let has_rules = !resolved_rules.rules.is_empty()
        || resolved_rules.host.is_some()
        || resolved_rules.proxy.is_some()
        || !resolved_rules.req_headers.is_empty()
        || !resolved_rules.res_headers.is_empty()
        || resolved_rules.status_code.is_some();

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

    let original_host = uri.host().unwrap_or("unknown").to_string();
    let original_port = uri.port_u16().unwrap_or(80);
    let (host, port) = extract_host_port(&uri, &resolved_rules)?;

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
        body_bytes
    };

    let stream = TcpStream::connect(format!("{}:{}", host, port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!("Failed to connect to {}:{}: {}", host, port, e))
        })?;

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

    let outgoing_req = Request::from_parts(parts, full_body(final_body));

    let res = sender
        .send_request(outgoing_req)
        .await
        .map_err(|e| BifrostError::Network(format!("Request failed: {}", e)))?;

    let (mut res_parts, res_body) = res.into_parts();

    apply_res_rules(&mut res_parts, &resolved_rules, verbose_logging, ctx);

    let res_body_bytes = res_body
        .collect()
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to read response body: {}", e)))?
        .to_bytes();

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
        res_body_bytes
    };

    Ok(Response::from_parts(res_parts, full_body(final_res_body)))
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

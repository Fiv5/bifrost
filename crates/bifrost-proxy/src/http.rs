use std::sync::Arc;
use std::time::Instant;

use bifrost_admin::{AdminState, MatchedRule, RequestTiming, TrafficRecord, TrafficType};
use bifrost_core::{protocol::Protocol, BifrostError, Result};
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

use crate::dns::DnsResolver;

use crate::body::{apply_body_rules, apply_content_injection, Phase};
use crate::decompress::{decompress_body, get_content_encoding};
use crate::logging::{format_rules_detail, format_rules_summary, RequestContext};
use crate::mock::{generate_mock_response, should_intercept_response};
use crate::request::apply_req_rules;
use crate::response::apply_res_rules;
use crate::server::{full_body, BoxBody, ResolvedRules, RulesResolver};
use crate::tee::{create_sse_tee_body, create_tee_body_with_store, store_request_body};
use crate::tunnel::get_tls_client_config;
use crate::url::apply_url_rules;

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

pub fn needs_request_body_processing(rules: &ResolvedRules) -> bool {
    rules.req_body.is_some()
        || rules.req_prepend.is_some()
        || rules.req_append.is_some()
        || !rules.req_replace.is_empty()
        || !rules.req_replace_regex.is_empty()
        || rules.req_merge.is_some()
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
        return handle_http_websocket(req, rules, ctx, admin_state).await;
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

    if resolved_rules.ignored {
        if verbose_logging {
            info!("[{}] [IGNORED] request ignored by rule", ctx.id_str());
        }
        return forward_without_rules(req, admin_state, &resolved_rules, has_rules, ctx).await;
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
    let (host, port) = extract_host_port(&processed_uri, &resolved_rules, false)?;

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

    let req_content_encoding = get_content_encoding(&req_headers);

    apply_req_rules(&mut parts, &resolved_rules, verbose_logging, ctx);

    let content_length = parts
        .headers
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());

    let needs_req_processing = needs_request_body_processing(&resolved_rules);
    let req_body_too_large = content_length
        .map(|len| len > max_body_buffer_size)
        .unwrap_or(false);

    let (body_bytes, final_body, req_body_rules_skipped) =
        if needs_req_processing && req_body_too_large {
            warn!(
                "[{}] [REQ_BODY] body too large ({} bytes > {} limit), skipping body rules",
                ctx.id_str(),
                content_length.unwrap(),
                max_body_buffer_size
            );
            let bytes = body
                .collect()
                .await
                .map_err(|e| BifrostError::Network(format!("Failed to read request body: {}", e)))?
                .to_bytes();
            (bytes.clone(), bytes, true)
        } else {
            let bytes = body
                .collect()
                .await
                .map_err(|e| BifrostError::Network(format!("Failed to read request body: {}", e)))?
                .to_bytes();

            let processed = if let Some(ref new_body) = resolved_rules.req_body {
                if verbose_logging {
                    info!(
                        "[{}] [REQ_BODY] replaced: {} bytes -> {} bytes",
                        ctx.id_str(),
                        bytes.len(),
                        new_body.len()
                    );
                }
                new_body.clone()
            } else {
                let req_content_type = parts
                    .headers
                    .get(hyper::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok());
                apply_body_rules(
                    bytes.clone(),
                    &resolved_rules,
                    Phase::Request,
                    req_content_type,
                    verbose_logging,
                    ctx,
                )
            };
            (bytes, processed, false)
        };
    let _ = req_body_rules_skipped;

    let dns_start = Instant::now();
    let (connect_host, dns_ms) = if !resolved_rules.dns_servers.is_empty() {
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
                    (ip.to_string(), Some(elapsed))
                }
                Ok(None) => {
                    debug!(
                        "[{}] [DNS] custom DNS returned None, using original host",
                        ctx.id_str()
                    );
                    (host.clone(), None)
                }
                Err(e) => {
                    debug!(
                        "[{}] [DNS] custom DNS failed: {}, using original host",
                        ctx.id_str(),
                        e
                    );
                    (host.clone(), None)
                }
            }
        } else {
            (host.clone(), None)
        }
    } else {
        (host.clone(), None)
    };

    let connect_start = Instant::now();
    let stream = TcpStream::connect(format!("{}:{}", connect_host, port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                connect_host, port, e
            ))
        })?;
    let tcp_connect_ms = connect_start.elapsed().as_millis() as u64;

    if let Err(e) = stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY on HTTP connection: {}", e);
    }

    let use_tls = matches!(
        resolved_rules.host_protocol,
        Some(Protocol::Https) | Some(Protocol::Wss)
    );

    let (mut sender, tls_ms) = if use_tls {
        let tls_start = Instant::now();
        let tls_config = get_tls_client_config(unsafe_ssl);
        let connector = TlsConnector::from(tls_config);

        let server_name = ServerName::try_from(host.clone())
            .map_err(|_| BifrostError::Network(format!("Invalid server name for TLS: {}", host)))?;

        let tls_stream = connector
            .connect(server_name, stream)
            .await
            .map_err(|e| BifrostError::Network(format!("TLS handshake failed: {}", e)))?;
        let tls_elapsed = tls_start.elapsed().as_millis() as u64;

        let io = TokioIo::new(tls_stream);
        let (sender, conn) = ClientBuilder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake(io)
            .await
            .map_err(|e| BifrostError::Network(format!("HTTP handshake failed: {}", e)))?;

        tokio::spawn(async move {
            if let Err(err) = conn.await {
                error!("Connection failed: {:?}", err);
            }
        });

        (sender, Some(tls_elapsed))
    } else {
        let io = TokioIo::new(stream);
        let (sender, conn) = ClientBuilder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake(io)
            .await
            .map_err(|e| BifrostError::Network(format!("HTTP handshake failed: {}", e)))?;

        tokio::spawn(async move {
            if let Err(err) = conn.await {
                error!("Connection failed: {:?}", err);
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

    let res_content_encoding = get_content_encoding(&res_headers);

    apply_res_rules(&mut res_parts, &resolved_rules, verbose_logging, ctx);

    let needs_processing = needs_body_processing(&resolved_rules);

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

    let skip_body_processing = !needs_processing || res_body_too_large;

    if needs_processing && res_body_too_large {
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

        if let Some(ref state) = admin_state {
            state
                .metrics_collector
                .add_bytes_sent_by_type(traffic_type, final_body.len() as u64);
            state
                .metrics_collector
                .increment_requests_by_type(traffic_type);

            let mut record = TrafficRecord::new(record_id.clone(), method, record_url.clone());
            record.status = res_parts.status.as_u16();
            record.content_type = res_parts
                .headers
                .get(hyper::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            record.request_size = final_body.len();
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
            record.response_headers = Some(res_headers);
            record.has_rule_hit = has_rules;
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
                            rule_name: r.rule_name.clone(),
                            raw: r.raw.clone(),
                            line: r.line,
                        })
                        .collect(),
                )
            };
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
            }

            if is_sse {
                record.set_sse();
                state.connection_monitor.register_connection(&record_id);
            }

            record.request_body_ref = store_request_body(
                &admin_state,
                &record_id,
                &body_bytes,
                req_content_encoding.as_deref(),
            );

            state.record_traffic(record);
        }

        if is_sse {
            let tee_body =
                create_sse_tee_body(res_body, admin_state.clone(), record_id, Some(traffic_type));
            return Ok(Response::from_parts(res_parts, tee_body.boxed()));
        } else {
            let tee_body = create_tee_body_with_store(
                res_body,
                admin_state.clone(),
                record_id,
                Some(max_body_buffer_size),
                res_content_encoding.clone(),
                Some(traffic_type),
            );
            return Ok(Response::from_parts(res_parts, tee_body.boxed()));
        }
    }

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

    if res_body_bytes.len() != final_res_body.len() {
        res_parts.headers.remove(hyper::header::CONTENT_LENGTH);
        res_parts.headers.insert(
            hyper::header::CONTENT_LENGTH,
            HeaderValue::from_str(&final_res_body.len().to_string()).unwrap(),
        );
        if verbose_logging {
            info!(
                "[{}] Updated Content-Length: {} -> {}",
                ctx.id_str(),
                res_body_bytes.len(),
                final_res_body.len()
            );
        }
    }

    let total_ms = start_time.elapsed().as_millis() as u64;

    if let Some(ref state) = admin_state {
        let traffic_type = get_traffic_type_from_url(&record_url);
        state
            .metrics_collector
            .add_bytes_sent_by_type(traffic_type, final_body.len() as u64);
        state
            .metrics_collector
            .add_bytes_received_by_type(traffic_type, final_res_body.len() as u64);
        state
            .metrics_collector
            .increment_requests_by_type(traffic_type);

        let mut record = TrafficRecord::new(ctx.id_str(), method, record_url.clone());
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
            dns_ms,
            connect_ms: Some(tcp_connect_ms),
            tls_ms,
            send_ms: None,
            wait_ms: Some(wait_ms),
            receive_ms: Some(receive_ms),
            total_ms,
        });
        record.request_headers = Some(req_headers.clone());
        record.response_headers = Some(res_headers);
        record.has_rule_hit = has_rules;
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
                        rule_name: r.rule_name.clone(),
                        raw: r.raw.clone(),
                        line: r.line,
                    })
                    .collect(),
            )
        };
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
            state.connection_monitor.register_connection(&ctx.id_str());
        }

        record.request_body_ref = store_request_body(
            &admin_state,
            &ctx.id_str(),
            &body_bytes,
            req_content_encoding.as_deref(),
        );

        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();

            let decompressed_res_body =
                decompress_body(&res_body_bytes, res_content_encoding.as_deref());
            record.response_body_ref =
                store.store(&ctx.id_str(), "res", decompressed_res_body.as_ref());
        }

        state.record_traffic(record);

        if is_sse {
            parse_and_record_sse_events(&final_res_body, &ctx.id_str(), state);
            state.connection_monitor.set_connection_closed(
                &ctx.id_str(),
                None,
                Some("SSE stream completed".to_string()),
                state.frame_store.as_ref(),
            );
        }
    }

    Ok(Response::from_parts(res_parts, full_body(final_res_body)))
}

async fn forward_without_rules(
    req: Request<Incoming>,
    admin_state: Option<Arc<AdminState>>,
    resolved_rules: &ResolvedRules,
    has_rules: bool,
    ctx: &RequestContext,
) -> Result<Response<BoxBody>> {
    let start_time = Instant::now();
    let method = req.method().to_string();
    let uri = req.uri().clone();
    let url = uri.to_string();
    let record_url = if ctx.url.is_empty() {
        url.clone()
    } else {
        ctx.url.clone()
    };
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

    if let Err(e) = stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY on HTTP connection: {}", e);
    }

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

    let req_content_encoding = get_content_encoding(&req_headers);

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

    let res_content_encoding = get_content_encoding(&res_headers);

    let total_ms = start_time.elapsed().as_millis() as u64;
    let record_id = format!(
        "{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

    let traffic_type = get_traffic_type_from_url(&record_url);

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .add_bytes_sent_by_type(traffic_type, body_bytes.len() as u64);
        state
            .metrics_collector
            .increment_requests_by_type(traffic_type);

        let mut record = TrafficRecord::new(record_id.clone(), method, record_url.clone());
        record.status = res_parts.status.as_u16();
        record.content_type = res_parts
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        record.request_size = body_bytes.len();
        record.response_size = 0;
        record.duration_ms = total_ms;
        record.timing = Some(RequestTiming {
            dns_ms: None,
            connect_ms: Some(connect_ms),
            tls_ms: None,
            send_ms: None,
            wait_ms: Some(wait_ms),
            receive_ms: None,
            total_ms,
        });
        record.request_headers = Some(req_headers.clone());
        record.response_headers = Some(res_headers);
        record.has_rule_hit = has_rules;
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
                        rule_name: r.rule_name.clone(),
                        raw: r.raw.clone(),
                        line: r.line,
                    })
                    .collect(),
            )
        };
        record.request_content_type = req_headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.clone());
        record.client_ip = ctx.client_ip.clone();
        record.client_app = ctx.client_app.clone();
        record.client_pid = ctx.client_pid;
        record.client_path = ctx.client_path.clone();

        let is_sse = is_sse_response(&res_parts);
        if is_sse {
            record.set_sse();
            state.connection_monitor.register_connection(&record_id);
        }

        record.request_body_ref = store_request_body(
            &admin_state,
            &record_id,
            &body_bytes,
            req_content_encoding.as_deref(),
        );

        state.record_traffic(record);
    }

    if is_sse_response(&res_parts) {
        let tee_body =
            create_sse_tee_body(res_body, admin_state.clone(), record_id, Some(traffic_type));
        Ok(Response::from_parts(res_parts, tee_body.boxed()))
    } else {
        let tee_body = create_tee_body_with_store(
            res_body,
            admin_state,
            record_id,
            None,
            res_content_encoding,
            Some(traffic_type),
        );
        Ok(Response::from_parts(res_parts, tee_body.boxed()))
    }
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
) -> Result<Response<BoxBody>> {
    use crate::server::empty_body;
    use crate::websocket::websocket_bidirectional_generic_with_capture;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let start_time = Instant::now();
    let uri = req.uri().clone();
    let url = uri.to_string();
    let method = req.method().to_string();

    let resolved_rules = rules.resolve(&url, "GET");
    let has_rules = !resolved_rules.rules.is_empty() || resolved_rules.host.is_some();

    let req_headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let host = uri
        .host()
        .or_else(|| {
            req.headers()
                .get(hyper::header::HOST)
                .and_then(|v| v.to_str().ok())
                .map(|h| h.split(':').next().unwrap_or(h))
        })
        .ok_or_else(|| BifrostError::Network("Missing host in WebSocket request".to_string()))?;

    let port = uri.port_u16().unwrap_or(80);

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
    let mut target_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
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

    let total_ms = start_time.elapsed().as_millis() as u64;
    let record_id = ctx.id_str();

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_requests_by_type(bifrost_admin::TrafficType::Ws);

        let ws_url = format!(
            "ws://{}:{}{}",
            host,
            port,
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
        );

        let mut record = bifrost_admin::TrafficRecord::new(record_id.clone(), method, ws_url);
        record.status = 101;
        record.protocol = "ws".to_string();
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
        record.matched_rules = if resolved_rules.rules.is_empty() {
            None
        } else {
            Some(
                resolved_rules
                    .rules
                    .iter()
                    .map(|r| bifrost_admin::MatchedRule {
                        pattern: r.pattern.clone(),
                        protocol: format!("{:?}", r.protocol),
                        value: r.value.clone(),
                        rule_name: r.rule_name.clone(),
                        raw: r.raw.clone(),
                        line: r.line,
                    })
                    .collect(),
            )
        };
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

pub fn parse_and_record_sse_events(body: &[u8], connection_id: &str, state: &AdminState) {
    let body_str = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut current_event = String::new();
    for line in body_str.lines() {
        if line.is_empty() {
            if !current_event.is_empty() {
                state.connection_monitor.record_sse_event(
                    connection_id,
                    current_event.as_bytes(),
                    state.body_store.as_ref(),
                    state.frame_store.as_ref(),
                );
                current_event.clear();
            }
        } else {
            if !current_event.is_empty() {
                current_event.push('\n');
            }
            current_event.push_str(line);
        }
    }

    if !current_event.is_empty() {
        state.connection_monitor.record_sse_event(
            connection_id,
            current_event.as_bytes(),
            state.body_store.as_ref(),
            state.frame_store.as_ref(),
        );
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

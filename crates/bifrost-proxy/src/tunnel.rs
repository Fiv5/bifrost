use std::sync::{Arc, OnceLock};
use std::time::Instant;

use bifrost_admin::{
    AdminState, ConnectionInfo, FrameDirection, FrameType, RequestTiming, TrafficRecord,
    TrafficType,
};
use bifrost_core::{BifrostError, Protocol, Result};
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::server::conn::http1::Builder as ServerBuilder;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_rustls::rustls::server::ResolvesServerCert;
use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};

use crate::body::{apply_body_rules, Phase};
use crate::dns::DnsResolver;
use crate::http::{needs_body_processing, needs_request_body_processing};
use crate::logging::{format_rules_summary, RequestContext};
use crate::protocol::{Opcode, WebSocketReader, WebSocketWriter};
use crate::response::apply_res_rules;
use crate::server::{
    empty_body, full_body, BoxBody, ProxyConfig, ResolvedRules, RulesResolver, TlsConfig,
    TlsInterceptConfig,
};
use crate::tee::create_tee_body_with_store;

use futures_util::StreamExt;

type HttpsPooledClient = Client<
    hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
    http_body_util::Full<Bytes>,
>;

static HTTPS_POOLED_CLIENT: OnceLock<HttpsPooledClient> = OnceLock::new();
static HTTPS_UNSAFE_POOLED_CLIENT: OnceLock<HttpsPooledClient> = OnceLock::new();

fn get_https_pooled_client() -> &'static HttpsPooledClient {
    HTTPS_POOLED_CLIENT.get_or_init(|| {
        let config = ClientConfig::builder()
            .with_root_certificates(build_root_cert_store())
            .with_no_client_auth();

        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(config)
            .https_or_http()
            .enable_http1()
            .build();

        Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build(https_connector)
    })
}

fn get_https_unsafe_pooled_client() -> &'static HttpsPooledClient {
    HTTPS_UNSAFE_POOLED_CLIENT.get_or_init(|| {
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(config)
            .https_or_http()
            .enable_http1()
            .build();

        Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build(https_connector)
    })
}

pub fn get_https_client(unsafe_ssl: bool) -> &'static HttpsPooledClient {
    if unsafe_ssl {
        get_https_unsafe_pooled_client()
    } else {
        get_https_pooled_client()
    }
}

pub fn get_tls_client_config(unsafe_ssl: bool) -> Arc<ClientConfig> {
    if unsafe_ssl {
        Arc::new(
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth(),
        )
    } else {
        Arc::new(
            ClientConfig::builder()
                .with_root_certificates(build_root_cert_store())
                .with_no_client_auth(),
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_connect(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    tls_intercept_config: &TlsInterceptConfig,
    proxy_config: &ProxyConfig,
    verbose_logging: bool,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Option<Arc<DnsResolver>>,
) -> Result<Response<BoxBody>> {
    let uri = req.uri().clone();
    let authority = uri
        .authority()
        .ok_or_else(|| BifrostError::Network("CONNECT request missing authority".to_string()))?;

    let host = authority.host().to_string();
    let port = authority.port_u16().unwrap_or(443);

    if verbose_logging {
        debug!(
            "[{}] CONNECT tunnel request to {}:{}",
            ctx.id_str(),
            host,
            port
        );
    } else {
        debug!("CONNECT tunnel to {}:{}", host, port);
    }

    let url = format!("https://{}:{}", host, port);
    let resolved_rules = rules.resolve(&url, "CONNECT");

    let intercept = should_intercept_tls(&host, tls_intercept_config, &tls_config, &resolved_rules);

    if intercept {
        if verbose_logging {
            let reason = if resolved_rules.tls_intercept.is_some() {
                "rule override"
            } else if is_domain_included(&host, &tls_intercept_config.intercept_include) {
                "in include list (force intercept)"
            } else {
                "global interception enabled (not excluded)"
            };
            debug!(
                "[{}] TLS interception enabled for {} ({})",
                ctx.id_str(),
                host,
                reason
            );
        }
        return handle_tls_interception(
            req,
            &host,
            port,
            rules,
            tls_config,
            verbose_logging,
            proxy_config.max_body_buffer_size,
            tls_intercept_config.unsafe_ssl,
            ctx,
            admin_state,
        )
        .await;
    } else if tls_config.ca_cert.is_some() && verbose_logging {
        let reason = if let Some(false) = resolved_rules.tls_intercept {
            "rule override (passthrough)"
        } else if is_domain_excluded(&host, &tls_intercept_config.intercept_exclude) {
            "in exclude list"
        } else {
            "global interception disabled"
        };
        debug!(
            "[{}] TLS interception skipped for {} ({})",
            ctx.id_str(),
            host,
            reason
        );
    }

    let has_rules = resolved_rules.host.is_some() || !resolved_rules.rules.is_empty();
    if verbose_logging && has_rules {
        info!(
            "[{}] CONNECT tunnel rules matched: {}",
            ctx.id_str(),
            format_rules_summary(&resolved_rules)
        );
    }

    let (target_host, target_port) = if let Some(ref host_rule) = resolved_rules.host {
        let parts: Vec<&str> = host_rule.split(':').collect();
        let h = parts[0].to_string();
        let p = if parts.len() > 1 {
            parts[1].parse().unwrap_or(port)
        } else {
            port
        };
        if verbose_logging {
            info!(
                "[{}] CONNECT tunnel target redirected: {}:{} -> {}:{}",
                ctx.id_str(),
                host,
                port,
                h,
                p
            );
        }
        (h, p)
    } else {
        (host.clone(), port)
    };

    let connect_host = if !resolved_rules.dns_servers.is_empty() {
        if let Some(ref resolver) = dns_resolver {
            if verbose_logging {
                info!(
                    "[{}] [DNS] resolving {} with custom servers: {:?}",
                    ctx.id_str(),
                    target_host,
                    resolved_rules.dns_servers
                );
            }
            match resolver
                .resolve(&target_host, &resolved_rules.dns_servers)
                .await
            {
                Ok(Some(ip)) => {
                    if verbose_logging {
                        info!(
                            "[{}] [DNS] resolved {} -> {}",
                            ctx.id_str(),
                            target_host,
                            ip
                        );
                    }
                    ip.to_string()
                }
                Ok(None) | Err(_) => target_host.clone(),
            }
        } else {
            target_host.clone()
        }
    } else {
        target_host.clone()
    };

    let target_stream = TcpStream::connect(format!("{}:{}", connect_host, target_port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                connect_host, target_port, e
            ))
        })?;

    if verbose_logging {
        info!(
            "[{}] CONNECT tunnel established to {}:{}",
            ctx.id_str(),
            target_host,
            target_port
        );
    }

    let req_id = ctx.id_str();
    let verbose = verbose_logging;

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_connections_by_type(TrafficType::Tunnel);

        let conn_info = ConnectionInfo::new(req_id.clone(), host.clone(), port, false, cancel_tx);
        state.connection_registry.register(conn_info);
    }

    let host_for_unregister = host.clone();
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                let result = tunnel_bidirectional_with_cancel(
                    upgraded,
                    target_stream,
                    verbose,
                    &req_id,
                    admin_state.as_ref(),
                    cancel_rx,
                )
                .await;
                if let Some(ref state) = admin_state {
                    state
                        .metrics_collector
                        .decrement_connections_by_type(TrafficType::Tunnel);
                    state.connection_registry.unregister(&req_id);
                }
                match result {
                    Ok(cancelled) if cancelled => {
                        info!(
                            "[{}] Tunnel {}:{} closed due to config change",
                            req_id, host_for_unregister, port
                        );
                    }
                    Err(e) => {
                        error!("[{}] Tunnel error: {}", req_id, e);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                if let Some(ref state) = admin_state {
                    state
                        .metrics_collector
                        .decrement_connections_by_type(TrafficType::Tunnel);
                    state.connection_registry.unregister(&req_id);
                }
                error!("[{}] Upgrade error: {}", req_id, e);
            }
        }
    });

    Ok(Response::builder().status(200).body(empty_body()).unwrap())
}

#[allow(clippy::too_many_arguments)]
async fn handle_tls_interception(
    req: Request<Incoming>,
    original_host: &str,
    original_port: u16,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    verbose_logging: bool,
    max_body_buffer_size: usize,
    unsafe_ssl: bool,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
) -> Result<Response<BoxBody>> {
    let certified_key = if let Some(ref sni_resolver) = tls_config.sni_resolver {
        sni_resolver.resolve(original_host)?
    } else if let Some(ref cert_generator) = tls_config.cert_generator {
        Arc::new(cert_generator.generate_for_domain(original_host)?)
    } else {
        return Err(BifrostError::Tls(
            "TLS interception enabled but cert generator not configured".to_string(),
        ));
    };

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(SingleCertResolver(certified_key)));

    let req_id = ctx.id_str();
    let verbose = verbose_logging;
    let original_host_owned = original_host.to_string();

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_connections_by_type(TrafficType::Https);

        let conn_info = ConnectionInfo::new(
            req_id.clone(),
            original_host_owned.clone(),
            original_port,
            true,
            cancel_tx,
        );
        state.connection_registry.register(conn_info);
    }

    let host_for_log = original_host_owned.clone();
    tokio::spawn(async move {
        let upgraded = match hyper::upgrade::on(req).await {
            Ok(u) => u,
            Err(e) => {
                if let Some(ref state) = admin_state {
                    state
                        .metrics_collector
                        .decrement_connections_by_type(TrafficType::Https);
                    state.connection_registry.unregister(&req_id);
                }
                error!("[{}] TLS interception upgrade error: {}", req_id, e);
                return;
            }
        };

        let result = tls_intercept_tunnel_with_cancel(
            upgraded,
            server_config,
            &original_host_owned,
            original_port,
            rules,
            verbose,
            max_body_buffer_size,
            unsafe_ssl,
            &req_id,
            admin_state.clone(),
            cancel_rx,
        )
        .await;

        if let Some(ref state) = admin_state {
            state
                .metrics_collector
                .decrement_connections_by_type(TrafficType::Https);
            state.connection_registry.unregister(&req_id);
        }

        match result {
            Ok(cancelled) if cancelled => {
                info!(
                    "[{}] TLS intercept tunnel {}:{} closed due to config change",
                    req_id, host_for_log, original_port
                );
            }
            Err(e) => {
                if verbose {
                    warn!("[{}] TLS interception error: {}", req_id, e);
                } else {
                    debug!("TLS interception error: {}", e);
                }
            }
            _ => {}
        }
    });

    Ok(Response::builder().status(200).body(empty_body()).unwrap())
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
async fn tls_intercept_tunnel(
    upgraded: Upgraded,
    server_config: ServerConfig,
    original_host: &str,
    original_port: u16,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    max_body_buffer_size: usize,
    unsafe_ssl: bool,
    req_id: &str,
    admin_state: Option<Arc<AdminState>>,
) -> Result<()> {
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let client_tls = acceptor
        .accept(TokioIo::new(upgraded))
        .await
        .map_err(|e| BifrostError::Tls(format!("TLS accept failed: {e}")))?;

    if verbose_logging {
        debug!("[{}] TLS handshake with client completed", req_id);
    }

    let original_host_for_requests = original_host.to_string();
    let original_port_for_requests = original_port;
    let req_id_owned = req_id.to_string();
    let admin_state_clone = admin_state.clone();
    let rules_clone = rules.clone();
    let verbose = verbose_logging;

    let service = service_fn(move |req: Request<Incoming>| {
        let original_host = original_host_for_requests.clone();
        let original_port = original_port_for_requests;
        let req_id = req_id_owned.clone();
        let admin_state = admin_state_clone.clone();
        let rules = rules_clone.clone();
        async move {
            handle_intercepted_request_with_protocol(
                req,
                &original_host,
                original_port,
                &req_id,
                admin_state,
                rules,
                verbose,
                max_body_buffer_size,
                unsafe_ssl,
            )
            .await
        }
    });

    let (client_read, client_write) = tokio::io::split(client_tls);
    let client_io = TokioIo::new(CombinedAsyncRw::new(client_read, client_write));

    let conn = ServerBuilder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .serve_connection(client_io, service)
        .with_upgrades();

    if let Err(e) = conn.await {
        if verbose_logging {
            debug!("[{}] HTTP connection ended: {}", req_id, e);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn tls_intercept_tunnel_with_cancel(
    upgraded: Upgraded,
    server_config: ServerConfig,
    original_host: &str,
    original_port: u16,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    max_body_buffer_size: usize,
    unsafe_ssl: bool,
    req_id: &str,
    admin_state: Option<Arc<AdminState>>,
    cancel_rx: oneshot::Receiver<()>,
) -> Result<bool> {
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let client_tls = acceptor
        .accept(TokioIo::new(upgraded))
        .await
        .map_err(|e| BifrostError::Tls(format!("TLS accept failed: {e}")))?;

    if verbose_logging {
        debug!("[{}] TLS handshake with client completed", req_id);
    }

    let original_host_for_requests = original_host.to_string();
    let original_port_for_requests = original_port;
    let req_id_owned = req_id.to_string();
    let admin_state_clone = admin_state.clone();
    let rules_clone = rules.clone();
    let verbose = verbose_logging;

    let service = service_fn(move |req: Request<Incoming>| {
        let original_host = original_host_for_requests.clone();
        let original_port = original_port_for_requests;
        let req_id = req_id_owned.clone();
        let admin_state = admin_state_clone.clone();
        let rules = rules_clone.clone();
        async move {
            handle_intercepted_request_with_protocol(
                req,
                &original_host,
                original_port,
                &req_id,
                admin_state,
                rules,
                verbose,
                max_body_buffer_size,
                unsafe_ssl,
            )
            .await
        }
    });

    let (client_read, client_write) = tokio::io::split(client_tls);
    let client_io = TokioIo::new(CombinedAsyncRw::new(client_read, client_write));

    let conn = ServerBuilder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .serve_connection(client_io, service)
        .with_upgrades();

    tokio::select! {
        result = conn => {
            if let Err(e) = result {
                if verbose_logging {
                    debug!("[{}] HTTP connection ended: {}", req_id, e);
                }
            }
            Ok(false)
        }
        _ = cancel_rx => {
            if verbose_logging {
                debug!("[{}] TLS intercept tunnel cancelled by config change", req_id);
            }
            Ok(true)
        }
    }
}

fn is_websocket_upgrade_request(req: &Request<Incoming>) -> bool {
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

#[allow(clippy::too_many_arguments)]
async fn handle_intercepted_request_with_protocol(
    req: Request<Incoming>,
    original_host: &str,
    original_port: u16,
    req_id: &str,
    admin_state: Option<Arc<AdminState>>,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    max_body_buffer_size: usize,
    unsafe_ssl: bool,
) -> std::result::Result<Response<BoxBody>, hyper::Error> {
    if is_websocket_upgrade_request(&req) {
        return handle_intercepted_websocket(
            req,
            original_host,
            original_port,
            req_id,
            admin_state,
            rules,
            verbose_logging,
        )
        .await;
    }

    let start_time = Instant::now();
    let method = req.method().clone();
    let method_str = method.to_string();
    let uri = req.uri().clone();
    let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    let query_string = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();

    let original_uri = format!("https://{}{}", original_host, path);

    let incoming_headers: std::collections::HashMap<String, String> = req
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    let incoming_cookies: std::collections::HashMap<String, String> = req
        .headers()
        .get(hyper::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(';')
                .filter_map(|part| {
                    let mut iter = part.trim().splitn(2, '=');
                    match (iter.next(), iter.next()) {
                        (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let query_params: std::collections::HashMap<String, String> = uri
        .query()
        .map(|q| {
            q.split('&')
                .filter_map(|part| {
                    let mut iter = part.splitn(2, '=');
                    match (iter.next(), iter.next()) {
                        (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let resolved_rules = rules.resolve_with_context(
        &original_uri,
        &method_str,
        &incoming_headers,
        &incoming_cookies,
    );

    let has_rules = !resolved_rules.rules.is_empty()
        || resolved_rules.host.is_some()
        || !resolved_rules.req_headers.is_empty()
        || !resolved_rules.res_headers.is_empty();

    if verbose_logging {
        if has_rules {
            info!(
                "[{}] [RULES] matched: {}",
                req_id,
                format_rules_summary(&resolved_rules)
            );
        } else {
            info!("[{}] [RULES] matched: none", req_id);
        }
    }

    let (actual_target_host, actual_target_port, actual_use_http) =
        if let Some(ref host_rule) = resolved_rules.host {
            let parts: Vec<&str> = host_rule.split(':').collect();
            let h = parts[0].to_string();
            let p = if parts.len() > 1 {
                parts[1].parse().unwrap_or(original_port)
            } else {
                match resolved_rules.host_protocol {
                    Some(Protocol::Http) | Some(Protocol::Ws) => 80,
                    Some(Protocol::Https) | Some(Protocol::Wss) => 443,
                    _ => original_port,
                }
            };
            let use_http_override = match resolved_rules.host_protocol {
                Some(Protocol::Http) | Some(Protocol::Ws) => true,
                Some(Protocol::Host) | Some(Protocol::XHost) => p != 443 && p != 8443,
                _ => false,
            };
            (h, p, use_http_override)
        } else {
            (original_host.to_string(), original_port, false)
        };

    let target_uri = if actual_use_http {
        if actual_target_port == 80 {
            format!("http://{}{}", actual_target_host, path)
        } else {
            format!(
                "http://{}:{}{}",
                actual_target_host, actual_target_port, path
            )
        }
    } else if actual_target_port == 443 {
        format!("https://{}{}", actual_target_host, path)
    } else {
        format!(
            "https://{}:{}{}",
            actual_target_host, actual_target_port, path
        )
    };

    debug!("[{}] Intercepted: {} {}", req_id, method_str, target_uri);

    if let Some(ref redirect_url) = resolved_rules.redirect {
        if verbose_logging {
            info!(
                "[{}] [REDIRECT] {} -> {}",
                req_id, original_uri, redirect_url
            );
        }
        return Ok(build_redirect_response(302, redirect_url));
    }

    if let Some(ref mock_file) = resolved_rules.mock_file {
        if verbose_logging {
            info!("[{}] [MOCK_FILE] Serving file: {}", req_id, mock_file);
        }
        let status_code = resolved_rules.status_code.unwrap_or(200);
        return Ok(serve_mock_file(mock_file, status_code, None).await);
    }

    if let Some(ref mock_template) = resolved_rules.mock_template {
        if verbose_logging {
            info!(
                "[{}] [MOCK_TPL] Serving template: {}",
                req_id, mock_template
            );
        }
        let template_vars = TemplateVars {
            url: original_uri.clone(),
            method: method_str.clone(),
            host: actual_target_host.clone(),
            pathname: path.to_string(),
            search: uri.query().map(|q| format!("?{}", q)).unwrap_or_default(),
            client_ip: "127.0.0.1".to_string(),
            req_id: req_id.to_string(),
        };
        let status_code = resolved_rules.status_code.unwrap_or(200);
        return Ok(serve_mock_file(mock_template, status_code, Some(&template_vars)).await);
    }

    if let Some(ref mock_rawfile) = resolved_rules.mock_rawfile {
        if verbose_logging {
            info!(
                "[{}] [MOCK_RAWFILE] Serving raw file: {}",
                req_id, mock_rawfile
            );
        }
        let status_code = resolved_rules.status_code.unwrap_or(200);
        return Ok(serve_mock_file(mock_rawfile, status_code, None).await);
    }

    let (parts, body) = req.into_parts();

    let actual_method = if let Some(ref method_override) = resolved_rules.method {
        if verbose_logging {
            info!(
                "[{}] [METHOD] {} -> {}",
                req_id, method_str, method_override
            );
        }
        hyper::Method::from_bytes(method_override.as_bytes()).unwrap_or(method)
    } else {
        method
    };

    let req_headers: Vec<(String, String)> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let req_content_length = parts
        .headers
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok());

    let needs_req_processing = needs_request_body_processing(&resolved_rules);
    let req_body_too_large = req_content_length
        .map(|len| len > max_body_buffer_size)
        .unwrap_or(false);

    if needs_req_processing && req_body_too_large {
        warn!(
            "[{}] [REQ_BODY] body too large ({} bytes > {} limit), skipping body rules",
            req_id,
            req_content_length.unwrap(),
            max_body_buffer_size
        );
    }

    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(e) => {
            error!("[{}] Failed to read request body: {}", req_id, e);
            return Ok(Response::builder()
                .status(502)
                .body(full_body(b"Bad Gateway".to_vec()))
                .unwrap());
        }
    };

    let parsed_uri: hyper::Uri = match target_uri.parse() {
        Ok(u) => u,
        Err(e) => {
            error!("[{}] Failed to parse URI: {}", req_id, e);
            return Ok(Response::builder()
                .status(502)
                .body(full_body(b"Bad Gateway".to_vec()))
                .unwrap());
        }
    };

    let mut new_req = Request::builder().method(actual_method).uri(&parsed_uri);

    let mut skip_referer = false;
    let mut skip_ua = false;

    for (name, value) in parts.headers.iter() {
        if name == hyper::header::HOST {
            continue;
        }
        if name == hyper::header::REFERER && resolved_rules.referer.is_some() {
            skip_referer = true;
            continue;
        }
        if name == hyper::header::USER_AGENT && resolved_rules.ua.is_some() {
            skip_ua = true;
            continue;
        }
        if name == hyper::header::COOKIE && !resolved_rules.req_cookies.is_empty() {
            continue;
        }
        new_req = new_req.header(name, value);
    }
    new_req = new_req.header(hyper::header::HOST, &actual_target_host);

    if let Some(ref referer) = resolved_rules.referer {
        if !referer.is_empty() {
            if verbose_logging {
                info!("[{}] [REFERER] -> {}", req_id, referer);
            }
            new_req = new_req.header(hyper::header::REFERER, referer);
        } else if verbose_logging && skip_referer {
            info!("[{}] [REFERER] Removed", req_id);
        }
    }

    if let Some(ref ua) = resolved_rules.ua {
        if !ua.is_empty() {
            if verbose_logging {
                info!("[{}] [USER-AGENT] -> {}", req_id, ua);
            }
            new_req = new_req.header(hyper::header::USER_AGENT, ua);
        } else if verbose_logging && skip_ua {
            info!("[{}] [USER-AGENT] Removed", req_id);
        }
    }

    for (name, value) in &resolved_rules.req_headers {
        if verbose_logging {
            info!("[{}] [REQ_HEADER] {} = {}", req_id, name, value);
        }
        new_req = new_req.header(name.as_str(), value.as_str());
    }

    if !resolved_rules.req_cookies.is_empty() {
        let existing_cookies: Vec<(String, String)> = parts
            .headers
            .get(hyper::header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|s| {
                s.split(';')
                    .filter_map(|part| {
                        let mut iter = part.trim().splitn(2, '=');
                        match (iter.next(), iter.next()) {
                            (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
                            _ => None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut cookie_map: std::collections::HashMap<String, String> =
            existing_cookies.into_iter().collect();

        for (name, value) in &resolved_rules.req_cookies {
            if verbose_logging {
                info!("[{}] [REQ_COOKIE] {} = {}", req_id, name, value);
            }
            cookie_map.insert(name.clone(), value.clone());
        }

        let cookie_str: String = cookie_map
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ");

        new_req = new_req.header(hyper::header::COOKIE, cookie_str);
    }

    let outgoing_req =
        match new_req.body(http_body_util::Full::new(Bytes::from(body_bytes.clone()))) {
            Ok(r) => r,
            Err(e) => {
                error!("[{}] Failed to build request: {}", req_id, e);
                return Ok(Response::builder()
                    .status(502)
                    .body(full_body(b"Bad Gateway".to_vec()))
                    .unwrap());
            }
        };

    if let Some(delay_ms) = resolved_rules.req_delay {
        if verbose_logging {
            info!("[{}] [REQ_DELAY] Sleeping {}ms", req_id, delay_ms);
        }
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    let client = get_https_client(unsafe_ssl);
    let send_start = Instant::now();
    let response = match client.request(outgoing_req).await {
        Ok(r) => r,
        Err(e) => {
            error!("[{}] Failed to send request: {}", req_id, e);
            return Ok(Response::builder()
                .status(502)
                .body(full_body(b"Bad Gateway".to_vec()))
                .unwrap());
        }
    };
    let wait_ms = send_start.elapsed().as_millis() as u64;

    let (mut res_parts, res_body) = response.into_parts();

    let target_status = resolved_rules.replace_status.or(resolved_rules.status_code);
    if let Some(status_code) = target_status {
        if let Ok(status) = hyper::StatusCode::from_u16(status_code) {
            if verbose_logging {
                info!(
                    "[{}] [RES_STATUS] {} -> {}",
                    req_id,
                    res_parts.status.as_u16(),
                    status_code
                );
            }
            res_parts.status = status;
        }
    }

    let res_headers: Vec<(String, String)> = res_parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let ctx = RequestContext::new()
        .with_request_info(
            original_uri.clone(),
            method_str.clone(),
            actual_target_host.clone(),
            path.to_string(),
            query_string.clone(),
            "127.0.0.1".to_string(),
        )
        .with_headers(incoming_headers.clone())
        .with_cookies(incoming_cookies.clone())
        .with_query_params(query_params.clone());
    apply_res_rules(&mut res_parts, &resolved_rules, verbose_logging, &ctx);

    let needs_processing = needs_body_processing(&resolved_rules);

    let is_websocket = res_parts.status == hyper::StatusCode::SWITCHING_PROTOCOLS
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
            req_id,
            res_content_length.unwrap(),
            max_body_buffer_size
        );
    }

    if skip_body_processing {
        let total_ms = start_time.elapsed().as_millis() as u64;
        let record_id = req_id.to_string();

        if let Some(ref state) = admin_state {
            let traffic_type = if is_websocket {
                TrafficType::Wss
            } else {
                TrafficType::Https
            };
            state
                .metrics_collector
                .add_bytes_sent_by_type(traffic_type, body_bytes.len() as u64);
            state
                .metrics_collector
                .increment_requests_by_type(traffic_type);

            let mut record = TrafficRecord::new(record_id.clone(), method_str, target_uri);
            record.status = res_parts.status.as_u16();
            record.content_type = res_parts
                .headers
                .get(hyper::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            record.request_size = body_bytes.len();
            record.response_size = 0;
            record.duration_ms = total_ms;
            record.host = original_host.to_string();
            record.timing = Some(RequestTiming {
                dns_ms: None,
                connect_ms: None,
                tls_ms: None,
                send_ms: None,
                wait_ms: Some(wait_ms),
                receive_ms: None,
                total_ms,
            });
            record.request_headers = Some(req_headers);
            record.response_headers = Some(res_headers);

            if is_websocket {
                record.protocol = "wss".to_string();
            }

            if let Some(ref body_store) = state.body_store {
                let store = body_store.read();
                record.request_body_ref = store.store(&record_id, "req", &body_bytes);
            }

            state.traffic_recorder.record(record);
        }

        if let Some(delay_ms) = resolved_rules.res_delay {
            if verbose_logging {
                info!("[{}] [RES_DELAY] Sleeping {}ms", req_id, delay_ms);
            }
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }

        let tee_body = create_tee_body_with_store(res_body, admin_state.clone(), record_id);
        return Ok(Response::from_parts(res_parts, tee_body.boxed()));
    }

    let receive_start = Instant::now();
    let res_body_bytes = match res_body.collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(e) => {
            error!("[{}] Failed to read response body: {}", req_id, e);
            return Ok(Response::builder()
                .status(502)
                .body(full_body(b"Bad Gateway".to_vec()))
                .unwrap());
        }
    };
    let receive_ms = receive_start.elapsed().as_millis() as u64;

    let total_ms = start_time.elapsed().as_millis() as u64;

    if let Some(ref state) = admin_state {
        let traffic_type = if is_websocket {
            TrafficType::Wss
        } else {
            TrafficType::Https
        };
        state
            .metrics_collector
            .add_bytes_sent_by_type(traffic_type, body_bytes.len() as u64);
        state
            .metrics_collector
            .add_bytes_received_by_type(traffic_type, res_body_bytes.len() as u64);
        state
            .metrics_collector
            .increment_requests_by_type(traffic_type);

        let mut record = TrafficRecord::new(req_id.to_string(), method_str, target_uri);
        record.status = res_parts.status.as_u16();
        record.content_type = res_parts
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        record.request_size = body_bytes.len();
        record.response_size = res_body_bytes.len();
        record.duration_ms = total_ms;
        record.host = original_host.to_string();
        record.timing = Some(RequestTiming {
            dns_ms: None,
            connect_ms: None,
            tls_ms: None,
            send_ms: None,
            wait_ms: Some(wait_ms),
            receive_ms: Some(receive_ms),
            total_ms,
        });
        record.request_headers = Some(req_headers);
        record.response_headers = Some(res_headers);

        if is_websocket {
            record.protocol = "wss".to_string();
        }

        if let Some(ref body_store) = state.body_store {
            let store = body_store.read();
            record.request_body_ref = store.store(req_id, "req", &body_bytes);
            record.response_body_ref = store.store(req_id, "res", &res_body_bytes);
        }

        state.traffic_recorder.record(record);
    }

    if let Some(delay_ms) = resolved_rules.res_delay {
        if verbose_logging {
            info!("[{}] [RES_DELAY] Sleeping {}ms", req_id, delay_ms);
        }
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    let res_content_type = res_parts
        .headers
        .get(hyper::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());
    let final_body = apply_body_rules(
        Bytes::from(res_body_bytes.clone()),
        &resolved_rules,
        Phase::Response,
        res_content_type,
        verbose_logging,
        &ctx,
    );

    if res_body_bytes.len() != final_body.len() {
        res_parts.headers.remove(hyper::header::CONTENT_LENGTH);
        res_parts.headers.insert(
            hyper::header::CONTENT_LENGTH,
            hyper::header::HeaderValue::from_str(&final_body.len().to_string()).unwrap(),
        );
        if verbose_logging {
            info!(
                "[{}] Updated Content-Length: {} -> {}",
                req_id,
                res_body_bytes.len(),
                final_body.len()
            );
        }
    }

    Ok(Response::from_parts(
        res_parts,
        full_body(final_body.to_vec()),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn handle_intercepted_websocket(
    req: Request<Incoming>,
    original_host: &str,
    original_port: u16,
    req_id: &str,
    admin_state: Option<Arc<AdminState>>,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
) -> std::result::Result<Response<BoxBody>, hyper::Error> {
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::TlsConnector;

    let start_time = Instant::now();
    let uri = req.uri().clone();
    let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    let method_str = req.method().to_string();

    if verbose_logging {
        info!("[{}] WebSocket upgrade request detected: {}", req_id, path);
    }

    let original_uri = format!("wss://{}{}", original_host, path);
    let incoming_headers: std::collections::HashMap<String, String> = req
        .headers()
        .iter()
        .map(|(k, v)| {
            (
                k.to_string().to_lowercase(),
                v.to_str().unwrap_or("").to_string(),
            )
        })
        .collect();

    let incoming_cookies: std::collections::HashMap<String, String> = req
        .headers()
        .get(hyper::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(';')
                .filter_map(|part| {
                    let mut iter = part.trim().splitn(2, '=');
                    match (iter.next(), iter.next()) {
                        (Some(k), Some(v)) => Some((k.to_string(), v.to_string())),
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let resolved_rules = rules.resolve_with_context(
        &original_uri,
        &method_str,
        &incoming_headers,
        &incoming_cookies,
    );

    let (target_host, target_port, use_http) = if let Some(ref host_rule) = resolved_rules.host {
        let parts: Vec<&str> = host_rule.split(':').collect();
        let h = parts[0].to_string();
        let p = if parts.len() > 1 {
            parts[1].parse().unwrap_or(original_port)
        } else {
            match resolved_rules.host_protocol {
                Some(Protocol::Http) | Some(Protocol::Ws) => 80,
                Some(Protocol::Https) | Some(Protocol::Wss) => 443,
                _ => original_port,
            }
        };
        let use_http_flag = match resolved_rules.host_protocol {
            Some(Protocol::Http) | Some(Protocol::Ws) => true,
            Some(Protocol::Host) | Some(Protocol::XHost) => p != 443 && p != 8443,
            _ => false,
        };
        if verbose_logging {
            info!(
                "[{}] [WS] WebSocket target resolved: wss://{}:{} -> {}://{}:{}",
                req_id,
                original_host,
                original_port,
                if use_http_flag { "ws" } else { "wss" },
                h,
                p
            );
        }
        (h, p, use_http_flag)
    } else {
        (original_host.to_string(), original_port, false)
    };

    let connect_start = Instant::now();
    let target_stream = match TcpStream::connect(format!("{}:{}", target_host, target_port)).await {
        Ok(s) => s,
        Err(e) => {
            error!(
                "[{}] Failed to connect to WebSocket target {}:{}: {}",
                req_id, target_host, target_port, e
            );
            return Ok(Response::builder()
                .status(502)
                .body(full_body(b"Bad Gateway".to_vec()))
                .unwrap());
        }
    };
    let tcp_connect_ms = connect_start.elapsed().as_millis() as u64;

    let stream: Box<dyn AsyncReadWrite + Send + Unpin> = if use_http {
        Box::new(target_stream)
    } else {
        let tls_config = get_tls_client_config(false);
        let connector = TlsConnector::from(tls_config);

        let server_name = match ServerName::try_from(target_host.to_string()) {
            Ok(name) => name,
            Err(_) => {
                error!("[{}] Invalid server name for TLS: {}", req_id, target_host);
                return Ok(Response::builder()
                    .status(502)
                    .body(full_body(b"Bad Gateway".to_vec()))
                    .unwrap());
            }
        };

        match connector.connect(server_name, target_stream).await {
            Ok(tls_stream) => Box::new(tls_stream),
            Err(e) => {
                error!("[{}] TLS handshake failed: {}", req_id, e);
                return Ok(Response::builder()
                    .status(502)
                    .body(full_body(b"Bad Gateway".to_vec()))
                    .unwrap());
            }
        }
    };

    let handshake = build_websocket_handshake_request(&req, &target_host);

    let (mut stream_read, mut stream_write) = tokio::io::split(stream);

    if let Err(e) = stream_write.write_all(handshake.as_bytes()).await {
        error!("[{}] Failed to send WebSocket handshake: {}", req_id, e);
        return Ok(Response::builder()
            .status(502)
            .body(full_body(b"Bad Gateway".to_vec()))
            .unwrap());
    }

    let mut response_buf = vec![0u8; 4096];
    let n = match stream_read.read(&mut response_buf).await {
        Ok(n) => n,
        Err(e) => {
            error!(
                "[{}] Failed to read WebSocket handshake response: {}",
                req_id, e
            );
            return Ok(Response::builder()
                .status(502)
                .body(full_body(b"Bad Gateway".to_vec()))
                .unwrap());
        }
    };

    let response_str = String::from_utf8_lossy(&response_buf[..n]);
    if !response_str.contains("101") {
        error!("[{}] WebSocket handshake failed: {}", req_id, response_str);
        return Ok(Response::builder()
            .status(502)
            .body(full_body(b"WebSocket handshake failed".to_vec()))
            .unwrap());
    }

    let sec_accept = extract_sec_websocket_accept(&response_str);
    let sec_protocol = extract_sec_websocket_protocol(&response_str);

    let total_ms = start_time.elapsed().as_millis() as u64;

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_requests_by_type(TrafficType::Wss);

        let ws_url = format!("wss://{}{}", original_host, path);
        let mut record = TrafficRecord::new(req_id.to_string(), "GET".to_string(), ws_url);
        record.status = 101;
        record.protocol = "wss".to_string();
        record.duration_ms = total_ms;
        record.timing = Some(RequestTiming {
            dns_ms: None,
            connect_ms: Some(tcp_connect_ms),
            tls_ms: if use_http {
                None
            } else {
                Some(total_ms.saturating_sub(tcp_connect_ms))
            },
            send_ms: None,
            wait_ms: None,
            receive_ms: None,
            total_ms,
        });
        record.request_headers = Some(
            req.headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect(),
        );
        record.host = original_host.to_string();
        record.set_websocket();

        state.websocket_monitor.register_connection(req_id);
        state.traffic_recorder.record(record);
    }

    if verbose_logging {
        info!(
            "[{}] WebSocket connection established to {}:{}",
            req_id, target_host, target_port
        );
    }

    let stream = stream_read.unsplit(stream_write);
    let req_id_owned = req_id.to_string();
    let admin_state_clone = admin_state.clone();

    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                if let Err(e) = websocket_bidirectional_generic_with_capture(
                    upgraded,
                    stream,
                    &req_id_owned,
                    admin_state_clone.clone(),
                )
                .await
                {
                    if verbose_logging {
                        debug!("[{}] WebSocket tunnel closed: {}", req_id_owned, e);
                    }
                }

                if let Some(ref state) = admin_state_clone {
                    state
                        .websocket_monitor
                        .set_connection_closed(&req_id_owned, None, None);
                }
            }
            Err(e) => {
                error!("[{}] WebSocket upgrade error: {}", req_id_owned, e);
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

    if let Some(protocol) = sec_protocol {
        response = response.header("Sec-WebSocket-Protocol", protocol);
    }

    Ok(response.body(empty_body()).unwrap())
}

fn build_websocket_handshake_request(req: &Request<Incoming>, target_host: &str) -> String {
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

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
        path, target_host, ws_key, ws_version
    );

    if let Some(protocol) = req
        .headers()
        .get("Sec-WebSocket-Protocol")
        .and_then(|v| v.to_str().ok())
    {
        handshake.push_str(&format!("Sec-WebSocket-Protocol: {}\r\n", protocol));
    }

    if let Some(extensions) = req
        .headers()
        .get("Sec-WebSocket-Extensions")
        .and_then(|v| v.to_str().ok())
    {
        handshake.push_str(&format!("Sec-WebSocket-Extensions: {}\r\n", extensions));
    }

    if let Some(origin) = req.headers().get("Origin").and_then(|v| v.to_str().ok()) {
        handshake.push_str(&format!("Origin: {}\r\n", origin));
    }

    handshake.push_str("\r\n");

    handshake
}

fn extract_sec_websocket_accept(response: &str) -> Option<String> {
    for line in response.lines() {
        if line.to_lowercase().starts_with("sec-websocket-accept:") {
            return Some(
                line.split(':')
                    .skip(1)
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
        }
    }
    None
}

fn extract_sec_websocket_protocol(response: &str) -> Option<String> {
    for line in response.lines() {
        if line.to_lowercase().starts_with("sec-websocket-protocol:") {
            return Some(
                line.split(':')
                    .skip(1)
                    .collect::<String>()
                    .trim()
                    .to_string(),
            );
        }
    }
    None
}

trait AsyncReadWrite: tokio::io::AsyncRead + tokio::io::AsyncWrite {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite> AsyncReadWrite for T {}

fn opcode_to_frame_type(opcode: Opcode) -> FrameType {
    match opcode {
        Opcode::Continuation => FrameType::Continuation,
        Opcode::Text => FrameType::Text,
        Opcode::Binary => FrameType::Binary,
        Opcode::Close => FrameType::Close,
        Opcode::Ping => FrameType::Ping,
        Opcode::Pong => FrameType::Pong,
    }
}

async fn websocket_bidirectional_generic_with_capture<S>(
    upgraded: Upgraded,
    target: S,
    record_id: &str,
    admin_state: Option<Arc<AdminState>>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let client = TokioIo::new(upgraded);
    let (target_read, target_write) = tokio::io::split(target);
    let (client_read, client_write) = tokio::io::split(client);

    let record_id_owned = record_id.to_string();
    let admin_state_c2s = admin_state.clone();
    let admin_state_s2c = admin_state.clone();

    let client_to_server = async move {
        let mut reader = WebSocketReader::new(client_read);
        let mut writer = WebSocketWriter::new(target_write, true);

        while let Some(result) = reader.next().await {
            let frame = match result {
                Ok(f) => f,
                Err(e) => {
                    debug!("Client read error: {}", e);
                    break;
                }
            };

            if let Some(ref state) = admin_state_c2s {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Wss, frame.payload.len() as u64);

                state.websocket_monitor.record_frame(
                    &record_id_owned,
                    FrameDirection::Send,
                    opcode_to_frame_type(frame.opcode),
                    &frame.payload,
                    frame.mask.is_some(),
                    frame.fin,
                    state.body_store.as_ref(),
                );

                if frame.opcode == Opcode::Close {
                    let close_code = frame.close_code();
                    let close_reason = frame.close_reason().map(str::to_string);
                    state.websocket_monitor.set_connection_closed(
                        &record_id_owned,
                        close_code,
                        close_reason,
                    );
                }
            }

            if let Err(e) = writer.write_frame(frame).await {
                debug!("Server write error: {}", e);
                break;
            }
        }

        Ok::<_, std::io::Error>(())
    };

    let record_id_owned2 = record_id.to_string();
    let server_to_client = async move {
        let mut reader = WebSocketReader::new(target_read);
        let mut writer = WebSocketWriter::new(client_write, false);

        while let Some(result) = reader.next().await {
            let frame = match result {
                Ok(f) => f,
                Err(e) => {
                    debug!("Server read error: {}", e);
                    break;
                }
            };

            if let Some(ref state) = admin_state_s2c {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Wss, frame.payload.len() as u64);

                state.websocket_monitor.record_frame(
                    &record_id_owned2,
                    FrameDirection::Receive,
                    opcode_to_frame_type(frame.opcode),
                    &frame.payload,
                    frame.mask.is_some(),
                    frame.fin,
                    state.body_store.as_ref(),
                );

                if frame.opcode == Opcode::Close {
                    let close_code = frame.close_code();
                    let close_reason = frame.close_reason().map(str::to_string);
                    state.websocket_monitor.set_connection_closed(
                        &record_id_owned2,
                        close_code,
                        close_reason,
                    );
                }
            }

            if let Err(e) = writer.write_frame(frame).await {
                debug!("Client write error: {}", e);
                break;
            }
        }

        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_server, server_to_client);

    match result {
        Ok(_) => {
            debug!("WebSocket connection closed normally");
            Ok(())
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::BrokenPipe
            {
                debug!("WebSocket connection closed: {}", e);
                Ok(())
            } else {
                Err(BifrostError::Network(format!("WebSocket error: {}", e)))
            }
        }
    }
}

struct SingleCertResolver(Arc<CertifiedKey>);

impl std::fmt::Debug for SingleCertResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SingleCertResolver")
    }
}

impl ResolvesServerCert for SingleCertResolver {
    fn resolve(
        &self,
        _client_hello: tokio_rustls::rustls::server::ClientHello<'_>,
    ) -> Option<Arc<CertifiedKey>> {
        Some(self.0.clone())
    }
}

fn build_root_cert_store() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    root_store
}

fn build_redirect_response(status_code: u16, location: &str) -> Response<BoxBody> {
    let body = format!(
        r#"<!DOCTYPE html><html>
<head><title>Redirect</title></head>
<body><a href="{}">Redirecting...</a></body>
</html>"#,
        location
    );

    Response::builder()
        .status(status_code)
        .header(hyper::header::LOCATION, location)
        .header(hyper::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(full_body(body.into_bytes()))
        .unwrap()
}

#[derive(Debug)]
struct NoVerifier;

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
        _server_name: &tokio_rustls::rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::danger::ServerCertVerified,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        vec![
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA512,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA512,
            tokio_rustls::rustls::SignatureScheme::ED25519,
        ]
    }
}

struct CombinedAsyncRw<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> CombinedAsyncRw<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }
}

impl<R: AsyncRead + Unpin, W: Unpin> AsyncRead for CombinedAsyncRw<R, W> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl<R: Unpin, W: AsyncWrite + Unpin> AsyncWrite for CombinedAsyncRw<R, W> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.writer).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.writer).poll_shutdown(cx)
    }
}

pub async fn tunnel_bidirectional(
    upgraded: Upgraded,
    target: TcpStream,
    verbose_logging: bool,
    req_id: &str,
    admin_state: Option<&Arc<AdminState>>,
) -> Result<()> {
    let client = TokioIo::new(upgraded);
    let (mut target_read, mut target_write) = target.into_split();

    let (client_read, client_write) = tokio::io::split(client);
    let mut client_read = client_read;
    let mut client_write = client_write;

    let admin_state_clone = admin_state.cloned();
    let admin_state_clone2 = admin_state.cloned();

    let client_to_target = async move {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Tunnel, n as u64);
            }
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(())
    };

    let target_to_client = async move {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone2 {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Tunnel, n as u64);
            }
        }
        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_target, target_to_client);

    match result {
        Ok(_) => {
            if verbose_logging {
                debug!("[{}] Tunnel closed normally", req_id);
            } else {
                debug!("Tunnel closed normally");
            }
            Ok(())
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::BrokenPipe
            {
                if verbose_logging {
                    debug!("[{}] Tunnel closed: {}", req_id, e);
                } else {
                    debug!("Tunnel closed: {}", e);
                }
                Ok(())
            } else {
                Err(BifrostError::Network(format!("Tunnel error: {}", e)))
            }
        }
    }
}

pub async fn tunnel_bidirectional_with_cancel(
    upgraded: Upgraded,
    target: TcpStream,
    verbose_logging: bool,
    req_id: &str,
    admin_state: Option<&Arc<AdminState>>,
    cancel_rx: oneshot::Receiver<()>,
) -> Result<bool> {
    let client = TokioIo::new(upgraded);
    let (mut target_read, mut target_write) = target.into_split();

    let (client_read, client_write) = tokio::io::split(client);
    let mut client_read = client_read;
    let mut client_write = client_write;

    let admin_state_clone = admin_state.cloned();
    let admin_state_clone2 = admin_state.cloned();

    let client_to_target = async move {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::Tunnel, n as u64);
            }
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(())
    };

    let target_to_client = async move {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;

            if let Some(ref state) = admin_state_clone2 {
                state
                    .metrics_collector
                    .add_bytes_received_by_type(TrafficType::Tunnel, n as u64);
            }
        }
        Ok::<_, std::io::Error>(())
    };

    let bidirectional = async { tokio::try_join!(client_to_target, target_to_client) };

    tokio::select! {
        result = bidirectional => {
            match result {
                Ok(_) => {
                    if verbose_logging {
                        debug!("[{}] Tunnel closed normally", req_id);
                    } else {
                        debug!("Tunnel closed normally");
                    }
                    Ok(false)
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::ConnectionReset
                        || e.kind() == std::io::ErrorKind::BrokenPipe
                    {
                        if verbose_logging {
                            debug!("[{}] Tunnel closed: {}", req_id, e);
                        } else {
                            debug!("Tunnel closed: {}", e);
                        }
                        Ok(false)
                    } else {
                        Err(BifrostError::Network(format!("Tunnel error: {}", e)))
                    }
                }
            }
        }
        _ = cancel_rx => {
            if verbose_logging {
                debug!("[{}] Tunnel cancelled by config change", req_id);
            }
            Ok(true)
        }
    }
}

fn is_domain_matched(host: &str, patterns: &[String]) -> bool {
    if patterns.is_empty() {
        return false;
    }

    let host_lower = host.to_lowercase();
    for pattern in patterns {
        let pattern_lower = pattern.to_lowercase();

        if let Some(base_domain) = pattern_lower.strip_prefix("*.") {
            let suffix = format!(".{}", base_domain);
            if host_lower.ends_with(&suffix) || host_lower == base_domain {
                return true;
            }
        } else if host_lower == pattern_lower
            || host_lower.ends_with(&format!(".{}", pattern_lower))
        {
            return true;
        }
    }

    false
}

fn is_domain_excluded(host: &str, exclude_list: &[String]) -> bool {
    is_domain_matched(host, exclude_list)
}

fn is_domain_included(host: &str, include_list: &[String]) -> bool {
    is_domain_matched(host, include_list)
}

pub fn should_intercept_tls(
    host: &str,
    tls_intercept_config: &TlsInterceptConfig,
    tls_config: &TlsConfig,
    resolved_rules: &ResolvedRules,
) -> bool {
    if tls_config.ca_cert.is_none() {
        return false;
    }

    if let Some(rule_intercept) = resolved_rules.tls_intercept {
        return rule_intercept;
    }

    if is_domain_included(host, &tls_intercept_config.intercept_include) {
        return true;
    }

    if is_domain_excluded(host, &tls_intercept_config.intercept_exclude) {
        return false;
    }

    tls_intercept_config.enable_tls_interception
}

pub fn parse_connect_authority(authority: &str) -> Result<(String, u16)> {
    let parts: Vec<&str> = authority.split(':').collect();
    match parts.len() {
        1 => Ok((parts[0].to_string(), 443)),
        2 => {
            let port = parts[1]
                .parse()
                .map_err(|_| BifrostError::Parse(format!("Invalid port: {}", parts[1])))?;
            Ok((parts[0].to_string(), port))
        }
        _ => Err(BifrostError::Parse(format!(
            "Invalid authority: {}",
            authority
        ))),
    }
}

struct TemplateVars {
    url: String,
    method: String,
    host: String,
    pathname: String,
    search: String,
    client_ip: String,
    req_id: String,
}

fn process_template(content: &str, vars: &TemplateVars) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_default();

    let random: u64 = rand::random();

    content
        .replace("${url}", &vars.url)
        .replace("${method}", &vars.method)
        .replace("${host}", &vars.host)
        .replace("${pathname}", &vars.pathname)
        .replace("${path}", &vars.pathname)
        .replace("${search}", &vars.search)
        .replace("${query}", &vars.search)
        .replace("${clientIp}", &vars.client_ip)
        .replace("${reqId}", &vars.req_id)
        .replace("${now}", &now)
        .replace("${timestamp}", &now)
        .replace("${random}", &random.to_string())
}

async fn serve_mock_file(
    file_path: &str,
    status_code: u16,
    template_vars: Option<&TemplateVars>,
) -> Response<BoxBody> {
    match tokio::fs::read_to_string(file_path).await {
        Ok(content) => {
            let body = if let Some(vars) = template_vars {
                process_template(&content, vars)
            } else {
                content
            };

            let content_type = if file_path.ends_with(".json") || body.trim_start().starts_with('{')
            {
                "application/json"
            } else if file_path.ends_with(".html") {
                "text/html; charset=utf-8"
            } else if file_path.ends_with(".xml") {
                "application/xml"
            } else {
                "text/plain; charset=utf-8"
            };

            Response::builder()
                .status(status_code)
                .header(hyper::header::CONTENT_TYPE, content_type)
                .body(full_body(body.into_bytes()))
                .unwrap()
        }
        Err(e) => {
            error!("Failed to read mock file {}: {}", file_path, e);
            Response::builder()
                .status(500)
                .body(full_body(
                    format!("Failed to read file: {}", e).into_bytes(),
                ))
                .unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect_authority_with_port() {
        let (host, port) = parse_connect_authority("example.com:443").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_connect_authority_custom_port() {
        let (host, port) = parse_connect_authority("example.com:8443").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 8443);
    }

    #[test]
    fn test_parse_connect_authority_default_port() {
        let (host, port) = parse_connect_authority("example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_connect_authority_invalid_port() {
        let result = parse_connect_authority("example.com:invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_connect_authority_multiple_colons() {
        let result = parse_connect_authority("example.com:443:extra");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_domain_excluded_exact_match() {
        let exclude = vec!["example.com".to_string()];
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(!is_domain_excluded("other.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_subdomain_match() {
        let exclude = vec!["example.com".to_string()];
        assert!(is_domain_excluded("sub.example.com", &exclude));
        assert!(is_domain_excluded("deep.sub.example.com", &exclude));
        assert!(!is_domain_excluded("notexample.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_wildcard() {
        let exclude = vec!["*.example.com".to_string()];
        assert!(is_domain_excluded("sub.example.com", &exclude));
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(!is_domain_excluded("other.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_case_insensitive() {
        let exclude = vec!["Example.COM".to_string()];
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(is_domain_excluded("EXAMPLE.COM", &exclude));
        assert!(is_domain_excluded("Sub.Example.Com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_empty_list() {
        let exclude: Vec<String> = vec![];
        assert!(!is_domain_excluded("example.com", &exclude));
    }

    #[test]
    fn test_is_domain_excluded_multiple_patterns() {
        let exclude = vec![
            "example.com".to_string(),
            "*.google.com".to_string(),
            "internal.corp".to_string(),
        ];
        assert!(is_domain_excluded("example.com", &exclude));
        assert!(is_domain_excluded("maps.google.com", &exclude));
        assert!(is_domain_excluded("api.internal.corp", &exclude));
        assert!(!is_domain_excluded("other.com", &exclude));
    }

    fn make_tls_config_with_ca() -> TlsConfig {
        TlsConfig {
            ca_cert: Some(vec![1, 2, 3]),
            ca_key: Some(vec![1, 2, 3]),
            cert_generator: None,
            sni_resolver: None,
        }
    }

    fn make_tls_config_without_ca() -> TlsConfig {
        TlsConfig {
            ca_cert: None,
            ca_key: None,
            cert_generator: None,
            sni_resolver: None,
        }
    }

    fn make_tls_intercept_config(
        enable: bool,
        exclude: Vec<String>,
        include: Vec<String>,
    ) -> TlsInterceptConfig {
        TlsInterceptConfig {
            enable_tls_interception: enable,
            intercept_exclude: exclude,
            intercept_include: include,
            unsafe_ssl: false,
        }
    }

    #[test]
    fn test_should_intercept_no_ca_cert() {
        let tls_intercept_config = make_tls_intercept_config(true, vec![], vec![]);
        let tls_config = make_tls_config_without_ca();
        let resolved_rules = ResolvedRules::default();

        let result = should_intercept_tls(
            "example.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            !result,
            "Should NOT intercept when CA cert is not available"
        );
        println!("✓ No CA cert: intercept={}", result);
    }

    #[test]
    fn test_should_intercept_enabled_default() {
        let tls_intercept_config = make_tls_intercept_config(true, vec![], vec![]);
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules::default();

        let result = should_intercept_tls(
            "example.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(result, "Should intercept when enabled with empty lists");
        println!("✓ Enabled (empty lists): intercept={}", result);
    }

    #[test]
    fn test_should_intercept_excluded_domains() {
        let tls_intercept_config = make_tls_intercept_config(
            true,
            vec!["*.apple.com".to_string(), "example.com".to_string()],
            vec![],
        );
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules::default();

        let result1 = should_intercept_tls(
            "example.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(!result1, "Should NOT intercept excluded domain");
        println!("✓ Excluded (example.com): intercept={}", result1);

        let result2 = should_intercept_tls(
            "api.apple.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(!result2, "Should NOT intercept wildcard excluded domain");
        println!("✓ Excluded (*.apple.com): intercept={}", result2);

        let result3 = should_intercept_tls(
            "other.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(result3, "Should intercept non-excluded domain");
        println!("✓ Not excluded (other.com): intercept={}", result3);
    }

    #[test]
    fn test_should_intercept_include_force_intercept() {
        let tls_intercept_config = make_tls_intercept_config(
            false,
            vec![],
            vec!["*.api.example.com".to_string(), "secure.local".to_string()],
        );
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules::default();

        let result1 = should_intercept_tls(
            "secure.local",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            result1,
            "Should intercept included domain even when globally disabled"
        );
        println!(
            "✓ Included (secure.local, global disabled): intercept={}",
            result1
        );

        let result2 = should_intercept_tls(
            "test.api.example.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            result2,
            "Should intercept wildcard included domain even when globally disabled"
        );
        println!(
            "✓ Included (*.api.example.com, global disabled): intercept={}",
            result2
        );

        let result3 = should_intercept_tls(
            "other.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            !result3,
            "Should NOT intercept non-included domain when globally disabled"
        );
        println!(
            "✓ Not included (other.com, global disabled): intercept={}",
            result3
        );
    }

    #[test]
    fn test_should_intercept_include_has_highest_priority() {
        let tls_intercept_config = make_tls_intercept_config(
            true,
            vec!["secure.local".to_string()],
            vec!["secure.local".to_string()],
        );
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules::default();

        let result = should_intercept_tls(
            "secure.local",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            result,
            "Include list should have higher priority than exclude list"
        );
        println!("✓ Include > Exclude priority: intercept={}", result);
    }

    #[test]
    fn test_should_intercept_rule_override_intercept() {
        let tls_intercept_config = make_tls_intercept_config(false, vec![], vec![]);
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules {
            tls_intercept: Some(true),
            ..Default::default()
        };

        let result = should_intercept_tls(
            "any.domain.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            result,
            "Rule override (tlsIntercept://) should force interception"
        );
        println!("✓ Rule override tlsIntercept://: intercept={}", result);
    }

    #[test]
    fn test_should_intercept_rule_override_passthrough() {
        let tls_intercept_config = make_tls_intercept_config(true, vec![], vec![]);
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules {
            tls_intercept: Some(false),
            ..Default::default()
        };

        let result = should_intercept_tls(
            "any.domain.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            !result,
            "Rule override (tlsPassthrough://) should force passthrough"
        );
        println!("✓ Rule override tlsPassthrough://: intercept={}", result);
    }

    #[test]
    fn test_should_intercept_disabled_globally() {
        let tls_intercept_config = make_tls_intercept_config(false, vec![], vec![]);
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules::default();

        let result = should_intercept_tls(
            "example.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(!result, "Should NOT intercept when globally disabled");
        println!("✓ Global disabled: intercept={}", result);
    }

    #[test]
    fn test_should_intercept_rule_overrides_global_disable() {
        let tls_intercept_config = make_tls_intercept_config(false, vec![], vec![]);
        let tls_config = make_tls_config_with_ca();
        let resolved_rules = ResolvedRules {
            tls_intercept: Some(true),
            ..Default::default()
        };

        let result = should_intercept_tls(
            "example.com",
            &tls_intercept_config,
            &tls_config,
            &resolved_rules,
        );
        assert!(
            result,
            "Rule override should work even when globally disabled"
        );
        println!("✓ Rule override with global disabled: intercept={}", result);
    }
}

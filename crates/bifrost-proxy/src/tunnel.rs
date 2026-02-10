use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use bifrost_admin::{AdminState, RequestTiming, TrafficRecord};
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
use tokio_rustls::rustls::server::ResolvesServerCert;
use tokio_rustls::rustls::sign::CertifiedKey;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};

use crate::http::needs_body_processing;
use crate::logging::{format_rules_summary, RequestContext};
use crate::server::{empty_body, full_body, BoxBody, ProxyConfig, RulesResolver, TlsConfig};
use crate::tee::create_tee_body_with_store;

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

pub async fn handle_connect(
    req: Request<Incoming>,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    proxy_config: &ProxyConfig,
    verbose_logging: bool,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
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

    let has_rules = resolved_rules.host.is_some() || !resolved_rules.rules.is_empty();
    if verbose_logging && has_rules {
        info!(
            "[{}] CONNECT rules matched: {}",
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
                "[{}] CONNECT target redirected: {}:{} -> {}:{}",
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

    let should_intercept = proxy_config.enable_tls_interception
        && tls_config.ca_cert.is_some()
        && !is_domain_excluded(&host, &proxy_config.intercept_exclude);

    if should_intercept {
        if verbose_logging {
            debug!("[{}] TLS interception enabled", ctx.id_str());
        }
        return handle_tls_interception(
            req,
            &host,
            &target_host,
            target_port,
            rules,
            tls_config,
            verbose_logging,
            ctx,
            admin_state,
            resolved_rules.host_protocol,
        )
        .await;
    } else if proxy_config.enable_tls_interception
        && tls_config.ca_cert.is_some()
        && verbose_logging
    {
        debug!(
            "[{}] TLS interception skipped (domain excluded): {}",
            ctx.id_str(),
            host
        );
    }

    let target_stream = TcpStream::connect(format!("{}:{}", target_host, target_port))
        .await
        .map_err(|e| {
            BifrostError::Network(format!(
                "Failed to connect to {}:{}: {}",
                target_host, target_port, e
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
    if let Some(ref state) = admin_state {
        state.metrics_collector.increment_connections();
    }
    tokio::spawn(async move {
        match hyper::upgrade::on(req).await {
            Ok(upgraded) => {
                let result = tunnel_bidirectional(
                    upgraded,
                    target_stream,
                    verbose,
                    &req_id,
                    admin_state.as_ref(),
                )
                .await;
                if let Some(ref state) = admin_state {
                    state.metrics_collector.decrement_connections();
                }
                if let Err(e) = result {
                    error!("[{}] Tunnel error: {}", req_id, e);
                }
            }
            Err(e) => {
                if let Some(ref state) = admin_state {
                    state.metrics_collector.decrement_connections();
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
    target_host: &str,
    target_port: u16,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    verbose_logging: bool,
    ctx: &RequestContext,
    admin_state: Option<Arc<AdminState>>,
    host_protocol: Option<Protocol>,
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
    let original_host = original_host.to_string();
    let target_host = target_host.to_string();

    if let Some(ref state) = admin_state {
        state.metrics_collector.increment_connections();
    }

    tokio::spawn(async move {
        let upgraded = match hyper::upgrade::on(req).await {
            Ok(u) => u,
            Err(e) => {
                if let Some(ref state) = admin_state {
                    state.metrics_collector.decrement_connections();
                }
                error!("[{}] TLS interception upgrade error: {}", req_id, e);
                return;
            }
        };

        let result = tls_intercept_tunnel(
            upgraded,
            server_config,
            &original_host,
            &target_host,
            target_port,
            rules,
            verbose,
            &req_id,
            admin_state.clone(),
            host_protocol,
        )
        .await;

        if let Some(ref state) = admin_state {
            state.metrics_collector.decrement_connections();
        }

        if let Err(e) = result {
            if verbose {
                warn!("[{}] TLS interception error: {}", req_id, e);
            } else {
                debug!("TLS interception error: {}", e);
            }
        }
    });

    Ok(Response::builder().status(200).body(empty_body()).unwrap())
}

#[allow(clippy::too_many_arguments)]
async fn tls_intercept_tunnel(
    upgraded: Upgraded,
    server_config: ServerConfig,
    original_host: &str,
    target_host: &str,
    target_port: u16,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
    req_id: &str,
    admin_state: Option<Arc<AdminState>>,
    host_protocol: Option<Protocol>,
) -> Result<()> {
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let client_tls = acceptor
        .accept(TokioIo::new(upgraded))
        .await
        .map_err(|e| BifrostError::Tls(format!("TLS accept failed: {e}")))?;

    if verbose_logging {
        debug!("[{}] TLS handshake with client completed", req_id);
    }

    let use_http = match host_protocol {
        Some(Protocol::Http) | Some(Protocol::Ws) => true,
        Some(Protocol::Host) | Some(Protocol::XHost) => target_port != 443 && target_port != 8443,
        _ => false,
    };

    if use_http && verbose_logging {
        debug!(
            "[{}] Using HTTP to connect to target (protocol downgrade)",
            req_id
        );
    }

    let original_host_for_requests = original_host.to_string();
    let target_host_for_requests = target_host.to_string();
    let target_port_for_requests = target_port;
    let req_id_owned = req_id.to_string();
    let admin_state_clone = admin_state.clone();
    let rules_clone = rules.clone();
    let verbose = verbose_logging;

    let service = service_fn(move |req: Request<Incoming>| {
        let original_host = original_host_for_requests.clone();
        let target_host = target_host_for_requests.clone();
        let target_port = target_port_for_requests;
        let req_id = req_id_owned.clone();
        let admin_state = admin_state_clone.clone();
        let rules = rules_clone.clone();
        async move {
            handle_intercepted_request_with_protocol(
                req,
                &original_host,
                &target_host,
                target_port,
                &req_id,
                admin_state,
                use_http,
                rules,
                verbose,
            )
            .await
        }
    });

    let (client_read, client_write) = tokio::io::split(client_tls);
    let client_io = TokioIo::new(CombinedAsyncRw::new(client_read, client_write));

    let conn = ServerBuilder::new().serve_connection(client_io, service);

    if let Err(e) = conn.await {
        if verbose_logging {
            debug!("[{}] HTTP connection ended: {}", req_id, e);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_intercepted_request_with_protocol(
    req: Request<Incoming>,
    original_host: &str,
    target_host: &str,
    target_port: u16,
    req_id: &str,
    admin_state: Option<Arc<AdminState>>,
    use_http: bool,
    rules: Arc<dyn RulesResolver>,
    verbose_logging: bool,
) -> std::result::Result<Response<BoxBody>, hyper::Error> {
    let start_time = Instant::now();
    let method = req.method().clone();
    let method_str = method.to_string();
    let uri = req.uri().clone();
    let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

    let original_uri = format!("https://{}{}", original_host, path);

    let target_uri = if use_http {
        if target_port == 80 {
            format!("http://{}{}", target_host, path)
        } else {
            format!("http://{}:{}{}", target_host, target_port, path)
        }
    } else if target_port == 443 {
        format!("https://{}{}", target_host, path)
    } else {
        format!("https://{}:{}{}", target_host, target_port, path)
    };

    debug!("[{}] Intercepted: {} {}", req_id, method_str, target_uri);

    let resolved_rules = rules.resolve(&original_uri, &method_str);

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
            host: target_host.to_string(),
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
        new_req = new_req.header(name, value);
    }
    new_req = new_req.header(hyper::header::HOST, target_host);

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

    let client = get_https_pooled_client();
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

    for (name, value) in &resolved_rules.res_headers {
        if verbose_logging {
            info!("[{}] [RES_HEADER] {} = {}", req_id, name, value);
        }
        if let Ok(header_name) = hyper::header::HeaderName::from_bytes(name.as_bytes()) {
            if let Ok(header_value) = hyper::header::HeaderValue::from_str(value) {
                res_parts.headers.insert(header_name, header_value);
            }
        }
    }

    let needs_processing = needs_body_processing(&resolved_rules);

    if !needs_processing {
        let total_ms = start_time.elapsed().as_millis() as u64;
        let record_id = req_id.to_string();

        if let Some(ref state) = admin_state {
            state
                .metrics_collector
                .add_bytes_sent(body_bytes.len() as u64);
            state.metrics_collector.increment_requests();

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
        state
            .metrics_collector
            .add_bytes_sent(body_bytes.len() as u64);
        state
            .metrics_collector
            .add_bytes_received(res_body_bytes.len() as u64);
        state.metrics_collector.increment_requests();

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

    let final_body = if let Some(ref new_body) = resolved_rules.res_body {
        if verbose_logging {
            info!(
                "[{}] [RES_BODY] replaced: {} bytes -> {} bytes",
                req_id,
                res_body_bytes.len(),
                new_body.len()
            );
        }
        new_body.to_vec()
    } else {
        res_body_bytes
    };

    Ok(Response::from_parts(res_parts, full_body(final_body)))
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

    let bytes_sent = Arc::new(AtomicU64::new(0));
    let bytes_received = Arc::new(AtomicU64::new(0));
    let bytes_sent_clone = bytes_sent.clone();
    let bytes_received_clone = bytes_received.clone();

    let client_to_target = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = client_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            target_write.write_all(&buf[..n]).await?;
            bytes_sent_clone.fetch_add(n as u64, Ordering::Relaxed);
        }
        target_write.shutdown().await?;
        Ok::<_, std::io::Error>(())
    };

    let target_to_client = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = target_read.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            client_write.write_all(&buf[..n]).await?;
            bytes_received_clone.fetch_add(n as u64, Ordering::Relaxed);
        }
        Ok::<_, std::io::Error>(())
    };

    let result = tokio::try_join!(client_to_target, target_to_client);

    if let Some(state) = admin_state {
        state
            .metrics_collector
            .add_bytes_sent(bytes_sent.load(Ordering::Relaxed));
        state
            .metrics_collector
            .add_bytes_received(bytes_received.load(Ordering::Relaxed));
    }

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

fn is_domain_excluded(host: &str, exclude_list: &[String]) -> bool {
    if exclude_list.is_empty() {
        return false;
    }

    let host_lower = host.to_lowercase();
    for pattern in exclude_list {
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
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::client::conn::http1::Builder as ClientBuilder;
use hyper::{Request, Response, Uri};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio_rustls::TlsConnector;
use tracing::{debug, error, info, warn};

use crate::replay_db::{ReplayHistory, RuleConfig, RuleMode, MAX_CONCURRENT_REPLAYS};
use crate::state::SharedAdminState;
use crate::traffic::{MatchedRule, RequestTiming, TrafficRecord};

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

#[derive(Debug)]
pub enum ReplayError {
    TooManyConcurrent,
    InvalidUrl(String),
    ConnectionFailed(String),
    RequestFailed(String),
    Internal(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::TooManyConcurrent => {
                write!(f, "Too many concurrent replay requests")
            }
            ReplayError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            ReplayError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ReplayError::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            ReplayError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ReplayError {}

pub type SharedReplayExecutor = Arc<ReplayExecutor>;

static REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub struct ReplayExecutor {
    semaphore: Arc<Semaphore>,
    admin_state: SharedAdminState,
    unsafe_ssl: bool,
}

impl ReplayExecutor {
    pub fn new(admin_state: SharedAdminState, unsafe_ssl: bool) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REPLAYS)),
            admin_state,
            unsafe_ssl,
        }
    }

    pub async fn execute(
        &self,
        request: ReplayExecuteRequest,
    ) -> Result<ReplayExecuteResponse, ReplayError> {
        let permit = self
            .semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| ReplayError::TooManyConcurrent)?;

        let result = self.execute_inner(request).await;
        drop(permit);
        result
    }

    async fn execute_inner(
        &self,
        request: ReplayExecuteRequest,
    ) -> Result<ReplayExecuteResponse, ReplayError> {
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

        let uri: Uri = url
            .parse()
            .map_err(|e| ReplayError::InvalidUrl(format!("{}", e)))?;

        let is_https = uri.scheme_str() == Some("https");
        let host = uri
            .host()
            .ok_or_else(|| ReplayError::InvalidUrl("Missing host".to_string()))?
            .to_string();
        let port = uri.port_u16().unwrap_or(if is_https { 443 } else { 80 });
        let path = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

        let resolved_rules = self.resolve_rules(&request.rule_config, url, method);

        let applied_rules: Vec<MatchedRule> = resolved_rules
            .iter()
            .map(|r| MatchedRule {
                pattern: r.pattern.clone(),
                protocol: r.protocol.clone(),
                value: r.value.clone(),
                rule_name: r.rule_name.clone(),
                raw: r.raw.clone(),
                line: r.line,
            })
            .collect();

        let mut timing = RequestTiming::default();
        let dns_start = Instant::now();

        let connect_addr = format!("{}:{}", host, port);
        timing.dns_ms = Some(dns_start.elapsed().as_millis() as u64);

        let connect_start = Instant::now();
        let tcp_stream = TcpStream::connect(&connect_addr).await.map_err(|e| {
            ReplayError::ConnectionFailed(format!("Failed to connect to {}: {}", connect_addr, e))
        })?;
        timing.connect_ms = Some(connect_start.elapsed().as_millis() as u64);

        let (status, response_headers, response_body, tls_ms) = if is_https {
            let tls_start = Instant::now();
            let (s, h, b) = self
                .send_https_request(
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
            let (s, h, b) = self
                .send_http_request(
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

        let traffic_id = self
            .record_traffic(
                &replay_id,
                &request,
                status,
                &response_headers,
                response_body.as_deref(),
                duration_ms,
                &applied_rules,
                &timing,
            )
            .await;

        if let Some(request_id) = &request.request_id {
            self.record_history(
                request_id,
                &traffic_id,
                method,
                url,
                status,
                duration_ms,
                &request.rule_config,
            )
            .await;
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

    fn resolve_rules(
        &self,
        rule_config: &RuleConfig,
        url: &str,
        method: &str,
    ) -> Vec<ResolvedRuleInfo> {
        match rule_config.mode {
            RuleMode::None => vec![],
            RuleMode::Enabled | RuleMode::Selected => {
                let rules_storage = &self.admin_state.rules_storage;
                let selected = if rule_config.mode == RuleMode::Selected {
                    Some(&rule_config.selected_rules)
                } else {
                    None
                };
                self.resolve_from_storage(rules_storage, url, method, selected)
            }
        }
    }

    fn resolve_from_storage(
        &self,
        rules_storage: &bifrost_storage::RulesStorage,
        url: &str,
        method: &str,
        selected_rules: Option<&Vec<String>>,
    ) -> Vec<ResolvedRuleInfo> {
        let mut result = vec![];

        let rule_files = match rules_storage.load_all() {
            Ok(files) => files,
            Err(e) => {
                warn!(error = %e, "[REPLAY] Failed to load rules");
                return vec![];
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

            if let Some(matched) =
                self.match_rules_in_content(&rule_file.content, url, method, &rule_file.name)
            {
                result.extend(matched);
            }
        }

        result
    }

    fn match_rules_in_content(
        &self,
        _content: &str,
        _url: &str,
        _method: &str,
        rule_name: &str,
    ) -> Option<Vec<ResolvedRuleInfo>> {
        debug!(
            rule_name = %rule_name,
            "[REPLAY] Checking rules (simplified matching)"
        );
        None
    }

    async fn send_http_request(
        &self,
        stream: TcpStream,
        host: &str,
        method: &str,
        path: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<(u16, Vec<(String, String)>, Option<String>), ReplayError> {
        let io = TokioIo::new(stream);

        let (mut sender, conn) = ClientBuilder::new()
            .handshake(io)
            .await
            .map_err(|e| ReplayError::ConnectionFailed(format!("HTTP handshake failed: {}", e)))?;

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                error!(error = %e, "[REPLAY] HTTP connection error");
            }
        });

        let mut req_builder = Request::builder()
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
            .map_err(|e| ReplayError::RequestFailed(format!("Failed to build request: {}", e)))?;

        let response = sender
            .send_request(request)
            .await
            .map_err(|e| ReplayError::RequestFailed(format!("Request failed: {}", e)))?;

        self.parse_response(response).await
    }

    async fn send_https_request(
        &self,
        stream: TcpStream,
        host: &str,
        method: &str,
        path: &str,
        headers: &[(String, String)],
        body: Option<&str>,
    ) -> Result<(u16, Vec<(String, String)>, Option<String>), ReplayError> {
        let tls_config = get_tls_client_config(self.unsafe_ssl);
        let connector = TlsConnector::from(Arc::new(tls_config));

        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| ReplayError::ConnectionFailed(format!("Invalid server name: {}", e)))?;

        let tls_stream = connector
            .connect(server_name, stream)
            .await
            .map_err(|e| ReplayError::ConnectionFailed(format!("TLS handshake failed: {}", e)))?;

        let io = TokioIo::new(tls_stream);

        let (mut sender, conn) = ClientBuilder::new()
            .handshake(io)
            .await
            .map_err(|e| ReplayError::ConnectionFailed(format!("HTTPS handshake failed: {}", e)))?;

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                error!(error = %e, "[REPLAY] HTTPS connection error");
            }
        });

        let mut req_builder = Request::builder()
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
            .map_err(|e| ReplayError::RequestFailed(format!("Failed to build request: {}", e)))?;

        let response = sender
            .send_request(request)
            .await
            .map_err(|e| ReplayError::RequestFailed(format!("Request failed: {}", e)))?;

        self.parse_response(response).await
    }

    async fn parse_response<B>(
        &self,
        response: Response<B>,
    ) -> Result<(u16, Vec<(String, String)>, Option<String>), ReplayError>
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
            .map_err(|e| {
                ReplayError::RequestFailed(format!("Failed to read response body: {}", e))
            })?
            .to_bytes();

        let body = if body_bytes.is_empty() {
            None
        } else {
            Some(String::from_utf8_lossy(&body_bytes).to_string())
        };

        Ok((status, headers, body))
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_traffic(
        &self,
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

        if let Some(ref traffic_db) = self.admin_state.traffic_db_store {
            traffic_db.record(record);
        } else if let Some(ref async_writer) = self.admin_state.async_traffic_writer {
            async_writer.record(record);
        }

        if let Some(body) = request.request.body.as_ref() {
            if let Some(ref body_store) = self.admin_state.body_store {
                let _ = body_store.read().store(&traffic_id, "req", body.as_bytes());
            }
        }

        if let Some(body) = response_body {
            if let Some(ref body_store) = self.admin_state.body_store {
                let _ = body_store.read().store(&traffic_id, "res", body.as_bytes());
            }
        }

        traffic_id
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_history(
        &self,
        request_id: &str,
        traffic_id: &str,
        method: &str,
        url: &str,
        status: u16,
        duration_ms: u64,
        rule_config: &RuleConfig,
    ) {
        if let Some(ref replay_db) = self.admin_state.replay_db_store {
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
}

#[derive(Debug, Clone)]
struct ResolvedRuleInfo {
    pattern: String,
    protocol: String,
    value: String,
    rule_name: Option<String>,
    raw: Option<String>,
    line: Option<usize>,
}

fn get_tls_client_config(unsafe_ssl: bool) -> rustls::ClientConfig {
    use rustls::{ClientConfig, RootCertStore};

    if unsafe_ssl {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertificateVerification {}))
            .with_no_client_auth()
    } else {
        let mut root_store = RootCertStore::empty();
        let certs = rustls_native_certs::load_native_certs();
        for cert in certs.certs {
            let _ = root_store.add(cert);
        }

        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    }
}

#[derive(Debug)]
struct NoCertificateVerification;

impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

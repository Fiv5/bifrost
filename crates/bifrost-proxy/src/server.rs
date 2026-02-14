use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use regex::Regex;

use bifrost_admin::{
    is_cert_public_request, is_valid_admin_request, AdminRouter, AdminSecurityConfig, AdminState,
    ADMIN_PATH_PREFIX, CERT_PUBLIC_PATH_PREFIX,
};
use bifrost_core::{BifrostError, Protocol, Result};
use bytes::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::dns::DnsResolver;
use crate::http::handle_http_request;
use crate::logging::RequestContext;
use crate::tunnel::handle_connect;
use bifrost_core::{AccessControlConfig, AccessDecision, AccessMode, ClientAccessControl};

#[derive(Debug, Clone)]
pub struct TlsInterceptConfig {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
}

impl TlsInterceptConfig {
    pub fn from_proxy_config(config: &ProxyConfig) -> Self {
        Self {
            enable_tls_interception: config.enable_tls_interception,
            intercept_exclude: config.intercept_exclude.clone(),
            intercept_include: config.intercept_include.clone(),
            unsafe_ssl: config.unsafe_ssl,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub port: u16,
    pub host: String,
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub timeout_secs: u64,
    pub socks5_port: Option<u16>,
    pub socks5_auth_required: bool,
    pub socks5_username: Option<String>,
    pub socks5_password: Option<String>,
    pub verbose_logging: bool,
    pub access_mode: AccessMode,
    pub client_whitelist: Vec<String>,
    pub allow_lan: bool,
    pub unsafe_ssl: bool,
    pub max_body_buffer_size: usize,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            port: 9900,
            host: "127.0.0.1".to_string(),
            enable_tls_interception: true,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            timeout_secs: 30,
            socks5_port: None,
            socks5_auth_required: false,
            socks5_username: None,
            socks5_password: None,
            verbose_logging: false,
            access_mode: AccessMode::LocalOnly,
            client_whitelist: Vec::new(),
            allow_lan: false,
            unsafe_ssl: false,
            max_body_buffer_size: 32 * 1024 * 1024, // 32MB
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuleValue {
    pub pattern: String,
    pub protocol: Protocol,
    pub value: String,
    pub options: HashMap<String, String>,
    pub rule_name: Option<String>,
    pub raw: Option<String>,
    pub line: Option<usize>,
}

#[derive(Clone)]
pub struct RegexReplace {
    pub pattern: Regex,
    pub replacement: String,
    pub global: bool,
}

impl std::fmt::Debug for RegexReplace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegexReplace")
            .field("pattern", &self.pattern.as_str())
            .field("replacement", &self.replacement)
            .field("global", &self.global)
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedRules {
    pub host: Option<String>,
    pub host_protocol: Option<Protocol>,
    pub proxy: Option<String>,
    pub req_headers: Vec<(String, String)>,
    pub res_headers: Vec<(String, String)>,
    pub req_body: Option<Bytes>,
    pub res_body: Option<Bytes>,
    pub req_cookies: Vec<(String, String)>,
    pub res_cookies: Vec<(String, String)>,
    pub req_delay: Option<u64>,
    pub res_delay: Option<u64>,
    pub status_code: Option<u16>,
    pub method: Option<String>,
    pub ua: Option<String>,
    pub referer: Option<String>,
    pub enable_cors: bool,
    pub rules: Vec<RuleValue>,

    pub req_prepend: Option<Bytes>,
    pub req_append: Option<Bytes>,
    pub res_prepend: Option<Bytes>,
    pub res_append: Option<Bytes>,
    pub req_replace: Vec<(String, String)>,
    pub res_replace: Vec<(String, String)>,
    pub req_replace_regex: Vec<RegexReplace>,
    pub res_replace_regex: Vec<RegexReplace>,
    pub req_merge: Option<serde_json::Value>,
    pub res_merge: Option<serde_json::Value>,

    pub url_params: Vec<(String, String)>,
    pub url_replace: Vec<(String, String)>,

    pub forwarded_for: Option<String>,
    pub req_type: Option<String>,
    pub req_charset: Option<String>,

    pub res_type: Option<String>,
    pub res_charset: Option<String>,
    pub replace_status: Option<u16>,
    pub cache: Option<String>,
    pub attachment: Option<String>,

    pub ignored: bool,

    pub mock_file: Option<String>,
    pub mock_rawfile: Option<String>,
    pub mock_template: Option<String>,

    pub redirect: Option<String>,
    pub location_href: Option<String>,

    pub req_speed: Option<u64>,
    pub res_speed: Option<u64>,

    pub html_append: Option<String>,
    pub html_prepend: Option<String>,
    pub html_body: Option<String>,
    pub js_append: Option<String>,
    pub js_prepend: Option<String>,
    pub js_body: Option<String>,
    pub css_append: Option<String>,
    pub css_prepend: Option<String>,
    pub css_body: Option<String>,

    pub dns_servers: Vec<String>,

    pub tls_intercept: Option<bool>,
}

pub trait RulesResolver: Send + Sync {
    fn resolve(&self, url: &str, method: &str) -> ResolvedRules {
        self.resolve_with_context(
            url,
            method,
            &std::collections::HashMap::new(),
            &std::collections::HashMap::new(),
        )
    }

    fn resolve_with_context(
        &self,
        url: &str,
        method: &str,
        req_headers: &std::collections::HashMap<String, String>,
        req_cookies: &std::collections::HashMap<String, String>,
    ) -> ResolvedRules;
}

#[derive(Default)]
pub struct NoOpRulesResolver;

impl RulesResolver for NoOpRulesResolver {
    fn resolve_with_context(
        &self,
        _url: &str,
        _method: &str,
        _req_headers: &std::collections::HashMap<String, String>,
        _req_cookies: &std::collections::HashMap<String, String>,
    ) -> ResolvedRules {
        ResolvedRules::default()
    }
}

#[derive(Default)]
pub struct TlsConfig {
    pub ca_cert: Option<Vec<u8>>,
    pub ca_key: Option<Vec<u8>>,
    pub cert_generator: Option<Arc<bifrost_tls::DynamicCertGenerator>>,
    pub sni_resolver: Option<Arc<bifrost_tls::SniResolver>>,
}

pub struct ProxyServer {
    config: ProxyConfig,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    admin_state: Option<Arc<AdminState>>,
    admin_security_config: AdminSecurityConfig,
    access_control: Arc<RwLock<ClientAccessControl>>,
    dns_resolver: Arc<DnsResolver>,
}

impl ProxyServer {
    pub fn new(config: ProxyConfig) -> Self {
        let admin_security_config = AdminSecurityConfig::new(config.port);
        let access_config = AccessControlConfig {
            mode: config.access_mode,
            whitelist: config.client_whitelist.clone(),
            allow_lan: config.allow_lan,
        };
        let dns_resolver = Arc::new(DnsResolver::new(config.verbose_logging));
        Self {
            config,
            rules: Arc::new(NoOpRulesResolver),
            tls_config: Arc::new(TlsConfig::default()),
            admin_state: None,
            admin_security_config,
            access_control: Arc::new(RwLock::new(ClientAccessControl::new(access_config))),
            dns_resolver,
        }
    }

    pub fn with_rules(mut self, rules: Arc<dyn RulesResolver>) -> Self {
        self.rules = rules;
        self
    }

    pub fn with_tls_config(mut self, tls_config: Arc<TlsConfig>) -> Self {
        self.tls_config = tls_config;
        self
    }

    pub fn with_admin_state(mut self, admin_state: AdminState) -> Self {
        let admin_state = admin_state.with_access_control(Arc::clone(&self.access_control));
        self.admin_state = Some(Arc::new(admin_state));
        self
    }

    pub fn with_admin_state_shared(mut self, admin_state: Arc<AdminState>) -> Self {
        self.admin_state = Some(admin_state);
        self
    }

    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }

    pub fn admin_state(&self) -> Option<&Arc<AdminState>> {
        self.admin_state.as_ref()
    }

    pub fn access_control(&self) -> &Arc<RwLock<ClientAccessControl>> {
        &self.access_control
    }

    pub async fn bind(&self, addr: SocketAddr) -> Result<TcpListener> {
        TcpListener::bind(addr)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to bind to {}: {}", addr, e)))
    }

    pub async fn run(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| BifrostError::Config(format!("Invalid address: {}", e)))?;

        let listener = self.bind(addr).await?;
        info!("Proxy server listening on {}", addr);

        if let Some(socks5_port) = self.config.socks5_port {
            let socks_config = crate::socks::SocksConfig {
                port: socks5_port,
                host: self.config.host.clone(),
                auth_required: self.config.socks5_auth_required,
                username: self.config.socks5_username.clone(),
                password: self.config.socks5_password.clone(),
                timeout_secs: self.config.timeout_secs,
                access_mode: self.config.access_mode,
                client_whitelist: self.config.client_whitelist.clone(),
                allow_lan: self.config.allow_lan,
            };
            let socks_server = crate::socks::SocksServer::new(socks_config)
                .with_rules(Arc::clone(&self.rules))
                .with_access_control(Arc::clone(&self.access_control));

            let http_future = self.serve(listener);
            let socks_future = socks_server.run();

            tokio::select! {
                result = http_future => result,
                result = socks_future => result,
            }
        } else {
            self.serve(listener).await
        }
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (stream, peer_addr) = listener.accept().await.map_err(|e| {
                BifrostError::Network(format!("Failed to accept connection: {}", e))
            })?;

            debug!("Accepted connection from {}", peer_addr);

            let decision = {
                let access_control = self.access_control.read().await;
                access_control.check_access(&peer_addr.ip())
            };

            match decision {
                AccessDecision::Allow => {}
                AccessDecision::Deny => {
                    warn!(
                        "Access denied for client {} (not in whitelist)",
                        peer_addr.ip()
                    );
                    continue;
                }
                AccessDecision::Prompt(ip) => {
                    {
                        let access_control = self.access_control.read().await;
                        access_control.add_pending_authorization(ip);
                    }
                    warn!(
                        "Non-whitelisted client {} added to pending authorization. \
                        Approve via admin UI or use `bifrost whitelist add {}`",
                        ip, ip
                    );
                    continue;
                }
            }

            let rules = Arc::clone(&self.rules);
            let tls_config = Arc::clone(&self.tls_config);
            let proxy_config = self.config.clone();
            let admin_state = self.admin_state.clone();
            let admin_security_config = self.admin_security_config.clone();
            let dns_resolver = Arc::clone(&self.dns_resolver);

            tokio::spawn(async move {
                let io = TokioIo::new(stream);

                let service = service_fn(move |req: Request<Incoming>| {
                    let rules = Arc::clone(&rules);
                    let tls_config = Arc::clone(&tls_config);
                    let proxy_config = proxy_config.clone();
                    let admin_state = admin_state.clone();
                    let admin_security_config = admin_security_config.clone();
                    let dns_resolver = Arc::clone(&dns_resolver);
                    async move {
                        handle_request(
                            req,
                            peer_addr,
                            rules,
                            tls_config,
                            proxy_config,
                            admin_state,
                            admin_security_config,
                            dns_resolver,
                        )
                        .await
                    }
                });

                if let Err(err) = http1::Builder::new()
                    .preserve_header_case(true)
                    .title_case_headers(true)
                    .serve_connection(io, service)
                    .with_upgrades()
                    .await
                {
                    error!("Error serving connection from {}: {:?}", peer_addr, err);
                }
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_request(
    req: Request<Incoming>,
    peer_addr: SocketAddr,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    admin_security_config: AdminSecurityConfig,
    dns_resolver: Arc<DnsResolver>,
) -> std::result::Result<Response<BoxBody>, hyper::Error> {
    let ctx = RequestContext::new();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();
    let verbose_logging = proxy_config.verbose_logging;

    if verbose_logging {
        info!(
            "[{}] --> {} {} (from {})",
            ctx.id_str(),
            method,
            uri,
            peer_addr
        );
    } else {
        debug!("Received request: {} {} from {}", method, uri, peer_addr);
    }

    if path.starts_with(ADMIN_PATH_PREFIX) {
        if let Some(state) = admin_state {
            if path.starts_with(CERT_PUBLIC_PATH_PREFIX) && is_cert_public_request(&req) {
                debug!(
                    "Public cert request from {}: {} {}",
                    peer_addr, method, path
                );
                return Ok(convert_admin_response(
                    AdminRouter::handle(req, state).await,
                ));
            } else if is_valid_admin_request(&req, peer_addr, &admin_security_config) {
                debug!(
                    "Valid admin request from {}: {} {}",
                    peer_addr, method, path
                );
                return Ok(convert_admin_response(
                    AdminRouter::handle(req, state).await,
                ));
            } else {
                warn!(
                    "Rejected invalid admin request from {}: {} {} (possible forgery attempt)",
                    peer_addr, method, uri
                );
                return Ok(error_response(403, "Forbidden"));
            }
        } else {
            return Ok(error_response(503, "Admin interface not enabled"));
        }
    }

    if is_direct_browser_access(&req, &proxy_config) && admin_state.is_some() {
        debug!(
            "Redirecting direct browser access from {} to admin UI",
            peer_addr
        );
        return Ok(redirect_response(ADMIN_PATH_PREFIX));
    }

    if let Some(ref state) = admin_state {
        state.metrics_collector.increment_requests();
    }

    if method == Method::CONNECT {
        let tls_intercept_config = if let Some(ref state) = admin_state {
            let runtime_config = state.runtime_config.read().await;
            TlsInterceptConfig {
                enable_tls_interception: runtime_config.enable_tls_interception,
                intercept_exclude: runtime_config.intercept_exclude.clone(),
                intercept_include: runtime_config.intercept_include.clone(),
                unsafe_ssl: runtime_config.unsafe_ssl,
            }
        } else {
            TlsInterceptConfig::from_proxy_config(&proxy_config)
        };

        match handle_connect(
            req,
            rules,
            tls_config,
            &tls_intercept_config,
            &proxy_config,
            verbose_logging,
            &ctx,
            admin_state.clone(),
            Some(dns_resolver),
        )
        .await
        {
            Ok(response) => {
                if verbose_logging {
                    info!(
                        "[{}] <-- CONNECT established ({}ms)",
                        ctx.id_str(),
                        ctx.elapsed_ms()
                    );
                }
                Ok(response)
            }
            Err(e) => {
                error!("[{}] CONNECT error: {}", ctx.id_str(), e);
                Ok(error_response(502, "Bad Gateway"))
            }
        }
    } else {
        let unsafe_ssl = if let Some(ref state) = admin_state {
            state.runtime_config.read().await.unsafe_ssl
        } else {
            proxy_config.unsafe_ssl
        };

        match handle_http_request(
            req,
            rules,
            verbose_logging,
            unsafe_ssl,
            proxy_config.max_body_buffer_size,
            &ctx,
            admin_state.clone(),
            Some(dns_resolver),
        )
        .await
        {
            Ok(response) => {
                if verbose_logging {
                    info!(
                        "[{}] <-- {} ({}ms)",
                        ctx.id_str(),
                        response.status(),
                        ctx.elapsed_ms()
                    );
                }
                Ok(response)
            }
            Err(e) => {
                error!("[{}] HTTP proxy error: {}", ctx.id_str(), e);
                Ok(error_response(502, "Bad Gateway"))
            }
        }
    }
}

pub type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

pub fn empty_body() -> BoxBody {
    use http_body_util::{BodyExt, Empty};
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

pub fn full_body(data: impl Into<Bytes>) -> BoxBody {
    use http_body_util::{BodyExt, Full};
    Full::new(data.into())
        .map_err(|never| match never {})
        .boxed()
}

fn error_response(status: u16, message: &str) -> Response<BoxBody> {
    Response::builder()
        .status(status)
        .body(full_body(message.to_string()))
        .unwrap()
}

fn redirect_response(location: &str) -> Response<BoxBody> {
    Response::builder()
        .status(302)
        .header("Location", location)
        .body(empty_body())
        .unwrap()
}

fn is_direct_browser_access(req: &Request<Incoming>, config: &ProxyConfig) -> bool {
    let uri = req.uri();
    let path = uri.path();

    if path != "/" {
        return false;
    }

    if uri.scheme().is_some() || uri.host().is_some() {
        return false;
    }

    let headers = req.headers();
    let host = match headers.get("host").and_then(|h| h.to_str().ok()) {
        Some(h) => h,
        None => return false,
    };

    let host_without_port = host.split(':').next().unwrap_or(host);
    let is_local = host_without_port == "localhost"
        || host_without_port == "127.0.0.1"
        || host_without_port == config.host;

    if !is_local {
        return false;
    }

    let port = host
        .split(':')
        .nth(1)
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(80);

    if port != config.port {
        return false;
    }

    let accept = headers
        .get("accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    accept.contains("text/html")
}

fn convert_admin_response(
    response: Response<http_body_util::combinators::BoxBody<Bytes, hyper::Error>>,
) -> Response<BoxBody> {
    let (parts, body) = response.into_parts();
    Response::from_parts(parts, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_default() {
        let config = ProxyConfig::default();
        assert_eq!(config.port, 9900);
        assert_eq!(config.host, "127.0.0.1");
        assert!(config.enable_tls_interception);
        assert!(config.intercept_exclude.is_empty());
        assert!(config.intercept_include.is_empty());
        assert_eq!(config.timeout_secs, 30);
        assert!(config.socks5_port.is_none());
        assert!(!config.socks5_auth_required);
        assert!(config.socks5_username.is_none());
        assert!(config.socks5_password.is_none());
    }

    #[test]
    fn test_resolved_rules_default() {
        let rules = ResolvedRules::default();
        assert!(rules.host.is_none());
        assert!(rules.proxy.is_none());
        assert!(rules.req_headers.is_empty());
        assert!(rules.res_headers.is_empty());
        assert!(rules.req_body.is_none());
        assert!(rules.res_body.is_none());
        assert!(rules.status_code.is_none());
        assert!(!rules.enable_cors);
        assert!(rules.tls_intercept.is_none());
    }

    #[test]
    fn test_noop_rules_resolver() {
        let resolver = NoOpRulesResolver;
        let rules = resolver.resolve("http://example.com", "GET");
        assert!(rules.host.is_none());
        assert!(rules.rules.is_empty());
    }

    #[test]
    fn test_proxy_server_new() {
        let config = ProxyConfig::default();
        let server = ProxyServer::new(config.clone());
        assert_eq!(server.config().port, config.port);
        assert_eq!(server.config().host, config.host);
    }

    #[test]
    fn test_proxy_server_with_config() {
        let config = ProxyConfig {
            port: 9000,
            host: "0.0.0.0".to_string(),
            enable_tls_interception: true,
            intercept_exclude: vec!["*.example.com".to_string()],
            intercept_include: vec!["*.api.com".to_string()],
            timeout_secs: 60,
            socks5_port: Some(1080),
            socks5_auth_required: true,
            socks5_username: Some("user".to_string()),
            socks5_password: Some("pass".to_string()),
            verbose_logging: true,
            access_mode: AccessMode::Whitelist,
            client_whitelist: vec!["192.168.1.0/24".to_string()],
            allow_lan: true,
            unsafe_ssl: false,
            max_body_buffer_size: 32 * 1024 * 1024,
        };
        let server = ProxyServer::new(config);
        assert_eq!(server.config().port, 9000);
        assert_eq!(server.config().host, "0.0.0.0");
        assert!(server.config().enable_tls_interception);
        assert_eq!(server.config().socks5_port, Some(1080));
        assert!(server.config().socks5_auth_required);
        assert!(server.config().verbose_logging);
        assert_eq!(server.config().access_mode, AccessMode::Whitelist);
        assert!(server.config().allow_lan);
    }

    #[test]
    fn test_empty_body() {
        use hyper::body::Body;
        let body = empty_body();
        assert!(body.is_end_stream());
    }

    #[test]
    fn test_full_body() {
        use hyper::body::Body;
        let body = full_body("test content");
        assert!(!body.is_end_stream());
    }

    #[test]
    fn test_rule_value() {
        let rule = RuleValue {
            pattern: "*.example.com".to_string(),
            protocol: Protocol::Host,
            value: "example.com".to_string(),
            options: HashMap::new(),
            rule_name: None,
            raw: None,
            line: None,
        };
        assert_eq!(rule.protocol, Protocol::Host);
        assert_eq!(rule.value, "example.com");
    }

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert!(config.ca_cert.is_none());
        assert!(config.ca_key.is_none());
    }
}

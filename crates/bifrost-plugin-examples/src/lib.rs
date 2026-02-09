use async_trait::async_trait;
#[cfg(test)]
use bifrost_plugin::PluginContext;
use bifrost_plugin::{
    AuthContext, BifrostPlugin, DataContext, HttpContext, PluginHook, Result, RulesContext,
    StatsContext, TunnelContext,
};
use bytes::Bytes;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct LoggerPlugin {
    log_count: AtomicU64,
}

impl LoggerPlugin {
    pub fn new() -> Self {
        Self {
            log_count: AtomicU64::new(0),
        }
    }

    pub fn get_log_count(&self) -> u64 {
        self.log_count.load(Ordering::SeqCst)
    }
}

impl Default for LoggerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BifrostPlugin for LoggerPlugin {
    fn name(&self) -> &str {
        "bifrost.logger"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http, PluginHook::ReqStats, PluginHook::ResStats]
    }

    fn priority(&self) -> i32 {
        100
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
        let count = self.log_count.fetch_add(1, Ordering::SeqCst) + 1;
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        tracing::info!(
            "[bifrost.logger] #{} {} {} {} (session: {}, request: {})",
            count,
            timestamp,
            ctx.base.method,
            ctx.base.url,
            ctx.base.session_id,
            ctx.base.request_id
        );

        ctx.set_header("X-Bifrost-Log-Id", &count.to_string());
        ctx.set_header("X-Bifrost-Log-Time", &timestamp);
        Ok(())
    }

    async fn on_req_stats(&self, ctx: &mut StatsContext) -> Result<()> {
        tracing::debug!(
            "[bifrost.logger] Request stats: {} bytes, {}ms",
            ctx.bytes_transferred,
            ctx.duration_ms
        );
        Ok(())
    }

    async fn on_res_stats(&self, ctx: &mut StatsContext) -> Result<()> {
        tracing::debug!(
            "[bifrost.logger] Response stats: {} bytes, {}ms",
            ctx.bytes_transferred,
            ctx.duration_ms
        );
        Ok(())
    }
}

pub struct HeaderInjectorPlugin {
    headers: RwLock<HashMap<String, String>>,
}

impl HeaderInjectorPlugin {
    pub fn new() -> Self {
        Self {
            headers: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_header(&self, key: &str, value: &str) {
        self.headers
            .write()
            .insert(key.to_string(), value.to_string());
    }

    pub fn remove_header(&self, key: &str) {
        self.headers.write().remove(key);
    }

    pub fn with_default_headers() -> Self {
        let plugin = Self::new();
        plugin.add_header("X-Proxy", "Bifrost");
        plugin.add_header("X-Proxy-Version", "1.0.0");
        plugin
    }
}

impl Default for HeaderInjectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BifrostPlugin for HeaderInjectorPlugin {
    fn name(&self) -> &str {
        "bifrost.header-injector"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http]
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
        let headers = self.headers.read();
        for (key, value) in headers.iter() {
            ctx.set_header(key, value);
        }
        Ok(())
    }
}

pub struct AuthPlugin {
    users: RwLock<HashMap<String, String>>,
    allow_anonymous: bool,
}

impl AuthPlugin {
    pub fn new(allow_anonymous: bool) -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
            allow_anonymous,
        }
    }

    pub fn add_user(&self, username: &str, password: &str) {
        self.users
            .write()
            .insert(username.to_string(), password.to_string());
    }

    pub fn remove_user(&self, username: &str) {
        self.users.write().remove(username);
    }

    pub fn with_default_users() -> Self {
        let plugin = Self::new(false);
        plugin.add_user("admin", "admin123");
        plugin.add_user("user", "user123");
        plugin
    }
}

#[async_trait]
impl BifrostPlugin for AuthPlugin {
    fn name(&self) -> &str {
        "bifrost.auth"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Auth]
    }

    fn priority(&self) -> i32 {
        200
    }

    async fn on_auth(&self, ctx: &mut AuthContext) -> Result<()> {
        if ctx.username.is_none() || ctx.password.is_none() {
            if self.allow_anonymous {
                ctx.approve();
                tracing::debug!("[bifrost.auth] Anonymous access allowed");
            } else {
                ctx.deny();
                tracing::warn!("[bifrost.auth] Anonymous access denied");
            }
            return Ok(());
        }

        let username = ctx.username.clone().unwrap();
        let password = ctx.password.clone().unwrap();

        let users = self.users.read();
        if let Some(stored_password) = users.get(&username) {
            if stored_password == &password {
                ctx.approve();
                tracing::info!(
                    "[bifrost.auth] User '{}' authenticated successfully",
                    username
                );
            } else {
                ctx.deny();
                tracing::warn!("[bifrost.auth] Invalid password for user '{}'", username);
            }
        } else {
            ctx.deny();
            tracing::warn!("[bifrost.auth] User '{}' not found", username);
        }
        Ok(())
    }
}

pub struct RateLimitPlugin {
    max_requests_per_minute: u64,
    request_counts: RwLock<HashMap<String, (u64, i64)>>,
}

impl RateLimitPlugin {
    pub fn new(max_requests_per_minute: u64) -> Self {
        Self {
            max_requests_per_minute,
            request_counts: RwLock::new(HashMap::new()),
        }
    }

    fn check_rate_limit(&self, client_ip: &str) -> bool {
        let now = Utc::now().timestamp();
        let mut counts = self.request_counts.write();

        if let Some((count, last_reset)) = counts.get_mut(client_ip) {
            if now - *last_reset >= 60 {
                *count = 1;
                *last_reset = now;
                true
            } else if *count < self.max_requests_per_minute {
                *count += 1;
                true
            } else {
                false
            }
        } else {
            counts.insert(client_ip.to_string(), (1, now));
            true
        }
    }
}

#[async_trait]
impl BifrostPlugin for RateLimitPlugin {
    fn name(&self) -> &str {
        "bifrost.rate-limit"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http]
    }

    fn priority(&self) -> i32 {
        150
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
        let client_ip = ctx.base.client_ip.clone();

        if self.check_rate_limit(&client_ip) {
            ctx.set_header(
                "X-RateLimit-Limit",
                &self.max_requests_per_minute.to_string(),
            );
            ctx.set_header("X-RateLimit-Status", "allowed");
            tracing::debug!(
                "[bifrost.rate-limit] Request allowed for client {}",
                client_ip
            );
        } else {
            ctx.set_header(
                "X-RateLimit-Limit",
                &self.max_requests_per_minute.to_string(),
            );
            ctx.set_header("X-RateLimit-Status", "exceeded");
            tracing::warn!(
                "[bifrost.rate-limit] Rate limit exceeded for client {}",
                client_ip
            );
        }
        Ok(())
    }
}

pub struct MockResponsePlugin {
    mocks: RwLock<HashMap<String, MockResponse>>,
}

#[derive(Clone)]
pub struct MockResponse {
    pub status_code: u16,
    pub body: Bytes,
    pub content_type: String,
}

impl MockResponsePlugin {
    pub fn new() -> Self {
        Self {
            mocks: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_mock(&self, url_pattern: &str, response: MockResponse) {
        self.mocks.write().insert(url_pattern.to_string(), response);
    }

    pub fn remove_mock(&self, url_pattern: &str) {
        self.mocks.write().remove(url_pattern);
    }

    pub fn with_default_mocks() -> Self {
        let plugin = Self::new();
        plugin.add_mock(
            "/api/health",
            MockResponse {
                status_code: 200,
                body: Bytes::from(r#"{"status":"ok"}"#),
                content_type: "application/json".to_string(),
            },
        );
        plugin.add_mock(
            "/api/version",
            MockResponse {
                status_code: 200,
                body: Bytes::from(r#"{"version":"1.0.0","name":"bifrost"}"#),
                content_type: "application/json".to_string(),
            },
        );
        plugin
    }

    fn match_url(&self, url: &str) -> Option<MockResponse> {
        let mocks = self.mocks.read();
        for (pattern, response) in mocks.iter() {
            if url.contains(pattern) {
                return Some(response.clone());
            }
        }
        None
    }
}

impl Default for MockResponsePlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BifrostPlugin for MockResponsePlugin {
    fn name(&self) -> &str {
        "bifrost.mock"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http, PluginHook::ReqRules]
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
        if let Some(mock) = self.match_url(&ctx.base.url) {
            ctx.set_header("X-Mock-Response", "true");
            ctx.set_header("Content-Type", &mock.content_type);
            ctx.set_body(mock.body);
            ctx.base.status_code = Some(mock.status_code);
            tracing::info!(
                "[bifrost.mock] Returning mock response for {}",
                ctx.base.url
            );
        }
        Ok(())
    }

    async fn on_req_rules(&self, ctx: &mut RulesContext) -> Result<()> {
        if self.match_url(&ctx.base.url).is_some() {
            ctx.add_rule(&format!("statusCode://{}", 200));
            tracing::debug!("[bifrost.mock] Added mock rule for {}", ctx.base.url);
        }
        Ok(())
    }
}

pub struct DataTransformPlugin {
    transform_fn: Box<dyn Fn(&Bytes) -> Bytes + Send + Sync>,
}

impl DataTransformPlugin {
    pub fn new<F>(transform_fn: F) -> Self
    where
        F: Fn(&Bytes) -> Bytes + Send + Sync + 'static,
    {
        Self {
            transform_fn: Box::new(transform_fn),
        }
    }

    pub fn uppercase() -> Self {
        Self::new(|data| {
            let s = String::from_utf8_lossy(data);
            Bytes::from(s.to_uppercase())
        })
    }
}

#[async_trait]
impl BifrostPlugin for DataTransformPlugin {
    fn name(&self) -> &str {
        "bifrost.data-transform"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::ReqRead, PluginHook::ResRead]
    }

    async fn on_req_read(&self, ctx: &mut DataContext) -> Result<()> {
        let transformed = (self.transform_fn)(&ctx.data);
        ctx.modify(transformed);
        Ok(())
    }

    async fn on_res_read(&self, ctx: &mut DataContext) -> Result<()> {
        let transformed = (self.transform_fn)(&ctx.data);
        ctx.modify(transformed);
        Ok(())
    }
}

pub struct TunnelInspectorPlugin {
    capture_domains: RwLock<Vec<String>>,
}

impl TunnelInspectorPlugin {
    pub fn new() -> Self {
        Self {
            capture_domains: RwLock::new(Vec::new()),
        }
    }

    pub fn add_capture_domain(&self, domain: &str) {
        self.capture_domains.write().push(domain.to_string());
    }

    pub fn should_capture(&self, host: &str) -> bool {
        let domains = self.capture_domains.read();
        domains.iter().any(|d| host.contains(d))
    }
}

impl Default for TunnelInspectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BifrostPlugin for TunnelInspectorPlugin {
    fn name(&self) -> &str {
        "bifrost.tunnel-inspector"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Tunnel, PluginHook::TunnelRules]
    }

    async fn on_tunnel(&self, ctx: &mut TunnelContext) -> Result<()> {
        if self.should_capture(&ctx.base.host) {
            ctx.enable_capture();
            tracing::info!(
                "[bifrost.tunnel-inspector] Capturing tunnel for {}",
                ctx.base.host
            );
        }
        Ok(())
    }

    async fn on_tunnel_rules(&self, ctx: &mut RulesContext) -> Result<()> {
        if self.should_capture(&ctx.base.host) {
            ctx.add_rule("capture://");
            tracing::debug!(
                "[bifrost.tunnel-inspector] Added capture rule for {}",
                ctx.base.host
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logger_plugin() {
        let plugin = LoggerPlugin::new();
        assert_eq!(plugin.name(), "bifrost.logger");
        assert_eq!(plugin.version(), "1.0.0");
        assert!(plugin.hooks().contains(&PluginHook::Http));

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = HttpContext::new(base);
        ctx.base.method = "GET".to_string();
        ctx.base.url = "http://example.com/test".to_string();

        plugin.on_http(&mut ctx).await.unwrap();

        assert!(ctx.modified);
        assert!(ctx.base.headers.contains_key("X-Bifrost-Log-Id"));
        assert!(ctx.base.headers.contains_key("X-Bifrost-Log-Time"));
        assert_eq!(plugin.get_log_count(), 1);
    }

    #[tokio::test]
    async fn test_header_injector_plugin() {
        let plugin = HeaderInjectorPlugin::with_default_headers();
        assert_eq!(plugin.name(), "bifrost.header-injector");

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = HttpContext::new(base);

        plugin.on_http(&mut ctx).await.unwrap();

        assert_eq!(
            ctx.base.headers.get("X-Proxy"),
            Some(&"Bifrost".to_string())
        );
        assert_eq!(
            ctx.base.headers.get("X-Proxy-Version"),
            Some(&"1.0.0".to_string())
        );
    }

    #[tokio::test]
    async fn test_auth_plugin_success() {
        let plugin = AuthPlugin::with_default_users();
        assert_eq!(plugin.name(), "bifrost.auth");

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = AuthContext::new(base);
        ctx.username = Some("admin".to_string());
        ctx.password = Some("admin123".to_string());

        plugin.on_auth(&mut ctx).await.unwrap();

        assert!(ctx.authenticated);
    }

    #[tokio::test]
    async fn test_auth_plugin_failure() {
        let plugin = AuthPlugin::with_default_users();

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = AuthContext::new(base);
        ctx.username = Some("admin".to_string());
        ctx.password = Some("wrongpassword".to_string());

        plugin.on_auth(&mut ctx).await.unwrap();

        assert!(!ctx.authenticated);
    }

    #[tokio::test]
    async fn test_auth_plugin_anonymous() {
        let plugin = AuthPlugin::new(true);

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = AuthContext::new(base);

        plugin.on_auth(&mut ctx).await.unwrap();

        assert!(ctx.authenticated);
    }

    #[tokio::test]
    async fn test_rate_limit_plugin() {
        let plugin = RateLimitPlugin::new(60);
        assert_eq!(plugin.name(), "bifrost.rate-limit");

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = HttpContext::new(base);
        ctx.base.client_ip = "192.168.1.100".to_string();

        plugin.on_http(&mut ctx).await.unwrap();

        assert_eq!(
            ctx.base.headers.get("X-RateLimit-Status"),
            Some(&"allowed".to_string())
        );
    }

    #[tokio::test]
    async fn test_mock_response_plugin() {
        let plugin = MockResponsePlugin::with_default_mocks();
        assert_eq!(plugin.name(), "bifrost.mock");

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = HttpContext::new(base);
        ctx.base.url = "http://example.com/api/health".to_string();

        plugin.on_http(&mut ctx).await.unwrap();

        assert!(ctx.modified);
        assert_eq!(
            ctx.base.headers.get("X-Mock-Response"),
            Some(&"true".to_string())
        );
        assert!(ctx.body.is_some());
    }

    #[tokio::test]
    async fn test_tunnel_inspector_plugin() {
        let plugin = TunnelInspectorPlugin::new();
        plugin.add_capture_domain("example.com");
        assert_eq!(plugin.name(), "bifrost.tunnel-inspector");

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = TunnelContext::new(base);
        ctx.base.host = "api.example.com".to_string();

        plugin.on_tunnel(&mut ctx).await.unwrap();

        assert!(ctx.capture);
    }

    #[test]
    fn test_plugin_names_conform_to_convention() {
        let logger = LoggerPlugin::new();
        let header = HeaderInjectorPlugin::new();
        let auth = AuthPlugin::new(false);
        let rate_limit = RateLimitPlugin::new(60);
        let mock = MockResponsePlugin::new();
        let tunnel = TunnelInspectorPlugin::new();

        assert!(logger.name().starts_with("bifrost."));
        assert!(header.name().starts_with("bifrost."));
        assert!(auth.name().starts_with("bifrost."));
        assert!(rate_limit.name().starts_with("bifrost."));
        assert!(mock.name().starts_with("bifrost."));
        assert!(tunnel.name().starts_with("bifrost."));
    }
}

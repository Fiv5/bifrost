use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const X_WHISTLE_SESSION_ID: &str = "x-bifrost-session-id";
pub const X_WHISTLE_REQUEST_ID: &str = "x-bifrost-request-id";
pub const X_WHISTLE_CLIENT_IP: &str = "x-bifrost-client-ip";
pub const X_WHISTLE_CLIENT_PORT: &str = "x-bifrost-client-port";
pub const X_WHISTLE_HOST: &str = "x-bifrost-host";
pub const X_WHISTLE_URL: &str = "x-bifrost-url";
pub const X_WHISTLE_METHOD: &str = "x-bifrost-method";
pub const X_WHISTLE_PROTOCOL: &str = "x-bifrost-protocol";
pub const X_WHISTLE_POLICY: &str = "x-bifrost-policy";
pub const X_WHISTLE_RULE: &str = "x-bifrost-rule";
pub const X_WHISTLE_RULES: &str = "x-bifrost-rules";
pub const X_WHISTLE_STATUS_CODE: &str = "x-bifrost-status-code";
pub const X_WHISTLE_HOOK: &str = "x-bifrost-hook";
pub const X_WHISTLE_PLUGIN: &str = "x-bifrost-plugin";
pub const X_WHISTLE_SNI: &str = "x-bifrost-sni";
pub const X_WHISTLE_REAL_HOST: &str = "x-bifrost-real-host";
pub const X_WHISTLE_REAL_PORT: &str = "x-bifrost-real-port";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginContext {
    pub session_id: String,
    pub request_id: String,
    pub client_ip: String,
    pub client_port: u16,
    pub host: String,
    pub url: String,
    pub method: String,
    pub protocol: String,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(default)]
    pub custom_data: HashMap<String, String>,
}

impl PluginContext {
    pub fn new(session_id: String, request_id: String) -> Self {
        Self {
            session_id,
            request_id,
            client_ip: String::new(),
            client_port: 0,
            host: String::new(),
            url: String::new(),
            method: String::new(),
            protocol: String::new(),
            headers: HashMap::new(),
            rule: None,
            rules: None,
            status_code: None,
            custom_data: HashMap::new(),
        }
    }

    pub fn to_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert(X_WHISTLE_SESSION_ID.to_string(), self.session_id.clone());
        headers.insert(X_WHISTLE_REQUEST_ID.to_string(), self.request_id.clone());
        headers.insert(X_WHISTLE_CLIENT_IP.to_string(), self.client_ip.clone());
        headers.insert(
            X_WHISTLE_CLIENT_PORT.to_string(),
            self.client_port.to_string(),
        );
        headers.insert(X_WHISTLE_HOST.to_string(), self.host.clone());
        headers.insert(X_WHISTLE_URL.to_string(), self.url.clone());
        headers.insert(X_WHISTLE_METHOD.to_string(), self.method.clone());
        headers.insert(X_WHISTLE_PROTOCOL.to_string(), self.protocol.clone());

        if let Some(ref rule) = self.rule {
            headers.insert(X_WHISTLE_RULE.to_string(), rule.clone());
        }
        if let Some(ref rules) = self.rules {
            headers.insert(X_WHISTLE_RULES.to_string(), rules.join(","));
        }
        if let Some(status_code) = self.status_code {
            headers.insert(X_WHISTLE_STATUS_CODE.to_string(), status_code.to_string());
        }

        headers
    }

    pub fn from_headers(headers: &HashMap<String, String>) -> Self {
        let session_id = headers
            .get(X_WHISTLE_SESSION_ID)
            .cloned()
            .unwrap_or_default();
        let request_id = headers
            .get(X_WHISTLE_REQUEST_ID)
            .cloned()
            .unwrap_or_default();
        let client_ip = headers.get(X_WHISTLE_CLIENT_IP).cloned().unwrap_or_default();
        let client_port = headers
            .get(X_WHISTLE_CLIENT_PORT)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let host = headers.get(X_WHISTLE_HOST).cloned().unwrap_or_default();
        let url = headers.get(X_WHISTLE_URL).cloned().unwrap_or_default();
        let method = headers.get(X_WHISTLE_METHOD).cloned().unwrap_or_default();
        let protocol = headers.get(X_WHISTLE_PROTOCOL).cloned().unwrap_or_default();
        let rule = headers.get(X_WHISTLE_RULE).cloned();
        let rules = headers.get(X_WHISTLE_RULES).map(|s| {
            s.split(',')
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty())
                .collect()
        });
        let status_code = headers
            .get(X_WHISTLE_STATUS_CODE)
            .and_then(|s| s.parse().ok());

        Self {
            session_id,
            request_id,
            client_ip,
            client_port,
            host,
            url,
            method,
            protocol,
            headers: headers.clone(),
            rule,
            rules,
            status_code,
            custom_data: HashMap::new(),
        }
    }

    pub fn set_custom(&mut self, key: &str, value: &str) {
        self.custom_data.insert(key.to_string(), value.to_string());
    }

    pub fn get_custom(&self, key: &str) -> Option<&String> {
        self.custom_data.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    #[serde(flatten)]
    pub base: PluginContext,
    pub username: Option<String>,
    pub password: Option<String>,
    pub authenticated: bool,
}

impl AuthContext {
    pub fn new(base: PluginContext) -> Self {
        Self {
            base,
            username: None,
            password: None,
            authenticated: false,
        }
    }

    pub fn set_credentials(&mut self, username: &str, password: &str) {
        self.username = Some(username.to_string());
        self.password = Some(password.to_string());
    }

    pub fn approve(&mut self) {
        self.authenticated = true;
    }

    pub fn deny(&mut self) {
        self.authenticated = false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpContext {
    #[serde(flatten)]
    pub base: PluginContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Bytes>,
    pub modified: bool,
}

impl HttpContext {
    pub fn new(base: PluginContext) -> Self {
        Self {
            base,
            body: None,
            modified: false,
        }
    }

    pub fn set_body(&mut self, body: Bytes) {
        self.body = Some(body);
        self.modified = true;
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        self.base.headers.insert(key.to_string(), value.to_string());
        self.modified = true;
    }

    pub fn remove_header(&mut self, key: &str) {
        self.base.headers.remove(key);
        self.modified = true;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelContext {
    #[serde(flatten)]
    pub base: PluginContext,
    pub sni: Option<String>,
    pub real_host: Option<String>,
    pub real_port: Option<u16>,
    pub capture: bool,
}

impl TunnelContext {
    pub fn new(base: PluginContext) -> Self {
        Self {
            base,
            sni: None,
            real_host: None,
            real_port: None,
            capture: false,
        }
    }

    pub fn set_target(&mut self, host: &str, port: u16) {
        self.real_host = Some(host.to_string());
        self.real_port = Some(port);
    }

    pub fn enable_capture(&mut self) {
        self.capture = true;
    }

    pub fn disable_capture(&mut self) {
        self.capture = false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesContext {
    #[serde(flatten)]
    pub base: PluginContext,
    pub matched_rules: Vec<String>,
    pub additional_rules: Vec<String>,
}

impl RulesContext {
    pub fn new(base: PluginContext) -> Self {
        Self {
            base,
            matched_rules: Vec::new(),
            additional_rules: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: &str) {
        self.additional_rules.push(rule.to_string());
    }

    pub fn clear_rules(&mut self) {
        self.matched_rules.clear();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataContext {
    #[serde(flatten)]
    pub base: PluginContext,
    pub data: Bytes,
    pub modified_data: Option<Bytes>,
}

impl DataContext {
    pub fn new(base: PluginContext, data: Bytes) -> Self {
        Self {
            base,
            data,
            modified_data: None,
        }
    }

    pub fn modify(&mut self, new_data: Bytes) {
        self.modified_data = Some(new_data);
    }

    pub fn get_data(&self) -> &Bytes {
        self.modified_data.as_ref().unwrap_or(&self.data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsContext {
    #[serde(flatten)]
    pub base: PluginContext,
    pub bytes_transferred: u64,
    pub duration_ms: u64,
    pub start_time: u64,
    pub end_time: u64,
}

impl StatsContext {
    pub fn new(base: PluginContext) -> Self {
        Self {
            base,
            bytes_transferred: 0,
            duration_ms: 0,
            start_time: 0,
            end_time: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_context_new() {
        let ctx = PluginContext::new("session-1".to_string(), "req-1".to_string());
        assert_eq!(ctx.session_id, "session-1");
        assert_eq!(ctx.request_id, "req-1");
    }

    #[test]
    fn test_plugin_context_to_headers() {
        let mut ctx = PluginContext::new("s1".to_string(), "r1".to_string());
        ctx.client_ip = "127.0.0.1".to_string();
        ctx.client_port = 8080;
        ctx.host = "example.com".to_string();
        ctx.url = "http://example.com/test".to_string();
        ctx.method = "GET".to_string();
        ctx.protocol = "HTTP/1.1".to_string();

        let headers = ctx.to_headers();
        assert_eq!(headers.get(X_WHISTLE_SESSION_ID).unwrap(), "s1");
        assert_eq!(headers.get(X_WHISTLE_REQUEST_ID).unwrap(), "r1");
        assert_eq!(headers.get(X_WHISTLE_CLIENT_IP).unwrap(), "127.0.0.1");
        assert_eq!(headers.get(X_WHISTLE_CLIENT_PORT).unwrap(), "8080");
    }

    #[test]
    fn test_plugin_context_from_headers() {
        let mut headers = HashMap::new();
        headers.insert(X_WHISTLE_SESSION_ID.to_string(), "s2".to_string());
        headers.insert(X_WHISTLE_REQUEST_ID.to_string(), "r2".to_string());
        headers.insert(X_WHISTLE_CLIENT_IP.to_string(), "192.168.1.1".to_string());
        headers.insert(X_WHISTLE_CLIENT_PORT.to_string(), "9090".to_string());

        let ctx = PluginContext::from_headers(&headers);
        assert_eq!(ctx.session_id, "s2");
        assert_eq!(ctx.request_id, "r2");
        assert_eq!(ctx.client_ip, "192.168.1.1");
        assert_eq!(ctx.client_port, 9090);
    }

    #[test]
    fn test_plugin_context_custom_data() {
        let mut ctx = PluginContext::new("s".to_string(), "r".to_string());
        ctx.set_custom("key1", "value1");
        assert_eq!(ctx.get_custom("key1"), Some(&"value1".to_string()));
        assert_eq!(ctx.get_custom("key2"), None);
    }

    #[test]
    fn test_auth_context() {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut auth = AuthContext::new(base);
        assert!(!auth.authenticated);

        auth.set_credentials("user", "pass");
        assert_eq!(auth.username, Some("user".to_string()));
        assert_eq!(auth.password, Some("pass".to_string()));

        auth.approve();
        assert!(auth.authenticated);

        auth.deny();
        assert!(!auth.authenticated);
    }

    #[test]
    fn test_http_context() {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut http = HttpContext::new(base);
        assert!(!http.modified);

        http.set_header("Content-Type", "application/json");
        assert!(http.modified);
        assert_eq!(
            http.base.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );

        http.set_body(Bytes::from("test body"));
        assert_eq!(http.body, Some(Bytes::from("test body")));
    }

    #[test]
    fn test_tunnel_context() {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut tunnel = TunnelContext::new(base);
        assert!(!tunnel.capture);

        tunnel.set_target("target.com", 443);
        assert_eq!(tunnel.real_host, Some("target.com".to_string()));
        assert_eq!(tunnel.real_port, Some(443));

        tunnel.enable_capture();
        assert!(tunnel.capture);
    }

    #[test]
    fn test_rules_context() {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut rules = RulesContext::new(base);

        rules.add_rule("rule1");
        rules.add_rule("rule2");
        assert_eq!(rules.additional_rules.len(), 2);

        rules.clear_rules();
        assert!(rules.matched_rules.is_empty());
    }

    #[test]
    fn test_data_context() {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let data = Bytes::from("original");
        let mut ctx = DataContext::new(base, data.clone());

        assert_eq!(ctx.get_data(), &data);

        ctx.modify(Bytes::from("modified"));
        assert_eq!(ctx.get_data(), &Bytes::from("modified"));
    }

    #[test]
    fn test_stats_context() {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let stats = StatsContext::new(base);
        assert_eq!(stats.bytes_transferred, 0);
        assert_eq!(stats.duration_ms, 0);
    }

    #[test]
    fn test_context_serialization() {
        let mut ctx = PluginContext::new("s".to_string(), "r".to_string());
        ctx.host = "example.com".to_string();
        ctx.method = "GET".to_string();

        let json = serde_json::to_string(&ctx).unwrap();
        assert!(json.contains("session_id"));
        assert!(json.contains("example.com"));

        let deserialized: PluginContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.session_id, "s");
        assert_eq!(deserialized.host, "example.com");
    }
}

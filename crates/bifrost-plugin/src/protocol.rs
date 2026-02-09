use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::context::{PluginContext, X_BIFROST_HOOK, X_BIFROST_PLUGIN, X_BIFROST_POLICY};
use crate::hook::PluginHook;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelPolicy {
    #[default]
    Tunnel,
    Capture,
    Connect,
}

impl TunnelPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            TunnelPolicy::Tunnel => "tunnel",
            TunnelPolicy::Capture => "capture",
            TunnelPolicy::Connect => "connect",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "tunnel" => Some(TunnelPolicy::Tunnel),
            "capture" => Some(TunnelPolicy::Capture),
            "connect" => Some(TunnelPolicy::Connect),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    pub plugin_name: String,
    pub hook: PluginHook,
    pub method: String,
    pub uri: String,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Bytes>,
}

impl PluginRequest {
    pub fn new(plugin_name: &str, hook: PluginHook, context: &PluginContext) -> Self {
        let mut headers = context.to_headers();
        headers.insert(X_BIFROST_PLUGIN.to_string(), plugin_name.to_string());
        headers.insert(X_BIFROST_HOOK.to_string(), hook.to_string());

        Self {
            plugin_name: plugin_name.to_string(),
            hook,
            method: context.method.clone(),
            uri: context.url.clone(),
            headers,
            body: None,
        }
    }

    pub fn with_body(mut self, body: Bytes) -> Self {
        self.body = Some(body);
        self
    }

    pub fn set_policy(&mut self, policy: TunnelPolicy) {
        self.headers
            .insert(X_BIFROST_POLICY.to_string(), policy.as_str().to_string());
    }

    pub fn get_policy(&self) -> TunnelPolicy {
        self.headers
            .get(X_BIFROST_POLICY)
            .and_then(|s| TunnelPolicy::parse(s))
            .unwrap_or_default()
    }

    pub fn to_http_path(&self) -> String {
        format!("/plugin/{}/{}", self.plugin_name, self.hook.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Bytes>,
    pub modified: bool,
}

impl PluginResponse {
    pub fn ok() -> Self {
        Self {
            status_code: 200,
            headers: HashMap::new(),
            body: None,
            modified: false,
        }
    }

    pub fn with_status(status_code: u16) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: None,
            modified: false,
        }
    }

    pub fn with_body(mut self, body: Bytes) -> Self {
        self.body = Some(body);
        self.modified = true;
        self
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }

    pub fn get_policy(&self) -> Option<TunnelPolicy> {
        self.headers
            .get(X_BIFROST_POLICY)
            .and_then(|s| TunnelPolicy::parse(s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub host: String,
    pub port: u16,
    pub policy: TunnelPolicy,
    pub context: PluginContext,
}

impl ConnectRequest {
    pub fn new(host: &str, port: u16, context: PluginContext) -> Self {
        Self {
            host: host.to_string(),
            port,
            policy: TunnelPolicy::Tunnel,
            context,
        }
    }

    pub fn with_policy(mut self, policy: TunnelPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn target_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMessage {
    pub session_id: String,
    pub request_id: String,
    pub direction: DataDirection,
    pub data: Bytes,
    pub is_final: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataDirection {
    Request,
    Response,
}

impl DataMessage {
    pub fn request(session_id: &str, request_id: &str, data: Bytes) -> Self {
        Self {
            session_id: session_id.to_string(),
            request_id: request_id.to_string(),
            direction: DataDirection::Request,
            data,
            is_final: false,
        }
    }

    pub fn response(session_id: &str, request_id: &str, data: Bytes) -> Self {
        Self {
            session_id: session_id.to_string(),
            request_id: request_id.to_string(),
            direction: DataDirection::Response,
            data,
            is_final: false,
        }
    }

    pub fn finalize(mut self) -> Self {
        self.is_final = true;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub hooks: Vec<PluginHook>,
    pub port: u16,
    pub protocol: PluginProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginProtocol {
    #[default]
    Http,
    WebSocket,
}

impl PluginInfo {
    pub fn new(name: &str, version: &str, hooks: Vec<PluginHook>, port: u16) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            hooks,
            port,
            protocol: PluginProtocol::Http,
        }
    }

    pub fn supports_hook(&self, hook: PluginHook) -> bool {
        self.hooks.contains(&hook)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_policy() {
        assert_eq!(TunnelPolicy::Tunnel.as_str(), "tunnel");
        assert_eq!(TunnelPolicy::Capture.as_str(), "capture");
        assert_eq!(TunnelPolicy::Connect.as_str(), "connect");
    }

    #[test]
    fn test_tunnel_policy_from_str() {
        assert_eq!(TunnelPolicy::parse("tunnel"), Some(TunnelPolicy::Tunnel));
        assert_eq!(TunnelPolicy::parse("CAPTURE"), Some(TunnelPolicy::Capture));
        assert_eq!(TunnelPolicy::parse("invalid"), None);
    }

    #[test]
    fn test_plugin_request() {
        let ctx = PluginContext::new("s1".to_string(), "r1".to_string());
        let req = PluginRequest::new("test-plugin", PluginHook::Http, &ctx);

        assert_eq!(req.plugin_name, "test-plugin");
        assert_eq!(req.hook, PluginHook::Http);
        assert_eq!(req.to_http_path(), "/plugin/test-plugin/http");
    }

    #[test]
    fn test_plugin_request_with_body() {
        let ctx = PluginContext::new("s1".to_string(), "r1".to_string());
        let req = PluginRequest::new("test-plugin", PluginHook::Http, &ctx)
            .with_body(Bytes::from("test"));

        assert_eq!(req.body, Some(Bytes::from("test")));
    }

    #[test]
    fn test_plugin_request_policy() {
        let ctx = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut req = PluginRequest::new("test-plugin", PluginHook::Tunnel, &ctx);

        req.set_policy(TunnelPolicy::Capture);
        assert_eq!(req.get_policy(), TunnelPolicy::Capture);
    }

    #[test]
    fn test_plugin_response() {
        let resp = PluginResponse::ok();
        assert!(resp.is_success());
        assert!(!resp.modified);
    }

    #[test]
    fn test_plugin_response_with_status() {
        let resp = PluginResponse::with_status(404);
        assert!(!resp.is_success());
        assert_eq!(resp.status_code, 404);
    }

    #[test]
    fn test_plugin_response_with_body() {
        let resp = PluginResponse::ok().with_body(Bytes::from("response body"));
        assert!(resp.modified);
        assert_eq!(resp.body, Some(Bytes::from("response body")));
    }

    #[test]
    fn test_connect_request() {
        let ctx = PluginContext::new("s1".to_string(), "r1".to_string());
        let req = ConnectRequest::new("example.com", 443, ctx);

        assert_eq!(req.host, "example.com");
        assert_eq!(req.port, 443);
        assert_eq!(req.target_address(), "example.com:443");
    }

    #[test]
    fn test_connect_request_with_policy() {
        let ctx = PluginContext::new("s1".to_string(), "r1".to_string());
        let req = ConnectRequest::new("example.com", 443, ctx).with_policy(TunnelPolicy::Capture);

        assert_eq!(req.policy, TunnelPolicy::Capture);
    }

    #[test]
    fn test_data_message_request() {
        let msg = DataMessage::request("s1", "r1", Bytes::from("data"));
        assert_eq!(msg.direction, DataDirection::Request);
        assert!(!msg.is_final);
    }

    #[test]
    fn test_data_message_response() {
        let msg = DataMessage::response("s1", "r1", Bytes::from("data")).finalize();
        assert_eq!(msg.direction, DataDirection::Response);
        assert!(msg.is_final);
    }

    #[test]
    fn test_plugin_info() {
        let info = PluginInfo::new(
            "my-plugin",
            "1.0.0",
            vec![PluginHook::Http, PluginHook::Auth],
            8080,
        );

        assert_eq!(info.name, "my-plugin");
        assert!(info.supports_hook(PluginHook::Http));
        assert!(info.supports_hook(PluginHook::Auth));
        assert!(!info.supports_hook(PluginHook::Tunnel));
    }

    #[test]
    fn test_protocol_serialization() {
        let policy = TunnelPolicy::Capture;
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(json, "\"capture\"");

        let deserialized: TunnelPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TunnelPolicy::Capture);
    }
}

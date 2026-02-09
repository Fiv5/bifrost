use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginHook {
    Auth,
    Sni,
    Ui,
    Http,
    Tunnel,
    ReqRules,
    ResRules,
    TunnelRules,
    ReqRead,
    ReqWrite,
    ResRead,
    ResWrite,
    WsReqRead,
    WsReqWrite,
    WsResRead,
    WsResWrite,
    TunnelReqRead,
    TunnelReqWrite,
    TunnelResRead,
    TunnelResWrite,
    ReqStats,
    ResStats,
}

impl PluginHook {
    pub const ALL: [PluginHook; 22] = [
        PluginHook::Auth,
        PluginHook::Sni,
        PluginHook::Ui,
        PluginHook::Http,
        PluginHook::Tunnel,
        PluginHook::ReqRules,
        PluginHook::ResRules,
        PluginHook::TunnelRules,
        PluginHook::ReqRead,
        PluginHook::ReqWrite,
        PluginHook::ResRead,
        PluginHook::ResWrite,
        PluginHook::WsReqRead,
        PluginHook::WsReqWrite,
        PluginHook::WsResRead,
        PluginHook::WsResWrite,
        PluginHook::TunnelReqRead,
        PluginHook::TunnelReqWrite,
        PluginHook::TunnelResRead,
        PluginHook::TunnelResWrite,
        PluginHook::ReqStats,
        PluginHook::ResStats,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            PluginHook::Auth => "auth",
            PluginHook::Sni => "sni",
            PluginHook::Ui => "ui",
            PluginHook::Http => "http",
            PluginHook::Tunnel => "tunnel",
            PluginHook::ReqRules => "req_rules",
            PluginHook::ResRules => "res_rules",
            PluginHook::TunnelRules => "tunnel_rules",
            PluginHook::ReqRead => "req_read",
            PluginHook::ReqWrite => "req_write",
            PluginHook::ResRead => "res_read",
            PluginHook::ResWrite => "res_write",
            PluginHook::WsReqRead => "ws_req_read",
            PluginHook::WsReqWrite => "ws_req_write",
            PluginHook::WsResRead => "ws_res_read",
            PluginHook::WsResWrite => "ws_res_write",
            PluginHook::TunnelReqRead => "tunnel_req_read",
            PluginHook::TunnelReqWrite => "tunnel_req_write",
            PluginHook::TunnelResRead => "tunnel_res_read",
            PluginHook::TunnelResWrite => "tunnel_res_write",
            PluginHook::ReqStats => "req_stats",
            PluginHook::ResStats => "res_stats",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "auth" => Some(PluginHook::Auth),
            "sni" => Some(PluginHook::Sni),
            "ui" => Some(PluginHook::Ui),
            "http" => Some(PluginHook::Http),
            "tunnel" => Some(PluginHook::Tunnel),
            "req_rules" => Some(PluginHook::ReqRules),
            "res_rules" => Some(PluginHook::ResRules),
            "tunnel_rules" => Some(PluginHook::TunnelRules),
            "req_read" => Some(PluginHook::ReqRead),
            "req_write" => Some(PluginHook::ReqWrite),
            "res_read" => Some(PluginHook::ResRead),
            "res_write" => Some(PluginHook::ResWrite),
            "ws_req_read" => Some(PluginHook::WsReqRead),
            "ws_req_write" => Some(PluginHook::WsReqWrite),
            "ws_res_read" => Some(PluginHook::WsResRead),
            "ws_res_write" => Some(PluginHook::WsResWrite),
            "tunnel_req_read" => Some(PluginHook::TunnelReqRead),
            "tunnel_req_write" => Some(PluginHook::TunnelReqWrite),
            "tunnel_res_read" => Some(PluginHook::TunnelResRead),
            "tunnel_res_write" => Some(PluginHook::TunnelResWrite),
            "req_stats" => Some(PluginHook::ReqStats),
            "res_stats" => Some(PluginHook::ResStats),
            _ => None,
        }
    }

    pub fn is_request_hook(&self) -> bool {
        matches!(
            self,
            PluginHook::ReqRules
                | PluginHook::ReqRead
                | PluginHook::ReqWrite
                | PluginHook::WsReqRead
                | PluginHook::WsReqWrite
                | PluginHook::TunnelReqRead
                | PluginHook::TunnelReqWrite
                | PluginHook::ReqStats
        )
    }

    pub fn is_response_hook(&self) -> bool {
        matches!(
            self,
            PluginHook::ResRules
                | PluginHook::ResRead
                | PluginHook::ResWrite
                | PluginHook::WsResRead
                | PluginHook::WsResWrite
                | PluginHook::TunnelResRead
                | PluginHook::TunnelResWrite
                | PluginHook::ResStats
        )
    }

    pub fn is_tunnel_hook(&self) -> bool {
        matches!(
            self,
            PluginHook::Tunnel
                | PluginHook::TunnelRules
                | PluginHook::TunnelReqRead
                | PluginHook::TunnelReqWrite
                | PluginHook::TunnelResRead
                | PluginHook::TunnelResWrite
        )
    }

    pub fn is_websocket_hook(&self) -> bool {
        matches!(
            self,
            PluginHook::WsReqRead
                | PluginHook::WsReqWrite
                | PluginHook::WsResRead
                | PluginHook::WsResWrite
        )
    }
}

impl fmt::Display for PluginHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_count() {
        assert_eq!(PluginHook::ALL.len(), 22);
    }

    #[test]
    fn test_hook_as_str() {
        assert_eq!(PluginHook::Auth.as_str(), "auth");
        assert_eq!(PluginHook::ReqRules.as_str(), "req_rules");
        assert_eq!(PluginHook::WsReqRead.as_str(), "ws_req_read");
    }

    #[test]
    fn test_hook_from_str() {
        assert_eq!(PluginHook::from_str("auth"), Some(PluginHook::Auth));
        assert_eq!(
            PluginHook::from_str("req_rules"),
            Some(PluginHook::ReqRules)
        );
        assert_eq!(PluginHook::from_str("invalid"), None);
    }

    #[test]
    fn test_hook_roundtrip() {
        for hook in PluginHook::ALL {
            let s = hook.as_str();
            assert_eq!(PluginHook::from_str(s), Some(hook));
        }
    }

    #[test]
    fn test_request_hooks() {
        assert!(PluginHook::ReqRead.is_request_hook());
        assert!(PluginHook::ReqWrite.is_request_hook());
        assert!(!PluginHook::ResRead.is_request_hook());
    }

    #[test]
    fn test_response_hooks() {
        assert!(PluginHook::ResRead.is_response_hook());
        assert!(PluginHook::ResWrite.is_response_hook());
        assert!(!PluginHook::ReqRead.is_response_hook());
    }

    #[test]
    fn test_tunnel_hooks() {
        assert!(PluginHook::Tunnel.is_tunnel_hook());
        assert!(PluginHook::TunnelReqRead.is_tunnel_hook());
        assert!(!PluginHook::Http.is_tunnel_hook());
    }

    #[test]
    fn test_websocket_hooks() {
        assert!(PluginHook::WsReqRead.is_websocket_hook());
        assert!(PluginHook::WsResWrite.is_websocket_hook());
        assert!(!PluginHook::ReqRead.is_websocket_hook());
    }

    #[test]
    fn test_serde_serialization() {
        let hook = PluginHook::Auth;
        let json = serde_json::to_string(&hook).unwrap();
        assert_eq!(json, "\"auth\"");
    }

    #[test]
    fn test_serde_deserialization() {
        let hook: PluginHook = serde_json::from_str("\"req_rules\"").unwrap();
        assert_eq!(hook, PluginHook::ReqRules);
    }
}

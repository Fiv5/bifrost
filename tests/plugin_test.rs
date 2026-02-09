use async_trait::async_trait;
use bytes::Bytes;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use bifrost_plugin::{
    AuthContext, DataContext, HttpContext, PluginContext, PluginHook, PluginManager,
    RulesContext, TunnelContext, BifrostPlugin, Result as PluginResult, StatsContext,
};

struct TestPlugin {
    name: String,
    hooks: Vec<PluginHook>,
    priority: i32,
    http_called: AtomicBool,
    auth_called: AtomicBool,
    tunnel_called: AtomicBool,
}

impl TestPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            hooks: vec![PluginHook::Http, PluginHook::Auth, PluginHook::Tunnel],
            priority: 0,
            http_called: AtomicBool::new(false),
            auth_called: AtomicBool::new(false),
            tunnel_called: AtomicBool::new(false),
        }
    }

    fn with_hooks(mut self, hooks: Vec<PluginHook>) -> Self {
        self.hooks = hooks;
        self
    }

    fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

#[async_trait]
impl BifrostPlugin for TestPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        self.hooks.clone()
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> PluginResult<()> {
        self.http_called.store(true, Ordering::SeqCst);
        ctx.base.headers.insert("X-Plugin".to_string(), self.name.clone());
        ctx.modified = true;
        Ok(())
    }

    async fn on_auth(&self, ctx: &mut AuthContext) -> PluginResult<()> {
        self.auth_called.store(true, Ordering::SeqCst);
        ctx.approve();
        ctx.username = Some("test-user".to_string());
        Ok(())
    }

    async fn on_tunnel(&self, ctx: &mut TunnelContext) -> PluginResult<()> {
        self.tunnel_called.store(true, Ordering::SeqCst);
        ctx.enable_capture();
        Ok(())
    }
}

#[tokio::test]
async fn test_rust_plugin_hook() {
    let plugin = TestPlugin::new("test-plugin");

    let base = PluginContext::new("session-1".to_string(), "request-1".to_string());
    let mut http_ctx = HttpContext::new(base);

    plugin.on_http(&mut http_ctx).await.unwrap();

    assert!(plugin.http_called.load(Ordering::SeqCst));
    assert!(http_ctx.modified);
    assert_eq!(http_ctx.base.headers.get("X-Plugin"), Some(&"test-plugin".to_string()));
}

#[tokio::test]
async fn test_plugin_auth_hook() {
    let plugin = TestPlugin::new("auth-plugin");

    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let mut auth_ctx = AuthContext::new(base);

    plugin.on_auth(&mut auth_ctx).await.unwrap();

    assert!(plugin.auth_called.load(Ordering::SeqCst));
    assert!(auth_ctx.authenticated);
    assert_eq!(auth_ctx.username, Some("test-user".to_string()));
}

#[tokio::test]
async fn test_plugin_tunnel_hook() {
    let plugin = TestPlugin::new("tunnel-plugin");

    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let mut tunnel_ctx = TunnelContext::new(base);

    plugin.on_tunnel(&mut tunnel_ctx).await.unwrap();

    assert!(plugin.tunnel_called.load(Ordering::SeqCst));
    assert!(tunnel_ctx.capture);
}

#[tokio::test]
async fn test_plugin_context_headers() {
    let mut base = PluginContext::new("s1".to_string(), "r1".to_string());

    base.headers.insert("Content-Type".to_string(), "application/json".to_string());
    base.headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    base.headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());

    assert_eq!(base.headers.get("Content-Type"), Some(&"application/json".to_string()));
    assert_eq!(base.headers.get("Authorization"), Some(&"Bearer token123".to_string()));
    assert_eq!(base.headers.len(), 3);
}

#[tokio::test]
async fn test_plugin_manager_registration() {
    let manager = PluginManager::new();

    let plugin1 = TestPlugin::new("plugin-1").with_priority(10);
    let plugin2 = TestPlugin::new("plugin-2").with_priority(5);

    manager.register_rust_plugin(plugin1).unwrap();
    manager.register_rust_plugin(plugin2).unwrap();

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 2);
}

#[tokio::test]
async fn test_plugin_manager_duplicate_registration() {
    let manager = PluginManager::new();

    let plugin1 = TestPlugin::new("same-name");
    let plugin2 = TestPlugin::new("same-name");

    manager.register_rust_plugin(plugin1).unwrap();
    let result = manager.register_rust_plugin(plugin2);

    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_manager_hooks_registry() {
    let manager = PluginManager::new();

    let http_plugin = TestPlugin::new("http-only")
        .with_hooks(vec![PluginHook::Http]);
    let auth_plugin = TestPlugin::new("auth-only")
        .with_hooks(vec![PluginHook::Auth]);
    let multi_plugin = TestPlugin::new("multi-hook")
        .with_hooks(vec![PluginHook::Http, PluginHook::Auth, PluginHook::Tunnel]);

    manager.register_rust_plugin(http_plugin).unwrap();
    manager.register_rust_plugin(auth_plugin).unwrap();
    manager.register_rust_plugin(multi_plugin).unwrap();

    let http_plugins = manager.get_plugins_for_hook(PluginHook::Http);
    assert_eq!(http_plugins.len(), 2);

    let auth_plugins = manager.get_plugins_for_hook(PluginHook::Auth);
    assert_eq!(auth_plugins.len(), 2);

    let tunnel_plugins = manager.get_plugins_for_hook(PluginHook::Tunnel);
    assert_eq!(tunnel_plugins.len(), 1);
}

#[tokio::test]
async fn test_plugin_manager_lifecycle() {
    let manager = PluginManager::new();

    let plugin = TestPlugin::new("lifecycle-test");
    manager.register_rust_plugin(plugin).unwrap();

    manager.start().await.unwrap();
    assert!(manager.is_running());

    manager.stop().await.unwrap();
    assert!(!manager.is_running());
}

#[tokio::test]
async fn test_plugin_priority_ordering() {
    let manager = PluginManager::new();

    let low_priority = TestPlugin::new("low-priority").with_priority(1);
    let high_priority = TestPlugin::new("high-priority").with_priority(100);
    let mid_priority = TestPlugin::new("mid-priority").with_priority(50);

    manager.register_rust_plugin(low_priority).unwrap();
    manager.register_rust_plugin(high_priority).unwrap();
    manager.register_rust_plugin(mid_priority).unwrap();

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 3);
}

#[tokio::test]
async fn test_data_context() {
    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let data = Bytes::from("test data content");
    let mut data_ctx = DataContext::new(base, data.clone());

    assert_eq!(data_ctx.data, data);
    assert!(data_ctx.modified_data.is_none());

    data_ctx.modify(Bytes::from("modified data"));

    assert!(data_ctx.modified_data.is_some());
    assert_eq!(data_ctx.modified_data.unwrap(), Bytes::from("modified data"));
}

#[tokio::test]
async fn test_rules_context() {
    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let mut rules_ctx = RulesContext::new(base);

    rules_ctx.add_rule("example.com host://127.0.0.1");
    rules_ctx.add_rule("*.api.com proxy://proxy.local:8080");

    assert_eq!(rules_ctx.additional_rules.len(), 2);
}

#[tokio::test]
async fn test_stats_context() {
    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let mut stats_ctx = StatsContext::new(base);

    stats_ctx.bytes_transferred = 1024;
    stats_ctx.duration_ms = 150;

    assert_eq!(stats_ctx.bytes_transferred, 1024);
    assert_eq!(stats_ctx.duration_ms, 150);
}

#[test]
fn test_plugin_hook_all() {
    assert_eq!(PluginHook::ALL.len(), 22);
}

#[test]
fn test_plugin_hook_as_str() {
    assert_eq!(PluginHook::Http.as_str(), "http");
    assert_eq!(PluginHook::Auth.as_str(), "auth");
    assert_eq!(PluginHook::Tunnel.as_str(), "tunnel");
    assert_eq!(PluginHook::ReqRules.as_str(), "req_rules");
    assert_eq!(PluginHook::ResRules.as_str(), "res_rules");
}

#[test]
fn test_plugin_hook_from_str() {
    assert_eq!(PluginHook::from_str("http"), Some(PluginHook::Http));
    assert_eq!(PluginHook::from_str("auth"), Some(PluginHook::Auth));
    assert_eq!(PluginHook::from_str("tunnel"), Some(PluginHook::Tunnel));
    assert_eq!(PluginHook::from_str("invalid"), None);
}

#[test]
fn test_plugin_hook_is_request() {
    assert!(PluginHook::ReqRules.is_request_hook());
    assert!(PluginHook::ReqRead.is_request_hook());
    assert!(PluginHook::ReqWrite.is_request_hook());
    assert!(!PluginHook::ResRules.is_request_hook());
}

#[test]
fn test_plugin_hook_is_response() {
    assert!(PluginHook::ResRules.is_response_hook());
    assert!(PluginHook::ResRead.is_response_hook());
    assert!(PluginHook::ResWrite.is_response_hook());
    assert!(!PluginHook::ReqRules.is_response_hook());
}

#[test]
fn test_plugin_hook_is_tunnel() {
    assert!(PluginHook::Tunnel.is_tunnel_hook());
    assert!(PluginHook::TunnelRules.is_tunnel_hook());
    assert!(PluginHook::TunnelReqRead.is_tunnel_hook());
    assert!(!PluginHook::Http.is_tunnel_hook());
}

#[test]
fn test_plugin_hook_is_websocket() {
    assert!(PluginHook::WsReqRead.is_websocket_hook());
    assert!(PluginHook::WsReqWrite.is_websocket_hook());
    assert!(PluginHook::WsResRead.is_websocket_hook());
    assert!(PluginHook::WsResWrite.is_websocket_hook());
    assert!(!PluginHook::Http.is_websocket_hook());
}

#[test]
fn test_plugin_hook_roundtrip() {
    for hook in PluginHook::ALL {
        let s = hook.as_str();
        let parsed = PluginHook::from_str(s);
        assert_eq!(parsed, Some(hook), "Roundtrip failed for {:?}", hook);
    }
}

#[test]
fn test_plugin_context_new() {
    let ctx = PluginContext::new("session-123".to_string(), "request-456".to_string());
    assert_eq!(ctx.session_id, "session-123");
    assert_eq!(ctx.request_id, "request-456");
    assert!(ctx.headers.is_empty());
}

#[test]
fn test_http_context_new() {
    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let ctx = HttpContext::new(base);
    assert!(!ctx.modified);
    assert!(ctx.body.is_none());
}

#[test]
fn test_auth_context_new() {
    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let ctx = AuthContext::new(base);
    assert!(!ctx.authenticated);
    assert!(ctx.username.is_none());
}

#[test]
fn test_tunnel_context_new() {
    let base = PluginContext::new("s1".to_string(), "r1".to_string());
    let ctx = TunnelContext::new(base);
    assert!(!ctx.capture);
    assert!(ctx.sni.is_none());
}

struct CountingPlugin {
    call_count: AtomicU32,
}

impl CountingPlugin {
    fn new() -> Self {
        Self {
            call_count: AtomicU32::new(0),
        }
    }

    fn get_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl BifrostPlugin for CountingPlugin {
    fn name(&self) -> &str {
        "counting-plugin"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http]
    }

    async fn on_http(&self, _ctx: &mut HttpContext) -> PluginResult<()> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn test_plugin_multiple_calls() {
    let plugin = CountingPlugin::new();

    for _ in 0..5 {
        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut ctx = HttpContext::new(base);
        plugin.on_http(&mut ctx).await.unwrap();
    }

    assert_eq!(plugin.get_count(), 5);
}

struct HeaderModifyPlugin;

#[async_trait]
impl BifrostPlugin for HeaderModifyPlugin {
    fn name(&self) -> &str {
        "header-modify"
    }

    fn hooks(&self) -> Vec<PluginHook> {
        vec![PluginHook::Http]
    }

    async fn on_http(&self, ctx: &mut HttpContext) -> PluginResult<()> {
        ctx.set_header("X-Modified", "true");
        ctx.set_header("X-Timestamp", "1234567890");
        Ok(())
    }
}

#[tokio::test]
async fn test_header_modification_plugin() {
    let plugin = HeaderModifyPlugin;

    let base = PluginContext::new("s".to_string(), "r".to_string());
    let mut ctx = HttpContext::new(base);

    plugin.on_http(&mut ctx).await.unwrap();

    assert!(ctx.modified);
    assert_eq!(ctx.base.headers.get("X-Modified"), Some(&"true".to_string()));
    assert_eq!(ctx.base.headers.get("X-Timestamp"), Some(&"1234567890".to_string()));
}

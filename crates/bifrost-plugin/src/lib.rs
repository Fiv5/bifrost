pub mod context;
pub mod error;
pub mod hook;
pub mod manager;
pub mod nodejs;
pub mod protocol;
pub mod rust_sdk;

pub use context::{
    AuthContext, DataContext, HttpContext, PluginContext, RulesContext, StatsContext, TunnelContext,
};
pub use error::{PluginError, Result};
pub use hook::PluginHook;
pub use manager::PluginManager;
pub use nodejs::{DiscoveredPlugin, NodePluginManager, NodePluginProcess, PluginStatus};
pub use protocol::{
    ConnectRequest, DataDirection, DataMessage, PluginInfo, PluginProtocol, PluginRequest,
    PluginResponse, TunnelPolicy,
};
pub use rust_sdk::{BifrostPlugin, PluginBuilder, PluginMetadata};

pub const HOOK_COUNT: usize = 22;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_count_constant() {
        assert_eq!(HOOK_COUNT, PluginHook::ALL.len());
        assert_eq!(HOOK_COUNT, 22);
    }

    #[test]
    fn test_all_hooks_defined() {
        let hooks = PluginHook::ALL;
        assert!(hooks.contains(&PluginHook::Auth));
        assert!(hooks.contains(&PluginHook::Sni));
        assert!(hooks.contains(&PluginHook::Ui));
        assert!(hooks.contains(&PluginHook::Http));
        assert!(hooks.contains(&PluginHook::Tunnel));
        assert!(hooks.contains(&PluginHook::ReqRules));
        assert!(hooks.contains(&PluginHook::ResRules));
        assert!(hooks.contains(&PluginHook::TunnelRules));
        assert!(hooks.contains(&PluginHook::ReqRead));
        assert!(hooks.contains(&PluginHook::ReqWrite));
        assert!(hooks.contains(&PluginHook::ResRead));
        assert!(hooks.contains(&PluginHook::ResWrite));
        assert!(hooks.contains(&PluginHook::WsReqRead));
        assert!(hooks.contains(&PluginHook::WsReqWrite));
        assert!(hooks.contains(&PluginHook::WsResRead));
        assert!(hooks.contains(&PluginHook::WsResWrite));
        assert!(hooks.contains(&PluginHook::TunnelReqRead));
        assert!(hooks.contains(&PluginHook::TunnelReqWrite));
        assert!(hooks.contains(&PluginHook::TunnelResRead));
        assert!(hooks.contains(&PluginHook::TunnelResWrite));
        assert!(hooks.contains(&PluginHook::ReqStats));
        assert!(hooks.contains(&PluginHook::ResStats));
    }

    #[test]
    fn test_public_exports() {
        let _ = PluginContext::new("s".to_string(), "r".to_string());
        let _ = PluginHook::Http;
        let _ = TunnelPolicy::Tunnel;
        let _ = PluginStatus::Stopped;
        let _ = DataDirection::Request;
    }

    #[tokio::test]
    async fn test_plugin_manager_integration() {
        use async_trait::async_trait;

        struct TestIntegrationPlugin;

        #[async_trait]
        impl BifrostPlugin for TestIntegrationPlugin {
            fn name(&self) -> &str {
                "integration-test"
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![PluginHook::Http, PluginHook::Auth]
            }
        }

        let manager = PluginManager::new();
        manager.register_rust_plugin(TestIntegrationPlugin).unwrap();

        let plugins = manager.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "integration-test");
        assert_eq!(plugins[0].hooks.len(), 2);

        let http_plugins = manager.get_plugins_for_hook(PluginHook::Http);
        assert_eq!(http_plugins.len(), 1);

        manager.start().await.unwrap();
        assert!(manager.is_running());

        manager.stop().await.unwrap();
        assert!(!manager.is_running());
    }
}

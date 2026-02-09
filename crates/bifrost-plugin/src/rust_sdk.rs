

use async_trait::async_trait;

use crate::context::{
    AuthContext, DataContext, HttpContext, PluginContext, RulesContext, StatsContext,
    TunnelContext,
};
use crate::error::Result;
use crate::hook::PluginHook;

#[async_trait]
pub trait BifrostPlugin: Send + Sync {
    fn name(&self) -> &str;

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn hooks(&self) -> Vec<PluginHook>;

    fn priority(&self) -> i32 {
        0
    }

    async fn on_init(&self) -> Result<()> {
        Ok(())
    }

    async fn on_shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn on_auth(&self, _ctx: &mut AuthContext) -> Result<()> {
        Ok(())
    }

    async fn on_sni(&self, _ctx: &mut PluginContext) -> Result<Option<String>> {
        Ok(None)
    }

    async fn on_ui(&self, _ctx: &mut PluginContext) -> Result<Option<String>> {
        Ok(None)
    }

    async fn on_http(&self, _ctx: &mut HttpContext) -> Result<()> {
        Ok(())
    }

    async fn on_tunnel(&self, _ctx: &mut TunnelContext) -> Result<()> {
        Ok(())
    }

    async fn on_req_rules(&self, _ctx: &mut RulesContext) -> Result<()> {
        Ok(())
    }

    async fn on_res_rules(&self, _ctx: &mut RulesContext) -> Result<()> {
        Ok(())
    }

    async fn on_tunnel_rules(&self, _ctx: &mut RulesContext) -> Result<()> {
        Ok(())
    }

    async fn on_req_read(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_req_write(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_res_read(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_res_write(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_ws_req_read(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_ws_req_write(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_ws_res_read(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_ws_res_write(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_tunnel_req_read(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_tunnel_req_write(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_tunnel_res_read(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_tunnel_res_write(&self, _ctx: &mut DataContext) -> Result<()> {
        Ok(())
    }

    async fn on_req_stats(&self, _ctx: &mut StatsContext) -> Result<()> {
        Ok(())
    }

    async fn on_res_stats(&self, _ctx: &mut StatsContext) -> Result<()> {
        Ok(())
    }
}

pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub hooks: Vec<PluginHook>,
    pub priority: i32,
}

impl PluginMetadata {
    pub fn from_plugin<P: BifrostPlugin + ?Sized>(plugin: &P) -> Self {
        Self {
            name: plugin.name().to_string(),
            version: plugin.version().to_string(),
            hooks: plugin.hooks(),
            priority: plugin.priority(),
        }
    }
}

pub struct PluginBuilder {
    name: String,
    version: String,
    hooks: Vec<PluginHook>,
    priority: i32,
}

impl PluginBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            hooks: Vec::new(),
            priority: 0,
        }
    }

    pub fn version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn hook(mut self, hook: PluginHook) -> Self {
        if !self.hooks.contains(&hook) {
            self.hooks.push(hook);
        }
        self
    }

    pub fn hooks(mut self, hooks: impl IntoIterator<Item = PluginHook>) -> Self {
        for hook in hooks {
            if !self.hooks.contains(&hook) {
                self.hooks.push(hook);
            }
        }
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn metadata(self) -> PluginMetadata {
        PluginMetadata {
            name: self.name,
            version: self.version,
            hooks: self.hooks,
            priority: self.priority,
        }
    }
}

#[macro_export]
macro_rules! impl_bifrost_plugin {
    ($type:ty, $name:expr, $hooks:expr) => {
        impl $crate::rust_sdk::BifrostPlugin for $type {
            fn name(&self) -> &str {
                $name
            }

            fn hooks(&self) -> Vec<$crate::hook::PluginHook> {
                $hooks.to_vec()
            }
        }
    };
    ($type:ty, $name:expr, $hooks:expr, priority = $priority:expr) => {
        impl $crate::rust_sdk::BifrostPlugin for $type {
            fn name(&self) -> &str {
                $name
            }

            fn hooks(&self) -> Vec<$crate::hook::PluginHook> {
                $hooks.to_vec()
            }

            fn priority(&self) -> i32 {
                $priority
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    struct TestPlugin {
        name: String,
    }

    #[async_trait]
    impl BifrostPlugin for TestPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        fn hooks(&self) -> Vec<PluginHook> {
            vec![PluginHook::Http, PluginHook::Auth]
        }

        fn priority(&self) -> i32 {
            10
        }

        async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
            ctx.set_header("X-Test-Plugin", "true");
            Ok(())
        }

        async fn on_auth(&self, ctx: &mut AuthContext) -> Result<()> {
            if ctx.username == Some("admin".to_string()) {
                ctx.approve();
            }
            Ok(())
        }
    }

    #[test]
    fn test_plugin_metadata() {
        let plugin = TestPlugin {
            name: "test-plugin".to_string(),
        };
        let metadata = PluginMetadata::from_plugin(&plugin);

        assert_eq!(metadata.name, "test-plugin");
        assert_eq!(metadata.version, "0.1.0");
        assert_eq!(metadata.hooks.len(), 2);
        assert_eq!(metadata.priority, 10);
    }

    #[test]
    fn test_plugin_builder() {
        let metadata = PluginBuilder::new("my-plugin")
            .version("1.0.0")
            .hook(PluginHook::Http)
            .hook(PluginHook::Tunnel)
            .hooks(vec![PluginHook::Auth, PluginHook::Sni])
            .priority(5)
            .metadata();

        assert_eq!(metadata.name, "my-plugin");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.hooks.len(), 4);
        assert_eq!(metadata.priority, 5);
    }

    #[test]
    fn test_plugin_builder_no_duplicate_hooks() {
        let metadata = PluginBuilder::new("test")
            .hook(PluginHook::Http)
            .hook(PluginHook::Http)
            .hook(PluginHook::Http)
            .metadata();

        assert_eq!(metadata.hooks.len(), 1);
    }

    #[tokio::test]
    async fn test_plugin_on_http() {
        let plugin = TestPlugin {
            name: "test".to_string(),
        };

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = HttpContext::new(base);

        plugin.on_http(&mut ctx).await.unwrap();

        assert!(ctx.modified);
        assert_eq!(
            ctx.base.headers.get("X-Test-Plugin"),
            Some(&"true".to_string())
        );
    }

    #[tokio::test]
    async fn test_plugin_on_auth() {
        let plugin = TestPlugin {
            name: "test".to_string(),
        };

        let base = PluginContext::new("s1".to_string(), "r1".to_string());
        let mut ctx = AuthContext::new(base);
        ctx.username = Some("admin".to_string());

        plugin.on_auth(&mut ctx).await.unwrap();
        assert!(ctx.authenticated);

        let base2 = PluginContext::new("s2".to_string(), "r2".to_string());
        let mut ctx2 = AuthContext::new(base2);
        ctx2.username = Some("user".to_string());

        plugin.on_auth(&mut ctx2).await.unwrap();
        assert!(!ctx2.authenticated);
    }

    #[tokio::test]
    async fn test_default_hook_implementations() {
        struct MinimalPlugin;

        #[async_trait]
        impl BifrostPlugin for MinimalPlugin {
            fn name(&self) -> &str {
                "minimal"
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![]
            }
        }

        let plugin = MinimalPlugin;

        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut auth_ctx = AuthContext::new(base.clone());
        assert!(plugin.on_auth(&mut auth_ctx).await.is_ok());

        let mut http_ctx = HttpContext::new(base.clone());
        assert!(plugin.on_http(&mut http_ctx).await.is_ok());

        let mut tunnel_ctx = TunnelContext::new(base.clone());
        assert!(plugin.on_tunnel(&mut tunnel_ctx).await.is_ok());

        let mut rules_ctx = RulesContext::new(base.clone());
        assert!(plugin.on_req_rules(&mut rules_ctx).await.is_ok());

        let mut data_ctx = DataContext::new(base.clone(), Bytes::new());
        assert!(plugin.on_req_read(&mut data_ctx).await.is_ok());

        let mut stats_ctx = StatsContext::new(base.clone());
        assert!(plugin.on_req_stats(&mut stats_ctx).await.is_ok());

        assert!(plugin.on_sni(&mut base.clone()).await.unwrap().is_none());
        assert!(plugin.on_ui(&mut base.clone()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_plugin_init_shutdown() {
        struct LifecyclePlugin {
            init_called: std::sync::atomic::AtomicBool,
            shutdown_called: std::sync::atomic::AtomicBool,
        }

        #[async_trait]
        impl BifrostPlugin for LifecyclePlugin {
            fn name(&self) -> &str {
                "lifecycle"
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![]
            }

            async fn on_init(&self) -> Result<()> {
                self.init_called
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }

            async fn on_shutdown(&self) -> Result<()> {
                self.shutdown_called
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        let plugin = LifecyclePlugin {
            init_called: std::sync::atomic::AtomicBool::new(false),
            shutdown_called: std::sync::atomic::AtomicBool::new(false),
        };

        plugin.on_init().await.unwrap();
        assert!(plugin.init_called.load(std::sync::atomic::Ordering::SeqCst));

        plugin.on_shutdown().await.unwrap();
        assert!(plugin
            .shutdown_called
            .load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_plugin_default_version() {
        struct VersionPlugin;

        #[async_trait]
        impl BifrostPlugin for VersionPlugin {
            fn name(&self) -> &str {
                "version"
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![]
            }
        }

        let plugin = VersionPlugin;
        assert_eq!(plugin.version(), "0.1.0");
    }

    #[test]
    fn test_plugin_custom_version() {
        struct CustomVersionPlugin;

        #[async_trait]
        impl BifrostPlugin for CustomVersionPlugin {
            fn name(&self) -> &str {
                "custom"
            }

            fn version(&self) -> &str {
                "2.0.0"
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![]
            }
        }

        let plugin = CustomVersionPlugin;
        assert_eq!(plugin.version(), "2.0.0");
    }
}

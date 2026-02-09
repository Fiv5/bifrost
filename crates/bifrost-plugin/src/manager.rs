use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::context::{
    AuthContext, DataContext, HttpContext, PluginContext, RulesContext, StatsContext,
    TunnelContext,
};
use crate::error::{PluginError, Result};
use crate::hook::PluginHook;
use crate::nodejs::NodePluginManager;
use crate::protocol::PluginInfo;
use crate::rust_sdk::BifrostPlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    Rust,
    NodeJs,
}

struct RegisteredRustPlugin {
    plugin: Arc<dyn BifrostPlugin>,
    hooks: Vec<PluginHook>,
    priority: i32,
}

pub struct PluginManager {
    rust_plugins: Arc<RwLock<HashMap<String, RegisteredRustPlugin>>>,
    nodejs_manager: Option<Arc<NodePluginManager>>,
    hook_registry: Arc<RwLock<HashMap<PluginHook, Vec<String>>>>,
    running: Arc<RwLock<bool>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            rust_plugins: Arc::new(RwLock::new(HashMap::new())),
            nodejs_manager: None,
            hook_registry: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub fn with_nodejs_plugins<P: AsRef<Path>>(mut self, plugin_dir: P, base_port: u16) -> Self {
        self.nodejs_manager = Some(Arc::new(NodePluginManager::new(plugin_dir, base_port)));
        self
    }

    pub fn register_rust_plugin<P: BifrostPlugin + 'static>(&self, plugin: P) -> Result<()> {
        let name = plugin.name().to_string();
        let hooks = plugin.hooks();
        let priority = plugin.priority();

        {
            let plugins = self.rust_plugins.read();
            if plugins.contains_key(&name) {
                return Err(PluginError::AlreadyRegistered(name));
            }
        }

        {
            let mut registry = self.hook_registry.write();
            for hook in &hooks {
                registry
                    .entry(*hook)
                    .or_insert_with(Vec::new)
                    .push(name.clone());
            }
        }

        {
            let mut plugins = self.rust_plugins.write();
            plugins.insert(
                name,
                RegisteredRustPlugin {
                    plugin: Arc::new(plugin),
                    hooks,
                    priority,
                },
            );
        }

        Ok(())
    }

    pub fn unregister_rust_plugin(&self, name: &str) -> Result<()> {
        let removed = {
            let mut plugins = self.rust_plugins.write();
            plugins.remove(name)
        };

        if let Some(plugin) = removed {
            let mut registry = self.hook_registry.write();
            for hook in &plugin.hooks {
                if let Some(names) = registry.get_mut(hook) {
                    names.retain(|n| n != name);
                }
            }
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()))
        }
    }

    pub async fn register_nodejs_plugin(&self, name: &str) -> Result<u16> {
        let manager = self
            .nodejs_manager
            .as_ref()
            .ok_or_else(|| PluginError::Config("NodeJS plugin manager not configured".into()))?;

        let port = manager.start_plugin(name).await?;

        if let Some(info) = manager.get_plugin(name) {
            let mut registry = self.hook_registry.write();
            for hook in &info.hooks {
                registry
                    .entry(*hook)
                    .or_insert_with(Vec::new)
                    .push(name.to_string());
            }
        }

        Ok(port)
    }

    pub async fn unregister_nodejs_plugin(&self, name: &str) -> Result<()> {
        let manager = self
            .nodejs_manager
            .as_ref()
            .ok_or_else(|| PluginError::Config("NodeJS plugin manager not configured".into()))?;

        if let Some(info) = manager.get_plugin(name) {
            let mut registry = self.hook_registry.write();
            for hook in &info.hooks {
                if let Some(names) = registry.get_mut(&hook) {
                    names.retain(|n| n != name);
                }
            }
        }

        manager.stop_plugin(name).await
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.write();
            if *running {
                return Ok(());
            }
            *running = true;
        }

        let plugins: Vec<Arc<dyn BifrostPlugin>> = {
            let plugins = self.rust_plugins.read();
            plugins.values().map(|p| p.plugin.clone()).collect()
        };

        for plugin in plugins {
            plugin.on_init().await?;
        }

        if let Some(ref manager) = self.nodejs_manager {
            manager.discover().await?;
        }

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        {
            let mut running = self.running.write();
            if !*running {
                return Ok(());
            }
            *running = false;
        }

        let plugins: Vec<Arc<dyn BifrostPlugin>> = {
            let plugins = self.rust_plugins.read();
            plugins.values().map(|p| p.plugin.clone()).collect()
        };

        for plugin in plugins {
            plugin.on_shutdown().await?;
        }

        if let Some(ref manager) = self.nodejs_manager {
            manager.stop_all().await?;
        }

        Ok(())
    }

    pub async fn reload(&self) -> Result<()> {
        if let Some(ref manager) = self.nodejs_manager {
            manager.stop_all().await?;
            manager.discover().await?;
        }
        Ok(())
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        let mut infos = Vec::new();

        {
            let plugins = self.rust_plugins.read();
            for (name, registered) in plugins.iter() {
                infos.push(PluginInfo {
                    name: name.clone(),
                    version: registered.plugin.version().to_string(),
                    hooks: registered.hooks.clone(),
                    port: 0,
                    protocol: crate::protocol::PluginProtocol::Http,
                });
            }
        }

        if let Some(ref manager) = self.nodejs_manager {
            infos.extend(manager.list_plugins());
        }

        infos
    }

    pub fn get_plugins_for_hook(&self, hook: PluginHook) -> Vec<String> {
        let registry = self.hook_registry.read();
        registry.get(&hook).cloned().unwrap_or_default()
    }

    #[allow(dead_code)]
    fn get_rust_plugin(&self, name: &str) -> Option<Arc<dyn BifrostPlugin>> {
        let plugins = self.rust_plugins.read();
        plugins.get(name).map(|p| p.plugin.clone())
    }

    fn get_sorted_rust_plugins_for_hook(&self, hook: PluginHook) -> Vec<Arc<dyn BifrostPlugin>> {
        let plugins = self.rust_plugins.read();
        let mut matching: Vec<_> = plugins
            .values()
            .filter(|p| p.hooks.contains(&hook))
            .collect();

        matching.sort_by(|a, b| b.priority.cmp(&a.priority));
        matching.into_iter().map(|p| p.plugin.clone()).collect()
    }

    pub async fn dispatch_auth(&self, ctx: &mut AuthContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::Auth) {
            plugin.on_auth(ctx).await?;
            if ctx.authenticated {
                break;
            }
        }
        Ok(())
    }

    pub async fn dispatch_http(&self, ctx: &mut HttpContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::Http) {
            plugin.on_http(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_tunnel(&self, ctx: &mut TunnelContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::Tunnel) {
            plugin.on_tunnel(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_req_rules(&self, ctx: &mut RulesContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ReqRules) {
            plugin.on_req_rules(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_res_rules(&self, ctx: &mut RulesContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ResRules) {
            plugin.on_res_rules(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_req_read(&self, ctx: &mut DataContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ReqRead) {
            plugin.on_req_read(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_req_write(&self, ctx: &mut DataContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ReqWrite) {
            plugin.on_req_write(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_res_read(&self, ctx: &mut DataContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ResRead) {
            plugin.on_res_read(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_res_write(&self, ctx: &mut DataContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ResWrite) {
            plugin.on_res_write(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_req_stats(&self, ctx: &mut StatsContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ReqStats) {
            plugin.on_req_stats(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_res_stats(&self, ctx: &mut StatsContext) -> Result<()> {
        for plugin in self.get_sorted_rust_plugins_for_hook(PluginHook::ResStats) {
            plugin.on_res_stats(ctx).await?;
        }
        Ok(())
    }

    pub async fn dispatch_hook(
        &self,
        hook: PluginHook,
        context: &PluginContext,
        body: Option<Bytes>,
    ) -> Result<Vec<crate::protocol::PluginResponse>> {
        let mut responses = Vec::new();

        if let Some(ref manager) = self.nodejs_manager {
            let plugin_names = self.get_plugins_for_hook(hook);
            for name in plugin_names {
                if manager.get_plugin(&name).is_some() {
                    let response = manager.forward(&name, hook, context, body.clone()).await?;
                    responses.push(response);
                }
            }
        }

        Ok(responses)
    }

    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    pub fn rust_plugin_count(&self) -> usize {
        self.rust_plugins.read().len()
    }

    pub fn nodejs_plugin_count(&self) -> usize {
        self.nodejs_manager
            .as_ref()
            .map(|m| m.list_plugins().len())
            .unwrap_or(0)
    }

    pub fn total_plugin_count(&self) -> usize {
        self.rust_plugin_count() + self.nodejs_plugin_count()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockPlugin {
        name: String,
        hooks: Vec<PluginHook>,
        priority: i32,
    }

    impl MockPlugin {
        fn new(name: &str, hooks: Vec<PluginHook>, priority: i32) -> Self {
            Self {
                name: name.to_string(),
                hooks,
                priority,
            }
        }
    }

    #[async_trait]
    impl BifrostPlugin for MockPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        fn hooks(&self) -> Vec<PluginHook> {
            self.hooks.clone()
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[test]
    fn test_plugin_manager_new() {
        let manager = PluginManager::new();
        assert!(!manager.is_running());
        assert_eq!(manager.rust_plugin_count(), 0);
    }

    #[test]
    fn test_register_rust_plugin() {
        let manager = PluginManager::new();
        let plugin = MockPlugin::new("test", vec![PluginHook::Http], 0);

        manager.register_rust_plugin(plugin).unwrap();
        assert_eq!(manager.rust_plugin_count(), 1);
    }

    #[test]
    fn test_register_duplicate_plugin() {
        let manager = PluginManager::new();
        let plugin1 = MockPlugin::new("test", vec![PluginHook::Http], 0);
        let plugin2 = MockPlugin::new("test", vec![PluginHook::Auth], 0);

        manager.register_rust_plugin(plugin1).unwrap();
        assert!(manager.register_rust_plugin(plugin2).is_err());
    }

    #[test]
    fn test_unregister_rust_plugin() {
        let manager = PluginManager::new();
        let plugin = MockPlugin::new("test", vec![PluginHook::Http], 0);

        manager.register_rust_plugin(plugin).unwrap();
        assert_eq!(manager.rust_plugin_count(), 1);

        manager.unregister_rust_plugin("test").unwrap();
        assert_eq!(manager.rust_plugin_count(), 0);
    }

    #[test]
    fn test_unregister_nonexistent_plugin() {
        let manager = PluginManager::new();
        assert!(manager.unregister_rust_plugin("nonexistent").is_err());
    }

    #[test]
    fn test_get_plugins_for_hook() {
        let manager = PluginManager::new();

        let plugin1 = MockPlugin::new("plugin1", vec![PluginHook::Http, PluginHook::Auth], 0);
        let plugin2 = MockPlugin::new("plugin2", vec![PluginHook::Http], 0);
        let plugin3 = MockPlugin::new("plugin3", vec![PluginHook::Tunnel], 0);

        manager.register_rust_plugin(plugin1).unwrap();
        manager.register_rust_plugin(plugin2).unwrap();
        manager.register_rust_plugin(plugin3).unwrap();

        let http_plugins = manager.get_plugins_for_hook(PluginHook::Http);
        assert_eq!(http_plugins.len(), 2);

        let auth_plugins = manager.get_plugins_for_hook(PluginHook::Auth);
        assert_eq!(auth_plugins.len(), 1);

        let tunnel_plugins = manager.get_plugins_for_hook(PluginHook::Tunnel);
        assert_eq!(tunnel_plugins.len(), 1);
    }

    #[test]
    fn test_list_plugins() {
        let manager = PluginManager::new();

        let plugin1 = MockPlugin::new("plugin1", vec![PluginHook::Http], 0);
        let plugin2 = MockPlugin::new("plugin2", vec![PluginHook::Auth], 0);

        manager.register_rust_plugin(plugin1).unwrap();
        manager.register_rust_plugin(plugin2).unwrap();

        let plugins = manager.list_plugins();
        assert_eq!(plugins.len(), 2);
    }

    #[tokio::test]
    async fn test_start_stop() {
        let manager = PluginManager::new();
        let plugin = MockPlugin::new("test", vec![PluginHook::Http], 0);

        manager.register_rust_plugin(plugin).unwrap();

        assert!(!manager.is_running());

        manager.start().await.unwrap();
        assert!(manager.is_running());

        manager.start().await.unwrap();
        assert!(manager.is_running());

        manager.stop().await.unwrap();
        assert!(!manager.is_running());

        manager.stop().await.unwrap();
        assert!(!manager.is_running());
    }

    #[tokio::test]
    async fn test_dispatch_auth() {
        struct AuthPlugin;

        #[async_trait]
        impl BifrostPlugin for AuthPlugin {
            fn name(&self) -> &str {
                "auth-plugin"
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![PluginHook::Auth]
            }

            async fn on_auth(&self, ctx: &mut AuthContext) -> Result<()> {
                if ctx.username == Some("admin".to_string()) {
                    ctx.approve();
                }
                Ok(())
            }
        }

        let manager = PluginManager::new();
        manager.register_rust_plugin(AuthPlugin).unwrap();

        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut ctx = AuthContext::new(base);
        ctx.username = Some("admin".to_string());

        manager.dispatch_auth(&mut ctx).await.unwrap();
        assert!(ctx.authenticated);
    }

    #[tokio::test]
    async fn test_dispatch_http() {
        struct HttpPlugin {
            name: String,
        }

        #[async_trait]
        impl BifrostPlugin for HttpPlugin {
            fn name(&self) -> &str {
                &self.name
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![PluginHook::Http]
            }

            async fn on_http(&self, ctx: &mut HttpContext) -> Result<()> {
                ctx.set_header(&format!("X-{}", self.name), "true");
                Ok(())
            }
        }

        let manager = PluginManager::new();
        manager
            .register_rust_plugin(HttpPlugin {
                name: "plugin1".to_string(),
            })
            .unwrap();
        manager
            .register_rust_plugin(HttpPlugin {
                name: "plugin2".to_string(),
            })
            .unwrap();

        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut ctx = HttpContext::new(base);

        manager.dispatch_http(&mut ctx).await.unwrap();

        assert!(ctx.base.headers.contains_key("X-plugin1"));
        assert!(ctx.base.headers.contains_key("X-plugin2"));
    }

    #[tokio::test]
    async fn test_plugin_priority_order() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static CALL_ORDER: AtomicUsize = AtomicUsize::new(0);

        struct PriorityPlugin {
            name: String,
            priority: i32,
            expected_order: usize,
        }

        #[async_trait]
        impl BifrostPlugin for PriorityPlugin {
            fn name(&self) -> &str {
                &self.name
            }

            fn hooks(&self) -> Vec<PluginHook> {
                vec![PluginHook::Http]
            }

            fn priority(&self) -> i32 {
                self.priority
            }

            async fn on_http(&self, _ctx: &mut HttpContext) -> Result<()> {
                let order = CALL_ORDER.fetch_add(1, Ordering::SeqCst);
                assert_eq!(
                    order, self.expected_order,
                    "Plugin {} called in wrong order",
                    self.name
                );
                Ok(())
            }
        }

        CALL_ORDER.store(0, Ordering::SeqCst);

        let manager = PluginManager::new();

        manager
            .register_rust_plugin(PriorityPlugin {
                name: "low".to_string(),
                priority: 1,
                expected_order: 2,
            })
            .unwrap();

        manager
            .register_rust_plugin(PriorityPlugin {
                name: "high".to_string(),
                priority: 10,
                expected_order: 0,
            })
            .unwrap();

        manager
            .register_rust_plugin(PriorityPlugin {
                name: "medium".to_string(),
                priority: 5,
                expected_order: 1,
            })
            .unwrap();

        let base = PluginContext::new("s".to_string(), "r".to_string());
        let mut ctx = HttpContext::new(base);

        manager.dispatch_http(&mut ctx).await.unwrap();
    }

    #[test]
    fn test_with_nodejs_plugins() {
        let manager = PluginManager::new().with_nodejs_plugins("/tmp/plugins", 9000);
        assert!(manager.nodejs_manager.is_some());
    }

    #[test]
    fn test_total_plugin_count() {
        let manager = PluginManager::new();

        let plugin = MockPlugin::new("test", vec![PluginHook::Http], 0);
        manager.register_rust_plugin(plugin).unwrap();

        assert_eq!(manager.rust_plugin_count(), 1);
        assert_eq!(manager.nodejs_plugin_count(), 0);
        assert_eq!(manager.total_plugin_count(), 1);
    }

    #[test]
    fn test_hook_registry_cleanup_on_unregister() {
        let manager = PluginManager::new();

        let plugin = MockPlugin::new("test", vec![PluginHook::Http, PluginHook::Auth], 0);
        manager.register_rust_plugin(plugin).unwrap();

        assert_eq!(manager.get_plugins_for_hook(PluginHook::Http).len(), 1);
        assert_eq!(manager.get_plugins_for_hook(PluginHook::Auth).len(), 1);

        manager.unregister_rust_plugin("test").unwrap();

        assert_eq!(manager.get_plugins_for_hook(PluginHook::Http).len(), 0);
        assert_eq!(manager.get_plugins_for_hook(PluginHook::Auth).len(), 0);
    }
}

use dashmap::DashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, info, warn};

#[derive(Clone, Debug)]
pub enum ConfigChangeEvent {
    GlobalInterceptionChanged {
        enabled: bool,
    },
    ExcludeListChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
    IncludeListChanged {
        added: Vec<String>,
        removed: Vec<String>,
    },
    RulesChanged,
}

pub struct ConnectionInfo {
    pub req_id: String,
    pub host: String,
    pub port: u16,
    pub intercept_mode: bool,
    pub client_app: Option<String>,
    pub established_at: Instant,
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl ConnectionInfo {
    pub fn new(
        req_id: String,
        host: String,
        port: u16,
        intercept_mode: bool,
        client_app: Option<String>,
        cancel_tx: oneshot::Sender<()>,
    ) -> Self {
        Self {
            req_id,
            host,
            port,
            intercept_mode,
            client_app,
            established_at: Instant::now(),
            cancel_tx: Some(cancel_tx),
        }
    }

    pub fn cancel(&mut self) -> bool {
        if let Some(tx) = self.cancel_tx.take() {
            tx.send(()).is_ok()
        } else {
            false
        }
    }
}

pub struct ConnectionRegistry {
    connections: DashMap<String, ConnectionInfo>,
    config_change_tx: broadcast::Sender<ConfigChangeEvent>,
    disconnect_on_config_change: bool,
}

impl Default for ConnectionRegistry {
    fn default() -> Self {
        Self::new(true)
    }
}

impl ConnectionRegistry {
    pub fn new(disconnect_on_config_change: bool) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            connections: DashMap::new(),
            config_change_tx: tx,
            disconnect_on_config_change,
        }
    }

    pub fn register(&self, info: ConnectionInfo) {
        let req_id = info.req_id.clone();
        let host = info.host.clone();
        let port = info.port;
        let intercept_mode = info.intercept_mode;

        self.connections.insert(req_id.clone(), info);

        debug!(
            "[{}] Connection registered: {}:{}, intercept={}, active={}",
            req_id,
            host,
            port,
            intercept_mode,
            self.connections.len()
        );
    }

    pub fn unregister(&self, req_id: &str) {
        if let Some((_, info)) = self.connections.remove(req_id) {
            debug!(
                "[{}] Connection unregistered: {}:{}, active={}",
                req_id,
                info.host,
                info.port,
                self.connections.len()
            );
        }
    }

    pub fn update_client_app(&self, req_id: &str, client_app: String) {
        if let Some(mut entry) = self.connections.get_mut(req_id) {
            entry.client_app = Some(client_app);
        }
    }

    pub fn active_count(&self) -> usize {
        self.connections.len()
    }

    pub fn is_disconnect_enabled(&self) -> bool {
        self.disconnect_on_config_change
    }

    pub fn notify_config_change(&self, event: ConfigChangeEvent) {
        if let Err(e) = self.config_change_tx.send(event) {
            debug!("No subscribers for config change event: {}", e);
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConfigChangeEvent> {
        self.config_change_tx.subscribe()
    }

    pub fn disconnect_affected<F>(&self, should_disconnect: F) -> Vec<String>
    where
        F: Fn(&ConnectionInfo) -> bool,
    {
        if !self.disconnect_on_config_change {
            debug!("Disconnect on config change is disabled, skipping");
            return Vec::new();
        }

        let mut disconnected = Vec::new();
        let mut to_remove = Vec::new();

        for mut entry in self.connections.iter_mut() {
            let info: &mut ConnectionInfo = entry.value_mut();
            if should_disconnect(info) {
                let req_id = info.req_id.clone();
                let host = info.host.clone();
                let port = info.port;
                let old_mode = if info.intercept_mode {
                    "intercept"
                } else {
                    "passthrough"
                };

                if info.cancel() {
                    info!(
                        "[{}] Disconnecting tunnel {}:{} (was: {})",
                        req_id, host, port, old_mode
                    );
                    disconnected.push(req_id.clone());
                    to_remove.push(req_id);
                } else {
                    warn!(
                        "[{}] Failed to cancel tunnel {}:{} - already closed",
                        req_id, host, port
                    );
                }
            }
        }

        for req_id in to_remove {
            self.connections.remove(&req_id);
        }

        if !disconnected.is_empty() {
            info!(
                "Config change disconnected {} connections, {} remaining",
                disconnected.len(),
                self.connections.len()
            );
        }

        disconnected
    }

    pub fn disconnect_by_host_pattern(&self, patterns: &[String]) -> Vec<String> {
        self.disconnect_affected(|info| is_domain_matched(&info.host, patterns))
    }

    pub fn disconnect_by_host_pattern_with_mode(
        &self,
        pattern: &str,
        intercept_mode: bool,
    ) -> Vec<String> {
        self.disconnect_affected(|info| {
            info.intercept_mode == intercept_mode
                && is_domain_matched(&info.host, &[pattern.to_string()])
        })
    }

    pub fn disconnect_all_with_mode(&self, intercept_mode: bool) -> Vec<String> {
        self.disconnect_affected(|info| info.intercept_mode == intercept_mode)
    }

    pub fn disconnect_by_app(&self, app_name: &str) -> Vec<String> {
        self.disconnect_affected(|info| {
            info.client_app
                .as_ref()
                .map(|app| app == app_name)
                .unwrap_or(false)
        })
    }

    pub fn list_connections(&self) -> Vec<(String, String, u16, bool)> {
        self.connections
            .iter()
            .map(
                |entry: dashmap::mapref::multiple::RefMulti<'_, String, ConnectionInfo>| {
                    let info: &ConnectionInfo = entry.value();
                    (
                        info.req_id.clone(),
                        info.host.clone(),
                        info.port,
                        info.intercept_mode,
                    )
                },
            )
            .collect()
    }

    pub fn list_connections_full(&self) -> Vec<(String, String, u16, bool, Option<String>)> {
        self.connections
            .iter()
            .map(
                |entry: dashmap::mapref::multiple::RefMulti<'_, String, ConnectionInfo>| {
                    let info: &ConnectionInfo = entry.value();
                    (
                        info.req_id.clone(),
                        info.host.clone(),
                        info.port,
                        info.intercept_mode,
                        info.client_app.clone(),
                    )
                },
            )
            .collect()
    }
}

fn is_domain_matched(host: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if pattern.starts_with("*.") {
            let suffix = &pattern[1..];
            if host.ends_with(suffix) || host == &pattern[2..] {
                return true;
            }
        } else if pattern == host {
            return true;
        }
    }
    false
}

pub type SharedConnectionRegistry = Arc<ConnectionRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_domain_matched() {
        assert!(is_domain_matched(
            "api.example.com",
            &["*.example.com".to_string()]
        ));
        assert!(is_domain_matched(
            "example.com",
            &["*.example.com".to_string()]
        ));
        assert!(is_domain_matched(
            "example.com",
            &["example.com".to_string()]
        ));
        assert!(!is_domain_matched(
            "other.com",
            &["*.example.com".to_string()]
        ));
        assert!(!is_domain_matched(
            "notexample.com",
            &["*.example.com".to_string()]
        ));
    }

    #[tokio::test]
    async fn test_connection_registry_basic() {
        let registry = ConnectionRegistry::new(true);

        let (cancel_tx, _cancel_rx) = oneshot::channel();
        let info = ConnectionInfo::new(
            "req-001".to_string(),
            "api.example.com".to_string(),
            443,
            true,
            Some("TestApp".to_string()),
            cancel_tx,
        );

        registry.register(info);
        assert_eq!(registry.active_count(), 1);

        registry.unregister("req-001");
        assert_eq!(registry.active_count(), 0);
    }

    #[tokio::test]
    async fn test_disconnect_by_pattern() {
        let registry = ConnectionRegistry::new(true);

        let (tx1, _rx1) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-001".to_string(),
            "api.example.com".to_string(),
            443,
            true,
            Some("App1".to_string()),
            tx1,
        ));

        let (tx2, _rx2) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-002".to_string(),
            "api.other.com".to_string(),
            443,
            true,
            Some("App2".to_string()),
            tx2,
        ));

        let (tx3, _rx3) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-003".to_string(),
            "cdn.example.com".to_string(),
            443,
            false,
            None,
            tx3,
        ));

        assert_eq!(registry.active_count(), 3);

        let disconnected = registry.disconnect_by_host_pattern(&["*.example.com".to_string()]);
        assert_eq!(disconnected.len(), 2);
        assert!(disconnected.contains(&"req-001".to_string()));
        assert!(disconnected.contains(&"req-003".to_string()));
        assert_eq!(registry.active_count(), 1);
    }

    #[tokio::test]
    async fn test_disconnect_by_app() {
        let registry = ConnectionRegistry::new(true);

        let (tx1, _rx1) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-001".to_string(),
            "api.example.com".to_string(),
            443,
            true,
            Some("Safari".to_string()),
            tx1,
        ));

        let (tx2, _rx2) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-002".to_string(),
            "api.other.com".to_string(),
            443,
            true,
            Some("Chrome".to_string()),
            tx2,
        ));

        let (tx3, _rx3) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-003".to_string(),
            "cdn.example.com".to_string(),
            443,
            false,
            Some("Safari".to_string()),
            tx3,
        ));

        assert_eq!(registry.active_count(), 3);

        let disconnected = registry.disconnect_by_app("Safari");
        assert_eq!(disconnected.len(), 2);
        assert!(disconnected.contains(&"req-001".to_string()));
        assert!(disconnected.contains(&"req-003".to_string()));
        assert_eq!(registry.active_count(), 1);
    }

    #[tokio::test]
    async fn test_disconnect_disabled() {
        let registry = ConnectionRegistry::new(false);

        let (tx1, _rx1) = oneshot::channel();
        registry.register(ConnectionInfo::new(
            "req-001".to_string(),
            "api.example.com".to_string(),
            443,
            true,
            None,
            tx1,
        ));

        let disconnected = registry.disconnect_affected(|_| true);
        assert!(disconnected.is_empty());
        assert_eq!(registry.active_count(), 1);
    }
}

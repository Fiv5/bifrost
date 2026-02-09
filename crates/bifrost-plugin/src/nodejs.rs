use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::context::PluginContext;
use crate::error::{PluginError, Result};
use crate::hook::PluginHook;
use crate::protocol::{PluginInfo, PluginRequest, PluginResponse};

const PLUGIN_PREFIX: &str = "whistle.";
const PACKAGE_JSON: &str = "package.json";

#[derive(Debug)]
pub struct NodePluginProcess {
    pub name: String,
    pub port: u16,
    pub hooks: Vec<PluginHook>,
    child: Option<Child>,
    status: PluginStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStatus {
    Stopped,
    Starting,
    Running,
    Failed,
}

impl NodePluginProcess {
    pub fn new(name: &str, port: u16, hooks: Vec<PluginHook>) -> Self {
        Self {
            name: name.to_string(),
            port,
            hooks,
            child: None,
            status: PluginStatus::Stopped,
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == PluginStatus::Running
    }

    pub fn status(&self) -> PluginStatus {
        self.status
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
    pub hooks: Vec<PluginHook>,
}

pub struct NodePluginManager {
    plugin_dir: PathBuf,
    plugins: Arc<RwLock<HashMap<String, NodePluginProcess>>>,
    discovered: Arc<RwLock<HashMap<String, DiscoveredPlugin>>>,
    #[allow(dead_code)]
    base_port: u16,
    next_port: Arc<RwLock<u16>>,
}

impl NodePluginManager {
    pub fn new<P: AsRef<Path>>(plugin_dir: P, base_port: u16) -> Self {
        Self {
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
            plugins: Arc::new(RwLock::new(HashMap::new())),
            discovered: Arc::new(RwLock::new(HashMap::new())),
            base_port,
            next_port: Arc::new(RwLock::new(base_port)),
        }
    }

    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    fn allocate_port(&self) -> u16 {
        let mut next = self.next_port.write();
        let port = *next;
        *next += 1;
        port
    }

    pub async fn discover(&self) -> Result<Vec<DiscoveredPlugin>> {
        let mut discovered = Vec::new();

        if !self.plugin_dir.exists() {
            return Ok(discovered);
        }

        let mut entries = tokio::fs::read_dir(&self.plugin_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            if !dir_name.starts_with(PLUGIN_PREFIX) {
                continue;
            }

            let package_path = path.join(PACKAGE_JSON);
            if !package_path.exists() {
                continue;
            }

            match self.parse_plugin_package(&package_path).await {
                Ok(plugin) => {
                    let mut disc = self.discovered.write();
                    disc.insert(plugin.name.clone(), plugin.clone());
                    discovered.push(plugin);
                }
                Err(e) => {
                    tracing::warn!("Failed to parse plugin at {:?}: {}", path, e);
                }
            }
        }

        Ok(discovered)
    }

    async fn parse_plugin_package(&self, package_path: &Path) -> Result<DiscoveredPlugin> {
        let content = tokio::fs::read_to_string(package_path).await?;
        let package: serde_json::Value = serde_json::from_str(&content)?;

        let name = package["name"]
            .as_str()
            .ok_or_else(|| PluginError::Config("Missing plugin name".into()))?
            .to_string();

        let version = package["version"].as_str().unwrap_or("0.0.0").to_string();

        let hooks = self.parse_hooks(&package);

        let plugin_dir = package_path.parent().unwrap().to_path_buf();

        Ok(DiscoveredPlugin {
            name,
            path: plugin_dir,
            version,
            hooks,
        })
    }

    fn parse_hooks(&self, package: &serde_json::Value) -> Vec<PluginHook> {
        let mut hooks = Vec::new();

        if let Some(whistle) = package.get("bifrost") {
            if let Some(hook_list) = whistle.get("hooks").and_then(|h| h.as_array()) {
                for hook_val in hook_list {
                    if let Some(hook_str) = hook_val.as_str() {
                        if let Some(hook) = PluginHook::parse(hook_str) {
                            hooks.push(hook);
                        }
                    }
                }
            }
        }

        if hooks.is_empty() {
            hooks.push(PluginHook::Http);
        }

        hooks
    }

    pub async fn start_plugin(&self, name: &str) -> Result<u16> {
        let plugin_info = {
            let discovered = self.discovered.read();
            discovered
                .get(name)
                .ok_or_else(|| PluginError::NotFound(name.to_string()))?
                .clone()
        };

        let port = self.allocate_port();

        let mut cmd = Command::new("node");
        cmd.current_dir(&plugin_info.path)
            .arg(".")
            .env("WHISTLE_PLUGIN_PORT", port.to_string())
            .env("WHISTLE_PLUGIN_NAME", &plugin_info.name)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let (ready_tx, mut ready_rx) = mpsc::channel::<bool>(1);

        if let Some(stdout) = child.stdout.take() {
            let tx = ready_tx.clone();
            let plugin_name = plugin_info.name.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!("[{}] {}", plugin_name, line);
                    if line.contains("listening") || line.contains("started") {
                        let _ = tx.send(true).await;
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let plugin_name = plugin_info.name.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::warn!("[{}] stderr: {}", plugin_name, line);
                }
            });
        }

        let process = NodePluginProcess {
            name: plugin_info.name.clone(),
            port,
            hooks: plugin_info.hooks.clone(),
            child: Some(child),
            status: PluginStatus::Starting,
        };

        {
            let mut plugins = self.plugins.write();
            plugins.insert(plugin_info.name.clone(), process);
        }

        let timeout = tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx.recv());

        match timeout.await {
            Ok(Some(true)) => {
                let mut plugins = self.plugins.write();
                if let Some(p) = plugins.get_mut(&plugin_info.name) {
                    p.status = PluginStatus::Running;
                }
            }
            _ => {
                let mut plugins = self.plugins.write();
                if let Some(p) = plugins.get_mut(&plugin_info.name) {
                    p.status = PluginStatus::Running;
                }
            }
        }

        Ok(port)
    }

    pub async fn stop_plugin(&self, name: &str) -> Result<()> {
        let process = {
            let mut plugins = self.plugins.write();
            plugins.remove(name)
        };

        if let Some(mut process) = process {
            if let Some(ref mut child) = process.child {
                child.kill().await.ok();
            }
            process.status = PluginStatus::Stopped;
        }
        Ok(())
    }

    pub async fn stop_all(&self) -> Result<()> {
        let names: Vec<String> = {
            let plugins = self.plugins.read();
            plugins.keys().cloned().collect()
        };

        for name in names {
            self.stop_plugin(&name).await?;
        }

        Ok(())
    }

    pub fn get_plugin(&self, name: &str) -> Option<PluginInfo> {
        let plugins = self.plugins.read();
        plugins.get(name).map(|p| PluginInfo {
            name: p.name.clone(),
            version: String::new(),
            hooks: p.hooks.clone(),
            port: p.port,
            protocol: crate::protocol::PluginProtocol::Http,
        })
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.read();
        plugins
            .values()
            .map(|p| PluginInfo {
                name: p.name.clone(),
                version: String::new(),
                hooks: p.hooks.clone(),
                port: p.port,
                protocol: crate::protocol::PluginProtocol::Http,
            })
            .collect()
    }

    pub fn list_discovered(&self) -> Vec<DiscoveredPlugin> {
        let discovered = self.discovered.read();
        discovered.values().cloned().collect()
    }

    pub async fn forward(
        &self,
        name: &str,
        hook: PluginHook,
        context: &PluginContext,
        body: Option<Bytes>,
    ) -> Result<PluginResponse> {
        let plugin = {
            let plugins = self.plugins.read();
            plugins.get(name).map(|p| (p.port, p.status))
        };

        let (port, status) = plugin.ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        if status != PluginStatus::Running {
            return Err(PluginError::NotRunning(name.to_string()));
        }

        let mut request = PluginRequest::new(name, hook, context);
        if let Some(b) = body {
            request = request.with_body(b);
        }

        self.send_http_request(port, request).await
    }

    async fn send_http_request(&self, port: u16, request: PluginRequest) -> Result<PluginResponse> {
        use http_body_util::{BodyExt, Full};
        use hyper::client::conn::http1::handshake;
        use hyper::Request;
        use hyper_util::rt::TokioIo;

        let addr = format!("127.0.0.1:{}", port);
        let stream = tokio::net::TcpStream::connect(&addr).await?;
        let io = TokioIo::new(stream);

        let (mut sender, conn) = handshake(io).await?;

        tokio::spawn(async move {
            if let Err(e) = conn.await {
                tracing::error!("Connection error: {}", e);
            }
        });

        let path = request.to_http_path();
        let body_bytes = request.body.unwrap_or_default();

        let mut builder = Request::builder()
            .method(request.method.as_str())
            .uri(&path);

        for (key, value) in &request.headers {
            builder = builder.header(key, value);
        }

        let req = builder.body(Full::new(body_bytes))?;
        let res = sender.send_request(req).await?;

        let status_code = res.status().as_u16();
        let headers: HashMap<String, String> = res
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (k.as_str().to_string(), v.to_string()))
            })
            .collect();

        let body = res.into_body().collect().await?.to_bytes();

        Ok(PluginResponse {
            status_code,
            headers,
            body: if body.is_empty() { None } else { Some(body) },
            modified: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_node_plugin_process() {
        let process = NodePluginProcess::new("test-plugin", 8080, vec![PluginHook::Http]);
        assert_eq!(process.name, "test-plugin");
        assert_eq!(process.port, 8080);
        assert!(!process.is_running());
        assert_eq!(process.status(), PluginStatus::Stopped);
    }

    #[test]
    fn test_plugin_manager_new() {
        let manager = NodePluginManager::new("/tmp/plugins", 9000);
        assert_eq!(manager.plugin_dir(), Path::new("/tmp/plugins"));
    }

    #[test]
    fn test_allocate_port() {
        let manager = NodePluginManager::new("/tmp/plugins", 9000);
        assert_eq!(manager.allocate_port(), 9000);
        assert_eq!(manager.allocate_port(), 9001);
        assert_eq!(manager.allocate_port(), 9002);
    }

    #[tokio::test]
    async fn test_discover_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let manager = NodePluginManager::new(temp_dir.path(), 9000);
        let plugins = manager.discover().await.unwrap();
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_discover_nonexistent_dir() {
        let manager = NodePluginManager::new("/nonexistent/path", 9000);
        let plugins = manager.discover().await.unwrap();
        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn test_discover_with_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let plugin_dir = temp_dir.path().join("whistle.test");
        std::fs::create_dir(&plugin_dir).unwrap();

        let package_json = r#"{
            "name": "whistle.test",
            "version": "1.0.0",
            "bifrost": {
                "hooks": ["http", "auth"]
            }
        }"#;
        std::fs::write(plugin_dir.join("package.json"), package_json).unwrap();

        let manager = NodePluginManager::new(temp_dir.path(), 9000);
        let plugins = manager.discover().await.unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "whistle.test");
        assert_eq!(plugins[0].version, "1.0.0");
        assert!(plugins[0].hooks.contains(&PluginHook::Http));
        assert!(plugins[0].hooks.contains(&PluginHook::Auth));
    }

    #[tokio::test]
    async fn test_discover_skips_non_whistle_dirs() {
        let temp_dir = TempDir::new().unwrap();

        let other_dir = temp_dir.path().join("other-plugin");
        std::fs::create_dir(&other_dir).unwrap();
        std::fs::write(
            other_dir.join("package.json"),
            r#"{"name": "other-plugin", "version": "1.0.0"}"#,
        )
        .unwrap();

        let manager = NodePluginManager::new(temp_dir.path(), 9000);
        let plugins = manager.discover().await.unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_list_plugins_empty() {
        let manager = NodePluginManager::new("/tmp/plugins", 9000);
        assert!(manager.list_plugins().is_empty());
    }

    #[test]
    fn test_get_plugin_not_found() {
        let manager = NodePluginManager::new("/tmp/plugins", 9000);
        assert!(manager.get_plugin("nonexistent").is_none());
    }

    #[test]
    fn test_discovered_plugin() {
        let plugin = DiscoveredPlugin {
            name: "whistle.test".to_string(),
            path: PathBuf::from("/path/to/plugin"),
            version: "1.0.0".to_string(),
            hooks: vec![PluginHook::Http, PluginHook::Auth],
        };

        assert_eq!(plugin.name, "whistle.test");
        assert_eq!(plugin.version, "1.0.0");
        assert_eq!(plugin.hooks.len(), 2);
    }

    #[test]
    fn test_parse_hooks_default() {
        let manager = NodePluginManager::new("/tmp", 9000);
        let package: serde_json::Value = serde_json::from_str(r#"{"name": "test"}"#).unwrap();
        let hooks = manager.parse_hooks(&package);
        assert_eq!(hooks, vec![PluginHook::Http]);
    }

    #[test]
    fn test_parse_hooks_custom() {
        let manager = NodePluginManager::new("/tmp", 9000);
        let package: serde_json::Value = serde_json::from_str(
            r#"{"name": "test", "bifrost": {"hooks": ["auth", "tunnel", "sni"]}}"#,
        )
        .unwrap();
        let hooks = manager.parse_hooks(&package);
        assert_eq!(hooks.len(), 3);
        assert!(hooks.contains(&PluginHook::Auth));
        assert!(hooks.contains(&PluginHook::Tunnel));
        assert!(hooks.contains(&PluginHook::Sni));
    }
}

use std::path::{Path, PathBuf};

use bifrost_core::AccessMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UnifiedConfig {
    pub server: ServerConfig,
    pub tls: TlsConfig,
    pub access: AccessConfig,
    pub proxy: ProxySettings,
    pub system_proxy: SystemProxyConfig,
    pub traffic: TrafficConfig,
    pub paths: PathsConfig,
}

impl UnifiedConfig {
    pub fn default_for_data_dir(data_dir: &Path) -> Self {
        Self {
            server: ServerConfig::default(),
            tls: TlsConfig::default(),
            access: AccessConfig::default(),
            proxy: ProxySettings::default(),
            system_proxy: SystemProxyConfig::default(),
            traffic: TrafficConfig::default_for_data_dir(data_dir),
            paths: PathsConfig::default_for_data_dir(data_dir),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub socks5_port: Option<u16>,
    pub socks5_auth: Option<SocksAuthConfig>,
    pub timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 9900,
            host: "127.0.0.1".to_string(),
            socks5_port: None,
            socks5_auth: None,
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SocksAuthConfig {
    pub enabled: bool,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsConfig {
    pub enable_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub app_intercept_exclude: Vec<String>,
    pub app_intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
    pub disconnect_on_change: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enable_interception: true,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            app_intercept_exclude: Vec::new(),
            app_intercept_include: Vec::new(),
            unsafe_ssl: false,
            disconnect_on_change: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessConfig {
    pub mode: AccessMode,
    pub whitelist: Vec<String>,
    pub allow_lan: bool,
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self {
            mode: AccessMode::LocalOnly,
            whitelist: Vec::new(),
            allow_lan: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxySettings {
    pub upstream_proxy: Option<String>,
    pub no_proxy: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemProxyConfig {
    pub enabled: bool,
    pub bypass: String,
    pub auto_enable: bool,
}

impl Default for SystemProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bypass: "localhost,127.0.0.1,::1,*.local".to_string(),
            auto_enable: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrafficConfig {
    pub max_records: usize,
    pub max_body_memory_size: usize,
    pub max_body_buffer_size: usize,
    pub file_retention_days: u64,
}

impl Default for TrafficConfig {
    fn default() -> Self {
        Self {
            max_records: 5000,
            max_body_memory_size: 2 * 1024 * 1024,
            max_body_buffer_size: 32 * 1024 * 1024,
            file_retention_days: 7,
        }
    }
}

impl TrafficConfig {
    pub fn default_for_data_dir(_data_dir: &Path) -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub rules_dir: PathBuf,
    pub values_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub cert_dir: PathBuf,
    pub traffic_dir: PathBuf,
}

impl Default for PathsConfig {
    fn default() -> Self {
        let base = crate::data_dir();
        Self {
            rules_dir: base.join("rules"),
            values_dir: base.join("values"),
            plugins_dir: base.join("plugins"),
            cert_dir: base.join("certs"),
            traffic_dir: base.join("traffic"),
        }
    }
}

impl PathsConfig {
    pub fn default_for_data_dir(data_dir: &Path) -> Self {
        Self {
            rules_dir: data_dir.join("rules"),
            values_dir: data_dir.join("values"),
            plugins_dir: data_dir.join("plugins"),
            cert_dir: data_dir.join("certs"),
            traffic_dir: data_dir.join("traffic"),
        }
    }

    pub fn resolve_paths(&mut self, data_dir: &Path) {
        if self.rules_dir.is_relative() {
            self.rules_dir = data_dir.join(&self.rules_dir);
        }
        if self.values_dir.is_relative() {
            self.values_dir = data_dir.join(&self.values_dir);
        }
        if self.plugins_dir.is_relative() {
            self.plugins_dir = data_dir.join(&self.plugins_dir);
        }
        if self.cert_dir.is_relative() {
            self.cert_dir = data_dir.join(&self.cert_dir);
        }
        if self.traffic_dir.is_relative() {
            self.traffic_dir = data_dir.join(&self.traffic_dir);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TlsConfigUpdate {
    pub enable_interception: Option<bool>,
    pub intercept_exclude: Option<Vec<String>>,
    pub intercept_include: Option<Vec<String>>,
    pub app_intercept_exclude: Option<Vec<String>>,
    pub app_intercept_include: Option<Vec<String>>,
    pub unsafe_ssl: Option<bool>,
    pub disconnect_on_change: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct AccessConfigUpdate {
    pub mode: Option<AccessMode>,
    pub whitelist: Option<Vec<String>>,
    pub allow_lan: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct SystemProxyConfigUpdate {
    pub enabled: Option<bool>,
    pub bypass: Option<String>,
    pub auto_enable: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct TrafficConfigUpdate {
    pub max_records: Option<usize>,
    pub max_body_memory_size: Option<usize>,
    pub max_body_buffer_size: Option<usize>,
    pub file_retention_days: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_unified_config_default() {
        let config = UnifiedConfig::default();
        assert_eq!(config.server.port, 9900);
        assert_eq!(config.server.host, "127.0.0.1");
        assert!(config.tls.enable_interception);
        assert!(config.tls.intercept_exclude.is_empty());
        assert_eq!(config.access.mode, AccessMode::LocalOnly);
        assert!(!config.system_proxy.enabled);
    }

    #[test]
    fn test_unified_config_for_data_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config = UnifiedConfig::default_for_data_dir(temp_dir.path());

        assert_eq!(config.paths.rules_dir, temp_dir.path().join("rules"));
        assert_eq!(config.paths.values_dir, temp_dir.path().join("values"));
        assert_eq!(config.paths.cert_dir, temp_dir.path().join("certs"));
    }

    #[test]
    fn test_paths_resolve() {
        let temp_dir = TempDir::new().unwrap();
        let mut paths = PathsConfig {
            rules_dir: PathBuf::from("rules"),
            values_dir: PathBuf::from("values"),
            plugins_dir: PathBuf::from("plugins"),
            cert_dir: PathBuf::from("certs"),
            traffic_dir: PathBuf::from("traffic"),
        };

        paths.resolve_paths(temp_dir.path());

        assert_eq!(paths.rules_dir, temp_dir.path().join("rules"));
        assert_eq!(paths.values_dir, temp_dir.path().join("values"));
    }

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert!(config.enable_interception);
        assert!(!config.unsafe_ssl);
        assert!(config.disconnect_on_change);
    }

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.port, 9900);
        assert_eq!(config.host, "127.0.0.1");
        assert!(config.socks5_port.is_none());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_serialization() {
        let config = UnifiedConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: UnifiedConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.server.port, parsed.server.port);
        assert_eq!(
            config.tls.enable_interception,
            parsed.tls.enable_interception
        );
    }
}

use std::path::PathBuf;

use bifrost_core::{BifrostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AccessConfig {
    pub mode: String,
    pub whitelist: Vec<String>,
    pub allow_lan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SystemProxyConfig {
    pub enabled: bool,
    pub bypass: String,
}

impl Default for SystemProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bypass: "localhost,127.0.0.1,::1,*.local".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrafficConfig {
    pub max_records: usize,
    pub max_body_memory_size: usize,
    pub max_body_buffer_size: usize,
    pub temp_dir: PathBuf,
    pub file_retention_days: u64,
}

impl Default for TrafficConfig {
    fn default() -> Self {
        Self {
            max_records: 5000,
            max_body_memory_size: 2 * 1024 * 1024,  // 2MB
            max_body_buffer_size: 32 * 1024 * 1024, // 32MB
            temp_dir: crate::data_dir().join("traffic"),
            file_retention_days: 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BifrostConfig {
    pub port: u16,
    pub host: String,
    pub rules_dir: PathBuf,
    pub values_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub cert_dir: PathBuf,
    pub access: AccessConfig,
    pub traffic: TrafficConfig,
    pub enable_tls_interception: bool,
    pub intercept_mode: Option<String>,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub system_proxy: SystemProxyConfig,
}

impl Default for BifrostConfig {
    fn default() -> Self {
        let base = crate::data_dir();
        Self {
            port: 9900,
            host: "127.0.0.1".to_string(),
            rules_dir: base.join("rules"),
            values_dir: base.join("values"),
            plugins_dir: base.join("plugins"),
            cert_dir: base.join("certs"),
            access: AccessConfig::default(),
            traffic: TrafficConfig::default(),
            enable_tls_interception: true,
            intercept_mode: None,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            system_proxy: SystemProxyConfig::default(),
        }
    }
}

impl BifrostConfig {
    pub fn from_toml(content: &str) -> Result<Self> {
        toml::from_str(content)
            .map_err(|e| BifrostError::Parse(format!("Failed to parse TOML config: {}", e)))
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| BifrostError::Config(format!("Failed to serialize config: {}", e)))
    }

    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let content = self.to_toml()?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

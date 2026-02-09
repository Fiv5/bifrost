use std::path::PathBuf;

use bifrost_core::{BifrostError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BifrostConfig {
    pub port: u16,
    pub host: String,
    pub rules_dir: PathBuf,
    pub values_dir: PathBuf,
    pub plugins_dir: PathBuf,
    pub cert_dir: PathBuf,
}

impl Default for BifrostConfig {
    fn default() -> Self {
        Self {
            port: 8899,
            host: "127.0.0.1".to_string(),
            rules_dir: PathBuf::from(".bifrost/rules"),
            values_dir: PathBuf::from(".bifrost/values"),
            plugins_dir: PathBuf::from(".bifrost/plugins"),
            cert_dir: PathBuf::from(".bifrost/certs"),
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

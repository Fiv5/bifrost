use std::path::PathBuf;
use sysproxy::Sysproxy;

use crate::{BifrostError, Result};

const DEFAULT_BYPASS: &str = "localhost,127.0.0.1,::1,*.local";
const BACKUP_FILE_NAME: &str = "proxy_backup.json";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProxyBackup {
    pub enable: bool,
    pub host: String,
    pub port: u16,
    pub bypass: String,
}

impl From<&Sysproxy> for ProxyBackup {
    fn from(proxy: &Sysproxy) -> Self {
        Self {
            enable: proxy.enable,
            host: proxy.host.clone(),
            port: proxy.port,
            bypass: proxy.bypass.clone(),
        }
    }
}

impl From<ProxyBackup> for Sysproxy {
    fn from(backup: ProxyBackup) -> Self {
        Self {
            enable: backup.enable,
            host: backup.host,
            port: backup.port,
            bypass: backup.bypass,
        }
    }
}

pub struct SystemProxyManager {
    original_proxy: Option<Sysproxy>,
    is_set: bool,
    data_dir: PathBuf,
}

impl SystemProxyManager {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            original_proxy: None,
            is_set: false,
            data_dir,
        }
    }

    pub fn is_supported() -> bool {
        Sysproxy::is_support()
    }

    pub fn enable(&mut self, host: &str, port: u16, bypass: Option<&str>) -> Result<()> {
        if !Self::is_supported() {
            return Err(BifrostError::Config(
                "System proxy is not supported on this platform".to_string(),
            ));
        }

        if self.is_set {
            return Ok(());
        }

        let current = Sysproxy::get_system_proxy().map_err(|e| {
            BifrostError::Config(format!("Failed to get current system proxy: {}", e))
        })?;

        self.original_proxy = Some(current.clone());
        self.save_backup(&current)?;

        let bypass_str = bypass.unwrap_or(DEFAULT_BYPASS);
        let proxy = Sysproxy {
            enable: true,
            host: host.to_string(),
            port,
            bypass: bypass_str.to_string(),
        };

        proxy
            .set_system_proxy()
            .map_err(|e| BifrostError::Config(format!("Failed to set system proxy: {}", e)))?;

        self.is_set = true;
        tracing::info!(
            "System proxy enabled: {}:{} (bypass: {})",
            host,
            port,
            bypass_str
        );

        Ok(())
    }

    pub fn disable(&mut self) -> Result<()> {
        if !Self::is_supported() {
            return Ok(());
        }

        if !self.is_set {
            return Ok(());
        }

        let proxy = Sysproxy {
            enable: false,
            host: String::new(),
            port: 0,
            bypass: String::new(),
        };

        proxy
            .set_system_proxy()
            .map_err(|e| BifrostError::Config(format!("Failed to disable system proxy: {}", e)))?;

        self.is_set = false;
        tracing::info!("System proxy disabled");

        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        if !Self::is_supported() {
            return Ok(());
        }

        if !self.is_set {
            return Ok(());
        }

        let original = self
            .original_proxy
            .take()
            .or_else(|| self.load_backup().ok())
            .unwrap_or_else(|| Sysproxy {
                enable: false,
                host: String::new(),
                port: 0,
                bypass: String::new(),
            });

        original
            .set_system_proxy()
            .map_err(|e| BifrostError::Config(format!("Failed to restore system proxy: {}", e)))?;

        self.remove_backup();

        self.is_set = false;
        tracing::info!(
            "System proxy restored to original state (enabled: {}, host: {}, port: {})",
            original.enable,
            original.host,
            original.port
        );

        Ok(())
    }

    pub fn get_current() -> Result<ProxyBackup> {
        if !Self::is_supported() {
            return Err(BifrostError::Config(
                "System proxy is not supported on this platform".to_string(),
            ));
        }

        let current = Sysproxy::get_system_proxy()
            .map_err(|e| BifrostError::Config(format!("Failed to get system proxy: {}", e)))?;

        Ok(ProxyBackup::from(&current))
    }

    pub fn is_set(&self) -> bool {
        self.is_set
    }

    fn backup_file_path(&self) -> PathBuf {
        self.data_dir.join(BACKUP_FILE_NAME)
    }

    fn save_backup(&self, proxy: &Sysproxy) -> Result<()> {
        let backup = ProxyBackup::from(proxy);
        let content = serde_json::to_string_pretty(&backup).map_err(|e| {
            BifrostError::Config(format!("Failed to serialize proxy backup: {}", e))
        })?;

        if let Some(parent) = self.backup_file_path().parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(self.backup_file_path(), content)?;
        Ok(())
    }

    fn load_backup(&self) -> Result<Sysproxy> {
        let content = std::fs::read_to_string(self.backup_file_path())?;
        let backup: ProxyBackup = serde_json::from_str(&content).map_err(|e| {
            BifrostError::Config(format!("Failed to deserialize proxy backup: {}", e))
        })?;
        Ok(backup.into())
    }

    fn remove_backup(&self) {
        let _ = std::fs::remove_file(self.backup_file_path());
    }

    pub fn recover_from_crash(data_dir: &std::path::Path) -> Result<()> {
        if !Self::is_supported() {
            return Ok(());
        }

        let backup_path = data_dir.join(BACKUP_FILE_NAME);
        if !backup_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&backup_path)?;
        let backup: ProxyBackup = serde_json::from_str(&content).map_err(|e| {
            BifrostError::Config(format!("Failed to deserialize proxy backup: {}", e))
        })?;

        let proxy: Sysproxy = backup.into();
        proxy.set_system_proxy().map_err(|e| {
            BifrostError::Config(format!("Failed to restore system proxy from crash: {}", e))
        })?;

        std::fs::remove_file(&backup_path)?;
        tracing::info!("Recovered system proxy from previous crash");

        Ok(())
    }
}

impl Drop for SystemProxyManager {
    fn drop(&mut self) {
        if self.is_set {
            if let Err(e) = self.restore() {
                tracing::error!("Failed to restore system proxy on drop: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        let supported = SystemProxyManager::is_supported();
        println!("System proxy supported: {}", supported);
    }

    #[test]
    fn test_proxy_backup_serialization() {
        let backup = ProxyBackup {
            enable: true,
            host: "127.0.0.1".to_string(),
            port: 8899,
            bypass: "localhost".to_string(),
        };

        let json = serde_json::to_string(&backup).unwrap();
        let restored: ProxyBackup = serde_json::from_str(&json).unwrap();

        assert_eq!(backup.enable, restored.enable);
        assert_eq!(backup.host, restored.host);
        assert_eq!(backup.port, restored.port);
        assert_eq!(backup.bypass, restored.bypass);
    }
}

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

        #[cfg(target_os = "macos")]
        {
            // Best-effort: ensure all active network services are configured
            if let Err(e) = set_macos_all_services_proxy(host, port, bypass_str) {
                tracing::warn!("Failed to set macOS proxies for all services: {}", e);
            }
        }

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

        #[cfg(target_os = "macos")]
        {
            if let Err(e) = disable_macos_all_services_proxy() {
                tracing::warn!("Failed to disable macOS proxies for all services: {}", e);
            }
        }

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

        #[cfg(target_os = "macos")]
        {
            // Restore to original enable state for all services as best-effort
            if original.enable {
                if let Err(e) =
                    set_macos_all_services_proxy(&original.host, original.port, &original.bypass)
                {
                    tracing::warn!("Failed to restore macOS proxies for all services: {}", e);
                }
            } else if let Err(e) = disable_macos_all_services_proxy() {
                tracing::warn!("Failed to disable macOS proxies for all services: {}", e);
            }
        }

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

#[cfg(target_os = "macos")]
fn list_macos_services() -> Result<Vec<String>> {
    use std::process::Command;
    let output = Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .map_err(|e| BifrostError::Config(format!("Failed to list network services: {}", e)))?;
    if !output.status.success() {
        return Err(BifrostError::Config(
            "networksetup -listallnetworkservices failed".to_string(),
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if idx == 0 {
            // Skip header line
            continue;
        }
        let l = line.trim();
        if l.is_empty() || l.starts_with('*') {
            continue;
        }
        services.push(l.to_string());
    }
    Ok(services)
}

#[cfg(target_os = "macos")]
fn set_macos_all_services_proxy(host: &str, port: u16, bypass: &str) -> Result<()> {
    let services = list_macos_services()?;
    let bypass_domains: Vec<String> = bypass
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for svc in services {
        // HTTP
        run_networksetup(
            "networksetup",
            &["-setwebproxy", &svc, host, &port.to_string()],
        )?;
        run_networksetup("networksetup", &["-setwebproxystate", &svc, "on"])?;
        // HTTPS
        run_networksetup(
            "networksetup",
            &["-setsecurewebproxy", &svc, host, &port.to_string()],
        )?;
        run_networksetup("networksetup", &["-setsecurewebproxystate", &svc, "on"])?;
        // Bypass
        if !bypass_domains.is_empty() {
            let mut args = vec!["-setproxybypassdomains".to_string(), svc.clone()];
            args.extend(bypass_domains.iter().cloned());
            let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_networksetup("networksetup", &str_args)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn disable_macos_all_services_proxy() -> Result<()> {
    let services = list_macos_services()?;
    for svc in services {
        run_networksetup("networksetup", &["-setwebproxystate", &svc, "off"])?;
        run_networksetup("networksetup", &["-setsecurewebproxystate", &svc, "off"])?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_networksetup(cmd: &str, args: &[&str]) -> Result<()> {
    use std::process::Command;
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| BifrostError::Config(format!("Failed to execute {}: {}", cmd, e)))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let msg = format!(
        "networksetup failed (code {:?}): {} {}",
        output.status.code(),
        stdout.trim(),
        stderr.trim()
    );
    if is_permission_error(&stderr) || is_permission_error(&stdout) {
        return Err(BifrostError::Config(format!("RequiresAdmin: {}", msg)));
    }
    tracing::warn!("{}", msg);
    Err(BifrostError::Config(msg))
}

#[cfg(target_os = "macos")]
fn is_permission_error(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("administrator")
        || lower.contains("not authorized")
        || lower.contains("permission")
        || lower.contains("require")
}

#[cfg(target_os = "macos")]
pub fn set_macos_all_services_proxy_with_sudo(host: &str, port: u16, bypass: &str) -> Result<()> {
    let services = list_macos_services()?;
    let bypass_domains: Vec<String> = bypass
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for svc in services {
        // HTTP
        run_networksetup_with_sudo(&["-setwebproxy", &svc, host, &port.to_string()])?;
        run_networksetup_with_sudo(&["-setwebproxystate", &svc, "on"])?;
        // HTTPS
        run_networksetup_with_sudo(&["-setsecurewebproxy", &svc, host, &port.to_string()])?;
        run_networksetup_with_sudo(&["-setsecurewebproxystate", &svc, "on"])?;
        // Bypass
        if !bypass_domains.is_empty() {
            let mut args = vec!["-setproxybypassdomains".to_string(), svc.clone()];
            args.extend(bypass_domains.iter().cloned());
            let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            run_networksetup_with_sudo(&str_args)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn disable_macos_all_services_proxy_with_sudo() -> Result<()> {
    let services = list_macos_services()?;
    for svc in services {
        run_networksetup_with_sudo(&["-setwebproxystate", &svc, "off"])?;
        run_networksetup_with_sudo(&["-setsecurewebproxystate", &svc, "off"])?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn run_networksetup_with_sudo(args: &[&str]) -> Result<()> {
    use std::process::Command;
    let output = Command::new("/usr/bin/sudo")
        .arg("networksetup")
        .args(args)
        .output()
        .map_err(|e| BifrostError::Config(format!("Failed to execute sudo networksetup: {}", e)))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let msg = format!(
        "sudo networksetup failed (code {:?}): {} {}",
        output.status.code(),
        stdout.trim(),
        stderr.trim()
    );
    Err(BifrostError::Config(msg))
}

#[cfg(target_os = "macos")]
impl SystemProxyManager {
    pub fn enable_with_privilege(
        &mut self,
        host: &str,
        port: u16,
        bypass: Option<&str>,
    ) -> Result<()> {
        let bypass_str = bypass.unwrap_or(DEFAULT_BYPASS);
        let current = Sysproxy::get_system_proxy().map_err(|e| {
            BifrostError::Config(format!("Failed to get current system proxy: {}", e))
        })?;
        self.original_proxy = Some(current.clone());
        self.save_backup(&current)?;
        set_macos_all_services_proxy_with_sudo(host, port, bypass_str)?;
        self.is_set = true;
        Ok(())
    }

    pub fn disable_with_privilege(&mut self) -> Result<()> {
        disable_macos_all_services_proxy_with_sudo()?;
        self.is_set = false;
        Ok(())
    }

    pub fn restore_with_privilege(&mut self) -> Result<()> {
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
        if original.enable {
            set_macos_all_services_proxy_with_sudo(
                &original.host,
                original.port,
                &original.bypass,
            )?;
        } else {
            disable_macos_all_services_proxy_with_sudo()?;
        }
        self.remove_backup();
        self.is_set = false;
        Ok(())
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
            port: 9900,
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

use std::path::PathBuf;

use crate::{BifrostError, Result};

const BACKUP_FILE_NAME: &str = "shell_proxy_backup.json";

const START_MARKER: &str = "# >>> Bifrost proxy start >>>";
const END_MARKER: &str = "# <<< Bifrost proxy end <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Unknown,
}

impl ShellType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "powershell",
            ShellType::Cmd => "cmd",
            ShellType::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShellProxyBackupFile {
    pub path: String,
    pub original_content: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShellProxyBackup {
    pub shell_type: String,
    #[serde(default)]
    pub files: Vec<ShellProxyBackupFile>,
    #[serde(default)]
    pub config_path: Option<String>,
    #[serde(default)]
    pub original_content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShellProxyStatus {
    pub shell_type: ShellType,
    pub config_paths: Vec<PathBuf>,
    pub has_persistent_config: bool,
    pub http_proxy: Option<String>,
    pub https_proxy: Option<String>,
}

pub struct ShellProxyManager {
    data_dir: PathBuf,
    shell_type: ShellType,
    config_paths: Vec<PathBuf>,
}

impl ShellProxyManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let shell_type = Self::detect_shell();
        let config_paths = Self::get_config_paths(shell_type);
        Self {
            data_dir,
            shell_type,
            config_paths,
        }
    }

    pub fn detect_shell() -> ShellType {
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("zsh") {
                return ShellType::Zsh;
            } else if shell.contains("bash") {
                return ShellType::Bash;
            } else if shell.contains("fish") {
                return ShellType::Fish;
            }
        }

        if std::env::var("PSModulePath").is_ok() || std::env::var("PROFILE").is_ok() {
            return ShellType::PowerShell;
        }

        if cfg!(windows) && std::env::var("COMSPEC").is_ok() {
            return ShellType::Cmd;
        }

        ShellType::Unknown
    }

    fn get_config_paths(shell_type: ShellType) -> Vec<PathBuf> {
        let home = std::env::var("HOME")
            .ok()
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(dirs::home_dir);
        let Some(home) = home else {
            return Vec::new();
        };

        match shell_type {
            ShellType::Bash => vec![home.join(".bashrc"), home.join(".bash_profile")],
            ShellType::Zsh => vec![home.join(".zshrc"), home.join(".zprofile")],
            ShellType::Fish => vec![home.join(".config").join("fish").join("config.fish")],
            ShellType::PowerShell | ShellType::Cmd | ShellType::Unknown => Vec::new(),
        }
    }

    pub fn shell_type(&self) -> ShellType {
        self.shell_type
    }

    pub fn config_paths(&self) -> &[PathBuf] {
        &self.config_paths
    }

    pub fn enable_temporary(host: &str, port: u16, bypass: &str) -> String {
        let shell_type = Self::detect_shell();
        let proxy_url = format!("http://{}:{}", host, port);

        match shell_type {
            ShellType::Bash | ShellType::Zsh => format!(
                r#"export HTTP_PROXY={proxy}
export HTTPS_PROXY={proxy}
export ALL_PROXY={proxy}
export NO_PROXY={bypass}
export http_proxy={proxy}
export https_proxy={proxy}
export all_proxy={proxy}
export no_proxy={bypass}"#,
                proxy = proxy_url,
                bypass = bypass
            ),
            ShellType::Fish => format!(
                r#"set -x HTTP_PROXY {proxy}
set -x HTTPS_PROXY {proxy}
set -x ALL_PROXY {proxy}
set -x NO_PROXY {bypass}
set -x http_proxy {proxy}
set -x https_proxy {proxy}
set -x all_proxy {proxy}
set -x no_proxy {bypass}"#,
                proxy = proxy_url,
                bypass = bypass
            ),
            ShellType::PowerShell => format!(
                r#"$env:HTTP_PROXY = "{proxy}"
$env:HTTPS_PROXY = "{proxy}"
$env:ALL_PROXY = "{proxy}"
$env:NO_PROXY = "{bypass}"
$env:http_proxy = "{proxy}"
$env:https_proxy = "{proxy}"
$env:all_proxy = "{proxy}"
$env:no_proxy = "{bypass}""#,
                proxy = proxy_url,
                bypass = bypass
            ),
            ShellType::Cmd => format!(
                r#"set HTTP_PROXY={proxy}
set HTTPS_PROXY={proxy}
set ALL_PROXY={proxy}
set NO_PROXY={bypass}"#,
                proxy = proxy_url,
                bypass = bypass
            ),
            ShellType::Unknown => format!(
                r#"# Unknown shell type
# Use these environment variables:
HTTP_PROXY={proxy}
HTTPS_PROXY={proxy}
ALL_PROXY={proxy}
NO_PROXY={bypass}"#,
                proxy = proxy_url,
                bypass = bypass
            ),
        }
    }

    pub fn disable_temporary() -> String {
        let shell_type = Self::detect_shell();

        match shell_type {
            ShellType::Bash | ShellType::Zsh => r#"unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY http_proxy https_proxy all_proxy no_proxy"#.to_string(),
            ShellType::Fish => r#"set -e HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY http_proxy https_proxy all_proxy no_proxy"#.to_string(),
            ShellType::PowerShell => r#"Remove-Item Env:\HTTP_PROXY, Env:\HTTPS_PROXY, Env:\ALL_PROXY, Env:\NO_PROXY, Env:\http_proxy, Env:\https_proxy, Env:\all_proxy, Env:\no_proxy -ErrorAction SilentlyContinue"#.to_string(),
            ShellType::Cmd => r#"set HTTP_PROXY=
set HTTPS_PROXY=
set ALL_PROXY=
set NO_PROXY=" "#.to_string(),
            ShellType::Unknown => r#"# Unknown shell type
# Unset these environment variables:
unset HTTP_PROXY HTTPS_PROXY ALL_PROXY NO_PROXY"#.to_string(),
        }
    }

    pub fn status(&self) -> ShellProxyStatus {
        let has_persistent_config = self.config_paths.iter().any(|p| {
            std::fs::read_to_string(p)
                .ok()
                .map(|content| content.contains(START_MARKER))
                .unwrap_or(false)
        });

        let http_proxy = std::env::var("HTTP_PROXY")
            .ok()
            .or_else(|| std::env::var("http_proxy").ok());
        let https_proxy = std::env::var("HTTPS_PROXY")
            .ok()
            .or_else(|| std::env::var("https_proxy").ok());

        ShellProxyStatus {
            shell_type: self.shell_type,
            config_paths: self.config_paths.clone(),
            has_persistent_config,
            http_proxy,
            https_proxy,
        }
    }

    pub fn enable_persistent(&mut self, host: &str, port: u16, bypass: &str) -> Result<()> {
        if self.config_paths.is_empty() {
            return Err(BifrostError::Config(
                "Could not determine shell config file path".to_string(),
            ));
        }

        let proxy_url = format!("http://{}:{}", host, port);
        let config_block = self.generate_config_block(&proxy_url, bypass);

        let mut backups = Vec::new();
        for config_path in &self.config_paths {
            let original_content = if config_path.exists() {
                Some(std::fs::read_to_string(config_path)?)
            } else {
                None
            };
            backups.push(ShellProxyBackupFile {
                path: config_path.to_string_lossy().to_string(),
                original_content,
            });
        }

        self.save_backup(backups)?;

        for config_path in &self.config_paths {
            let content = if config_path.exists() {
                std::fs::read_to_string(config_path)?
            } else {
                String::new()
            };
            let new_content = self.replace_or_add_config_block(&content, &config_block);
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(config_path, new_content)?;
        }

        Ok(())
    }

    pub fn disable_persistent(&mut self) -> Result<()> {
        if self.config_paths.is_empty() {
            return Ok(());
        }

        let mut backups = Vec::new();
        for config_path in &self.config_paths {
            let original_content = if config_path.exists() {
                Some(std::fs::read_to_string(config_path)?)
            } else {
                None
            };
            backups.push(ShellProxyBackupFile {
                path: config_path.to_string_lossy().to_string(),
                original_content,
            });
        }

        self.save_backup(backups)?;

        for config_path in &self.config_paths {
            if !config_path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(config_path)?;
            let new_content = self.remove_config_block(&content);
            std::fs::write(config_path, new_content)?;
        }

        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        let backup = self.normalize_backup(self.load_backup()?);

        for file in backup.files {
            let config_path = PathBuf::from(file.path);
            if let Some(original_content) = file.original_content {
                if let Some(parent) = config_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(config_path, original_content)?;
            } else {
                let _ = std::fs::remove_file(config_path);
            }
        }

        self.remove_backup();

        Ok(())
    }

    pub fn recover_from_crash(data_dir: &PathBuf) -> Result<()> {
        let mut manager = Self::new(data_dir.clone());
        if manager.backup_file_path().exists() {
            manager.restore()?;
        }
        Ok(())
    }

    fn generate_config_block(&self, proxy_url: &str, bypass: &str) -> String {
        match self.shell_type {
            ShellType::Bash | ShellType::Zsh => format!(
                "{}\nexport HTTP_PROXY={}\nexport HTTPS_PROXY={}\nexport ALL_PROXY={}\nexport NO_PROXY={}\nexport http_proxy={}\nexport https_proxy={}\nexport all_proxy={}\nexport no_proxy={}\n{}",
                START_MARKER, proxy_url, proxy_url, proxy_url, bypass, proxy_url, proxy_url, proxy_url, bypass, END_MARKER
            ),
            ShellType::Fish => format!(
                "{}\nset -x HTTP_PROXY {}\nset -x HTTPS_PROXY {}\nset -x ALL_PROXY {}\nset -x NO_PROXY {}\nset -x http_proxy {}\nset -x https_proxy {}\nset -x all_proxy {}\nset -x no_proxy {}\n{}",
                START_MARKER, proxy_url, proxy_url, proxy_url, bypass, proxy_url, proxy_url, proxy_url, bypass, END_MARKER
            ),
            _ => String::new(),
        }
    }

    fn replace_or_add_config_block(&self, content: &str, new_block: &str) -> String {
        let has_start = content.find(START_MARKER);
        let has_end = content.find(END_MARKER);

        match (has_start, has_end) {
            (Some(start), Some(end)) if start < end => {
                let before = &content[..start];
                let after = &content[end + END_MARKER.len()..];
                format!("{}{}{}", before, new_block, after)
            }
            _ => {
                if content.is_empty() {
                    new_block.to_string()
                } else {
                    let trimmed = content.trim_end();
                    format!("{}\n\n{}", trimmed, new_block)
                }
            }
        }
    }

    fn remove_config_block(&self, content: &str) -> String {
        let has_start = content.find(START_MARKER);
        let has_end = content.find(END_MARKER);

        match (has_start, has_end) {
            (Some(start), Some(end)) if start < end => {
                let before = &content[..start];
                let after = &content[end + END_MARKER.len()..];
                format!("{}{}", before.trim_end(), after)
            }
            _ => content.to_string(),
        }
    }

    fn backup_file_path(&self) -> PathBuf {
        self.data_dir.join(BACKUP_FILE_NAME)
    }

    fn save_backup(&self, files: Vec<ShellProxyBackupFile>) -> Result<()> {
        let backup = ShellProxyBackup {
            shell_type: self.shell_type.as_str().to_string(),
            files,
            config_path: None,
            original_content: None,
        };

        let content = serde_json::to_string_pretty(&backup)?;

        if let Some(parent) = self.backup_file_path().parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(self.backup_file_path(), content)?;

        Ok(())
    }

    fn load_backup(&self) -> Result<ShellProxyBackup> {
        let content = std::fs::read_to_string(self.backup_file_path())?;
        let backup: ShellProxyBackup = serde_json::from_str(&content)?;
        Ok(backup)
    }

    fn normalize_backup(&self, mut backup: ShellProxyBackup) -> ShellProxyBackup {
        if backup.files.is_empty() {
            if let Some(config_path) = backup.config_path.take() {
                backup.files.push(ShellProxyBackupFile {
                    path: config_path,
                    original_content: backup.original_content.take(),
                });
            }
        }
        backup
    }

    fn remove_backup(&self) {
        let _ = std::fs::remove_file(self.backup_file_path());
    }
}

use bifrost_core::BifrostError;
use colored::Colorize;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use super::update_check::{
    get_latest_version, get_latest_version_fresh, is_newer_version, VersionCache,
};

const GITHUB_RELEASE_URL: &str = "https://github.com/bifrost-proxy/bifrost/releases/tag";
const GITHUB_DOWNLOAD_URL: &str = "https://github.com/bifrost-proxy/bifrost/releases/download";

#[derive(Debug, Clone, PartialEq)]
pub enum InstallMethod {
    Homebrew,
    Script,
    Manual(PathBuf),
    Unknown,
}

impl std::fmt::Display for InstallMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallMethod::Homebrew => write!(f, "Homebrew"),
            InstallMethod::Script => write!(f, "Install script"),
            InstallMethod::Manual(path) => write!(f, "Manual ({})", path.display()),
            InstallMethod::Unknown => write!(f, "Unknown"),
        }
    }
}

fn detect_install_method() -> InstallMethod {
    let exe_path = match env::current_exe() {
        Ok(path) => path,
        Err(_) => return InstallMethod::Unknown,
    };

    let exe_path_str = exe_path.to_string_lossy();

    if exe_path_str.contains("/opt/homebrew/")
        || exe_path_str.contains("/usr/local/Cellar/")
        || exe_path_str.contains("/home/linuxbrew/")
    {
        return InstallMethod::Homebrew;
    }

    if exe_path_str.contains("/.bifrost/bin/") {
        return InstallMethod::Script;
    }

    InstallMethod::Manual(exe_path)
}

fn get_target_triple() -> Option<&'static str> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some("aarch64-apple-darwin")
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        Some("x86_64-apple-darwin")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        Some("x86_64-unknown-linux-gnu")
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        Some("aarch64-unknown-linux-gnu")
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        Some("x86_64-pc-windows-msvc")
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        Some("aarch64-pc-windows-msvc")
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
    )))]
    {
        None
    }
}

fn prompt_confirm(message: &str) -> bool {
    print!("{} [y/N]: ", message);
    io::stdout().flush().ok();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
}

fn print_update_info(current: &str, cache: &VersionCache) {
    let separator = "─".repeat(64);
    let release_url = format!("{}/v{}", GITHUB_RELEASE_URL, cache.latest_version);

    println!();
    println!("{}", separator.bright_cyan());
    println!("{}", "  📦 New version available!".bright_cyan().bold());
    println!();
    println!("     Current version: {}", current.bright_yellow().bold());
    println!(
        "     Latest version:  {}",
        cache.latest_version.bright_green().bold()
    );

    if !cache.release_highlights.is_empty() {
        println!();
        println!("     {}", "What's new:".bright_white().bold());
        for highlight in &cache.release_highlights {
            let display = if highlight.len() > 50 {
                format!("{}...", &highlight[..47])
            } else {
                highlight.clone()
            };
            println!("       {} {}", "•".bright_cyan(), display.bright_white());
        }
    }

    println!();
    println!(
        "     {} {}",
        "Release notes:".dimmed(),
        release_url.dimmed()
    );
    println!("{}", separator.bright_cyan());
    println!();
}

const HOMEBREW_FORMULA_NAME: &str = "bifrost-proxy/bifrost/bifrost";

fn upgrade_via_homebrew(target_version: &str) -> Result<(), BifrostError> {
    println!("{}", "Refreshing Homebrew tap...".bright_cyan());

    let output = Command::new("brew")
        .args(["--repository", "bifrost-proxy/bifrost"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            if let Ok(tap_path) = String::from_utf8(output.stdout) {
                let tap_path = tap_path.trim();
                if !tap_path.is_empty() {
                    let _ = Command::new("git")
                        .args(["-C", tap_path, "fetch", "--all", "-q"])
                        .status();
                    let _ = Command::new("git")
                        .args(["-C", tap_path, "reset", "--hard", "origin/main", "-q"])
                        .status();
                }
            }
        }
    }

    println!("{}", "Upgrading via Homebrew...".bright_cyan());

    let status = Command::new("brew")
        .args(["reinstall", HOMEBREW_FORMULA_NAME])
        .status();

    let success = match status {
        Ok(s) if s.success() => true,
        _ => {
            println!(
                "{}",
                "Standard install failed, trying --build-from-source...".bright_yellow()
            );
            let fallback_status = Command::new("brew")
                .args(["reinstall", "--build-from-source", HOMEBREW_FORMULA_NAME])
                .status()
                .map_err(BifrostError::Io)?;
            fallback_status.success()
        }
    };

    if !success {
        return Err(BifrostError::Network(format!(
            "Homebrew upgrade failed. Try: brew reinstall {}",
            HOMEBREW_FORMULA_NAME
        )));
    }

    let output = Command::new("brew")
        .args(["info", "--json=v2", HOMEBREW_FORMULA_NAME])
        .output()
        .map_err(BifrostError::Io)?;

    if output.status.success() {
        if let Ok(json_str) = String::from_utf8(output.stdout) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(installed) = json["formulae"]
                    .get(0)
                    .and_then(|f| f["installed"].as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|i| i["version"].as_str())
                {
                    if installed == target_version {
                        println!(
                            "{}",
                            "✓ Upgrade completed successfully!".bright_green().bold()
                        );
                        return Ok(());
                    } else {
                        println!(
                            "{}",
                            format!(
                                "⚠ Installed version ({}) doesn't match target version ({}).",
                                installed, target_version
                            )
                            .bright_yellow()
                        );
                        println!(
                            "{}",
                            "  The Homebrew tap may not be updated yet. Please try again later or install manually:"
                                .bright_yellow()
                        );
                        println!(
                            "  {}",
                            "curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash"
                                .bright_cyan()
                        );
                        return Ok(());
                    }
                }
            }
        }
    }

    println!(
        "{}",
        "✓ Upgrade completed successfully!".bright_green().bold()
    );
    Ok(())
}

fn upgrade_via_script() -> Result<(), BifrostError> {
    println!("{}", "Upgrading via install script...".bright_cyan());

    let status = Command::new("sh")
        .args([
            "-c",
            "curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash",
        ])
        .status()
        .map_err(BifrostError::Io)?;

    if status.success() {
        println!(
            "{}",
            "✓ Upgrade completed successfully!".bright_green().bold()
        );
        Ok(())
    } else {
        Err(BifrostError::Network(
            "Install script failed. Check network connection and try again.".to_string(),
        ))
    }
}

fn upgrade_manual(target_path: &PathBuf, version: &str) -> Result<(), BifrostError> {
    let target = get_target_triple().ok_or_else(|| {
        BifrostError::Config("Unsupported platform for automatic upgrade".to_string())
    })?;

    let archive_ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let archive_name = format!("bifrost-v{}-{}.{}", version, target, archive_ext);
    let download_url = format!("{}/v{}/{}", GITHUB_DOWNLOAD_URL, version, archive_name);

    println!("{} {}", "Downloading:".bright_cyan(), download_url.dimmed());

    let temp_dir = tempfile::tempdir().map_err(|e| BifrostError::Io(std::io::Error::other(e)))?;
    let archive_path = temp_dir.path().join(&archive_name);

    let status = Command::new("curl")
        .args(["-fsSL", "-o", archive_path.to_str().unwrap(), &download_url])
        .status()
        .map_err(BifrostError::Io)?;

    if !status.success() {
        return Err(BifrostError::Network(format!(
            "Failed to download {}. Check if the release exists.",
            archive_name
        )));
    }

    println!("{}", "Extracting archive...".bright_cyan());

    let extract_dir = temp_dir.path().join("extract");
    fs::create_dir_all(&extract_dir)?;

    if cfg!(windows) {
        let status = Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}'",
                    archive_path.display(),
                    extract_dir.display()
                ),
            ])
            .status()
            .map_err(BifrostError::Io)?;

        if !status.success() {
            return Err(BifrostError::Parse("Failed to extract archive".to_string()));
        }
    } else {
        let status = Command::new("tar")
            .args([
                "-xzf",
                archive_path.to_str().unwrap(),
                "-C",
                extract_dir.to_str().unwrap(),
            ])
            .status()
            .map_err(BifrostError::Io)?;

        if !status.success() {
            return Err(BifrostError::Parse("Failed to extract archive".to_string()));
        }
    }

    let binary_name = if cfg!(windows) {
        "bifrost.exe"
    } else {
        "bifrost"
    };
    let extracted_dir = extract_dir.join(format!("bifrost-v{}-{}", version, target));
    let new_binary = extracted_dir.join(binary_name);

    if !new_binary.exists() {
        return Err(BifrostError::NotFound(format!(
            "Binary not found in archive: {}",
            new_binary.display()
        )));
    }

    println!(
        "{} {}",
        "Replacing binary at:".bright_cyan(),
        target_path.display()
    );

    let backup_path = target_path.with_extension("backup");
    if target_path.exists() {
        fs::rename(target_path, &backup_path)?;
    }

    match fs::copy(&new_binary, target_path) {
        Ok(_) => {
            if backup_path.exists() {
                let _ = fs::remove_file(&backup_path);
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(target_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(target_path, perms)?;
            }

            println!(
                "{}",
                "✓ Upgrade completed successfully!".bright_green().bold()
            );
            Ok(())
        }
        Err(e) => {
            if backup_path.exists() {
                let _ = fs::rename(&backup_path, target_path);
            }
            Err(BifrostError::Io(e))
        }
    }
}

pub fn handle_upgrade(force: bool) -> Result<(), BifrostError> {
    let current_version = env!("CARGO_PKG_VERSION");

    println!(
        "{} {}",
        "Checking for updates...".bright_cyan(),
        format!("(current: v{})", current_version).dimmed()
    );

    let cache = if let Some(c) = get_latest_version_fresh() {
        c
    } else if let Some(cached) = get_latest_version() {
        cached
    } else {
        println!(
            "{}",
            "⚠ Could not check for updates. Check your network connection.".bright_yellow()
        );
        return Ok(());
    };

    if !is_newer_version(current_version, &cache.latest_version) {
        println!(
            "{}",
            format!(
                "✓ You're already on the latest version (v{})",
                current_version
            )
            .bright_green()
            .bold()
        );
        return Ok(());
    }

    print_update_info(current_version, &cache);

    let install_method = detect_install_method();
    println!(
        "     {} {}",
        "Install method:".dimmed(),
        format!("{}", install_method).bright_white()
    );
    println!();

    if !force && !prompt_confirm("Do you want to upgrade now?") {
        println!("{}", "Upgrade cancelled.".dimmed());
        return Ok(());
    }

    println!();

    match install_method {
        InstallMethod::Homebrew => upgrade_via_homebrew(&cache.latest_version),
        InstallMethod::Script => upgrade_via_script(),
        InstallMethod::Manual(path) => upgrade_manual(&path, &cache.latest_version),
        InstallMethod::Unknown => {
            println!(
                "{}",
                "⚠ Could not detect installation method.".bright_yellow()
            );
            println!("Please upgrade manually:");
            println!(
                "  {}",
                "curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash"
                    .bright_cyan()
            );
            println!(
                "  Or download from: {}",
                format!("{}/v{}", GITHUB_RELEASE_URL, cache.latest_version).bright_cyan()
            );
            Ok(())
        }
    }
}

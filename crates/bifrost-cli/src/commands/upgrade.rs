use bifrost_core::BifrostError;
use colored::Colorize;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
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
    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "musl"))]
    {
        Some("x86_64-unknown-linux-musl")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64", not(target_env = "musl")))]
    {
        if should_use_musl_fallback() {
            Some("x86_64-unknown-linux-musl")
        } else {
            Some("x86_64-unknown-linux-gnu")
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "musl"))]
    {
        Some("aarch64-unknown-linux-musl")
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64", not(target_env = "musl")))]
    {
        if should_use_musl_fallback() {
            Some("aarch64-unknown-linux-musl")
        } else {
            Some("aarch64-unknown-linux-gnu")
        }
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

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "musl")
))]
const MIN_GLIBC_VERSION: (u32, u32) = (2, 29);

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "musl")
))]
fn should_use_musl_fallback() -> bool {
    if let Some((major, minor)) = detect_glibc_version() {
        return (major, minor) < MIN_GLIBC_VERSION;
    }
    true
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64"),
    not(target_env = "musl")
))]
fn detect_glibc_version() -> Option<(u32, u32)> {
    let output = Command::new("ldd").arg("--version").output().ok()?;

    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);

    if !text.to_lowercase().contains("glibc") && !text.to_lowercase().contains("gnu libc") {
        return None;
    }

    let first_line = text.lines().next()?;
    let version_str = first_line.split_whitespace().rfind(|word| {
        let parts: Vec<&str> = word.split('.').collect();
        parts.len() == 2 && parts[0].parse::<u32>().is_ok() && parts[1].parse::<u32>().is_ok()
    })?;

    let parts: Vec<&str> = version_str.split('.').collect();
    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    Some((major, minor))
}

fn get_musl_fallback_triple(target: &str) -> Option<String> {
    match target {
        "x86_64-unknown-linux-gnu" => Some("x86_64-unknown-linux-musl".to_string()),
        "aarch64-unknown-linux-gnu" => Some("aarch64-unknown-linux-musl".to_string()),
        _ => None,
    }
}

fn verify_binary(_path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        Command::new(_path)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "linux"))]
    {
        true
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
            println!("       {} {}", "•".bright_cyan(), highlight.bright_white());
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

fn download_and_install(
    target: &str,
    version: &str,
    target_path: &PathBuf,
    temp_dir: &tempfile::TempDir,
) -> Result<(), BifrostError> {
    let archive_ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let archive_name = format!("bifrost-v{}-{}.{}", version, target, archive_ext);
    let download_url = format!("{}/v{}/{}", GITHUB_DOWNLOAD_URL, version, archive_name);

    println!("{} {}", "Downloading:".bright_cyan(), download_url.dimmed());

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

    let extract_dir = temp_dir.path().join(format!("extract_{}", target));
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

fn upgrade_manual(target_path: &PathBuf, version: &str) -> Result<(), BifrostError> {
    let target = get_target_triple().ok_or_else(|| {
        BifrostError::Config("Unsupported platform for automatic upgrade".to_string())
    })?;

    let temp_dir = tempfile::tempdir().map_err(|e| BifrostError::Io(std::io::Error::other(e)))?;

    let mut effective_target = target.to_string();

    let install_result = download_and_install(target, version, target_path, &temp_dir);

    let needs_musl_fallback = match &install_result {
        Ok(()) => !verify_binary(target_path),
        Err(_) => true,
    };

    if needs_musl_fallback {
        if let Some(musl_target) = get_musl_fallback_triple(target) {
            let reason = if install_result.is_err() {
                "download/install failed"
            } else {
                "binary failed to run — likely a glibc version mismatch"
            };
            println!(
                "{}",
                format!("⚠ {} binary {}", target, reason).bright_yellow()
            );
            println!(
                "{}",
                format!("  Retrying with musl build: {}", musl_target).bright_cyan()
            );

            download_and_install(&musl_target, version, target_path, &temp_dir)?;

            if !verify_binary(target_path) {
                return Err(BifrostError::Config(
                    "Fallback musl binary also failed to run".to_string(),
                ));
            }

            effective_target = musl_target;
            println!("{}", "✓ musl fallback succeeded".bright_green());
        } else if let Err(e) = install_result {
            return Err(e);
        } else {
            return Err(BifrostError::Config(
                "Installed binary failed to run. Try installing manually with: curl -fsSL https://raw.githubusercontent.com/bifrost-proxy/bifrost/main/install-binary.sh | bash -s -- --libc musl".to_string(),
            ));
        }
    }

    println!(
        "{}",
        format!("✓ Upgrade completed successfully! ({})", effective_target)
            .bright_green()
            .bold()
    );
    Ok(())
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

use bifrost_core::error::{BifrostError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "macos")]
use security_framework::authorization::{
    Authorization, AuthorizationItemSetBuilder, Flags as AuthorizationFlags,
};

#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::mem::size_of;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HWND};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

#[cfg(target_os = "windows")]
fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(target_os = "windows")]
fn install_cert_with_uac(cert_path: &Path) -> bool {
    let verb = to_wide("runas");
    let file = to_wide("certutil");
    let params = to_wide(format!("-addstore Root \"{}\"", cert_path.display()).as_str());
    let mut exec_info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: HWND(std::ptr::null_mut()),
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOW.0,
        ..Default::default()
    };
    let launched = unsafe { ShellExecuteExW(&mut exec_info) }.is_ok();
    if !launched || exec_info.hProcess.is_invalid() {
        return false;
    }
    unsafe {
        WaitForSingleObject(exec_info.hProcess, INFINITE);
    }
    let mut exit_code: u32 = 1;
    let got_exit = unsafe { GetExitCodeProcess(exec_info.hProcess, &mut exit_code) }.is_ok();
    unsafe {
        let _ = CloseHandle(exec_info.hProcess);
    }
    got_exit && exit_code == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertStatus {
    NotInstalled,
    InstalledNotTrusted,
    InstalledAndTrusted,
}

impl std::fmt::Display for CertStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertStatus::NotInstalled => write!(f, "Not installed"),
            CertStatus::InstalledNotTrusted => write!(f, "Installed but not trusted"),
            CertStatus::InstalledAndTrusted => write!(f, "Installed and trusted"),
        }
    }
}

impl CertStatus {
    pub fn is_installed(self) -> bool {
        !matches!(self, Self::NotInstalled)
    }

    pub fn is_trusted(self) -> bool {
        matches!(self, Self::InstalledAndTrusted)
    }
}

#[derive(Debug, Clone)]
pub struct CertSystemInfo {
    pub status: CertStatus,
    pub keychain_location: Option<String>,
    pub system_cert_path: Option<PathBuf>,
    pub fingerprint_match: Option<bool>,
}

pub struct CertInstaller {
    cert_path: std::path::PathBuf,
    #[allow(dead_code)]
    cert_name: String,
}

impl CertInstaller {
    pub fn new<P: AsRef<Path>>(cert_path: P) -> Self {
        Self {
            cert_path: cert_path.as_ref().to_path_buf(),
            cert_name: "Bifrost CA".to_string(),
        }
    }

    pub fn check_status(&self) -> Result<CertStatus> {
        #[cfg(target_os = "macos")]
        return self.check_status_macos();

        #[cfg(target_os = "linux")]
        return self.check_status_linux();

        #[cfg(target_os = "windows")]
        return self.check_status_windows();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Err(BifrostError::Config(
            "Unsupported operating system".to_string(),
        ));
    }

    pub fn get_detailed_status(&self) -> Result<CertSystemInfo> {
        #[cfg(target_os = "macos")]
        return self.get_detailed_status_macos();

        #[cfg(target_os = "linux")]
        return self.get_detailed_status_linux();

        #[cfg(target_os = "windows")]
        return self.get_detailed_status_windows();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Err(BifrostError::Config(
            "Unsupported operating system".to_string(),
        ));
    }

    pub fn install_and_trust(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return self.install_macos();

        #[cfg(target_os = "linux")]
        return self.install_linux();

        #[cfg(target_os = "windows")]
        return self.install_windows();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Err(BifrostError::Config(
            "Unsupported operating system".to_string(),
        ));
    }

    pub fn install_and_trust_gui(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        return self.install_macos_gui();

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        return self.install_and_trust();

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return Err(BifrostError::Config(
            "Unsupported operating system".to_string(),
        ));
    }

    pub fn get_install_instructions(&self) -> String {
        #[cfg(target_os = "macos")]
        return format!(
            "To manually install the certificate on macOS:\n\
             1. Double-click the certificate file: {}\n\
             2. Add it to the 'System' keychain\n\
             3. Open Keychain Access, find 'Bifrost CA'\n\
             4. Double-click it, expand 'Trust', set 'Always Trust'",
            self.cert_path.display()
        );

        #[cfg(target_os = "linux")]
        return format!(
            "To manually install the certificate on Linux:\n\
             1. Copy the certificate:\n\
                sudo cp {} /usr/local/share/ca-certificates/bifrost-ca.crt\n\
             2. Update CA certificates:\n\
                sudo update-ca-certificates",
            self.cert_path.display()
        );

        #[cfg(target_os = "windows")]
        return format!(
            "To manually install the certificate on Windows:\n\
             1. Open Command Prompt as Administrator\n\
             2. Run: certutil -addstore Root \"{}\"",
            self.cert_path.display()
        );

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        return "Manual certificate installation required for your platform.".to_string();
    }

    #[cfg(target_os = "macos")]
    fn check_status_macos(&self) -> Result<CertStatus> {
        let current_fingerprint = self.current_cert_fingerprint_macos()?;
        let system_match = keychain_contains_fingerprint_macos(
            "/Library/Keychains/System.keychain",
            &self.cert_name,
            &current_fingerprint,
        )?;

        if system_match {
            if self.check_trust_macos()? {
                Ok(CertStatus::InstalledAndTrusted)
            } else {
                Ok(CertStatus::InstalledNotTrusted)
            }
        } else {
            Ok(CertStatus::NotInstalled)
        }
    }

    #[cfg(target_os = "macos")]
    fn check_trust_macos(&self) -> Result<bool> {
        if !self.cert_path.exists() {
            return Ok(false);
        }

        let output = Command::new("security")
            .args([
                "verify-cert",
                "-c",
                self.cert_path.to_str().unwrap_or(""),
                "-p",
                "ssl",
            ])
            .output();

        match output {
            Ok(out) => Ok(out.status.success()),
            Err(_) => Ok(false),
        }
    }

    #[cfg(target_os = "macos")]
    fn get_detailed_status_macos(&self) -> Result<CertSystemInfo> {
        let current_fingerprint = self.current_cert_fingerprint_macos()?;
        let system_keychain = "/Library/Keychains/System.keychain";
        let system_fingerprints =
            list_macos_keychain_fingerprints(system_keychain, &self.cert_name)?;
        let system_match = system_fingerprints.contains(&current_fingerprint);
        let trusted = self.check_trust_macos()?;

        if system_match {
            return Ok(CertSystemInfo {
                status: if trusted {
                    CertStatus::InstalledAndTrusted
                } else {
                    CertStatus::InstalledNotTrusted
                },
                keychain_location: Some("System Keychain".to_string()),
                system_cert_path: Some(PathBuf::from(system_keychain)),
                fingerprint_match: Some(true),
            });
        }

        if !system_fingerprints.is_empty() {
            return Ok(CertSystemInfo {
                status: CertStatus::NotInstalled,
                keychain_location: Some("System Keychain".to_string()),
                system_cert_path: Some(PathBuf::from(system_keychain)),
                fingerprint_match: Some(false),
            });
        }

        Ok(CertSystemInfo {
            status: CertStatus::NotInstalled,
            keychain_location: None,
            system_cert_path: None,
            fingerprint_match: None,
        })
    }

    #[cfg(target_os = "macos")]
    fn current_cert_fingerprint_macos(&self) -> Result<String> {
        let output = Command::new("openssl")
            .args([
                "x509",
                "-in",
                self.cert_path.to_str().unwrap_or(""),
                "-noout",
                "-fingerprint",
                "-sha256",
            ])
            .output()
            .map_err(|e| BifrostError::Tls(format!("Failed to execute openssl: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(BifrostError::Tls(format!(
                "openssl fingerprint failed: {}",
                stderr.trim()
            )));
        }

        parse_openssl_sha256_fingerprint(&String::from_utf8_lossy(&output.stdout)).ok_or_else(
            || BifrostError::Tls("Failed to parse current certificate fingerprint".to_string()),
        )
    }

    #[cfg(target_os = "macos")]
    fn install_macos(&self) -> Result<()> {
        if !self.cert_path.exists() {
            return Err(BifrostError::NotFound(format!(
                "Certificate file not found: {}",
                self.cert_path.display()
            )));
        }

        println!("Installing CA certificate to System keychain...");
        println!("This requires administrator privileges.");
        purge_macos_named_certificates(&self.cert_name, &resolve_macos_login_keychain()?, false)?;
        install_macos_cert_to_system_keychain(&self.cert_path)?;
        println!("✓ CA certificate installed and trusted successfully.");
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn install_macos_gui(&self) -> Result<()> {
        if !self.cert_path.exists() {
            return Err(BifrostError::NotFound(format!(
                "Certificate file not found: {}",
                self.cert_path.display()
            )));
        }

        println!("Installing CA certificate to System keychain...");
        println!("macOS will prompt for administrator authorization.");
        purge_macos_named_certificates(&self.cert_name, &resolve_macos_login_keychain()?, false)?;
        run_macos_security_add_trusted_cert_gui(&self.cert_path)?;
        println!("✓ CA certificate installed and trusted successfully.");
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn check_status_linux(&self) -> Result<CertStatus> {
        let system_cert_path = Path::new("/usr/local/share/ca-certificates/bifrost-ca.crt");
        let trusted_cert_link = Path::new("/etc/ssl/certs/bifrost-ca.pem");

        if system_cert_path.exists() {
            if trusted_cert_link.exists() || self.check_cert_in_bundle_linux() {
                Ok(CertStatus::InstalledAndTrusted)
            } else {
                Ok(CertStatus::InstalledNotTrusted)
            }
        } else {
            Ok(CertStatus::NotInstalled)
        }
    }

    #[cfg(target_os = "linux")]
    fn check_cert_in_bundle_linux(&self) -> bool {
        let ca_bundle = Path::new("/etc/ssl/certs/ca-certificates.crt");
        if ca_bundle.exists() {
            if let Ok(content) = std::fs::read_to_string(ca_bundle) {
                return content.contains("Bifrost CA");
            }
        }
        false
    }

    #[cfg(target_os = "linux")]
    fn get_detailed_status_linux(&self) -> Result<CertSystemInfo> {
        let system_cert_path = PathBuf::from("/usr/local/share/ca-certificates/bifrost-ca.crt");
        let trusted_cert_link = Path::new("/etc/ssl/certs/bifrost-ca.pem");

        if system_cert_path.exists() {
            let is_trusted = trusted_cert_link.exists() || self.check_cert_in_bundle_linux();
            Ok(CertSystemInfo {
                status: if is_trusted {
                    CertStatus::InstalledAndTrusted
                } else {
                    CertStatus::InstalledNotTrusted
                },
                keychain_location: Some("System CA Store".to_string()),
                system_cert_path: Some(system_cert_path),
                fingerprint_match: Some(true),
            })
        } else {
            Ok(CertSystemInfo {
                status: CertStatus::NotInstalled,
                keychain_location: None,
                system_cert_path: None,
                fingerprint_match: None,
            })
        }
    }

    #[cfg(target_os = "linux")]
    fn install_linux(&self) -> Result<()> {
        if !self.cert_path.exists() {
            return Err(BifrostError::NotFound(format!(
                "Certificate file not found: {}",
                self.cert_path.display()
            )));
        }

        println!("Installing CA certificate to system trust store...");
        println!("This requires administrator privileges.");

        let copy_status = Command::new("sudo")
            .args([
                "cp",
                self.cert_path.to_str().unwrap_or(""),
                "/usr/local/share/ca-certificates/bifrost-ca.crt",
            ])
            .status();

        match copy_status {
            Ok(status) if status.success() => {}
            Ok(_) => {
                return Err(BifrostError::Tls(
                    "Failed to copy certificate to system directory".to_string(),
                ))
            }
            Err(e) => {
                return Err(BifrostError::Tls(format!(
                    "Failed to execute copy command: {}",
                    e
                )))
            }
        }

        let update_status = Command::new("sudo")
            .args(["update-ca-certificates"])
            .status();

        match update_status {
            Ok(status) => {
                if status.success() {
                    println!("✓ CA certificate installed and trusted successfully.");
                    Ok(())
                } else {
                    Err(BifrostError::Tls(
                        "Failed to update CA certificates".to_string(),
                    ))
                }
            }
            Err(e) => Err(BifrostError::Tls(format!(
                "Failed to execute update-ca-certificates: {}",
                e
            ))),
        }
    }

    #[cfg(target_os = "windows")]
    fn check_status_windows(&self) -> Result<CertStatus> {
        let output = Command::new("certutil")
            .args(["-store", "Root", &self.cert_name])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if out.status.success() && stdout.contains(&self.cert_name) {
                    Ok(CertStatus::InstalledAndTrusted)
                } else {
                    Ok(CertStatus::NotInstalled)
                }
            }
            Err(_) => Ok(CertStatus::NotInstalled),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_detailed_status_windows(&self) -> Result<CertSystemInfo> {
        let output = Command::new("certutil")
            .args(["-store", "Root", &self.cert_name])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if out.status.success() && stdout.contains(&self.cert_name) {
                    Ok(CertSystemInfo {
                        status: CertStatus::InstalledAndTrusted,
                        keychain_location: Some("Root Certificate Store".to_string()),
                        system_cert_path: None,
                        fingerprint_match: Some(true),
                    })
                } else {
                    Ok(CertSystemInfo {
                        status: CertStatus::NotInstalled,
                        keychain_location: None,
                        system_cert_path: None,
                        fingerprint_match: None,
                    })
                }
            }
            Err(_) => Ok(CertSystemInfo {
                status: CertStatus::NotInstalled,
                keychain_location: None,
                system_cert_path: None,
                fingerprint_match: None,
            }),
        }
    }

    #[cfg(target_os = "windows")]
    fn install_windows(&self) -> Result<()> {
        if !self.cert_path.exists() {
            return Err(BifrostError::NotFound(format!(
                "Certificate file not found: {}",
                self.cert_path.display()
            )));
        }

        println!("Installing CA certificate to Windows certificate store...");
        println!("This requires administrator privileges.");
        println!("A UAC prompt may appear.");

        let cert_path = self.cert_path.to_str().unwrap_or("");
        let output = Command::new("certutil")
            .args(["-addstore", "Root", cert_path])
            .status();

        match output {
            Ok(status) => {
                if status.success() {
                    println!("✓ CA certificate installed and trusted successfully.");
                    Ok(())
                } else {
                    println!("Requesting administrator approval to install the certificate...");
                    if install_cert_with_uac(&self.cert_path) {
                        println!("✓ CA certificate installed and trusted successfully.");
                        Ok(())
                    } else {
                        println!();
                        println!(
                            "Failed to install certificate. Please try running as Administrator:"
                        );
                        println!("  certutil -addstore Root \"{}\"", self.cert_path.display());
                        Err(BifrostError::Tls(
                            "Failed to install CA certificate. Administrator privileges required."
                                .to_string(),
                        ))
                    }
                }
            }
            Err(e) => Err(BifrostError::Tls(format!(
                "Failed to execute certutil: {}",
                e
            ))),
        }
    }
}

#[cfg(target_os = "macos")]
fn install_macos_cert_to_system_keychain(cert_path: &Path) -> Result<()> {
    run_macos_security_add_trusted_cert(
        cert_path,
        Path::new("/Library/Keychains/System.keychain"),
        true,
    )
}

#[cfg(target_os = "macos")]
fn resolve_macos_login_keychain() -> Result<PathBuf> {
    if let Ok(output) = Command::new("security")
        .args(["default-keychain", "-d", "user"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let trimmed = stdout.trim().trim_matches('"');
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed));
            }
        }
    }

    let home = std::env::var("HOME").map_err(|e| {
        BifrostError::Tls(format!("Failed to resolve HOME for login keychain: {}", e))
    })?;
    Ok(PathBuf::from(home).join("Library/Keychains/login.keychain-db"))
}

#[cfg(target_os = "macos")]
fn run_macos_security_add_trusted_cert(
    cert_path: &Path,
    keychain: &Path,
    use_sudo: bool,
) -> Result<()> {
    let cert_path = cert_path.to_str().ok_or_else(|| {
        BifrostError::Tls(format!(
            "Certificate path is not valid UTF-8: {}",
            cert_path.display()
        ))
    })?;
    let keychain = keychain.to_str().ok_or_else(|| {
        BifrostError::Tls(format!(
            "Keychain path is not valid UTF-8: {}",
            keychain.display()
        ))
    })?;

    let mut command = if use_sudo {
        let mut cmd = Command::new("sudo");
        cmd.arg("security");
        cmd
    } else {
        Command::new("security")
    };
    let output = command
        .args([
            "add-trusted-cert",
            "-d",
            "-r",
            "trustRoot",
            "-k",
            keychain,
            cert_path,
        ])
        .output()
        .map_err(|e| BifrostError::Tls(format!("Failed to execute security command: {}", e)))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(BifrostError::Tls(format!(
        "security add-trusted-cert failed for {}: {} {}",
        keychain,
        stdout.trim(),
        stderr.trim()
    )))
}

#[cfg(target_os = "macos")]
fn run_macos_security_add_trusted_cert_gui(cert_path: &Path) -> Result<()> {
    let cert_path = cert_path.to_str().ok_or_else(|| {
        BifrostError::Tls(format!(
            "Certificate path is not valid UTF-8: {}",
            cert_path.display()
        ))
    })?;
    run_macos_security_with_authorization(&[
        "add-trusted-cert",
        "-d",
        "-r",
        "trustRoot",
        "-k",
        "/Library/Keychains/System.keychain",
        cert_path,
    ])
}

#[cfg(target_os = "macos")]
fn list_macos_keychain_fingerprints(keychain: &str, cert_name: &str) -> Result<Vec<String>> {
    let output = Command::new("security")
        .args(["find-certificate", "-c", cert_name, "-a", "-Z", keychain])
        .output()
        .map_err(|e| {
            BifrostError::Tls(format!(
                "Failed to execute security find-certificate: {}",
                e
            ))
        })?;

    if !output.status.success() || output.stdout.is_empty() {
        return Ok(Vec::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_security_sha256_fingerprint)
        .collect())
}

#[cfg(target_os = "macos")]
fn keychain_contains_fingerprint_macos(
    keychain: &str,
    cert_name: &str,
    fingerprint: &str,
) -> Result<bool> {
    Ok(list_macos_keychain_fingerprints(keychain, cert_name)?
        .iter()
        .any(|candidate| candidate == fingerprint))
}

#[cfg(target_os = "macos")]
fn purge_macos_named_certificates(cert_name: &str, keychain: &Path, use_sudo: bool) -> Result<()> {
    let keychain = keychain.to_str().ok_or_else(|| {
        BifrostError::Tls(format!(
            "Keychain path is not valid UTF-8: {}",
            keychain.display()
        ))
    })?;

    let mut command = if use_sudo {
        let mut cmd = Command::new("sudo");
        cmd.arg("security");
        cmd
    } else {
        Command::new("security")
    };

    let output = command
        .args(["delete-certificate", "-c", cert_name, keychain])
        .output()
        .map_err(|e| {
            BifrostError::Tls(format!(
                "Failed to execute security delete-certificate: {}",
                e
            ))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_macos_delete_certificate_not_found(&stderr) {
        return Ok(());
    }

    Err(BifrostError::Tls(format!(
        "security delete-certificate failed for {}: {}",
        keychain,
        stderr.trim()
    )))
}

#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn run_macos_security_with_authorization(args: &[&str]) -> Result<()> {
    let rights = AuthorizationItemSetBuilder::new()
        .add_right("system.privilege.admin")
        .map_err(|error| BifrostError::Tls(format!("Failed to build auth rights: {error}")))?
        .build();
    let auth = Authorization::new(
        Some(rights),
        None,
        AuthorizationFlags::INTERACTION_ALLOWED
            | AuthorizationFlags::EXTEND_RIGHTS
            | AuthorizationFlags::PREAUTHORIZE,
    )
    .map_err(|error| {
        map_macos_authorization_error("Failed to request macOS authorization", &error.to_string())
    })?;

    auth.execute_with_privileges(
        "/usr/bin/security",
        args.iter().copied(),
        AuthorizationFlags::DEFAULTS,
    )
    .map_err(|error| {
        map_macos_authorization_error("security authorization command failed", &error.to_string())
    })
}

fn is_macos_delete_certificate_not_found(message: &str) -> bool {
    message.contains("could not find item")
        || message.contains("The specified item could not be found")
        || message.contains("Unable to delete certificate matching")
}

#[cfg(target_os = "macos")]
fn map_macos_authorization_error(context: &str, message: &str) -> BifrostError {
    if is_macos_user_cancelled(message) {
        return BifrostError::Tls(format!("UserCancelled: {message}"));
    }

    BifrostError::Tls(format!("{context}: {message}"))
}

#[cfg(target_os = "macos")]
fn is_macos_user_cancelled(message: &str) -> bool {
    let lowercase = message.to_ascii_lowercase();
    lowercase.contains("user canceled")
        || lowercase.contains("user cancelled")
        || lowercase.contains("canceled by the user")
        || lowercase.contains("cancelled by the user")
        || lowercase.contains("errauthorizationcanceled")
}

fn parse_security_sha256_fingerprint(line: &str) -> Option<String> {
    line.strip_prefix("SHA-256 hash:")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn parse_openssl_sha256_fingerprint(output: &str) -> Option<String> {
    output
        .trim()
        .strip_prefix("SHA256 Fingerprint=")
        .map(|value| value.replace(':', ""))
}

pub fn get_platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    return "macOS";

    #[cfg(target_os = "linux")]
    return "Linux";

    #[cfg(target_os = "windows")]
    return "Windows";

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return "Unknown";
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cert_status_display() {
        assert_eq!(CertStatus::NotInstalled.to_string(), "Not installed");
        assert_eq!(
            CertStatus::InstalledNotTrusted.to_string(),
            "Installed but not trusted"
        );
        assert_eq!(
            CertStatus::InstalledAndTrusted.to_string(),
            "Installed and trusted"
        );
    }

    #[test]
    fn test_cert_status_helpers() {
        assert!(!CertStatus::NotInstalled.is_installed());
        assert!(!CertStatus::NotInstalled.is_trusted());

        assert!(CertStatus::InstalledNotTrusted.is_installed());
        assert!(!CertStatus::InstalledNotTrusted.is_trusted());

        assert!(CertStatus::InstalledAndTrusted.is_installed());
        assert!(CertStatus::InstalledAndTrusted.is_trusted());
    }

    #[test]
    fn test_cert_installer_new() {
        let dir = tempdir().expect("Failed to create temp dir");
        let cert_path = dir.path().join("test.crt");
        let installer = CertInstaller::new(&cert_path);
        assert_eq!(installer.cert_path, cert_path);
        assert_eq!(installer.cert_name, "Bifrost CA");
    }

    #[test]
    fn test_get_install_instructions() {
        let dir = tempdir().expect("Failed to create temp dir");
        let cert_path = dir.path().join("test.crt");
        let installer = CertInstaller::new(&cert_path);
        let instructions = installer.get_install_instructions();
        assert!(!instructions.is_empty());
    }

    #[test]
    fn test_get_platform_name() {
        let name = get_platform_name();
        assert!(!name.is_empty());
        #[cfg(target_os = "macos")]
        assert_eq!(name, "macOS");
        #[cfg(target_os = "linux")]
        assert_eq!(name, "Linux");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "Windows");
    }

    #[test]
    fn test_is_macos_delete_certificate_not_found() {
        assert!(is_macos_delete_certificate_not_found(
            "Unable to delete certificate matching \"Bifrost CA\""
        ));
        assert!(is_macos_delete_certificate_not_found(
            "The specified item could not be found in the keychain."
        ));
        assert!(!is_macos_delete_certificate_not_found("some other failure"));
    }

    #[test]
    fn test_is_macos_user_cancelled() {
        assert!(is_macos_user_cancelled("User canceled."));
        assert!(is_macos_user_cancelled("errAuthorizationCanceled"));
        assert!(!is_macos_user_cancelled("authorization denied"));
    }

    #[test]
    fn test_parse_security_sha256_fingerprint() {
        assert_eq!(
            parse_security_sha256_fingerprint("SHA-256 hash: ABCDEF1234"),
            Some("ABCDEF1234".to_string())
        );
    }

    #[test]
    fn test_parse_openssl_sha256_fingerprint() {
        assert_eq!(
            parse_openssl_sha256_fingerprint("SHA256 Fingerprint=AA:BB:CC:DD:EE:FF\n"),
            Some("AABBCCDDEEFF".to_string())
        );
    }
}

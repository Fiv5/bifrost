use bifrost_core::error::{BifrostError, Result};
use std::path::Path;
use std::process::Command;

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

pub struct CertInstaller {
    cert_path: std::path::PathBuf,
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
        let output = Command::new("security")
            .args([
                "find-certificate",
                "-c",
                &self.cert_name,
                "-a",
                "-Z",
                "/Library/Keychains/System.keychain",
            ])
            .output();

        match output {
            Ok(out) => {
                if out.status.success() && !out.stdout.is_empty() {
                    if self.check_trust_macos()? {
                        Ok(CertStatus::InstalledAndTrusted)
                    } else {
                        Ok(CertStatus::InstalledNotTrusted)
                    }
                } else {
                    let user_keychain_output = Command::new("security")
                        .args(["find-certificate", "-c", &self.cert_name, "-a", "-Z"])
                        .output();

                    match user_keychain_output {
                        Ok(user_out) => {
                            if user_out.status.success() && !user_out.stdout.is_empty() {
                                if self.check_trust_macos()? {
                                    Ok(CertStatus::InstalledAndTrusted)
                                } else {
                                    Ok(CertStatus::InstalledNotTrusted)
                                }
                            } else {
                                Ok(CertStatus::NotInstalled)
                            }
                        }
                        Err(_) => Ok(CertStatus::NotInstalled),
                    }
                }
            }
            Err(_) => Ok(CertStatus::NotInstalled),
        }
    }

    #[cfg(target_os = "macos")]
    fn check_trust_macos(&self) -> Result<bool> {
        if !self.cert_path.exists() {
            return Ok(false);
        }

        let output = Command::new("security")
            .args(["verify-cert", "-c", self.cert_path.to_str().unwrap_or("")])
            .output();

        match output {
            Ok(out) => Ok(out.status.success()),
            Err(_) => Ok(false),
        }
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

        let output = Command::new("sudo")
            .args([
                "security",
                "add-trusted-cert",
                "-d",
                "-r",
                "trustRoot",
                "-k",
                "/Library/Keychains/System.keychain",
                self.cert_path.to_str().unwrap_or(""),
            ])
            .status();

        match output {
            Ok(status) => {
                if status.success() {
                    println!("✓ CA certificate installed and trusted successfully.");
                    Ok(())
                } else {
                    Err(BifrostError::Tls(
                        "Failed to install CA certificate. You may need to install it manually."
                            .to_string(),
                    ))
                }
            }
            Err(e) => Err(BifrostError::Tls(format!(
                "Failed to execute security command: {}",
                e
            ))),
        }
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

        let output = Command::new("certutil")
            .args(["-addstore", "Root", self.cert_path.to_str().unwrap_or("")])
            .status();

        match output {
            Ok(status) => {
                if status.success() {
                    println!("✓ CA certificate installed and trusted successfully.");
                    Ok(())
                } else {
                    println!();
                    println!("Failed to install certificate. Please try running as Administrator:");
                    println!("  certutil -addstore Root \"{}\"", self.cert_path.display());
                    Err(BifrostError::Tls(
                        "Failed to install CA certificate. Administrator privileges required."
                            .to_string(),
                    ))
                }
            }
            Err(e) => Err(BifrostError::Tls(format!(
                "Failed to execute certutil: {}",
                e
            ))),
        }
    }
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
}

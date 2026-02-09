use bifrost_core::error::{BifrostError, Result};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::fs;
use std::path::Path;

pub struct CertificateAuthority {
    pub certificate: Certificate,
}

impl std::fmt::Debug for CertificateAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertificateAuthority")
            .field("certificate", &"<Certificate>")
            .finish()
    }
}

impl CertificateAuthority {
    pub fn new(certificate: Certificate) -> Self {
        Self { certificate }
    }

    pub fn certificate_der(&self) -> Result<CertificateDer<'static>> {
        let der = self
            .certificate
            .serialize_der()
            .map_err(|e| BifrostError::Tls(format!("Failed to serialize certificate: {e}")))?;
        Ok(CertificateDer::from(der))
    }

    pub fn private_key_der(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            self.certificate.serialize_private_key_der(),
        ))
    }

    pub fn key_pair(&self) -> &KeyPair {
        self.certificate.get_key_pair()
    }
}

pub fn generate_root_ca() -> Result<CertificateAuthority> {
    let mut params = CertificateParams::default();
    params
        .distinguished_name
        .push(DnType::CommonName, "Bifrost CA");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Bifrost Proxy");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    params.extended_key_usages = vec![
        ExtendedKeyUsagePurpose::ServerAuth,
        ExtendedKeyUsagePurpose::ClientAuth,
    ];
    params.alg = &PKCS_ECDSA_P256_SHA256;

    let cert = Certificate::from_params(params)
        .map_err(|e| BifrostError::Tls(format!("Failed to generate root certificate: {e}")))?;

    Ok(CertificateAuthority::new(cert))
}

pub fn load_root_ca(cert_path: &Path, key_path: &Path) -> Result<CertificateAuthority> {
    let cert_pem = fs::read_to_string(cert_path)?;
    let key_pem = fs::read_to_string(key_path)?;

    let key_pair = KeyPair::from_pem(&key_pem)
        .map_err(|e| BifrostError::Tls(format!("Failed to parse CA key: {e}")))?;

    let params = CertificateParams::from_ca_cert_pem(&cert_pem, key_pair)
        .map_err(|e| BifrostError::Tls(format!("Failed to parse CA certificate: {e}")))?;

    let cert = Certificate::from_params(params)
        .map_err(|e| BifrostError::Tls(format!("Failed to reconstruct CA certificate: {e}")))?;

    Ok(CertificateAuthority::new(cert))
}

pub fn save_root_ca(cert_path: &Path, key_path: &Path, ca: &CertificateAuthority) -> Result<()> {
    let cert_pem = ca
        .certificate
        .serialize_pem()
        .map_err(|e| BifrostError::Tls(format!("Failed to serialize certificate: {e}")))?;
    let key_pem = ca.certificate.serialize_private_key_pem();

    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(cert_path, cert_pem)?;
    fs::write(key_path, key_pem)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_root_ca() {
        let ca = generate_root_ca().expect("Failed to generate root CA");
        let cert_pem = ca.certificate.serialize_pem().expect("Failed to serialize");
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(cert_pem.contains("END CERTIFICATE"));
    }

    #[test]
    fn test_save_and_load_root_ca() {
        let dir = tempdir().expect("Failed to create temp dir");
        let cert_path = dir.path().join("ca.crt");
        let key_path = dir.path().join("ca.key");

        let ca = generate_root_ca().expect("Failed to generate root CA");
        save_root_ca(&cert_path, &key_path, &ca).expect("Failed to save root CA");

        assert!(cert_path.exists());
        assert!(key_path.exists());

        let _loaded_ca = load_root_ca(&cert_path, &key_path).expect("Failed to load root CA");
    }

    #[test]
    fn test_certificate_der() {
        let ca = generate_root_ca().expect("Failed to generate root CA");
        let der = ca.certificate_der().expect("Failed to get DER");
        assert!(!der.is_empty());
    }
}

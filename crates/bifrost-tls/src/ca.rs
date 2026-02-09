use bifrost_core::error::{BifrostError, Result};
use chrono::{DateTime, Utc};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::fs;
use std::path::Path;
use x509_parser::prelude::*;
use x509_parser::public_key::PublicKey;

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

#[derive(Debug, Clone)]
pub struct CertInfo {
    pub subject: String,
    pub issuer: String,
    pub serial_number: String,
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub signature_algorithm: String,
    pub fingerprint_sha256: String,
    pub key_type: String,
    pub key_size: Option<usize>,
    pub is_ca: bool,
    pub key_usages: Vec<String>,
    pub extended_key_usages: Vec<String>,
}

impl CertInfo {
    pub fn days_remaining(&self) -> i64 {
        let now = Utc::now();
        (self.not_after - now).num_days()
    }

    pub fn is_expired(&self) -> bool {
        Utc::now() > self.not_after
    }

    pub fn is_not_yet_valid(&self) -> bool {
        Utc::now() < self.not_before
    }
}

pub fn parse_cert_info(cert_path: &Path) -> Result<CertInfo> {
    let pem_data = fs::read_to_string(cert_path)?;
    let pem = parse_x509_pem(pem_data.as_bytes())
        .map_err(|e| BifrostError::Tls(format!("Failed to parse PEM: {e}")))?
        .1;

    let cert = pem
        .parse_x509()
        .map_err(|e| BifrostError::Tls(format!("Failed to parse X.509 certificate: {e}")))?;

    let subject = cert.subject().to_string();
    let issuer = cert.issuer().to_string();

    let serial_bytes = cert.serial.to_bytes_be();
    let serial_number = serial_bytes
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(":");

    let not_before = DateTime::from_timestamp(cert.validity().not_before.timestamp(), 0)
        .unwrap_or(DateTime::UNIX_EPOCH);
    let not_after = DateTime::from_timestamp(cert.validity().not_after.timestamp(), 0)
        .unwrap_or(DateTime::UNIX_EPOCH);

    let signature_algorithm = oid_to_algorithm_name(&cert.signature_algorithm.algorithm);

    let fingerprint_sha256 = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(cert.as_ref());
        let result = hasher.finalize();
        result
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(":")
    };

    let algo_oid = cert.public_key().algorithm.algorithm.to_id_string();
    let (key_type, key_size) = match cert.public_key().parsed() {
        Ok(PublicKey::RSA(rsa)) => {
            let bits = rsa.key_size();
            ("RSA".to_string(), Some(bits))
        }
        Ok(PublicKey::EC(_)) => {
            let curve_name =
                if algo_oid.contains("prime256v1") || algo_oid.contains("1.2.840.10045.3.1.7") {
                    "P-256"
                } else if algo_oid.contains("secp384r1") || algo_oid.contains("1.3.132.0.34") {
                    "P-384"
                } else if algo_oid.contains("secp521r1") || algo_oid.contains("1.3.132.0.35") {
                    "P-521"
                } else {
                    "EC"
                };
            (format!("ECDSA {}", curve_name), None)
        }
        Ok(PublicKey::DSA(_)) => ("DSA".to_string(), None),
        Ok(PublicKey::GostR3410(_)) => ("GOST R 34.10".to_string(), None),
        Ok(PublicKey::GostR3410_2012(_)) => ("GOST R 34.10-2012".to_string(), None),
        Ok(PublicKey::Unknown(_)) => ("Unknown".to_string(), None),
        Err(_) => ("Unknown".to_string(), None),
    };

    let is_ca = cert.is_ca();

    let key_usages = cert
        .key_usage()
        .ok()
        .flatten()
        .map(|ku| {
            let mut usages = Vec::new();
            let ku_value = ku.value;
            if ku_value.digital_signature() {
                usages.push("Digital Signature".to_string());
            }
            if ku_value.non_repudiation() {
                usages.push("Non Repudiation".to_string());
            }
            if ku_value.key_encipherment() {
                usages.push("Key Encipherment".to_string());
            }
            if ku_value.data_encipherment() {
                usages.push("Data Encipherment".to_string());
            }
            if ku_value.key_agreement() {
                usages.push("Key Agreement".to_string());
            }
            if ku_value.key_cert_sign() {
                usages.push("Certificate Sign".to_string());
            }
            if ku_value.crl_sign() {
                usages.push("CRL Sign".to_string());
            }
            if ku_value.encipher_only() {
                usages.push("Encipher Only".to_string());
            }
            if ku_value.decipher_only() {
                usages.push("Decipher Only".to_string());
            }
            usages
        })
        .unwrap_or_default();

    let extended_key_usages = cert
        .extended_key_usage()
        .ok()
        .flatten()
        .map(|eku| {
            eku.value
                .other
                .iter()
                .map(|oid| oid_to_eku_name(oid))
                .chain(
                    [
                        eku.value.any.then_some("Any"),
                        eku.value.server_auth.then_some("Server Auth"),
                        eku.value.client_auth.then_some("Client Auth"),
                        eku.value.code_signing.then_some("Code Signing"),
                        eku.value.email_protection.then_some("Email Protection"),
                        eku.value.time_stamping.then_some("Time Stamping"),
                        eku.value.ocsp_signing.then_some("OCSP Signing"),
                    ]
                    .into_iter()
                    .flatten()
                    .map(String::from),
                )
                .collect()
        })
        .unwrap_or_default();

    Ok(CertInfo {
        subject,
        issuer,
        serial_number,
        not_before,
        not_after,
        signature_algorithm,
        fingerprint_sha256,
        key_type,
        key_size,
        is_ca,
        key_usages,
        extended_key_usages,
    })
}

fn oid_to_eku_name(oid: &x509_parser::oid_registry::Oid) -> String {
    oid.to_id_string()
}

fn oid_to_algorithm_name(oid: &x509_parser::oid_registry::Oid) -> String {
    let oid_str = oid.to_id_string();
    match oid_str.as_str() {
        "1.2.840.113549.1.1.5" => "SHA-1 with RSA".to_string(),
        "1.2.840.113549.1.1.11" => "SHA-256 with RSA".to_string(),
        "1.2.840.113549.1.1.12" => "SHA-384 with RSA".to_string(),
        "1.2.840.113549.1.1.13" => "SHA-512 with RSA".to_string(),
        "1.2.840.10045.4.3.2" => "ECDSA with SHA-256".to_string(),
        "1.2.840.10045.4.3.3" => "ECDSA with SHA-384".to_string(),
        "1.2.840.10045.4.3.4" => "ECDSA with SHA-512".to_string(),
        "1.3.101.112" => "Ed25519".to_string(),
        "1.3.101.113" => "Ed448".to_string(),
        _ => oid_str,
    }
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

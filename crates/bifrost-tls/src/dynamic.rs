use crate::ca::CertificateAuthority;
use bifrost_core::error::{BifrostError, Result};
use rcgen::{
    CertificateParams, DnType, ExtendedKeyUsagePurpose, Issuer, KeyPair, KeyUsagePurpose, SanType,
    PKCS_ECDSA_P256_SHA256,
};
use rustls::crypto::ring::sign::any_supported_type;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::sign::CertifiedKey;
use std::sync::Arc;

#[derive(Debug)]
pub struct DynamicCertGenerator {
    ca: Arc<CertificateAuthority>,
}

impl DynamicCertGenerator {
    pub fn new(ca: Arc<CertificateAuthority>) -> Self {
        Self { ca }
    }

    pub fn generate_for_domain(&self, domain: &str) -> Result<CertifiedKey> {
        let mut params = CertificateParams::default();
        params.distinguished_name.push(DnType::CommonName, domain);
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Bifrost Proxy");

        if domain.parse::<std::net::IpAddr>().is_ok() {
            params.subject_alt_names =
                vec![SanType::IpAddress(domain.parse().map_err(|e| {
                    BifrostError::Tls(format!("Invalid IP address: {e}"))
                })?)];
        } else {
            params.subject_alt_names =
                vec![SanType::DnsName(domain.to_string().try_into().map_err(
                    |e| BifrostError::Tls(format!("Invalid DNS name: {e}")),
                )?)];
        }

        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ServerAuth,
            ExtendedKeyUsagePurpose::ClientAuth,
        ];

        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| BifrostError::Tls(format!("Failed to generate key pair: {e}")))?;

        let ca_pem = self.ca.certificate.pem();
        let issuer = Issuer::from_ca_cert_pem(&ca_pem, self.ca.key_pair())
            .map_err(|e| BifrostError::Tls(format!("Failed to create issuer: {e}")))?;

        let cert = params
            .signed_by(&key_pair, &issuer)
            .map_err(|e| BifrostError::Tls(format!("Failed to sign certificate: {e}")))?;

        let cert_der = CertificateDer::from(cert.der().to_vec());
        let ca_cert_der = self.ca.certificate_der()?;

        let key_der: PrivateKeyDer<'static> =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));

        let signing_key = any_supported_type(&key_der)
            .map_err(|e| BifrostError::Tls(format!("Failed to create signing key: {e}")))?;

        let cert_chain = vec![cert_der, ca_cert_der];

        Ok(CertifiedKey::new(cert_chain, signing_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::generate_root_ca;
    use crate::init_crypto_provider;

    #[test]
    fn test_generate_for_domain() {
        init_crypto_provider();
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        let generator = DynamicCertGenerator::new(ca);

        let cert_key = generator
            .generate_for_domain("example.com")
            .expect("Failed to generate certificate");
        assert_eq!(cert_key.cert.len(), 2);
    }

    #[test]
    fn test_generate_for_wildcard_domain() {
        init_crypto_provider();
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        let generator = DynamicCertGenerator::new(ca);

        let cert_key = generator
            .generate_for_domain("*.example.com")
            .expect("Failed to generate wildcard certificate");
        assert_eq!(cert_key.cert.len(), 2);
    }

    #[test]
    fn test_generate_for_ip() {
        init_crypto_provider();
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        let generator = DynamicCertGenerator::new(ca);

        let cert_key = generator
            .generate_for_domain("127.0.0.1")
            .expect("Failed to generate certificate for IP");
        assert_eq!(cert_key.cert.len(), 2);
    }

    #[test]
    fn test_generate_for_subdomain() {
        init_crypto_provider();
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        let generator = DynamicCertGenerator::new(ca);

        let cert_key = generator
            .generate_for_domain("api.sub.example.com")
            .expect("Failed to generate subdomain certificate");
        assert_eq!(cert_key.cert.len(), 2);
    }
}

use crate::cache::CertCache;
use crate::ca::CertificateAuthority;
use crate::dynamic::DynamicCertGenerator;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::ServerConfig;
use std::sync::Arc;
use bifrost_core::error::Result;

#[derive(Debug)]
pub struct SniResolver {
    cert_generator: DynamicCertGenerator,
    cert_cache: CertCache,
}

impl SniResolver {
    pub fn new(ca: Arc<CertificateAuthority>) -> Self {
        Self {
            cert_generator: DynamicCertGenerator::new(ca),
            cert_cache: CertCache::new(),
        }
    }

    pub fn with_cache_capacity(ca: Arc<CertificateAuthority>, capacity: usize) -> Self {
        Self {
            cert_generator: DynamicCertGenerator::new(ca),
            cert_cache: CertCache::with_capacity(capacity),
        }
    }

    pub fn resolve(&self, server_name: &str) -> Result<Arc<CertifiedKey>> {
        if let Some(cert) = self.cert_cache.get(server_name) {
            return Ok(cert);
        }

        let cert_key = self.cert_generator.generate_for_domain(server_name)?;
        let cert = Arc::new(cert_key);

        self.cert_cache.insert(server_name, cert.clone());

        Ok(cert)
    }

    pub fn resolve_server_config(&self, server_name: &str) -> Result<Arc<ServerConfig>> {
        let cert_key = self.resolve(server_name)?;

        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(SingleCertResolver {
                cert_key: (*cert_key).clone(),
            }));

        Ok(Arc::new(config))
    }

    pub fn clear_cache(&self) {
        self.cert_cache.clear();
    }

    pub fn cache_len(&self) -> usize {
        self.cert_cache.len()
    }
}

impl ResolvesServerCert for SniResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let server_name = client_hello.server_name()?;

        match self.cert_cache.get(server_name) {
            Some(cert) => Some(cert),
            None => {
                let cert_key = self.cert_generator.generate_for_domain(server_name).ok()?;
                let cert = Arc::new(cert_key);
                self.cert_cache.insert(server_name, cert.clone());
                Some(cert)
            }
        }
    }
}

#[derive(Debug)]
struct SingleCertResolver {
    cert_key: CertifiedKey,
}

impl ResolvesServerCert for SingleCertResolver {
    fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(Arc::new(self.cert_key.clone()))
    }
}

pub fn build_sni_server_config(resolver: Arc<SniResolver>) -> Arc<ServerConfig> {
    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(resolver);

    Arc::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::generate_root_ca;

    fn setup_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    fn create_test_resolver() -> SniResolver {
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        SniResolver::new(ca)
    }

    #[test]
    fn test_sni_resolver_new() {
        let resolver = create_test_resolver();
        assert_eq!(resolver.cache_len(), 0);
    }

    #[test]
    fn test_sni_resolver_with_capacity() {
        let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
        let resolver = SniResolver::with_cache_capacity(ca, 500);
        assert_eq!(resolver.cert_cache.capacity(), 500);
    }

    #[test]
    fn test_resolve_domain() {
        let resolver = create_test_resolver();

        let cert = resolver.resolve("example.com").expect("Failed to resolve");
        assert_eq!(cert.cert.len(), 2);
        assert_eq!(resolver.cache_len(), 1);
    }

    #[test]
    fn test_resolve_cached() {
        let resolver = create_test_resolver();

        let cert1 = resolver.resolve("example.com").expect("First resolve");
        let cert2 = resolver.resolve("example.com").expect("Second resolve");

        assert!(Arc::ptr_eq(&cert1, &cert2));
        assert_eq!(resolver.cache_len(), 1);
    }

    #[test]
    fn test_resolve_multiple_domains() {
        let resolver = create_test_resolver();

        resolver.resolve("example1.com").expect("First domain");
        resolver.resolve("example2.com").expect("Second domain");
        resolver.resolve("example3.com").expect("Third domain");

        assert_eq!(resolver.cache_len(), 3);
    }

    #[test]
    fn test_resolve_server_config() {
        setup_crypto_provider();
        let resolver = create_test_resolver();
        let config = resolver
            .resolve_server_config("example.com")
            .expect("Failed to resolve config");

        assert!(config.alpn_protocols.is_empty());
    }

    #[test]
    fn test_clear_cache() {
        let resolver = create_test_resolver();

        resolver.resolve("example.com").expect("Resolve");
        assert_eq!(resolver.cache_len(), 1);

        resolver.clear_cache();
        assert_eq!(resolver.cache_len(), 0);
    }

    #[test]
    fn test_build_sni_server_config() {
        setup_crypto_provider();
        let resolver = Arc::new(create_test_resolver());
        let config = build_sni_server_config(resolver);
        assert!(config.alpn_protocols.is_empty());
    }
}

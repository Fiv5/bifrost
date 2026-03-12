use crate::ca::CertificateAuthority;
use crate::cache::{CertCache, ServerConfigCache};
use crate::dynamic::DynamicCertGenerator;
use bifrost_core::error::Result;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls::ServerConfig;
use std::sync::Arc;

#[derive(Debug)]
pub struct SniResolver {
    cert_generator: DynamicCertGenerator,
    cert_cache: CertCache,
    server_config_cache: ServerConfigCache,
}

impl SniResolver {
    pub fn new(ca: Arc<CertificateAuthority>) -> Self {
        Self {
            cert_generator: DynamicCertGenerator::new(ca),
            cert_cache: CertCache::new(),
            server_config_cache: ServerConfigCache::new(),
        }
    }

    pub fn with_cache_capacity(ca: Arc<CertificateAuthority>, capacity: usize) -> Self {
        Self {
            cert_generator: DynamicCertGenerator::new(ca),
            cert_cache: CertCache::with_capacity(capacity),
            server_config_cache: ServerConfigCache::with_capacity(capacity),
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
        self.resolve_server_config_with_alpn(server_name, &[])
    }

    pub fn resolve_server_config_with_alpn(
        &self,
        server_name: &str,
        alpn_protocols: &[Vec<u8>],
    ) -> Result<Arc<ServerConfig>> {
        if let Some(config) = self.server_config_cache.get(server_name, alpn_protocols) {
            return Ok(config);
        }

        let cert_key = self.resolve(server_name)?;

        let mut config = ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(SingleCertResolver {
                cert_key: cert_key.clone(),
            }));
        config.alpn_protocols = alpn_protocols.to_vec();

        let config = Arc::new(config);
        self.server_config_cache
            .insert(server_name, alpn_protocols, config.clone());

        Ok(config)
    }

    pub fn clear_cache(&self) {
        self.cert_cache.clear();
        self.server_config_cache.clear();
    }

    pub fn cache_len(&self) -> usize {
        self.cert_cache.len()
    }

    pub fn server_config_cache_len(&self) -> usize {
        self.server_config_cache.len()
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
    cert_key: Arc<CertifiedKey>,
}

impl ResolvesServerCert for SingleCertResolver {
    fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.cert_key.clone())
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
        assert_eq!(resolver.server_config_cache_len(), 0);
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
        assert_eq!(resolver.server_config_cache_len(), 1);
    }

    #[test]
    fn test_resolve_server_config_cached() {
        setup_crypto_provider();
        let resolver = create_test_resolver();

        let config1 = resolver
            .resolve_server_config("example.com")
            .expect("Failed to resolve config");
        let config2 = resolver
            .resolve_server_config("example.com")
            .expect("Failed to resolve cached config");

        assert!(Arc::ptr_eq(&config1, &config2));
        assert_eq!(resolver.server_config_cache_len(), 1);
    }

    #[test]
    fn test_resolve_server_config_with_alpn() {
        setup_crypto_provider();
        let resolver = create_test_resolver();
        let alpn = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let config1 = resolver
            .resolve_server_config_with_alpn("example.com", &alpn)
            .expect("Failed to resolve ALPN config");
        let config2 = resolver
            .resolve_server_config_with_alpn("example.com", &alpn)
            .expect("Failed to resolve cached ALPN config");
        let config3 = resolver
            .resolve_server_config("example.com")
            .expect("Failed to resolve empty ALPN config");

        assert!(Arc::ptr_eq(&config1, &config2));
        assert!(!Arc::ptr_eq(&config1, &config3));
        assert_eq!(config1.alpn_protocols, alpn);
        assert!(config3.alpn_protocols.is_empty());
        assert_eq!(resolver.server_config_cache_len(), 2);
    }

    #[test]
    fn test_clear_cache() {
        let resolver = create_test_resolver();

        resolver.resolve("example.com").expect("Resolve");
        resolver
            .resolve_server_config("example.com")
            .expect("Resolve config");
        assert_eq!(resolver.cache_len(), 1);
        assert_eq!(resolver.server_config_cache_len(), 1);

        resolver.clear_cache();
        assert_eq!(resolver.cache_len(), 0);
        assert_eq!(resolver.server_config_cache_len(), 0);
    }

    #[test]
    fn test_build_sni_server_config() {
        setup_crypto_provider();
        let resolver = Arc::new(create_test_resolver());
        let config = build_sni_server_config(resolver);
        assert!(config.alpn_protocols.is_empty());
    }
}

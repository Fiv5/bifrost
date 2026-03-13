use std::sync::Arc;

use tokio_rustls::rustls::server::ResolvesServerCert;
use tokio_rustls::rustls::sign::CertifiedKey;

pub struct SingleCertResolver(pub Arc<CertifiedKey>);

impl std::fmt::Debug for SingleCertResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SingleCertResolver")
    }
}

impl ResolvesServerCert for SingleCertResolver {
    fn resolve(
        &self,
        _client_hello: tokio_rustls::rustls::server::ClientHello<'_>,
    ) -> Option<Arc<CertifiedKey>> {
        Some(self.0.clone())
    }
}

use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;

use crate::dns::DnsResolver;
use crate::server::BoxBody;
use hyper::body::Incoming;
use hyper::Request;
use hyper_util::client::legacy::connect::dns::Name;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::Error as ClientError;
use hyper_util::rt::TokioExecutor;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tower::Service;

pub(super) type PooledHttpsConnector =
    hyper_rustls::HttpsConnector<HttpConnector<ProxyDnsResolver>>;
type HttpsPooledClient = Client<PooledHttpsConnector, BoxBody>;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ClientCacheKey {
    unsafe_ssl: bool,
    dns_servers: Vec<String>,
    pool_partition: String,
}

#[derive(Clone)]
pub(super) struct ProxyDnsResolver {
    dns_servers: Arc<Vec<String>>,
    resolver: Arc<DnsResolver>,
}

type ResolveAddrs = std::vec::IntoIter<SocketAddr>;
type ResolveFuture = Pin<Box<dyn Future<Output = io::Result<ResolveAddrs>> + Send>>;

static HTTPS_CLIENTS: LazyLock<RwLock<HashMap<ClientCacheKey, Arc<HttpsPooledClient>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn build_root_cert_store() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    root_store
}

impl ProxyDnsResolver {
    fn new(dns_servers: Vec<String>) -> Self {
        Self {
            dns_servers: Arc::new(dns_servers),
            resolver: Arc::new(DnsResolver::new(false)),
        }
    }
}

impl Service<Name> for ProxyDnsResolver {
    type Response = ResolveAddrs;
    type Error = io::Error;
    type Future = ResolveFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, name: Name) -> Self::Future {
        let dns_servers = Arc::clone(&self.dns_servers);
        let resolver = Arc::clone(&self.resolver);
        let host = name.as_str().to_string();

        Box::pin(async move {
            if dns_servers.is_empty() {
                let addrs: Vec<SocketAddr> =
                    tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
                return Ok(addrs.into_iter());
            }

            match resolver.resolve(&host, dns_servers.as_slice()).await {
                Ok(Some(ip)) => Ok(vec![SocketAddr::new(ip, 0)].into_iter()),
                Ok(None) => {
                    let addrs: Vec<SocketAddr> =
                        tokio::net::lookup_host((host.as_str(), 0)).await?.collect();
                    Ok(addrs.into_iter())
                }
                Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string())),
            }
        })
    }
}

#[derive(Debug)]
struct NoVerifier;

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[tokio_rustls::rustls::pki_types::CertificateDer<'_>],
        _server_name: &tokio_rustls::rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: tokio_rustls::rustls::pki_types::UnixTime,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::danger::ServerCertVerified,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &tokio_rustls::rustls::pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        vec![
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PKCS1_SHA512,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            tokio_rustls::rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA256,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA384,
            tokio_rustls::rustls::SignatureScheme::RSA_PSS_SHA512,
            tokio_rustls::rustls::SignatureScheme::ED25519,
        ]
    }
}

fn build_https_client(unsafe_ssl: bool, dns_servers: &[String]) -> HttpsPooledClient {
    let config = if unsafe_ssl {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    } else {
        ClientConfig::builder()
            .with_root_certificates(build_root_cert_store())
            .with_no_client_auth()
    };

    let resolver = ProxyDnsResolver::new(dns_servers.to_vec());
    let mut http_connector = HttpConnector::new_with_resolver(resolver);
    http_connector.enforce_http(false);
    http_connector.set_nodelay(true);
    http_connector.set_keepalive(Some(Duration::from_secs(60)));
    http_connector.set_connect_timeout(Some(Duration::from_secs(10)));

    let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(config)
        .https_or_http()
        .enable_all_versions()
        .wrap_connector(http_connector);

    Client::builder(TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(60))
        .pool_max_idle_per_host(32)
        .build(https_connector)
}

pub(super) fn get_https_client(
    unsafe_ssl: bool,
    dns_servers: &[String],
    pool_partition: &str,
) -> Arc<HttpsPooledClient> {
    let key = ClientCacheKey {
        unsafe_ssl,
        dns_servers: dns_servers.to_vec(),
        pool_partition: pool_partition.to_string(),
    };

    if let Ok(clients) = HTTPS_CLIENTS.read() {
        if let Some(client) = clients.get(&key) {
            return Arc::clone(client);
        }
    }

    let client = Arc::new(build_https_client(unsafe_ssl, &key.dns_servers));
    if let Ok(mut clients) = HTTPS_CLIENTS.write() {
        let entry = clients.entry(key).or_insert_with(|| Arc::clone(&client));
        return Arc::clone(entry);
    }
    client
}

pub(super) async fn send_pooled_request(
    request: Request<BoxBody>,
    unsafe_ssl: bool,
    dns_servers: &[String],
    pool_partition: &str,
) -> Result<hyper::Response<Incoming>, ClientError> {
    get_https_client(unsafe_ssl, dns_servers, pool_partition)
        .request(request)
        .await
}

pub(super) fn get_tls_client_config(unsafe_ssl: bool) -> Arc<ClientConfig> {
    // 允许 TLS 上游通过 ALPN 协商到 HTTP/2，从而避免被强制降级到 HTTP/1.1 造成大文件下载吞吐下降。
    // 这里显式打开 h2 + http/1.1，后续会根据协商结果选择对应的 Hyper handshake。
    let mut config = if unsafe_ssl {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    } else {
        ClientConfig::builder()
            .with_root_certificates(build_root_cert_store())
            .with_no_client_auth()
    };

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Arc::new(config)
}

pub(super) fn sanitize_upstream_headers(headers: &mut hyper::HeaderMap) {
    use hyper::header;

    // RFC7540: HTTP/2 禁止 hop-by-hop headers。
    // 同时移除 Connection 指定的额外 header。
    let extra_to_remove: Vec<header::HeaderName> = headers
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|conn| {
            conn.split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|token| header::HeaderName::from_bytes(token.as_bytes()).ok())
                .collect()
        })
        .unwrap_or_default();

    headers.remove(header::CONNECTION);
    for name in extra_to_remove {
        headers.remove(name);
    }
    headers.remove("proxy-connection");
    headers.remove("keep-alive");
    headers.remove("transfer-encoding");
    headers.remove("upgrade");
    headers.remove("trailer");

    // TE 在 HTTP/2 仅允许 "trailers"。
    if let Some(te) = headers.get(header::TE).and_then(|v| v.to_str().ok()) {
        if !te.trim().eq_ignore_ascii_case("trailers") {
            headers.remove(header::TE);
        }
    }
}

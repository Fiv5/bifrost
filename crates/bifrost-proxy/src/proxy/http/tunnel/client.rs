use std::sync::{Arc, OnceLock};

use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

use super::HttpsPooledClient;

static HTTPS_POOLED_CLIENT: OnceLock<HttpsPooledClient> = OnceLock::new();
static HTTPS_UNSAFE_POOLED_CLIENT: OnceLock<HttpsPooledClient> = OnceLock::new();

fn build_root_cert_store() -> RootCertStore {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    root_store
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

fn get_https_pooled_client() -> &'static HttpsPooledClient {
    HTTPS_POOLED_CLIENT.get_or_init(|| {
        let config = ClientConfig::builder()
            .with_root_certificates(build_root_cert_store())
            .with_no_client_auth();

        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(config)
            .https_or_http()
            .enable_all_versions()
            .build();

        Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(32)
            .build(https_connector)
    })
}

fn get_https_unsafe_pooled_client() -> &'static HttpsPooledClient {
    HTTPS_UNSAFE_POOLED_CLIENT.get_or_init(|| {
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(config)
            .https_or_http()
            .enable_all_versions()
            .build();

        Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(32)
            .build(https_connector)
    })
}

pub(super) fn get_https_client(unsafe_ssl: bool) -> &'static HttpsPooledClient {
    if unsafe_ssl {
        get_https_unsafe_pooled_client()
    } else {
        get_https_pooled_client()
    }
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

pub(super) fn sanitize_headers_for_http2(headers: &mut hyper::HeaderMap) {
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

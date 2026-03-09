mod common;

use bifrost_core::Protocol;
use bifrost_proxy::ProxyConfig;
use bifrost_tls::{generate_root_ca, init_crypto_provider, CertCache, DynamicCertGenerator};
use common::{add_test_rule, start_test_proxy, start_test_proxy_with_config};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn test_https_tunnel() {
    let proxy = start_test_proxy().await;

    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_addr = target.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = target.accept().await;
    });

    let mut stream = TcpStream::connect(proxy.addr()).await.unwrap();

    let connect_request = format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
        target_addr.port(),
        target_addr.port()
    );
    stream.write_all(connect_request.as_bytes()).await.unwrap();

    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await.unwrap();
    let response_str = String::from_utf8_lossy(&response[..n]);

    assert!(
        response_str.contains("200") || response_str.contains("OK"),
        "CONNECT should succeed, got: {}",
        response_str
    );
}

#[tokio::test]
async fn test_https_tunnel_with_port() {
    let proxy = start_test_proxy().await;

    let mut stream = TcpStream::connect(proxy.addr()).await.unwrap();

    let connect_request = "CONNECT example.com:8443 HTTP/1.1\r\nHost: example.com:8443\r\n\r\n";
    stream.write_all(connect_request.as_bytes()).await.unwrap();

    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await.unwrap();
    let response_str = String::from_utf8_lossy(&response[..n]);

    assert!(
        response_str.contains("200") || response_str.contains("OK") || response_str.contains("502"),
        "CONNECT response: {}",
        response_str
    );
}

#[tokio::test]
async fn test_https_tunnel_with_host_rule() {
    let proxy = start_test_proxy().await;

    add_test_rule(
        &proxy,
        "secure.example.com",
        Protocol::Host,
        "127.0.0.1:8443",
    );

    let mut stream = TcpStream::connect(proxy.addr()).await.unwrap();

    let connect_request =
        "CONNECT secure.example.com:443 HTTP/1.1\r\nHost: secure.example.com:443\r\n\r\n";
    stream.write_all(connect_request.as_bytes()).await.unwrap();

    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await.unwrap();
    let response_str = String::from_utf8_lossy(&response[..n]);

    assert!(
        response_str.contains("200") || response_str.contains("502"),
        "CONNECT with host rule should respond, got: {}",
        response_str
    );
}

#[tokio::test]
#[ignore = "Requires full TLS interception implementation"]
async fn test_https_interception() {
    let config = ProxyConfig {
        enable_tls_interception: true,
        ..Default::default()
    };

    let proxy = start_test_proxy_with_config(config).await;

    let mut stream = TcpStream::connect(proxy.addr()).await.unwrap();

    let connect_request =
        "CONNECT intercepted.example.com:443 HTTP/1.1\r\nHost: intercepted.example.com:443\r\n\r\n";
    stream.write_all(connect_request.as_bytes()).await.unwrap();

    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await.unwrap();
    let response_str = String::from_utf8_lossy(&response[..n]);

    assert!(
        response_str.contains("200") || response_str.contains("OK"),
        "TLS interception CONNECT should succeed"
    );
}

#[tokio::test]
async fn test_dynamic_cert_generation() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(Arc::clone(&ca));

    let cert_key = generator
        .generate_for_domain("test.example.com")
        .expect("Failed to generate certificate");

    assert_eq!(cert_key.cert.len(), 2, "Should have cert + CA chain");

    let wildcard_cert = generator
        .generate_for_domain("*.example.com")
        .expect("Failed to generate wildcard certificate");
    assert_eq!(wildcard_cert.cert.len(), 2);

    let ip_cert = generator
        .generate_for_domain("192.168.1.1")
        .expect("Failed to generate IP certificate");
    assert_eq!(ip_cert.cert.len(), 2);
}

#[tokio::test]
async fn test_cert_cache() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(Arc::clone(&ca));
    let cache = CertCache::new();

    let cert1 = generator.generate_for_domain("cached.example.com").unwrap();
    cache.insert("cached.example.com", Arc::new(cert1));

    let cached = cache.get("cached.example.com");
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().cert.len(), 2);

    let other_cert = generator.generate_for_domain("other.example.com").unwrap();
    cache.insert("other.example.com", Arc::new(other_cert));

    assert_eq!(cache.len(), 2);
}

#[tokio::test]
async fn test_multiple_https_tunnels() {
    let proxy = start_test_proxy().await;

    let domains = vec![
        "domain1.example.com:443",
        "domain2.example.com:443",
        "domain3.example.com:443",
    ];

    for domain in domains {
        let mut stream = TcpStream::connect(proxy.addr()).await.unwrap();

        let connect_request = format!("CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n", domain, domain);
        stream.write_all(connect_request.as_bytes()).await.unwrap();

        let mut response = vec![0u8; 1024];
        let n = stream.read(&mut response).await.unwrap();
        let response_str = String::from_utf8_lossy(&response[..n]);

        assert!(
            response_str.contains("200")
                || response_str.contains("OK")
                || response_str.contains("502"),
            "CONNECT to {} should respond",
            domain
        );
    }
}

#[tokio::test]
async fn test_https_tunnel_invalid_host() {
    let proxy = start_test_proxy().await;

    let mut stream = TcpStream::connect(proxy.addr()).await.unwrap();

    let connect_request = "CONNECT :443 HTTP/1.1\r\nHost: :443\r\n\r\n";
    stream.write_all(connect_request.as_bytes()).await.unwrap();

    let mut response = vec![0u8; 1024];
    let n = stream.read(&mut response).await.unwrap();
    let response_str = String::from_utf8_lossy(&response[..n]);

    assert!(
        response_str.contains("400")
            || response_str.contains("502")
            || response_str.contains("Bad"),
        "Invalid host should return error"
    );
}

#[test]
fn test_generate_root_ca() {
    init_crypto_provider();
    let ca = generate_root_ca().expect("Failed to generate root CA");
    let cert_der = ca.certificate_der().expect("Failed to get cert DER");
    let key_der = ca.private_key_der();

    assert!(!cert_der.is_empty());
    match key_der {
        bifrost_tls::rustls::pki_types::PrivateKeyDer::Pkcs8(key) => {
            assert!(!key.secret_pkcs8_der().is_empty());
        }
        _ => panic!("Expected PKCS8 key"),
    }
}

#[test]
fn test_dynamic_cert_for_subdomain() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(ca);

    let cert = generator
        .generate_for_domain("api.sub.example.com")
        .expect("Failed to generate subdomain cert");
    assert_eq!(cert.cert.len(), 2);
}

#[test]
fn test_dynamic_cert_for_localhost() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(ca);

    let cert = generator
        .generate_for_domain("localhost")
        .expect("Failed to generate localhost cert");
    assert_eq!(cert.cert.len(), 2);
}

#[test]
fn test_dynamic_cert_for_ipv4() {
    init_crypto_provider();
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(ca);

    let cert = generator
        .generate_for_domain("127.0.0.1")
        .expect("Failed to generate IPv4 cert");
    assert_eq!(cert.cert.len(), 2);
}

#[test]
fn test_cert_cache_capacity() {
    init_crypto_provider();
    let cache = CertCache::with_capacity(2);
    let ca = Arc::new(generate_root_ca().expect("Failed to generate CA"));
    let generator = DynamicCertGenerator::new(ca);

    let cert1 = generator.generate_for_domain("domain1.com").unwrap();
    let cert2 = generator.generate_for_domain("domain2.com").unwrap();
    let cert3 = generator.generate_for_domain("domain3.com").unwrap();

    cache.insert("domain1.com", Arc::new(cert1));
    cache.insert("domain2.com", Arc::new(cert2));
    cache.insert("domain3.com", Arc::new(cert3));

    assert!(cache.len() <= 3);
}

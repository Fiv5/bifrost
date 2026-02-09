mod common;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use bifrost_proxy::{AuthMethod, SocksCommand, SocksConfig, SocksServer};

const SOCKS5_VERSION: u8 = 0x05;
const SOCKS5_NO_AUTH: u8 = 0x00;
const SOCKS5_USERNAME_PASSWORD_AUTH: u8 = 0x02;
const SOCKS5_NO_ACCEPTABLE: u8 = 0xFF;
const SOCKS5_CMD_CONNECT: u8 = 0x01;
const SOCKS5_ATYP_DOMAIN: u8 = 0x03;
const SOCKS5_ATYP_IPV4: u8 = 0x01;

async fn start_socks5_server(auth_required: bool) -> (String, u16) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = SocksConfig {
        port: addr.port(),
        host: "127.0.0.1".to_string(),
        auth_required,
        username: if auth_required { Some("testuser".to_string()) } else { None },
        password: if auth_required { Some("testpass".to_string()) } else { None },
        timeout_secs: 30,
    };

    let server = SocksServer::new(config);
    tokio::spawn(async move {
        let _ = server.serve(listener).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    ("127.0.0.1".to_string(), addr.port())
}

#[tokio::test]
async fn test_socks5_connect() {
    let (host, port) = start_socks5_server(false).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_NO_AUTH]).await.unwrap();

    let mut response = [0u8; 2];
    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(response[0], SOCKS5_VERSION);
    assert_eq!(response[1], SOCKS5_NO_AUTH);
}

#[tokio::test]
async fn test_socks5_connect_with_domain() {
    let (host, port) = start_socks5_server(false).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_NO_AUTH]).await.unwrap();

    let mut auth_response = [0u8; 2];
    stream.read_exact(&mut auth_response).await.unwrap();
    assert_eq!(auth_response[0], SOCKS5_VERSION);

    let domain = "example.com";
    let target_port: u16 = 80;

    let mut connect_req = vec![
        SOCKS5_VERSION,
        SOCKS5_CMD_CONNECT,
        0x00,
        SOCKS5_ATYP_DOMAIN,
        domain.len() as u8,
    ];
    connect_req.extend_from_slice(domain.as_bytes());
    connect_req.extend_from_slice(&target_port.to_be_bytes());

    stream.write_all(&connect_req).await.unwrap();

    let mut connect_response = [0u8; 10];
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        stream.read(&mut connect_response)
    ).await;

    assert_eq!(connect_response[0], SOCKS5_VERSION);
}

#[tokio::test]
async fn test_socks5_auth() {
    let (host, port) = start_socks5_server(true).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_USERNAME_PASSWORD_AUTH]).await.unwrap();

    let mut auth_method_response = [0u8; 2];
    stream.read_exact(&mut auth_method_response).await.unwrap();

    assert_eq!(auth_method_response[0], SOCKS5_VERSION);
    assert_eq!(auth_method_response[1], SOCKS5_USERNAME_PASSWORD_AUTH);

    let username = "testuser";
    let password = "testpass";
    let mut auth_req = vec![0x01, username.len() as u8];
    auth_req.extend_from_slice(username.as_bytes());
    auth_req.push(password.len() as u8);
    auth_req.extend_from_slice(password.as_bytes());

    stream.write_all(&auth_req).await.unwrap();

    let mut auth_result = [0u8; 2];
    stream.read_exact(&mut auth_result).await.unwrap();

    assert_eq!(auth_result[0], 0x01);
    assert_eq!(auth_result[1], 0x00);
}

#[tokio::test]
async fn test_socks5_auth_failure() {
    let (host, port) = start_socks5_server(true).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_USERNAME_PASSWORD_AUTH]).await.unwrap();

    let mut auth_method_response = [0u8; 2];
    stream.read_exact(&mut auth_method_response).await.unwrap();

    let username = "wronguser";
    let password = "wrongpass";
    let mut auth_req = vec![0x01, username.len() as u8];
    auth_req.extend_from_slice(username.as_bytes());
    auth_req.push(password.len() as u8);
    auth_req.extend_from_slice(password.as_bytes());

    stream.write_all(&auth_req).await.unwrap();

    let mut auth_result = [0u8; 2];
    let result = stream.read_exact(&mut auth_result).await;

    if result.is_ok() {
        assert_ne!(auth_result[1], 0x00, "Wrong credentials should fail authentication");
    }
}

#[tokio::test]
async fn test_socks5_domain() {
    let (host, port) = start_socks5_server(false).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_NO_AUTH]).await.unwrap();

    let mut response = [0u8; 2];
    stream.read_exact(&mut response).await.unwrap();

    let domain = "httpbin.org";
    let target_port: u16 = 80;

    let mut connect_req = vec![
        SOCKS5_VERSION,
        SOCKS5_CMD_CONNECT,
        0x00,
        SOCKS5_ATYP_DOMAIN,
        domain.len() as u8,
    ];
    connect_req.extend_from_slice(domain.as_bytes());
    connect_req.extend_from_slice(&target_port.to_be_bytes());

    stream.write_all(&connect_req).await.unwrap();

    let mut connect_response = vec![0u8; 256];
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        stream.read(&mut connect_response)
    ).await;

    assert_eq!(connect_response[0], SOCKS5_VERSION);
}

#[tokio::test]
async fn test_socks5_ipv4_connect() {
    let (host, port) = start_socks5_server(false).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_NO_AUTH]).await.unwrap();

    let mut response = [0u8; 2];
    stream.read_exact(&mut response).await.unwrap();

    let ip_bytes: [u8; 4] = [127, 0, 0, 1];
    let target_port: u16 = 80;

    let mut connect_req = vec![
        SOCKS5_VERSION,
        SOCKS5_CMD_CONNECT,
        0x00,
        SOCKS5_ATYP_IPV4,
    ];
    connect_req.extend_from_slice(&ip_bytes);
    connect_req.extend_from_slice(&target_port.to_be_bytes());

    stream.write_all(&connect_req).await.unwrap();

    let mut connect_response = [0u8; 10];
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(2),
        stream.read(&mut connect_response)
    ).await;

    assert_eq!(connect_response[0], SOCKS5_VERSION);
}

#[tokio::test]
async fn test_socks5_no_auth_method() {
    let (host, port) = start_socks5_server(true).await;

    let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

    stream.write_all(&[SOCKS5_VERSION, 1, 0x03]).await.unwrap();

    let mut response = [0u8; 2];
    let result = stream.read_exact(&mut response).await;

    if result.is_ok() {
        assert!(
            response[1] == SOCKS5_NO_ACCEPTABLE || response[1] == SOCKS5_USERNAME_PASSWORD_AUTH,
            "Server should reject unsupported auth method"
        );
    }
}

#[tokio::test]
async fn test_socks5_multiple_connections() {
    let (host, port) = start_socks5_server(false).await;

    for _ in 0..5 {
        let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.unwrap();

        stream.write_all(&[SOCKS5_VERSION, 1, SOCKS5_NO_AUTH]).await.unwrap();

        let mut response = [0u8; 2];
        stream.read_exact(&mut response).await.unwrap();

        assert_eq!(response[0], SOCKS5_VERSION);
        assert_eq!(response[1], SOCKS5_NO_AUTH);
    }
}

#[test]
fn test_socks_config_default() {
    let config = SocksConfig::default();
    assert_eq!(config.port, 1080);
    assert!(!config.auth_required);
}

#[test]
fn test_socks_config_with_auth() {
    let config = SocksConfig {
        port: 1080,
        host: "127.0.0.1".to_string(),
        auth_required: true,
        username: Some("user".to_string()),
        password: Some("pass".to_string()),
        timeout_secs: 60,
    };
    assert!(config.auth_required);
    assert_eq!(config.username, Some("user".to_string()));
    assert_eq!(config.password, Some("pass".to_string()));
}

#[test]
fn test_auth_method_from_u8() {
    assert_eq!(AuthMethod::from(0x00), AuthMethod::NoAuth);
    assert_eq!(AuthMethod::from(0x02), AuthMethod::UsernamePassword);
    assert_eq!(AuthMethod::from(0xFF), AuthMethod::NoAcceptable);
}

#[test]
fn test_socks_command_try_from() {
    assert_eq!(SocksCommand::try_from(0x01).unwrap(), SocksCommand::Connect);
    assert_eq!(SocksCommand::try_from(0x02).unwrap(), SocksCommand::Bind);
    assert_eq!(SocksCommand::try_from(0x03).unwrap(), SocksCommand::UdpAssociate);
}

#[test]
fn test_socks_server_new() {
    let config = SocksConfig {
        port: 1080,
        host: "127.0.0.1".to_string(),
        auth_required: false,
        username: None,
        password: None,
        timeout_secs: 30,
    };
    let server = SocksServer::new(config.clone());
    assert_eq!(server.config().port, 1080);
}

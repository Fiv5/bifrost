use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};
use bifrost_core::{Result, BifrostError};

use crate::server::RulesResolver;

const SOCKS5_VERSION: u8 = 0x05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthMethod {
    NoAuth = 0x00,
    GssApi = 0x01,
    UsernamePassword = 0x02,
    NoAcceptable = 0xFF,
}

impl From<u8> for AuthMethod {
    fn from(value: u8) -> Self {
        match value {
            0x00 => AuthMethod::NoAuth,
            0x01 => AuthMethod::GssApi,
            0x02 => AuthMethod::UsernamePassword,
            _ => AuthMethod::NoAcceptable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocksCommand {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x03,
}

impl TryFrom<u8> for SocksCommand {
    type Error = BifrostError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x01 => Ok(SocksCommand::Connect),
            0x02 => Ok(SocksCommand::Bind),
            0x03 => Ok(SocksCommand::UdpAssociate),
            _ => Err(BifrostError::Parse(format!(
                "Invalid SOCKS5 command: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AddressType {
    IPv4 = 0x01,
    DomainName = 0x03,
    IPv6 = 0x04,
}

impl TryFrom<u8> for AddressType {
    type Error = BifrostError;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x01 => Ok(AddressType::IPv4),
            0x03 => Ok(AddressType::DomainName),
            0x04 => Ok(AddressType::IPv6),
            _ => Err(BifrostError::Parse(format!(
                "Invalid SOCKS5 address type: {}",
                value
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocksReply {
    Succeeded = 0x00,
    GeneralFailure = 0x01,
    ConnectionNotAllowed = 0x02,
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddressTypeNotSupported = 0x08,
}

#[derive(Debug, Clone)]
pub enum SocksAddress {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    DomainName(String),
}

impl SocksAddress {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SocksAddress::IPv4(addr) => {
                let mut bytes = vec![AddressType::IPv4 as u8];
                bytes.extend_from_slice(&addr.octets());
                bytes
            }
            SocksAddress::IPv6(addr) => {
                let mut bytes = vec![AddressType::IPv6 as u8];
                bytes.extend_from_slice(&addr.octets());
                bytes
            }
            SocksAddress::DomainName(domain) => {
                let mut bytes = vec![AddressType::DomainName as u8];
                bytes.push(domain.len() as u8);
                bytes.extend_from_slice(domain.as_bytes());
                bytes
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SocksConfig {
    pub port: u16,
    pub host: String,
    pub auth_required: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub timeout_secs: u64,
}

impl Default for SocksConfig {
    fn default() -> Self {
        Self {
            port: 1080,
            host: "127.0.0.1".to_string(),
            auth_required: false,
            username: None,
            password: None,
            timeout_secs: 30,
        }
    }
}

pub struct SocksServer {
    config: SocksConfig,
    rules: Arc<dyn RulesResolver>,
}

impl SocksServer {
    pub fn new(config: SocksConfig) -> Self {
        Self {
            config,
            rules: Arc::new(crate::server::NoOpRulesResolver),
        }
    }

    pub fn with_rules(mut self, rules: Arc<dyn RulesResolver>) -> Self {
        self.rules = rules;
        self
    }

    pub fn config(&self) -> &SocksConfig {
        &self.config
    }

    pub async fn bind(&self, addr: SocketAddr) -> Result<TcpListener> {
        TcpListener::bind(addr)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to bind to {}: {}", addr, e)))
    }

    pub async fn run(&self) -> Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .map_err(|e| BifrostError::Config(format!("Invalid address: {}", e)))?;

        let listener = self.bind(addr).await?;
        info!("SOCKS5 server listening on {}", addr);

        self.serve(listener).await
    }

    pub async fn serve(&self, listener: TcpListener) -> Result<()> {
        loop {
            let (stream, peer_addr) = listener
                .accept()
                .await
                .map_err(|e| BifrostError::Network(format!("Failed to accept connection: {}", e)))?;

            debug!("SOCKS5: Accepted connection from {}", peer_addr);

            let config = self.config.clone();
            let rules = Arc::clone(&self.rules);

            tokio::spawn(async move {
                let mut handler = SocksHandler::new(stream, config, rules);
                if let Err(e) = handler.handle_client().await {
                    error!("SOCKS5 error for {}: {}", peer_addr, e);
                }
            });
        }
    }
}

struct SocksHandler {
    stream: TcpStream,
    config: SocksConfig,
    rules: Arc<dyn RulesResolver>,
}

impl SocksHandler {
    fn new(stream: TcpStream, config: SocksConfig, rules: Arc<dyn RulesResolver>) -> Self {
        Self {
            stream,
            config,
            rules,
        }
    }

    pub async fn handle_client(&mut self) -> Result<()> {
        let auth_method = self.handle_handshake().await?;

        if auth_method == AuthMethod::UsernamePassword {
            self.handle_auth().await?;
        }

        let (address, port) = self.handle_request().await?;
        self.connect_and_relay(address, port).await
    }

    async fn handle_handshake(&mut self) -> Result<AuthMethod> {
        let mut header = [0u8; 2];
        self.stream.read_exact(&mut header).await?;

        let version = header[0];
        let nmethods = header[1];

        if version != SOCKS5_VERSION {
            return Err(BifrostError::Parse(format!(
                "Invalid SOCKS version: {}",
                version
            )));
        }

        let mut methods = vec![0u8; nmethods as usize];
        self.stream.read_exact(&mut methods).await?;

        let selected_method = self.select_auth_method(&methods);

        let response = [SOCKS5_VERSION, selected_method as u8];
        self.stream.write_all(&response).await?;

        if selected_method == AuthMethod::NoAcceptable {
            return Err(BifrostError::Network(
                "No acceptable authentication method".to_string(),
            ));
        }

        Ok(selected_method)
    }

    fn select_auth_method(&self, methods: &[u8]) -> AuthMethod {
        if self.config.auth_required {
            if methods.contains(&(AuthMethod::UsernamePassword as u8)) {
                return AuthMethod::UsernamePassword;
            }
        } else if methods.contains(&(AuthMethod::NoAuth as u8)) {
            return AuthMethod::NoAuth;
        }

        if methods.contains(&(AuthMethod::UsernamePassword as u8))
            && self.config.username.is_some()
            && self.config.password.is_some()
        {
            return AuthMethod::UsernamePassword;
        }

        if methods.contains(&(AuthMethod::NoAuth as u8)) && !self.config.auth_required {
            return AuthMethod::NoAuth;
        }

        AuthMethod::NoAcceptable
    }

    async fn handle_auth(&mut self) -> Result<()> {
        let mut version = [0u8; 1];
        self.stream.read_exact(&mut version).await?;

        if version[0] != 0x01 {
            return Err(BifrostError::Parse(format!(
                "Invalid auth version: {}",
                version[0]
            )));
        }

        let mut ulen = [0u8; 1];
        self.stream.read_exact(&mut ulen).await?;
        let mut username = vec![0u8; ulen[0] as usize];
        self.stream.read_exact(&mut username).await?;

        let mut plen = [0u8; 1];
        self.stream.read_exact(&mut plen).await?;
        let mut password = vec![0u8; plen[0] as usize];
        self.stream.read_exact(&mut password).await?;

        let username = String::from_utf8_lossy(&username).to_string();
        let password = String::from_utf8_lossy(&password).to_string();

        let auth_success = self.verify_credentials(&username, &password);

        let response = if auth_success {
            [0x01, 0x00]
        } else {
            [0x01, 0x01]
        };

        self.stream.write_all(&response).await?;

        if !auth_success {
            return Err(BifrostError::Network("Authentication failed".to_string()));
        }

        debug!("SOCKS5: User '{}' authenticated successfully", username);
        Ok(())
    }

    fn verify_credentials(&self, username: &str, password: &str) -> bool {
        match (&self.config.username, &self.config.password) {
            (Some(expected_user), Some(expected_pass)) => {
                username == expected_user && password == expected_pass
            }
            _ => false,
        }
    }

    async fn handle_request(&mut self) -> Result<(SocksAddress, u16)> {
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header).await?;

        let version = header[0];
        let cmd = header[1];
        let atyp = header[3];

        if version != SOCKS5_VERSION {
            return Err(BifrostError::Parse(format!(
                "Invalid SOCKS version in request: {}",
                version
            )));
        }

        let command = SocksCommand::try_from(cmd)?;
        let addr_type = AddressType::try_from(atyp)?;

        if command != SocksCommand::Connect {
            self.send_reply(SocksReply::CommandNotSupported, None)
                .await?;
            return Err(BifrostError::Network(format!(
                "Unsupported SOCKS5 command: {:?}",
                command
            )));
        }

        let address = self.read_address(addr_type).await?;

        let mut port_bytes = [0u8; 2];
        self.stream.read_exact(&mut port_bytes).await?;
        let port = u16::from_be_bytes(port_bytes);

        debug!("SOCKS5: Request to connect to {:?}:{}", address, port);

        Ok((address, port))
    }

    async fn read_address(&mut self, addr_type: AddressType) -> Result<SocksAddress> {
        match addr_type {
            AddressType::IPv4 => {
                let mut addr = [0u8; 4];
                self.stream.read_exact(&mut addr).await?;
                Ok(SocksAddress::IPv4(Ipv4Addr::from(addr)))
            }
            AddressType::IPv6 => {
                let mut addr = [0u8; 16];
                self.stream.read_exact(&mut addr).await?;
                Ok(SocksAddress::IPv6(Ipv6Addr::from(addr)))
            }
            AddressType::DomainName => {
                let mut len = [0u8; 1];
                self.stream.read_exact(&mut len).await?;
                let mut domain = vec![0u8; len[0] as usize];
                self.stream.read_exact(&mut domain).await?;
                let domain_str = String::from_utf8(domain).map_err(|e| {
                    BifrostError::Parse(format!("Invalid domain name encoding: {}", e))
                })?;
                Ok(SocksAddress::DomainName(domain_str))
            }
        }
    }

    async fn connect_and_relay(&mut self, address: SocksAddress, port: u16) -> Result<()> {
        let url = match &address {
            SocksAddress::IPv4(ip) => format!("socks5://{}:{}", ip, port),
            SocksAddress::IPv6(ip) => format!("socks5://[{}]:{}", ip, port),
            SocksAddress::DomainName(domain) => format!("socks5://{}:{}", domain, port),
        };

        let resolved_rules = self.rules.resolve(&url, "CONNECT");

        let (target_host, target_port) = if let Some(ref host_rule) = resolved_rules.host {
            let parts: Vec<&str> = host_rule.split(':').collect();
            let h = parts[0].to_string();
            let p = if parts.len() > 1 {
                parts[1].parse().unwrap_or(port)
            } else {
                port
            };
            (h, p)
        } else {
            match &address {
                SocksAddress::IPv4(ip) => (ip.to_string(), port),
                SocksAddress::IPv6(ip) => (ip.to_string(), port),
                SocksAddress::DomainName(domain) => (domain.clone(), port),
            }
        };

        let target_addr = format!("{}:{}", target_host, target_port);
        match TcpStream::connect(&target_addr).await {
            Ok(target_stream) => {
                let local_addr = target_stream.local_addr().ok();
                self.send_reply(SocksReply::Succeeded, local_addr).await?;
                debug!("SOCKS5: Connected to {}", target_addr);
                self.relay_data(target_stream).await
            }
            Err(e) => {
                let reply = match e.kind() {
                    std::io::ErrorKind::ConnectionRefused => SocksReply::ConnectionRefused,
                    std::io::ErrorKind::AddrNotAvailable => SocksReply::HostUnreachable,
                    _ => SocksReply::GeneralFailure,
                };
                self.send_reply(reply, None).await?;
                Err(BifrostError::Network(format!(
                    "Failed to connect to {}: {}",
                    target_addr, e
                )))
            }
        }
    }

    async fn send_reply(
        &mut self,
        reply: SocksReply,
        bound_addr: Option<SocketAddr>,
    ) -> Result<()> {
        let mut response = vec![SOCKS5_VERSION, reply as u8, 0x00];

        match bound_addr {
            Some(SocketAddr::V4(addr)) => {
                response.push(AddressType::IPv4 as u8);
                response.extend_from_slice(&addr.ip().octets());
                response.extend_from_slice(&addr.port().to_be_bytes());
            }
            Some(SocketAddr::V6(addr)) => {
                response.push(AddressType::IPv6 as u8);
                response.extend_from_slice(&addr.ip().octets());
                response.extend_from_slice(&addr.port().to_be_bytes());
            }
            None => {
                response.push(AddressType::IPv4 as u8);
                response.extend_from_slice(&[0, 0, 0, 0]);
                response.extend_from_slice(&[0, 0]);
            }
        }

        self.stream.write_all(&response).await?;
        Ok(())
    }

    async fn relay_data(&mut self, target_stream: TcpStream) -> Result<()> {
        let (mut client_read, mut client_write) = self.stream.split();
        let (mut target_read, mut target_write) = target_stream.into_split();

        let client_to_target = async {
            let mut buf = vec![0u8; 8192];
            loop {
                let n = client_read.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                target_write.write_all(&buf[..n]).await?;
            }
            target_write.shutdown().await?;
            Ok::<_, std::io::Error>(())
        };

        let target_to_client = async {
            let mut buf = vec![0u8; 8192];
            loop {
                let n = target_read.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                client_write.write_all(&buf[..n]).await?;
            }
            Ok::<_, std::io::Error>(())
        };

        let result = tokio::try_join!(client_to_target, target_to_client);

        match result {
            Ok(_) => {
                debug!("SOCKS5: Connection closed normally");
                Ok(())
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionReset
                    || e.kind() == std::io::ErrorKind::BrokenPipe
                {
                    debug!("SOCKS5: Connection closed: {}", e);
                    Ok(())
                } else {
                    Err(BifrostError::Network(format!("Relay error: {}", e)))
                }
            }
        }
    }
}

pub async fn parse_socks5_handshake(data: &[u8]) -> Result<(u8, Vec<AuthMethod>)> {
    if data.len() < 2 {
        return Err(BifrostError::Parse(
            "Insufficient data for SOCKS5 handshake".to_string(),
        ));
    }

    let version = data[0];
    let nmethods = data[1] as usize;

    if data.len() < 2 + nmethods {
        return Err(BifrostError::Parse(
            "Insufficient data for auth methods".to_string(),
        ));
    }

    let methods: Vec<AuthMethod> = data[2..2 + nmethods]
        .iter()
        .map(|&m| AuthMethod::from(m))
        .collect();

    Ok((version, methods))
}

pub fn build_handshake_response(method: AuthMethod) -> Vec<u8> {
    vec![SOCKS5_VERSION, method as u8]
}

pub fn build_reply(reply: SocksReply, addr: Option<&SocksAddress>, port: u16) -> Vec<u8> {
    let mut response = vec![SOCKS5_VERSION, reply as u8, 0x00];

    if let Some(address) = addr {
        response.extend(address.to_bytes());
    } else {
        response.push(AddressType::IPv4 as u8);
        response.extend_from_slice(&[0, 0, 0, 0]);
    }

    response.extend_from_slice(&port.to_be_bytes());
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_method_from_u8() {
        assert_eq!(AuthMethod::from(0x00), AuthMethod::NoAuth);
        assert_eq!(AuthMethod::from(0x01), AuthMethod::GssApi);
        assert_eq!(AuthMethod::from(0x02), AuthMethod::UsernamePassword);
        assert_eq!(AuthMethod::from(0xFF), AuthMethod::NoAcceptable);
        assert_eq!(AuthMethod::from(0x10), AuthMethod::NoAcceptable);
    }

    #[test]
    fn test_socks_command_try_from() {
        assert_eq!(SocksCommand::try_from(0x01).unwrap(), SocksCommand::Connect);
        assert_eq!(SocksCommand::try_from(0x02).unwrap(), SocksCommand::Bind);
        assert_eq!(
            SocksCommand::try_from(0x03).unwrap(),
            SocksCommand::UdpAssociate
        );
        assert!(SocksCommand::try_from(0x04).is_err());
        assert!(SocksCommand::try_from(0x00).is_err());
    }

    #[test]
    fn test_address_type_try_from() {
        assert_eq!(AddressType::try_from(0x01).unwrap(), AddressType::IPv4);
        assert_eq!(
            AddressType::try_from(0x03).unwrap(),
            AddressType::DomainName
        );
        assert_eq!(AddressType::try_from(0x04).unwrap(), AddressType::IPv6);
        assert!(AddressType::try_from(0x02).is_err());
        assert!(AddressType::try_from(0x05).is_err());
    }

    #[test]
    fn test_socks_address_ipv4_to_bytes() {
        let addr = SocksAddress::IPv4(Ipv4Addr::new(192, 168, 1, 1));
        let bytes = addr.to_bytes();
        assert_eq!(bytes, vec![0x01, 192, 168, 1, 1]);
    }

    #[test]
    fn test_socks_address_ipv6_to_bytes() {
        let addr = SocksAddress::IPv6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
        let bytes = addr.to_bytes();
        assert_eq!(bytes.len(), 17);
        assert_eq!(bytes[0], 0x04);
    }

    #[test]
    fn test_socks_address_domain_to_bytes() {
        let addr = SocksAddress::DomainName("example.com".to_string());
        let bytes = addr.to_bytes();
        assert_eq!(bytes[0], 0x03);
        assert_eq!(bytes[1], 11);
        assert_eq!(&bytes[2..], b"example.com");
    }

    #[test]
    fn test_socks_config_default() {
        let config = SocksConfig::default();
        assert_eq!(config.port, 1080);
        assert_eq!(config.host, "127.0.0.1");
        assert!(!config.auth_required);
        assert!(config.username.is_none());
        assert!(config.password.is_none());
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_socks_server_new() {
        let config = SocksConfig::default();
        let server = SocksServer::new(config);
        assert_eq!(server.config().port, 1080);
        assert_eq!(server.config().host, "127.0.0.1");
    }

    #[test]
    fn test_socks_server_custom_config() {
        let config = SocksConfig {
            port: 9050,
            host: "0.0.0.0".to_string(),
            auth_required: true,
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            timeout_secs: 60,
        };
        let server = SocksServer::new(config);
        assert_eq!(server.config().port, 9050);
        assert_eq!(server.config().host, "0.0.0.0");
        assert!(server.config().auth_required);
    }

    #[tokio::test]
    async fn test_parse_socks5_handshake_no_auth() {
        let data = [0x05, 0x01, 0x00];
        let (version, methods) = parse_socks5_handshake(&data).await.unwrap();
        assert_eq!(version, 0x05);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0], AuthMethod::NoAuth);
    }

    #[tokio::test]
    async fn test_parse_socks5_handshake_multiple_methods() {
        let data = [0x05, 0x03, 0x00, 0x01, 0x02];
        let (version, methods) = parse_socks5_handshake(&data).await.unwrap();
        assert_eq!(version, 0x05);
        assert_eq!(methods.len(), 3);
        assert_eq!(methods[0], AuthMethod::NoAuth);
        assert_eq!(methods[1], AuthMethod::GssApi);
        assert_eq!(methods[2], AuthMethod::UsernamePassword);
    }

    #[tokio::test]
    async fn test_parse_socks5_handshake_insufficient_data() {
        let data = [0x05];
        let result = parse_socks5_handshake(&data).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_socks5_handshake_insufficient_methods() {
        let data = [0x05, 0x03, 0x00];
        let result = parse_socks5_handshake(&data).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_build_handshake_response() {
        let response = build_handshake_response(AuthMethod::NoAuth);
        assert_eq!(response, vec![0x05, 0x00]);

        let response = build_handshake_response(AuthMethod::UsernamePassword);
        assert_eq!(response, vec![0x05, 0x02]);

        let response = build_handshake_response(AuthMethod::NoAcceptable);
        assert_eq!(response, vec![0x05, 0xFF]);
    }

    #[test]
    fn test_build_reply_success() {
        let addr = SocksAddress::IPv4(Ipv4Addr::new(127, 0, 0, 1));
        let reply = build_reply(SocksReply::Succeeded, Some(&addr), 8080);
        assert_eq!(reply[0], 0x05);
        assert_eq!(reply[1], 0x00);
        assert_eq!(reply[2], 0x00);
        assert_eq!(reply[3], 0x01);
        assert_eq!(&reply[4..8], &[127, 0, 0, 1]);
        assert_eq!(&reply[8..10], &[0x1F, 0x90]);
    }

    #[test]
    fn test_build_reply_failure() {
        let reply = build_reply(SocksReply::ConnectionRefused, None, 0);
        assert_eq!(reply[0], 0x05);
        assert_eq!(reply[1], 0x05);
        assert_eq!(reply[2], 0x00);
        assert_eq!(reply[3], 0x01);
        assert_eq!(&reply[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn test_build_reply_with_domain() {
        let addr = SocksAddress::DomainName("test.com".to_string());
        let reply = build_reply(SocksReply::Succeeded, Some(&addr), 443);
        assert_eq!(reply[0], 0x05);
        assert_eq!(reply[1], 0x00);
        assert_eq!(reply[2], 0x00);
        assert_eq!(reply[3], 0x03);
        assert_eq!(reply[4], 8);
        assert_eq!(&reply[5..13], b"test.com");
    }

    #[test]
    fn test_build_reply_with_ipv6() {
        let addr = SocksAddress::IPv6(Ipv6Addr::LOCALHOST);
        let reply = build_reply(SocksReply::Succeeded, Some(&addr), 8080);
        assert_eq!(reply[0], 0x05);
        assert_eq!(reply[1], 0x00);
        assert_eq!(reply[2], 0x00);
        assert_eq!(reply[3], 0x04);
        assert_eq!(reply.len(), 22);
    }

    #[test]
    fn test_socks_reply_values() {
        assert_eq!(SocksReply::Succeeded as u8, 0x00);
        assert_eq!(SocksReply::GeneralFailure as u8, 0x01);
        assert_eq!(SocksReply::ConnectionNotAllowed as u8, 0x02);
        assert_eq!(SocksReply::NetworkUnreachable as u8, 0x03);
        assert_eq!(SocksReply::HostUnreachable as u8, 0x04);
        assert_eq!(SocksReply::ConnectionRefused as u8, 0x05);
        assert_eq!(SocksReply::TtlExpired as u8, 0x06);
        assert_eq!(SocksReply::CommandNotSupported as u8, 0x07);
        assert_eq!(SocksReply::AddressTypeNotSupported as u8, 0x08);
    }

    #[test]
    fn test_socks_config_with_auth() {
        let config = SocksConfig {
            port: 1080,
            host: "127.0.0.1".to_string(),
            auth_required: true,
            username: Some("admin".to_string()),
            password: Some("secret".to_string()),
            timeout_secs: 30,
        };
        assert!(config.auth_required);
        assert_eq!(config.username, Some("admin".to_string()));
        assert_eq!(config.password, Some("secret".to_string()));
    }

    struct MockSocksHandler {
        config: SocksConfig,
    }

    impl MockSocksHandler {
        fn select_auth_method(&self, methods: &[u8]) -> AuthMethod {
            if self.config.auth_required {
                if methods.contains(&(AuthMethod::UsernamePassword as u8)) {
                    return AuthMethod::UsernamePassword;
                }
            } else if methods.contains(&(AuthMethod::NoAuth as u8)) {
                return AuthMethod::NoAuth;
            }

            if methods.contains(&(AuthMethod::UsernamePassword as u8))
                && self.config.username.is_some()
                && self.config.password.is_some()
            {
                return AuthMethod::UsernamePassword;
            }

            if methods.contains(&(AuthMethod::NoAuth as u8)) && !self.config.auth_required {
                return AuthMethod::NoAuth;
            }

            AuthMethod::NoAcceptable
        }

        fn verify_credentials(&self, username: &str, password: &str) -> bool {
            match (&self.config.username, &self.config.password) {
                (Some(expected_user), Some(expected_pass)) => {
                    username == expected_user && password == expected_pass
                }
                _ => false,
            }
        }
    }

    #[test]
    fn test_select_auth_method_no_auth_required() {
        let handler = MockSocksHandler {
            config: SocksConfig::default(),
        };
        let methods = vec![0x00, 0x02];
        assert_eq!(handler.select_auth_method(&methods), AuthMethod::NoAuth);
    }

    #[test]
    fn test_select_auth_method_auth_required() {
        let handler = MockSocksHandler {
            config: SocksConfig {
                auth_required: true,
                username: Some("user".to_string()),
                password: Some("pass".to_string()),
                ..Default::default()
            },
        };
        let methods = vec![0x00, 0x02];
        assert_eq!(
            handler.select_auth_method(&methods),
            AuthMethod::UsernamePassword
        );
    }

    #[test]
    fn test_select_auth_method_no_acceptable() {
        let handler = MockSocksHandler {
            config: SocksConfig {
                auth_required: true,
                username: None,
                password: None,
                ..Default::default()
            },
        };
        let methods = vec![0x00];
        assert_eq!(
            handler.select_auth_method(&methods),
            AuthMethod::NoAcceptable
        );
    }

    #[test]
    fn test_verify_credentials_success() {
        let handler = MockSocksHandler {
            config: SocksConfig {
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                ..Default::default()
            },
        };
        assert!(handler.verify_credentials("admin", "secret"));
    }

    #[test]
    fn test_verify_credentials_wrong_username() {
        let handler = MockSocksHandler {
            config: SocksConfig {
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                ..Default::default()
            },
        };
        assert!(!handler.verify_credentials("wrong", "secret"));
    }

    #[test]
    fn test_verify_credentials_wrong_password() {
        let handler = MockSocksHandler {
            config: SocksConfig {
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                ..Default::default()
            },
        };
        assert!(!handler.verify_credentials("admin", "wrong"));
    }

    #[test]
    fn test_verify_credentials_no_config() {
        let handler = MockSocksHandler {
            config: SocksConfig::default(),
        };
        assert!(!handler.verify_credentials("admin", "secret"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(SOCKS5_VERSION, 0x05);
    }
}

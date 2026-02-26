use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bifrost_admin::AdminState;
use bifrost_core::{BifrostError, Result};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::dns::DnsResolver;
use crate::protocol::QuicPacketDetector;
use crate::server::{ProxyConfig, RulesResolver, TlsConfig};

use super::tcp::{AddressType, SocksAddress};

#[cfg(feature = "http3")]
use crate::http3::QuicMitmRelay;

const UDP_BUFFER_SIZE: usize = 65535;
const SESSION_TIMEOUT: Duration = Duration::from_secs(300);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UdpSession {
    pub client_addr: SocketAddr,
    pub relay_socket: Arc<UdpSocket>,
    pub last_activity: Instant,
}

pub struct UdpRelay {
    bind_addr: SocketAddr,
    sessions: Arc<RwLock<HashMap<SocketAddr, UdpSession>>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    rules: Option<Arc<dyn RulesResolver>>,
    tls_config: Option<Arc<TlsConfig>>,
    proxy_config: Option<ProxyConfig>,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Option<Arc<DnsResolver>>,
    enable_quic_mitm: bool,
    #[cfg(feature = "http3")]
    #[allow(dead_code)]
    quic_mitm_relay: Option<QuicMitmRelay>,
}

impl UdpRelay {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            rules: None,
            tls_config: None,
            proxy_config: None,
            admin_state: None,
            dns_resolver: None,
            enable_quic_mitm: false,
            #[cfg(feature = "http3")]
            quic_mitm_relay: None,
        }
    }

    pub fn with_rules(mut self, rules: Arc<dyn RulesResolver>) -> Self {
        self.rules = Some(rules);
        self
    }

    #[allow(dead_code)]
    pub fn with_tls_config(mut self, tls_config: Arc<TlsConfig>) -> Self {
        self.tls_config = Some(tls_config);
        self
    }

    #[allow(dead_code)]
    pub fn with_proxy_config(mut self, proxy_config: ProxyConfig) -> Self {
        self.proxy_config = Some(proxy_config);
        self
    }

    #[allow(dead_code)]
    pub fn with_admin_state(mut self, admin_state: Arc<AdminState>) -> Self {
        self.admin_state = Some(admin_state);
        self
    }

    #[allow(dead_code)]
    pub fn with_dns_resolver(mut self, dns_resolver: Arc<DnsResolver>) -> Self {
        self.dns_resolver = Some(dns_resolver);
        self
    }

    #[allow(dead_code)]
    pub fn with_quic_mitm(mut self, enable: bool) -> Self {
        self.enable_quic_mitm = enable;
        self
    }

    pub async fn start(&mut self) -> Result<SocketAddr> {
        let socket = UdpSocket::bind(self.bind_addr).await.map_err(|e| {
            BifrostError::Network(format!(
                "Failed to bind UDP relay on {}: {}",
                self.bind_addr, e
            ))
        })?;

        let local_addr = socket.local_addr().map_err(|e| {
            BifrostError::Network(format!("Failed to get UDP relay local address: {}", e))
        })?;

        info!("SOCKS5 UDP relay listening on {}", local_addr);

        let socket = Arc::new(socket);
        let sessions = Arc::clone(&self.sessions);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let relay_socket = Arc::clone(&socket);
        let relay_sessions = Arc::clone(&sessions);
        let rules = self.rules.clone();
        let dns_resolver = self.dns_resolver.clone();
        let verbose = self
            .proxy_config
            .as_ref()
            .map(|c| c.verbose_logging)
            .unwrap_or(false);

        tokio::spawn(async move {
            let mut buf = vec![0u8; UDP_BUFFER_SIZE];

            loop {
                tokio::select! {
                    result = relay_socket.recv_from(&mut buf) => {
                        match result {
                            Ok((len, src_addr)) => {
                                if let Err(e) = Self::handle_packet(
                                    &relay_socket,
                                    &relay_sessions,
                                    &buf[..len],
                                    src_addr,
                                    &rules,
                                    &dns_resolver,
                                    verbose,
                                ).await {
                                    debug!("UDP relay packet error from {}: {}", src_addr, e);
                                }
                            }
                            Err(e) => {
                                error!("UDP relay recv error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("UDP relay shutting down");
                        break;
                    }
                }
            }
        });

        let cleanup_sessions = Arc::clone(&sessions);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                Self::cleanup_sessions(&cleanup_sessions).await;
            }
        });

        Ok(local_addr)
    }

    async fn handle_packet(
        relay_socket: &Arc<UdpSocket>,
        sessions: &Arc<RwLock<HashMap<SocketAddr, UdpSession>>>,
        data: &[u8],
        src_addr: SocketAddr,
        rules: &Option<Arc<dyn RulesResolver>>,
        dns_resolver: &Option<Arc<DnsResolver>>,
        verbose: bool,
    ) -> Result<()> {
        if data.len() < 10 {
            return Err(BifrostError::Parse("UDP packet too short".to_string()));
        }

        let rsv = u16::from_be_bytes([data[0], data[1]]);
        let frag = data[2];
        let atyp = data[3];

        if rsv != 0 {
            return Err(BifrostError::Parse("Invalid RSV field".to_string()));
        }

        if frag != 0 {
            debug!("UDP fragmentation not supported, dropping packet");
            return Ok(());
        }

        let (dest_addr, dest_port, payload_offset) = Self::parse_address(atyp, &data[4..])?;

        let payload = &data[4 + payload_offset..];

        let is_quic = Self::is_quic_packet(payload);

        debug!(
            "UDP relay: {} -> {:?}:{} ({} bytes, quic={})",
            src_addr,
            dest_addr,
            dest_port,
            payload.len(),
            is_quic
        );

        let (final_host, final_port, dns_servers) =
            Self::apply_rules(&dest_addr, dest_port, rules, is_quic, verbose);

        let target_addr = match &final_host {
            SocksAddress::IPv4(ip) => SocketAddr::new((*ip).into(), final_port),
            SocksAddress::IPv6(ip) => SocketAddr::new((*ip).into(), final_port),
            SocksAddress::DomainName(domain) => {
                if let Some(resolver) = dns_resolver {
                    if !dns_servers.is_empty() {
                        if verbose {
                            info!(
                                "UDP relay: [DNS] resolving {} with custom servers: {:?}",
                                domain, dns_servers
                            );
                        }
                        match resolver.resolve(domain, &dns_servers).await {
                            Ok(Some(ip)) => {
                                if verbose {
                                    info!("UDP relay: [DNS] resolved {} -> {}", domain, ip);
                                }
                                SocketAddr::new(ip, final_port)
                            }
                            Ok(None) | Err(_) => {
                                tokio::net::lookup_host(format!("{}:{}", domain, final_port))
                                    .await
                                    .map_err(|e| {
                                        BifrostError::Network(format!("DNS lookup failed: {}", e))
                                    })?
                                    .next()
                                    .ok_or_else(|| {
                                        BifrostError::Network("No address resolved".to_string())
                                    })?
                            }
                        }
                    } else {
                        match resolver.resolve(domain, &[]).await {
                            Ok(Some(ip)) => SocketAddr::new(ip, final_port),
                            Ok(None) | Err(_) => {
                                tokio::net::lookup_host(format!("{}:{}", domain, final_port))
                                    .await
                                    .map_err(|e| {
                                        BifrostError::Network(format!("DNS lookup failed: {}", e))
                                    })?
                                    .next()
                                    .ok_or_else(|| {
                                        BifrostError::Network("No address resolved".to_string())
                                    })?
                            }
                        }
                    }
                } else {
                    tokio::net::lookup_host(format!("{}:{}", domain, final_port))
                        .await
                        .map_err(|e| BifrostError::Network(format!("DNS lookup failed: {}", e)))?
                        .next()
                        .ok_or_else(|| BifrostError::Network("No address resolved".to_string()))?
                }
            }
        };

        let session = {
            let sessions_read = sessions.read().await;
            sessions_read.get(&src_addr).cloned()
        };

        let relay_socket_for_target = if let Some(mut session) = session {
            session.last_activity = Instant::now();
            {
                let mut sessions_write = sessions.write().await;
                sessions_write.insert(src_addr, session.clone());
            }
            session.relay_socket
        } else {
            let new_socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|e| {
                BifrostError::Network(format!("Failed to create relay socket: {}", e))
            })?;

            let new_socket = Arc::new(new_socket);

            let session = UdpSession {
                client_addr: src_addr,
                relay_socket: Arc::clone(&new_socket),
                last_activity: Instant::now(),
            };

            {
                let mut sessions_write = sessions.write().await;
                sessions_write.insert(src_addr, session);
            }

            let response_socket = Arc::clone(&new_socket);
            let main_relay = Arc::clone(relay_socket);
            let client = src_addr;

            tokio::spawn(async move {
                let mut buf = vec![0u8; UDP_BUFFER_SIZE];
                loop {
                    match tokio::time::timeout(SESSION_TIMEOUT, response_socket.recv_from(&mut buf))
                        .await
                    {
                        Ok(Ok((len, remote_addr))) => {
                            let response = Self::build_udp_response(&remote_addr, &buf[..len]);

                            if let Err(e) = main_relay.send_to(&response, client).await {
                                debug!("Failed to send UDP response to client: {}", e);
                                break;
                            }
                        }
                        Ok(Err(e)) => {
                            debug!("UDP session recv error: {}", e);
                            break;
                        }
                        Err(_) => {
                            debug!("UDP session timeout for client {}", client);
                            break;
                        }
                    }
                }
            });

            new_socket
        };

        relay_socket_for_target
            .send_to(payload, target_addr)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to send UDP packet: {}", e)))?;

        Ok(())
    }

    fn parse_address(atyp: u8, data: &[u8]) -> Result<(SocksAddress, u16, usize)> {
        SocksAddress::parse_from_bytes(atyp, data)
    }

    fn is_quic_packet(data: &[u8]) -> bool {
        QuicPacketDetector::is_quic_packet(data)
    }

    fn apply_rules(
        dest_addr: &SocksAddress,
        dest_port: u16,
        rules: &Option<Arc<dyn RulesResolver>>,
        is_quic: bool,
        verbose: bool,
    ) -> (SocksAddress, u16, Vec<String>) {
        let Some(rules) = rules else {
            return (dest_addr.clone(), dest_port, vec![]);
        };

        let host_str = match dest_addr {
            SocksAddress::IPv4(ip) => ip.to_string(),
            SocksAddress::IPv6(ip) => ip.to_string(),
            SocksAddress::DomainName(domain) => domain.clone(),
        };

        let scheme = if is_quic || dest_port == 443 {
            "https"
        } else {
            "http"
        };
        let url = format!("{}://{}:{}/", scheme, host_str, dest_port);

        let resolved = rules.resolve(&url, "GET");

        let dns_servers = resolved.dns_servers.clone();

        if let Some(ref host_rule) = resolved.host {
            let parts: Vec<&str> = host_rule.split(':').collect();
            let new_host = parts[0].to_string();
            let new_port = if parts.len() > 1 {
                parts[1].parse().unwrap_or(dest_port)
            } else {
                dest_port
            };

            if verbose {
                info!(
                    "UDP relay: host rule applied - {}:{} -> {}:{}",
                    host_str, dest_port, new_host, new_port
                );
            }

            if let Ok(ipv4) = new_host.parse::<std::net::Ipv4Addr>() {
                return (SocksAddress::IPv4(ipv4), new_port, dns_servers);
            }
            if let Ok(ipv6) = new_host.parse::<std::net::Ipv6Addr>() {
                return (SocksAddress::IPv6(ipv6), new_port, dns_servers);
            }
            return (SocksAddress::DomainName(new_host), new_port, dns_servers);
        }

        (dest_addr.clone(), dest_port, dns_servers)
    }

    fn build_udp_response(remote_addr: &SocketAddr, payload: &[u8]) -> Vec<u8> {
        let mut response = vec![0u8, 0u8, 0u8];

        match remote_addr {
            SocketAddr::V4(addr) => {
                response.push(AddressType::IPv4 as u8);
                response.extend_from_slice(&addr.ip().octets());
                response.extend_from_slice(&addr.port().to_be_bytes());
            }
            SocketAddr::V6(addr) => {
                response.push(AddressType::IPv6 as u8);
                response.extend_from_slice(&addr.ip().octets());
                response.extend_from_slice(&addr.port().to_be_bytes());
            }
        }

        response.extend_from_slice(payload);
        response
    }

    async fn cleanup_sessions(sessions: &Arc<RwLock<HashMap<SocketAddr, UdpSession>>>) {
        let now = Instant::now();
        let mut sessions_write = sessions.write().await;
        let before = sessions_write.len();

        sessions_write
            .retain(|_, session| now.duration_since(session.last_activity) < SESSION_TIMEOUT);

        let after = sessions_write.len();
        if before != after {
            debug!("UDP relay: cleaned up {} expired sessions", before - after);
        }
    }

    #[allow(dead_code)]
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

impl Drop for UdpRelay {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4_address() {
        let data = [192, 168, 1, 1, 0x1F, 0x90, 0x00];
        let (addr, port, offset) = UdpRelay::parse_address(0x01, &data).unwrap();
        assert!(matches!(addr, SocksAddress::IPv4(_)));
        assert_eq!(port, 8080);
        assert_eq!(offset, 6);
    }

    #[test]
    fn test_parse_domain_address() {
        let mut data = vec![11u8];
        data.extend_from_slice(b"example.com");
        data.extend_from_slice(&[0x01, 0xBB]);

        let (addr, port, offset) = UdpRelay::parse_address(0x03, &data).unwrap();
        assert!(matches!(addr, SocksAddress::DomainName(ref d) if d == "example.com"));
        assert_eq!(port, 443);
        assert_eq!(offset, 14);
    }

    #[test]
    fn test_build_udp_response_ipv4() {
        let addr: SocketAddr = "192.168.1.1:8080".parse().unwrap();
        let payload = b"test";
        let response = UdpRelay::build_udp_response(&addr, payload);

        assert_eq!(response[0], 0);
        assert_eq!(response[1], 0);
        assert_eq!(response[2], 0);
        assert_eq!(response[3], 0x01);
        assert_eq!(&response[4..8], &[192, 168, 1, 1]);
        assert_eq!(&response[8..10], &[0x1F, 0x90]);
        assert_eq!(&response[10..], b"test");
    }
}

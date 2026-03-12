use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::ensure_crypto_provider;
use bifrost_admin::AdminState;
use bifrost_core::{BifrostError, Result};
use quinn::{Endpoint, ServerConfig};
use rustls::sign::CertifiedKey;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info};

use crate::dns::DnsResolver;
use crate::protocol::QuicPacketDetector;
use crate::server::{ProxyConfig, RulesResolver, TlsConfig};
use crate::socks::SocksAddress;

use super::proxy::handle_h3_proxy_request;

const UDP_BUFFER_SIZE: usize = 65535;
const SESSION_TIMEOUT: Duration = Duration::from_secs(300);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct QuicMitmSession {
    pub client_addr: SocketAddr,
    pub target_host: String,
    pub target_port: u16,
    pub last_activity: Instant,
    pub quic_endpoint: Option<Arc<Endpoint>>,
}

pub struct QuicMitmRelay {
    bind_addr: SocketAddr,
    sessions: Arc<RwLock<HashMap<(SocketAddr, String), QuicMitmSession>>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    rules: Arc<dyn RulesResolver>,
    tls_config: Arc<TlsConfig>,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Arc<DnsResolver>,
    enable_mitm: bool,
}

impl QuicMitmRelay {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bind_addr: SocketAddr,
        rules: Arc<dyn RulesResolver>,
        tls_config: Arc<TlsConfig>,
        proxy_config: ProxyConfig,
        admin_state: Option<Arc<AdminState>>,
        dns_resolver: Arc<DnsResolver>,
        enable_mitm: bool,
    ) -> Self {
        Self {
            bind_addr,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            rules,
            tls_config,
            proxy_config,
            admin_state,
            dns_resolver,
            enable_mitm,
        }
    }

    pub async fn start(&mut self) -> Result<SocketAddr> {
        let socket = UdpSocket::bind(self.bind_addr).await.map_err(|e| {
            BifrostError::Network(format!(
                "Failed to bind QUIC MITM relay on {}: {}",
                self.bind_addr, e
            ))
        })?;

        let local_addr = socket.local_addr().map_err(|e| {
            BifrostError::Network(format!("Failed to get QUIC relay local address: {}", e))
        })?;

        info!("SOCKS5 QUIC MITM relay listening on {}", local_addr);

        let socket = Arc::new(socket);
        let sessions = Arc::clone(&self.sessions);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let relay_socket = Arc::clone(&socket);
        let relay_sessions = Arc::clone(&sessions);
        let rules = Arc::clone(&self.rules);
        let tls_config = Arc::clone(&self.tls_config);
        let proxy_config = self.proxy_config.clone();
        let admin_state = self.admin_state.clone();
        let dns_resolver = Arc::clone(&self.dns_resolver);
        let enable_mitm = self.enable_mitm;

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
                                    &tls_config,
                                    &proxy_config,
                                    &admin_state,
                                    &dns_resolver,
                                    enable_mitm,
                                ).await {
                                    debug!("QUIC MITM relay packet error from {}: {}", src_addr, e);
                                }
                            }
                            Err(e) => {
                                error!("QUIC MITM relay recv error: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("QUIC MITM relay shutting down");
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

    #[allow(clippy::too_many_arguments)]
    async fn handle_packet(
        relay_socket: &Arc<UdpSocket>,
        sessions: &Arc<RwLock<HashMap<(SocketAddr, String), QuicMitmSession>>>,
        data: &[u8],
        src_addr: SocketAddr,
        rules: &Arc<dyn RulesResolver>,
        tls_config: &Arc<TlsConfig>,
        proxy_config: &ProxyConfig,
        admin_state: &Option<Arc<AdminState>>,
        dns_resolver: &Arc<DnsResolver>,
        enable_mitm: bool,
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
        let sni = if is_quic {
            Self::extract_sni_from_quic(payload)
        } else {
            None
        };

        debug!(
            "QUIC MITM relay: {} -> {:?}:{} ({} bytes, quic={}, sni={:?})",
            src_addr,
            dest_addr,
            dest_port,
            payload.len(),
            is_quic,
            sni
        );

        if is_quic && enable_mitm && dest_port == 443 {
            if let Some(ref server_name) = sni {
                let should_intercept = Self::should_intercept(
                    server_name,
                    dest_port,
                    rules,
                    proxy_config,
                    admin_state,
                )
                .await;

                if should_intercept {
                    return Self::handle_quic_mitm(
                        relay_socket,
                        sessions,
                        payload,
                        src_addr,
                        server_name,
                        dest_port,
                        rules,
                        tls_config,
                        proxy_config,
                        admin_state,
                        dns_resolver,
                    )
                    .await;
                }
            }
        }

        Self::forward_raw_packet(relay_socket, payload, &dest_addr, dest_port, src_addr).await
    }

    async fn should_intercept(
        server_name: &str,
        port: u16,
        rules: &Arc<dyn RulesResolver>,
        proxy_config: &ProxyConfig,
        admin_state: &Option<Arc<AdminState>>,
    ) -> bool {
        let enable_tls_interception = if let Some(ref state) = admin_state {
            state.runtime_config.read().await.enable_tls_interception
        } else {
            proxy_config.enable_tls_interception
        };

        if !enable_tls_interception {
            return false;
        }

        let url = format!("https://{}:{}/", server_name, port);
        let resolved = rules.resolve(&url, "GET");

        if let Some(intercept) = resolved.tls_intercept {
            debug!(
                "QUIC MITM: TLS intercept rule matched for {} (intercept={})",
                server_name, intercept
            );
            return intercept;
        }

        !resolved.rules.is_empty()
            || resolved.host.is_some()
            || !resolved.req_headers.is_empty()
            || !resolved.res_headers.is_empty()
    }

    fn is_quic_packet(data: &[u8]) -> bool {
        QuicPacketDetector::is_quic_packet(data)
    }

    fn extract_sni_from_quic(data: &[u8]) -> Option<String> {
        if data.len() < 5 {
            return None;
        }

        let first_byte = data[0];
        let header_form = (first_byte >> 7) & 0x01;

        if header_form != 1 {
            return None;
        }

        let packet_type = (first_byte >> 4) & 0x03;
        if packet_type != 0 {
            return None;
        }

        if data.len() < 6 {
            return None;
        }

        let version = u32::from_be_bytes([data[1], data[2], data[3], data[4]]);
        if version == 0 {
            return None;
        }

        let mut offset = 5;

        if offset >= data.len() {
            return None;
        }
        let dcid_len = data[offset] as usize;
        offset += 1 + dcid_len;

        if offset >= data.len() {
            return None;
        }
        let scid_len = data[offset] as usize;
        offset += 1 + scid_len;

        if offset >= data.len() {
            return None;
        }

        let (token_len, token_len_size) = Self::read_varint(&data[offset..])?;
        offset += token_len_size + token_len as usize;

        if offset >= data.len() {
            return None;
        }

        let (_payload_len, payload_len_size) = Self::read_varint(&data[offset..])?;
        offset += payload_len_size;

        if offset + 4 >= data.len() {
            return None;
        }

        let _packet_number = &data[offset..offset + 4];
        offset += 4;

        Self::find_sni_in_crypto_frame(&data[offset..])
    }

    fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
        if data.is_empty() {
            return None;
        }

        let first_byte = data[0];
        let prefix = (first_byte >> 6) & 0x03;

        match prefix {
            0 => Some((first_byte as u64 & 0x3f, 1)),
            1 => {
                if data.len() < 2 {
                    return None;
                }
                let value = ((data[0] as u64 & 0x3f) << 8) | data[1] as u64;
                Some((value, 2))
            }
            2 => {
                if data.len() < 4 {
                    return None;
                }
                let value = ((data[0] as u64 & 0x3f) << 24)
                    | ((data[1] as u64) << 16)
                    | ((data[2] as u64) << 8)
                    | data[3] as u64;
                Some((value, 4))
            }
            3 => {
                if data.len() < 8 {
                    return None;
                }
                let value = ((data[0] as u64 & 0x3f) << 56)
                    | ((data[1] as u64) << 48)
                    | ((data[2] as u64) << 40)
                    | ((data[3] as u64) << 32)
                    | ((data[4] as u64) << 24)
                    | ((data[5] as u64) << 16)
                    | ((data[6] as u64) << 8)
                    | data[7] as u64;
                Some((value, 8))
            }
            _ => None,
        }
    }

    fn find_sni_in_crypto_frame(data: &[u8]) -> Option<String> {
        let mut offset = 0;

        while offset + 50 < data.len() {
            if data[offset..].starts_with(&[0x01]) {
                offset += 1;

                if offset + 4 > data.len() {
                    break;
                }

                let _client_hello_len = ((data[offset + 1] as usize) << 16)
                    | ((data[offset + 2] as usize) << 8)
                    | (data[offset + 3] as usize);

                offset += 4;

                if offset + 2 > data.len() {
                    break;
                }
                offset += 2;

                if offset + 32 > data.len() {
                    break;
                }
                offset += 32;

                if offset + 1 > data.len() {
                    break;
                }
                let session_id_len = data[offset] as usize;
                offset += 1 + session_id_len;

                if offset + 2 > data.len() {
                    break;
                }
                let cipher_suites_len =
                    ((data[offset] as usize) << 8) | (data[offset + 1] as usize);
                offset += 2 + cipher_suites_len;

                if offset + 1 > data.len() {
                    break;
                }
                let compression_len = data[offset] as usize;
                offset += 1 + compression_len;

                if offset + 2 > data.len() {
                    break;
                }
                let extensions_len = ((data[offset] as usize) << 8) | (data[offset + 1] as usize);
                offset += 2;

                let extensions_end = offset + extensions_len;
                while offset + 4 <= extensions_end && offset + 4 <= data.len() {
                    let ext_type = ((data[offset] as u16) << 8) | (data[offset + 1] as u16);
                    let ext_len = ((data[offset + 2] as usize) << 8) | (data[offset + 3] as usize);
                    offset += 4;

                    if ext_type == 0 {
                        if offset + 2 > data.len() {
                            break;
                        }
                        let _sni_list_len =
                            ((data[offset] as usize) << 8) | (data[offset + 1] as usize);
                        offset += 2;

                        if offset + 3 > data.len() {
                            break;
                        }
                        let name_type = data[offset];
                        let name_len =
                            ((data[offset + 1] as usize) << 8) | (data[offset + 2] as usize);
                        offset += 3;

                        if name_type == 0 && offset + name_len <= data.len() {
                            return String::from_utf8(data[offset..offset + name_len].to_vec())
                                .ok();
                        }
                    }

                    offset += ext_len;
                }

                break;
            }
            offset += 1;
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(unused_variables)]
    async fn handle_quic_mitm(
        relay_socket: &Arc<UdpSocket>,
        sessions: &Arc<RwLock<HashMap<(SocketAddr, String), QuicMitmSession>>>,
        payload: &[u8],
        src_addr: SocketAddr,
        server_name: &str,
        dest_port: u16,
        rules: &Arc<dyn RulesResolver>,
        tls_config: &Arc<TlsConfig>,
        proxy_config: &ProxyConfig,
        admin_state: &Option<Arc<AdminState>>,
        dns_resolver: &Arc<DnsResolver>,
    ) -> Result<()> {
        let session_key = (src_addr, server_name.to_string());

        let session = {
            let sessions_read = sessions.read().await;
            sessions_read.get(&session_key).cloned()
        };

        if let Some(mut session) = session {
            session.last_activity = Instant::now();
            {
                let mut sessions_write = sessions.write().await;
                sessions_write.insert(session_key, session);
            }
            return Ok(());
        }

        info!(
            "QUIC MITM: Starting interception for {} from {}",
            server_name, src_addr
        );

        let cert_generator = tls_config.cert_generator.as_ref().ok_or_else(|| {
            BifrostError::Tls("Certificate generator not configured for QUIC MITM".to_string())
        })?;

        let cert = cert_generator
            .generate_for_domain(server_name)
            .map_err(|e| BifrostError::Tls(format!("Failed to generate cert: {}", e)))?;

        let quic_server_config = Self::build_quic_server_config(&cert)?;

        let mitm_socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to bind MITM socket: {}", e)))?;
        let mitm_addr = mitm_socket.local_addr().map_err(|e| {
            BifrostError::Network(format!("Failed to get MITM socket address: {}", e))
        })?;

        let endpoint = Endpoint::server(quic_server_config, mitm_addr)
            .map_err(|e| BifrostError::Network(format!("Failed to create QUIC endpoint: {}", e)))?;

        let endpoint = Arc::new(endpoint);

        let session = QuicMitmSession {
            client_addr: src_addr,
            target_host: server_name.to_string(),
            target_port: dest_port,
            last_activity: Instant::now(),
            quic_endpoint: Some(Arc::clone(&endpoint)),
        };

        {
            let mut sessions_write = sessions.write().await;
            sessions_write.insert(session_key, session);
        }

        let rules = Arc::clone(rules);
        let proxy_config = proxy_config.clone();
        let admin_state = admin_state.clone();
        let dns_resolver = Arc::clone(dns_resolver);
        let server_name = server_name.to_string();
        let relay_socket = Arc::clone(relay_socket);

        tokio::spawn(async move {
            if let Err(e) = Self::run_quic_mitm_server(
                endpoint,
                &server_name,
                dest_port,
                &rules,
                &proxy_config,
                &admin_state,
                &dns_resolver,
                src_addr,
                &relay_socket,
            )
            .await
            {
                error!("QUIC MITM server error for {}: {}", server_name, e);
            }
        });

        Ok(())
    }

    fn build_quic_server_config(cert: &CertifiedKey) -> Result<ServerConfig> {
        ensure_crypto_provider();

        let mut crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_cert_resolver(Arc::new(SingleCertResolver(cert.clone())));

        crypto.max_early_data_size = u32::MAX;
        crypto.alpn_protocols = vec![b"h3".to_vec()];

        let quic_config =
            quinn::crypto::rustls::QuicServerConfig::try_from(crypto).map_err(|e| {
                BifrostError::Tls(format!("Failed to create QUIC server config: {}", e))
            })?;

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_config));

        let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
        transport_config.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
        transport_config.keep_alive_interval(Some(Duration::from_secs(15)));

        Ok(server_config)
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_quic_mitm_server(
        endpoint: Arc<Endpoint>,
        server_name: &str,
        dest_port: u16,
        rules: &Arc<dyn RulesResolver>,
        proxy_config: &ProxyConfig,
        admin_state: &Option<Arc<AdminState>>,
        dns_resolver: &Arc<DnsResolver>,
        client_addr: SocketAddr,
        _relay_socket: &Arc<UdpSocket>,
    ) -> Result<()> {
        info!(
            "QUIC MITM server started for {} (client: {})",
            server_name, client_addr
        );

        while let Some(incoming) = endpoint.accept().await {
            let connection = incoming.await.map_err(|e| {
                BifrostError::Network(format!("Failed to accept QUIC connection: {}", e))
            })?;

            let rules = Arc::clone(rules);
            let proxy_config = proxy_config.clone();
            let admin_state = admin_state.clone();
            let dns_resolver = Arc::clone(dns_resolver);
            let server_name = server_name.to_string();
            let target_port = dest_port;

            tokio::spawn(async move {
                if let Err(e) = Self::handle_quic_connection(
                    connection,
                    &server_name,
                    target_port,
                    &rules,
                    &proxy_config,
                    &admin_state,
                    &dns_resolver,
                )
                .await
                {
                    debug!("QUIC MITM connection error: {}", e);
                }
            });
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_quic_connection(
        connection: quinn::Connection,
        server_name: &str,
        _dest_port: u16,
        rules: &Arc<dyn RulesResolver>,
        proxy_config: &ProxyConfig,
        admin_state: &Option<Arc<AdminState>>,
        dns_resolver: &Arc<DnsResolver>,
    ) -> Result<()> {
        let peer_addr = connection.remote_address();

        info!(
            "QUIC MITM: Accepted connection from {} for {}",
            peer_addr, server_name
        );

        let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(connection))
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to create H3 connection: {}", e)))?;

        loop {
            match h3_conn.accept().await {
                Ok(Some(resolver)) => {
                    let (req, stream) = match resolver.resolve_request().await {
                        Ok(result) => result,
                        Err(e) => {
                            debug!("QUIC MITM: Failed to resolve request: {}", e);
                            continue;
                        }
                    };

                    let rules = Arc::clone(rules);
                    let proxy_config = proxy_config.clone();
                    let admin_state = admin_state.clone();
                    let dns_resolver = Arc::clone(dns_resolver);

                    tokio::spawn(async move {
                        if let Err(e) = handle_h3_proxy_request(
                            req,
                            stream,
                            peer_addr,
                            rules,
                            proxy_config,
                            admin_state,
                            dns_resolver,
                        )
                        .await
                        {
                            debug!("QUIC MITM request error: {}", e);
                        }
                    });
                }
                Ok(None) => {
                    debug!("QUIC MITM: Connection closed by peer: {}", peer_addr);
                    break;
                }
                Err(e) => {
                    debug!("QUIC MITM accept error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    #[allow(unused_variables)]
    async fn forward_raw_packet(
        relay_socket: &Arc<UdpSocket>,
        payload: &[u8],
        dest_addr: &SocksAddress,
        dest_port: u16,
        src_addr: SocketAddr,
    ) -> Result<()> {
        let target_addr = match dest_addr {
            SocksAddress::IPv4(ip) => SocketAddr::new((*ip).into(), dest_port),
            SocksAddress::IPv6(ip) => SocketAddr::new((*ip).into(), dest_port),
            SocksAddress::DomainName(domain) => {
                let resolved = tokio::net::lookup_host(format!("{}:{}", domain, dest_port))
                    .await
                    .map_err(|e| BifrostError::Network(format!("DNS lookup failed: {}", e)))?
                    .next()
                    .ok_or_else(|| BifrostError::Network("No address resolved".to_string()))?;
                resolved
            }
        };

        let send_socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to create send socket: {}", e)))?;

        send_socket
            .send_to(payload, target_addr)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to send UDP packet: {}", e)))?;

        Ok(())
    }

    fn parse_address(atyp: u8, data: &[u8]) -> Result<(SocksAddress, u16, usize)> {
        SocksAddress::parse_from_bytes(atyp, data)
    }

    async fn cleanup_sessions(
        sessions: &Arc<RwLock<HashMap<(SocketAddr, String), QuicMitmSession>>>,
    ) {
        let now = Instant::now();
        let mut sessions_write = sessions.write().await;
        let before = sessions_write.len();

        sessions_write
            .retain(|_, session| now.duration_since(session.last_activity) < SESSION_TIMEOUT);

        let after = sessions_write.len();
        if before != after {
            debug!(
                "QUIC MITM relay: cleaned up {} expired sessions",
                before - after
            );
        }
    }

    #[allow(dead_code)]
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

impl Drop for QuicMitmRelay {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}

#[derive(Debug)]
struct SingleCertResolver(CertifiedKey);

impl rustls::server::ResolvesServerCert for SingleCertResolver {
    fn resolve(&self, _client_hello: rustls::server::ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(Arc::new(self.0.clone()))
    }
}

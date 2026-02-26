use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bifrost_admin::{AdminState, TrafficRecord, TrafficType};
use bifrost_core::{BifrostError, Result};
use bytes::{Buf, Bytes, BytesMut};
use h3::quic::BidiStream;
use h3::server::RequestStream;
use hyper::{Request, Response, StatusCode};
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, info};

use super::capsule::{Capsule, CapsuleType};
use crate::dns::DnsResolver;
use crate::logging::RequestContext;
use crate::server::ProxyConfig;

const UDP_BUFFER_SIZE: usize = 65535;
const UDP_TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_UDP_PROTOCOL: &str = "connect-udp";

#[derive(Debug, Clone)]
pub struct ConnectUdpTarget {
    pub host: String,
    pub port: u16,
}

impl ConnectUdpTarget {
    pub fn from_uri(uri: &str) -> Result<Self> {
        let path = uri.trim_start_matches('/');

        if path.starts_with(".well-known/masque/udp/") {
            let parts: Vec<&str> = path
                .trim_start_matches(".well-known/masque/udp/")
                .split('/')
                .collect();

            if parts.len() >= 2 {
                let host = urlencoding::decode(parts[0])
                    .map_err(|e| BifrostError::Parse(format!("Invalid host encoding: {}", e)))?
                    .to_string();
                let port = parts[1]
                    .parse::<u16>()
                    .map_err(|e| BifrostError::Parse(format!("Invalid port: {}", e)))?;

                return Ok(Self { host, port });
            }
        }

        Err(BifrostError::Parse(format!(
            "Invalid CONNECT-UDP URI: {}",
            uri
        )))
    }

    pub fn to_socket_addr(&self) -> Result<String> {
        Ok(format!("{}:{}", self.host, self.port))
    }
}

pub struct UdpProxySession {
    target_socket: Arc<UdpSocket>,
    target_addr: SocketAddr,
    context_id: u64,
}

impl UdpProxySession {
    pub async fn new(target_addr: SocketAddr, context_id: u64) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to bind UDP socket: {}", e)))?;

        socket
            .connect(target_addr)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to connect UDP socket: {}", e)))?;

        Ok(Self {
            target_socket: Arc::new(socket),
            target_addr,
            context_id,
        })
    }

    pub fn target_addr(&self) -> SocketAddr {
        self.target_addr
    }

    pub fn context_id(&self) -> u64 {
        self.context_id
    }

    pub async fn send(&self, data: &[u8]) -> Result<usize> {
        self.target_socket
            .send(data)
            .await
            .map_err(|e| BifrostError::Network(format!("UDP send error: {}", e)))
    }

    pub async fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        self.target_socket
            .recv(buf)
            .await
            .map_err(|e| BifrostError::Network(format!("UDP recv error: {}", e)))
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_connect_udp<S>(
    req: Request<()>,
    stream: RequestStream<S, Bytes>,
    peer_addr: SocketAddr,
    proxy_config: ProxyConfig,
    admin_state: Option<Arc<AdminState>>,
    dns_resolver: Arc<DnsResolver>,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let uri = req.uri().path().to_string();
    let verbose = proxy_config.verbose_logging;
    let ctx = RequestContext::new();

    let target = ConnectUdpTarget::from_uri(&uri)?;

    if verbose {
        info!(
            "[{}] CONNECT-UDP request to {}:{}",
            ctx.id_str(),
            target.host,
            target.port
        );
    }

    let resolved_host = dns_resolver
        .resolve(&target.host, &[])
        .await
        .ok()
        .flatten()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| target.host.clone());

    let target_addr: SocketAddr = format!("{}:{}", resolved_host, target.port)
        .parse()
        .map_err(|e| BifrostError::Parse(format!("Invalid target address: {}", e)))?;

    let session = UdpProxySession::new(target_addr, 0).await?;

    let mut stream = stream;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("capsule-protocol", "?1")
        .body(())
        .unwrap();

    stream
        .send_response(response)
        .await
        .map_err(|e| BifrostError::Network(format!("Failed to send response: {}", e)))?;

    if verbose {
        info!(
            "[{}] CONNECT-UDP session established to {}",
            ctx.id_str(),
            target_addr
        );
    }

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .increment_connections_by_type(TrafficType::H3);
        state
            .metrics_collector
            .increment_requests_by_type(TrafficType::H3);

        let mut record = TrafficRecord::new(
            ctx.id_str(),
            "CONNECT-UDP".to_string(),
            format!("masque://{}:{}", target.host, target.port),
        );
        record.status = 200;
        record.protocol = "connect-udp".to_string();
        record.host = target.host.clone();
        record.client_ip = peer_addr.ip().to_string();
        record.set_h3();
        state.record_traffic(record);
    }

    let result = run_udp_proxy_session(
        stream,
        session,
        verbose,
        &ctx.id_str(),
        admin_state.as_ref(),
    )
    .await;

    if let Some(ref state) = admin_state {
        state
            .metrics_collector
            .decrement_connections_by_type(TrafficType::H3);
    }

    result
}

async fn run_udp_proxy_session<S>(
    stream: RequestStream<S, Bytes>,
    session: UdpProxySession,
    verbose: bool,
    req_id: &str,
    admin_state: Option<&Arc<AdminState>>,
) -> Result<()>
where
    S: BidiStream<Bytes> + Send + 'static,
{
    let (mut send_stream, mut recv_stream) = stream.split();
    let target_socket = session.target_socket.clone();
    let target_socket_for_recv = target_socket.clone();

    let admin_state_clone = admin_state.cloned();
    let admin_state_clone2 = admin_state.cloned();
    let req_id_owned = req_id.to_string();
    let req_id_owned2 = req_id.to_string();

    let client_to_target = async move {
        let mut capsule_buf = BytesMut::new();
        let mut total_sent: u64 = 0;

        loop {
            match recv_stream.recv_data().await {
                Ok(Some(data)) => {
                    capsule_buf.extend_from_slice(data.chunk());

                    while !capsule_buf.is_empty() {
                        let mut cursor = Cursor::new(&capsule_buf[..]);

                        match Capsule::decode(&mut cursor) {
                            Ok(Some(capsule)) => {
                                let consumed = capsule_buf.len() - cursor.remaining();
                                capsule_buf.advance(consumed);

                                if capsule.capsule_type == CapsuleType::Datagram {
                                    if let Ok((context_id, payload)) =
                                        capsule.parse_datagram_payload()
                                    {
                                        if context_id == 0 {
                                            match target_socket.send(&payload).await {
                                                Ok(n) => {
                                                    total_sent += n as u64;
                                                    if let Some(ref state) = admin_state_clone {
                                                        state
                                                            .metrics_collector
                                                            .add_bytes_sent_by_type(
                                                                TrafficType::H3,
                                                                n as u64,
                                                            );
                                                    }
                                                }
                                                Err(e) => {
                                                    debug!("UDP send error: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                debug!("Capsule decode error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    debug!("[{}] H3 recv error: {}", req_id_owned, e);
                    break;
                }
            }
        }

        Ok::<_, BifrostError>(total_sent)
    };

    let target_to_client = async move {
        let mut buf = vec![0u8; UDP_BUFFER_SIZE];
        let mut total_received: u64 = 0;

        loop {
            match timeout(UDP_TIMEOUT, target_socket_for_recv.recv(&mut buf)).await {
                Ok(Ok(n)) => {
                    if n == 0 {
                        continue;
                    }

                    let capsule = Capsule::datagram(0, Bytes::copy_from_slice(&buf[..n]));
                    let encoded = capsule.encode();

                    match send_stream.send_data(encoded).await {
                        Ok(_) => {
                            total_received += n as u64;
                            if let Some(ref state) = admin_state_clone2 {
                                state
                                    .metrics_collector
                                    .add_bytes_received_by_type(TrafficType::H3, n as u64);
                            }
                        }
                        Err(e) => {
                            debug!("[{}] H3 send error: {}", req_id_owned2, e);
                            break;
                        }
                    }
                }
                Ok(Err(e)) => {
                    debug!("[{}] UDP recv error: {}", req_id_owned2, e);
                    break;
                }
                Err(_) => {
                    debug!("[{}] UDP session timeout", req_id_owned2);
                    break;
                }
            }
        }

        let _ = send_stream.finish().await;
        Ok::<_, BifrostError>(total_received)
    };

    let result = tokio::try_join!(client_to_target, target_to_client);

    match result {
        Ok((sent, received)) => {
            if verbose {
                info!(
                    "[{}] CONNECT-UDP session closed (sent: {}, received: {})",
                    req_id, sent, received
                );
            }
            Ok(())
        }
        Err(e) => {
            if verbose {
                debug!("[{}] CONNECT-UDP session error: {}", req_id, e);
            }
            Err(e)
        }
    }
}

pub fn is_connect_udp_request(req: &Request<()>) -> bool {
    if req.method() != hyper::Method::from_bytes(b"CONNECT").unwrap() {
        return false;
    }

    if let Some(protocol) = req.headers().get(":protocol") {
        if protocol
            .to_str()
            .map(|s| s == CONNECT_UDP_PROTOCOL)
            .unwrap_or(false)
        {
            return true;
        }
    }

    let path = req.uri().path();
    path.contains(".well-known/masque/udp/")
}

pub fn parse_connect_udp_target(req: &Request<()>) -> Result<ConnectUdpTarget> {
    ConnectUdpTarget::from_uri(req.uri().path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_masque_uri() {
        let uri = "/.well-known/masque/udp/example.com/443/";
        let target = ConnectUdpTarget::from_uri(uri).unwrap();
        assert_eq!(target.host, "example.com");
        assert_eq!(target.port, 443);
    }

    #[test]
    fn test_parse_encoded_uri() {
        let uri = "/.well-known/masque/udp/test%2Eexample.com/8080/";
        let target = ConnectUdpTarget::from_uri(uri).unwrap();
        assert_eq!(target.host, "test.example.com");
        assert_eq!(target.port, 8080);
    }

    #[test]
    fn test_invalid_uri() {
        let uri = "/invalid/path";
        assert!(ConnectUdpTarget::from_uri(uri).is_err());
    }
}

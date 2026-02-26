use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bifrost_core::{BifrostError, Result};
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::RwLock;
use tracing::debug;

use crate::protocol::{ProtocolDetector, TransportProtocol};

const PEEK_BUFFER_SIZE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedProtocol {
    Http,
    Socks5,
    Socks4,
    Tls,
    Unknown,
}

impl From<TransportProtocol> for DetectedProtocol {
    fn from(p: TransportProtocol) -> Self {
        match p {
            TransportProtocol::Http1 | TransportProtocol::Http2 => DetectedProtocol::Http,
            TransportProtocol::Tls => DetectedProtocol::Tls,
            TransportProtocol::Socks5 => DetectedProtocol::Socks5,
            TransportProtocol::Socks4 => DetectedProtocol::Socks4,
            TransportProtocol::WebSocket
            | TransportProtocol::Sse
            | TransportProtocol::Grpc
            | TransportProtocol::Raw => DetectedProtocol::Http,
        }
    }
}

pub struct PeekableStream {
    stream: TcpStream,
    peeked_data: BytesMut,
    peeked_pos: usize,
}

impl PeekableStream {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            peeked_data: BytesMut::new(),
            peeked_pos: 0,
        }
    }

    pub async fn detect_protocol(&mut self) -> Result<DetectedProtocol> {
        let mut buf = [0u8; PEEK_BUFFER_SIZE];

        let n = self
            .stream
            .peek(&mut buf)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to peek stream: {}", e)))?;

        if n == 0 {
            return Ok(DetectedProtocol::Unknown);
        }

        self.peeked_data.extend_from_slice(&buf[..n]);

        match ProtocolDetector::detect_protocol_type(&buf[..n]) {
            Some(p) => Ok(DetectedProtocol::from(p)),
            None => Ok(DetectedProtocol::Unknown),
        }
    }

    pub fn into_inner(self) -> TcpStream {
        self.stream
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}

impl AsyncRead for PeekableStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.peeked_pos < self.peeked_data.len() {
            let remaining = &self.peeked_data[self.peeked_pos..];
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.peeked_pos += to_copy;
            return Poll::Ready(Ok(()));
        }

        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for PeekableStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpPacketType {
    Quic,
    Socks5Relay,
    Unknown,
}

pub struct UdpPacketDetector;

impl UdpPacketDetector {
    pub fn detect(
        data: &[u8],
        registered_clients: &[SocketAddr],
        source: &SocketAddr,
    ) -> UdpPacketType {
        if data.len() < 4 {
            return UdpPacketType::Unknown;
        }

        if registered_clients.contains(source) && Self::is_socks5_udp_packet(data) {
            return UdpPacketType::Socks5Relay;
        }

        if Self::is_quic_packet(data) {
            return UdpPacketType::Quic;
        }

        UdpPacketType::Unknown
    }

    fn is_quic_packet(data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }

        let first_byte = data[0];

        if first_byte & 0x80 != 0 {
            return true;
        }

        if first_byte & 0x40 != 0 {
            return true;
        }

        false
    }

    fn is_socks5_udp_packet(data: &[u8]) -> bool {
        if data.len() < 10 {
            return false;
        }

        if data[0] != 0 || data[1] != 0 {
            return false;
        }

        let atyp = data[3];
        match atyp {
            0x01 => data.len() >= 10,
            0x03 => {
                if data.len() < 5 {
                    return false;
                }
                let domain_len = data[4] as usize;
                data.len() >= 5 + domain_len + 2
            }
            0x04 => data.len() >= 22,
            _ => false,
        }
    }
}

pub struct UnifiedUdpSocket {
    socket: Arc<UdpSocket>,
    registered_socks5_clients: Arc<RwLock<Vec<SocketAddr>>>,
}

impl UnifiedUdpSocket {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            socket: Arc::new(socket),
            registered_socks5_clients: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn register_socks5_client(&self, addr: SocketAddr) {
        let mut clients = self.registered_socks5_clients.write().await;
        if !clients.contains(&addr) {
            clients.push(addr);
            debug!("Registered SOCKS5 UDP client: {}", addr);
        }
    }

    pub async fn unregister_socks5_client(&self, addr: &SocketAddr) {
        let mut clients = self.registered_socks5_clients.write().await;
        clients.retain(|a| a != addr);
        debug!("Unregistered SOCKS5 UDP client: {}", addr);
    }

    pub async fn recv_from_with_type(
        &self,
        buf: &mut [u8],
    ) -> io::Result<(usize, SocketAddr, UdpPacketType)> {
        let (len, addr) = self.socket.recv_from(buf).await?;
        let clients = self.registered_socks5_clients.read().await;
        let packet_type = UdpPacketDetector::detect(&buf[..len], &clients, &addr);
        Ok((len, addr, packet_type))
    }

    pub async fn send_to(&self, buf: &[u8], target: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(buf, target).await
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn inner(&self) -> &Arc<UdpSocket> {
        &self.socket
    }

    pub fn into_inner(self) -> Arc<UdpSocket> {
        self.socket
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_long_header_detection() {
        let quic_long_header = [0xC0, 0x00, 0x00, 0x01];
        assert!(UdpPacketDetector::is_quic_packet(&quic_long_header));

        let quic_initial = [0xC3, 0x00, 0x00, 0x01, 0x08];
        assert!(UdpPacketDetector::is_quic_packet(&quic_initial));
    }

    #[test]
    fn test_quic_short_header_detection() {
        let quic_short_header = [0x40, 0x00, 0x00, 0x01];
        assert!(UdpPacketDetector::is_quic_packet(&quic_short_header));
    }

    #[test]
    fn test_socks5_udp_detection() {
        let socks5_ipv4 = [0x00, 0x00, 0x00, 0x01, 8, 8, 8, 8, 0x00, 0x35, 0x12, 0x34];
        assert!(UdpPacketDetector::is_socks5_udp_packet(&socks5_ipv4));

        let socks5_domain = [
            0x00, 0x00, 0x00, 0x03, 0x06, b'g', b'o', b'o', b'g', b'l', b'e', 0x01, 0xBB, 0x00,
        ];
        assert!(UdpPacketDetector::is_socks5_udp_packet(&socks5_domain));

        let invalid = [0x00, 0x01, 0x00, 0x01];
        assert!(!UdpPacketDetector::is_socks5_udp_packet(&invalid));
    }

    #[test]
    fn test_packet_type_detection() {
        let source = "127.0.0.1:12345".parse().unwrap();
        let registered = vec![source];

        let socks5_packet = [0x00, 0x00, 0x00, 0x01, 8, 8, 8, 8, 0x00, 0x35, 0x00];
        assert_eq!(
            UdpPacketDetector::detect(&socks5_packet, &registered, &source),
            UdpPacketType::Socks5Relay
        );

        let quic_packet = [0xC0, 0x00, 0x00, 0x01];
        assert_eq!(
            UdpPacketDetector::detect(&quic_packet, &registered, &source),
            UdpPacketType::Quic
        );

        let unknown_source: SocketAddr = "127.0.0.1:54321".parse().unwrap();
        assert_eq!(
            UdpPacketDetector::detect(&socks5_packet, &registered, &unknown_source),
            UdpPacketType::Unknown
        );
    }
}

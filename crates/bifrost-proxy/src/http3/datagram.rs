use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bifrost_admin::{AdminState, TrafficType};
use bifrost_core::{BifrostError, Result};
use bytes::Bytes;
use quinn::Connection;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

use super::capsule::{decode_varint, encode_varint};

const DATAGRAM_BUFFER_SIZE: usize = 65535;
const SESSION_TIMEOUT: Duration = Duration::from_secs(300);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub struct DatagramSession {
    pub context_id: u64,
    pub target_addr: SocketAddr,
    pub udp_socket: Arc<UdpSocket>,
    pub last_activity: Instant,
}

pub struct DatagramHandler {
    connection: Connection,
    sessions: Arc<RwLock<HashMap<u64, DatagramSession>>>,
    admin_state: Option<Arc<AdminState>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl DatagramHandler {
    pub fn new(connection: Connection, admin_state: Option<Arc<AdminState>>) -> Self {
        Self {
            connection,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            admin_state,
            shutdown_tx: None,
        }
    }

    pub async fn register_session(&self, context_id: u64, target_addr: SocketAddr) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to bind UDP socket: {}", e)))?;

        socket
            .connect(target_addr)
            .await
            .map_err(|e| BifrostError::Network(format!("Failed to connect UDP socket: {}", e)))?;

        let session = DatagramSession {
            context_id,
            target_addr,
            udp_socket: Arc::new(socket),
            last_activity: Instant::now(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(context_id, session);

        info!(
            "MASQUE datagram session registered: context_id={}, target={}",
            context_id, target_addr
        );

        Ok(())
    }

    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let connection = self.connection.clone();
        let sessions = Arc::clone(&self.sessions);
        let admin_state = self.admin_state.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = connection.read_datagram() => {
                        match result {
                            Ok(data) => {
                                if let Err(e) = Self::handle_incoming_datagram(
                                    &data,
                                    &sessions,
                                    admin_state.as_ref(),
                                ).await {
                                    debug!("Datagram handling error: {}", e);
                                }
                            }
                            Err(e) => {
                                debug!("Datagram recv error: {}", e);
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Datagram handler shutting down");
                        break;
                    }
                }
            }
        });

        let sessions_for_recv = Arc::clone(&self.sessions);
        let connection_for_send = self.connection.clone();
        let admin_state_for_recv = self.admin_state.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; DATAGRAM_BUFFER_SIZE];

            loop {
                let sessions_snapshot: Vec<(u64, Arc<UdpSocket>)> = {
                    let sessions = sessions_for_recv.read().await;
                    sessions
                        .iter()
                        .map(|(id, s)| (*id, Arc::clone(&s.udp_socket)))
                        .collect()
                };

                if sessions_snapshot.is_empty() {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                for (context_id, socket) in sessions_snapshot {
                    match tokio::time::timeout(Duration::from_millis(10), socket.recv(&mut buf))
                        .await
                    {
                        Ok(Ok(n)) => {
                            if n > 0 {
                                let mut datagram = bytes::BytesMut::new();
                                encode_varint(context_id, &mut datagram);
                                datagram.extend_from_slice(&buf[..n]);

                                if let Err(e) = connection_for_send.send_datagram(datagram.freeze())
                                {
                                    debug!("Failed to send datagram: {}", e);
                                } else if let Some(ref state) = admin_state_for_recv {
                                    state
                                        .metrics_collector
                                        .add_bytes_received_by_type(TrafficType::H3, n as u64);
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            debug!("UDP recv error for context {}: {}", context_id, e);
                        }
                        Err(_) => {}
                    }
                }
            }
        });

        let cleanup_sessions = Arc::clone(&self.sessions);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVAL).await;
                Self::cleanup_expired_sessions(&cleanup_sessions).await;
            }
        });

        Ok(())
    }

    async fn handle_incoming_datagram(
        data: &Bytes,
        sessions: &Arc<RwLock<HashMap<u64, DatagramSession>>>,
        admin_state: Option<&Arc<AdminState>>,
    ) -> Result<()> {
        let mut cursor = std::io::Cursor::new(&data[..]);

        let context_id = decode_varint(&mut cursor)
            .ok_or_else(|| BifrostError::Parse("Invalid context ID in datagram".to_string()))?;

        let pos = cursor.position() as usize;
        let payload = &data[pos..];

        let session = {
            let mut sessions_write = sessions.write().await;
            if let Some(session) = sessions_write.get_mut(&context_id) {
                session.last_activity = Instant::now();
                Some(Arc::clone(&session.udp_socket))
            } else {
                None
            }
        };

        if let Some(socket) = session {
            socket
                .send(payload)
                .await
                .map_err(|e| BifrostError::Network(format!("UDP send error: {}", e)))?;

            if let Some(state) = admin_state {
                state
                    .metrics_collector
                    .add_bytes_sent_by_type(TrafficType::H3, payload.len() as u64);
            }
        } else {
            debug!("Received datagram for unknown context_id: {}", context_id);
        }

        Ok(())
    }

    async fn cleanup_expired_sessions(sessions: &Arc<RwLock<HashMap<u64, DatagramSession>>>) {
        let now = Instant::now();
        let mut sessions_write = sessions.write().await;
        let before = sessions_write.len();

        sessions_write
            .retain(|_, session| now.duration_since(session.last_activity) < SESSION_TIMEOUT);

        let after = sessions_write.len();
        if before != after {
            debug!("Cleaned up {} expired datagram sessions", before - after);
        }
    }

    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

impl Drop for DatagramHandler {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}

pub fn encode_datagram_payload(context_id: u64, payload: &[u8]) -> Bytes {
    let mut buf = bytes::BytesMut::new();
    encode_varint(context_id, &mut buf);
    buf.extend_from_slice(payload);
    buf.freeze()
}

pub fn decode_datagram_payload(data: &[u8]) -> Option<(u64, &[u8])> {
    let mut cursor = std::io::Cursor::new(data);
    let context_id = decode_varint(&mut cursor)?;
    let pos = cursor.position() as usize;
    Some((context_id, &data[pos..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_datagram_payload() {
        let context_id = 12345u64;
        let payload = b"hello world";

        let encoded = encode_datagram_payload(context_id, payload);
        let (decoded_id, decoded_payload) = decode_datagram_payload(&encoded).unwrap();

        assert_eq!(decoded_id, context_id);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn test_decode_empty_payload() {
        let data = [];
        assert!(decode_datagram_payload(&data).is_none());
    }
}

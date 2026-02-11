use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::body_store::{BodyRef, SharedBodyStore};
use crate::traffic::{FrameDirection, FrameType, SocketStatus};

const DEFAULT_PREVIEW_LIMIT: usize = 1024 * 1024; // 1MB for WebSocket
const DEFAULT_SSE_PREVIEW_LIMIT: usize = 4 * 1024 * 1024; // 4MB for SSE (same as normal request)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketFrameRecord {
    pub frame_id: u64,
    pub timestamp: u64,
    pub direction: FrameDirection,
    pub frame_type: FrameType,
    pub payload_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_preview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<BodyRef>,
    pub is_masked: bool,
    pub is_fin: bool,
}

impl WebSocketFrameRecord {
    pub fn new(
        frame_id: u64,
        direction: FrameDirection,
        frame_type: FrameType,
        payload: &[u8],
        is_masked: bool,
        is_fin: bool,
        preview_limit: usize,
    ) -> Self {
        let payload_preview = if frame_type == FrameType::Text
            || frame_type == FrameType::Close
            || frame_type == FrameType::Sse
        {
            let preview_bytes = &payload[..payload.len().min(preview_limit)];
            String::from_utf8_lossy(preview_bytes).to_string().into()
        } else if payload.len() <= preview_limit {
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                payload,
            ))
        } else {
            Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &payload[..preview_limit],
            ))
        };

        Self {
            frame_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            direction,
            frame_type,
            payload_size: payload.len(),
            payload_preview,
            payload_ref: None,
            is_masked,
            is_fin,
        }
    }

    pub fn new_sse_event(frame_id: u64, payload: &[u8], preview_limit: usize) -> Self {
        let payload_preview = {
            let preview_bytes = &payload[..payload.len().min(preview_limit)];
            String::from_utf8_lossy(preview_bytes).to_string().into()
        };

        Self {
            frame_id,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            direction: FrameDirection::Receive,
            frame_type: FrameType::Sse,
            payload_size: payload.len(),
            payload_preview,
            payload_ref: None,
            is_masked: false,
            is_fin: true,
        }
    }
}
const DEFAULT_MAX_FRAMES_PER_CONNECTION: usize = 1000;
const BROADCAST_CHANNEL_SIZE: usize = 256;

#[derive(Debug, Clone)]
pub struct FrameEvent {
    pub connection_id: String,
    pub frame: WebSocketFrameRecord,
}

pub struct ConnectionFrameStore {
    frames: VecDeque<WebSocketFrameRecord>,
    max_frames: usize,
    frame_id_counter: AtomicU64,
    status: SocketStatus,
    is_monitored: bool,
    tx: broadcast::Sender<FrameEvent>,
}

impl ConnectionFrameStore {
    pub fn new(max_frames: usize) -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CHANNEL_SIZE);
        Self {
            frames: VecDeque::with_capacity(max_frames.min(100)),
            max_frames,
            frame_id_counter: AtomicU64::new(0),
            status: SocketStatus::default(),
            is_monitored: false,
            tx,
        }
    }

    pub fn next_frame_id(&self) -> u64 {
        self.frame_id_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub fn add_frame(&mut self, frame: WebSocketFrameRecord) {
        tracing::debug!(
            "[WS_MONITOR] add_frame: frame_id={}, frame_type={:?}, direction={:?}, payload_size={}",
            frame.frame_id,
            frame.frame_type,
            frame.direction,
            frame.payload_size
        );
        match frame.direction {
            FrameDirection::Send => {
                self.status.send_count += 1;
                self.status.send_bytes += frame.payload_size as u64;
            }
            FrameDirection::Receive => {
                self.status.receive_count += 1;
                self.status.receive_bytes += frame.payload_size as u64;
            }
        }

        if self.frames.len() >= self.max_frames {
            self.frames.pop_front();
        }
        self.frames.push_back(frame.clone());
        tracing::debug!(
            "[WS_MONITOR] after add_frame: frames.len()={}",
            self.frames.len()
        );
    }

    pub fn set_closed(&mut self, code: Option<u16>, reason: Option<String>) {
        self.status.is_open = false;
        self.status.close_code = code;
        self.status.close_reason = reason;
    }
}

pub struct WebSocketMonitor {
    connections: RwLock<HashMap<String, ConnectionFrameStore>>,
    preview_limit: usize,
    sse_preview_limit: usize,
    max_frames_per_connection: usize,
    global_tx: broadcast::Sender<FrameEvent>,
}

impl WebSocketMonitor {
    pub fn new() -> Self {
        Self::with_config(
            DEFAULT_PREVIEW_LIMIT,
            DEFAULT_SSE_PREVIEW_LIMIT,
            DEFAULT_MAX_FRAMES_PER_CONNECTION,
        )
    }

    pub fn with_config(
        preview_limit: usize,
        sse_preview_limit: usize,
        max_frames_per_connection: usize,
    ) -> Self {
        let (global_tx, _) = broadcast::channel(BROADCAST_CHANNEL_SIZE * 4);
        Self {
            connections: RwLock::new(HashMap::new()),
            preview_limit,
            sse_preview_limit,
            max_frames_per_connection,
            global_tx,
        }
    }

    pub fn register_connection(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        if !connections.contains_key(connection_id) {
            connections.insert(
                connection_id.to_string(),
                ConnectionFrameStore::new(self.max_frames_per_connection),
            );
        }
    }

    pub fn unregister_connection(&self, connection_id: &str) {
        self.connections.write().remove(connection_id);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_frame(
        &self,
        connection_id: &str,
        direction: FrameDirection,
        frame_type: FrameType,
        payload: &[u8],
        is_masked: bool,
        is_fin: bool,
        body_store: Option<&SharedBodyStore>,
    ) -> Option<WebSocketFrameRecord> {
        let mut connections = self.connections.write();
        let store = connections.get_mut(connection_id)?;

        let frame_id = store.next_frame_id();
        let mut frame = WebSocketFrameRecord::new(
            frame_id,
            direction,
            frame_type,
            payload,
            is_masked,
            is_fin,
            self.preview_limit,
        );

        if store.is_monitored && payload.len() > self.preview_limit {
            if let Some(body_store) = body_store {
                let direction_str = match direction {
                    FrameDirection::Send => "send",
                    FrameDirection::Receive => "recv",
                };
                let ref_key = format!("{}_frame_{}_{}", connection_id, frame_id, direction_str);
                frame.payload_ref = body_store.read().store(&ref_key, "frame", payload);
            }
        }

        let frame_clone = frame.clone();
        store.add_frame(frame);

        let event = FrameEvent {
            connection_id: connection_id.to_string(),
            frame: frame_clone.clone(),
        };
        let _ = store.tx.send(event.clone());
        let _ = self.global_tx.send(event);

        Some(frame_clone)
    }

    pub fn record_sse_event(
        &self,
        connection_id: &str,
        payload: &[u8],
        body_store: Option<&SharedBodyStore>,
    ) -> Option<WebSocketFrameRecord> {
        let mut connections = self.connections.write();
        let store = connections.get_mut(connection_id)?;

        let frame_id = store.next_frame_id();
        let mut frame =
            WebSocketFrameRecord::new_sse_event(frame_id, payload, self.sse_preview_limit);

        if store.is_monitored && payload.len() > self.sse_preview_limit {
            if let Some(body_store) = body_store {
                let ref_key = format!("{}_sse_event_{}", connection_id, frame_id);
                frame.payload_ref = body_store.read().store(&ref_key, "sse", payload);
            }
        }

        let frame_clone = frame.clone();
        store.add_frame(frame);

        let event = FrameEvent {
            connection_id: connection_id.to_string(),
            frame: frame_clone.clone(),
        };
        let _ = store.tx.send(event.clone());
        let _ = self.global_tx.send(event);

        Some(frame_clone)
    }

    pub fn set_connection_closed(
        &self,
        connection_id: &str,
        code: Option<u16>,
        reason: Option<String>,
    ) {
        let mut connections = self.connections.write();
        if let Some(store) = connections.get_mut(connection_id) {
            store.set_closed(code, reason);
        }
    }

    pub fn start_monitoring(&self, connection_id: &str) -> bool {
        let mut connections = self.connections.write();
        if let Some(store) = connections.get_mut(connection_id) {
            store.is_monitored = true;
            true
        } else {
            false
        }
    }

    pub fn stop_monitoring(&self, connection_id: &str) -> bool {
        let mut connections = self.connections.write();
        if let Some(store) = connections.get_mut(connection_id) {
            store.is_monitored = false;
            true
        } else {
            false
        }
    }

    pub fn is_monitored(&self, connection_id: &str) -> bool {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.is_monitored)
            .unwrap_or(false)
    }

    pub fn get_frames(
        &self,
        connection_id: &str,
        after_frame_id: Option<u64>,
        limit: usize,
    ) -> Option<(Vec<WebSocketFrameRecord>, bool)> {
        let connections = self.connections.read();
        let store = connections.get(connection_id)?;

        tracing::debug!(
            "[WS_MONITOR] get_frames for {}: store.frames.len()={}, after_frame_id={:?}, limit={}",
            connection_id,
            store.frames.len(),
            after_frame_id,
            limit
        );

        let frames: Vec<_> = if let Some(after_id) = after_frame_id {
            store
                .frames
                .iter()
                .filter(|f| f.frame_id > after_id)
                .cloned()
                .collect()
        } else {
            store.frames.iter().cloned().collect()
        };

        let has_more = frames.len() > limit;
        let result = frames.into_iter().take(limit).collect();

        Some((result, has_more))
    }

    pub fn get_status(&self, connection_id: &str) -> Option<SocketStatus> {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.status.clone())
    }

    pub fn get_last_frame_id(&self, connection_id: &str) -> Option<u64> {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.frame_id_counter.load(Ordering::SeqCst).saturating_sub(1))
    }

    pub fn get_frame_count(&self, connection_id: &str) -> Option<usize> {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.frames.len())
    }

    pub fn subscribe_connection(
        &self,
        connection_id: &str,
    ) -> Option<broadcast::Receiver<FrameEvent>> {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.tx.subscribe())
    }

    pub fn subscribe_all(&self) -> broadcast::Receiver<FrameEvent> {
        self.global_tx.subscribe()
    }

    pub fn cleanup_closed_connections(&self) {
        let mut connections = self.connections.write();
        connections.retain(|_, store| store.status.is_open);
    }

    pub fn connection_count(&self) -> usize {
        self.connections.read().len()
    }

    pub fn active_connection_ids(&self) -> Vec<String> {
        self.connections
            .read()
            .iter()
            .filter(|(_, store)| store.status.is_open)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

impl Default for WebSocketMonitor {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedWebSocketMonitor = Arc<WebSocketMonitor>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_connection() {
        let monitor = WebSocketMonitor::new();
        monitor.register_connection("conn-1");
        assert_eq!(monitor.connection_count(), 1);
    }

    #[test]
    fn test_record_frame() {
        let monitor = WebSocketMonitor::new();
        monitor.register_connection("conn-1");

        let frame = monitor.record_frame(
            "conn-1",
            FrameDirection::Send,
            FrameType::Text,
            b"Hello, World!",
            true,
            true,
            None,
        );

        assert!(frame.is_some());
        let frame = frame.unwrap();
        assert_eq!(frame.frame_id, 0);
        assert_eq!(frame.direction, FrameDirection::Send);
        assert_eq!(frame.frame_type, FrameType::Text);
        assert_eq!(frame.payload_size, 13);
    }

    #[test]
    fn test_get_frames() {
        let monitor = WebSocketMonitor::new();
        monitor.register_connection("conn-1");

        for i in 0..5 {
            monitor.record_frame(
                "conn-1",
                FrameDirection::Send,
                FrameType::Text,
                format!("Message {}", i).as_bytes(),
                true,
                true,
                None,
            );
        }

        let (frames, has_more) = monitor.get_frames("conn-1", None, 10).unwrap();
        assert_eq!(frames.len(), 5);
        assert!(!has_more);

        let (frames, has_more) = monitor.get_frames("conn-1", Some(2), 10).unwrap();
        assert_eq!(frames.len(), 2);
        assert!(!has_more);
    }

    #[test]
    fn test_monitoring_state() {
        let monitor = WebSocketMonitor::new();
        monitor.register_connection("conn-1");

        assert!(!monitor.is_monitored("conn-1"));
        monitor.start_monitoring("conn-1");
        assert!(monitor.is_monitored("conn-1"));
        monitor.stop_monitoring("conn-1");
        assert!(!monitor.is_monitored("conn-1"));
    }

    #[test]
    fn test_socket_status() {
        let monitor = WebSocketMonitor::new();
        monitor.register_connection("conn-1");

        monitor.record_frame(
            "conn-1",
            FrameDirection::Send,
            FrameType::Text,
            b"test",
            true,
            true,
            None,
        );
        monitor.record_frame(
            "conn-1",
            FrameDirection::Receive,
            FrameType::Text,
            b"response",
            false,
            true,
            None,
        );

        let status = monitor.get_status("conn-1").unwrap();
        assert!(status.is_open);
        assert_eq!(status.send_count, 1);
        assert_eq!(status.receive_count, 1);
        assert_eq!(status.send_bytes, 4);
        assert_eq!(status.receive_bytes, 8);
    }

    #[test]
    fn test_connection_closed() {
        let monitor = WebSocketMonitor::new();
        monitor.register_connection("conn-1");

        monitor.set_connection_closed("conn-1", Some(1000), Some("Normal closure".to_string()));

        let status = monitor.get_status("conn-1").unwrap();
        assert!(!status.is_open);
        assert_eq!(status.close_code, Some(1000));
        assert_eq!(status.close_reason, Some("Normal closure".to_string()));
    }
}

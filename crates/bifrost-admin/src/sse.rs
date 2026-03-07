use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::traffic::SocketStatus;

const BROADCAST_CHANNEL_SIZE: usize = 1024;
const DEFAULT_RING_CAPACITY: usize = 2048;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub seq: u64,
    pub ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<u64>,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    #[serde(default)]
    pub parse_error: bool,
}

#[derive(Debug, Clone)]
pub struct SseEventEnvelope {
    pub connection_id: String,
    pub event: SseEvent,
}

#[derive(Debug)]
struct SseConnectionState {
    is_open: bool,
    receive_bytes: u64,
    receive_count: u64,
    seq: u64,
    tx: broadcast::Sender<SseEventEnvelope>,
    recent: VecDeque<SseEvent>,
    recent_capacity: usize,
}

impl SseConnectionState {
    fn new(recent_capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CHANNEL_SIZE);
        Self {
            is_open: true,
            receive_bytes: 0,
            receive_count: 0,
            seq: 0,
            tx,
            recent: VecDeque::with_capacity(recent_capacity.min(32)),
            recent_capacity,
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    fn push_recent(&mut self, event: SseEvent) {
        if self.recent.len() >= self.recent_capacity {
            self.recent.pop_front();
        }
        self.recent.push_back(event);
    }
}

#[derive(Debug, Default)]
pub struct SseHub {
    connections: RwLock<HashMap<String, SseConnectionState>>,
}

impl SseHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn register(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        connections
            .entry(connection_id.to_string())
            .or_insert_with(|| SseConnectionState::new(DEFAULT_RING_CAPACITY));
    }

    pub fn set_closed(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        if let Some(state) = connections.get_mut(connection_id) {
            state.is_open = false;
        }
    }

    pub fn unregister(&self, connection_id: &str) {
        self.connections.write().remove(connection_id);
    }

    pub fn add_receive_bytes(&self, connection_id: &str, bytes: usize) {
        let mut connections = self.connections.write();
        if let Some(state) = connections.get_mut(connection_id) {
            state.receive_bytes = state.receive_bytes.saturating_add(bytes as u64);
        }
    }

    pub fn is_open(&self, connection_id: &str) -> Option<bool> {
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.is_open)
    }

    pub fn get_socket_status(&self, connection_id: &str) -> Option<SocketStatus> {
        let connections = self.connections.read();
        let state = connections.get(connection_id)?;
        Some(SocketStatus {
            is_open: state.is_open,
            send_count: 0,
            receive_count: state.receive_count,
            send_bytes: 0,
            receive_bytes: state.receive_bytes,
            frame_count: state.receive_count as usize,
            close_code: None,
            close_reason: None,
        })
    }

    pub fn subscribe(&self, connection_id: &str) -> Option<broadcast::Receiver<SseEventEnvelope>> {
        let connections = self.connections.read();
        connections.get(connection_id).map(|s| s.tx.subscribe())
    }

    pub fn get_events_since(&self, connection_id: &str, last_seq: u64) -> Vec<SseEvent> {
        let connections = self.connections.read();
        let Some(state) = connections.get(connection_id) else {
            return Vec::new();
        };
        state
            .recent
            .iter()
            .filter(|e| e.seq > last_seq)
            .cloned()
            .collect()
    }

    pub fn publish_raw_event(&self, connection_id: &str, raw_event: &[u8]) -> Option<SseEvent> {
        let raw = String::from_utf8_lossy(raw_event).to_string();
        let mut connections = self.connections.write();
        let state = connections.get_mut(connection_id)?;
        if !state.is_open {
            return None;
        }

        let seq = state.next_seq();
        state.receive_count = state.receive_count.saturating_add(1);
        let ts = now_ms();
        let mut event = parse_sse_event(&raw);
        event.seq = seq;
        event.ts = ts;
        let envelope = SseEventEnvelope {
            connection_id: connection_id.to_string(),
            event: event.clone(),
        };
        let _ = state.tx.send(envelope);
        state.push_recent(event.clone());
        Some(event)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn parse_sse_event(raw: &str) -> SseEvent {
    let mut id: Option<String> = None;
    let mut event: Option<String> = None;
    let mut retry: Option<u64> = None;
    let mut data_lines: Vec<String> = Vec::new();

    for raw_line in raw.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            let v = rest.trim_start();
            if !v.is_empty() {
                event = Some(v.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("id:") {
            let v = rest.trim_start();
            if !v.is_empty() {
                id = Some(v.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("retry:") {
            let v = rest.trim_start();
            if let Ok(n) = v.parse::<u64>() {
                retry = Some(n);
            }
            continue;
        }
    }

    let data = if !data_lines.is_empty() {
        data_lines.join("\n")
    } else {
        raw.to_string()
    };

    SseEvent {
        seq: 0,
        ts: 0,
        id,
        event,
        retry,
        data,
        raw: Some(raw.to_string()),
        parse_error: false,
    }
}

pub fn parse_sse_events_from_text(input: &str) -> (Vec<SseEvent>, String) {
    let normalized = input.replace("\r\n", "\n");
    let mut events: Vec<SseEvent> = Vec::new();
    let mut start = 0usize;
    let bytes = normalized.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\n' && bytes[i + 1] == b'\n' {
            let chunk = &normalized[start..i];
            let chunk = chunk.trim_end_matches('\n');
            if !chunk.is_empty() {
                events.push(parse_sse_event(chunk));
            }
            i += 2;
            start = i;
            continue;
        }
        i += 1;
    }
    let remainder = if start < normalized.len() {
        normalized[start..].to_string()
    } else {
        String::new()
    };
    (events, remainder)
}

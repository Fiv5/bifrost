use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::traffic::SocketStatus;

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
struct SseConnectionState {
    is_open: bool,
    receive_bytes: u64,
    receive_count: u64,
    // 当用户在管理端主动查看 SSE messages 时，proxy 侧需要更激进地把 sse_raw 写盘，
    // 否则可能出现：count 在涨（基于解析/计数），但文件未 flush 导致详情流读不到数据。
    force_flush_until_ms: u64,
}

impl SseConnectionState {
    fn new() -> Self {
        Self {
            is_open: true,
            receive_bytes: 0,
            receive_count: 0,
            force_flush_until_ms: 0,
        }
    }
}

#[derive(Debug)]
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
            .or_insert_with(SseConnectionState::new);
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

    pub fn add_receive_event(&self, connection_id: &str) {
        let mut connections = self.connections.write();
        if let Some(state) = connections.get_mut(connection_id) {
            state.receive_count = state.receive_count.saturating_add(1);
        }
    }

    pub fn request_force_flush(&self, connection_id: &str, duration_ms: u64) {
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        let until = now.saturating_add(duration_ms);
        let mut connections = self.connections.write();
        let state = connections
            .entry(connection_id.to_string())
            .or_insert_with(SseConnectionState::new);
        state.force_flush_until_ms = state.force_flush_until_ms.max(until);
    }

    pub fn should_force_flush(&self, connection_id: &str) -> bool {
        let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
        self.connections
            .read()
            .get(connection_id)
            .map(|s| s.force_flush_until_ms > now)
            .unwrap_or(false)
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
}

impl Default for SseHub {
    fn default() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }
}

pub fn parse_sse_event(raw: &str) -> SseEvent {
    parse_sse_event_with_error(raw, false)
}

fn parse_sse_event_with_error(raw: &str, parse_error: bool) -> SseEvent {
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
        raw: if parse_error {
            Some(raw.to_string())
        } else {
            None
        },
        parse_error,
    }
}

pub fn parse_sse_events_from_text(input: &str) -> (Vec<SseEvent>, String) {
    let mut events: Vec<SseEvent> = Vec::new();
    let mut buffer = String::new();
    let mut prev_nl = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' && matches!(chars.peek(), Some('\n')) {
            continue;
        }
        if ch == '\n' {
            if prev_nl {
                let chunk = buffer.trim_end_matches('\n').to_string();
                if !chunk.is_empty() {
                    events.push(parse_sse_event(&chunk));
                }
                buffer.clear();
                prev_nl = false;
                continue;
            }
            buffer.push('\n');
            prev_nl = true;
            continue;
        }
        prev_nl = false;
        buffer.push(ch);
    }
    (events, buffer)
}

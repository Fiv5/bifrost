use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::body_store::BodyRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameType {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
    Continuation,
    Sse,
}

impl FrameType {
    pub fn from_opcode(opcode: u8) -> Self {
        match opcode {
            0x0 => FrameType::Continuation,
            0x1 => FrameType::Text,
            0x2 => FrameType::Binary,
            0x8 => FrameType::Close,
            0x9 => FrameType::Ping,
            0xA => FrameType::Pong,
            _ => FrameType::Binary,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketStatus {
    pub is_open: bool,
    pub send_count: u64,
    pub receive_count: u64,
    pub send_bytes: u64,
    pub receive_bytes: u64,
    #[serde(default)]
    pub frame_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_reason: Option<String>,
}

impl Default for SocketStatus {
    fn default() -> Self {
        Self {
            is_open: true,
            send_count: 0,
            receive_count: 0,
            send_bytes: 0,
            receive_bytes: 0,
            frame_count: 0,
            close_code: None,
            close_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedRule {
    pub pattern: String,
    pub protocol: String,
    pub value: String,
    pub rule_name: Option<String>,
    pub raw: Option<String>,
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RequestTiming {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connect_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receive_ms: Option<u64>,
    pub total_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficRecord {
    pub id: String,
    #[serde(default)]
    pub sequence: u64,
    pub timestamp: u64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub request_size: usize,
    pub response_size: usize,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<RequestTiming>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_headers: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_headers: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body_ref: Option<BodyRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body_ref: Option<BodyRef>,
    pub client_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_path: Option<String>,
    pub host: String,
    pub path: String,
    pub protocol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_request_headers: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_response_headers: Option<Vec<(String, String)>>,
    #[serde(default)]
    pub is_tunnel: bool,
    #[serde(default)]
    pub has_rule_hit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_rules: Option<Vec<MatchedRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_content_type: Option<String>,
    #[serde(default)]
    pub is_websocket: bool,
    #[serde(default)]
    pub is_sse: bool,
    #[serde(default)]
    pub is_h3: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_status: Option<SocketStatus>,
    #[serde(default)]
    pub frame_count: usize,
    #[serde(default)]
    pub last_frame_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_script_results: Option<Vec<ScriptExecutionResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub res_script_results: Option<Vec<ScriptExecutionResult>>,
}

pub use bifrost_script::ScriptExecutionResult;

impl TrafficRecord {
    pub fn new(id: String, method: String, url: String) -> Self {
        let parsed_url = url::Url::parse(&url).ok();
        let host = parsed_url
            .as_ref()
            .and_then(|u| u.host_str())
            .unwrap_or("")
            .to_string();
        let path = parsed_url
            .as_ref()
            .map(|u| u.path().to_string())
            .unwrap_or_default();
        let protocol = parsed_url
            .as_ref()
            .map(|u| u.scheme().to_string())
            .unwrap_or_default();

        Self {
            id,
            sequence: 0,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            method,
            url,
            status: 0,
            content_type: None,
            request_size: 0,
            response_size: 0,
            duration_ms: 0,
            timing: None,
            request_headers: None,
            response_headers: None,
            request_body_ref: None,
            response_body_ref: None,
            client_ip: String::new(),
            client_app: None,
            client_pid: None,
            client_path: None,
            host,
            path,
            protocol,
            actual_url: None,
            actual_host: None,
            original_request_headers: None,
            actual_response_headers: None,
            is_tunnel: false,
            has_rule_hit: false,
            matched_rules: None,
            request_content_type: None,
            is_websocket: false,
            is_sse: false,
            is_h3: false,
            socket_status: None,
            frame_count: 0,
            last_frame_id: 0,
            error_message: None,
            req_script_results: None,
            res_script_results: None,
        }
    }

    pub fn set_h3(&mut self) {
        self.is_h3 = true;
    }

    pub fn set_websocket(&mut self) {
        self.is_websocket = true;
        self.socket_status = Some(SocketStatus::default());
    }

    pub fn set_sse(&mut self) {
        self.is_sse = true;
        self.socket_status = Some(SocketStatus::default());
    }

    pub fn set_streaming(&mut self) {
        self.socket_status = Some(SocketStatus::default());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSummary {
    pub id: String,
    #[serde(default)]
    pub sequence: u64,
    pub timestamp: u64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub request_size: usize,
    pub response_size: usize,
    pub duration_ms: u64,
    pub host: String,
    pub path: String,
    pub protocol: String,
    pub client_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_path: Option<String>,
    pub has_rule_hit: bool,
    pub matched_rule_count: usize,
    pub matched_protocols: Vec<String>,
    #[serde(default)]
    pub is_websocket: bool,
    #[serde(default)]
    pub is_sse: bool,
    #[serde(default)]
    pub is_h3: bool,
    #[serde(default)]
    pub is_tunnel: bool,
    #[serde(default)]
    pub frame_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_status: Option<SocketStatus>,
    pub start_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
}

fn format_timestamp_ms(timestamp_ms: u64) -> String {
    use chrono::{Local, TimeZone};
    let secs = (timestamp_ms / 1000) as i64;
    let nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;
    Local
        .timestamp_opt(secs, nanos)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
        .unwrap_or_else(|| "-".to_string())
}

impl From<&TrafficRecord> for TrafficSummary {
    fn from(record: &TrafficRecord) -> Self {
        let (matched_rule_count, matched_protocols) = if let Some(ref rules) = record.matched_rules
        {
            let protocols: Vec<String> = rules
                .iter()
                .map(|r| r.protocol.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            (rules.len(), protocols)
        } else {
            (0, Vec::new())
        };

        let start_time = format_timestamp_ms(record.timestamp);
        let end_time = if record.duration_ms > 0 {
            Some(format_timestamp_ms(record.timestamp + record.duration_ms))
        } else {
            None
        };

        Self {
            id: record.id.clone(),
            sequence: record.sequence,
            timestamp: record.timestamp,
            method: record.method.clone(),
            url: record.url.clone(),
            status: record.status,
            content_type: record.content_type.clone(),
            request_size: record.request_size,
            response_size: record.response_size,
            duration_ms: record.duration_ms,
            host: record.host.clone(),
            path: record.path.clone(),
            protocol: record.protocol.clone(),
            client_ip: record.client_ip.clone(),
            client_app: record.client_app.clone(),
            client_pid: record.client_pid,
            client_path: record.client_path.clone(),
            has_rule_hit: record.has_rule_hit,
            matched_rule_count,
            matched_protocols,
            is_websocket: record.is_websocket,
            is_sse: record.is_sse,
            is_h3: record.is_h3,
            is_tunnel: record.is_tunnel,
            frame_count: record.frame_count,
            socket_status: record.socket_status.clone(),
            start_time,
            end_time,
        }
    }
}

pub struct TrafficRecorder {
    records: RwLock<VecDeque<TrafficRecord>>,
    max_records: AtomicUsize,
    tx: broadcast::Sender<TrafficRecord>,
    sequence: AtomicU64,
}

impl TrafficRecorder {
    pub fn new(max_records: usize) -> Self {
        let (tx, _) = broadcast::channel(1000);
        let initial_capacity = max_records.min(1000);
        Self {
            records: RwLock::new(VecDeque::with_capacity(initial_capacity)),
            max_records: AtomicUsize::new(max_records),
            tx,
            sequence: AtomicU64::new(1),
        }
    }

    pub fn set_max_records(&self, max_records: usize) {
        let old = self.max_records.swap(max_records, Ordering::SeqCst);
        if old != max_records {
            tracing::info!(
                "TrafficRecorder config updated: max_records {} -> {}",
                old,
                max_records
            );
        }
    }

    pub fn set_initial_sequence(&self, sequence: u64) {
        let old = self.sequence.swap(sequence, Ordering::SeqCst);
        tracing::info!(
            "TrafficRecorder sequence initialized: {} -> {}",
            old,
            sequence
        );
    }

    pub fn record(&self, mut record: TrafficRecord) {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        record.sequence = seq;

        let _ = self.tx.send(record.clone());

        let max = self.max_records.load(Ordering::Relaxed);
        let mut records = self.records.write();
        if records.len() >= max {
            records.pop_front();
        }
        records.push_back(record);
    }

    pub fn get_all(&self) -> Vec<TrafficSummary> {
        self.records
            .read()
            .iter()
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn get_recent(&self, limit: usize) -> Vec<TrafficSummary> {
        self.records
            .read()
            .iter()
            .rev()
            .take(limit)
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn get_by_id(&self, id: &str) -> Option<TrafficRecord> {
        self.records.read().iter().find(|r| r.id == id).cloned()
    }

    pub fn update_by_id<F>(&self, id: &str, updater: F) -> bool
    where
        F: FnOnce(&mut TrafficRecord),
    {
        let mut records = self.records.write();
        if let Some(record) = records.iter_mut().find(|r| r.id == id) {
            updater(record);
            true
        } else {
            false
        }
    }

    pub fn clear(&self) {
        self.records.write().clear();
        self.sequence.store(1, Ordering::SeqCst);
    }

    pub fn count(&self) -> usize {
        self.records.read().len()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TrafficRecord> {
        self.tx.subscribe()
    }

    pub fn filter(&self, filter: &TrafficFilter) -> Vec<TrafficSummary> {
        self.records
            .read()
            .iter()
            .filter(|r| filter.matches(r))
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn get_after(
        &self,
        after_id: Option<&str>,
        filter: &TrafficFilter,
        limit: usize,
    ) -> (Vec<TrafficSummary>, bool) {
        let records = self.records.read();

        let start_idx = if let Some(after_id) = after_id {
            records
                .iter()
                .position(|r| r.id == after_id)
                .map(|idx| idx + 1)
                .unwrap_or(0)
        } else {
            0
        };

        let filtered: Vec<TrafficSummary> = records
            .iter()
            .skip(start_idx)
            .filter(|r| filter.matches(r))
            .map(TrafficSummary::from)
            .collect();

        let total = filtered.len();
        let has_more = total > limit;
        let result = filtered.into_iter().take(limit).collect();

        (result, has_more)
    }

    pub fn get_by_ids(&self, ids: &[&str]) -> Vec<TrafficSummary> {
        let records = self.records.read();
        ids.iter()
            .filter_map(|id| records.iter().find(|r| r.id == *id))
            .map(TrafficSummary::from)
            .collect()
    }

    pub fn total(&self) -> usize {
        self.records.read().len()
    }
}

impl Default for TrafficRecorder {
    fn default() -> Self {
        Self::new(10000)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TrafficFilter {
    pub method: Option<String>,
    pub status: Option<u16>,
    pub status_min: Option<u16>,
    pub status_max: Option<u16>,
    pub url_contains: Option<String>,
    pub host: Option<String>,
    pub content_type: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub has_rule_hit: Option<bool>,
    pub protocol: Option<String>,
    pub request_content_type: Option<String>,
    pub domain: Option<String>,
    pub path_contains: Option<String>,
    pub header_contains: Option<String>,
    pub client_ip: Option<String>,
    pub client_app: Option<String>,
    pub is_h3: Option<bool>,
    pub is_websocket: Option<bool>,
    pub is_sse: Option<bool>,
}

impl TrafficFilter {
    pub fn matches(&self, record: &TrafficRecord) -> bool {
        if let Some(ref method) = self.method {
            if !record.method.eq_ignore_ascii_case(method) {
                return false;
            }
        }

        if let Some(status) = self.status {
            if record.status != status {
                return false;
            }
        }

        if let Some(min) = self.status_min {
            if record.status < min {
                return false;
            }
        }

        if let Some(max) = self.status_max {
            if record.status > max {
                return false;
            }
        }

        if let Some(ref url_contains) = self.url_contains {
            if !record
                .url
                .to_lowercase()
                .contains(&url_contains.to_lowercase())
            {
                return false;
            }
        }

        if let Some(ref host) = self.host {
            if !record.host.to_lowercase().contains(&host.to_lowercase()) {
                return false;
            }
        }

        if let Some(ref content_type) = self.content_type {
            if let Some(ref ct) = record.content_type {
                if !ct.to_lowercase().contains(&content_type.to_lowercase()) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(has_rule_hit) = self.has_rule_hit {
            if record.has_rule_hit != has_rule_hit {
                return false;
            }
        }

        if let Some(ref protocol) = self.protocol {
            if !record.protocol.eq_ignore_ascii_case(protocol) {
                return false;
            }
        }

        if let Some(ref request_ct) = self.request_content_type {
            if let Some(ref ct) = record.request_content_type {
                if !ct.to_lowercase().contains(&request_ct.to_lowercase()) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(ref domain) = self.domain {
            if !record.host.to_lowercase().contains(&domain.to_lowercase()) {
                return false;
            }
        }

        if let Some(ref path_contains) = self.path_contains {
            if !record
                .path
                .to_lowercase()
                .contains(&path_contains.to_lowercase())
            {
                return false;
            }
        }

        if let Some(ref header_contains) = self.header_contains {
            let search = header_contains.to_lowercase();
            let mut found = false;
            if let Some(ref headers) = record.request_headers {
                for (k, v) in headers {
                    if k.to_lowercase().contains(&search) || v.to_lowercase().contains(&search) {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                if let Some(ref headers) = record.response_headers {
                    for (k, v) in headers {
                        if k.to_lowercase().contains(&search) || v.to_lowercase().contains(&search)
                        {
                            found = true;
                            break;
                        }
                    }
                }
            }
            if !found {
                return false;
            }
        }

        if let Some(ref client_ip) = self.client_ip {
            if !record.client_ip.contains(client_ip) {
                return false;
            }
        }

        if let Some(ref client_app) = self.client_app {
            if let Some(ref app) = record.client_app {
                if !app.to_lowercase().contains(&client_app.to_lowercase()) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(is_h3) = self.is_h3 {
            if record.is_h3 != is_h3 {
                return false;
            }
        }

        if let Some(is_websocket) = self.is_websocket {
            if record.is_websocket != is_websocket {
                return false;
            }
        }

        if let Some(is_sse) = self.is_sse {
            if record.is_sse != is_sse {
                return false;
            }
        }

        true
    }
}

pub type SharedTrafficRecorder = Arc<TrafficRecorder>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traffic_record_new() {
        let record = TrafficRecord::new(
            "test-id".to_string(),
            "GET".to_string(),
            "https://example.com/api/test".to_string(),
        );

        assert_eq!(record.id, "test-id");
        assert_eq!(record.method, "GET");
        assert_eq!(record.host, "example.com");
        assert_eq!(record.path, "/api/test");
        assert_eq!(record.protocol, "https");
    }

    #[test]
    fn test_traffic_recorder() {
        let recorder = TrafficRecorder::new(100);

        let record = TrafficRecord::new(
            "1".to_string(),
            "GET".to_string(),
            "https://example.com".to_string(),
        );
        recorder.record(record);

        assert_eq!(recorder.count(), 1);
        assert!(recorder.get_by_id("1").is_some());
        assert!(recorder.get_by_id("2").is_none());
    }

    #[test]
    fn test_traffic_recorder_max_records() {
        let recorder = TrafficRecorder::new(3);

        for i in 0..5 {
            let record = TrafficRecord::new(
                i.to_string(),
                "GET".to_string(),
                "https://example.com".to_string(),
            );
            recorder.record(record);
        }

        assert_eq!(recorder.count(), 3);
        assert!(recorder.get_by_id("0").is_none());
        assert!(recorder.get_by_id("1").is_none());
        assert!(recorder.get_by_id("2").is_some());
    }

    #[test]
    fn test_traffic_filter() {
        let mut record = TrafficRecord::new(
            "1".to_string(),
            "POST".to_string(),
            "https://api.example.com/v1/users".to_string(),
        );
        record.status = 200;
        record.content_type = Some("application/json".to_string());

        let filter = TrafficFilter {
            method: Some("POST".to_string()),
            ..Default::default()
        };
        assert!(filter.matches(&record));

        let filter = TrafficFilter {
            method: Some("GET".to_string()),
            ..Default::default()
        };
        assert!(!filter.matches(&record));

        let filter = TrafficFilter {
            status: Some(200),
            ..Default::default()
        };
        assert!(filter.matches(&record));

        let filter = TrafficFilter {
            url_contains: Some("users".to_string()),
            ..Default::default()
        };
        assert!(filter.matches(&record));
    }

    #[test]
    fn test_traffic_recorder_sequence() {
        let recorder = TrafficRecorder::new(100);

        for i in 0..3 {
            let record = TrafficRecord::new(
                format!("id-{}", i),
                "GET".to_string(),
                "https://example.com".to_string(),
            );
            recorder.record(record);
        }

        let record1 = recorder.get_by_id("id-0").unwrap();
        let record2 = recorder.get_by_id("id-1").unwrap();
        let record3 = recorder.get_by_id("id-2").unwrap();

        assert_eq!(record1.sequence, 1);
        assert_eq!(record2.sequence, 2);
        assert_eq!(record3.sequence, 3);
    }

    #[test]
    fn test_traffic_recorder_sequence_reset_on_clear() {
        let recorder = TrafficRecorder::new(100);

        for i in 0..3 {
            let record = TrafficRecord::new(
                format!("id-{}", i),
                "GET".to_string(),
                "https://example.com".to_string(),
            );
            recorder.record(record);
        }

        assert_eq!(recorder.get_by_id("id-2").unwrap().sequence, 3);

        recorder.clear();

        let record = TrafficRecord::new(
            "new-id".to_string(),
            "GET".to_string(),
            "https://example.com".to_string(),
        );
        recorder.record(record);

        assert_eq!(recorder.get_by_id("new-id").unwrap().sequence, 1);
    }

    #[test]
    fn test_traffic_summary_includes_sequence() {
        let recorder = TrafficRecorder::new(100);

        let record = TrafficRecord::new(
            "test-id".to_string(),
            "GET".to_string(),
            "https://example.com".to_string(),
        );
        recorder.record(record);

        let summaries = recorder.get_all();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].sequence, 1);
    }
}

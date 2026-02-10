use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::body_store::BodyRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedRule {
    pub pattern: String,
    pub protocol: String,
    pub value: String,
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
    pub host: String,
    pub path: String,
    pub protocol: String,
    #[serde(default)]
    pub is_tunnel: bool,
    #[serde(default)]
    pub has_rule_hit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_rules: Option<Vec<MatchedRule>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_content_type: Option<String>,
}

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
            host,
            path,
            protocol,
            is_tunnel: false,
            has_rule_hit: false,
            matched_rules: None,
            request_content_type: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSummary {
    pub id: String,
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
    pub has_rule_hit: bool,
    pub matched_rule_count: usize,
    pub matched_protocols: Vec<String>,
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

        Self {
            id: record.id.clone(),
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
            has_rule_hit: record.has_rule_hit,
            matched_rule_count,
            matched_protocols,
        }
    }
}

pub struct TrafficRecorder {
    records: RwLock<VecDeque<TrafficRecord>>,
    max_records: usize,
    tx: broadcast::Sender<TrafficRecord>,
}

impl TrafficRecorder {
    pub fn new(max_records: usize) -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self {
            records: RwLock::new(VecDeque::with_capacity(max_records)),
            max_records,
            tx,
        }
    }

    pub fn record(&self, record: TrafficRecord) {
        let _ = self.tx.send(record.clone());

        let mut records = self.records.write();
        if records.len() >= self.max_records {
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

    pub fn clear(&self) {
        self.records.write().clear();
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
}

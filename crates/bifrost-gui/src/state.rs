use std::collections::VecDeque;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProxyStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl std::fmt::Display for ProxyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyStatus::Stopped => write!(f, "Stopped"),
            ProxyStatus::Starting => write!(f, "Starting..."),
            ProxyStatus::Running => write!(f, "Running"),
            ProxyStatus::Stopping => write!(f, "Stopping..."),
            ProxyStatus::Error => write!(f, "Error"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySettings {
    pub port: u16,
    pub host: String,
    pub socks5_port: Option<u16>,
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub unsafe_ssl: bool,
    pub allow_lan: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            port: 8899,
            host: "0.0.0.0".to_string(),
            socks5_port: None,
            enable_tls_interception: true,
            intercept_exclude: Vec::new(),
            unsafe_ssl: false,
            allow_lan: false,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TrafficEntry {
    pub id: u64,
    pub timestamp: DateTime<Local>,
    pub method: String,
    pub url: String,
    pub status: Option<u16>,
    pub duration_ms: Option<u64>,
    pub size: Option<u64>,
    pub content_type: Option<String>,
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub active_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub requests_per_second: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub name: String,
    pub enabled: bool,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub ip_or_cidr: String,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub proxy_status: ProxyStatus,
    pub settings: ProxySettings,
    pub metrics: MetricsSnapshot,
    pub traffic: VecDeque<TrafficEntry>,
    pub rules: Vec<RuleEntry>,
    pub whitelist: Vec<WhitelistEntry>,
    pub error_message: Option<String>,
    pub ca_installed: Option<bool>,
    pub started_at: Option<DateTime<Local>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            traffic: VecDeque::with_capacity(1000),
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn add_traffic(&mut self, entry: TrafficEntry) {
        if self.traffic.len() >= 1000 {
            self.traffic.pop_front();
        }
        self.traffic.push_back(entry);
    }

    pub fn clear_traffic(&mut self) {
        self.traffic.clear();
    }

    pub fn uptime(&self) -> Option<String> {
        self.started_at.map(|start| {
            let duration = Local::now().signed_duration_since(start);
            let hours = duration.num_hours();
            let minutes = duration.num_minutes() % 60;
            let seconds = duration.num_seconds() % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        })
    }
}

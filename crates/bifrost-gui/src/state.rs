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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrafficDetailTab {
    #[default]
    Overview,
    Headers,
    Body,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsInterceptMode {
    #[default]
    Blacklist,
    Whitelist,
}

impl std::fmt::Display for TlsInterceptMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsInterceptMode::Blacklist => write!(f, "blacklist"),
            TlsInterceptMode::Whitelist => write!(f, "whitelist"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySettings {
    pub port: u16,
    pub host: String,
    pub socks5_port: Option<u16>,
    pub enable_tls_interception: bool,
    pub intercept_mode: TlsInterceptMode,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
    pub allow_lan: bool,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            port: 9900,
            host: "0.0.0.0".to_string(),
            socks5_port: None,
            enable_tls_interception: true,
            intercept_mode: TlsInterceptMode::default(),
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            unsafe_ssl: false,
            allow_lan: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficHeaders {
    pub entries: Vec<(String, String)>,
}

impl TrafficHeaders {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficRequest {
    pub method: String,
    pub url: String,
    pub http_version: String,
    pub headers: TrafficHeaders,
    pub body: Option<String>,
    pub body_size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrafficResponse {
    pub status_code: u16,
    pub status_message: String,
    pub http_version: String,
    pub headers: TrafficHeaders,
    pub body: Option<String>,
    pub body_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrafficProtocol {
    #[default]
    Http,
    Https,
    Ws,
    Wss,
    Tunnel,
}

impl std::fmt::Display for TrafficProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrafficProtocol::Http => write!(f, "HTTP"),
            TrafficProtocol::Https => write!(f, "HTTPS"),
            TrafficProtocol::Ws => write!(f, "WS"),
            TrafficProtocol::Wss => write!(f, "WSS"),
            TrafficProtocol::Tunnel => write!(f, "TUNNEL"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TrafficStatus {
    #[default]
    Pending,
    Complete,
    Error,
    Aborted,
}

#[derive(Debug, Clone)]
pub struct TrafficEntry {
    pub id: u64,
    pub timestamp: DateTime<Local>,
    pub protocol: TrafficProtocol,
    pub status: TrafficStatus,
    pub host: String,
    pub path: String,
    pub request: TrafficRequest,
    pub response: Option<TrafficResponse>,
    pub duration_ms: Option<u64>,
    pub matched_rules: Vec<String>,
    pub client_ip: String,
}

impl TrafficEntry {
    pub fn full_url(&self) -> String {
        format!("{}{}", self.host, self.path)
    }

    pub fn status_code(&self) -> Option<u16> {
        self.response.as_ref().map(|r| r.status_code)
    }

    pub fn content_type(&self) -> Option<&str> {
        self.response.as_ref()?.headers.get("content-type")
    }

    pub fn response_size(&self) -> Option<u64> {
        self.response.as_ref().map(|r| r.body_size)
    }

    pub fn method(&self) -> &str {
        &self.request.method
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProtocolMetrics {
    pub requests: u64,
    pub connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub active_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub requests_per_second: f64,
    pub http: ProtocolMetrics,
    pub https: ProtocolMetrics,
    pub ws: ProtocolMetrics,
    pub wss: ProtocolMetrics,
    pub tunnel: ProtocolMetrics,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub memory_bytes: u64,
    pub upload_speed: f64,
    pub download_speed: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct MetricsHistory {
    pub timestamps: VecDeque<DateTime<Local>>,
    pub cpu_usage: VecDeque<f64>,
    pub memory_usage: VecDeque<f64>,
    pub qps: VecDeque<f64>,
    pub upload_speed: VecDeque<f64>,
    pub download_speed: VecDeque<f64>,
}

impl MetricsHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            timestamps: VecDeque::with_capacity(capacity),
            cpu_usage: VecDeque::with_capacity(capacity),
            memory_usage: VecDeque::with_capacity(capacity),
            qps: VecDeque::with_capacity(capacity),
            upload_speed: VecDeque::with_capacity(capacity),
            download_speed: VecDeque::with_capacity(capacity),
        }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, metrics: &MetricsSnapshot) {
        const MAX_HISTORY: usize = 60;
        if self.timestamps.len() >= MAX_HISTORY {
            self.timestamps.pop_front();
            self.cpu_usage.pop_front();
            self.memory_usage.pop_front();
            self.qps.pop_front();
            self.upload_speed.pop_front();
            self.download_speed.pop_front();
        }
        self.timestamps.push_back(Local::now());
        self.cpu_usage.push_back(metrics.cpu_usage);
        self.memory_usage.push_back(metrics.memory_usage);
        self.qps.push_back(metrics.requests_per_second);
        self.upload_speed.push_back(metrics.upload_speed);
        self.download_speed.push_back(metrics.download_speed);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterProtocol {
    #[default]
    All,
    Http,
    Https,
    Ws,
    Wss,
    Tunnel,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterStatus {
    #[default]
    All,
    Status1xx,
    Status2xx,
    Status3xx,
    Status4xx,
    Status5xx,
    Pending,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct TrafficFilter {
    pub protocol: FilterProtocol,
    pub status: FilterStatus,
    pub search_text: String,
    pub method_filter: Option<String>,
}

impl TrafficFilter {
    pub fn matches(&self, entry: &TrafficEntry) -> bool {
        if !matches!(self.protocol, FilterProtocol::All) {
            let protocol_matches = match self.protocol {
                FilterProtocol::Http => matches!(entry.protocol, TrafficProtocol::Http),
                FilterProtocol::Https => matches!(entry.protocol, TrafficProtocol::Https),
                FilterProtocol::Ws => matches!(entry.protocol, TrafficProtocol::Ws),
                FilterProtocol::Wss => matches!(entry.protocol, TrafficProtocol::Wss),
                FilterProtocol::Tunnel => matches!(entry.protocol, TrafficProtocol::Tunnel),
                FilterProtocol::All => true,
            };
            if !protocol_matches {
                return false;
            }
        }

        if !matches!(self.status, FilterStatus::All) {
            let status_matches = match self.status {
                FilterStatus::Pending => matches!(entry.status, TrafficStatus::Pending),
                FilterStatus::Error => matches!(entry.status, TrafficStatus::Error),
                FilterStatus::Status1xx => {
                    entry.status_code().is_some_and(|s| (100..200).contains(&s))
                }
                FilterStatus::Status2xx => {
                    entry.status_code().is_some_and(|s| (200..300).contains(&s))
                }
                FilterStatus::Status3xx => {
                    entry.status_code().is_some_and(|s| (300..400).contains(&s))
                }
                FilterStatus::Status4xx => {
                    entry.status_code().is_some_and(|s| (400..500).contains(&s))
                }
                FilterStatus::Status5xx => entry.status_code().is_some_and(|s| s >= 500),
                FilterStatus::All => true,
            };
            if !status_matches {
                return false;
            }
        }

        if !self.search_text.is_empty() {
            let search_lower = self.search_text.to_lowercase();
            let url_matches = entry.full_url().to_lowercase().contains(&search_lower);
            let method_matches = entry.method().to_lowercase().contains(&search_lower);
            let host_matches = entry.host.to_lowercase().contains(&search_lower);
            if !url_matches && !method_matches && !host_matches {
                return false;
            }
        }

        if let Some(ref method) = self.method_filter {
            if !entry.method().eq_ignore_ascii_case(method) {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrafficViewState {
    pub selected_id: Option<u64>,
    pub filter: TrafficFilter,
    pub detail_tab: TrafficDetailTab,
    pub paused: bool,
    pub show_detail_panel: bool,
    pub detail_panel_ratio: f32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AccessMode {
    #[default]
    AllowAll,
    LocalOnly,
    Whitelist,
    Interactive,
}

impl std::fmt::Display for AccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessMode::AllowAll => write!(f, "Allow All"),
            AccessMode::LocalOnly => write!(f, "Local Only"),
            AccessMode::Whitelist => write!(f, "Whitelist"),
            AccessMode::Interactive => write!(f, "Interactive"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WhitelistState {
    pub access_mode: AccessMode,
    pub permanent: Vec<WhitelistEntry>,
    pub temporary: Vec<WhitelistEntry>,
    pub pending_authorization: Vec<String>,
    pub allow_lan: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub proxy_status: ProxyStatus,
    pub settings: ProxySettings,
    pub metrics: MetricsSnapshot,
    pub metrics_history: MetricsHistory,
    pub traffic: VecDeque<TrafficEntry>,
    pub traffic_view: TrafficViewState,
    pub rules: Vec<RuleEntry>,
    pub values: Vec<ValueEntry>,
    pub whitelist: WhitelistState,
    pub error_message: Option<String>,
    pub ca_installed: Option<bool>,
    pub started_at: Option<DateTime<Local>>,
    pub system_proxy_enabled: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            traffic: VecDeque::with_capacity(1000),
            metrics_history: MetricsHistory::new(60),
            traffic_view: TrafficViewState {
                detail_panel_ratio: 0.5,
                show_detail_panel: true,
                ..Default::default()
            },
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
        self.traffic_view.selected_id = None;
    }

    pub fn get_selected_traffic(&self) -> Option<&TrafficEntry> {
        self.traffic_view
            .selected_id
            .and_then(|id| self.traffic.iter().find(|e| e.id == id))
    }

    pub fn filtered_traffic(&self) -> impl Iterator<Item = &TrafficEntry> {
        self.traffic
            .iter()
            .filter(|e| self.traffic_view.filter.matches(e))
    }

    #[allow(dead_code)]
    pub fn update_metrics(&mut self, metrics: MetricsSnapshot) {
        self.metrics_history.push(&metrics);
        self.metrics = metrics;
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

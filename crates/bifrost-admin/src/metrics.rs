use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrafficType {
    Http,
    Https,
    Tunnel,
    Ws,
    Wss,
    H3,
    Socks5,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrafficTypeMetrics {
    pub requests: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub active_connections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: u64,
    pub memory_used: u64,
    pub memory_total: u64,
    pub cpu_usage: f32,
    pub total_requests: u64,
    pub active_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub bytes_sent_rate: f32,
    pub bytes_received_rate: f32,
    pub qps: f32,
    pub max_qps: f32,
    pub max_bytes_sent_rate: f32,
    pub max_bytes_received_rate: f32,
    pub http: TrafficTypeMetrics,
    pub https: TrafficTypeMetrics,
    pub tunnel: TrafficTypeMetrics,
    pub ws: TrafficTypeMetrics,
    pub wss: TrafficTypeMetrics,
    pub h3: TrafficTypeMetrics,
    pub socks5: TrafficTypeMetrics,
}

#[derive(Default)]
struct TrafficTypeCounters {
    requests: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    active_connections: AtomicU64,
}

impl TrafficTypeCounters {
    fn new() -> Self {
        Self::default()
    }

    fn to_metrics(&self) -> TrafficTypeMetrics {
        TrafficTypeMetrics {
            requests: self.requests.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
        }
    }
}

pub struct MetricsCollector {
    total_requests: AtomicU64,
    active_connections: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    history: RwLock<VecDeque<MetricsSnapshot>>,
    max_history: usize,
    last_request_count: AtomicU64,
    last_bytes_sent: AtomicU64,
    last_bytes_received: AtomicU64,
    last_snapshot_time: AtomicU64,
    realtime_last_request_count: AtomicU64,
    realtime_last_bytes_sent: AtomicU64,
    realtime_last_bytes_received: AtomicU64,
    realtime_last_time: AtomicU64,
    smoothed_qps: RwLock<f32>,
    smoothed_bytes_sent_rate: RwLock<f32>,
    smoothed_bytes_received_rate: RwLock<f32>,
    system: RwLock<System>,
    pid: Pid,
    max_qps: RwLock<f32>,
    max_bytes_sent_rate: RwLock<f32>,
    max_bytes_received_rate: RwLock<f32>,
    http: TrafficTypeCounters,
    https: TrafficTypeCounters,
    tunnel: TrafficTypeCounters,
    ws: TrafficTypeCounters,
    wss: TrafficTypeCounters,
    h3: TrafficTypeCounters,
    socks5: TrafficTypeCounters,
}

impl MetricsCollector {
    pub fn new(max_history: usize) -> Self {
        let system = System::new_all();
        let pid = Pid::from_u32(std::process::id());
        Self {
            total_requests: AtomicU64::new(0),
            active_connections: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            history: RwLock::new(VecDeque::with_capacity(max_history)),
            max_history,
            last_request_count: AtomicU64::new(0),
            last_bytes_sent: AtomicU64::new(0),
            last_bytes_received: AtomicU64::new(0),
            last_snapshot_time: AtomicU64::new(0),
            realtime_last_request_count: AtomicU64::new(0),
            realtime_last_bytes_sent: AtomicU64::new(0),
            realtime_last_bytes_received: AtomicU64::new(0),
            realtime_last_time: AtomicU64::new(0),
            smoothed_qps: RwLock::new(0.0),
            smoothed_bytes_sent_rate: RwLock::new(0.0),
            smoothed_bytes_received_rate: RwLock::new(0.0),
            system: RwLock::new(system),
            pid,
            max_qps: RwLock::new(0.0),
            max_bytes_sent_rate: RwLock::new(0.0),
            max_bytes_received_rate: RwLock::new(0.0),
            http: TrafficTypeCounters::new(),
            https: TrafficTypeCounters::new(),
            tunnel: TrafficTypeCounters::new(),
            ws: TrafficTypeCounters::new(),
            wss: TrafficTypeCounters::new(),
            h3: TrafficTypeCounters::new(),
            socks5: TrafficTypeCounters::new(),
        }
    }

    fn get_counters(&self, traffic_type: TrafficType) -> &TrafficTypeCounters {
        match traffic_type {
            TrafficType::Http => &self.http,
            TrafficType::Https => &self.https,
            TrafficType::Tunnel => &self.tunnel,
            TrafficType::Ws => &self.ws,
            TrafficType::Wss => &self.wss,
            TrafficType::H3 => &self.h3,
            TrafficType::Socks5 => &self.socks5,
        }
    }

    pub fn increment_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_requests_by_type(&self, traffic_type: TrafficType) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.get_counters(traffic_type)
            .requests
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_connections_by_type(&self, traffic_type: TrafficType) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.get_counters(traffic_type)
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn decrement_connections_by_type(&self, traffic_type: TrafficType) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        self.get_counters(traffic_type)
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);
    }

    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_sent_by_type(&self, traffic_type: TrafficType, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        self.get_counters(traffic_type)
            .bytes_sent
            .fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_received_by_type(&self, traffic_type: TrafficType, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
        self.get_counters(traffic_type)
            .bytes_received
            .fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn get_current(&self) -> MetricsSnapshot {
        let mut system = self.system.write();
        system.refresh_processes(ProcessesToUpdate::Some(&[self.pid]));

        let (memory_used, cpu_usage) = if let Some(process) = system.process(self.pid) {
            (process.memory(), process.cpu_usage())
        } else {
            (0, 0.0)
        };

        let memory_total = system.total_memory();

        let now = chrono::Utc::now().timestamp_millis() as u64;
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        let bytes_received = self.bytes_received.load(Ordering::Relaxed);

        let realtime_last_count = self.realtime_last_request_count.load(Ordering::Relaxed);
        let realtime_last_bytes_sent = self.realtime_last_bytes_sent.load(Ordering::Relaxed);
        let realtime_last_bytes_received =
            self.realtime_last_bytes_received.load(Ordering::Relaxed);
        let realtime_last_time = self.realtime_last_time.load(Ordering::Relaxed);

        let min_update_interval_ms: u64 = 500;
        let elapsed_since_last = now.saturating_sub(realtime_last_time);
        let should_update_realtime = elapsed_since_last >= min_update_interval_ms;

        let (raw_qps, raw_bytes_sent_rate, raw_bytes_received_rate) =
            if realtime_last_time > 0 && elapsed_since_last > 0 {
                let elapsed_secs = elapsed_since_last as f32 / 1000.0;
                if elapsed_secs > 0.0 {
                    (
                        (total_requests.saturating_sub(realtime_last_count)) as f32 / elapsed_secs,
                        (bytes_sent.saturating_sub(realtime_last_bytes_sent)) as f32 / elapsed_secs,
                        (bytes_received.saturating_sub(realtime_last_bytes_received)) as f32
                            / elapsed_secs,
                    )
                } else {
                    (0.0, 0.0, 0.0)
                }
            } else {
                (0.0, 0.0, 0.0)
            };

        let smoothing_alpha: f32 = 0.4;
        let decay_alpha: f32 = 0.85;

        let (qps, bytes_sent_rate, bytes_received_rate) = if should_update_realtime {
            let mut smoothed_qps = self.smoothed_qps.write();
            let mut smoothed_sent = self.smoothed_bytes_sent_rate.write();
            let mut smoothed_recv = self.smoothed_bytes_received_rate.write();

            if raw_qps > 0.0 || raw_bytes_sent_rate > 0.0 || raw_bytes_received_rate > 0.0 {
                *smoothed_qps = smoothing_alpha * raw_qps + (1.0 - smoothing_alpha) * *smoothed_qps;
                *smoothed_sent = smoothing_alpha * raw_bytes_sent_rate
                    + (1.0 - smoothing_alpha) * *smoothed_sent;
                *smoothed_recv = smoothing_alpha * raw_bytes_received_rate
                    + (1.0 - smoothing_alpha) * *smoothed_recv;
            } else {
                *smoothed_qps *= decay_alpha;
                *smoothed_sent *= decay_alpha;
                *smoothed_recv *= decay_alpha;

                if *smoothed_qps < 0.01 {
                    *smoothed_qps = 0.0;
                }
                if *smoothed_sent < 1.0 {
                    *smoothed_sent = 0.0;
                }
                if *smoothed_recv < 1.0 {
                    *smoothed_recv = 0.0;
                }
            }

            self.realtime_last_request_count
                .store(total_requests, Ordering::Relaxed);
            self.realtime_last_bytes_sent
                .store(bytes_sent, Ordering::Relaxed);
            self.realtime_last_bytes_received
                .store(bytes_received, Ordering::Relaxed);
            self.realtime_last_time.store(now, Ordering::Relaxed);

            (*smoothed_qps, *smoothed_sent, *smoothed_recv)
        } else {
            let smoothed_qps = *self.smoothed_qps.read();
            let smoothed_sent = *self.smoothed_bytes_sent_rate.read();
            let smoothed_recv = *self.smoothed_bytes_received_rate.read();
            (smoothed_qps, smoothed_sent, smoothed_recv)
        };

        let max_qps = *self.max_qps.read();
        let max_bytes_sent_rate = *self.max_bytes_sent_rate.read();
        let max_bytes_received_rate = *self.max_bytes_received_rate.read();

        MetricsSnapshot {
            timestamp: now,
            memory_used,
            memory_total,
            cpu_usage,
            total_requests,
            active_connections: self.active_connections.load(Ordering::Relaxed),
            bytes_sent,
            bytes_received,
            bytes_sent_rate,
            bytes_received_rate,
            qps,
            max_qps,
            max_bytes_sent_rate,
            max_bytes_received_rate,
            http: self.http.to_metrics(),
            https: self.https.to_metrics(),
            tunnel: self.tunnel.to_metrics(),
            ws: self.ws.to_metrics(),
            wss: self.wss.to_metrics(),
            h3: self.h3.to_metrics(),
            socks5: self.socks5.to_metrics(),
        }
    }

    pub fn take_snapshot(&self) -> MetricsSnapshot {
        let mut snapshot = self.get_current();

        self.last_request_count
            .store(snapshot.total_requests, Ordering::Relaxed);
        self.last_bytes_sent
            .store(snapshot.bytes_sent, Ordering::Relaxed);
        self.last_bytes_received
            .store(snapshot.bytes_received, Ordering::Relaxed);
        self.last_snapshot_time
            .store(snapshot.timestamp, Ordering::Relaxed);

        {
            let mut max_qps = self.max_qps.write();
            if snapshot.qps > *max_qps {
                *max_qps = snapshot.qps;
            }
            snapshot.max_qps = *max_qps;
        }

        {
            let mut max_sent = self.max_bytes_sent_rate.write();
            if snapshot.bytes_sent_rate > *max_sent {
                *max_sent = snapshot.bytes_sent_rate;
            }
            snapshot.max_bytes_sent_rate = *max_sent;
        }

        {
            let mut max_recv = self.max_bytes_received_rate.write();
            if snapshot.bytes_received_rate > *max_recv {
                *max_recv = snapshot.bytes_received_rate;
            }
            snapshot.max_bytes_received_rate = *max_recv;
        }

        let mut history = self.history.write();
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(snapshot.clone());

        snapshot
    }

    pub fn get_history(&self, limit: Option<usize>) -> Vec<MetricsSnapshot> {
        let history = self.history.read();
        match limit {
            Some(n) => history.iter().rev().take(n).cloned().collect(),
            None => history.iter().cloned().collect(),
        }
    }

    pub fn get_total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    pub fn get_active_connections(&self) -> u64 {
        self.active_connections.load(Ordering::Relaxed)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(3600)
    }
}

pub type SharedMetricsCollector = Arc<MetricsCollector>;

pub fn start_metrics_collector_task(
    collector: SharedMetricsCollector,
    interval_secs: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            collector.take_snapshot();
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub rust_version: String,
    pub os: String,
    pub arch: String,
    pub uptime_secs: u64,
    pub pid: u32,
}

impl SystemInfo {
    pub fn new(start_time: u64) -> Self {
        let now = chrono::Utc::now().timestamp() as u64;
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            rust_version: rustc_version(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            uptime_secs: now.saturating_sub(start_time),
            pid: std::process::id(),
        }
    }
}

fn rustc_version() -> String {
    option_env!("RUSTC_VERSION")
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new(100);

        assert_eq!(collector.get_total_requests(), 0);
        assert_eq!(collector.get_active_connections(), 0);

        collector.increment_requests();
        collector.increment_requests();
        assert_eq!(collector.get_total_requests(), 2);

        collector.increment_connections();
        assert_eq!(collector.get_active_connections(), 1);

        collector.decrement_connections();
        assert_eq!(collector.get_active_connections(), 0);
    }

    #[test]
    fn test_metrics_snapshot() {
        let collector = MetricsCollector::new(10);

        collector.increment_requests();
        collector.add_bytes_sent(100);
        collector.add_bytes_received(200);

        let snapshot = collector.take_snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.bytes_sent, 100);
        assert_eq!(snapshot.bytes_received, 200);
    }

    #[test]
    fn test_metrics_history() {
        let collector = MetricsCollector::new(3);

        for _ in 0..5 {
            collector.increment_requests();
            collector.take_snapshot();
        }

        let history = collector.get_history(None);
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_system_info() {
        let start_time = chrono::Utc::now().timestamp() as u64 - 60;
        let info = SystemInfo::new(start_time);

        assert!(!info.version.is_empty());
        assert!(!info.os.is_empty());
        assert!(info.uptime_secs >= 60);
    }

    #[test]
    fn test_traffic_type_metrics() {
        let collector = MetricsCollector::new(10);

        collector.increment_requests_by_type(TrafficType::Http);
        collector.increment_requests_by_type(TrafficType::Http);
        collector.increment_requests_by_type(TrafficType::Https);
        collector.add_bytes_sent_by_type(TrafficType::Http, 100);
        collector.add_bytes_received_by_type(TrafficType::Https, 200);

        let snapshot = collector.take_snapshot();
        assert_eq!(snapshot.total_requests, 3);
        assert_eq!(snapshot.http.requests, 2);
        assert_eq!(snapshot.https.requests, 1);
        assert_eq!(snapshot.http.bytes_sent, 100);
        assert_eq!(snapshot.https.bytes_received, 200);
    }

    #[test]
    fn test_connection_tracking_by_type() {
        let collector = MetricsCollector::new(10);

        collector.increment_connections_by_type(TrafficType::Ws);
        collector.increment_connections_by_type(TrafficType::Wss);
        collector.increment_connections_by_type(TrafficType::Tunnel);

        let snapshot = collector.get_current();
        assert_eq!(snapshot.active_connections, 3);
        assert_eq!(snapshot.ws.active_connections, 1);
        assert_eq!(snapshot.wss.active_connections, 1);
        assert_eq!(snapshot.tunnel.active_connections, 1);

        collector.decrement_connections_by_type(TrafficType::Ws);
        let snapshot = collector.get_current();
        assert_eq!(snapshot.active_connections, 2);
        assert_eq!(snapshot.ws.active_connections, 0);
    }
}

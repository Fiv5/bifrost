use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessesToUpdate, System};

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
    pub qps: f32,
}

pub struct MetricsCollector {
    total_requests: AtomicU64,
    active_connections: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    history: RwLock<VecDeque<MetricsSnapshot>>,
    max_history: usize,
    last_request_count: AtomicU64,
    last_snapshot_time: AtomicU64,
    system: RwLock<System>,
    pid: Pid,
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
            last_snapshot_time: AtomicU64::new(0),
            system: RwLock::new(system),
            pid,
        }
    }

    pub fn increment_requests(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
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
        let last_count = self.last_request_count.load(Ordering::Relaxed);
        let last_time = self.last_snapshot_time.load(Ordering::Relaxed);

        let qps = if last_time > 0 && now > last_time {
            let elapsed_secs = (now - last_time) as f32 / 1000.0;
            if elapsed_secs > 0.0 {
                (total_requests - last_count) as f32 / elapsed_secs
            } else {
                0.0
            }
        } else {
            0.0
        };

        MetricsSnapshot {
            timestamp: now,
            memory_used,
            memory_total,
            cpu_usage,
            total_requests,
            active_connections: self.active_connections.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            qps,
        }
    }

    pub fn take_snapshot(&self) -> MetricsSnapshot {
        let snapshot = self.get_current();

        self.last_request_count
            .store(snapshot.total_requests, Ordering::Relaxed);
        self.last_snapshot_time
            .store(snapshot.timestamp, Ordering::Relaxed);

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
}

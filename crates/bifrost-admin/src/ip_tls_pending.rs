use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::broadcast;
use tracing::info;

#[derive(Debug, Clone, Serialize)]
pub struct PendingIpTls {
    pub ip: String,
    pub first_seen: u64,
    pub attempt_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingIpTlsEvent {
    pub event_type: String,
    pub pending: PendingIpTls,
    pub total_pending: usize,
}

pub struct IpTlsPendingManager {
    pending: RwLock<Vec<(IpAddr, u64, u32)>>,
    session_decided: RwLock<HashSet<IpAddr>>,
    event_sender: broadcast::Sender<PendingIpTlsEvent>,
}

impl IpTlsPendingManager {
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(64);
        Self {
            pending: RwLock::new(Vec::new()),
            session_decided: RwLock::new(HashSet::new()),
            event_sender,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PendingIpTlsEvent> {
        self.event_sender.subscribe()
    }

    fn broadcast_event(&self, event_type: &str, pending: PendingIpTls, total_pending: usize) {
        let event = PendingIpTlsEvent {
            event_type: event_type.to_string(),
            pending,
            total_pending,
        };
        let _ = self.event_sender.send(event);
    }

    pub fn check_and_add_pending(&self, ip: IpAddr) -> bool {
        {
            let decided = self.session_decided.read().unwrap();
            if decided.contains(&ip) {
                return false;
            }
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut pending = self.pending.write().unwrap();

        if let Some(entry) = pending.iter_mut().find(|(addr, _, _)| *addr == ip) {
            entry.2 += 1;
            return false;
        }

        pending.push((ip, now, 1));
        let total_pending = pending.len();
        drop(pending);

        info!("New IP {} requires TLS interception decision", ip);

        let pending_info = PendingIpTls {
            ip: ip.to_string(),
            first_seen: now,
            attempt_count: 1,
        };
        self.broadcast_event("new", pending_info, total_pending);
        true
    }

    pub fn get_pending_list(&self) -> Vec<PendingIpTls> {
        let pending = self.pending.read().unwrap();
        pending
            .iter()
            .map(|(ip, first_seen, count)| PendingIpTls {
                ip: ip.to_string(),
                first_seen: *first_seen,
                attempt_count: *count,
            })
            .collect()
    }

    pub fn pending_count(&self) -> usize {
        let pending = self.pending.read().unwrap();
        pending.len()
    }

    pub fn approve(&self, ip: &IpAddr) -> bool {
        let mut pending = self.pending.write().unwrap();
        let removed_entry = pending.iter().find(|(addr, _, _)| addr == ip).cloned();

        if let Some((_, first_seen, attempt_count)) = removed_entry {
            pending.retain(|(addr, _, _)| addr != ip);
            let total_pending = pending.len();
            drop(pending);

            {
                let mut decided = self.session_decided.write().unwrap();
                decided.insert(*ip);
            }

            info!("Approved TLS interception for IP {}", ip);

            let pending_info = PendingIpTls {
                ip: ip.to_string(),
                first_seen,
                attempt_count,
            };
            self.broadcast_event("approved", pending_info, total_pending);
            true
        } else {
            false
        }
    }

    pub fn skip(&self, ip: &IpAddr) -> bool {
        let mut pending = self.pending.write().unwrap();
        let removed_entry = pending.iter().find(|(addr, _, _)| addr == ip).cloned();

        if let Some((_, first_seen, attempt_count)) = removed_entry {
            pending.retain(|(addr, _, _)| addr != ip);
            let total_pending = pending.len();
            drop(pending);

            {
                let mut decided = self.session_decided.write().unwrap();
                decided.insert(*ip);
            }

            info!("Skipped TLS interception for IP {}", ip);

            let pending_info = PendingIpTls {
                ip: ip.to_string(),
                first_seen,
                attempt_count,
            };
            self.broadcast_event("skipped", pending_info, total_pending);
            true
        } else {
            false
        }
    }

    pub fn clear_pending(&self) {
        let mut pending = self.pending.write().unwrap();
        pending.clear();
        info!("Cleared all pending IP TLS decisions");
    }

    pub fn is_pending_or_decided(&self, ip: &IpAddr) -> bool {
        let decided = self.session_decided.read().unwrap();
        if decided.contains(ip) {
            return true;
        }
        drop(decided);

        let pending = self.pending.read().unwrap();
        pending.iter().any(|(addr, _, _)| addr == ip)
    }
}

impl Default for IpTlsPendingManager {
    fn default() -> Self {
        Self::new()
    }
}

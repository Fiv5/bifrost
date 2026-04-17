use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::Serialize;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

const EVENT_CHANNEL_CAPACITY: usize = 64;
const MIN_SAMPLE_FOR_INFERENCE: u32 = 3;
const HIGH_UNTRUST_RATIO: f32 = 0.8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsAcceptFailureReason {
    ClientDoesNotTrustCa,
    ProbablyClientDoesNotTrustCa,
    CertificateExpired,
    ProtocolIncompatible,
    ConnectionReset,
    Unknown,
}

impl std::fmt::Display for TlsAcceptFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientDoesNotTrustCa => write!(f, "client_does_not_trust_ca"),
            Self::ProbablyClientDoesNotTrustCa => {
                write!(f, "probably_client_does_not_trust_ca")
            }
            Self::CertificateExpired => write!(f, "certificate_expired"),
            Self::ProtocolIncompatible => write!(f, "protocol_incompatible"),
            Self::ConnectionReset => write!(f, "connection_reset"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ClientTrustStatus {
    Trusted,
    NotTrusted { reason: String },
    LikelyUntrusted { confidence: f32, sample_count: u32 },
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ClientTrustRecord {
    pub first_seen: u64,
    pub last_seen: u64,
    pub last_success_at: Option<u64>,
    pub last_failure_at: Option<u64>,
    pub handshake_success: u32,
    pub handshake_fail_untrust: u32,
    pub handshake_fail_other: u32,
    pub last_failure_reason: Option<TlsAcceptFailureReason>,
    pub last_failure_domain: Option<String>,
}

impl ClientTrustRecord {
    fn new(now: u64) -> Self {
        Self {
            first_seen: now,
            last_seen: now,
            last_success_at: None,
            last_failure_at: None,
            handshake_success: 0,
            handshake_fail_untrust: 0,
            handshake_fail_other: 0,
            last_failure_reason: None,
            last_failure_domain: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientTrustSummary {
    pub identifier: String,
    pub identifier_type: String,
    pub trust_status: ClientTrustStatus,
    pub handshake_success: u32,
    pub handshake_fail_untrust: u32,
    pub handshake_fail_other: u32,
    pub first_seen: u64,
    pub last_seen: u64,
    pub last_failure_domain: Option<String>,
    pub last_failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientTrustEvent {
    pub identifier: String,
    pub identifier_type: String,
    pub old_status: String,
    pub new_status: String,
    pub reason: Option<String>,
    pub domain: Option<String>,
    pub timestamp: u64,
}

pub struct ClientTlsTrustTracker {
    by_ip: RwLock<HashMap<IpAddr, ClientTrustRecord>>,
    by_app: RwLock<HashMap<String, ClientTrustRecord>>,
    event_sender: broadcast::Sender<ClientTrustEvent>,
}

impl Default for ClientTlsTrustTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientTlsTrustTracker {
    pub fn new() -> Self {
        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            by_ip: RwLock::new(HashMap::new()),
            by_app: RwLock::new(HashMap::new()),
            event_sender,
        }
    }

    pub fn record_handshake_success(
        &self,
        client_ip: &str,
        client_app: Option<&str>,
        domain: &str,
    ) {
        let now = current_timestamp();
        let ctx = Some(("handshake_success".to_string(), domain.to_string()));

        if let Ok(ip) = client_ip.parse::<IpAddr>() {
            let mut map = self.by_ip.write();
            let record = map.entry(ip).or_insert_with(|| ClientTrustRecord::new(now));
            let old_status = evaluate_trust(record);
            record.last_seen = now;
            record.last_success_at = Some(now);
            record.handshake_success += 1;
            let new_status = evaluate_trust(record);
            self.maybe_emit_event(client_ip, "ip", &old_status, &new_status, ctx.clone());
        }

        if let Some(app) = client_app {
            if !app.is_empty() {
                let mut map = self.by_app.write();
                let record = map
                    .entry(app.to_string())
                    .or_insert_with(|| ClientTrustRecord::new(now));
                let old_status = evaluate_trust(record);
                record.last_seen = now;
                record.last_success_at = Some(now);
                record.handshake_success += 1;
                let new_status = evaluate_trust(record);
                self.maybe_emit_event(app, "app", &old_status, &new_status, ctx.clone());
            }
        }
    }

    pub fn record_handshake_failure(
        &self,
        client_ip: &str,
        client_app: Option<&str>,
        domain: &str,
        reason: &TlsAcceptFailureReason,
    ) {
        let now = current_timestamp();
        let is_untrust = matches!(
            reason,
            TlsAcceptFailureReason::ClientDoesNotTrustCa
                | TlsAcceptFailureReason::ProbablyClientDoesNotTrustCa
        );

        if let Ok(ip) = client_ip.parse::<IpAddr>() {
            let mut map = self.by_ip.write();
            let record = map.entry(ip).or_insert_with(|| ClientTrustRecord::new(now));
            let old_status = evaluate_trust(record);
            record.last_seen = now;
            record.last_failure_at = Some(now);
            record.last_failure_reason = Some(reason.clone());
            record.last_failure_domain = Some(domain.to_string());
            if is_untrust {
                record.handshake_fail_untrust += 1;
            } else {
                record.handshake_fail_other += 1;
            }
            let new_status = evaluate_trust(record);
            self.maybe_emit_event(
                client_ip,
                "ip",
                &old_status,
                &new_status,
                Some((reason.to_string(), domain.to_string())),
            );
        }

        if let Some(app) = client_app {
            if !app.is_empty() {
                let mut map = self.by_app.write();
                let record = map
                    .entry(app.to_string())
                    .or_insert_with(|| ClientTrustRecord::new(now));
                let old_status = evaluate_trust(record);
                record.last_seen = now;
                record.last_failure_at = Some(now);
                record.last_failure_reason = Some(reason.clone());
                record.last_failure_domain = Some(domain.to_string());
                if is_untrust {
                    record.handshake_fail_untrust += 1;
                } else {
                    record.handshake_fail_other += 1;
                }
                let new_status = evaluate_trust(record);
                self.maybe_emit_event(
                    app,
                    "app",
                    &old_status,
                    &new_status,
                    Some((reason.to_string(), domain.to_string())),
                );
            }
        }

        if is_untrust {
            warn!(
                client_ip = %client_ip,
                client_app = ?client_app,
                domain = %domain,
                reason = %reason,
                "TLS trust detection: client does not trust Bifrost CA"
            );
        } else {
            debug!(
                client_ip = %client_ip,
                client_app = ?client_app,
                domain = %domain,
                reason = %reason,
                "TLS handshake failure recorded"
            );
        }
    }

    pub fn get_trust_status_by_ip(&self, ip: &IpAddr) -> ClientTrustStatus {
        let map = self.by_ip.read();
        match map.get(ip) {
            Some(record) => evaluate_trust(record),
            None => ClientTrustStatus::Unknown,
        }
    }

    pub fn get_trust_status_by_app(&self, app: &str) -> ClientTrustStatus {
        let map = self.by_app.read();
        match map.get(app) {
            Some(record) => evaluate_trust(record),
            None => ClientTrustStatus::Unknown,
        }
    }

    pub fn get_all_statuses(&self) -> Vec<ClientTrustSummary> {
        let mut results = Vec::new();

        {
            let map = self.by_ip.read();
            for (ip, record) in map.iter() {
                results.push(ClientTrustSummary {
                    identifier: ip.to_string(),
                    identifier_type: "ip".to_string(),
                    trust_status: evaluate_trust(record),
                    handshake_success: record.handshake_success,
                    handshake_fail_untrust: record.handshake_fail_untrust,
                    handshake_fail_other: record.handshake_fail_other,
                    first_seen: record.first_seen,
                    last_seen: record.last_seen,
                    last_failure_domain: record.last_failure_domain.clone(),
                    last_failure_reason: record.last_failure_reason.as_ref().map(|r| r.to_string()),
                });
            }
        }

        {
            let map = self.by_app.read();
            for (app, record) in map.iter() {
                results.push(ClientTrustSummary {
                    identifier: app.clone(),
                    identifier_type: "app".to_string(),
                    trust_status: evaluate_trust(record),
                    handshake_success: record.handshake_success,
                    handshake_fail_untrust: record.handshake_fail_untrust,
                    handshake_fail_other: record.handshake_fail_other,
                    first_seen: record.first_seen,
                    last_seen: record.last_seen,
                    last_failure_domain: record.last_failure_domain.clone(),
                    last_failure_reason: record.last_failure_reason.as_ref().map(|r| r.to_string()),
                });
            }
        }

        results.sort_by_key(|a| std::cmp::Reverse(a.last_seen));
        results
    }

    pub fn get_untrusted_count(&self) -> usize {
        let mut count = 0;

        {
            let map = self.by_ip.read();
            for record in map.values() {
                if matches!(
                    evaluate_trust(record),
                    ClientTrustStatus::NotTrusted { .. }
                        | ClientTrustStatus::LikelyUntrusted { .. }
                ) {
                    count += 1;
                }
            }
        }

        {
            let map = self.by_app.read();
            for record in map.values() {
                if matches!(
                    evaluate_trust(record),
                    ClientTrustStatus::NotTrusted { .. }
                        | ClientTrustStatus::LikelyUntrusted { .. }
                ) {
                    count += 1;
                }
            }
        }

        count
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ClientTrustEvent> {
        self.event_sender.subscribe()
    }

    pub fn clear(&self) {
        self.by_ip.write().clear();
        self.by_app.write().clear();
        info!("Client trust tracker cleared");
    }

    fn maybe_emit_event(
        &self,
        identifier: &str,
        identifier_type: &str,
        old_status: &ClientTrustStatus,
        new_status: &ClientTrustStatus,
        event_ctx: Option<(String, String)>,
    ) {
        let old_tag = status_tag(old_status);
        let new_tag = status_tag(new_status);
        if old_tag != new_tag {
            let (reason, domain) = event_ctx.unzip();
            let now = current_timestamp();
            let event = ClientTrustEvent {
                identifier: identifier.to_string(),
                identifier_type: identifier_type.to_string(),
                old_status: old_tag.to_string(),
                new_status: new_tag.to_string(),
                reason,
                domain,
                timestamp: now,
            };
            let _ = self.event_sender.send(event);
        }
    }
}

pub fn classify_tls_accept_error(error: &std::io::Error) -> TlsAcceptFailureReason {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();

    if lower.contains("unknownca") || lower.contains("unknown_ca") || lower.contains("unknown ca") {
        return TlsAcceptFailureReason::ClientDoesNotTrustCa;
    }
    if lower.contains("badcertificate")
        || lower.contains("bad_certificate")
        || lower.contains("bad certificate")
    {
        return TlsAcceptFailureReason::ClientDoesNotTrustCa;
    }
    if lower.contains("certificateunknown")
        || lower.contains("certificate_unknown")
        || lower.contains("certificate unknown")
    {
        return TlsAcceptFailureReason::ClientDoesNotTrustCa;
    }

    if lower.contains("decrypt") {
        return TlsAcceptFailureReason::ProbablyClientDoesNotTrustCa;
    }

    if lower.contains("certificateexpired") || lower.contains("certificate expired") {
        return TlsAcceptFailureReason::CertificateExpired;
    }

    if lower.contains("handshakefailure") || lower.contains("protocolversion") {
        return TlsAcceptFailureReason::ProtocolIncompatible;
    }

    if lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
    {
        return TlsAcceptFailureReason::ConnectionReset;
    }

    TlsAcceptFailureReason::Unknown
}

fn evaluate_trust(record: &ClientTrustRecord) -> ClientTrustStatus {
    let total =
        record.handshake_success + record.handshake_fail_untrust + record.handshake_fail_other;

    if total == 0 {
        return ClientTrustStatus::Unknown;
    }

    if record.handshake_fail_untrust > 0 && record.handshake_success == 0 {
        return ClientTrustStatus::NotTrusted {
            reason: record
                .last_failure_reason
                .as_ref()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        };
    }

    if record.handshake_fail_untrust > 0 && record.handshake_success > 0 {
        if let (Some(last_success), Some(last_failure)) =
            (record.last_success_at, record.last_failure_at)
        {
            if last_success > last_failure {
                return ClientTrustStatus::Trusted;
            }
        }
    }

    if record.handshake_fail_untrust == 0 && record.handshake_success > 0 {
        return ClientTrustStatus::Trusted;
    }

    if total >= MIN_SAMPLE_FOR_INFERENCE {
        let fail_ratio = record.handshake_fail_untrust as f32 / total as f32;
        if fail_ratio > HIGH_UNTRUST_RATIO {
            return ClientTrustStatus::LikelyUntrusted {
                confidence: fail_ratio,
                sample_count: total,
            };
        }
    }

    ClientTrustStatus::Unknown
}

fn status_tag(status: &ClientTrustStatus) -> &'static str {
    match status {
        ClientTrustStatus::Trusted => "trusted",
        ClientTrustStatus::NotTrusted { .. } => "not_trusted",
        ClientTrustStatus::LikelyUntrusted { .. } => "likely_untrusted",
        ClientTrustStatus::Unknown => "unknown",
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_unknown_ca() {
        let err = std::io::Error::other("peer sent fatal alert: UnknownCA");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::ClientDoesNotTrustCa
        );
    }

    #[test]
    fn test_classify_bad_certificate() {
        let err = std::io::Error::other("peer sent fatal alert: BadCertificate");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::ClientDoesNotTrustCa
        );
    }

    #[test]
    fn test_classify_certificate_unknown() {
        let err = std::io::Error::other("peer sent fatal alert: CertificateUnknown");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::ClientDoesNotTrustCa
        );
    }

    #[test]
    fn test_classify_decrypt_error() {
        let err = std::io::Error::other("decrypt error");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::ProbablyClientDoesNotTrustCa
        );
    }

    #[test]
    fn test_classify_connection_reset() {
        let err = std::io::Error::other("connection reset by peer");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::ConnectionReset
        );
    }

    #[test]
    fn test_classify_unexpected_eof() {
        let err = std::io::Error::other("unexpected eof");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::ConnectionReset
        );
    }

    #[test]
    fn test_classify_unknown() {
        let err = std::io::Error::other("some random error");
        assert_eq!(
            classify_tls_accept_error(&err),
            TlsAcceptFailureReason::Unknown
        );
    }

    #[test]
    fn test_evaluate_trust_no_data() {
        let record = ClientTrustRecord::new(100);
        assert!(matches!(
            evaluate_trust(&record),
            ClientTrustStatus::Unknown
        ));
    }

    #[test]
    fn test_evaluate_trust_all_success() {
        let mut record = ClientTrustRecord::new(100);
        record.handshake_success = 5;
        assert!(matches!(
            evaluate_trust(&record),
            ClientTrustStatus::Trusted
        ));
    }

    #[test]
    fn test_evaluate_trust_all_untrust_failure() {
        let mut record = ClientTrustRecord::new(100);
        record.handshake_fail_untrust = 3;
        record.last_failure_reason = Some(TlsAcceptFailureReason::ClientDoesNotTrustCa);
        assert!(matches!(
            evaluate_trust(&record),
            ClientTrustStatus::NotTrusted { .. }
        ));
    }

    #[test]
    fn test_evaluate_trust_recovered() {
        let mut record = ClientTrustRecord::new(100);
        record.handshake_fail_untrust = 3;
        record.handshake_success = 2;
        record.last_failure_at = Some(100);
        record.last_success_at = Some(200);
        assert!(matches!(
            evaluate_trust(&record),
            ClientTrustStatus::Trusted
        ));
    }

    #[test]
    fn test_tracker_record_and_query() {
        let tracker = ClientTlsTrustTracker::new();

        tracker.record_handshake_failure(
            "192.168.1.100",
            Some("Firefox"),
            "example.com",
            &TlsAcceptFailureReason::ClientDoesNotTrustCa,
        );
        tracker.record_handshake_failure(
            "192.168.1.100",
            Some("Firefox"),
            "github.com",
            &TlsAcceptFailureReason::ClientDoesNotTrustCa,
        );

        let ip: IpAddr = "192.168.1.100".parse().unwrap();
        assert!(matches!(
            tracker.get_trust_status_by_ip(&ip),
            ClientTrustStatus::NotTrusted { .. }
        ));
        assert!(matches!(
            tracker.get_trust_status_by_app("Firefox"),
            ClientTrustStatus::NotTrusted { .. }
        ));

        let statuses = tracker.get_all_statuses();
        assert_eq!(statuses.len(), 2);
        assert_eq!(tracker.get_untrusted_count(), 2);
    }

    #[test]
    fn test_tracker_event_emission() {
        let tracker = ClientTlsTrustTracker::new();
        let mut rx = tracker.subscribe();

        tracker.record_handshake_failure(
            "10.0.0.1",
            None,
            "example.com",
            &TlsAcceptFailureReason::ClientDoesNotTrustCa,
        );

        let event = rx.try_recv();
        assert!(event.is_ok());
        let event = event.unwrap();
        assert_eq!(event.identifier, "10.0.0.1");
        assert_eq!(event.old_status, "unknown");
        assert_eq!(event.new_status, "not_trusted");
    }

    #[test]
    fn test_tracker_clear() {
        let tracker = ClientTlsTrustTracker::new();

        tracker.record_handshake_success("10.0.0.1", Some("Chrome"), "example.com");
        assert_eq!(tracker.get_all_statuses().len(), 2);

        tracker.clear();
        assert_eq!(tracker.get_all_statuses().len(), 0);
    }
}

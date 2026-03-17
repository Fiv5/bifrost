use serde::{Deserialize, Serialize};

use crate::body_store::BodyStoreStats;
use crate::ws_payload_store::WsPayloadStoreStats;

pub const RESOURCE_ALERT_WARN_RATIO: f64 = 0.80;
pub const RESOURCE_ALERT_CRITICAL_RATIO: f64 = 0.95;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAlertLevel {
    Ok,
    Warn,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceAlertStatus {
    pub level: ResourceAlertLevel,
    pub current: usize,
    pub limit: usize,
    pub usage_ratio: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceAlerts {
    pub overall_level: ResourceAlertLevel,
    pub body_stream_writers: Option<ResourceAlertStatus>,
    pub ws_payload_writers: Option<ResourceAlertStatus>,
}

pub fn resource_alert_level(current: usize, limit: usize) -> ResourceAlertLevel {
    let ratio = usage_ratio(current, limit);
    if ratio >= RESOURCE_ALERT_CRITICAL_RATIO {
        ResourceAlertLevel::Critical
    } else if ratio >= RESOURCE_ALERT_WARN_RATIO {
        ResourceAlertLevel::Warn
    } else {
        ResourceAlertLevel::Ok
    }
}

pub fn usage_ratio(current: usize, limit: usize) -> f64 {
    if limit == 0 {
        return 0.0;
    }
    current as f64 / limit as f64
}

pub fn build_resource_alerts(
    body_store_stats: Option<&BodyStoreStats>,
    ws_payload_store_stats: Option<&WsPayloadStoreStats>,
) -> ResourceAlerts {
    let body_stream_writers = body_store_stats.map(|stats| {
        build_status(
            "Body stream writers",
            stats.active_stream_writers,
            stats.max_open_stream_writers,
        )
    });
    let ws_payload_writers = ws_payload_store_stats.map(|stats| {
        build_status(
            "WebSocket payload writers",
            stats.active_writers,
            stats.max_open_files,
        )
    });

    let overall_level = body_stream_writers
        .iter()
        .chain(ws_payload_writers.iter())
        .map(|status| status.level)
        .max()
        .unwrap_or(ResourceAlertLevel::Ok);

    ResourceAlerts {
        overall_level,
        body_stream_writers,
        ws_payload_writers,
    }
}

fn build_status(label: &str, current: usize, limit: usize) -> ResourceAlertStatus {
    let level = resource_alert_level(current, limit);
    let usage_ratio = usage_ratio(current, limit);
    let usage_percent = (usage_ratio * 100.0).round() as usize;
    let message = match level {
        ResourceAlertLevel::Ok => format!(
            "{} are within the safe range ({}/{}, {}%).",
            label, current, limit, usage_percent
        ),
        ResourceAlertLevel::Warn => format!(
            "{} are nearing the open-file limit ({}/{}, {}%).",
            label, current, limit, usage_percent
        ),
        ResourceAlertLevel::Critical => format!(
            "{} are close to exhausting the open-file limit ({}/{}, {}%).",
            label, current, limit, usage_percent
        ),
    };

    ResourceAlertStatus {
        level,
        current,
        limit,
        usage_ratio,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_stats(active_stream_writers: usize, max_open_stream_writers: usize) -> BodyStoreStats {
        BodyStoreStats {
            file_count: 0,
            total_size: 0,
            temp_dir: String::new(),
            max_memory_size: 0,
            retention_days: 7,
            active_stream_writers,
            max_open_stream_writers,
        }
    }

    fn ws_stats(active_writers: usize, max_open_files: usize) -> WsPayloadStoreStats {
        WsPayloadStoreStats {
            file_count: 0,
            total_size: 0,
            payload_dir: String::new(),
            retention_days: 7,
            active_writers,
            max_open_files,
        }
    }

    #[test]
    fn builds_warn_alert_when_usage_crosses_warn_threshold() {
        let alerts = build_resource_alerts(Some(&body_stats(103, 128)), None);

        assert_eq!(alerts.overall_level, ResourceAlertLevel::Warn);
        assert_eq!(
            alerts
                .body_stream_writers
                .as_ref()
                .map(|status| status.level),
            Some(ResourceAlertLevel::Warn)
        );
    }

    #[test]
    fn builds_critical_alert_when_usage_crosses_critical_threshold() {
        let alerts = build_resource_alerts(None, Some(&ws_stats(122, 128)));

        assert_eq!(alerts.overall_level, ResourceAlertLevel::Critical);
        assert_eq!(
            alerts
                .ws_payload_writers
                .as_ref()
                .map(|status| status.level),
            Some(ResourceAlertLevel::Critical)
        );
    }
}

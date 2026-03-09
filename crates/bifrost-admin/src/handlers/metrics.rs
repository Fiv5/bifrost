use std::collections::HashMap;

use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::state::SharedAdminState;
use crate::traffic::SocketStatus;
use crate::traffic_db::{AppMetricsAggregate, HostMetricsAggregate};

#[derive(Debug, Clone)]
struct MetricTraffic {
    id: String,
    host: String,
    protocol: String,
    request_size: u64,
    response_size: u64,
    is_websocket: bool,
    is_sse: bool,
    is_tunnel: bool,
    socket_status: Option<SocketStatus>,
    client_app: Option<String>,
}

impl From<crate::traffic::TrafficSummary> for MetricTraffic {
    fn from(value: crate::traffic::TrafficSummary) -> Self {
        Self {
            id: value.id,
            host: value.host,
            protocol: value.protocol,
            request_size: value.request_size as u64,
            response_size: value.response_size as u64,
            is_websocket: value.is_websocket,
            is_sse: value.is_sse,
            is_tunnel: value.is_tunnel,
            socket_status: value.socket_status,
            client_app: value.client_app,
        }
    }
}

async fn load_metric_traffic(state: SharedAdminState) -> Vec<MetricTraffic> {
    if let Some(ref traffic_store) = state.traffic_store {
        return traffic_store
            .get_all()
            .into_iter()
            .map(MetricTraffic::from)
            .collect();
    }

    state
        .traffic_recorder
        .get_all()
        .into_iter()
        .map(MetricTraffic::from)
        .collect()
}

pub async fn handle_metrics(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/api/metrics" | "/api/metrics/" => match method {
            Method::GET => get_current_metrics(state).await,
            _ => method_not_allowed(),
        },
        "/api/metrics/history" => match method {
            Method::GET => get_metrics_history(req, state).await,
            _ => method_not_allowed(),
        },
        "/api/metrics/apps" => match method {
            Method::GET => get_app_metrics(state).await,
            _ => method_not_allowed(),
        },
        "/api/metrics/hosts" => match method {
            Method::GET => get_host_metrics(state).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_current_metrics(state: SharedAdminState) -> Response<BoxBody> {
    let metrics = state.metrics_collector.get_current();
    json_response(&metrics)
}

async fn get_metrics_history(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let limit = parse_limit(query);

    let history = state.metrics_collector.get_history(limit);
    json_response(&history)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppMetrics {
    pub app_name: String,
    pub requests: u64,
    pub active_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub http_requests: u64,
    pub https_requests: u64,
    pub tunnel_requests: u64,
    pub ws_requests: u64,
    pub wss_requests: u64,
    pub h3_requests: u64,
    pub socks5_requests: u64,
}

async fn get_app_metrics(state: SharedAdminState) -> Response<BoxBody> {
    let mut app_stats: HashMap<String, AppMetrics> = HashMap::new();

    if let Some(ref db_store) = state.traffic_db_store {
        let db_store = db_store.clone();
        let aggregates = tokio::task::spawn_blocking(move || db_store.aggregate_app_metrics())
            .await
            .unwrap_or_default();
        for aggregate in aggregates {
            let AppMetricsAggregate {
                app_name,
                requests,
                bytes_sent,
                bytes_received,
                http_requests,
                https_requests,
                tunnel_requests,
                ws_requests,
                wss_requests,
                h3_requests,
                socks5_requests,
            } = aggregate;
            app_stats.insert(
                app_name.clone(),
                AppMetrics {
                    app_name,
                    requests,
                    bytes_sent,
                    bytes_received,
                    http_requests,
                    https_requests,
                    tunnel_requests,
                    ws_requests,
                    wss_requests,
                    h3_requests,
                    socks5_requests,
                    active_connections: 0,
                },
            );
        }
    } else {
        let records = load_metric_traffic(state.clone()).await;

        for mut record in records {
            if (record.is_websocket || record.is_sse || record.is_tunnel)
                && record.socket_status.is_none()
            {
                if let Some(status) = state.connection_monitor.get_connection_status(&record.id) {
                    record.socket_status = Some(status);
                }
            }

            let app_name = record
                .client_app
                .clone()
                .unwrap_or_else(|| "Unknown".to_string());

            let entry = app_stats
                .entry(app_name.clone())
                .or_insert_with(|| AppMetrics {
                    app_name,
                    ..Default::default()
                });

            entry.requests += 1;

            if record.is_websocket || record.is_sse || record.is_tunnel {
                if let Some(ref socket_status) = record.socket_status {
                    entry.bytes_sent += socket_status.send_bytes;
                    entry.bytes_received += socket_status.receive_bytes;
                } else {
                    entry.bytes_sent += record.request_size;
                    entry.bytes_received += record.response_size;
                }
            } else {
                entry.bytes_sent += record.request_size;
                entry.bytes_received += record.response_size;
            }

            match record.protocol.as_str() {
                "http" => entry.http_requests += 1,
                "https" => entry.https_requests += 1,
                "tunnel" => entry.tunnel_requests += 1,
                "ws" => entry.ws_requests += 1,
                "wss" => entry.wss_requests += 1,
                "h3" => entry.h3_requests += 1,
                "socks5" => entry.socks5_requests += 1,
                _ => {}
            }
        }
    }

    for (_, _, _, _, client_app) in state.connection_registry.list_connections_full() {
        let app_name = client_app.unwrap_or_else(|| "Unknown".to_string());
        let entry = app_stats
            .entry(app_name.clone())
            .or_insert_with(|| AppMetrics {
                app_name,
                ..Default::default()
            });
        entry.active_connections += 1;
    }

    let mut result: Vec<AppMetrics> = app_stats.into_values().collect();
    result.sort_by(|a, b| b.requests.cmp(&a.requests));

    json_response(&result)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostMetrics {
    pub host: String,
    pub requests: u64,
    pub active_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub http_requests: u64,
    pub https_requests: u64,
    pub tunnel_requests: u64,
    pub ws_requests: u64,
    pub wss_requests: u64,
    pub h3_requests: u64,
    pub socks5_requests: u64,
}

async fn get_host_metrics(state: SharedAdminState) -> Response<BoxBody> {
    let mut host_stats: HashMap<String, HostMetrics> = HashMap::new();

    if let Some(ref db_store) = state.traffic_db_store {
        let db_store = db_store.clone();
        let aggregates = tokio::task::spawn_blocking(move || db_store.aggregate_host_metrics())
            .await
            .unwrap_or_default();
        for aggregate in aggregates {
            let HostMetricsAggregate {
                host,
                requests,
                bytes_sent,
                bytes_received,
                http_requests,
                https_requests,
                tunnel_requests,
                ws_requests,
                wss_requests,
                h3_requests,
                socks5_requests,
            } = aggregate;
            host_stats.insert(
                host.clone(),
                HostMetrics {
                    host,
                    requests,
                    bytes_sent,
                    bytes_received,
                    http_requests,
                    https_requests,
                    tunnel_requests,
                    ws_requests,
                    wss_requests,
                    h3_requests,
                    socks5_requests,
                    active_connections: 0,
                },
            );
        }
    } else {
        let records = load_metric_traffic(state.clone()).await;

        for mut record in records {
            if (record.is_websocket || record.is_sse || record.is_tunnel)
                && record.socket_status.is_none()
            {
                if let Some(status) = state.connection_monitor.get_connection_status(&record.id) {
                    record.socket_status = Some(status);
                }
            }

            let host = if record.host.is_empty() {
                "Unknown".to_string()
            } else {
                record.host.clone()
            };

            let entry = host_stats
                .entry(host.clone())
                .or_insert_with(|| HostMetrics {
                    host,
                    ..Default::default()
                });

            entry.requests += 1;

            if record.is_websocket || record.is_sse || record.is_tunnel {
                if let Some(ref socket_status) = record.socket_status {
                    entry.bytes_sent += socket_status.send_bytes;
                    entry.bytes_received += socket_status.receive_bytes;
                } else {
                    entry.bytes_sent += record.request_size;
                    entry.bytes_received += record.response_size;
                }
            } else {
                entry.bytes_sent += record.request_size;
                entry.bytes_received += record.response_size;
            }

            match record.protocol.as_str() {
                "http" => entry.http_requests += 1,
                "https" => entry.https_requests += 1,
                "tunnel" => entry.tunnel_requests += 1,
                "ws" => entry.ws_requests += 1,
                "wss" => entry.wss_requests += 1,
                "h3" => entry.h3_requests += 1,
                "socks5" => entry.socks5_requests += 1,
                _ => {}
            }
        }
    }

    for (_, host, _, _, _) in state.connection_registry.list_connections_full() {
        let host = if host.is_empty() {
            "Unknown".to_string()
        } else {
            host
        };
        let entry = host_stats
            .entry(host.clone())
            .or_insert_with(|| HostMetrics {
                host,
                ..Default::default()
            });
        entry.active_connections += 1;
    }

    let mut result: Vec<HostMetrics> = host_stats.into_values().collect();
    result.sort_by(|a, b| b.requests.cmp(&a.requests));

    json_response(&result)
}

fn parse_limit(query: &str) -> Option<usize> {
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            if key == "limit" {
                return value.parse().ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use http_body_util::BodyExt;

    use super::*;
    use crate::state::AdminState;
    use crate::traffic::TrafficRecord;
    use crate::traffic_db::TrafficDbStore;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bifrost-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn host_metrics_include_traffic_db_records() {
        let db_dir = temp_dir("metrics-hosts");
        let db_store = TrafficDbStore::new(db_dir.clone(), 5000, 0, None).unwrap();

        let state = Arc::new(AdminState::new(0).with_traffic_db_store(db_store));

        let mut record = TrafficRecord::new(
            "req-1".to_string(),
            "GET".to_string(),
            "https://example.com/a".to_string(),
        );
        record.status = 200;
        record.request_size = 10;
        record.response_size = 20;
        state.record_traffic(record);

        let resp = super::get_host_metrics(state).await;
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let metrics: Vec<HostMetrics> = serde_json::from_slice(&body).unwrap();

        let m = metrics.iter().find(|m| m.host == "example.com").unwrap();
        assert_eq!(m.requests, 1);
        assert_eq!(m.bytes_sent, 10);
        assert_eq!(m.bytes_received, 20);
        assert_eq!(m.https_requests, 1);

        std::fs::remove_dir_all(&db_dir).ok();
    }

    #[tokio::test]
    async fn app_metrics_include_traffic_db_records() {
        let db_dir = temp_dir("metrics-apps");
        let db_store = TrafficDbStore::new(db_dir.clone(), 5000, 0, None).unwrap();

        let state = Arc::new(AdminState::new(0).with_traffic_db_store(db_store));

        let mut record = TrafficRecord::new(
            "req-2".to_string(),
            "GET".to_string(),
            "https://example.com/b".to_string(),
        );
        record.status = 200;
        record.request_size = 7;
        record.response_size = 9;
        record.client_app = Some("TestApp".to_string());
        state.record_traffic(record);

        let resp = super::get_app_metrics(state).await;
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let metrics: Vec<AppMetrics> = serde_json::from_slice(&body).unwrap();

        let m = metrics.iter().find(|m| m.app_name == "TestApp").unwrap();
        assert_eq!(m.requests, 1);
        assert_eq!(m.bytes_sent, 7);
        assert_eq!(m.bytes_received, 9);
        assert_eq!(m.https_requests, 1);

        std::fs::remove_dir_all(&db_dir).ok();
    }

    #[tokio::test]
    async fn host_metrics_include_in_memory_records() {
        let state = Arc::new(AdminState::new(0));

        let mut record = TrafficRecord::new(
            "req-3".to_string(),
            "GET".to_string(),
            "http://example.net/c".to_string(),
        );
        record.status = 200;
        record.request_size = 3;
        record.response_size = 5;
        state.record_traffic(record);

        let resp = super::get_host_metrics(state).await;
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let metrics: Vec<HostMetrics> = serde_json::from_slice(&body).unwrap();

        let m = metrics.iter().find(|m| m.host == "example.net").unwrap();
        assert_eq!(m.requests, 1);
        assert_eq!(m.bytes_sent, 3);
        assert_eq!(m.bytes_received, 5);
        assert_eq!(m.http_requests, 1);
    }
}

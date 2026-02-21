use std::collections::HashMap;

use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::Serialize;

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::state::SharedAdminState;

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

#[derive(Debug, Clone, Serialize, Default)]
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
}

async fn get_app_metrics(state: SharedAdminState) -> Response<BoxBody> {
    let mut app_stats: HashMap<String, AppMetrics> = HashMap::new();

    if let Some(ref traffic_store) = state.traffic_store {
        let records = traffic_store.get_all();

        for record in records {
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
                    entry.bytes_sent += record.request_size as u64;
                    entry.bytes_received += record.response_size as u64;
                }
            } else {
                entry.bytes_sent += record.request_size as u64;
                entry.bytes_received += record.response_size as u64;
            }

            match record.protocol.as_str() {
                "http" => entry.http_requests += 1,
                "https" => entry.https_requests += 1,
                "tunnel" => entry.tunnel_requests += 1,
                "ws" => entry.ws_requests += 1,
                "wss" => entry.wss_requests += 1,
                _ => {}
            }
        }
    }

    let mut result: Vec<AppMetrics> = app_stats.into_values().collect();
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

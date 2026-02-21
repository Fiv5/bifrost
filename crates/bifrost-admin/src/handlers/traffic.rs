use hyper::{body::Incoming, Method, Request, Response, StatusCode};

use super::frames::{get_frame_detail, get_frames, subscribe_frames, unsubscribe_frames};
use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::body_store::BodyRef;
use crate::state::SharedAdminState;
use crate::traffic::TrafficFilter;

pub async fn handle_traffic(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/traffic" || path == "/api/traffic/" {
        match method {
            Method::GET => list_traffic(req, state).await,
            Method::DELETE => clear_traffic(state).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/traffic/updates" {
        match method {
            Method::GET => get_traffic_updates(req, state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(rest) = path.strip_prefix("/api/traffic/") {
        if let Some(id) = rest.strip_suffix("/request-body") {
            match method {
                Method::GET => get_request_body(state, id).await,
                _ => method_not_allowed(),
            }
        } else if let Some(id) = rest.strip_suffix("/response-body") {
            match method {
                Method::GET => get_response_body(state, id).await,
                _ => method_not_allowed(),
            }
        } else if let Some(id) = rest.strip_suffix("/frames/stream") {
            match method {
                Method::GET => subscribe_frames(state, id).await,
                _ => method_not_allowed(),
            }
        } else if let Some(id) = rest.strip_suffix("/frames/unsubscribe") {
            match method {
                Method::DELETE => unsubscribe_frames(state, id).await,
                _ => method_not_allowed(),
            }
        } else if rest.contains("/frames/") {
            if let Some((id, frame_part)) = rest.split_once("/frames/") {
                if let Ok(frame_id) = frame_part.parse::<u64>() {
                    match method {
                        Method::GET => get_frame_detail(state, id, frame_id).await,
                        _ => method_not_allowed(),
                    }
                } else {
                    error_response(StatusCode::BAD_REQUEST, "Invalid frame ID")
                }
            } else {
                error_response(StatusCode::BAD_REQUEST, "Invalid path")
            }
        } else if let Some(id) = rest.strip_suffix("/frames") {
            match method {
                Method::GET => get_frames(state, id, req.uri().query()).await,
                _ => method_not_allowed(),
            }
        } else {
            match method {
                Method::GET => get_traffic_detail(state, rest).await,
                _ => method_not_allowed(),
            }
        }
    } else {
        error_response(StatusCode::NOT_FOUND, "Not Found")
    }
}

async fn list_traffic(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let filter = parse_traffic_filter(query);

    let records = if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.filter(&filter)
    } else {
        state.traffic_recorder.filter(&filter)
    };

    let (offset, limit) = (filter.offset.unwrap_or(0), filter.limit.unwrap_or(100));

    let total = records.len();
    let paginated: Vec<_> = records
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|mut summary| {
            if summary.is_sse || summary.is_websocket {
                if let Some(status) = state.websocket_monitor.get_connection_status(&summary.id) {
                    summary.frame_count = status.frame_count;
                    summary.socket_status = Some(status);
                }
            }
            summary
        })
        .collect();

    let response = serde_json::json!({
        "total": total,
        "offset": offset,
        "limit": limit,
        "records": paginated
    });

    json_response(&response)
}

async fn get_traffic_updates(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let params = parse_updates_params(query);
    let filter = parse_traffic_filter(query);

    let limit = params.limit.unwrap_or(100);

    let (new_records, has_more) = if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_after(params.after_id.as_deref(), &filter, limit)
    } else {
        state
            .traffic_recorder
            .get_after(params.after_id.as_deref(), &filter, limit)
    };

    let enrich_summary = |mut summary: crate::traffic::TrafficSummary| {
        if summary.is_sse || summary.is_websocket {
            if let Some(status) = state.websocket_monitor.get_connection_status(&summary.id) {
                summary.frame_count = status.frame_count;
                summary.socket_status = Some(status);
            }
        }
        summary
    };

    let new_records: Vec<_> = new_records.into_iter().map(enrich_summary).collect();

    let updated_records = if !params.pending_ids.is_empty() {
        let ids: Vec<&str> = params.pending_ids.iter().map(|s| s.as_str()).collect();
        let summaries = if let Some(ref traffic_store) = state.traffic_store {
            traffic_store.get_by_ids(&ids)
        } else {
            state.traffic_recorder.get_by_ids(&ids)
        };
        summaries
            .into_iter()
            .map(|mut summary| {
                if summary.is_sse || summary.is_websocket {
                    if let Some(status) = state.websocket_monitor.get_connection_status(&summary.id)
                    {
                        summary.frame_count = status.frame_count;
                        summary.socket_status = Some(status);
                    }
                }
                summary
            })
            .collect()
    } else {
        Vec::new()
    };

    let server_total = if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.total()
    } else {
        state.traffic_recorder.total()
    };

    let response = serde_json::json!({
        "new_records": new_records,
        "updated_records": updated_records,
        "has_more": has_more,
        "server_total": server_total
    });

    json_response(&response)
}

#[derive(Debug, Default)]
struct UpdatesParams {
    after_id: Option<String>,
    pending_ids: Vec<String>,
    limit: Option<usize>,
}

fn parse_updates_params(query: &str) -> UpdatesParams {
    let mut params = UpdatesParams::default();

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "after_id" => {
                    if !value.is_empty() {
                        params.after_id = Some(value.to_string());
                    }
                }
                "pending_ids" => {
                    if !value.is_empty() {
                        params.pending_ids = value.split(',').map(|s| s.to_string()).collect();
                    }
                }
                "limit" => {
                    params.limit = value.parse().ok();
                }
                _ => {}
            }
        }
    }

    params
}

async fn get_traffic_detail(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let record = if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_by_id(id)
    } else {
        state.traffic_recorder.get_by_id(id)
    };

    match record {
        Some(mut record) => {
            if record.is_websocket || record.is_sse {
                if let Some(status) = state.websocket_monitor.get_connection_status(&record.id) {
                    record.frame_count = status.frame_count;
                    record.last_frame_id = status.frame_count as u64;
                    record.socket_status = Some(status);
                }
            }
            json_response(&record)
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Traffic record '{}' not found", id),
        ),
    }
}

async fn clear_traffic(state: SharedAdminState) -> Response<BoxBody> {
    if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.clear();
        let new_sequence = traffic_store.current_sequence();
        state.traffic_recorder.set_initial_sequence(new_sequence);
    }
    state.traffic_recorder.clear();

    if let Some(ref body_store) = state.body_store {
        let body_store_clone = body_store.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = body_store_clone.write().clear() {
                tracing::warn!("Failed to clear body store: {}", e);
            }
        })
        .await;
    }

    if let Some(ref frame_store) = state.frame_store {
        let frame_store_clone = frame_store.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = frame_store_clone.clear() {
                tracing::warn!("Failed to clear frame store: {}", e);
            }
        })
        .await;
    }

    state.websocket_monitor.clear();

    success_response("All traffic data cleared successfully")
}

async fn get_request_body(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let record = if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_by_id(id)
    } else {
        state.traffic_recorder.get_by_id(id)
    };

    match record {
        Some(record) => {
            if let Some(body_ref) = &record.request_body_ref {
                get_body_content_async(&state, body_ref).await
            } else {
                json_response(&serde_json::json!({
                    "success": true,
                    "data": null
                }))
            }
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Traffic record '{}' not found", id),
        ),
    }
}

async fn get_response_body(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let record = if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_by_id(id)
    } else {
        state.traffic_recorder.get_by_id(id)
    };

    match record {
        Some(record) => {
            if let Some(body_ref) = &record.response_body_ref {
                get_body_content_async(&state, body_ref).await
            } else {
                json_response(&serde_json::json!({
                    "success": true,
                    "data": null
                }))
            }
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Traffic record '{}' not found", id),
        ),
    }
}

async fn get_body_content_async(state: &SharedAdminState, body_ref: &BodyRef) -> Response<BoxBody> {
    match body_ref {
        BodyRef::Inline { data } => json_response(&serde_json::json!({
            "success": true,
            "data": data
        })),
        BodyRef::File { path, size } => {
            if let Some(ref body_store) = state.body_store {
                let body_store_clone = body_store.clone();
                let body_ref_clone = body_ref.clone();
                let path_clone = path.clone();

                let data = tokio::task::spawn_blocking(move || {
                    let store = body_store_clone.read();
                    store.load(&body_ref_clone)
                })
                .await
                .ok()
                .flatten();

                match data {
                    Some(content) => json_response(&serde_json::json!({
                        "success": true,
                        "data": content
                    })),
                    None => error_response(
                        StatusCode::NOT_FOUND,
                        &format!("Body file not found: {}", path_clone),
                    ),
                }
            } else {
                json_response(&serde_json::json!({
                    "success": false,
                    "error": "Body store not configured",
                    "path": path,
                    "size": size
                }))
            }
        }
    }
}

fn parse_traffic_filter(query: &str) -> TrafficFilter {
    let mut filter = TrafficFilter::default();

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "method" => filter.method = Some(value.to_string()),
                "status" => filter.status = value.parse().ok(),
                "status_min" => filter.status_min = value.parse().ok(),
                "status_max" => filter.status_max = value.parse().ok(),
                "url" | "url_contains" => filter.url_contains = Some(value.to_string()),
                "host" => filter.host = Some(value.to_string()),
                "content_type" => filter.content_type = Some(value.to_string()),
                "limit" => filter.limit = value.parse().ok(),
                "offset" => filter.offset = value.parse().ok(),
                "has_rule_hit" => filter.has_rule_hit = value.parse().ok(),
                "protocol" => filter.protocol = Some(value.to_string()),
                "request_content_type" => filter.request_content_type = Some(value.to_string()),
                "domain" => filter.domain = Some(value.to_string()),
                "path_contains" | "path" => filter.path_contains = Some(value.to_string()),
                "header_contains" | "header" => filter.header_contains = Some(value.to_string()),
                "client_ip" => filter.client_ip = Some(value.to_string()),
                "client_app" => filter.client_app = Some(value.to_string()),
                _ => {}
            }
        }
    }

    filter
}

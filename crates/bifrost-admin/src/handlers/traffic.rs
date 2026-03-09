use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::frames::{get_frame_detail, get_frames, subscribe_frames, unsubscribe_frames};
use super::{error_response, json_response, method_not_allowed, success_response, BoxBody};
use crate::body_store::BodyRef;
use crate::push::{SharedPushManager, MAX_ID_LEN, MAX_SUBSCRIBED_IDS};
use crate::sse::{parse_sse_events_from_text, SseEventEnvelope};
use crate::state::{AdminState, SharedAdminState};
use crate::traffic::{SocketStatus, TrafficFilter, TrafficSummary};
use crate::traffic_db::{QueryParams, TrafficSummaryCompact};

fn enrich_frame_info(summary: &mut TrafficSummary, state: &AdminState) {
    if !summary.is_sse
        && !summary.is_websocket
        && !summary.is_tunnel
        && summary.socket_status.is_none()
    {
        return;
    }

    if summary.is_sse {
        if let Some(status) = state.sse_hub.get_socket_status(&summary.id) {
            summary.frame_count = status.frame_count;
            summary.socket_status = Some(status);
        }
    } else if let Some(status) = state.connection_monitor.get_connection_status(&summary.id) {
        summary.frame_count = status.frame_count;
        summary.socket_status = Some(status);
    } else if let Some(ref fs) = state.frame_store {
        if let Some(metadata) = fs.get_metadata(&summary.id) {
            summary.frame_count = metadata.frame_count as usize;
            summary.socket_status = Some(SocketStatus {
                is_open: !metadata.is_closed,
                frame_count: metadata.frame_count as usize,
                ..Default::default()
            });
        }
    }

    if summary.is_sse {
        if let Some(ref socket_status) = summary.socket_status {
            let total = socket_status.send_bytes + socket_status.receive_bytes;
            if total > 0 {
                summary.response_size = summary.response_size.max(total as usize);
            }

            if !socket_status.is_open && summary.response_size > 0 {
                let total = summary.response_size;
                let status = socket_status.clone();
                let frame_count = summary.frame_count;
                state.update_traffic_by_id(&summary.id, move |record| {
                    record.response_size = total;
                    record.socket_status = Some(status.clone());
                    record.frame_count = frame_count;
                });
            }
        }
    }
}

fn enrich_compact_frame_info(summary: &mut TrafficSummaryCompact, state: &AdminState) {
    if !summary.is_sse() && !summary.is_websocket() && !summary.is_tunnel() && summary.ss.is_none()
    {
        return;
    }

    if summary.is_sse() {
        if let Some(status) = state.sse_hub.get_socket_status(&summary.id) {
            summary.fc = status.frame_count;
            summary.ss = Some(status);
        }
    } else if let Some(status) = state.connection_monitor.get_connection_status(&summary.id) {
        summary.fc = status.frame_count;
        summary.ss = Some(status);
    } else if let Some(ref fs) = state.frame_store {
        if let Some(metadata) = fs.get_metadata(&summary.id) {
            summary.fc = metadata.frame_count as usize;
            summary.ss = Some(SocketStatus {
                is_open: !metadata.is_closed,
                frame_count: metadata.frame_count as usize,
                ..Default::default()
            });
        }
    }

    if summary.is_sse() {
        if let Some(ref socket_status) = summary.ss {
            let total = socket_status.send_bytes + socket_status.receive_bytes;
            if total > 0 {
                summary.res_sz = summary.res_sz.max(total as usize);
            }

            if !socket_status.is_open && summary.res_sz > 0 {
                let total = summary.res_sz;
                let status = socket_status.clone();
                let frame_count = summary.fc;
                let record_id = summary.id.clone();
                state.update_traffic_by_id(&record_id, move |record| {
                    record.response_size = total;
                    record.socket_status = Some(status.clone());
                    record.frame_count = frame_count;
                });
            }
        }
    }
}

pub async fn handle_traffic(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
    path: &str,
) -> Response<BoxBody> {
    let path = path.trim_end_matches('/');
    let method = req.method().clone();

    if path == "/api/traffic" {
        match method {
            Method::GET => list_traffic(req, state).await,
            Method::DELETE => clear_traffic(req, state, push_manager).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/traffic/query" {
        match method {
            Method::POST => query_traffic(req, state).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/traffic/updates" {
        match method {
            Method::GET => get_traffic_updates(req, state).await,
            _ => method_not_allowed(),
        }
    } else if let Some(rest) = path.strip_prefix("/api/traffic/") {
        let rest = rest.trim_end_matches('/');
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
        } else if let Some((id, after)) = rest.split_once("/sse/stream") {
            let after = after.trim().trim_matches('/');
            if !after.is_empty() {
                return error_response(StatusCode::BAD_REQUEST, "Invalid SSE stream path");
            }
            match method {
                Method::GET => subscribe_sse_stream(state, id).await,
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

async fn subscribe_sse_stream(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let record = if let Some(ref db_store) = state.traffic_db_store {
        let db_clone = db_store.clone();
        let id_owned = id.to_string();
        tokio::task::spawn_blocking(move || db_clone.get_by_id(&id_owned))
            .await
            .unwrap_or_default()
    } else if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_by_id(id)
    } else {
        state.traffic_recorder.get_by_id(id)
    };

    let Some(record) = record else {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("Traffic record '{}' not found", id),
        );
    };

    if !record.is_sse {
        return error_response(StatusCode::BAD_REQUEST, "Not a SSE traffic record");
    }

    let receiver = match state.sse_hub.subscribe(id) {
        Some(rx) => rx,
        None => {
            return error_response(
                StatusCode::CONFLICT,
                "SSE connection already closed; use /response-body to load and render events",
            );
        }
    };

    if state.sse_hub.is_open(id) != Some(true) {
        return error_response(
            StatusCode::CONFLICT,
            "SSE connection already closed; use /response-body to load and render events",
        );
    }

    let body_ref = match record.response_body_ref {
        Some(r) => r,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("SSE response body for {} not found", id),
            );
        }
    };

    let max_body_size = state.get_max_body_buffer_size();
    let snapshot_text = load_body_snapshot_text(state.clone(), &body_ref, max_body_size).await;
    let (mut events, _) = parse_sse_events_from_text(&snapshot_text);
    for (idx, e) in events.iter_mut().enumerate() {
        e.seq = (idx as u64) + 1;
        e.ts = record.timestamp;
    }
    let last_seq = events.len() as u64;

    let mut backlog = state.sse_hub.get_events_since(id, last_seq);
    backlog.sort_by_key(|e| e.seq);
    let max_seq_sent = backlog.last().map(|e| e.seq).unwrap_or(last_seq);

    let id_owned = id.to_string();
    let live = BroadcastStream::new(receiver).filter_map(move |result| match result {
        Ok(SseEventEnvelope {
            connection_id,
            event,
        }) if connection_id == id_owned && event.seq > max_seq_sent => {
            let data = serde_json::to_string(&event).ok()?;
            let sse_data = format!("id: {}\ndata: {}\n\n", event.seq, data);
            Some(sse_data)
        }
        _ => None,
    });

    let history = futures_util::stream::iter(events.into_iter().chain(backlog.into_iter()))
        .filter_map(|e| {
            let data = serde_json::to_string(&e).ok()?;
            Some(format!("id: {}\ndata: {}\n\n", e.seq, data))
        });
    let stream = history
        .chain(live)
        .map(|s| Ok::<_, hyper::Error>(hyper::body::Frame::data(bytes::Bytes::from(s))));
    let body_stream = http_body_util::StreamBody::new(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .body(BoxBody::new(body_stream))
        .unwrap()
}

async fn load_body_snapshot_text(
    state: SharedAdminState,
    body_ref: &BodyRef,
    max_size: usize,
) -> String {
    let Some(ref store) = state.body_store else {
        return String::new();
    };
    let store = store.clone();
    let body_ref = body_ref.clone();
    tokio::task::spawn_blocking(move || match body_ref {
        BodyRef::Inline { data } => truncate_utf8(&data, max_size),
        BodyRef::File { path, .. } => {
            let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let size = (len as usize).min(max_size);
            if size == 0 {
                return String::new();
            }
            let offset = len.saturating_sub(size as u64);
            let range = BodyRef::FileRange { path, offset, size };
            store.read().load(&range).unwrap_or_default()
        }
        BodyRef::FileRange { path, offset, size } => {
            let tail_size = size.min(max_size);
            if tail_size == 0 {
                return String::new();
            }
            let end = offset.saturating_add(size as u64);
            let start = end.saturating_sub(tail_size as u64).max(offset);
            let range = BodyRef::FileRange {
                path,
                offset: start,
                size: tail_size,
            };
            store.read().load(&range).unwrap_or_default()
        }
    })
    .await
    .unwrap_or_default()
}

fn truncate_utf8(value: &str, max_size: usize) -> String {
    if value.len() <= max_size {
        return value.to_string();
    }
    let mut end = max_size;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

async fn query_traffic(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read request body: {}", e),
            );
        }
    };

    let params: QueryParams = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid JSON body: {}", e),
            );
        }
    };

    if let Some(ref db_store) = state.traffic_db_store {
        let db_store = db_store.clone();
        let query_result = tokio::task::spawn_blocking(move || db_store.query(&params)).await;

        let mut result = match query_result {
            Ok(r) => r,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Query failed: {}", e),
                );
            }
        };

        for record in &mut result.records {
            enrich_compact_frame_info(record, &state);
        }
        json_response(&result)
    } else {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Traffic database not available",
        )
    }
}

async fn list_traffic(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");

    if let Some(ref db_store) = state.traffic_db_store {
        let params = parse_query_params_from_query_string(query);
        let db_store = db_store.clone();
        let query_result = tokio::task::spawn_blocking(move || db_store.query(&params)).await;

        let mut result = match query_result {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("[TRAFFIC_API] Query task failed: {}", e);
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Query failed: {}", e),
                );
            }
        };

        for record in &mut result.records {
            enrich_compact_frame_info(record, &state);
        }
        json_response(&result)
    } else {
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
                enrich_frame_info(&mut summary, &state);
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
}

async fn get_traffic_updates(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let query = req.uri().query().unwrap_or("");
    let params = parse_updates_params(query);

    if let Some(ref db_store) = state.traffic_db_store {
        let limit = params.limit.unwrap_or(100);
        let cursor = params.after_seq;

        let query_params = QueryParams {
            cursor,
            limit: Some(limit),
            direction: crate::traffic_db::Direction::Forward,
            ..Default::default()
        };

        let db_clone = db_store.clone();
        let query_result = tokio::task::spawn_blocking(move || db_clone.query(&query_params)).await;

        let result = match query_result {
            Ok(r) => r,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Query failed: {}", e),
                );
            }
        };

        let mut new_records: Vec<TrafficSummaryCompact> = result.records;
        for record in &mut new_records {
            enrich_compact_frame_info(record, &state);
        }

        let updated_records: Vec<TrafficSummaryCompact> = if !params.pending_ids.is_empty() {
            let ids: Vec<String> = params.pending_ids.clone();
            let db_clone = db_store.clone();
            let ids_result = tokio::task::spawn_blocking(move || {
                let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
                db_clone.get_by_ids(&id_refs)
            })
            .await;

            match ids_result {
                Ok(mut summaries) => {
                    for summary in &mut summaries {
                        enrich_compact_frame_info(summary, &state);
                    }
                    summaries
                }
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        let response = serde_json::json!({
            "new_records": new_records,
            "updated_records": updated_records,
            "has_more": result.has_more,
            "server_total": result.total,
            "server_sequence": result.server_sequence
        });

        json_response(&response)
    } else {
        let filter = parse_traffic_filter(query);
        let limit = params.limit.unwrap_or(100);

        let (new_records, has_more) = if let Some(ref traffic_store) = state.traffic_store {
            traffic_store.get_after(params.after_id.as_deref(), &filter, limit)
        } else {
            state
                .traffic_recorder
                .get_after(params.after_id.as_deref(), &filter, limit)
        };

        let new_records: Vec<_> = new_records
            .into_iter()
            .map(|mut summary| {
                enrich_frame_info(&mut summary, &state);
                summary
            })
            .collect();

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
                    enrich_frame_info(&mut summary, &state);
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
}

#[derive(Debug, Default)]
struct UpdatesParams {
    after_id: Option<String>,
    after_seq: Option<u64>,
    pending_ids: Vec<String>,
    limit: Option<usize>,
}

fn parse_updates_params(query: &str) -> UpdatesParams {
    let mut params = UpdatesParams::default();

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = decode_query_value(value);
            match key {
                "after_id" => {
                    if !value.is_empty() {
                        params.after_id = Some(value.to_string());
                    }
                }
                "after_seq" | "cursor" => {
                    params.after_seq = value.parse().ok();
                }
                "pending_ids" => {
                    if !value.is_empty() {
                        params.pending_ids = value
                            .split(',')
                            .take(MAX_SUBSCRIBED_IDS)
                            .filter_map(|s| {
                                let id = s.to_string();
                                if id.is_empty() || id.len() > MAX_ID_LEN {
                                    None
                                } else {
                                    Some(id)
                                }
                            })
                            .collect();
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

fn parse_query_params_from_query_string(query: &str) -> QueryParams {
    let mut params = QueryParams::default();

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = decode_query_value(value);
            match key {
                "cursor" => params.cursor = value.parse().ok(),
                "limit" => params.limit = value.parse().ok(),
                "direction" => {
                    if value == "forward" {
                        params.direction = crate::traffic_db::Direction::Forward;
                    }
                }
                "method" => params.method = Some(value),
                "status" => params.status = value.parse().ok(),
                "status_min" => params.status_min = value.parse().ok(),
                "status_max" => params.status_max = value.parse().ok(),
                "protocol" => params.protocol = Some(value),
                "has_rule_hit" => params.has_rule_hit = value.parse().ok(),
                "is_websocket" => params.is_websocket = value.parse().ok(),
                "is_sse" => params.is_sse = value.parse().ok(),
                "is_h3" => params.is_h3 = value.parse().ok(),
                "is_tunnel" => params.is_tunnel = value.parse().ok(),
                "host" | "host_contains" => params.host_contains = Some(value),
                "url" | "url_contains" => params.url_contains = Some(value),
                "path" | "path_contains" => params.path_contains = Some(value),
                "client_app" => params.client_app = Some(value),
                "client_ip" => params.client_ip = Some(value),
                "content_type" => params.content_type = Some(value),
                _ => {}
            }
        }
    }

    if params.limit.is_none() {
        params.limit = Some(100);
    }

    params
}

async fn get_traffic_detail(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let record = if let Some(ref db_store) = state.traffic_db_store {
        let db_clone = db_store.clone();
        let id_owned = id.to_string();
        tokio::task::spawn_blocking(move || db_clone.get_by_id(&id_owned))
            .await
            .unwrap_or_default()
    } else if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_by_id(id)
    } else {
        state.traffic_recorder.get_by_id(id)
    };

    match record {
        Some(mut record) => {
            if record.is_websocket || record.is_sse || record.is_tunnel {
                if record.is_sse {
                    if let Some(status) = state.sse_hub.get_socket_status(&record.id) {
                        record.frame_count = status.frame_count;
                        record.last_frame_id = status.frame_count as u64;
                        record.socket_status = Some(status);
                    }
                } else if let Some(status) =
                    state.connection_monitor.get_connection_status(&record.id)
                {
                    record.frame_count = status.frame_count;
                    record.last_frame_id = status.frame_count as u64;
                    record.socket_status = Some(status);
                } else if let Some(ref fs) = state.frame_store {
                    if let Some(metadata) = fs.get_metadata(&record.id) {
                        record.frame_count = metadata.frame_count as usize;
                        record.last_frame_id = metadata.last_frame_id;
                        record.socket_status = Some(SocketStatus {
                            is_open: !metadata.is_closed,
                            frame_count: metadata.frame_count as usize,
                            ..Default::default()
                        });
                    }
                }
            }
            if let Some(ref socket_status) = record.socket_status {
                let total = socket_status.send_bytes + socket_status.receive_bytes;
                if record.is_sse && total > 0 {
                    record.response_size = record.response_size.max(total as usize);
                }
                if !socket_status.is_open && record.response_size == 0 && total > 0 {
                    record.response_size = total as usize;
                    let status = socket_status.clone();
                    let frame_count = record.frame_count;
                    let last_frame_id = record.last_frame_id;
                    let record_id = record.id.clone();
                    state.update_traffic_by_id(&record_id, move |record| {
                        record.response_size = total as usize;
                        record.socket_status = Some(status.clone());
                        record.frame_count = frame_count;
                        record.last_frame_id = last_frame_id;
                    });
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

#[derive(Debug, serde::Deserialize)]
struct ClearTrafficRequest {
    ids: Option<Vec<String>>,
}

async fn clear_traffic(
    req: Request<Incoming>,
    state: SharedAdminState,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    let body = match req.collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => bytes::Bytes::new(),
    };

    let request: ClearTrafficRequest = if body.is_empty() {
        ClearTrafficRequest { ids: None }
    } else {
        match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[CLEAR_TRAFFIC] Failed to parse request body: {}", e);
                ClearTrafficRequest { ids: None }
            }
        }
    };

    if let Some(ids) = request.ids {
        if !ids.is_empty() {
            return clear_traffic_by_ids(state, ids, push_manager).await;
        }
    }

    clear_all_traffic(state, push_manager).await
}

async fn clear_traffic_by_ids(
    state: SharedAdminState,
    ids: Vec<String>,
    push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    let active_connection_ids = state.connection_monitor.active_connection_ids();
    let active_set: std::collections::HashSet<&String> = active_connection_ids.iter().collect();

    let ids_to_delete: Vec<String> = ids
        .into_iter()
        .filter(|id| !active_set.contains(id))
        .collect();

    if ids_to_delete.is_empty() {
        return success_response("No traffic records to clear (all are active connections)");
    }

    let count = ids_to_delete.len();

    if let Some(ref db_store) = state.traffic_db_store {
        db_store.delete_by_ids(&ids_to_delete);
    } else if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.delete_by_ids(&ids_to_delete);
    }
    state.traffic_recorder.delete_by_ids(&ids_to_delete);

    if let Some(ref body_store) = state.body_store {
        let body_store_clone = body_store.clone();
        let ids_for_body = ids_to_delete.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = body_store_clone.write().delete_by_ids(&ids_for_body) {
                tracing::warn!("Failed to delete bodies: {}", e);
            }
        })
        .await;
    }

    if let Some(ref frame_store) = state.frame_store {
        let frame_store_clone = frame_store.clone();
        let ids_for_frame = ids_to_delete.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = frame_store_clone.delete_by_ids(&ids_for_frame) {
                tracing::warn!("Failed to delete frames: {}", e);
            }
        })
        .await;
    }

    if let Some(ref ws_payload_store) = state.ws_payload_store {
        let ws_payload_store_clone = ws_payload_store.clone();
        let ids_for_payload = ids_to_delete.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = ws_payload_store_clone.delete_by_ids(&ids_for_payload) {
                tracing::warn!("Failed to delete ws payloads: {}", e);
            }
        })
        .await;
    }

    if let Some(pm) = push_manager {
        pm.broadcast_traffic_deleted(ids_to_delete.clone());
    }

    tracing::info!("[CLEAR_TRAFFIC] Deleted {} traffic records", count);
    success_response(&format!("{} traffic records cleared successfully", count))
}

async fn clear_all_traffic(
    state: SharedAdminState,
    _push_manager: Option<SharedPushManager>,
) -> Response<BoxBody> {
    let active_connection_ids = state.connection_monitor.active_connection_ids();

    if let Some(ref db_store) = state.traffic_db_store {
        db_store.clear_with_active_ids(&active_connection_ids);
    } else if let Some(ref traffic_store) = state.traffic_store {
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

    if let Some(ref ws_payload_store) = state.ws_payload_store {
        let ws_payload_store_clone = ws_payload_store.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = ws_payload_store_clone.clear() {
                tracing::warn!("Failed to clear ws payload store: {}", e);
            }
        })
        .await;
    }

    state.connection_monitor.clear();

    success_response("All traffic data cleared successfully")
}

async fn get_request_body(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    let record = if let Some(ref db_store) = state.traffic_db_store {
        let db_clone = db_store.clone();
        let id_owned = id.to_string();
        tokio::task::spawn_blocking(move || db_clone.get_by_id(&id_owned))
            .await
            .unwrap_or_default()
    } else if let Some(ref traffic_store) = state.traffic_store {
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
    let record = if let Some(ref db_store) = state.traffic_db_store {
        let db_clone = db_store.clone();
        let id_owned = id.to_string();
        tokio::task::spawn_blocking(move || db_clone.get_by_id(&id_owned))
            .await
            .unwrap_or_default()
    } else if let Some(ref traffic_store) = state.traffic_store {
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
        BodyRef::File { path, size } | BodyRef::FileRange { path, size, .. } => {
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

fn decode_query_value(value: &str) -> String {
    let value_with_spaces = value.replace('+', " ");
    urlencoding::decode(&value_with_spaces)
        .unwrap_or_default()
        .to_string()
}

fn parse_traffic_filter(query: &str) -> TrafficFilter {
    let mut filter = TrafficFilter::default();

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let value = decode_query_value(value);
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

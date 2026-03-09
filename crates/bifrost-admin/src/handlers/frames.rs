use std::sync::Arc;

use bytes::Bytes;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::{error_response, full_body, json_response, BoxBody};
use crate::body_store::BodyRef;
use crate::connection_monitor::WebSocketFrameRecord;
use crate::state::AdminState;
use crate::traffic::{FrameType, SocketStatus, TrafficRecord};

#[derive(Debug, Serialize)]
struct FramesResponse {
    frames: Vec<WebSocketFrameRecord>,
    socket_status: Option<SocketStatus>,
    last_frame_id: u64,
    has_more: bool,
    is_monitored: bool,
}

#[derive(Debug, Deserialize)]
pub struct FramesQuery {
    #[serde(default)]
    pub after: Option<u64>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

impl Default for FramesQuery {
    fn default() -> Self {
        Self {
            after: None,
            limit: 100,
        }
    }
}

fn default_limit() -> usize {
    100
}

fn parse_frames_query(query: Option<&str>) -> FramesQuery {
    query
        .and_then(|q| serde_urlencoded::from_str(q).ok())
        .unwrap_or_default()
}

async fn get_traffic_record(state: Arc<AdminState>, id: &str) -> Option<TrafficRecord> {
    if let Some(ref db_store) = state.traffic_db_store {
        let db_clone = db_store.clone();
        let id_owned = id.to_string();
        tokio::task::spawn_blocking(move || db_clone.get_by_id(&id_owned))
            .await
            .unwrap_or_default()
    } else if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.get_by_id(id)
    } else {
        None
    }
}

pub async fn get_frames(
    state: Arc<AdminState>,
    connection_id: &str,
    query_str: Option<&str>,
) -> Response<BoxBody> {
    tracing::debug!(
        "[FRAMES API] get_frames called for connection_id: {}",
        connection_id
    );
    let query = parse_frames_query(query_str);
    let monitor = &state.connection_monitor;

    let conn_id = connection_id.to_string();
    let frame_store = state.frame_store.clone();
    let after = query.after;
    let limit = query.limit;

    if let Some(fs) = frame_store.as_ref() {
        fs.flush();
    }

    let (file_frames, pending_frames) = if let Some(fs) = frame_store.as_ref() {
        let fs = fs.clone();
        let fs_for_file = fs.clone();
        let conn_id_clone = conn_id.clone();
        let file_frames = match tokio::task::spawn_blocking(move || {
            fs_for_file.load_frames(&conn_id_clone, after, limit)
        })
        .await
        {
            Ok(Ok((frames, _))) => frames,
            Ok(Err(e)) => {
                tracing::warn!("[FRAMES API] Failed to load frames from file: {}", e);
                Vec::new()
            }
            Err(e) => {
                tracing::warn!("[FRAMES API] spawn_blocking failed: {}", e);
                Vec::new()
            }
        };
        let pending_frames = fs.load_pending_frames(&conn_id, after, limit);
        (file_frames, pending_frames)
    } else {
        (Vec::new(), Vec::new())
    };

    let (mem_frames, is_active) = {
        let connections = monitor.connections.read();
        if let Some(store) = connections.get(&conn_id) {
            let frames: Vec<_> = if let Some(after_id) = after {
                store
                    .frames
                    .iter()
                    .filter(|f| f.frame_id > after_id)
                    .cloned()
                    .collect()
            } else {
                store.frames.iter().cloned().collect()
            };
            (frames, true)
        } else {
            (Vec::new(), false)
        }
    };

    use std::collections::HashSet;
    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut all_frames = Vec::new();

    for frame in file_frames {
        if !seen_ids.contains(&frame.frame_id) {
            seen_ids.insert(frame.frame_id);
            all_frames.push(frame);
        }
    }

    for frame in pending_frames {
        if !seen_ids.contains(&frame.frame_id) {
            seen_ids.insert(frame.frame_id);
            all_frames.push(frame);
        }
    }

    for frame in mem_frames {
        if !seen_ids.contains(&frame.frame_id) {
            seen_ids.insert(frame.frame_id);
            all_frames.push(frame);
        }
    }

    all_frames.sort_by_key(|f| f.frame_id);
    let has_more = all_frames.len() > limit;
    let frames: Vec<_> = all_frames.into_iter().take(limit).collect();

    let has_data = !frames.is_empty()
        || is_active
        || monitor.has_persisted_frames(&conn_id, state.frame_store.as_ref());

    if has_data {
        let socket_status = monitor.get_status(&conn_id);
        let last_frame_id = frames
            .last()
            .map(|f| f.frame_id)
            .or_else(|| monitor.get_last_frame_id(&conn_id))
            .or_else(|| {
                state
                    .frame_store
                    .as_ref()
                    .and_then(|fs| fs.get_last_frame_id(&conn_id))
            })
            .unwrap_or(0);
        let is_monitored = monitor.is_monitored(&conn_id);

        let response = FramesResponse {
            frames,
            socket_status,
            last_frame_id,
            has_more,
            is_monitored,
        };

        json_response(&response)
    } else {
        if let Some(record) = get_traffic_record(state.clone(), connection_id).await {
            if record.is_sse {
                let socket_status = state
                    .sse_hub
                    .get_socket_status(&conn_id)
                    .or(record.socket_status);
                let response = FramesResponse {
                    frames: Vec::new(),
                    socket_status,
                    last_frame_id: record.last_frame_id,
                    has_more: false,
                    is_monitored: false,
                };
                return json_response(&response);
            }
        }

        error_response(
            StatusCode::NOT_FOUND,
            &format!("Connection {} not found", connection_id),
        )
    }
}

pub async fn subscribe_frames(state: Arc<AdminState>, connection_id: &str) -> Response<BoxBody> {
    let monitor = &state.connection_monitor;

    if !monitor.start_monitoring(connection_id) {
        return error_response(
            StatusCode::NOT_FOUND,
            &format!("Connection {} not found", connection_id),
        );
    }

    let receiver = match monitor.subscribe_connection(connection_id) {
        Some(rx) => rx,
        None => {
            return error_response(
                StatusCode::NOT_FOUND,
                &format!("Connection {} not found", connection_id),
            );
        }
    };

    let connection_id_owned = connection_id.to_string();
    let stream = BroadcastStream::new(receiver).filter_map(move |result| match result {
        Ok(event) if event.connection_id == connection_id_owned => {
            let data = serde_json::to_string(&event.frame).ok()?;
            let sse_data = format!("data: {}\n\n", data);
            Some(sse_data)
        }
        _ => None,
    });

    let body_stream = http_body_util::StreamBody::new(
        stream.map(|s| Ok::<_, hyper::Error>(hyper::body::Frame::data(Bytes::from(s)))),
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("Access-Control-Allow-Origin", "*")
        .body(BoxBody::new(body_stream))
        .unwrap()
}

pub async fn unsubscribe_frames(state: Arc<AdminState>, connection_id: &str) -> Response<BoxBody> {
    let monitor = &state.connection_monitor;

    if monitor.stop_monitoring(connection_id) {
        let body = serde_json::json!({
            "success": true,
            "message": format!("Stopped monitoring connection {}", connection_id)
        });
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(full_body(body.to_string()))
            .unwrap()
    } else {
        error_response(
            StatusCode::NOT_FOUND,
            &format!("Connection {} not found", connection_id),
        )
    }
}

#[derive(Debug, Serialize)]
struct WebSocketConnectionsResponse {
    connections: Vec<WebSocketConnectionInfo>,
    total: usize,
}

#[derive(Debug, Serialize)]
struct WebSocketConnectionInfo {
    id: String,
    frame_count: usize,
    socket_status: Option<SocketStatus>,
    is_monitored: bool,
}

pub async fn list_websocket_connections(state: Arc<AdminState>) -> Response<BoxBody> {
    let monitor = &state.connection_monitor;
    let connection_ids = monitor.active_connection_ids();

    let connections: Vec<WebSocketConnectionInfo> = connection_ids
        .iter()
        .filter_map(|id| {
            Some(WebSocketConnectionInfo {
                id: id.clone(),
                frame_count: monitor.get_frame_count(id)?,
                socket_status: monitor.get_status(id),
                is_monitored: monitor.is_monitored(id),
            })
        })
        .collect();

    let total = connections.len();
    let response = WebSocketConnectionsResponse { connections, total };

    json_response(&response)
}

pub async fn get_frame_detail(
    state: Arc<AdminState>,
    connection_id: &str,
    frame_id: u64,
) -> Response<BoxBody> {
    let monitor = &state.connection_monitor;

    let frame = {
        let connections = monitor.connections.read();
        connections.get(connection_id).and_then(|store| {
            store
                .frames
                .iter()
                .find(|f| f.frame_id == frame_id)
                .cloned()
        })
    }
    .or_else(|| {
        state.frame_store.as_ref().and_then(|fs| {
            fs.flush();
            let pending = fs.load_pending_frames(connection_id, None, usize::MAX);
            pending
                .into_iter()
                .find(|f| f.frame_id == frame_id)
                .or_else(|| fs.load_frame_by_id(connection_id, frame_id).ok().flatten())
        })
    });

    if let Some(frame) = frame {
        if let Some(ref body_ref) = frame.payload_ref {
            if let BodyRef::Inline { data } = body_ref {
                let body = serde_json::json!({
                    "frame": frame.clone(),
                    "full_payload": data
                });
                return json_response(&body);
            }
            if let Some(ref ws_payload_store) = state.ws_payload_store {
                if ws_payload_store.is_ws_payload_ref(body_ref) {
                    let body_ref_clone = body_ref.clone();
                    let store_clone = ws_payload_store.clone();
                    let frame_clone = frame.clone();
                    let data = tokio::task::spawn_blocking(move || {
                        store_clone.read_range(&body_ref_clone)
                    })
                    .await
                    .ok()
                    .flatten();

                    if let Some(payload_bytes) = data {
                        let payload = match frame_clone.frame_type {
                            FrameType::Text | FrameType::Close | FrameType::Sse => {
                                String::from_utf8_lossy(&payload_bytes).to_string()
                            }
                            _ => base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                payload_bytes,
                            ),
                        };
                        let body = serde_json::json!({
                            "frame": frame_clone,
                            "full_payload": payload
                        });
                        return json_response(&body);
                    }
                }
            }
            if let Some(ref body_store) = state.body_store {
                let body_ref_clone = body_ref.clone();
                let body_store_clone = body_store.clone();
                let frame_clone = frame.clone();

                let data = tokio::task::spawn_blocking(move || {
                    let store = body_store_clone.read();
                    store.load(&body_ref_clone)
                })
                .await
                .ok()
                .flatten();

                if let Some(payload_data) = data {
                    let body = serde_json::json!({
                        "frame": frame_clone,
                        "full_payload": payload_data
                    });
                    return json_response(&body);
                }
            }
        }

        if frame.frame_type == FrameType::Sse {
            let record = if let Some(ref db_store) = state.traffic_db_store {
                let db_store = db_store.clone();
                let id = connection_id.to_string();
                tokio::task::spawn_blocking(move || db_store.get_by_id(&id))
                    .await
                    .ok()
                    .flatten()
            } else if let Some(ref traffic_store) = state.traffic_store {
                traffic_store.get_by_id(connection_id)
            } else {
                None
            };

            if let Some(record) = record {
                if let (Some(body_store), Some(body_ref)) =
                    (state.body_store.clone(), record.response_body_ref.clone())
                {
                    let data =
                        tokio::task::spawn_blocking(move || body_store.read().load(&body_ref))
                            .await
                            .ok()
                            .flatten();

                    if let Some(raw) = data {
                        let mut events = Vec::new();
                        let mut current = String::new();
                        for line in raw.lines() {
                            if line.is_empty() {
                                if !current.is_empty() {
                                    events.push(std::mem::take(&mut current));
                                }
                                continue;
                            }
                            current.push_str(line);
                            current.push('\n');
                        }
                        if !current.is_empty() {
                            events.push(current);
                        }

                        if let Some(payload) = events.get(frame_id as usize) {
                            let body = serde_json::json!({
                                "frame": frame.clone(),
                                "full_payload": payload
                            });
                            return json_response(&body);
                        }
                    }
                }
            }
        }

        json_response(&frame)
    } else {
        error_response(
            StatusCode::NOT_FOUND,
            &format!(
                "Frame {} not found in connection {}",
                frame_id, connection_id
            ),
        )
    }
}

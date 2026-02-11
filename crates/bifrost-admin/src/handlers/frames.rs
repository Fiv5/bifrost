use std::sync::Arc;

use bytes::Bytes;
use hyper::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use super::{error_response, full_body, json_response, BoxBody};
use crate::state::AdminState;
use crate::traffic::SocketStatus;
use crate::websocket_monitor::WebSocketFrameRecord;

#[derive(Debug, Serialize)]
struct FramesResponse {
    frames: Vec<WebSocketFrameRecord>,
    socket_status: Option<SocketStatus>,
    last_frame_id: u64,
    has_more: bool,
    is_monitored: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct FramesQuery {
    #[serde(default)]
    pub after: Option<u64>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

fn parse_frames_query(query: Option<&str>) -> FramesQuery {
    query
        .and_then(|q| serde_urlencoded::from_str(q).ok())
        .unwrap_or_default()
}

pub async fn get_frames(
    state: Arc<AdminState>,
    connection_id: &str,
    query_str: Option<&str>,
) -> Response<BoxBody> {
    let query = parse_frames_query(query_str);
    let monitor = &state.websocket_monitor;

    let frames_result = monitor.get_frames(connection_id, query.after, query.limit);

    match frames_result {
        Some((frames, has_more)) => {
            let socket_status = monitor.get_status(connection_id);
            let last_frame_id = monitor.get_last_frame_id(connection_id).unwrap_or(0);
            let is_monitored = monitor.is_monitored(connection_id);

            let response = FramesResponse {
                frames,
                socket_status,
                last_frame_id,
                has_more,
                is_monitored,
            };

            json_response(&response)
        }
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Connection {} not found", connection_id),
        ),
    }
}

pub async fn subscribe_frames(state: Arc<AdminState>, connection_id: &str) -> Response<BoxBody> {
    let monitor = &state.websocket_monitor;

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
    let monitor = &state.websocket_monitor;

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
    let monitor = &state.websocket_monitor;
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
    let monitor = &state.websocket_monitor;

    let frames_result = monitor.get_frames(connection_id, None, usize::MAX);

    match frames_result {
        Some((frames, _)) => {
            if let Some(frame) = frames.iter().find(|f| f.frame_id == frame_id) {
                if let Some(ref body_ref) = frame.payload_ref {
                    if let Some(ref body_store) = state.body_store {
                        let store = body_store.read();
                        if let Some(data) = store.load(body_ref) {
                            let body = serde_json::json!({
                                "frame": frame,
                                "full_payload": data
                            });
                            return json_response(&body);
                        }
                    }
                }
                json_response(frame)
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
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Connection {} not found", connection_id),
        ),
    }
}

use std::time::Duration;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use tokio_stream::StreamExt;

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::search::{SearchEngine, SearchProgress, SearchRequest};
use crate::state::SharedAdminState;
use crate::traffic_db::TrafficSummaryCompact;

const SEARCH_HANDLER_TIMEOUT: Duration = Duration::from_secs(310);

fn enrich_compact_frame_info(summary: &mut TrafficSummaryCompact, state: &SharedAdminState) {
    state.reconcile_socket_summary(summary);
}

pub async fn handle_search(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    let path = path.trim_end_matches('/');
    if path == "/api/search" {
        match method {
            Method::POST => execute_search(req, state).await,
            _ => method_not_allowed(),
        }
    } else if path == "/api/search/stream" {
        match method {
            Method::POST => execute_search_stream(req, state).await,
            _ => method_not_allowed(),
        }
    } else {
        error_response(StatusCode::NOT_FOUND, "Search endpoint not found")
    }
}

async fn execute_search(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read request body: {}", e),
            );
        }
    };

    let search_request: SearchRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid search request: {}", e),
            );
        }
    };

    if search_request.keyword.trim().is_empty() && !search_request.filters.has_constraints() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Search keyword cannot be empty without any filters",
        );
    }

    let traffic_db = match &state.traffic_db_store {
        Some(db) => db.clone(),
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Traffic database not available",
            );
        }
    };

    let body_store = state.body_store.clone();
    let frame_store = state.frame_store.clone();
    let connection_monitor = Some(state.connection_monitor.clone());

    let search_future = tokio::task::spawn_blocking(move || {
        let engine = SearchEngine::with_frame_support(
            traffic_db,
            body_store,
            frame_store,
            connection_monitor,
        );
        engine.search(&search_request)
    });

    let search_result = match tokio::time::timeout(SEARCH_HANDLER_TIMEOUT, search_future).await {
        Ok(join_result) => join_result,
        Err(_) => {
            return error_response(
                StatusCode::GATEWAY_TIMEOUT,
                "Search timed out. Try narrowing your search with filters or a more specific keyword.",
            );
        }
    };

    match search_result {
        Ok(mut response) => {
            for result in &mut response.results {
                let mut record = result.record.clone();
                enrich_compact_frame_info(&mut record, &state);
                result.record = record;
            }
            json_response(&response)
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Search failed: {}", e),
        ),
    }
}

#[derive(Debug, serde::Serialize)]
struct SearchStreamProgressPayload {
    total_searched: usize,
    total_matched: usize,
    next_cursor: Option<u64>,
    has_more_hint: bool,
    iterations: usize,
}

#[derive(Debug, serde::Serialize)]
struct SearchStreamDonePayload {
    total_searched: usize,
    total_matched: usize,
    next_cursor: Option<u64>,
    has_more: bool,
    search_id: String,
}

async fn execute_search_stream(
    req: Request<Incoming>,
    state: SharedAdminState,
) -> Response<BoxBody> {
    let body_bytes = match req.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read request body: {}", e),
            );
        }
    };

    let search_request: SearchRequest = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid search request: {}", e),
            );
        }
    };

    if search_request.keyword.trim().is_empty() && !search_request.filters.has_constraints() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Search keyword cannot be empty without any filters",
        );
    }

    let traffic_db = match &state.traffic_db_store {
        Some(db) => db.clone(),
        None => {
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "Traffic database not available",
            );
        }
    };

    let body_store = state.body_store.clone();
    let frame_store = state.frame_store.clone();
    let connection_monitor = Some(state.connection_monitor.clone());

    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);

    tokio::task::spawn_blocking(move || {
        let engine = SearchEngine::with_frame_support(
            traffic_db,
            body_store,
            frame_store,
            connection_monitor,
        );

        let mut last_progress: Option<SearchProgress> = None;

        let response = engine.search_stream(
            &search_request,
            |item| {
                if let Ok(json) = serde_json::to_string(item) {
                    let _ = tx.blocking_send(Bytes::from(sse_event("result", &json)));
                }
            },
            |p| {
                // 避免过于频繁的进度推送：只在关键字段变化时发送
                let changed = last_progress
                    .as_ref()
                    .map(|prev| {
                        prev.total_searched != p.total_searched
                            || prev.total_matched != p.total_matched
                            || prev.cursor != p.cursor
                            || prev.iterations != p.iterations
                            || prev.has_more_hint != p.has_more_hint
                    })
                    .unwrap_or(true);
                if !changed {
                    return;
                }
                last_progress = Some(p.clone());

                let payload = SearchStreamProgressPayload {
                    total_searched: p.total_searched,
                    total_matched: p.total_matched,
                    next_cursor: p.cursor,
                    has_more_hint: p.has_more_hint,
                    iterations: p.iterations,
                };

                if let Ok(json) = serde_json::to_string(&payload) {
                    let _ = tx.blocking_send(Bytes::from(sse_event("progress", &json)));
                }
            },
        );

        let done = SearchStreamDonePayload {
            total_searched: response.total_searched,
            total_matched: response.total_matched,
            next_cursor: response.next_cursor,
            has_more: response.has_more,
            search_id: response.search_id,
        };
        if let Ok(json) = serde_json::to_string(&done) {
            let _ = tx.blocking_send(Bytes::from(sse_event("done", &json)));
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(|b| Ok::<_, hyper::Error>(hyper::body::Frame::data(b)));

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

fn sse_event(event: &str, json_data: &str) -> String {
    // SSE 数据行必须以 data: 开头，事件以空行结束
    // 这里保证 json_data 不包含换行，避免破坏 SSE 帧。
    let data = json_data.replace('\n', "\\n");
    format!("event: {}\ndata: {}\n\n", event, data)
}

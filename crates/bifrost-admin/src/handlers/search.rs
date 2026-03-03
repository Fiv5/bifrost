use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::search::{SearchEngine, SearchRequest};
use crate::state::SharedAdminState;
use crate::traffic::SocketStatus;
use crate::traffic_db::TrafficSummaryCompact;

fn enrich_compact_frame_info(summary: &mut TrafficSummaryCompact, state: &SharedAdminState) {
    if !summary.is_sse() && !summary.is_websocket() && !summary.is_tunnel() {
        return;
    }

    if let Some(status) = state.connection_monitor.get_connection_status(&summary.id) {
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
}

pub async fn handle_search(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    if path == "/api/search" || path == "/api/search/" {
        match method {
            Method::POST => execute_search(req, state).await,
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

    if search_request.keyword.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Search keyword cannot be empty");
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

    let search_result = tokio::task::spawn_blocking(move || {
        let engine = SearchEngine::with_frame_support(
            traffic_db,
            body_store,
            frame_store,
            connection_monitor,
        );
        engine.search(&search_request)
    })
    .await;

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

use hyper::{body::Incoming, Method, Request, Response, StatusCode};

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

    let records = state.traffic_recorder.filter(&filter);

    let (offset, limit) = (filter.offset.unwrap_or(0), filter.limit.unwrap_or(100));

    let total = records.len();
    let paginated: Vec<_> = records.into_iter().skip(offset).take(limit).collect();

    let response = serde_json::json!({
        "total": total,
        "offset": offset,
        "limit": limit,
        "records": paginated
    });

    json_response(&response)
}

async fn get_traffic_detail(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    match state.traffic_recorder.get_by_id(id) {
        Some(record) => json_response(&record),
        None => error_response(
            StatusCode::NOT_FOUND,
            &format!("Traffic record '{}' not found", id),
        ),
    }
}

async fn clear_traffic(state: SharedAdminState) -> Response<BoxBody> {
    state.traffic_recorder.clear();
    success_response("Traffic records cleared successfully")
}

async fn get_request_body(state: SharedAdminState, id: &str) -> Response<BoxBody> {
    match state.traffic_recorder.get_by_id(id) {
        Some(record) => {
            if let Some(body_ref) = &record.request_body_ref {
                get_body_content(&state, body_ref)
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
    match state.traffic_recorder.get_by_id(id) {
        Some(record) => {
            if let Some(body_ref) = &record.response_body_ref {
                get_body_content(&state, body_ref)
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

fn get_body_content(state: &SharedAdminState, body_ref: &BodyRef) -> Response<BoxBody> {
    match body_ref {
        BodyRef::Inline { data } => json_response(&serde_json::json!({
            "success": true,
            "data": data
        })),
        BodyRef::File { path, size } => {
            if let Some(ref body_store) = state.body_store {
                let store = body_store.read();
                match store.load(body_ref) {
                    Some(data) => json_response(&serde_json::json!({
                        "success": true,
                        "data": data
                    })),
                    None => error_response(
                        StatusCode::NOT_FOUND,
                        &format!("Body file not found: {}", path),
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
                _ => {}
            }
        }
    }

    filter
}

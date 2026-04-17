use http_body_util::BodyExt;
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{cors_preflight, error_response, json_response, method_not_allowed, BoxBody};
use crate::notification_db;
use crate::state::SharedAdminState;

#[derive(Debug, Serialize)]
struct NotificationListResponse {
    total: i64,
    unread_count: i64,
    items: Vec<notification_db::NotificationRecord>,
    limit: usize,
    offset: usize,
}

#[derive(Debug, Serialize)]
struct ClientTrustResponse {
    items: Vec<crate::client_trust_tracker::ClientTrustSummary>,
    untrusted_count: usize,
}

#[derive(Debug, Deserialize)]
struct UpdateStatusRequest {
    status: String,
    action_taken: Option<String>,
}

fn parse_query_param(uri: &hyper::Uri, key: &str) -> Option<String> {
    let q = uri.query()?;
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?.trim();
        if k != key {
            continue;
        }
        return Some(it.next().unwrap_or("").to_string());
    }
    None
}

fn parse_usize_query(uri: &hyper::Uri, key: &str) -> Option<usize> {
    parse_query_param(uri, key)?.parse::<usize>().ok()
}

pub async fn handle_notification(
    req: Request<hyper::body::Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    if req.method() == Method::OPTIONS {
        return cors_preflight();
    }

    let sub_path = path.strip_prefix("/api/notifications").unwrap_or("");

    if sub_path.is_empty() || sub_path == "/" {
        return handle_list_notifications(req).await;
    }

    if sub_path == "/client-trust" {
        return handle_client_trust(req, state).await;
    }

    if sub_path == "/mark-all-read" {
        return handle_mark_all_read(req).await;
    }

    if sub_path == "/unread-count" {
        return handle_unread_count(req).await;
    }

    if let Some(id_str) = sub_path.strip_prefix('/') {
        if let Some(id_str) = id_str.strip_prefix("status/") {
            return handle_update_status(req, id_str).await;
        }
    }

    error_response(StatusCode::NOT_FOUND, "Notification endpoint not found")
}

async fn handle_list_notifications(req: Request<hyper::body::Incoming>) -> Response<BoxBody> {
    if *req.method() != Method::GET {
        return method_not_allowed();
    }

    let notification_type = parse_query_param(req.uri(), "type");
    let status = parse_query_param(req.uri(), "status");
    let mut limit = parse_usize_query(req.uri(), "limit").unwrap_or(50);
    let offset = parse_usize_query(req.uri(), "offset").unwrap_or(0);
    if limit == 0 {
        limit = 50;
    }
    limit = limit.min(500);

    let total =
        match notification_db::count_notifications(notification_type.as_deref(), status.as_deref())
        {
            Ok(v) => v,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Failed to count notifications: {e}"),
                )
            }
        };

    let unread_count = notification_db::count_unread().unwrap_or(0);

    let items = match notification_db::list_notifications(
        notification_type.as_deref(),
        status.as_deref(),
        limit,
        offset,
    ) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to query notifications: {e}"),
            )
        }
    };

    json_response(&NotificationListResponse {
        total,
        unread_count,
        items,
        limit,
        offset,
    })
}

async fn handle_client_trust(
    req: Request<hyper::body::Incoming>,
    state: SharedAdminState,
) -> Response<BoxBody> {
    if *req.method() == Method::GET {
        if let Some(ref tracker) = state.client_trust_tracker {
            let items = tracker.get_all_statuses();
            let untrusted_count = tracker.get_untrusted_count();
            json_response(&ClientTrustResponse {
                items,
                untrusted_count,
            })
        } else {
            json_response(&ClientTrustResponse {
                items: vec![],
                untrusted_count: 0,
            })
        }
    } else if *req.method() == Method::DELETE {
        if let Some(ref tracker) = state.client_trust_tracker {
            tracker.clear();
        }
        json_response(&serde_json::json!({"success": true, "message": "Client trust data cleared"}))
    } else {
        method_not_allowed()
    }
}

async fn handle_mark_all_read(req: Request<hyper::body::Incoming>) -> Response<BoxBody> {
    if *req.method() != Method::POST {
        return method_not_allowed();
    }

    let notification_type = parse_query_param(req.uri(), "type");

    match notification_db::mark_all_as_read(notification_type.as_deref()) {
        Ok(count) => json_response(&serde_json::json!({
            "success": true,
            "updated": count,
        })),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to mark notifications as read: {e}"),
        ),
    }
}

async fn handle_unread_count(req: Request<hyper::body::Incoming>) -> Response<BoxBody> {
    if *req.method() != Method::GET {
        return method_not_allowed();
    }

    match notification_db::count_unread() {
        Ok(count) => json_response(&serde_json::json!({ "unread_count": count })),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to count unread notifications: {e}"),
        ),
    }
}

async fn handle_update_status(
    req: Request<hyper::body::Incoming>,
    id_str: &str,
) -> Response<BoxBody> {
    if *req.method() != Method::PUT {
        return method_not_allowed();
    }

    let id: i64 = match id_str.parse() {
        Ok(v) => v,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "Invalid notification ID"),
    };

    let body_bytes = match req.collect().await {
        Ok(body) => body.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read request body: {e}"),
            )
        }
    };

    let update: UpdateStatusRequest = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Invalid request body: {e}"),
            )
        }
    };

    match notification_db::update_notification_status(
        id,
        &update.status,
        update.action_taken.as_deref(),
    ) {
        Ok(true) => json_response(&serde_json::json!({"success": true})),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Notification not found"),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to update notification: {e}"),
        ),
    }
}

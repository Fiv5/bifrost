use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use tracing::debug;

use super::{error_response, full_body, BoxBody};
use crate::state::SharedAdminState;

pub async fn handle_room(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let Some(sync_manager) = state.sync_manager.clone() else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Sync manager not available",
        );
    };

    let method = req.method().clone();
    let query = req.uri().query().map(|q| q.to_string());

    let body = match req.collect().await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            if bytes.is_empty() {
                None
            } else {
                Some(bytes.to_vec())
            }
        }
        Err(error) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {error}"),
            )
        }
    };

    let is_create_room = method == Method::POST && path == "/api/room";

    let (remote_path, adapted_body) = if is_create_room {
        match adapt_create_room_to_invite(&body) {
            Some(invite_body) => ("/v4/group/invite".to_string(), Some(invite_body)),
            None => {
                return error_response(StatusCode::BAD_REQUEST, "Invalid create room request body");
            }
        }
    } else {
        (path.replacen("/api/room", "/v4/room", 1), body)
    };

    let reqwest_method = match method {
        Method::GET => reqwest::Method::GET,
        Method::POST => reqwest::Method::POST,
        Method::PUT => reqwest::Method::PUT,
        Method::PATCH => reqwest::Method::PATCH,
        Method::DELETE => reqwest::Method::DELETE,
        _ => {
            return error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed");
        }
    };

    match sync_manager
        .proxy_forward(reqwest_method, &remote_path, query.as_deref(), adapted_body)
        .await
    {
        Ok((status, content_type, response_body)) => {
            let status_code =
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Response::builder()
                .status(status_code)
                .header("Content-Type", content_type)
                .body(full_body(response_body))
                .unwrap()
        }
        Err(error) => error_response(
            StatusCode::BAD_GATEWAY,
            &format!("Failed to proxy room request: {error}"),
        ),
    }
}

fn adapt_create_room_to_invite(body: &Option<Vec<u8>>) -> Option<Vec<u8>> {
    let bytes = body.as_ref()?;
    let parsed: serde_json::Value = serde_json::from_slice(bytes).ok()?;

    let group_id = parsed.get("group_id")?.as_str()?;
    let user_id = parsed.get("user_id")?.as_str()?;
    let level = parsed.get("level").and_then(|v| v.as_i64()).unwrap_or(0);

    let invite_body = serde_json::json!({
        "group_id": group_id,
        "user_id": [user_id],
        "level": level,
    });

    debug!(
        group_id = %group_id,
        user_id = %user_id,
        level = level,
        "adapted POST /room to POST /v4/group/invite"
    );

    serde_json::to_vec(&invite_body).ok()
}

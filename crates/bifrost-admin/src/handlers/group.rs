use http_body_util::BodyExt;
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use tracing::debug;

use super::{error_response, full_body, BoxBody};
use crate::state::SharedAdminState;
use bifrost_sync::SharedSyncManager;

fn is_single_group_detail(method: &Method, api_path: &str) -> Option<String> {
    if *method != Method::GET {
        return None;
    }
    let sub = api_path.strip_prefix("/api/group/")?;
    if sub.is_empty() || sub.contains('/') {
        return None;
    }
    Some(sub.to_string())
}

async fn enrich_group_detail(
    sync_manager: &SharedSyncManager,
    group_id: &str,
    mut response_body: Vec<u8>,
) -> Vec<u8> {
    let parsed: serde_json::Value = match serde_json::from_slice(&response_body) {
        Ok(v) => v,
        Err(_) => return response_body,
    };

    let has_visibility = parsed
        .pointer("/data/visibility")
        .is_some_and(|v| !v.is_null());
    let has_level = parsed.pointer("/data/level").is_some_and(|v| !v.is_null());

    if has_visibility || has_level {
        return response_body;
    }

    let setting_path = format!("/v4/group/{}/setting", group_id);
    let setting_result = sync_manager
        .proxy_forward(reqwest::Method::GET, &setting_path, None, None)
        .await;

    if let Ok((200, _, setting_bytes)) = setting_result {
        if let Ok(setting_json) = serde_json::from_slice::<serde_json::Value>(&setting_bytes) {
            let level = setting_json
                .pointer("/data/level")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let visibility = if level == 1 { 1 } else { 0 };

            if let Ok(mut root) = serde_json::from_slice::<serde_json::Value>(&response_body) {
                if let Some(data) = root.get_mut("data").and_then(|d| d.as_object_mut()) {
                    data.insert(
                        "visibility".to_string(),
                        serde_json::Value::Number(visibility.into()),
                    );
                    data.insert("level".to_string(), serde_json::Value::Number(level.into()));
                    if let Ok(enriched) = serde_json::to_vec(&root) {
                        debug!(group_id = %group_id, level = level, "enriched group detail with visibility from setting");
                        response_body = enriched;
                    }
                }
            }
        }
    }

    response_body
}

pub async fn handle_group(
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
    let remote_path = path.replacen("/api/group", "/v4/group", 1);
    let detail_group_id = is_single_group_detail(&method, path);

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
        .proxy_forward(reqwest_method, &remote_path, query.as_deref(), body)
        .await
    {
        Ok((status, content_type, mut response_body)) => {
            if status == 200 {
                if let Some(ref group_id) = detail_group_id {
                    response_body =
                        enrich_group_detail(&sync_manager, group_id, response_body).await;
                }
            }

            let status_code =
                StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            Response::builder()
                .status(status_code)
                .header("Content-Type", content_type)
                .header("Access-Control-Allow-Origin", "*")
                .body(full_body(response_body))
                .unwrap()
        }
        Err(error) => error_response(
            StatusCode::BAD_GATEWAY,
            &format!("Failed to proxy group request: {error}"),
        ),
    }
}

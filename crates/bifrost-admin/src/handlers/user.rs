use http_body_util::BodyExt;
use hyper::{body::Incoming, Request, Response, StatusCode};

use super::{error_response, full_body, BoxBody};
use crate::state::SharedAdminState;

pub async fn handle_user(
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
    let remote_path = path.replacen("/api/user", "/v4/user", 1);

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
        hyper::Method::GET => reqwest::Method::GET,
        hyper::Method::POST => reqwest::Method::POST,
        hyper::Method::PUT => reqwest::Method::PUT,
        hyper::Method::PATCH => reqwest::Method::PATCH,
        hyper::Method::DELETE => reqwest::Method::DELETE,
        _ => {
            return error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed");
        }
    };

    match sync_manager
        .proxy_forward(reqwest_method, &remote_path, query.as_deref(), body)
        .await
    {
        Ok((status, content_type, response_body)) => {
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
            &format!("Failed to proxy user request: {error}"),
        ),
    }
}

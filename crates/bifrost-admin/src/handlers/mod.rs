pub mod app_icon;
pub mod bifrost_file;
pub mod cert;
pub mod config;
pub mod frames;
pub mod metrics;
pub mod proxy;
pub mod replay;
mod replay_ws;
pub mod rules;
pub mod scripts;
pub mod search;
pub mod syntax;
pub mod system;
pub mod traffic;
pub mod values;
pub mod websocket;
pub mod whitelist;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Response, StatusCode};
use serde::Serialize;

pub type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;
pub const ADMIN_CORS_ALLOW_HEADERS: &str = "Content-Type, Authorization, X-Client-Id";

pub fn full_body(body: impl Into<Bytes>) -> BoxBody {
    http_body_util::Full::new(body.into())
        .map_err(|e| match e {})
        .boxed()
}

pub fn empty_body() -> BoxBody {
    http_body_util::Empty::new().map_err(|e| match e {}).boxed()
}

pub fn json_response<T: Serialize>(data: &T) -> Response<BoxBody> {
    json_response_with_status(StatusCode::OK, data)
}

pub fn json_response_with_status<T: Serialize>(status: StatusCode, data: &T) -> Response<BoxBody> {
    match serde_json::to_string(data) {
        Ok(json) => Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(full_body(json))
            .unwrap(),
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("JSON serialization error: {}", e),
        ),
    }
}

pub fn error_response(status: StatusCode, message: &str) -> Response<BoxBody> {
    let body = serde_json::json!({
        "error": message,
        "status": status.as_u16()
    });
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(full_body(body.to_string()))
        .unwrap()
}

pub fn success_response(message: &str) -> Response<BoxBody> {
    let body = serde_json::json!({
        "success": true,
        "message": message
    });
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .body(full_body(body.to_string()))
        .unwrap()
}

#[allow(dead_code)]
pub fn not_found() -> Response<BoxBody> {
    error_response(StatusCode::NOT_FOUND, "Not Found")
}

pub fn method_not_allowed() -> Response<BoxBody> {
    error_response(StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed")
}

pub fn cors_preflight() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header("Access-Control-Allow-Origin", "*")
        .header(
            "Access-Control-Allow-Methods",
            "GET, POST, PUT, DELETE, OPTIONS",
        )
        .header("Access-Control-Allow-Headers", ADMIN_CORS_ALLOW_HEADERS)
        .header("Access-Control-Max-Age", "86400")
        .body(empty_body())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::{cors_preflight, ADMIN_CORS_ALLOW_HEADERS};

    #[test]
    fn cors_preflight_allows_desktop_client_header() {
        let response = cors_preflight();
        let headers = response.headers();

        assert_eq!(
            headers
                .get("Access-Control-Allow-Headers")
                .and_then(|value| value.to_str().ok()),
            Some(ADMIN_CORS_ALLOW_HEADERS)
        );
        assert!(
            ADMIN_CORS_ALLOW_HEADERS.contains("X-Client-Id"),
            "desktop requests require X-Client-Id to pass CORS preflight"
        );
    }
}

pub mod cert;
pub mod metrics;
pub mod rules;
pub mod system;
pub mod traffic;
pub mod values;
pub mod whitelist;

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::{Response, StatusCode};
use serde::Serialize;

pub type BoxBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

pub fn full_body(body: impl Into<Bytes>) -> BoxBody {
    http_body_util::Full::new(body.into())
        .map_err(|e| match e {})
        .boxed()
}

pub fn empty_body() -> BoxBody {
    http_body_util::Empty::new().map_err(|e| match e {}).boxed()
}

pub fn json_response<T: Serialize>(data: &T) -> Response<BoxBody> {
    match serde_json::to_string(data) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
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
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
        .header("Access-Control-Max-Age", "86400")
        .body(empty_body())
        .unwrap()
}

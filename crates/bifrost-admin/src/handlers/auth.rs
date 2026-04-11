use hyper::{body::Incoming, header, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{cors_preflight, error_response, json_response, method_not_allowed, BoxBody};
use crate::admin_audit;
use crate::admin_auth::{
    get_admin_username, is_remote_access_enabled, issue_admin_jwt, verify_admin_credentials,
};
use crate::state::SharedAdminState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: String,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusResponse {
    pub remote_access_enabled: bool,
    pub auth_required: bool,
    pub username: String,
}

pub async fn handle_auth(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    if req.method() == Method::OPTIONS {
        return cors_preflight();
    }

    if path == "/api/auth/status" {
        if *req.method() != Method::GET {
            return method_not_allowed();
        }
        let remote_enabled = is_remote_access_enabled(&state);
        let username = get_admin_username(&state).unwrap_or_else(|| "admin".to_string());
        return json_response(&AuthStatusResponse {
            remote_access_enabled: remote_enabled,
            auth_required: remote_enabled,
            username,
        });
    }

    if path == "/api/auth/login" {
        if *req.method() != Method::POST {
            return method_not_allowed();
        }

        if !is_remote_access_enabled(&state) {
            // 默认本地管理端不需要登录；仅在开启远程访问时启用鉴权。
            return error_response(StatusCode::FORBIDDEN, "Remote admin access is not enabled");
        }

        let peer_ip = req
            .headers()
            .get("x-bifrost-peer-ip")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();
        let user_agent = req
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = match http_body_util::BodyExt::collect(req.into_body()).await {
            Ok(collected) => collected.to_bytes(),
            Err(_) => {
                return error_response(StatusCode::BAD_REQUEST, "Failed to read request body")
            }
        };

        let login: LoginRequest = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {e}"))
            }
        };

        let ok = match verify_admin_credentials(&state, &login.username, &login.password) {
            Ok(v) => v,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Auth error: {e}"),
                )
            }
        };
        if !ok {
            return error_response(StatusCode::UNAUTHORIZED, "Invalid credentials");
        }

        let (token, claims) = match issue_admin_jwt(&state, &login.username) {
            Ok(v) => v,
            Err(e) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Failed to issue token: {e}"),
                )
            }
        };

        if let Err(e) = admin_audit::record_login(&login.username, &peer_ip, &user_agent) {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to write audit log: {e}"),
            );
        }
        let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(claims.exp, 0)
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        // 对于浏览器/CLI：同时支持 Authorization: Bearer 与显式 token 返回。
        // 这里不使用 Cookie，避免引入 CSRF/跨域复杂度。
        let mut resp = json_response(&LoginResponse {
            token: token.clone(),
            expires_at,
            username: login.username,
        });
        resp.headers_mut().insert(
            header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
        return resp;
    }

    if path == "/api/auth/logout" {
        // 无状态 JWT：前端/CLI 丢弃 token 即完成注销。
        if *req.method() != Method::POST {
            return method_not_allowed();
        }
        return Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .body(super::full_body("{\"success\":true}"))
            .unwrap();
    }

    error_response(StatusCode::NOT_FOUND, "API endpoint not found")
}

pub fn extract_bearer_token(req: &Request<Incoming>) -> Option<String> {
    let header_val = req.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    let v = header_val.trim();
    let token = v
        .strip_prefix("Bearer ")
        .or_else(|| v.strip_prefix("bearer "))?;
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

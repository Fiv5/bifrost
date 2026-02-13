use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::state::SharedAdminState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub unsafe_ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySettingsResponse {
    pub tls: TlsConfig,
    pub port: u16,
    pub host: String,
}

#[derive(Deserialize)]
pub struct UpdateTlsConfigRequest {
    pub enable_tls_interception: Option<bool>,
    pub intercept_exclude: Option<Vec<String>>,
    pub unsafe_ssl: Option<bool>,
}

pub async fn handle_config(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/api/config" | "/api/config/" => match method {
            Method::GET => get_proxy_settings(state).await,
            _ => method_not_allowed(),
        },
        "/api/config/tls" | "/api/config/tls/" => match method {
            Method::GET => get_tls_config(state).await,
            Method::PUT => update_tls_config(req, state).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_proxy_settings(state: SharedAdminState) -> Response<BoxBody> {
    let runtime_config = state.runtime_config.read().await;

    let response = ProxySettingsResponse {
        tls: TlsConfig {
            enable_tls_interception: runtime_config.enable_tls_interception,
            intercept_exclude: runtime_config.intercept_exclude.clone(),
            unsafe_ssl: runtime_config.unsafe_ssl,
        },
        port: state.port,
        host: "127.0.0.1".to_string(),
    };

    json_response(&response)
}

async fn get_tls_config(state: SharedAdminState) -> Response<BoxBody> {
    let runtime_config = state.runtime_config.read().await;

    let tls_config = TlsConfig {
        enable_tls_interception: runtime_config.enable_tls_interception,
        intercept_exclude: runtime_config.intercept_exclude.clone(),
        unsafe_ssl: runtime_config.unsafe_ssl,
    };

    json_response(&tls_config)
}

async fn update_tls_config(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    use http_body_util::BodyExt;

    let body = match req.collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            )
        }
    };

    let request: UpdateTlsConfigRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    {
        let mut runtime_config = state.runtime_config.write().await;

        if let Some(enable) = request.enable_tls_interception {
            runtime_config.enable_tls_interception = enable;
        }

        if let Some(exclude) = request.intercept_exclude {
            runtime_config.intercept_exclude = exclude;
        }

        if let Some(unsafe_ssl) = request.unsafe_ssl {
            runtime_config.unsafe_ssl = unsafe_ssl;
        }
    }

    let runtime_config = state.runtime_config.read().await;
    let tls_config = TlsConfig {
        enable_tls_interception: runtime_config.enable_tls_interception,
        intercept_exclude: runtime_config.intercept_exclude.clone(),
        unsafe_ssl: runtime_config.unsafe_ssl,
    };

    json_response(&tls_config)
}

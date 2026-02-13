use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::state::SharedAdminState;
use bifrost_core::SystemProxyManager;

#[derive(Serialize)]
struct SystemProxyStatus {
    supported: bool,
    enabled: bool,
    host: String,
    port: u16,
    bypass: String,
}

#[derive(Serialize)]
struct SystemProxySupportStatus {
    supported: bool,
    platform: String,
}

#[derive(Deserialize)]
struct SetSystemProxyRequest {
    enabled: bool,
    bypass: Option<String>,
}

pub async fn handle_proxy(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();

    match path {
        "/api/proxy/system" | "/api/proxy/system/" => match method {
            Method::GET => get_system_proxy_status(state).await,
            Method::PUT => set_system_proxy(req, state).await,
            _ => method_not_allowed(),
        },
        "/api/proxy/system/support" => match method {
            Method::GET => get_system_proxy_support().await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_system_proxy_status(_state: SharedAdminState) -> Response<BoxBody> {
    if !SystemProxyManager::is_supported() {
        let status = SystemProxyStatus {
            supported: false,
            enabled: false,
            host: String::new(),
            port: 0,
            bypass: String::new(),
        };
        return json_response(&status);
    }

    match SystemProxyManager::get_current() {
        Ok(proxy) => {
            let status = SystemProxyStatus {
                supported: true,
                enabled: proxy.enable,
                host: proxy.host,
                port: proxy.port,
                bypass: proxy.bypass,
            };
            json_response(&status)
        }
        Err(e) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to get system proxy: {}", e),
        ),
    }
}

async fn set_system_proxy(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
    use http_body_util::BodyExt;

    if !SystemProxyManager::is_supported() {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "System proxy is not supported on this platform",
        );
    }

    let body = match req.collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => {
            return error_response(
                StatusCode::BAD_REQUEST,
                &format!("Failed to read body: {}", e),
            )
        }
    };

    let request: SetSystemProxyRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    let bypass = request
        .bypass
        .unwrap_or_else(|| "localhost,127.0.0.1,::1,*.local".to_string());

    if let Some(ref manager) = state.system_proxy_manager {
        let mut manager = manager.write().await;
        let host = "127.0.0.1";

        let result = if request.enabled {
            manager.enable(host, state.port, Some(&bypass))
        } else {
            manager.restore()
        };

        let final_result = match &result {
            Ok(()) => result,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("RequiresAdmin") {
                    tracing::info!("Permission denied, trying GUI authorization...");
                    #[cfg(target_os = "macos")]
                    {
                        if request.enabled {
                            manager.enable_with_gui_auth(host, state.port, Some(&bypass))
                        } else {
                            manager.restore_with_gui_auth()
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        result
                    }
                } else {
                    result
                }
            }
        };

        match final_result {
            Ok(()) => {
                let status = SystemProxyStatus {
                    supported: true,
                    enabled: request.enabled,
                    host: if request.enabled {
                        "127.0.0.1".to_string()
                    } else {
                        String::new()
                    },
                    port: if request.enabled { state.port } else { 0 },
                    bypass: if request.enabled {
                        bypass
                    } else {
                        String::new()
                    },
                };
                json_response(&status)
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("UserCancelled") {
                    #[derive(Serialize)]
                    struct UserCancelledError {
                        error: &'static str,
                        message: &'static str,
                    }
                    let body = UserCancelledError {
                        error: "user_cancelled",
                        message: "Authorization was cancelled by user.",
                    };
                    json_response(&body)
                } else if msg.contains("RequiresAdmin") {
                    #[derive(Serialize)]
                    struct AdminError {
                        error: &'static str,
                        message: &'static str,
                    }
                    let body = AdminError {
                        error: "requires_admin",
                        message: "System proxy requires administrator privileges. Please run the CLI with sudo or grant permission.",
                    };
                    json_response(&body)
                } else {
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Failed to set system proxy: {}", e),
                    )
                }
            }
        }
    } else {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "System proxy manager not initialized",
        )
    }
}

async fn get_system_proxy_support() -> Response<BoxBody> {
    let status = SystemProxySupportStatus {
        supported: SystemProxyManager::is_supported(),
        platform: get_platform_name(),
    };
    json_response(&status)
}

fn get_platform_name() -> String {
    #[cfg(target_os = "macos")]
    {
        "macOS".to_string()
    }
    #[cfg(target_os = "windows")]
    {
        "Windows".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "Linux".to_string()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "Unknown".to_string()
    }
}

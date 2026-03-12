use std::net::IpAddr;

use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{
    error_response, json_response, json_response_with_status, method_not_allowed, BoxBody,
};
use crate::state::SharedAdminState;
use bifrost_core::ShellProxyManager;
use bifrost_core::SystemProxyManager;
use bifrost_storage::SystemProxyConfigUpdate;

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

#[derive(Serialize)]
struct CliProxyStatus {
    enabled: bool,
    shell: String,
    config_files: Vec<String>,
    proxy_url: String,
}

#[derive(Deserialize)]
struct SetSystemProxyRequest {
    enabled: bool,
    bypass: Option<String>,
}

#[derive(Serialize)]
struct ProxyAddressInfo {
    port: u16,
    local_ips: Vec<String>,
    addresses: Vec<ProxyAddress>,
}

#[derive(Serialize)]
struct ProxyAddress {
    ip: String,
    address: String,
    qrcode_url: String,
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
        "/api/proxy/cli" | "/api/proxy/cli/" => match method {
            Method::GET => get_cli_proxy_status(state).await,
            _ => method_not_allowed(),
        },
        "/api/proxy/system/support" => match method {
            Method::GET => get_system_proxy_support().await,
            _ => method_not_allowed(),
        },
        "/api/proxy/address" | "/api/proxy/address/" => match method {
            Method::GET => get_proxy_address_info(state).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_cli_proxy_status(state: SharedAdminState) -> Response<BoxBody> {
    let Some(ref config_manager) = state.config_manager else {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Config manager not available",
        );
    };

    let data_dir = config_manager.data_dir().to_path_buf();
    let manager = ShellProxyManager::new(data_dir);
    let status = manager.status();

    let resp = CliProxyStatus {
        enabled: status.has_persistent_config,
        shell: status.shell_type.as_str().to_string(),
        config_files: status
            .config_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        proxy_url: format!("http://127.0.0.1:{}", state.port),
    };
    json_response(&resp)
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
            manager.force_disable()
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
                            manager.disable_with_gui_auth()
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
                if let Some(ref config_manager) = state.config_manager {
                    let update = SystemProxyConfigUpdate {
                        enabled: Some(request.enabled),
                        bypass: if request.enabled {
                            Some(bypass.clone())
                        } else {
                            None
                        },
                        auto_enable: None,
                    };
                    if let Err(e) = config_manager.update_system_proxy_config(update).await {
                        tracing::error!("Failed to persist system proxy config: {}", e);
                    } else {
                        tracing::info!(
                            "System proxy config persisted: enabled={}",
                            request.enabled
                        );
                    }
                }

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
                    json_response_with_status(StatusCode::FORBIDDEN, &body)
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
                    json_response_with_status(StatusCode::FORBIDDEN, &body)
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

async fn get_proxy_address_info(state: SharedAdminState) -> Response<BoxBody> {
    let local_ips = get_local_ips();
    let port = state.port;

    let addresses: Vec<ProxyAddress> = local_ips
        .iter()
        .map(|ip| ProxyAddress {
            ip: ip.clone(),
            address: format!("{}:{}", ip, port),
            qrcode_url: format!(
                "/_bifrost/public/proxy/qrcode?ip={}",
                urlencoding::encode(ip)
            ),
        })
        .collect();

    let info = ProxyAddressInfo {
        port,
        local_ips,
        addresses,
    };

    json_response(&info)
}

fn get_local_ips() -> Vec<String> {
    let mut ips = Vec::new();

    if let Ok(interfaces) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if interfaces.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = interfaces.local_addr() {
                ips.push(addr.ip().to_string());
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("hostname").arg("-I").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for ip_str in stdout.split_whitespace() {
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        if is_private_ip(&ip) && !ips.contains(&ip.to_string()) {
                            ips.push(ip.to_string());
                        }
                    }
                }
            }
        }
    }

    if ips.is_empty() {
        ips.push("127.0.0.1".to_string());
    }

    ips
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private() || ipv4.is_loopback() || ipv4.is_link_local(),
        IpAddr::V6(ipv6) => ipv6.is_loopback(),
    }
}

use bifrost_storage::{TlsConfigUpdate, TrafficConfigUpdate};
use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::body_store::{BodyStoreConfigUpdate, BodyStoreStats};
use crate::state::SharedAdminState;
use crate::status_printer::TlsStatusInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
    pub disconnect_on_config_change: bool,
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
    pub intercept_include: Option<Vec<String>>,
    pub unsafe_ssl: Option<bool>,
    pub disconnect_on_config_change: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficConfig {
    pub max_records: usize,
    pub max_body_memory_size: usize,
    pub max_body_buffer_size: usize,
    pub file_retention_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfigResponse {
    pub traffic: TrafficConfig,
    pub body_store_stats: Option<BodyStoreStats>,
}

#[derive(Deserialize)]
pub struct UpdateTrafficConfigRequest {
    pub max_records: Option<usize>,
    pub max_body_memory_size: Option<usize>,
    pub max_body_buffer_size: Option<usize>,
    pub file_retention_days: Option<u64>,
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
        "/api/config/performance" | "/api/config/performance/" => match method {
            Method::GET => get_performance_config(state).await,
            Method::PUT => update_performance_config(req, state).await,
            _ => method_not_allowed(),
        },
        "/api/config/performance/clear-cache" | "/api/config/performance/clear-cache/" => {
            match method {
                Method::DELETE => clear_body_cache(state).await,
                _ => method_not_allowed(),
            }
        }
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_proxy_settings(state: SharedAdminState) -> Response<BoxBody> {
    let runtime_config = state.runtime_config.read().await;

    let response = ProxySettingsResponse {
        tls: TlsConfig {
            enable_tls_interception: runtime_config.enable_tls_interception,
            intercept_exclude: runtime_config.intercept_exclude.clone(),
            intercept_include: runtime_config.intercept_include.clone(),
            unsafe_ssl: runtime_config.unsafe_ssl,
            disconnect_on_config_change: runtime_config.disconnect_on_config_change,
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
        intercept_include: runtime_config.intercept_include.clone(),
        unsafe_ssl: runtime_config.unsafe_ssl,
        disconnect_on_config_change: runtime_config.disconnect_on_config_change,
    };

    json_response(&tls_config)
}

async fn get_performance_config(state: SharedAdminState) -> Response<BoxBody> {
    let body_store_stats = state.body_store.as_ref().map(|bs| bs.read().stats());

    let traffic_config = if let Some(ref config_manager) = state.config_manager {
        let config = config_manager.config().await;
        TrafficConfig {
            max_records: config.traffic.max_records,
            max_body_memory_size: config.traffic.max_body_memory_size,
            max_body_buffer_size: config.traffic.max_body_buffer_size,
            file_retention_days: config.traffic.file_retention_days,
        }
    } else {
        TrafficConfig {
            max_records: 5000,
            max_body_memory_size: 2 * 1024 * 1024,
            max_body_buffer_size: 32 * 1024 * 1024,
            file_retention_days: 7,
        }
    };

    let response = PerformanceConfigResponse {
        traffic: traffic_config,
        body_store_stats,
    };

    json_response(&response)
}

async fn update_performance_config(
    req: Request<Incoming>,
    state: SharedAdminState,
) -> Response<BoxBody> {
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

    let request: UpdateTrafficConfigRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };

    if let Some(days) = request.file_retention_days {
        if days > 7 {
            return error_response(
                StatusCode::BAD_REQUEST,
                "file_retention_days cannot exceed 7 days",
            );
        }
    }

    if let Some(ref config_manager) = state.config_manager {
        let update = TrafficConfigUpdate {
            max_records: request.max_records,
            max_body_memory_size: request.max_body_memory_size,
            max_body_buffer_size: request.max_body_buffer_size,
            file_retention_days: request.file_retention_days,
        };

        if let Err(e) = config_manager.update_traffic_config(update).await {
            tracing::error!("Failed to persist traffic config: {}", e);
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to save config: {}", e),
            );
        }
        tracing::info!("Traffic config updated and persisted");
    } else {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Config manager not available",
        );
    }

    if let Some(max_records) = request.max_records {
        state.traffic_recorder.set_max_records(max_records);
    }

    if let Some(ref body_store) = state.body_store {
        let body_store_update = BodyStoreConfigUpdate {
            max_memory_size: request.max_body_memory_size,
            retention_days: request.file_retention_days,
        };
        body_store.write().update_config(body_store_update);
    }

    get_performance_config(state).await
}

#[derive(Debug, Clone, Serialize)]
struct ClearCacheResponse {
    removed_files: usize,
    message: String,
}

async fn clear_body_cache(state: SharedAdminState) -> Response<BoxBody> {
    let Some(ref body_store) = state.body_store else {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Body store not available",
        );
    };

    match body_store.write().clear() {
        Ok(removed_count) => {
            tracing::info!("Cleared {} body cache files", removed_count);
            let response = ClearCacheResponse {
                removed_files: removed_count,
                message: format!("Successfully cleared {} cache files", removed_count),
            };
            json_response(&response)
        }
        Err(e) => {
            tracing::error!("Failed to clear body cache: {}", e);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Failed to clear cache: {}", e),
            )
        }
    }
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

    let old_config = {
        let config = state.runtime_config.read().await;
        config.clone()
    };

    let mut affected_patterns: Vec<String> = Vec::new();
    let mut global_changed = false;

    {
        let mut runtime_config = state.runtime_config.write().await;

        if let Some(enable) = request.enable_tls_interception {
            if enable != runtime_config.enable_tls_interception {
                global_changed = true;
                tracing::info!(
                    "TLS config changed: enable_tls_interception: {} -> {}",
                    runtime_config.enable_tls_interception,
                    enable
                );
            }
            runtime_config.enable_tls_interception = enable;
        }

        if let Some(ref exclude) = request.intercept_exclude {
            let added: Vec<_> = exclude
                .iter()
                .filter(|p| !old_config.intercept_exclude.contains(p))
                .cloned()
                .collect();
            let removed: Vec<_> = old_config
                .intercept_exclude
                .iter()
                .filter(|p| !exclude.contains(p))
                .cloned()
                .collect();

            if !added.is_empty() || !removed.is_empty() {
                tracing::info!(
                    "TLS config changed: intercept_exclude added={:?}, removed={:?}",
                    added,
                    removed
                );
                affected_patterns.extend(added);
                affected_patterns.extend(removed);
            }
            runtime_config.intercept_exclude = exclude.clone();
        }

        if let Some(ref include) = request.intercept_include {
            let added: Vec<_> = include
                .iter()
                .filter(|p| !old_config.intercept_include.contains(p))
                .cloned()
                .collect();
            let removed: Vec<_> = old_config
                .intercept_include
                .iter()
                .filter(|p| !include.contains(p))
                .cloned()
                .collect();

            if !added.is_empty() || !removed.is_empty() {
                tracing::info!(
                    "TLS config changed: intercept_include added={:?}, removed={:?}",
                    added,
                    removed
                );
                affected_patterns.extend(added);
                affected_patterns.extend(removed);
            }
            runtime_config.intercept_include = include.clone();
        }

        if let Some(unsafe_ssl) = request.unsafe_ssl {
            runtime_config.unsafe_ssl = unsafe_ssl;
        }

        if let Some(disconnect_on_change) = request.disconnect_on_config_change {
            runtime_config.disconnect_on_config_change = disconnect_on_change;
            tracing::info!(
                "TLS config changed: disconnect_on_config_change = {}",
                disconnect_on_change
            );
        }
    }

    if let Some(ref config_manager) = state.config_manager {
        let update = TlsConfigUpdate {
            enable_interception: request.enable_tls_interception,
            intercept_exclude: request.intercept_exclude.clone(),
            intercept_include: request.intercept_include.clone(),
            unsafe_ssl: request.unsafe_ssl,
            disconnect_on_change: request.disconnect_on_config_change,
        };

        if let Err(e) = config_manager.update_tls_config(update).await {
            tracing::error!("Failed to persist TLS config: {}", e);
        } else {
            tracing::debug!("TLS config persisted to config.toml");
        }
    }

    let should_disconnect = state
        .runtime_config
        .read()
        .await
        .disconnect_on_config_change;

    if should_disconnect {
        if global_changed {
            let new_enabled = state.runtime_config.read().await.enable_tls_interception;
            let disconnected = state
                .connection_registry
                .disconnect_all_with_mode(!new_enabled);
            if !disconnected.is_empty() {
                tracing::info!(
                    "Global TLS switch change disconnected {} connections",
                    disconnected.len()
                );
            }
        } else if !affected_patterns.is_empty() {
            let disconnected = state
                .connection_registry
                .disconnect_by_host_pattern(&affected_patterns);
            if !disconnected.is_empty() {
                tracing::info!(
                    "TLS pattern change disconnected {} connections matching {:?}",
                    disconnected.len(),
                    affected_patterns
                );
            }
        }
    } else if global_changed || !affected_patterns.is_empty() {
        tracing::info!(
            "TLS config changed but disconnect_on_config_change is disabled, {} existing connections will continue with old config",
            state.connection_registry.active_count()
        );
    }

    if global_changed || !affected_patterns.is_empty() {
        let config = state.runtime_config.read().await;
        let status_info =
            TlsStatusInfo::from_runtime_config(&config, state.connection_registry.active_count());
        status_info.log_update_banner();
    }

    let runtime_config = state.runtime_config.read().await;
    let tls_config = TlsConfig {
        enable_tls_interception: runtime_config.enable_tls_interception,
        intercept_exclude: runtime_config.intercept_exclude.clone(),
        intercept_include: runtime_config.intercept_include.clone(),
        unsafe_ssl: runtime_config.unsafe_ssl,
        disconnect_on_config_change: runtime_config.disconnect_on_config_change,
    };

    json_response(&tls_config)
}

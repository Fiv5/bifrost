use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub struct ConfigApiClient {
    base_url: String,
}

impl ConfigApiClient {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            base_url: format!("http://{}:{}/_bifrost/api", host, port),
        }
    }

    pub fn get_tls_config(&self) -> Result<TlsConfigResponse, String> {
        self.get("/config/tls")
    }

    pub fn get_server_config(&self) -> Result<ServerConfigResponse, String> {
        self.get("/config/server")
    }

    pub fn update_server_config(
        &self,
        req: &UpdateServerConfigRequest,
    ) -> Result<ServerConfigResponse, String> {
        self.put("/config/server", req)
    }

    pub fn update_tls_config(
        &self,
        req: &UpdateTlsConfigRequest,
    ) -> Result<TlsConfigResponse, String> {
        self.put("/config/tls", req)
    }

    pub fn get_performance_config(&self) -> Result<PerformanceConfigResponse, String> {
        self.get("/config/performance")
    }

    pub fn update_performance_config(
        &self,
        req: &UpdatePerformanceConfigRequest,
    ) -> Result<PerformanceConfigResponse, String> {
        self.put("/config/performance", req)
    }

    pub fn clear_cache(&self) -> Result<ClearCacheResponse, String> {
        self.delete("/config/performance/clear-cache")
    }

    pub fn disconnect_by_domain(&self, domain: &str) -> Result<DisconnectResponse, String> {
        self.post(
            "/config/connections/disconnect",
            &DisconnectRequest {
                domain: domain.to_string(),
            },
        )
    }

    pub fn get_whitelist(&self) -> Result<WhitelistResponse, String> {
        self.get("/whitelist")
    }

    pub fn set_access_mode(&self, mode: &str) -> Result<serde_json::Value, String> {
        self.put(
            "/whitelist/mode",
            &AccessModeRequest {
                mode: mode.to_string(),
            },
        )
    }

    pub fn set_allow_lan(&self, allow: bool) -> Result<serde_json::Value, String> {
        self.put(
            "/whitelist/allow-lan",
            &AllowLanRequest { allow_lan: allow },
        )
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = bifrost_core::direct_ureq_agent().get(&url).call().map_err(|e| {
            format!(
                "Failed to connect to Bifrost admin API at {}\nIs the proxy server running?\n\nHint: Start the proxy with: bifrost start\n\nError: {}",
                url, e
            )
        })?;

        let body = resp
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn put<T: DeserializeOwned, R: Serialize>(&self, path: &str, body: &R) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = bifrost_core::direct_ureq_agent()
            .put(&url)
            .send_json(body)
            .map_err(|e| {
            format!(
                "Failed to connect to Bifrost admin API at {}\nIs the proxy server running?\n\nHint: Start the proxy with: bifrost start\n\nError: {}",
                url, e
            )
        })?;

        let body = resp
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn post<T: DeserializeOwned, R: Serialize>(&self, path: &str, body: &R) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = bifrost_core::direct_ureq_agent()
            .post(&url)
            .send_json(body)
            .map_err(|e| {
            format!(
                "Failed to connect to Bifrost admin API at {}\nIs the proxy server running?\n\nHint: Start the proxy with: bifrost start\n\nError: {}",
                url, e
            )
        })?;

        let body = resp
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))
    }

    fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = bifrost_core::direct_ureq_agent()
            .delete(&url)
            .call()
            .map_err(|e| {
            format!(
                "Failed to connect to Bifrost admin API at {}\nIs the proxy server running?\n\nHint: Start the proxy with: bifrost start\n\nError: {}",
                url, e
            )
        })?;

        let body = resp
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfigResponse {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub app_intercept_exclude: Vec<String>,
    pub app_intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
    pub disconnect_on_config_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfigResponse {
    pub timeout_secs: u64,
    pub http1_max_header_size: usize,
    pub http2_max_header_list_size: usize,
    pub websocket_handshake_max_header_size: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateServerConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http1_max_header_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http2_max_header_list_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub websocket_handshake_max_header_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdateTlsConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_tls_interception: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intercept_exclude: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intercept_include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_intercept_exclude: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_intercept_include: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsafe_ssl: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disconnect_on_config_change: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfigResponse {
    pub traffic: TrafficConfig,
    pub body_store_stats: Option<BodyStoreStats>,
    pub frame_store_stats: Option<FrameStoreStats>,
    pub ws_payload_store_stats: Option<WsPayloadStoreStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficConfig {
    pub max_records: usize,
    pub max_db_size_bytes: u64,
    pub max_body_memory_size: usize,
    pub max_body_buffer_size: usize,
    pub max_body_probe_size: usize,
    pub binary_traffic_performance_mode: bool,
    pub file_retention_days: u64,
    pub sse_stream_flush_bytes: usize,
    pub sse_stream_flush_interval_ms: u64,
    pub ws_payload_flush_bytes: usize,
    pub ws_payload_flush_interval_ms: u64,
    pub ws_payload_max_open_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyStoreStats {
    pub file_count: usize,
    pub total_size: u64,
    pub temp_dir: String,
    pub max_memory_size: usize,
    pub retention_days: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameStoreStats {
    pub connection_count: usize,
    pub total_size: u64,
    pub frames_dir: String,
    pub retention_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsPayloadStoreStats {
    pub file_count: usize,
    pub total_size: u64,
    pub payload_dir: String,
    pub retention_days: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct UpdatePerformanceConfigRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_records: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_db_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_memory_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_buffer_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_body_probe_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_traffic_performance_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_retention_days: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse_stream_flush_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sse_stream_flush_interval_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_payload_flush_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_payload_flush_interval_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_payload_max_open_files: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearCacheResponse {
    pub body_cache_removed: usize,
    pub traffic_cache_removed: usize,
    pub frame_cache_removed: usize,
    pub ws_payload_cache_removed: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisconnectRequest {
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisconnectResponse {
    pub success: bool,
    pub disconnected_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistResponse {
    pub mode: String,
    pub allow_lan: bool,
    pub whitelist: Vec<String>,
    pub temporary_whitelist: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccessModeRequest {
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AllowLanRequest {
    pub allow_lan: bool,
}

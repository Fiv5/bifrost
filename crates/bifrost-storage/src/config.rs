use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub(crate) struct LegacyAccessConfig {
    pub mode: String,
    pub whitelist: Vec<String>,
    pub allow_lan: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub(crate) struct LegacySystemProxyConfig {
    pub enabled: bool,
    pub bypass: String,
}

impl Default for LegacySystemProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bypass: "localhost,127.0.0.1,::1,*.local".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub(crate) struct LegacyTrafficConfig {
    pub max_records: usize,
    pub max_body_memory_size: usize,
    pub max_body_buffer_size: usize,
    pub temp_dir: PathBuf,
    pub file_retention_days: u64,
    pub sse_stream_flush_bytes: usize,
    pub sse_stream_flush_interval_ms: u64,
    pub ws_payload_flush_bytes: usize,
    pub ws_payload_flush_interval_ms: u64,
    pub ws_payload_max_open_files: usize,
}

impl Default for LegacyTrafficConfig {
    fn default() -> Self {
        Self {
            max_records: 5000,
            max_body_memory_size: 512 * 1024,
            max_body_buffer_size: 10 * 1024 * 1024,
            temp_dir: crate::data_dir().join("traffic"),
            file_retention_days: 7,
            sse_stream_flush_bytes: 256 * 1024,
            sse_stream_flush_interval_ms: 1000,
            ws_payload_flush_bytes: 512 * 1024,
            ws_payload_flush_interval_ms: 1000,
            ws_payload_max_open_files: 128,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub(crate) struct BifrostConfig {
    pub rules_dir: PathBuf,
    pub values_dir: PathBuf,
    pub cert_dir: PathBuf,
    pub access: LegacyAccessConfig,
    pub traffic: LegacyTrafficConfig,
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub system_proxy: LegacySystemProxyConfig,
    pub disconnect_on_config_change: bool,
}

impl Default for BifrostConfig {
    fn default() -> Self {
        let base = crate::data_dir();
        Self {
            rules_dir: base.join("rules"),
            values_dir: base.join("values"),
            cert_dir: base.join("certs"),
            access: LegacyAccessConfig::default(),
            traffic: LegacyTrafficConfig::default(),
            enable_tls_interception: true,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            system_proxy: LegacySystemProxyConfig::default(),
            disconnect_on_config_change: true,
        }
    }
}

use std::path::PathBuf;
use std::sync::Arc;

use bifrost_core::{ClientAccessControl, SystemProxyManager};
use bifrost_storage::{RulesStorage, ValuesStorage};
use parking_lot::RwLock as ParkingRwLock;
use tokio::sync::RwLock;

use crate::body_store::SharedBodyStore;
use crate::metrics::{MetricsCollector, SharedMetricsCollector};
use crate::traffic::{SharedTrafficRecorder, TrafficRecorder};
use crate::websocket_monitor::{SharedWebSocketMonitor, WebSocketMonitor};

pub type SharedAccessControl = Arc<RwLock<ClientAccessControl>>;
pub type SharedValuesStorage = Arc<ParkingRwLock<ValuesStorage>>;
pub type SharedSystemProxyManager = Arc<RwLock<SystemProxyManager>>;
pub type SharedRuntimeConfig = Arc<RwLock<RuntimeConfig>>;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enable_tls_interception: true,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            unsafe_ssl: false,
        }
    }
}

pub struct AdminState {
    pub traffic_recorder: SharedTrafficRecorder,
    pub metrics_collector: SharedMetricsCollector,
    pub rules_storage: RulesStorage,
    pub values_storage: Option<SharedValuesStorage>,
    pub access_control: Option<SharedAccessControl>,
    pub body_store: Option<SharedBodyStore>,
    pub start_time: u64,
    pub port: u16,
    pub ca_cert_path: Option<PathBuf>,
    pub system_proxy_manager: Option<SharedSystemProxyManager>,
    pub websocket_monitor: SharedWebSocketMonitor,
    pub runtime_config: SharedRuntimeConfig,
}

impl AdminState {
    pub fn new(port: u16) -> Self {
        Self {
            traffic_recorder: Arc::new(TrafficRecorder::default()),
            metrics_collector: Arc::new(MetricsCollector::default()),
            rules_storage: RulesStorage::default(),
            values_storage: None,
            access_control: None,
            body_store: None,
            start_time: chrono::Utc::now().timestamp() as u64,
            port,
            ca_cert_path: None,
            system_proxy_manager: None,
            websocket_monitor: Arc::new(WebSocketMonitor::new()),
            runtime_config: Arc::new(RwLock::new(RuntimeConfig::default())),
        }
    }

    pub fn with_rules_storage(mut self, storage: RulesStorage) -> Self {
        self.rules_storage = storage;
        self
    }

    pub fn with_values_storage(mut self, storage: ValuesStorage) -> Self {
        self.values_storage = Some(Arc::new(ParkingRwLock::new(storage)));
        self
    }

    pub fn with_traffic_recorder(mut self, recorder: TrafficRecorder) -> Self {
        self.traffic_recorder = Arc::new(recorder);
        self
    }

    pub fn with_metrics_collector(mut self, collector: MetricsCollector) -> Self {
        self.metrics_collector = Arc::new(collector);
        self
    }

    pub fn with_access_control(mut self, access_control: SharedAccessControl) -> Self {
        self.access_control = Some(access_control);
        self
    }

    pub fn with_body_store(mut self, body_store: SharedBodyStore) -> Self {
        self.body_store = Some(body_store);
        self
    }

    pub fn with_ca_cert_path(mut self, ca_cert_path: PathBuf) -> Self {
        self.ca_cert_path = Some(ca_cert_path);
        self
    }

    pub fn with_system_proxy_manager(mut self, manager: SystemProxyManager) -> Self {
        self.system_proxy_manager = Some(Arc::new(RwLock::new(manager)));
        self
    }

    pub fn with_system_proxy_manager_shared(mut self, manager: SharedSystemProxyManager) -> Self {
        self.system_proxy_manager = Some(manager);
        self
    }

    pub fn with_runtime_config(mut self, config: RuntimeConfig) -> Self {
        self.runtime_config = Arc::new(RwLock::new(config));
        self
    }

    pub fn with_runtime_config_shared(mut self, config: SharedRuntimeConfig) -> Self {
        self.runtime_config = config;
        self
    }
}

pub type SharedAdminState = Arc<AdminState>;

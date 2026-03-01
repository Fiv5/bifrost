use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bifrost_core::{ClientAccessControl, SystemProxyManager};
use bifrost_storage::{ConfigManager, RulesStorage, SharedConfigManager, ValuesStorage};
use parking_lot::RwLock as ParkingRwLock;
use tokio::sync::RwLock;

use crate::app_icon::SharedAppIconCache;
use crate::async_traffic::{AsyncTrafficWriter, SharedAsyncTrafficWriter};
use crate::body_store::SharedBodyStore;
use crate::connection_monitor::{ConnectionMonitor, SharedConnectionMonitor};
use crate::connection_registry::{ConnectionRegistry, SharedConnectionRegistry};
use crate::frame_store::{FrameStore, SharedFrameStore};
use crate::handlers::scripts::ScriptManager;
use crate::metrics::{MetricsCollector, SharedMetricsCollector};
use crate::replay_db::{ReplayDbStore, SharedReplayDbStore};
use crate::replay_executor::{ReplayExecutor, SharedReplayExecutor};
use crate::traffic::{SharedTrafficRecorder, TrafficRecorder};
use crate::traffic_db::{SharedTrafficDbStore, TrafficDbStore};
use crate::traffic_store::{SharedTrafficStore, TrafficStore};
use crate::version_check::{SharedVersionChecker, VersionChecker};

pub type SharedScriptManager = Arc<RwLock<ScriptManager>>;

pub type SharedAccessControl = Arc<RwLock<ClientAccessControl>>;
pub type SharedValuesStorage = Arc<ParkingRwLock<ValuesStorage>>;
pub type SharedSystemProxyManager = Arc<RwLock<SystemProxyManager>>;
pub type SharedRuntimeConfig = Arc<RwLock<RuntimeConfig>>;

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub app_intercept_exclude: Vec<String>,
    pub app_intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
    pub disconnect_on_config_change: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            enable_tls_interception: true,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            app_intercept_exclude: Vec::new(),
            app_intercept_include: Vec::new(),
            unsafe_ssl: false,
            disconnect_on_config_change: true,
        }
    }
}

impl RuntimeConfig {
    pub fn from_tls_config(tls: &bifrost_storage::TlsConfig) -> Self {
        Self {
            enable_tls_interception: tls.enable_interception,
            intercept_exclude: tls.intercept_exclude.clone(),
            intercept_include: tls.intercept_include.clone(),
            app_intercept_exclude: tls.app_intercept_exclude.clone(),
            app_intercept_include: tls.app_intercept_include.clone(),
            unsafe_ssl: tls.unsafe_ssl,
            disconnect_on_config_change: tls.disconnect_on_change,
        }
    }
}

pub struct AdminState {
    pub traffic_recorder: SharedTrafficRecorder,
    pub traffic_store: Option<SharedTrafficStore>,
    pub traffic_db_store: Option<SharedTrafficDbStore>,
    pub async_traffic_writer: Option<SharedAsyncTrafficWriter>,
    pub metrics_collector: SharedMetricsCollector,
    pub rules_storage: RulesStorage,
    pub values_storage: Option<SharedValuesStorage>,
    pub access_control: Option<SharedAccessControl>,
    pub body_store: Option<SharedBodyStore>,
    pub frame_store: Option<SharedFrameStore>,
    pub start_time: u64,
    pub port: u16,
    pub ca_cert_path: Option<PathBuf>,
    pub system_proxy_manager: Option<SharedSystemProxyManager>,
    pub connection_monitor: SharedConnectionMonitor,
    pub runtime_config: SharedRuntimeConfig,
    pub connection_registry: SharedConnectionRegistry,
    pub config_manager: Option<SharedConfigManager>,
    pub max_body_buffer_size: AtomicUsize,
    pub app_icon_cache: Option<SharedAppIconCache>,
    pub version_checker: SharedVersionChecker,
    pub script_manager: Option<SharedScriptManager>,
    pub replay_db_store: Option<SharedReplayDbStore>,
    pub replay_executor: Option<SharedReplayExecutor>,
}

const DEFAULT_MAX_BODY_BUFFER_SIZE: usize = 10 * 1024 * 1024;

impl AdminState {
    pub fn new(port: u16) -> Self {
        Self {
            traffic_recorder: Arc::new(TrafficRecorder::default()),
            traffic_store: None,
            traffic_db_store: None,
            async_traffic_writer: None,
            metrics_collector: Arc::new(MetricsCollector::default()),
            rules_storage: RulesStorage::default(),
            values_storage: None,
            access_control: None,
            body_store: None,
            frame_store: None,
            start_time: chrono::Utc::now().timestamp() as u64,
            port,
            ca_cert_path: None,
            system_proxy_manager: None,
            connection_monitor: Arc::new(ConnectionMonitor::new()),
            runtime_config: Arc::new(RwLock::new(RuntimeConfig::default())),
            connection_registry: Arc::new(ConnectionRegistry::default()),
            config_manager: None,
            max_body_buffer_size: AtomicUsize::new(DEFAULT_MAX_BODY_BUFFER_SIZE),
            app_icon_cache: None,
            version_checker: Arc::new(VersionChecker::new()),
            script_manager: None,
            replay_db_store: None,
            replay_executor: None,
        }
    }

    pub fn get_max_body_buffer_size(&self) -> usize {
        self.max_body_buffer_size.load(Ordering::Relaxed)
    }

    pub fn set_max_body_buffer_size(&self, size: usize) {
        let old = self.max_body_buffer_size.swap(size, Ordering::SeqCst);
        if old != size {
            tracing::info!(
                "AdminState config updated: max_body_buffer_size {} -> {}",
                old,
                size
            );
        }
    }

    #[inline]
    pub fn record_traffic(&self, record: crate::traffic::TrafficRecord) {
        if let Some(ref writer) = self.async_traffic_writer {
            writer.record(record);
        } else if let Some(ref db_store) = self.traffic_db_store {
            db_store.record(record);
        } else {
            if let Some(ref traffic_store) = self.traffic_store {
                traffic_store.record(record.clone());
            }
            self.traffic_recorder.record(record);
        }
    }

    #[inline]
    pub fn update_traffic_by_id<F>(&self, id: &str, updater: F)
    where
        F: Fn(&mut crate::traffic::TrafficRecord) + Send + Sync + Clone + 'static,
    {
        if let Some(ref writer) = self.async_traffic_writer {
            writer.update_by_id(id, updater);
        } else if let Some(ref db_store) = self.traffic_db_store {
            db_store.update_by_id(id, updater);
        } else {
            if let Some(ref traffic_store) = self.traffic_store {
                traffic_store.update_by_id(id, updater.clone());
            }
            self.traffic_recorder.update_by_id(id, updater);
        }
    }

    pub fn update_client_process(
        &self,
        id: &str,
        client_app: String,
        client_pid: u32,
        client_path: Option<String>,
    ) {
        self.update_traffic_by_id(id, move |record| {
            record.client_app = Some(client_app.clone());
            record.client_pid = Some(client_pid);
            record.client_path = client_path.clone();
        });
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

    pub fn with_traffic_recorder_shared(mut self, recorder: SharedTrafficRecorder) -> Self {
        self.traffic_recorder = recorder;
        self
    }

    pub fn with_traffic_store(mut self, store: TrafficStore) -> Self {
        let sequence = store.current_sequence();
        self.traffic_recorder.set_initial_sequence(sequence);
        self.traffic_store = Some(Arc::new(store));
        self
    }

    pub fn with_traffic_store_shared(mut self, store: SharedTrafficStore) -> Self {
        let sequence = store.current_sequence();
        self.traffic_recorder.set_initial_sequence(sequence);
        self.traffic_store = Some(store);
        self
    }

    pub fn with_traffic_db_store(mut self, store: TrafficDbStore) -> Self {
        self.traffic_db_store = Some(Arc::new(store));
        self
    }

    pub fn with_traffic_db_store_shared(mut self, store: SharedTrafficDbStore) -> Self {
        self.traffic_db_store = Some(store);
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

    pub fn with_frame_store(mut self, frame_store: FrameStore) -> Self {
        self.frame_store = Some(Arc::new(frame_store));
        self
    }

    pub fn with_frame_store_shared(mut self, frame_store: SharedFrameStore) -> Self {
        self.frame_store = Some(frame_store);
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

    pub fn with_connection_registry(mut self, registry: ConnectionRegistry) -> Self {
        self.connection_registry = Arc::new(registry);
        self
    }

    pub fn with_connection_registry_shared(mut self, registry: SharedConnectionRegistry) -> Self {
        self.connection_registry = registry;
        self
    }

    pub fn with_config_manager(mut self, manager: ConfigManager) -> Self {
        self.config_manager = Some(Arc::new(manager));
        self
    }

    pub fn with_config_manager_shared(mut self, manager: SharedConfigManager) -> Self {
        self.config_manager = Some(manager);
        self
    }

    pub fn with_max_body_buffer_size(self, size: usize) -> Self {
        self.max_body_buffer_size.store(size, Ordering::SeqCst);
        self
    }

    pub fn with_app_icon_cache(mut self, cache: SharedAppIconCache) -> Self {
        self.app_icon_cache = Some(cache);
        self
    }

    pub fn with_async_traffic_writer(mut self, writer: AsyncTrafficWriter) -> Self {
        self.async_traffic_writer = Some(Arc::new(writer));
        self
    }

    pub fn with_async_traffic_writer_shared(mut self, writer: SharedAsyncTrafficWriter) -> Self {
        self.async_traffic_writer = Some(writer);
        self
    }

    pub fn with_script_manager(mut self, manager: ScriptManager) -> Self {
        self.script_manager = Some(Arc::new(RwLock::new(manager)));
        self
    }

    pub fn with_script_manager_shared(mut self, manager: SharedScriptManager) -> Self {
        self.script_manager = Some(manager);
        self
    }

    pub fn with_replay_db_store(mut self, store: ReplayDbStore) -> Self {
        self.replay_db_store = Some(Arc::new(store));
        self
    }

    pub fn with_replay_db_store_shared(mut self, store: SharedReplayDbStore) -> Self {
        self.replay_db_store = Some(store);
        self
    }

    pub fn with_replay_db_store_shared_opt(mut self, store: Option<SharedReplayDbStore>) -> Self {
        self.replay_db_store = store;
        self
    }

    pub fn with_replay_executor(mut self, executor: ReplayExecutor) -> Self {
        self.replay_executor = Some(Arc::new(executor));
        self
    }

    pub fn with_replay_executor_shared(mut self, executor: SharedReplayExecutor) -> Self {
        self.replay_executor = Some(executor);
        self
    }
}

pub type SharedAdminState = Arc<AdminState>;

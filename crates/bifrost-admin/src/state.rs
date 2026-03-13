use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
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
use crate::port_rebind::SharedPortRebindManager;
use crate::replay_db::{ReplayDbStore, SharedReplayDbStore};
use crate::replay_executor::SharedReplayExecutor;
use crate::sse::SseHub;
use crate::traffic_db::{SharedTrafficDbStore, TrafficDbStore};
use crate::version_check::{SharedVersionChecker, VersionChecker};
use crate::ws_payload_store::{SharedWsPayloadStore, WsPayloadStore};
use once_cell::sync::OnceCell;

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
    pub traffic_db_store: Option<SharedTrafficDbStore>,
    pub async_traffic_writer: Option<SharedAsyncTrafficWriter>,
    pub metrics_collector: SharedMetricsCollector,
    pub rules_storage: RulesStorage,
    pub values_storage: Option<SharedValuesStorage>,
    pub access_control: Option<SharedAccessControl>,
    pub body_store: Option<SharedBodyStore>,
    pub frame_store: Option<SharedFrameStore>,
    pub ws_payload_store: Option<SharedWsPayloadStore>,
    pub start_time: u64,
    port: AtomicU16,
    pub ca_cert_path: Option<PathBuf>,
    pub system_proxy_manager: Option<SharedSystemProxyManager>,
    pub connection_monitor: SharedConnectionMonitor,
    pub sse_hub: Arc<SseHub>,
    pub runtime_config: SharedRuntimeConfig,
    pub connection_registry: SharedConnectionRegistry,
    pub config_manager: Option<SharedConfigManager>,
    pub max_body_buffer_size: AtomicUsize,
    pub max_body_probe_size: AtomicUsize,
    pub app_icon_cache: Option<SharedAppIconCache>,
    pub version_checker: SharedVersionChecker,
    pub script_manager: Option<SharedScriptManager>,
    pub replay_db_store: Option<SharedReplayDbStore>,
    pub replay_executor: OnceCell<SharedReplayExecutor>,
    pub total_size_cleanup_counter: AtomicUsize,
    pub port_rebind_manager: Option<SharedPortRebindManager>,
}

const DEFAULT_MAX_BODY_BUFFER_SIZE: usize = 10 * 1024 * 1024;
const DEFAULT_MAX_BODY_PROBE_SIZE: usize = 64 * 1024;

impl AdminState {
    pub fn new(port: u16) -> Self {
        Self {
            traffic_db_store: None,
            async_traffic_writer: None,
            metrics_collector: Arc::new(MetricsCollector::default()),
            rules_storage: RulesStorage::default(),
            values_storage: None,
            access_control: None,
            body_store: None,
            frame_store: None,
            ws_payload_store: None,
            start_time: chrono::Utc::now().timestamp() as u64,
            port: AtomicU16::new(port),
            ca_cert_path: None,
            system_proxy_manager: None,
            connection_monitor: Arc::new(ConnectionMonitor::new()),
            sse_hub: SseHub::new(),
            runtime_config: Arc::new(RwLock::new(RuntimeConfig::default())),
            connection_registry: Arc::new(ConnectionRegistry::default()),
            config_manager: None,
            max_body_buffer_size: AtomicUsize::new(DEFAULT_MAX_BODY_BUFFER_SIZE),
            max_body_probe_size: AtomicUsize::new(DEFAULT_MAX_BODY_PROBE_SIZE),
            app_icon_cache: None,
            version_checker: Arc::new(VersionChecker::new()),
            script_manager: None,
            replay_db_store: None,
            replay_executor: OnceCell::new(),
            total_size_cleanup_counter: AtomicUsize::new(0),
            port_rebind_manager: None,
        }
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::Relaxed)
    }

    pub fn set_port(&self, port: u16) {
        let old = self.port.swap(port, Ordering::SeqCst);
        if old != port {
            tracing::info!("AdminState port updated: {} -> {}", old, port);
        }
    }

    pub fn get_max_body_buffer_size(&self) -> usize {
        self.max_body_buffer_size.load(Ordering::Relaxed)
    }

    pub fn get_max_body_probe_size(&self) -> usize {
        self.max_body_probe_size.load(Ordering::Relaxed)
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

    pub fn set_max_body_probe_size(&self, size: usize) {
        let old = self.max_body_probe_size.swap(size, Ordering::SeqCst);
        if old != size {
            tracing::info!(
                "AdminState config updated: max_body_probe_size {} -> {}",
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
            tracing::error!("[ADMIN_STATE] No traffic_db_store configured; drop record");
        }
        self.maybe_cleanup_total_disk_usage();
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
            tracing::error!("[ADMIN_STATE] No traffic_db_store configured; drop update");
        }
    }

    fn maybe_cleanup_total_disk_usage(&self) {
        const CLEANUP_CHECK_INTERVAL: usize = 100;
        let counter = self
            .total_size_cleanup_counter
            .fetch_add(1, Ordering::Relaxed);
        if !counter.is_multiple_of(CLEANUP_CHECK_INTERVAL) {
            return;
        }
        self.cleanup_total_disk_usage();
    }

    fn cleanup_total_disk_usage(&self) {
        let Some(ref traffic_db_store) = self.traffic_db_store else {
            return;
        };
        let max_db_size_bytes = traffic_db_store.max_db_size_bytes();
        if max_db_size_bytes == 0 {
            return;
        }

        let db_stats = traffic_db_store.stats();
        let mut total_size = db_stats.db_size;

        let body_sizes = self
            .body_store
            .as_ref()
            .and_then(|store| store.read().sizes_by_id().ok())
            .unwrap_or_default();
        total_size += body_sizes.values().sum::<u64>();

        let frame_sizes = self
            .frame_store
            .as_ref()
            .and_then(|store| store.sizes_by_id().ok())
            .unwrap_or_default();
        total_size += frame_sizes.values().sum::<u64>();

        let ws_payload_sizes = self
            .ws_payload_store
            .as_ref()
            .and_then(|store| store.sizes_by_safe_id().ok())
            .unwrap_or_default();
        total_size += ws_payload_sizes.values().sum::<u64>();

        if total_size <= max_db_size_bytes {
            return;
        }

        let target_size = max_db_size_bytes.saturating_sub(max_db_size_bytes / 4);
        let bytes_to_remove = total_size.saturating_sub(target_size);
        if bytes_to_remove == 0 {
            return;
        }

        let record_count = db_stats.record_count;
        if record_count == 0 {
            return;
        }
        let avg_db_bytes = (db_stats.db_size / record_count as u64).max(1);

        let mut ids_to_delete = Vec::new();
        let mut removed_estimate = 0u64;
        let mut offset = 0usize;
        let batch = 500usize;

        while removed_estimate < bytes_to_remove {
            let ids = traffic_db_store.oldest_ids(batch, offset);
            if ids.is_empty() {
                break;
            }
            for id in ids {
                let mut size = avg_db_bytes;
                if let Some(extra) = body_sizes.get(&id) {
                    size += *extra;
                }
                if let Some(extra) = frame_sizes.get(&id) {
                    size += *extra;
                }
                let safe_id = WsPayloadStore::safe_connection_id(&id);
                if let Some(extra) = ws_payload_sizes.get(&safe_id) {
                    size += *extra;
                }
                removed_estimate += size;
                ids_to_delete.push(id);
                if removed_estimate >= bytes_to_remove {
                    break;
                }
            }
            offset += batch;
            if offset >= record_count {
                break;
            }
        }

        if ids_to_delete.is_empty() {
            return;
        }

        traffic_db_store.delete_by_ids(&ids_to_delete);
        if let Some(ref body_store) = self.body_store {
            let _ = body_store.write().delete_by_ids(&ids_to_delete);
        }
        if let Some(ref frame_store) = self.frame_store {
            let _ = frame_store.delete_by_ids(&ids_to_delete);
        }
        if let Some(ref ws_payload_store) = self.ws_payload_store {
            let _ = ws_payload_store.delete_by_ids(&ids_to_delete);
        }
        // Disk-size fallback cleanup is still on the hot path; avoid full VACUUM here.
        traffic_db_store.compact_db(false);

        tracing::info!(
            deleted = ids_to_delete.len(),
            total_size = total_size,
            max_db_size_bytes = max_db_size_bytes,
            target_size = target_size,
            "[TRAFFIC] Cleaned up data due to total size limit"
        );
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

    pub fn with_ws_payload_store(mut self, store: SharedWsPayloadStore) -> Self {
        self.ws_payload_store = Some(store);
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

    pub fn with_max_body_probe_size(self, size: usize) -> Self {
        self.max_body_probe_size.store(size, Ordering::SeqCst);
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

    pub fn with_port_rebind_manager_shared(mut self, manager: SharedPortRebindManager) -> Self {
        self.port_rebind_manager = Some(manager);
        self
    }

    pub fn set_replay_executor(&self, executor: SharedReplayExecutor) {
        let _ = self.replay_executor.set(executor);
    }

    pub fn get_replay_executor(&self) -> Option<&SharedReplayExecutor> {
        self.replay_executor.get()
    }
}

pub type SharedAdminState = Arc<AdminState>;

mod app_icon;
mod async_traffic;
mod body_store;
pub mod connection_monitor;
pub mod connection_registry;
mod frame_store;
mod handlers;
mod metrics;
pub mod push;
mod router;
pub mod search;
mod security;
mod state;
mod static_files;
pub mod status_printer;
mod traffic;
pub mod traffic_db;
mod traffic_store;

#[cfg(test)]
mod tests;

pub use app_icon::{create_app_icon_cache, AppIconCache, SharedAppIconCache};
pub use async_traffic::{
    start_async_traffic_processor, AsyncTrafficWriter, SharedAsyncTrafficWriter, TrafficCommand,
};
pub use body_store::{BodyRef, BodyStore, SharedBodyStore};
pub use connection_monitor::{ConnectionMonitor, SharedConnectionMonitor, WebSocketFrameRecord};
pub use connection_registry::{
    ConfigChangeEvent, ConnectionInfo, ConnectionRegistry, SharedConnectionRegistry,
};
pub use frame_store::{FrameStore, FrameStoreStats, SharedFrameStore};
pub use metrics::{
    start_metrics_collector_task, MetricsCollector, MetricsSnapshot, TrafficType,
    TrafficTypeMetrics,
};
pub use push::{start_push_tasks, PushManager, SharedPushManager};
pub use router::AdminRouter;
pub use security::{is_cert_public_request, is_valid_admin_request, AdminSecurityConfig};
pub use state::{
    AdminState, RuntimeConfig, SharedAccessControl, SharedRuntimeConfig, SharedSystemProxyManager,
    SharedValuesStorage,
};
pub use traffic::{
    FrameDirection, FrameType, MatchedRule, RequestTiming, SharedTrafficRecorder, SocketStatus,
    TrafficRecord, TrafficRecorder,
};
pub use traffic_db::{
    start_db_cleanup_task, Direction, QueryParams, QueryResult, SharedTrafficDbStore,
    TrafficDbStats, TrafficDbStore, TrafficFlags, TrafficSummaryCompact,
};
pub use traffic_store::{
    start_traffic_cleanup_task, SharedTrafficStore, TrafficStore, TrafficStoreStats,
};

pub use search::{
    FilterCondition, MatchLocation, SearchEngine, SearchFilters, SearchRequest, SearchResponse,
    SearchResultItem, SearchScope,
};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";
pub const CERT_PUBLIC_PATH_PREFIX: &str = "/_bifrost/public/cert";

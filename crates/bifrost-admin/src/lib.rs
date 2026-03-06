mod app_icon;
mod async_traffic;
mod body_store;
pub mod connection_monitor;
pub mod connection_registry;
mod frame_store;
mod handlers;
mod metrics;
pub mod push;
pub mod replay_db;
pub mod replay_executor;
pub mod request_rules;
mod router;
pub mod search;
mod security;
mod sse;
mod state;
mod static_files;
pub mod status_printer;
mod traffic;
pub mod traffic_db;
mod traffic_store;
mod version_check;

#[cfg(test)]
mod tests;

pub use app_icon::{create_app_icon_cache, AppIconCache, SharedAppIconCache};
pub use async_traffic::{
    start_async_traffic_processor, AsyncTrafficWriter, SharedAsyncTrafficWriter, TrafficCommand,
};
pub use body_store::{
    start_body_cleanup_task, BodyRef, BodyStore, BodyStreamWriter, SharedBodyStore,
};
pub use connection_monitor::{
    start_connection_cleanup_task, ConnectionMonitor, SharedConnectionMonitor, WebSocketFrameRecord,
};
pub use connection_registry::{
    ConfigChangeEvent, ConnectionInfo, ConnectionRegistry, SharedConnectionRegistry,
};
pub use frame_store::{FrameStore, FrameStoreStats, SharedFrameStore};
pub use handlers::scripts::ScriptManager;
pub use metrics::{
    start_metrics_collector_task, MetricsCollector, MetricsSnapshot, TrafficType,
    TrafficTypeMetrics,
};
pub use push::{start_push_tasks, PushManager, SharedPushManager};
pub use router::AdminRouter;
pub use security::{is_cert_public_request, is_valid_admin_request, AdminSecurityConfig};
pub use sse::{parse_sse_event, parse_sse_events_from_text, SseEvent, SseEventEnvelope, SseHub};
pub use state::{
    AdminState, RuntimeConfig, SharedAccessControl, SharedRuntimeConfig, SharedScriptManager,
    SharedSystemProxyManager, SharedValuesStorage,
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

pub use replay_db::{
    ReplayDbStore, ReplayGroup, ReplayHistory, ReplayRequest, ReplayRequestSummary, RuleConfig,
    RuleMode, SharedReplayDbStore, MAX_CONCURRENT_REPLAYS, MAX_HISTORY, MAX_REQUESTS,
};
pub use replay_executor::{
    ReplayError, ReplayExecuteRequest, ReplayExecuteResponse, ReplayExecutor, ReplayRequestData,
    SharedReplayExecutor,
};
pub use request_rules::{
    apply_all_request_rules, apply_host_rule, apply_url_rules, build_applied_rules, AppliedRequest,
    AppliedRules,
};
pub use search::{
    FilterCondition, MatchLocation, SearchEngine, SearchFilters, SearchRequest, SearchResponse,
    SearchResultItem, SearchScope,
};
pub use version_check::{SharedVersionChecker, VersionCheckResponse, VersionChecker};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";
pub const CERT_PUBLIC_PATH_PREFIX: &str = "/_bifrost/public/cert";

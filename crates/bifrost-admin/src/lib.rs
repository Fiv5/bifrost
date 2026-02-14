mod body_store;
pub mod connection_registry;
mod handlers;
mod metrics;
mod router;
mod security;
mod state;
mod static_files;
pub mod status_printer;
mod traffic;
pub mod websocket_monitor;

pub use body_store::{BodyRef, BodyStore, SharedBodyStore};
pub use connection_registry::{
    ConfigChangeEvent, ConnectionInfo, ConnectionRegistry, SharedConnectionRegistry,
};
pub use metrics::{
    start_metrics_collector_task, MetricsCollector, MetricsSnapshot, TrafficType,
    TrafficTypeMetrics,
};
pub use router::AdminRouter;
pub use security::{is_cert_public_request, is_valid_admin_request, AdminSecurityConfig};
pub use state::{
    AdminState, RuntimeConfig, SharedAccessControl, SharedRuntimeConfig, SharedSystemProxyManager,
};
pub use traffic::{
    FrameDirection, FrameType, MatchedRule, RequestTiming, SocketStatus, TrafficRecord,
    TrafficRecorder,
};
pub use websocket_monitor::{SharedWebSocketMonitor, WebSocketFrameRecord, WebSocketMonitor};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";
pub const CERT_PUBLIC_PATH_PREFIX: &str = "/_bifrost/public/cert";

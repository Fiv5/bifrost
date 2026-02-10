mod body_store;
mod handlers;
mod metrics;
mod router;
mod security;
mod state;
mod static_files;
mod traffic;

pub use body_store::{BodyRef, BodyStore, SharedBodyStore};
pub use metrics::{start_metrics_collector_task, MetricsCollector, MetricsSnapshot};
pub use router::AdminRouter;
pub use security::{is_cert_public_request, is_valid_admin_request, AdminSecurityConfig};
pub use state::{AdminState, SharedAccessControl, SharedSystemProxyManager};
pub use traffic::{MatchedRule, RequestTiming, TrafficRecord, TrafficRecorder};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";
pub const CERT_PUBLIC_PATH_PREFIX: &str = "/_bifrost/public/cert";

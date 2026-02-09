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
pub use security::{is_valid_admin_request, AdminSecurityConfig};
pub use state::{AdminState, SharedAccessControl};
pub use traffic::{MatchedRule, RequestTiming, TrafficRecord, TrafficRecorder};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";

mod handlers;
mod metrics;
mod router;
mod security;
mod state;
mod static_files;
mod traffic;

pub use metrics::{MetricsCollector, MetricsSnapshot};
pub use router::AdminRouter;
pub use security::{is_valid_admin_request, AdminSecurityConfig};
pub use state::{AdminState, SharedAccessControl};
pub use traffic::{TrafficRecord, TrafficRecorder};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";

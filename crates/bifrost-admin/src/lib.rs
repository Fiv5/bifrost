mod handlers;
mod router;
mod security;
mod state;
mod static_files;
mod traffic;
mod metrics;

pub use router::AdminRouter;
pub use security::{is_valid_admin_request, AdminSecurityConfig};
pub use state::AdminState;
pub use traffic::{TrafficRecord, TrafficRecorder};
pub use metrics::{MetricsCollector, MetricsSnapshot};

pub const ADMIN_PATH_PREFIX: &str = "/_bifrost";

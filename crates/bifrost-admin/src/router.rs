use hyper::{body::Incoming, Method, Request, Response, StatusCode};

use crate::handlers::{
    cors_preflight, error_response, metrics::handle_metrics, rules::handle_rules,
    system::handle_system, traffic::handle_traffic, BoxBody,
};
use crate::state::SharedAdminState;
use crate::static_files::serve_static_file;
use crate::ADMIN_PATH_PREFIX;

pub struct AdminRouter;

impl AdminRouter {
    pub async fn handle(req: Request<Incoming>, state: SharedAdminState) -> Response<BoxBody> {
        let path = req.uri().path().to_string();

        let admin_path = match path.strip_prefix(ADMIN_PATH_PREFIX) {
            Some(p) => p.to_string(),
            None => return error_response(StatusCode::NOT_FOUND, "Not Found"),
        };

        if req.method() == Method::OPTIONS {
            return cors_preflight();
        }

        if admin_path.starts_with("/api/") {
            Self::handle_api(req, state, &admin_path).await
        } else {
            serve_static_file(&admin_path)
        }
    }

    async fn handle_api(
        req: Request<Incoming>,
        state: SharedAdminState,
        path: &str,
    ) -> Response<BoxBody> {
        if path.starts_with("/api/rules") {
            handle_rules(req, state, path).await
        } else if path.starts_with("/api/traffic") {
            handle_traffic(req, state, path).await
        } else if path.starts_with("/api/metrics") {
            handle_metrics(req, state, path).await
        } else if path.starts_with("/api/system") {
            handle_system(req, state, path).await
        } else {
            error_response(StatusCode::NOT_FOUND, "API endpoint not found")
        }
    }
}

use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use tracing::debug;

use crate::handlers::{
    app_icon::handle_app_icon,
    cert::{handle_cert, handle_cert_public},
    config::handle_config,
    cors_preflight, error_response, frames,
    metrics::handle_metrics,
    proxy::handle_proxy,
    rules::handle_rules,
    system::handle_system,
    traffic::handle_traffic,
    values::handle_values,
    whitelist::handle_whitelist_request,
    BoxBody,
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

        if admin_path.starts_with("/public/cert") {
            return handle_cert_public(req, state, &admin_path).await;
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
        } else if path.starts_with("/api/values") {
            let path_suffix = path.strip_prefix("/api/values").unwrap_or("");
            handle_values(req, state, path_suffix).await
        } else if path.starts_with("/api/whitelist") {
            if let Some(access_control) = &state.access_control {
                handle_whitelist_request(req, access_control.clone(), path).await
            } else {
                error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Access control not configured",
                )
            }
        } else if path.starts_with("/api/cert") {
            handle_cert(req, state, path).await
        } else if path.starts_with("/api/proxy") {
            handle_proxy(req, state, path).await
        } else if path.starts_with("/api/config") {
            handle_config(req, state, path).await
        } else if path.starts_with("/api/websocket/connections") {
            frames::list_websocket_connections(state).await
        } else if path.starts_with("/api/app-icon/") {
            debug!(path = %path, "Routing to app_icon handler");
            handle_app_icon(req, state, path).await
        } else {
            error_response(StatusCode::NOT_FOUND, "API endpoint not found")
        }
    }
}

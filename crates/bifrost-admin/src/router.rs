use hyper::{body::Incoming, Method, Request, Response, StatusCode};
use tracing::debug;

use crate::handlers::{
    app_icon::handle_app_icon,
    audit::handle_audit,
    auth::{extract_bearer_token, handle_auth},
    bifrost_file::handle_bifrost_file,
    cert::{handle_cert, handle_cert_public, handle_proxy_public},
    config::handle_config,
    cors_preflight,
    env::handle_env,
    error_response, frames,
    group::handle_group,
    group_rules::handle_group_rules,
    metrics::handle_metrics,
    proxy::handle_proxy,
    replay::handle_replay,
    room::handle_room,
    rules::handle_rules,
    scripts::handle_scripts_request,
    search::handle_search,
    sync::{handle_sync, handle_sync_public},
    syntax::handle_syntax,
    system::handle_system,
    traffic::handle_traffic,
    user::handle_user,
    values::handle_values,
    websocket::handle_websocket_upgrade,
    whitelist::handle_whitelist_request,
    BoxBody,
};
use crate::push::SharedPushManager;
use crate::state::SharedAdminState;
use crate::static_files::serve_static_file;
use crate::{is_remote_access_enabled, validate_admin_jwt, ADMIN_PATH_PREFIX};

pub struct AdminRouter;

impl AdminRouter {
    pub async fn handle(
        req: Request<Incoming>,
        state: SharedAdminState,
        push_manager: Option<SharedPushManager>,
    ) -> Response<BoxBody> {
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

        if admin_path.starts_with("/public/proxy") {
            return handle_proxy_public(req, state, &admin_path).await;
        }

        if admin_path.starts_with("/public/sync-login") {
            return handle_sync_public(req, state, &admin_path).await;
        }

        if admin_path.starts_with("/api/") {
            Self::handle_api(req, state, push_manager, &admin_path).await
        } else {
            serve_static_file(&admin_path)
        }
    }

    async fn handle_api(
        req: Request<Incoming>,
        state: SharedAdminState,
        push_manager: Option<SharedPushManager>,
        path: &str,
    ) -> Response<BoxBody> {
        // 远程访问开启时：除登录/状态接口外，所有管理端 API 强制 JWT 鉴权。
        if is_remote_access_enabled(&state) && !path.starts_with("/api/auth/") {
            let token = extract_bearer_token(&req);
            let Some(token) = token else {
                return error_response(StatusCode::UNAUTHORIZED, "Missing bearer token");
            };
            if let Err(e) = validate_admin_jwt(&state, &token) {
                return error_response(StatusCode::UNAUTHORIZED, &format!("Unauthorized: {e}"));
            }
        }

        if path.starts_with("/api/auth") {
            return handle_auth(req, state, path).await;
        }

        if path.starts_with("/api/admin/audit") {
            return handle_audit(req, path).await;
        }

        if path.starts_with("/api/rules") {
            handle_rules(req, state, push_manager.clone(), path).await
        } else if path.starts_with("/api/traffic") {
            handle_traffic(req, state, push_manager.clone(), path).await
        } else if path.starts_with("/api/metrics") {
            handle_metrics(req, state, path).await
        } else if path.starts_with("/api/system") {
            handle_system(req, state, path).await
        } else if path.starts_with("/api/values") {
            let path_suffix = path.strip_prefix("/api/values").unwrap_or("");
            handle_values(req, state, path_suffix).await
        } else if path.starts_with("/api/whitelist") {
            if let Some(access_control) = &state.access_control {
                handle_whitelist_request(
                    req,
                    access_control.clone(),
                    state.config_manager.clone(),
                    push_manager.clone(),
                    path,
                )
                .await
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
            handle_config(req, state, push_manager, path).await
        } else if path.starts_with("/api/websocket/connections") {
            frames::list_websocket_connections(state).await
        } else if path.starts_with("/api/push") {
            if let Some(pm) = push_manager {
                handle_websocket_upgrade(req, pm).await
            } else {
                error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Push manager not configured",
                )
            }
        } else if path.starts_with("/api/app-icon/") {
            debug!(path = %path, "Routing to app_icon handler");
            handle_app_icon(req, state, path).await
        } else if path.starts_with("/api/search") {
            handle_search(req, state, path).await
        } else if path.starts_with("/api/sync") {
            handle_sync(req, state, path).await
        } else if path.starts_with("/api/group-rules") {
            handle_group_rules(req, state, path).await
        } else if path.starts_with("/api/group") {
            handle_group(req, state, path).await
        } else if path.starts_with("/api/env") {
            handle_env(req, state, path).await
        } else if path.starts_with("/api/room") {
            handle_room(req, state, path).await
        } else if path.starts_with("/api/user") {
            handle_user(req, state, path).await
        } else if path.starts_with("/api/scripts") {
            if let Some(script_manager) = &state.script_manager {
                handle_scripts_request(
                    req,
                    script_manager.clone(),
                    state.config_manager.clone(),
                    path,
                )
                .await
            } else {
                error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Script manager not configured",
                )
            }
        } else if path.starts_with("/api/replay") {
            handle_replay(req, state, push_manager, path).await
        } else if path.starts_with("/api/syntax") {
            handle_syntax(req, state, path).await
        } else if path.starts_with("/api/bifrost-file") {
            let path_suffix = path.strip_prefix("/api/bifrost-file").unwrap_or("");
            handle_bifrost_file(req, path_suffix, state.clone()).await
        } else {
            error_response(StatusCode::NOT_FOUND, "API endpoint not found")
        }
    }
}

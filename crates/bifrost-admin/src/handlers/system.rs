use hyper::{body::Incoming, Method, Request, Response, StatusCode};

use super::{error_response, json_response, method_not_allowed, BoxBody};
use crate::metrics::SystemInfo;
use crate::state::SharedAdminState;

pub async fn handle_system(
    req: Request<Incoming>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    let method = req.method().clone();
    let query = req.uri().query();

    match path {
        "/api/system" | "/api/system/" => match method {
            Method::GET => get_system_info(state).await,
            _ => method_not_allowed(),
        },
        "/api/system/overview" => match method {
            Method::GET => get_overview(state).await,
            _ => method_not_allowed(),
        },
        "/api/system/version-check" => match method {
            Method::GET => check_version(state, query).await,
            _ => method_not_allowed(),
        },
        _ => error_response(StatusCode::NOT_FOUND, "Not Found"),
    }
}

async fn get_system_info(state: SharedAdminState) -> Response<BoxBody> {
    let info = SystemInfo::new(state.start_time);
    json_response(&info)
}

async fn get_overview(state: SharedAdminState) -> Response<BoxBody> {
    let system_info = SystemInfo::new(state.start_time);
    let metrics = state.metrics_collector.get_current();
    let traffic_count = if let Some(ref db_store) = state.traffic_db_store {
        db_store.stats().record_count
    } else if let Some(ref traffic_store) = state.traffic_store {
        traffic_store.total()
    } else {
        0
    };

    let (rules_total, rules_enabled) = match state.rules_storage.load_all() {
        Ok(rules) => {
            let enabled = rules.iter().filter(|r| r.enabled).count();
            (rules.len(), enabled)
        }
        Err(_) => (0, 0),
    };

    let pending_count = if let Some(ref access_control) = state.access_control {
        let ac = access_control.read().await;
        ac.pending_authorization_count()
    } else {
        0
    };

    let overview = serde_json::json!({
        "system": system_info,
        "metrics": metrics,
        "rules": {
            "total": rules_total,
            "enabled": rules_enabled
        },
        "traffic": {
            "recorded": traffic_count
        },
        "server": {
            "port": state.port,
            "admin_url": format!("http://127.0.0.1:{}/_bifrost/", state.port)
        },
        "pending_authorizations": pending_count
    });

    json_response(&overview)
}

async fn check_version(state: SharedAdminState, query: Option<&str>) -> Response<BoxBody> {
    let force_refresh = query
        .map(|q| q.contains("refresh=true") || q.contains("refresh=1"))
        .unwrap_or(false);

    let response = state.version_checker.check(force_refresh).await;
    json_response(&response)
}

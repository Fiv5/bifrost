use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, header, Method, Request, Response, StatusCode};
use tracing::{debug, warn};

use super::{error_response, BoxBody};
use crate::state::SharedAdminState;

pub async fn handle_app_icon<B>(
    req: Request<B>,
    state: SharedAdminState,
    path: &str,
) -> Response<BoxBody> {
    if req.method() != Method::GET {
        return error_response(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed");
    }

    let app_icon_cache = match &state.app_icon_cache {
        Some(cache) => cache,
        None => {
            warn!("App icon cache not initialized");
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "App icon cache not initialized",
            );
        }
    };

    let app_name = path.strip_prefix("/api/app-icon/").unwrap_or("").trim();

    if app_name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "App name is required");
    }

    let app_name = urlencoding::decode(app_name)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| app_name.to_string());

    let app_path = get_app_path_from_traffic(&state, &app_name);

    debug!(
        app_name = %app_name,
        app_path = ?app_path,
        "Fetching app icon"
    );

    match app_icon_cache.get_icon(&app_name, app_path.as_deref()) {
        Some(icon_data) => {
            let body = Full::new(Bytes::from(icon_data)).map_err(|e| match e {});
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/png")
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(BoxBody::new(body))
                .unwrap()
        }
        None => error_response(StatusCode::NOT_FOUND, "Icon not found"),
    }
}

fn get_app_path_from_traffic(state: &SharedAdminState, app_name: &str) -> Option<String> {
    if let Some(ref traffic_store) = state.traffic_store {
        let records = traffic_store.get_all();
        for record in records.iter().rev() {
            if let Some(ref client_app) = record.client_app {
                if client_app == app_name {
                    return record.client_path.clone();
                }
            }
        }
    }

    None
}

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
                    if let Some(ref path) = record.client_path {
                        return Some(path.clone());
                    }
                }
            }
        }
    }

    search_app_bundle_by_name(app_name)
}

fn search_app_bundle_by_name(app_name: &str) -> Option<String> {
    let normalized_variants = normalize_app_name(app_name);

    debug!(
        app_name = %app_name,
        variants = ?normalized_variants,
        "Searching for app bundle"
    );

    let search_dirs = get_app_search_dirs();

    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "app").unwrap_or(false) {
                    let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

                    if matches_app_name(file_name, &normalized_variants, app_name) {
                        debug!(
                            app_name = %app_name,
                            found_path = %path.display(),
                            "Found app bundle by name search"
                        );
                        return Some(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    None
}

fn normalize_app_name(name: &str) -> Vec<String> {
    let base = name
        .replace(" Helper (Renderer)", "")
        .replace(" Helper (GPU)", "")
        .replace(" Helper (Plugin)", "")
        .replace(" Helper EH", "")
        .replace(" Helper NP", "")
        .replace(" Helper", "")
        .trim()
        .to_string();

    let mut variants = vec![base.clone()];

    let without_browser = base
        .replace(" Browser", "")
        .replace(" browser", "")
        .trim()
        .to_string();
    if without_browser != base && !without_browser.is_empty() {
        variants.push(without_browser);
    }

    let words: Vec<&str> = base.split_whitespace().collect();
    if words.len() > 1 {
        variants.push(words[0].to_string());
    }

    variants
}

fn matches_app_name(
    bundle_name: &str,
    normalized_variants: &[String],
    original_name: &str,
) -> bool {
    let bundle_lower = bundle_name.to_lowercase();
    let original_lower = original_name.to_lowercase();

    if bundle_lower == original_lower {
        return true;
    }

    for variant in normalized_variants {
        let variant_lower = variant.to_lowercase();

        if bundle_lower == variant_lower {
            return true;
        }

        if variant_lower.starts_with(&bundle_lower) || bundle_lower.starts_with(&variant_lower) {
            return true;
        }
    }

    let bundle_words: Vec<&str> = bundle_lower.split_whitespace().collect();
    for variant in normalized_variants {
        let variant_lower = variant.to_lowercase();
        let variant_words: Vec<&str> = variant_lower.split_whitespace().collect();
        if !bundle_words.is_empty()
            && !variant_words.is_empty()
            && bundle_words[0] == variant_words[0]
        {
            return true;
        }
    }

    false
}

fn get_app_search_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;

    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
    ];

    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join("Applications"));
    }

    dirs
}

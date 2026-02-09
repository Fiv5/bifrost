use bytes::Bytes;
use hyper::{header, Response, StatusCode};
use std::path::Path;
use tracing::{debug, warn};

use crate::logging::RequestContext;
use crate::server::{full_body, BoxBody, ResolvedRules};
use crate::url::build_redirect_uri;

pub async fn generate_mock_response(
    rules: &ResolvedRules,
    request_uri: &hyper::Uri,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Option<Response<BoxBody>> {
    if rules.ignored {
        return None;
    }

    if let Some(status) = rules.status_code {
        if verbose_logging {
            debug!("[{}] [MOCK] status code: {}", ctx.id_str(), status);
        }
        return Some(build_status_response(status, rules));
    }

    if let Some(redirect_target) = &rules.redirect {
        if let Some(location) = build_redirect_uri(request_uri, redirect_target) {
            if verbose_logging {
                debug!("[{}] [REDIRECT] 302 -> {}", ctx.id_str(), location);
            }
            return Some(build_redirect_response(302, &location));
        }
    }

    if let Some(location) = &rules.location_href {
        if verbose_logging {
            debug!("[{}] [LOCATION_HREF] -> {}", ctx.id_str(), location);
        }
        return Some(build_redirect_response(302, location));
    }

    if let Some(file_path) = &rules.mock_file {
        return load_file_response(file_path, rules, verbose_logging, ctx).await;
    }

    if let Some(file_path) = &rules.mock_rawfile {
        return load_rawfile_response(file_path, verbose_logging, ctx).await;
    }

    if let Some(template) = &rules.mock_template {
        return Some(build_template_response(
            template,
            rules,
            verbose_logging,
            ctx,
        ));
    }

    None
}

fn build_status_response(status: u16, rules: &ResolvedRules) -> Response<BoxBody> {
    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    let body = rules
        .res_body
        .clone()
        .unwrap_or_else(|| Bytes::from(status_code.canonical_reason().unwrap_or("")));

    let mut builder = Response::builder().status(status_code);

    for (key, value) in &rules.res_headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    builder.body(full_body(body)).unwrap()
}

fn build_redirect_response(status: u16, location: &str) -> Response<BoxBody> {
    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::FOUND);
    Response::builder()
        .status(status_code)
        .header(header::LOCATION, location)
        .body(full_body(Bytes::new()))
        .unwrap()
}

async fn load_file_response(
    file_path: &str,
    rules: &ResolvedRules,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Option<Response<BoxBody>> {
    let path = Path::new(file_path);

    match tokio::fs::read(path).await {
        Ok(content) => {
            if verbose_logging {
                debug!(
                    "[{}] [FILE] loaded {} ({} bytes)",
                    ctx.id_str(),
                    file_path,
                    content.len()
                );
            }

            let content_type = guess_content_type(file_path);
            let status = rules.status_code.unwrap_or(200);
            let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);

            let mut builder = Response::builder()
                .status(status_code)
                .header(header::CONTENT_TYPE, content_type);

            for (key, value) in &rules.res_headers {
                builder = builder.header(key.as_str(), value.as_str());
            }

            Some(builder.body(full_body(content)).unwrap())
        }
        Err(e) => {
            warn!(
                "[{}] [FILE] failed to read {}: {}",
                ctx.id_str(),
                file_path,
                e
            );
            Some(build_error_response(404, "File not found"))
        }
    }
}

async fn load_rawfile_response(
    file_path: &str,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Option<Response<BoxBody>> {
    let path = Path::new(file_path);

    match tokio::fs::read(path).await {
        Ok(content) => {
            if verbose_logging {
                debug!(
                    "[{}] [RAWFILE] loaded {} ({} bytes)",
                    ctx.id_str(),
                    file_path,
                    content.len()
                );
            }

            let content_type = guess_content_type(file_path);

            Some(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, content_type)
                    .body(full_body(content))
                    .unwrap(),
            )
        }
        Err(e) => {
            warn!(
                "[{}] [RAWFILE] failed to read {}: {}",
                ctx.id_str(),
                file_path,
                e
            );
            Some(build_error_response(404, "File not found"))
        }
    }
}

fn build_template_response(
    template: &str,
    rules: &ResolvedRules,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Response<BoxBody> {
    let rendered = template.to_string();

    if verbose_logging {
        debug!(
            "[{}] [TPL] rendered template ({} bytes)",
            ctx.id_str(),
            rendered.len()
        );
    }

    let status = rules.status_code.unwrap_or(200);
    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);

    let content_type = rules.res_type.as_deref().unwrap_or("application/json");

    let mut builder = Response::builder()
        .status(status_code)
        .header(header::CONTENT_TYPE, content_type);

    for (key, value) in &rules.res_headers {
        builder = builder.header(key.as_str(), value.as_str());
    }

    builder.body(full_body(rendered)).unwrap()
}

fn build_error_response(status: u16, message: &str) -> Response<BoxBody> {
    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    Response::builder()
        .status(status_code)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(full_body(message.to_string()))
        .unwrap()
}

fn guess_content_type(file_path: &str) -> &'static str {
    let ext = Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext.to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

pub fn should_intercept_response(rules: &ResolvedRules) -> bool {
    rules.status_code.is_some()
        || rules.redirect.is_some()
        || rules.location_href.is_some()
        || rules.mock_file.is_some()
        || rules.mock_rawfile.is_some()
        || rules.mock_template.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guess_content_type_html() {
        assert_eq!(
            guess_content_type("/path/to/file.html"),
            "text/html; charset=utf-8"
        );
        assert_eq!(
            guess_content_type("/path/to/file.htm"),
            "text/html; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_content_type_js() {
        assert_eq!(
            guess_content_type("/path/to/file.js"),
            "application/javascript; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_content_type_json() {
        assert_eq!(
            guess_content_type("/path/to/file.json"),
            "application/json; charset=utf-8"
        );
    }

    #[test]
    fn test_guess_content_type_image() {
        assert_eq!(guess_content_type("/path/to/file.png"), "image/png");
        assert_eq!(guess_content_type("/path/to/file.jpg"), "image/jpeg");
        assert_eq!(guess_content_type("/path/to/file.gif"), "image/gif");
    }

    #[test]
    fn test_guess_content_type_unknown() {
        assert_eq!(
            guess_content_type("/path/to/file.xyz"),
            "application/octet-stream"
        );
        assert_eq!(
            guess_content_type("/path/to/file"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_build_redirect_response() {
        let response = build_redirect_response(302, "https://example.com/new");
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "https://example.com/new"
        );
    }

    #[test]
    fn test_build_status_response() {
        let rules = ResolvedRules::default();
        let response = build_status_response(404, &rules);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_should_intercept_response() {
        let mut rules = ResolvedRules::default();
        assert!(!should_intercept_response(&rules));

        rules.status_code = Some(200);
        assert!(should_intercept_response(&rules));

        rules.status_code = None;
        rules.mock_file = Some("/path/to/file".to_string());
        assert!(should_intercept_response(&rules));

        rules.mock_file = None;
        rules.redirect = Some("/new/path".to_string());
        assert!(should_intercept_response(&rules));
    }
}

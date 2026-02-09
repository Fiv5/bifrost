use hyper::{Response, StatusCode};
use rust_embed::{Embed, RustEmbed};

use crate::handlers::{full_body, BoxBody};

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[prefix = ""]
struct Asset;

pub fn serve_static_file(path: &str) -> Response<BoxBody> {
    let file_path = if path.is_empty() || path == "/" {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };

    match <Asset as Embed>::get(file_path) {
        Some(content) => {
            let mime = mime_guess::from_path(file_path)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime)
                .header("Cache-Control", "public, max-age=3600")
                .body(full_body(content.data.to_vec()))
                .unwrap()
        }
        None => match <Asset as Embed>::get("index.html") {
            Some(content) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/html")
                .body(full_body(content.data.to_vec()))
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain")
                .body(full_body("Not Found"))
                .unwrap(),
        },
    }
}

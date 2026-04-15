use hyper::header::HeaderValue;

pub fn is_allowed_origin(origin: &str) -> bool {
    let origin_lower = origin.to_ascii_lowercase();

    let host = origin_lower
        .strip_prefix("http://")
        .or_else(|| origin_lower.strip_prefix("https://"))
        .or_else(|| origin_lower.strip_prefix("tauri://"))
        .unwrap_or(&origin_lower);

    let host_without_port = if let Some(bracket_end) = host.find(']') {
        let after_bracket = &host[bracket_end + 1..];
        if let Some(rest) = after_bracket.strip_prefix(':') {
            if rest.chars().all(|c| c.is_ascii_digit()) {
                &host[..bracket_end + 1]
            } else {
                host
            }
        } else {
            host
        }
    } else if let Some(colon_pos) = host.rfind(':') {
        let after_colon = &host[colon_pos + 1..];
        if after_colon.chars().all(|c| c.is_ascii_digit()) {
            &host[..colon_pos]
        } else {
            host
        }
    } else {
        host
    };

    matches!(
        host_without_port,
        "localhost" | "127.0.0.1" | "[::1]" | "0.0.0.0" | "tauri.localhost" | "bifrost.local"
    )
}

pub fn allowed_origin_header_value(origin: &str) -> Option<HeaderValue> {
    if is_allowed_origin(origin) {
        HeaderValue::from_str(origin).ok()
    } else {
        None
    }
}

pub fn apply_cors_headers(
    resp: &mut hyper::Response<super::handlers::BoxBody>,
    origin: Option<&str>,
) {
    let headers = resp.headers_mut();
    headers.remove("Access-Control-Allow-Origin");

    if let Some(origin) = origin {
        if let Some(value) = allowed_origin_header_value(origin) {
            headers.insert("Access-Control-Allow-Origin", value);
            headers.insert("Vary", HeaderValue::from_static("Origin"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_localhost_origins() {
        assert!(is_allowed_origin("http://localhost"));
        assert!(is_allowed_origin("http://localhost:8800"));
        assert!(is_allowed_origin("http://localhost:3000"));
        assert!(is_allowed_origin("https://localhost:8800"));
        assert!(is_allowed_origin("http://127.0.0.1"));
        assert!(is_allowed_origin("http://127.0.0.1:8800"));
        assert!(is_allowed_origin("http://127.0.0.1:9900"));
        assert!(is_allowed_origin("https://127.0.0.1:8800"));
        assert!(is_allowed_origin("http://[::1]"));
        assert!(is_allowed_origin("http://[::1]:8800"));
        assert!(is_allowed_origin("http://0.0.0.0:8800"));
        assert!(is_allowed_origin("https://tauri.localhost"));
        assert!(is_allowed_origin("http://bifrost.local"));
        assert!(is_allowed_origin("http://bifrost.local:8800"));
        assert!(is_allowed_origin("tauri://localhost"));
    }

    #[test]
    fn blocks_external_origins() {
        assert!(!is_allowed_origin("http://evil.com"));
        assert!(!is_allowed_origin("https://attacker.example.com"));
        assert!(!is_allowed_origin("http://192.168.1.100:8800"));
        assert!(!is_allowed_origin("http://10.0.0.1:8800"));
        assert!(!is_allowed_origin("http://localhost.evil.com"));
        assert!(!is_allowed_origin("http://my-server.com"));
    }

    #[test]
    fn no_origin_returns_none() {
        assert!(allowed_origin_header_value("http://evil.com").is_none());
    }

    #[test]
    fn valid_origin_returns_header_value() {
        let val = allowed_origin_header_value("http://localhost:8800");
        assert!(val.is_some());
        assert_eq!(val.unwrap().to_str().unwrap(), "http://localhost:8800");
    }

    #[test]
    fn apply_cors_headers_adds_allowed_origin() {
        let mut resp = hyper::Response::builder()
            .status(200)
            .header("Access-Control-Allow-Origin", "*")
            .body(super::super::handlers::empty_body())
            .unwrap();

        apply_cors_headers(&mut resp, Some("http://localhost:8800"));

        assert_eq!(
            resp.headers()
                .get("Access-Control-Allow-Origin")
                .unwrap()
                .to_str()
                .unwrap(),
            "http://localhost:8800"
        );
        assert_eq!(
            resp.headers().get("Vary").unwrap().to_str().unwrap(),
            "Origin"
        );
    }

    #[test]
    fn apply_cors_headers_removes_wildcard_for_disallowed_origin() {
        let mut resp = hyper::Response::builder()
            .status(200)
            .header("Access-Control-Allow-Origin", "*")
            .body(super::super::handlers::empty_body())
            .unwrap();

        apply_cors_headers(&mut resp, Some("http://evil.com"));

        assert!(resp.headers().get("Access-Control-Allow-Origin").is_none());
    }

    #[test]
    fn apply_cors_headers_no_origin_header() {
        let mut resp = hyper::Response::builder()
            .status(200)
            .header("Access-Control-Allow-Origin", "*")
            .body(super::super::handlers::empty_body())
            .unwrap();

        apply_cors_headers(&mut resp, None);

        assert!(resp.headers().get("Access-Control-Allow-Origin").is_none());
    }
}

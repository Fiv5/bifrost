use hyper::header::{HeaderName, HeaderValue};
use hyper::http::request::Parts;
use tracing::debug;

use crate::server::ResolvedRules;

pub fn apply_req_rules(parts: &mut Parts, rules: &ResolvedRules) {
    apply_req_headers(parts, rules);
    apply_req_cookies(parts, rules);
    apply_req_method(parts, rules);
    apply_req_ua(parts, rules);
    apply_req_referer(parts, rules);

    if rules.enable_cors {
        apply_req_cors(parts);
    }
}

fn apply_req_headers(parts: &mut Parts, rules: &ResolvedRules) {
    for (name, value) in &rules.req_headers {
        if let (Ok(header_name), Ok(header_value)) =
            (name.parse::<HeaderName>(), value.parse::<HeaderValue>())
        {
            debug!("Setting request header: {} = {}", name, value);
            parts.headers.insert(header_name, header_value);
        }
    }
}

fn apply_req_cookies(parts: &mut Parts, rules: &ResolvedRules) {
    if rules.req_cookies.is_empty() {
        return;
    }

    let existing_cookies = parts
        .headers
        .get(hyper::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let mut cookies: Vec<String> = if existing_cookies.is_empty() {
        Vec::new()
    } else {
        existing_cookies
            .split(';')
            .map(|s| s.trim().to_string())
            .collect()
    };

    for (name, value) in &rules.req_cookies {
        let cookie_str = format!("{}={}", name, value);
        let found = cookies
            .iter()
            .position(|c| c.starts_with(&format!("{}=", name)));
        if let Some(idx) = found {
            cookies[idx] = cookie_str;
        } else {
            cookies.push(cookie_str);
        }
    }

    let cookie_header = cookies.join("; ");
    if let Ok(header_value) = cookie_header.parse::<HeaderValue>() {
        debug!("Setting Cookie header: {}", cookie_header);
        parts.headers.insert(hyper::header::COOKIE, header_value);
    }
}

fn apply_req_method(parts: &mut Parts, rules: &ResolvedRules) {
    if let Some(ref method) = rules.method {
        if let Ok(m) = method.parse() {
            debug!("Changing request method to: {}", method);
            parts.method = m;
        }
    }
}

fn apply_req_ua(parts: &mut Parts, rules: &ResolvedRules) {
    if let Some(ref ua) = rules.ua {
        if let Ok(header_value) = ua.parse::<HeaderValue>() {
            debug!("Setting User-Agent: {}", ua);
            parts
                .headers
                .insert(hyper::header::USER_AGENT, header_value);
        }
    }
}

fn apply_req_referer(parts: &mut Parts, rules: &ResolvedRules) {
    if let Some(ref referer) = rules.referer {
        if let Ok(header_value) = referer.parse::<HeaderValue>() {
            debug!("Setting Referer: {}", referer);
            parts.headers.insert(hyper::header::REFERER, header_value);
        }
    }
}

fn apply_req_cors(parts: &mut Parts) {
    parts.headers.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    parts.headers.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS, PATCH"),
    );
    parts.headers.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("*"),
    );
    parts.headers.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
}

pub fn parse_cookie_string(cookie_str: &str) -> Vec<(String, String)> {
    cookie_str
        .split(';')
        .filter_map(|pair| {
            let mut parts = pair.trim().splitn(2, '=');
            let name = parts.next()?.trim().to_string();
            let value = parts.next().unwrap_or("").trim().to_string();
            if name.is_empty() {
                None
            } else {
                Some((name, value))
            }
        })
        .collect()
}

pub fn format_cookie_header(cookies: &[(String, String)]) -> String {
    cookies
        .iter()
        .map(|(name, value)| format!("{}={}", name, value))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Method;

    fn create_test_parts() -> Parts {
        let (parts, _) = hyper::Request::builder()
            .method(Method::GET)
            .uri("http://example.com/path")
            .body(())
            .unwrap()
            .into_parts();
        parts
    }

    #[test]
    fn test_apply_req_headers() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules
            .req_headers
            .push(("X-Custom-Header".to_string(), "custom-value".to_string()));
        rules
            .req_headers
            .push(("X-Another".to_string(), "another-value".to_string()));

        apply_req_rules(&mut parts, &rules);

        assert_eq!(
            parts
                .headers
                .get("X-Custom-Header")
                .unwrap()
                .to_str()
                .unwrap(),
            "custom-value"
        );
        assert_eq!(
            parts.headers.get("X-Another").unwrap().to_str().unwrap(),
            "another-value"
        );
    }

    #[test]
    fn test_apply_req_cookies_new() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules
            .req_cookies
            .push(("session".to_string(), "abc123".to_string()));
        rules
            .req_cookies
            .push(("user".to_string(), "test".to_string()));

        apply_req_rules(&mut parts, &rules);

        let cookie = parts
            .headers
            .get(hyper::header::COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("session=abc123"));
        assert!(cookie.contains("user=test"));
    }

    #[test]
    fn test_apply_req_cookies_merge() {
        let mut parts = create_test_parts();
        parts.headers.insert(
            hyper::header::COOKIE,
            HeaderValue::from_static("existing=value; session=old"),
        );

        let mut rules = ResolvedRules::default();
        rules
            .req_cookies
            .push(("session".to_string(), "new".to_string()));
        rules
            .req_cookies
            .push(("added".to_string(), "cookie".to_string()));

        apply_req_rules(&mut parts, &rules);

        let cookie = parts
            .headers
            .get(hyper::header::COOKIE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(cookie.contains("existing=value"));
        assert!(cookie.contains("session=new"));
        assert!(cookie.contains("added=cookie"));
        assert!(!cookie.contains("session=old"));
    }

    #[test]
    fn test_apply_req_method() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules.method = Some("POST".to_string());

        apply_req_rules(&mut parts, &rules);

        assert_eq!(parts.method, Method::POST);
    }

    #[test]
    fn test_apply_req_ua() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules.ua = Some("Custom-Agent/1.0".to_string());

        apply_req_rules(&mut parts, &rules);

        assert_eq!(
            parts
                .headers
                .get(hyper::header::USER_AGENT)
                .unwrap()
                .to_str()
                .unwrap(),
            "Custom-Agent/1.0"
        );
    }

    #[test]
    fn test_apply_req_referer() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules.referer = Some("http://referrer.com".to_string());

        apply_req_rules(&mut parts, &rules);

        assert_eq!(
            parts
                .headers
                .get(hyper::header::REFERER)
                .unwrap()
                .to_str()
                .unwrap(),
            "http://referrer.com"
        );
    }

    #[test]
    fn test_apply_req_cors() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules.enable_cors = true;

        apply_req_rules(&mut parts, &rules);

        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_ALLOW_METHODS));
        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS));
    }

    #[test]
    fn test_parse_cookie_string() {
        let cookies = parse_cookie_string("name=value; session=abc123; empty=");
        assert_eq!(cookies.len(), 3);
        assert_eq!(cookies[0], ("name".to_string(), "value".to_string()));
        assert_eq!(cookies[1], ("session".to_string(), "abc123".to_string()));
        assert_eq!(cookies[2], ("empty".to_string(), "".to_string()));
    }

    #[test]
    fn test_parse_cookie_string_empty() {
        let cookies = parse_cookie_string("");
        assert!(cookies.is_empty());
    }

    #[test]
    fn test_format_cookie_header() {
        let cookies = vec![
            ("name".to_string(), "value".to_string()),
            ("session".to_string(), "abc123".to_string()),
        ];
        let header = format_cookie_header(&cookies);
        assert_eq!(header, "name=value; session=abc123");
    }

    #[test]
    fn test_format_cookie_header_empty() {
        let cookies: Vec<(String, String)> = vec![];
        let header = format_cookie_header(&cookies);
        assert_eq!(header, "");
    }
}

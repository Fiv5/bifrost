use hyper::header::{HeaderName, HeaderValue};
use hyper::http::response::Parts;
use hyper::StatusCode;
use tracing::debug;

use crate::server::ResolvedRules;

pub fn apply_res_rules(parts: &mut Parts, rules: &ResolvedRules) {
    apply_res_status(parts, rules);
    apply_res_headers(parts, rules);
    apply_res_cookies(parts, rules);

    if rules.enable_cors {
        apply_res_cors(parts);
    }
}

fn apply_res_status(parts: &mut Parts, rules: &ResolvedRules) {
    if let Some(status_code) = rules.status_code {
        if let Ok(status) = StatusCode::from_u16(status_code) {
            debug!("Setting response status: {}", status_code);
            parts.status = status;
        }
    }
}

fn apply_res_headers(parts: &mut Parts, rules: &ResolvedRules) {
    for (name, value) in &rules.res_headers {
        if let (Ok(header_name), Ok(header_value)) =
            (name.parse::<HeaderName>(), value.parse::<HeaderValue>())
        {
            debug!("Setting response header: {} = {}", name, value);
            parts.headers.insert(header_name, header_value);
        }
    }
}

fn apply_res_cookies(parts: &mut Parts, rules: &ResolvedRules) {
    for (name, value) in &rules.res_cookies {
        let cookie_value = format!("{}={}", name, value);
        if let Ok(header_value) = cookie_value.parse::<HeaderValue>() {
            debug!("Setting Set-Cookie: {}", cookie_value);
            parts
                .headers
                .append(hyper::header::SET_COOKIE, header_value);
        }
    }
}

fn apply_res_cors(parts: &mut Parts) {
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
    parts.headers.insert(
        hyper::header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("*"),
    );
}

pub fn parse_set_cookie(cookie_str: &str) -> Option<(String, String, SetCookieOptions)> {
    let mut parts = cookie_str.split(';');
    let name_value = parts.next()?;
    let mut nv_parts = name_value.splitn(2, '=');
    let name = nv_parts.next()?.trim().to_string();
    let value = nv_parts.next().unwrap_or("").trim().to_string();

    if name.is_empty() {
        return None;
    }

    let mut options = SetCookieOptions::default();

    for part in parts {
        let part = part.trim();
        let lower = part.to_lowercase();

        if lower.starts_with("path=") {
            options.path = Some(part[5..].to_string());
        } else if lower.starts_with("domain=") {
            options.domain = Some(part[7..].to_string());
        } else if lower.starts_with("max-age=") {
            if let Ok(max_age) = part[8..].parse() {
                options.max_age = Some(max_age);
            }
        } else if lower.starts_with("expires=") {
            options.expires = Some(part[8..].to_string());
        } else if lower == "secure" {
            options.secure = true;
        } else if lower == "httponly" {
            options.http_only = true;
        } else if lower.starts_with("samesite=") {
            options.same_site = Some(part[9..].to_string());
        }
    }

    Some((name, value, options))
}

#[derive(Debug, Clone, Default)]
pub struct SetCookieOptions {
    pub path: Option<String>,
    pub domain: Option<String>,
    pub max_age: Option<i64>,
    pub expires: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
}

impl SetCookieOptions {
    pub fn to_cookie_string(&self, name: &str, value: &str) -> String {
        let mut cookie = format!("{}={}", name, value);

        if let Some(ref path) = self.path {
            cookie.push_str(&format!("; Path={}", path));
        }
        if let Some(ref domain) = self.domain {
            cookie.push_str(&format!("; Domain={}", domain));
        }
        if let Some(max_age) = self.max_age {
            cookie.push_str(&format!("; Max-Age={}", max_age));
        }
        if let Some(ref expires) = self.expires {
            cookie.push_str(&format!("; Expires={}", expires));
        }
        if self.secure {
            cookie.push_str("; Secure");
        }
        if self.http_only {
            cookie.push_str("; HttpOnly");
        }
        if let Some(ref same_site) = self.same_site {
            cookie.push_str(&format!("; SameSite={}", same_site));
        }

        cookie
    }
}

pub fn format_set_cookie(name: &str, value: &str, options: &SetCookieOptions) -> String {
    options.to_cookie_string(name, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Response;

    fn create_test_parts() -> Parts {
        let (parts, _) = Response::builder()
            .status(200)
            .body(())
            .unwrap()
            .into_parts();
        parts
    }

    #[test]
    fn test_apply_res_status() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules.status_code = Some(404);

        apply_res_rules(&mut parts, &rules);

        assert_eq!(parts.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_apply_res_headers() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules
            .res_headers
            .push(("X-Custom-Header".to_string(), "custom-value".to_string()));
        rules
            .res_headers
            .push(("Content-Type".to_string(), "application/json".to_string()));

        apply_res_rules(&mut parts, &rules);

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
            parts.headers.get("Content-Type").unwrap().to_str().unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_apply_res_cookies() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules
            .res_cookies
            .push(("session".to_string(), "abc123".to_string()));
        rules
            .res_cookies
            .push(("user".to_string(), "test".to_string()));

        apply_res_rules(&mut parts, &rules);

        let cookies: Vec<_> = parts
            .headers
            .get_all(hyper::header::SET_COOKIE)
            .iter()
            .collect();
        assert_eq!(cookies.len(), 2);
    }

    #[test]
    fn test_apply_res_cors() {
        let mut parts = create_test_parts();
        let mut rules = ResolvedRules::default();
        rules.enable_cors = true;

        apply_res_rules(&mut parts, &rules);

        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_ALLOW_METHODS));
        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS));
        assert!(parts
            .headers
            .contains_key(hyper::header::ACCESS_CONTROL_EXPOSE_HEADERS));
    }

    #[test]
    fn test_parse_set_cookie_simple() {
        let (name, value, options) = parse_set_cookie("session=abc123").unwrap();
        assert_eq!(name, "session");
        assert_eq!(value, "abc123");
        assert!(options.path.is_none());
    }

    #[test]
    fn test_parse_set_cookie_with_options() {
        let cookie =
            "session=abc123; Path=/; Domain=example.com; Secure; HttpOnly; SameSite=Strict";
        let (name, value, options) = parse_set_cookie(cookie).unwrap();
        assert_eq!(name, "session");
        assert_eq!(value, "abc123");
        assert_eq!(options.path, Some("/".to_string()));
        assert_eq!(options.domain, Some("example.com".to_string()));
        assert!(options.secure);
        assert!(options.http_only);
        assert_eq!(options.same_site, Some("Strict".to_string()));
    }

    #[test]
    fn test_parse_set_cookie_with_max_age() {
        let cookie = "session=abc123; Max-Age=3600";
        let (_, _, options) = parse_set_cookie(cookie).unwrap();
        assert_eq!(options.max_age, Some(3600));
    }

    #[test]
    fn test_parse_set_cookie_with_expires() {
        let cookie = "session=abc123; Expires=Wed, 09 Jun 2021 10:18:14 GMT";
        let (_, _, options) = parse_set_cookie(cookie).unwrap();
        assert_eq!(
            options.expires,
            Some("Wed, 09 Jun 2021 10:18:14 GMT".to_string())
        );
    }

    #[test]
    fn test_parse_set_cookie_empty_value() {
        let (name, value, _) = parse_set_cookie("session=").unwrap();
        assert_eq!(name, "session");
        assert_eq!(value, "");
    }

    #[test]
    fn test_parse_set_cookie_invalid() {
        let result = parse_set_cookie("=value");
        assert!(result.is_none());
    }

    #[test]
    fn test_set_cookie_options_to_string() {
        let options = SetCookieOptions {
            path: Some("/api".to_string()),
            domain: Some("example.com".to_string()),
            max_age: Some(3600),
            expires: None,
            secure: true,
            http_only: true,
            same_site: Some("Lax".to_string()),
        };

        let cookie_str = options.to_cookie_string("session", "abc123");
        assert!(cookie_str.contains("session=abc123"));
        assert!(cookie_str.contains("Path=/api"));
        assert!(cookie_str.contains("Domain=example.com"));
        assert!(cookie_str.contains("Max-Age=3600"));
        assert!(cookie_str.contains("Secure"));
        assert!(cookie_str.contains("HttpOnly"));
        assert!(cookie_str.contains("SameSite=Lax"));
    }

    #[test]
    fn test_set_cookie_options_default() {
        let options = SetCookieOptions::default();
        assert!(options.path.is_none());
        assert!(options.domain.is_none());
        assert!(options.max_age.is_none());
        assert!(!options.secure);
        assert!(!options.http_only);
    }

    #[test]
    fn test_format_set_cookie() {
        let options = SetCookieOptions {
            path: Some("/".to_string()),
            secure: true,
            ..Default::default()
        };

        let cookie = format_set_cookie("test", "value", &options);
        assert_eq!(cookie, "test=value; Path=/; Secure");
    }
}

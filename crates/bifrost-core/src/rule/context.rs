use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);
static START_TIME: once_cell::sync::Lazy<i64> =
    once_cell::sync::Lazy::new(|| Utc::now().timestamp_millis());

fn generate_req_id() -> String {
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let time = *START_TIME;
    format!("{:x}-{:x}", time, counter)
}

#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    pub req_id: String,
    pub client_id: Option<String>,
    pub local_client_id: Option<String>,

    pub url: String,
    pub host: String,
    pub hostname: String,
    pub port: u16,
    pub path: String,
    pub pathname: String,
    pub query: Option<String>,
    pub search: Option<String>,

    pub real_url: Option<String>,
    pub real_host: Option<String>,
    pub real_port: Option<u16>,

    pub client_ip: String,
    pub client_port: u16,
    pub remote_address: String,
    pub remote_port: u16,

    pub method: String,
    pub req_headers: HashMap<String, String>,
    pub req_cookies: HashMap<String, String>,

    pub status_code: Option<u16>,
    pub server_ip: Option<String>,
    pub server_port: Option<u16>,
    pub res_headers: Option<HashMap<String, String>>,
    pub res_cookies: Option<HashMap<String, String>>,
}

impl RequestContext {
    pub fn new() -> Self {
        Self {
            req_id: generate_req_id(),
            ..Default::default()
        }
    }

    pub fn from_url(url_str: &str) -> Self {
        let mut ctx = Self::new();
        ctx.url = url_str.to_string();

        if let Ok(parsed) = url::Url::parse(url_str) {
            ctx.hostname = parsed.host_str().unwrap_or("").to_string();
            ctx.port = parsed.port().unwrap_or_else(|| match parsed.scheme() {
                "https" | "wss" => 443,
                _ => 80,
            });
            ctx.host = if ctx.port == 80 || ctx.port == 443 {
                ctx.hostname.clone()
            } else {
                format!("{}:{}", ctx.hostname, ctx.port)
            };
            ctx.pathname = parsed.path().to_string();
            ctx.query = parsed.query().map(|s| s.to_string());
            ctx.search = ctx.query.as_ref().map(|q| format!("?{}", q));
            ctx.path = if let Some(ref q) = ctx.search {
                format!("{}{}", ctx.pathname, q)
            } else {
                ctx.pathname.clone()
            };
        }

        ctx
    }

    pub fn builder() -> RequestContextBuilder {
        RequestContextBuilder::new()
    }

    pub fn with_method(mut self, method: &str) -> Self {
        self.method = method.to_string();
        self
    }

    pub fn with_client_ip(mut self, ip: &str) -> Self {
        self.client_ip = ip.to_string();
        self
    }

    pub fn with_client_port(mut self, port: u16) -> Self {
        self.client_port = port;
        self
    }

    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.req_headers
            .insert(key.to_lowercase(), value.to_string());
        self
    }

    pub fn with_cookie(mut self, key: &str, value: &str) -> Self {
        self.req_cookies.insert(key.to_string(), value.to_string());
        self
    }

    pub fn set_response(&mut self, status_code: u16, headers: HashMap<String, String>) {
        self.status_code = Some(status_code);
        self.res_headers = Some(headers);
    }

    pub fn set_server_info(&mut self, ip: &str, port: u16) {
        self.server_ip = Some(ip.to_string());
        self.server_port = Some(port);
    }

    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.req_headers.get(&key.to_lowercase())
    }

    pub fn get_res_header(&self, key: &str) -> Option<&String> {
        self.res_headers.as_ref()?.get(&key.to_lowercase())
    }

    pub fn get_cookie(&self, key: &str) -> Option<&String> {
        self.req_cookies.get(key)
    }

    pub fn get_res_cookie(&self, key: &str) -> Option<&String> {
        self.res_cookies.as_ref()?.get(key)
    }

    pub fn get_query_param(&self, key: &str) -> Option<String> {
        let query = self.query.as_ref()?;
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                if k == key {
                    return Some(urlencoding::decode(v).unwrap_or_default().into_owned());
                }
            }
        }
        None
    }

    pub fn parse_cookies_from_header(&mut self) {
        if let Some(cookie_header) = self.req_headers.get("cookie").cloned() {
            for cookie in cookie_header.split(';') {
                let cookie = cookie.trim();
                if let Some(eq_pos) = cookie.find('=') {
                    let key = cookie[..eq_pos].trim().to_string();
                    let value = cookie[eq_pos + 1..].trim().to_string();
                    self.req_cookies.insert(key, value);
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct RequestContextBuilder {
    ctx: RequestContext,
}

impl RequestContextBuilder {
    pub fn new() -> Self {
        Self {
            ctx: RequestContext::new(),
        }
    }

    pub fn url(mut self, url: &str) -> Self {
        let parsed_ctx = RequestContext::from_url(url);
        self.ctx.url = parsed_ctx.url;
        self.ctx.host = parsed_ctx.host;
        self.ctx.hostname = parsed_ctx.hostname;
        self.ctx.port = parsed_ctx.port;
        self.ctx.path = parsed_ctx.path;
        self.ctx.pathname = parsed_ctx.pathname;
        self.ctx.query = parsed_ctx.query;
        self.ctx.search = parsed_ctx.search;
        self
    }

    pub fn host(mut self, host: &str) -> Self {
        self.ctx.host = host.to_string();
        self
    }

    pub fn hostname(mut self, hostname: &str) -> Self {
        self.ctx.hostname = hostname.to_string();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.ctx.port = port;
        self
    }

    pub fn path(mut self, path: &str) -> Self {
        self.ctx.path = path.to_string();
        self
    }

    pub fn pathname(mut self, pathname: &str) -> Self {
        self.ctx.pathname = pathname.to_string();
        self
    }

    pub fn query(mut self, query: &str) -> Self {
        self.ctx.query = Some(query.to_string());
        self
    }

    pub fn search(mut self, search: &str) -> Self {
        self.ctx.search = Some(search.to_string());
        self
    }

    pub fn method(mut self, method: &str) -> Self {
        self.ctx.method = method.to_string();
        self
    }

    pub fn status_code(mut self, code: u16) -> Self {
        self.ctx.status_code = Some(code);
        self
    }

    pub fn client_ip(mut self, ip: &str) -> Self {
        self.ctx.client_ip = ip.to_string();
        self.ctx.remote_address = ip.to_string();
        self
    }

    pub fn client_port(mut self, port: u16) -> Self {
        self.ctx.client_port = port;
        self.ctx.remote_port = port;
        self
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.ctx
            .req_headers
            .insert(key.to_lowercase(), value.to_string());
        self
    }

    pub fn req_headers(mut self, headers: HashMap<String, String>) -> Self {
        for (k, v) in headers {
            self.ctx.req_headers.insert(k.to_lowercase(), v);
        }
        self
    }

    pub fn cookie(mut self, key: &str, value: &str) -> Self {
        self.ctx
            .req_cookies
            .insert(key.to_string(), value.to_string());
        self
    }

    pub fn req_cookies(mut self, cookies: HashMap<String, String>) -> Self {
        self.ctx.req_cookies.extend(cookies);
        self
    }

    pub fn client_id(mut self, id: &str) -> Self {
        self.ctx.client_id = Some(id.to_string());
        self
    }

    pub fn local_client_id(mut self, id: &str) -> Self {
        self.ctx.local_client_id = Some(id.to_string());
        self
    }

    pub fn real_url(mut self, url: &str) -> Self {
        self.ctx.real_url = Some(url.to_string());
        self
    }

    pub fn real_host(mut self, host: &str) -> Self {
        self.ctx.real_host = Some(host.to_string());
        self
    }

    pub fn real_port(mut self, port: u16) -> Self {
        self.ctx.real_port = Some(port);
        self
    }

    pub fn build(mut self) -> RequestContext {
        self.ctx.parse_cookies_from_header();
        self.ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_context_new() {
        let ctx = RequestContext::new();
        assert!(!ctx.req_id.is_empty());
    }

    #[test]
    fn test_request_context_from_url() {
        let ctx = RequestContext::from_url(
            "https://api.example.com:8080/path/to/resource?name=test&id=123",
        );
        assert_eq!(ctx.hostname, "api.example.com");
        assert_eq!(ctx.port, 8080);
        assert_eq!(ctx.host, "api.example.com:8080");
        assert_eq!(ctx.pathname, "/path/to/resource");
        assert_eq!(ctx.query, Some("name=test&id=123".to_string()));
        assert_eq!(ctx.search, Some("?name=test&id=123".to_string()));
        assert_eq!(ctx.path, "/path/to/resource?name=test&id=123");
    }

    #[test]
    fn test_request_context_from_url_default_port() {
        let ctx = RequestContext::from_url("https://example.com/path");
        assert_eq!(ctx.port, 443);
        assert_eq!(ctx.host, "example.com");

        let ctx = RequestContext::from_url("http://example.com/path");
        assert_eq!(ctx.port, 80);
        assert_eq!(ctx.host, "example.com");
    }

    #[test]
    fn test_request_context_builder() {
        let ctx = RequestContext::builder()
            .url("https://api.example.com/test")
            .method("POST")
            .client_ip("192.168.1.100")
            .client_port(54321)
            .header("Content-Type", "application/json")
            .header("Authorization", "Bearer token123")
            .cookie("session", "abc123")
            .build();

        assert_eq!(ctx.hostname, "api.example.com");
        assert_eq!(ctx.method, "POST");
        assert_eq!(ctx.client_ip, "192.168.1.100");
        assert_eq!(ctx.client_port, 54321);
        assert_eq!(
            ctx.get_header("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(ctx.get_cookie("session"), Some(&"abc123".to_string()));
    }

    #[test]
    fn test_get_query_param() {
        let ctx =
            RequestContext::from_url("https://example.com/path?name=john&age=30&city=new%20york");
        assert_eq!(ctx.get_query_param("name"), Some("john".to_string()));
        assert_eq!(ctx.get_query_param("age"), Some("30".to_string()));
        assert_eq!(ctx.get_query_param("city"), Some("new york".to_string()));
        assert_eq!(ctx.get_query_param("unknown"), None);
    }

    #[test]
    fn test_parse_cookies_from_header() {
        let mut ctx = RequestContext::new();
        ctx.req_headers.insert(
            "cookie".to_string(),
            "session=abc123; user=john; token=xyz".to_string(),
        );
        ctx.parse_cookies_from_header();

        assert_eq!(ctx.get_cookie("session"), Some(&"abc123".to_string()));
        assert_eq!(ctx.get_cookie("user"), Some(&"john".to_string()));
        assert_eq!(ctx.get_cookie("token"), Some(&"xyz".to_string()));
    }

    #[test]
    fn test_set_response() {
        let mut ctx = RequestContext::new();
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "text/html".to_string());
        ctx.set_response(200, headers);

        assert_eq!(ctx.status_code, Some(200));
        assert_eq!(
            ctx.get_res_header("content-type"),
            Some(&"text/html".to_string())
        );
    }

    #[test]
    fn test_set_server_info() {
        let mut ctx = RequestContext::new();
        ctx.set_server_info("10.0.0.1", 8080);

        assert_eq!(ctx.server_ip, Some("10.0.0.1".to_string()));
        assert_eq!(ctx.server_port, Some(8080));
    }

    #[test]
    fn test_unique_req_ids() {
        let ctx1 = RequestContext::new();
        let ctx2 = RequestContext::new();
        let ctx3 = RequestContext::new();

        assert_ne!(ctx1.req_id, ctx2.req_id);
        assert_ne!(ctx2.req_id, ctx3.req_id);
        assert_ne!(ctx1.req_id, ctx3.req_id);
    }

    #[test]
    fn test_with_methods() {
        let ctx = RequestContext::from_url("https://example.com/api")
            .with_method("PUT")
            .with_client_ip("10.0.0.5")
            .with_client_port(12345)
            .with_header("X-Custom", "value")
            .with_cookie("token", "secret");

        assert_eq!(ctx.method, "PUT");
        assert_eq!(ctx.client_ip, "10.0.0.5");
        assert_eq!(ctx.client_port, 12345);
        assert_eq!(ctx.get_header("x-custom"), Some(&"value".to_string()));
        assert_eq!(ctx.get_cookie("token"), Some(&"secret".to_string()));
    }
}

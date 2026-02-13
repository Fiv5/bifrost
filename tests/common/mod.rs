use bifrost_core::Protocol;
use bifrost_proxy::{ProxyConfig, ProxyServer, ResolvedRules, RuleValue, RulesResolver};
use bytes::Bytes;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[allow(dead_code)]
pub struct TestProxy {
    pub port: u16,
    pub host: String,
    pub server: ProxyServer,
    rules: Arc<TestRulesResolver>,
}

pub struct TestRulesResolver {
    rules: parking_lot::RwLock<Vec<TestRule>>,
}

struct TestRule {
    pattern: String,
    protocol: Protocol,
    value: String,
}

impl TestRulesResolver {
    pub fn new() -> Self {
        Self {
            rules: parking_lot::RwLock::new(Vec::new()),
        }
    }

    pub fn add_rule(&self, pattern: &str, protocol: Protocol, value: &str) {
        let mut rules = self.rules.write();
        rules.push(TestRule {
            pattern: pattern.to_string(),
            protocol,
            value: value.to_string(),
        });
    }

    pub fn clear(&self) {
        let mut rules = self.rules.write();
        rules.clear();
    }
}

impl RulesResolver for TestRulesResolver {
    fn resolve_with_context(
        &self,
        url: &str,
        _method: &str,
        _req_headers: &std::collections::HashMap<String, String>,
        _req_cookies: &std::collections::HashMap<String, String>,
    ) -> ResolvedRules {
        let rules = self.rules.read();
        let mut resolved = ResolvedRules::default();

        for rule in rules.iter() {
            if url.contains(&rule.pattern) || rule.pattern == "*" {
                match rule.protocol {
                    Protocol::Host => {
                        resolved.host = Some(rule.value.clone());
                    }
                    Protocol::ReqHeaders => {
                        if let Some((key, value)) = rule.value.split_once('=') {
                            resolved
                                .req_headers
                                .push((key.to_string(), value.to_string()));
                        }
                    }
                    Protocol::ResHeaders => {
                        if let Some((key, value)) = rule.value.split_once('=') {
                            resolved
                                .res_headers
                                .push((key.to_string(), value.to_string()));
                        }
                    }
                    Protocol::ReqDelay => {
                        if let Ok(delay) = rule.value.parse() {
                            resolved.req_delay = Some(delay);
                        }
                    }
                    Protocol::ResDelay => {
                        if let Ok(delay) = rule.value.parse() {
                            resolved.res_delay = Some(delay);
                        }
                    }
                    Protocol::StatusCode => {
                        if let Ok(code) = rule.value.parse() {
                            resolved.status_code = Some(code);
                        }
                    }
                    Protocol::Method => {
                        resolved.method = Some(rule.value.clone());
                    }
                    Protocol::Ua => {
                        resolved.ua = Some(rule.value.clone());
                    }
                    Protocol::Referer => {
                        resolved.referer = Some(rule.value.clone());
                    }
                    Protocol::ReqCors | Protocol::ResCors => {
                        resolved.enable_cors = true;
                    }
                    Protocol::ReqBody => {
                        resolved.req_body = Some(Bytes::from(rule.value.clone()));
                    }
                    Protocol::ResBody => {
                        resolved.res_body = Some(Bytes::from(rule.value.clone()));
                    }
                    _ => {
                        resolved.rules.push(RuleValue {
                            pattern: rule.pattern.clone(),
                            protocol: rule.protocol,
                            value: rule.value.clone(),
                            options: HashMap::new(),
                            rule_name: None,
                            raw: None,
                            line: None,
                        });
                    }
                }
            }
        }

        resolved
    }
}

impl TestProxy {
    #[allow(dead_code)]
    pub fn port(&self) -> u16 {
        self.port
    }

    #[allow(dead_code)]
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn proxy_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

pub async fn start_test_proxy() -> TestProxy {
    start_test_proxy_with_config(ProxyConfig::default()).await
}

pub async fn start_test_proxy_with_config(mut config: ProxyConfig) -> TestProxy {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    config.port = addr.port();
    config.host = "127.0.0.1".to_string();

    let rules = Arc::new(TestRulesResolver::new());
    let server =
        ProxyServer::new(config.clone()).with_rules(Arc::clone(&rules) as Arc<dyn RulesResolver>);

    let serve_rules = Arc::clone(&rules);
    let serve_server =
        ProxyServer::new(config.clone()).with_rules(serve_rules as Arc<dyn RulesResolver>);

    tokio::spawn(async move {
        let _ = serve_server.serve(listener).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    TestProxy {
        port: config.port,
        host: config.host,
        server,
        rules,
    }
}

#[allow(dead_code)]
pub async fn start_test_proxy_with_socks5(socks5_port: u16) -> TestProxy {
    let config = ProxyConfig {
        socks5_port: Some(socks5_port),
        ..Default::default()
    };
    start_test_proxy_with_config(config).await
}

pub fn create_proxy_client(proxy: &TestProxy) -> reqwest::Client {
    let proxy_url = proxy.proxy_url();
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy_url).unwrap())
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

#[allow(dead_code)]
pub fn create_socks5_client(host: &str, port: u16) -> reqwest::Client {
    let proxy_url = format!("socks5://{}:{}", host, port);
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy_url).unwrap())
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

#[allow(dead_code)]
pub fn create_socks5_client_with_auth(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> reqwest::Client {
    let proxy_url = format!("socks5://{}:{}@{}:{}", username, password, host, port);
    reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(&proxy_url).unwrap())
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

pub fn add_test_rule(proxy: &TestProxy, pattern: &str, protocol: Protocol, value: &str) {
    proxy.rules.add_rule(pattern, protocol, value);
}

pub fn clear_test_rules(proxy: &TestProxy) {
    proxy.rules.clear();
}

#[allow(dead_code)]
pub struct MockHttpServer {
    pub port: u16,
    pub addr: SocketAddr,
}

impl MockHttpServer {
    pub async fn start() -> Self {
        use http_body_util::Full;
        use hyper::server::conn::http1;
        use hyper::service::service_fn;
        use hyper::{Request, Response};
        use hyper_util::rt::TokioIo;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let io = TokioIo::new(stream);
                    tokio::spawn(async move {
                        let service =
                            service_fn(|req: Request<hyper::body::Incoming>| async move {
                                let path = req.uri().path().to_string();
                                let method = req.method().to_string();
                                let headers: Vec<String> = req
                                    .headers()
                                    .iter()
                                    .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("")))
                                    .collect();

                                let body = format!(
                                    "{{\"path\":\"{}\",\"method\":\"{}\",\"headers\":[{}]}}",
                                    path,
                                    method,
                                    headers
                                        .iter()
                                        .map(|h| format!("\"{}\"", h))
                                        .collect::<Vec<_>>()
                                        .join(",")
                                );

                                Ok::<_, hyper::Error>(Response::new(Full::new(bytes::Bytes::from(
                                    body,
                                ))))
                            });

                        let _ = http1::Builder::new().serve_connection(io, service).await;
                    });
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Self {
            port: addr.port(),
            addr,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }
}

#[allow(dead_code)]
pub struct MockHttpsServer {
    pub port: u16,
    pub addr: SocketAddr,
}

#[allow(dead_code)]
impl MockHttpsServer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        Self {
            port: addr.port(),
            addr,
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("https://127.0.0.1:{}{}", self.port, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_start_test_proxy() {
        let proxy = start_test_proxy().await;
        assert!(proxy.port > 0);
        assert_eq!(proxy.host, "127.0.0.1");
    }

    #[tokio::test]
    async fn test_mock_http_server() {
        let server = MockHttpServer::start().await;
        assert!(server.port > 0);
        assert_eq!(
            server.url("/test"),
            format!("http://127.0.0.1:{}/test", server.port)
        );
    }

    #[test]
    fn test_create_proxy_client() {
        let config = ProxyConfig::default();
        let rules = Arc::new(TestRulesResolver::new());
        let server =
            ProxyServer::new(config.clone()).with_rules(rules.clone() as Arc<dyn RulesResolver>);
        let proxy = TestProxy {
            port: 8080,
            host: "127.0.0.1".to_string(),
            server,
            rules,
        };
        let _client = create_proxy_client(&proxy);
    }

    #[test]
    fn test_add_and_clear_rules() {
        let config = ProxyConfig::default();
        let rules = Arc::new(TestRulesResolver::new());
        let server =
            ProxyServer::new(config.clone()).with_rules(rules.clone() as Arc<dyn RulesResolver>);
        let proxy = TestProxy {
            port: 8080,
            host: "127.0.0.1".to_string(),
            server,
            rules,
        };

        add_test_rule(&proxy, "example.com", Protocol::Host, "127.0.0.1");
        let resolved = proxy.rules.resolve("http://example.com", "GET");
        assert_eq!(resolved.host, Some("127.0.0.1".to_string()));

        clear_test_rules(&proxy);
        let resolved = proxy.rules.resolve("http://example.com", "GET");
        assert_eq!(resolved.host, None);
    }
}

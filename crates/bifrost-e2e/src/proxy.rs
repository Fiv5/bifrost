use bifrost_core::{parse_rules, Rule, RulesResolver as CoreRulesResolver, RequestContext, Protocol};
use bifrost_proxy::{ProxyConfig, ProxyServer, ResolvedRules as ProxyResolvedRules, RulesResolver as ProxyRulesResolverTrait};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

struct RulesResolverAdapter {
    inner: CoreRulesResolver,
}

impl ProxyRulesResolverTrait for RulesResolverAdapter {
    fn resolve(&self, url: &str, method: &str) -> ProxyResolvedRules {
        let mut ctx = RequestContext::from_url(url);
        ctx.method = method.to_string();

        let core_result = self.inner.resolve(&ctx);
        let mut result = ProxyResolvedRules::default();

        tracing::debug!("Resolving rules for URL: {}", url);
        tracing::debug!("Found {} rules", core_result.rules.len());

        for resolved_rule in &core_result.rules {
            let protocol = resolved_rule.rule.protocol;
            let value = &resolved_rule.resolved_value;
            tracing::debug!("Processing rule: {:?} = {}", protocol, value);

            match protocol {
                Protocol::Host => {
                    result.host = Some(value.to_string());
                }
                Protocol::ReqHeaders => {
                    if let Some(headers) = parse_header_value(value) {
                        for (k, v) in headers {
                            result.req_headers.push((k, v));
                        }
                    }
                }
                Protocol::ResHeaders => {
                    tracing::debug!("Parsing ResHeaders value: {}", value);
                    if let Some(headers) = parse_header_value(value) {
                        for (k, v) in &headers {
                            tracing::debug!("Adding res header: {} = {}", k, v);
                        }
                        for (k, v) in headers {
                            result.res_headers.push((k, v));
                        }
                    } else {
                        tracing::warn!("Failed to parse ResHeaders value: {}", value);
                    }
                }
                Protocol::ReqCookies => {
                    if let Some(cookies) = parse_header_value(value) {
                        for (k, v) in cookies {
                            result.req_cookies.push((k, v));
                        }
                    }
                }
                Protocol::ResCookies => {
                    if let Some(cookies) = parse_header_value(value) {
                        for (k, v) in cookies {
                            result.res_cookies.push((k, v));
                        }
                    }
                }
                Protocol::StatusCode => {
                    if let Ok(code) = value.parse::<u16>() {
                        result.status_code = Some(code);
                    }
                }
                Protocol::Method => {
                    result.method = Some(value.to_string());
                }
                Protocol::Ua => {
                    result.ua = Some(value.to_string());
                }
                Protocol::Referer => {
                    result.referer = Some(value.to_string());
                }
                Protocol::ResCors => {
                    result.enable_cors = true;
                }
                Protocol::Proxy => {
                    result.proxy = Some(value.to_string());
                }
                _ => {}
            }
        }

        result
    }
}

fn parse_header_value(value: &str) -> Option<Vec<(String, String)>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let content = if trimmed.starts_with('{') && trimmed.ends_with('}') {
        &trimmed[1..trimmed.len()-1]
    } else {
        trimmed
    };

    let mut headers = Vec::new();
    for part in content.split(',') {
        let part = part.trim();
        if let Some(pos) = part.find(':') {
            let key = part[..pos].trim().to_string();
            let val = part[pos+1..].trim().to_string();
            if !key.is_empty() {
                headers.push((key, val));
            }
        }
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

pub struct ProxyInstance {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ProxyInstance {
    pub async fn start(port: u16, rules: Vec<&str>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let parsed_rules: Vec<Rule> = rules
            .iter()
            .filter_map(|r| parse_rules(r).ok())
            .flatten()
            .collect();

        let resolver = Arc::new(RulesResolverAdapter {
            inner: CoreRulesResolver::new(parsed_rules),
        });
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let config = ProxyConfig {
            port,
            host: "127.0.0.1".to_string(),
            enable_tls_interception: false,
            timeout_secs: 30,
            socks5_port: None,
            socks5_auth_required: false,
            socks5_username: None,
            socks5_password: None,
        };

        let server = ProxyServer::new(config).with_rules(resolver);

        tokio::spawn(async move {
            tokio::select! {
                result = server.run() => {
                    if let Err(e) = result {
                        tracing::error!("Proxy server error: {}", e);
                    }
                }
                _ = shutdown_rx => {
                    tracing::info!("Proxy server shutting down");
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn proxy_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for ProxyInstance {
    fn drop(&mut self) {
        self.shutdown();
    }
}

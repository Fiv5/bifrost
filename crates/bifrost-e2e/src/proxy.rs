use bifrost_admin::{AdminState, ConnectionRegistry, RuntimeConfig};
use bifrost_core::{
    parse_rules, Protocol, RequestContext, Rule, RuleParser, RulesResolver as CoreRulesResolver,
};
use bifrost_proxy::{
    ProxyConfig, ProxyServer, ResolvedRules as ProxyResolvedRules, RuleValue,
    RulesResolver as ProxyRulesResolverTrait, TlsConfig,
};
use bifrost_tls::{generate_root_ca, init_crypto_provider, DynamicCertGenerator};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

struct RulesResolverAdapter {
    inner: CoreRulesResolver,
}

impl ProxyRulesResolverTrait for RulesResolverAdapter {
    fn resolve_with_context(
        &self,
        url: &str,
        method: &str,
        _req_headers: &std::collections::HashMap<String, String>,
        _req_cookies: &std::collections::HashMap<String, String>,
    ) -> ProxyResolvedRules {
        let mut ctx = RequestContext::from_url(url);
        ctx.method = method.to_string();

        let core_result = self.inner.resolve(&ctx);
        let mut result = ProxyResolvedRules::default();

        tracing::debug!("Resolving rules for URL: {}", url);
        tracing::debug!("Found {} rules", core_result.rules.len());

        for resolved_rule in &core_result.rules {
            let protocol = resolved_rule.rule.protocol;
            let value = &resolved_rule.resolved_value;
            let pattern = &resolved_rule.rule.pattern;
            tracing::debug!("Processing rule: {:?} = {}", protocol, value);

            result.rules.push(RuleValue {
                pattern: pattern.clone(),
                protocol,
                value: value.clone(),
                options: std::collections::HashMap::new(),
                rule_name: resolved_rule.rule.file.clone(),
                raw: Some(resolved_rule.rule.raw.clone()),
                line: resolved_rule.rule.line,
            });

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
                Protocol::ReplaceStatus => {
                    if let Ok(code) = value.parse::<u16>() {
                        result.replace_status = Some(code);
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
                Protocol::Ignore => {
                    result.ignored = true;
                }
                Protocol::ReqPrepend => {
                    result.req_prepend = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqAppend => {
                    result.req_append = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ResPrepend => {
                    result.res_prepend = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ResAppend => {
                    result.res_append = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqBody => {
                    result.req_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ResBody => {
                    result.res_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqReplace => {
                    let parsed = parse_replace_value(value);
                    result.req_replace.extend(parsed.string_rules);
                    result.req_replace_regex.extend(parsed.regex_rules);
                }
                Protocol::ResReplace => {
                    let parsed = parse_replace_value(value);
                    result.res_replace.extend(parsed.string_rules);
                    result.res_replace_regex.extend(parsed.regex_rules);
                }
                Protocol::Params => {
                    if let Ok(json_value) = serde_json::from_str(value) {
                        result.req_merge = Some(json_value);
                    }
                }
                Protocol::ResMerge => {
                    if let Ok(json_value) = serde_json::from_str(value) {
                        result.res_merge = Some(json_value);
                    }
                }
                Protocol::UrlParams => {
                    if let Some(params) = parse_header_value(value) {
                        for (k, v) in params {
                            result.url_params.push((k, v));
                        }
                    }
                }
                Protocol::UrlReplace => {
                    let parsed = parse_replace_value(value);
                    result.url_replace.extend(parsed.string_rules);
                }
                Protocol::ForwardedFor => {
                    result.forwarded_for = Some(value.to_string());
                }
                Protocol::ReqType => {
                    result.req_type = Some(value.to_string());
                }
                Protocol::ReqCharset => {
                    result.req_charset = Some(value.to_string());
                }
                Protocol::ResType => {
                    result.res_type = Some(value.to_string());
                }
                Protocol::ResCharset => {
                    result.res_charset = Some(value.to_string());
                }
                Protocol::Cache => {
                    result.cache = Some(value.to_string());
                }
                Protocol::Attachment => {
                    result.attachment = Some(value.to_string());
                }
                Protocol::HtmlAppend => {
                    result.html_append = Some(value.to_string());
                }
                Protocol::HtmlPrepend => {
                    result.html_prepend = Some(value.to_string());
                }
                Protocol::HtmlBody => {
                    result.html_body = Some(value.to_string());
                }
                Protocol::JsAppend => {
                    result.js_append = Some(value.to_string());
                }
                Protocol::JsPrepend => {
                    result.js_prepend = Some(value.to_string());
                }
                Protocol::JsBody => {
                    result.js_body = Some(value.to_string());
                }
                Protocol::CssAppend => {
                    result.css_append = Some(value.to_string());
                }
                Protocol::CssPrepend => {
                    result.css_prepend = Some(value.to_string());
                }
                Protocol::CssBody => {
                    result.css_body = Some(value.to_string());
                }
                Protocol::ReqSpeed => {
                    if let Ok(speed) = value.parse::<u64>() {
                        result.req_speed = Some(speed);
                    }
                }
                Protocol::ResSpeed => {
                    if let Ok(speed) = value.parse::<u64>() {
                        result.res_speed = Some(speed);
                    }
                }
                Protocol::Redirect => {
                    result.redirect = Some(value.to_string());
                }
                Protocol::File => {
                    result.mock_file = Some(value.to_string());
                }
                Protocol::Dns => {
                    result.dns_servers.push(value.to_string());
                }
                Protocol::XHost => {
                    result.host = Some(value.to_string());
                }
                Protocol::Http => {
                    result.host = Some(value.to_string());
                    result.host_protocol = Some(Protocol::Http);
                }
                Protocol::Https => {
                    result.host = Some(value.to_string());
                    result.host_protocol = Some(Protocol::Https);
                }
                Protocol::Ws => {
                    result.host = Some(value.to_string());
                    result.host_protocol = Some(Protocol::Ws);
                }
                Protocol::Wss => {
                    result.host = Some(value.to_string());
                    result.host_protocol = Some(Protocol::Wss);
                }
                Protocol::TlsIntercept => {
                    result.tls_intercept = Some(true);
                }
                Protocol::TlsPassthrough => {
                    result.tls_intercept = Some(false);
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

    let (content, use_colon) = if trimmed.starts_with('(') && trimmed.ends_with(')') {
        (&trimmed[1..trimmed.len() - 1], true)
    } else {
        (trimmed, false)
    };

    let mut headers = Vec::new();

    let is_multiline = content.contains('\n');
    let parts: Vec<&str> = if is_multiline {
        content.lines().collect()
    } else {
        content.split(',').collect()
    };

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let separator = if use_colon || is_multiline { ':' } else { '=' };
        if let Some(pos) = part.find(separator) {
            let key = part[..pos].trim().to_string();
            let val = part[pos + 1..].trim().to_string();
            if !key.is_empty() {
                headers.push((key, val));
            }
        } else if !use_colon && !is_multiline {
            if let Some(pos) = part.find('=') {
                let key = part[..pos].trim().to_string();
                let val = part[pos + 1..].trim().to_string();
                if !key.is_empty() {
                    headers.push((key, val));
                }
            }
        }
    }

    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

fn url_decode(s: &str) -> String {
    urlencoding::decode(s)
        .unwrap_or(std::borrow::Cow::Borrowed(s))
        .into_owned()
}

struct ParsedReplaceRules {
    string_rules: Vec<(String, String)>,
    regex_rules: Vec<bifrost_proxy::RegexReplace>,
}

fn parse_regex_pattern(s: &str) -> Option<(regex::Regex, bool)> {
    let s = s.trim();
    if !s.starts_with('/') {
        return None;
    }

    let global = s.ends_with("/g") || s.ends_with("/gi") || s.ends_with("/ig");
    let case_insensitive = s.ends_with("/i") || s.ends_with("/gi") || s.ends_with("/ig");

    let end_pos = if global && case_insensitive {
        s.len() - 3
    } else if global || case_insensitive {
        s.len() - 2
    } else if s.len() > 1 && s.ends_with('/') {
        s.len() - 1
    } else {
        return None;
    };

    let pattern_str = &s[1..end_pos];
    if pattern_str.is_empty() {
        return None;
    }

    let regex_result = if case_insensitive {
        regex::RegexBuilder::new(pattern_str)
            .case_insensitive(true)
            .build()
    } else {
        regex::Regex::new(pattern_str)
    };

    match regex_result {
        Ok(re) => Some((re, global)),
        Err(_) => None,
    }
}

fn parse_replace_value(value: &str) -> ParsedReplaceRules {
    let mut string_rules = Vec::new();
    let mut regex_rules = Vec::new();

    for pair in value.split('&') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }

        if let Some((from, to)) = pair.split_once('=') {
            let from = url_decode(from);
            let to = url_decode(to);

            if let Some((regex, global)) = parse_regex_pattern(&from) {
                regex_rules.push(bifrost_proxy::RegexReplace {
                    pattern: regex,
                    replacement: to,
                    global,
                });
            } else {
                string_rules.push((from, to));
            }
        } else {
            let from = url_decode(pair);
            if let Some((regex, global)) = parse_regex_pattern(&from) {
                regex_rules.push(bifrost_proxy::RegexReplace {
                    pattern: regex,
                    replacement: String::new(),
                    global,
                });
            } else {
                string_rules.push((from, String::new()));
            }
        }
    }

    ParsedReplaceRules {
        string_rules,
        regex_rules,
    }
}

pub struct ProxyInstance {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ProxyInstance {
    pub async fn start(
        port: u16,
        rules: Vec<&str>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::start_with_values(port, rules, HashMap::new()).await
    }

    pub async fn start_with_values(
        port: u16,
        rules: Vec<&str>,
        values: HashMap<String, String>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let parsed_rules: Vec<Rule> = rules
            .iter()
            .filter_map(|r| parse_rules(r).ok())
            .flatten()
            .collect();

        let resolver = Arc::new(RulesResolverAdapter {
            inner: CoreRulesResolver::new(parsed_rules).with_values(values),
        });
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let config = ProxyConfig {
            port,
            host: "127.0.0.1".to_string(),
            enable_tls_interception: false,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            timeout_secs: 30,
            socks5_port: None,
            socks5_auth_required: false,
            socks5_username: None,
            socks5_password: None,
            verbose_logging: true,
            access_mode: bifrost_proxy::AccessMode::AllowAll,
            client_whitelist: Vec::new(),
            allow_lan: true,
            unsafe_ssl: false,
            max_body_buffer_size: 32 * 1024 * 1024,
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

    pub async fn start_with_rules_text(
        port: u16,
        rules_text: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let parser = RuleParser::new();
        let (rules, inline_values) = parser
            .parse_rules_with_inline_values(rules_text)
            .map_err(|e| format!("Failed to parse rules: {}", e))?;

        let resolver = Arc::new(RulesResolverAdapter {
            inner: CoreRulesResolver::new(rules).with_values(inline_values),
        });
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let config = ProxyConfig {
            port,
            host: "127.0.0.1".to_string(),
            enable_tls_interception: false,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            timeout_secs: 30,
            socks5_port: None,
            socks5_auth_required: false,
            socks5_username: None,
            socks5_password: None,
            verbose_logging: true,
            access_mode: bifrost_proxy::AccessMode::AllowAll,
            client_whitelist: Vec::new(),
            allow_lan: true,
            unsafe_ssl: false,
            max_body_buffer_size: 32 * 1024 * 1024,
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

    #[allow(clippy::too_many_arguments)]
    pub async fn start_with_admin(
        port: u16,
        rules: Vec<&str>,
        enable_tls_interception: bool,
        unsafe_ssl: bool,
    ) -> Result<(Self, Arc<AdminState>), Box<dyn std::error::Error + Send + Sync>> {
        let parsed_rules: Vec<Rule> = rules
            .iter()
            .filter_map(|r| parse_rules(r).ok())
            .flatten()
            .collect();

        let resolver = Arc::new(RulesResolverAdapter {
            inner: CoreRulesResolver::new(parsed_rules).with_values(HashMap::new()),
        });
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let config = ProxyConfig {
            port,
            host: "127.0.0.1".to_string(),
            enable_tls_interception,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            timeout_secs: 30,
            socks5_port: None,
            socks5_auth_required: false,
            socks5_username: None,
            socks5_password: None,
            verbose_logging: true,
            access_mode: bifrost_proxy::AccessMode::AllowAll,
            client_whitelist: Vec::new(),
            allow_lan: true,
            unsafe_ssl,
            max_body_buffer_size: 32 * 1024 * 1024,
        };

        let tls_config = if enable_tls_interception {
            init_crypto_provider();
            let ca = generate_root_ca().map_err(|e| format!("Failed to generate CA: {}", e))?;
            let ca_cert = ca
                .certificate_der()
                .map_err(|e| format!("Failed to get CA cert: {}", e))?;
            let ca_key = ca.private_key_der();
            let cert_generator = Arc::new(DynamicCertGenerator::new(Arc::new(ca)));
            Arc::new(TlsConfig {
                ca_cert: Some(ca_cert.to_vec()),
                ca_key: Some(ca_key.secret_der().to_vec()),
                cert_generator: Some(cert_generator),
                sni_resolver: None,
            })
        } else {
            Arc::new(TlsConfig::default())
        };

        let runtime_config = RuntimeConfig {
            enable_tls_interception,
            intercept_exclude: Vec::new(),
            intercept_include: Vec::new(),
            unsafe_ssl,
            disconnect_on_config_change: true,
        };

        let connection_registry = ConnectionRegistry::new(true);

        let admin_state = AdminState::new(port)
            .with_runtime_config(runtime_config)
            .with_connection_registry(connection_registry);

        let admin_state_arc = Arc::new(admin_state);

        let server = ProxyServer::new(config)
            .with_rules(resolver)
            .with_tls_config(tls_config)
            .with_admin_state_shared(admin_state_arc.clone());

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

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        Ok((
            Self {
                addr,
                shutdown_tx: Some(shutdown_tx),
            },
            admin_state_arc,
        ))
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

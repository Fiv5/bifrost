use std::collections::HashMap;
use std::path::PathBuf;

use bifrost_core::{Protocol, RequestContext, Rule, RulesResolver as CoreRulesResolver};
use bifrost_proxy::{
    ResolvedRules as ProxyResolvedRules, RuleValue, RulesResolver as ProxyRulesResolverTrait,
};

use super::{parse_cors_config, parse_header_value, parse_replace_value, parse_res_cookies_value};

pub fn parse_cli_rules(
    rules: &[String],
    rules_file: &Option<PathBuf>,
    values: &HashMap<String, String>,
) -> bifrost_core::Result<(Vec<Rule>, HashMap<String, String>)> {
    let mut all_rules = Vec::new();
    let mut merged_values = values.clone();

    let parser = bifrost_core::RuleParser::with_values(values.clone());

    for rule_str in rules {
        match parser.parse_rules(rule_str) {
            Ok(parsed) => all_rules.extend(parsed),
            Err(e) => {
                return Err(bifrost_core::BifrostError::Config(format!(
                    "Failed to parse rule '{}': {}",
                    rule_str, e
                )));
            }
        }
    }

    if let Some(file_path) = rules_file {
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            bifrost_core::BifrostError::Config(format!(
                "Failed to read rules file '{}': {}",
                file_path.display(),
                e
            ))
        })?;
        let parser_with_file = bifrost_core::RuleParser::with_values(merged_values.clone());
        match parser_with_file.parse_rules_with_inline_values(&content) {
            Ok((parsed, inline_values)) => {
                all_rules.extend(parsed);
                for (k, v) in inline_values {
                    merged_values.entry(k).or_insert(v);
                }
            }
            Err(e) => {
                return Err(bifrost_core::BifrostError::Config(format!(
                    "Failed to parse rules file '{}': {}",
                    file_path.display(),
                    e
                )));
            }
        }
    }

    Ok((all_rules, merged_values))
}

pub struct RulesResolverAdapter {
    pub inner: CoreRulesResolver,
}

impl ProxyRulesResolverTrait for RulesResolverAdapter {
    fn resolve_with_context(
        &self,
        url: &str,
        method: &str,
        req_headers: &std::collections::HashMap<String, String>,
        req_cookies: &std::collections::HashMap<String, String>,
    ) -> ProxyResolvedRules {
        let mut ctx = RequestContext::from_url(url);
        ctx.method = method.to_string();
        ctx.client_ip = "127.0.0.1".to_string();
        ctx.req_headers = req_headers.clone();
        ctx.req_cookies = req_cookies.clone();

        tracing::debug!(
            target: "bifrost_proxy::rules",
            url = %url,
            method = %method,
            host = %ctx.host,
            path = %ctx.path,
            "resolving rules for request"
        );

        let core_result = self.inner.resolve(&ctx);

        if core_result.rules.is_empty() {
            tracing::debug!(
                target: "bifrost_proxy::rules",
                url = %url,
                "no rules matched"
            );
        } else {
            tracing::info!(
                target: "bifrost_proxy::rules",
                url = %url,
                matched_count = core_result.rules.len(),
                "rules matched for request"
            );
            for (idx, resolved) in core_result.rules.iter().enumerate() {
                let rule = &resolved.rule;
                tracing::info!(
                    target: "bifrost_proxy::rules",
                    rule_index = idx + 1,
                    pattern = %rule.pattern,
                    protocol = %rule.protocol.to_str(),
                    value = %resolved.resolved_value,
                    raw = %rule.raw,
                    file = rule.file.as_deref().unwrap_or("<cli>"),
                    line = rule.line.unwrap_or(0),
                    disabled = rule.is_disabled(),
                    "matched rule detail"
                );
            }
        }

        let mut result = ProxyResolvedRules::default();

        for resolved_rule in &core_result.rules {
            let protocol = resolved_rule.rule.protocol;
            let value = &resolved_rule.resolved_value;
            let pattern = &resolved_rule.rule.pattern;

            result.rules.push(RuleValue {
                pattern: pattern.clone(),
                protocol,
                value: value.clone(),
                options: HashMap::new(),
                rule_name: resolved_rule.rule.file.clone(),
                raw: Some(resolved_rule.rule.raw.clone()),
                line: resolved_rule.rule.line,
            });

            match protocol {
                Protocol::Host
                | Protocol::XHost
                | Protocol::Http
                | Protocol::Https
                | Protocol::Ws
                | Protocol::Wss => {
                    result.host = Some(value.to_string());
                    result.host_protocol = Some(protocol);
                }
                Protocol::Redirect => {
                    result.redirect = Some(value.to_string());
                }
                Protocol::ReqHeaders => {
                    if let Some(headers) = parse_header_value(value) {
                        for (k, v) in headers {
                            let key_lower = k.to_lowercase();
                            if !result
                                .req_headers
                                .iter()
                                .any(|(existing, _)| existing.to_lowercase() == key_lower)
                            {
                                result.req_headers.push((k, v));
                            }
                        }
                    }
                }
                Protocol::ResHeaders => {
                    if let Some(headers) = parse_header_value(value) {
                        for (k, v) in headers {
                            let key_lower = k.to_lowercase();
                            if !result
                                .res_headers
                                .iter()
                                .any(|(existing, _)| existing.to_lowercase() == key_lower)
                            {
                                result.res_headers.push((k, v));
                            }
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
                Protocol::ResBody => {
                    result.res_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::ReqBody => {
                    result.req_body = Some(bytes::Bytes::from(value.to_string()));
                }
                Protocol::Proxy => {
                    result.proxy = Some(value.to_string());
                }
                Protocol::Ignore => {
                    result.ignored = true;
                }
                Protocol::ReqCors => {
                    let cors = parse_cors_config(value);
                    result.req_cors = cors;
                }
                Protocol::ResCors => {
                    let cors = parse_cors_config(value);
                    result.res_cors = cors;
                }
                Protocol::File => {
                    result.mock_file = Some(value.to_string());
                }
                Protocol::Tpl => {
                    result.mock_template = Some(value.to_string());
                }
                Protocol::RawFile => {
                    result.mock_rawfile = Some(value.to_string());
                }
                Protocol::Ua => {
                    result.ua = Some(value.to_string());
                }
                Protocol::Referer => {
                    result.referer = Some(value.to_string());
                }
                Protocol::Method => {
                    result.method = Some(value.to_string());
                }
                Protocol::ReqDelay => {
                    if let Ok(delay) = value.parse::<u64>() {
                        result.req_delay = Some(delay);
                    }
                }
                Protocol::ResDelay => {
                    if let Ok(delay) = value.parse::<u64>() {
                        result.res_delay = Some(delay);
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
                    let parsed_cookies = parse_res_cookies_value(value);
                    result.res_cookies.extend(parsed_cookies);
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
                Protocol::Dns => {
                    result.dns_servers.push(value.to_string());
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

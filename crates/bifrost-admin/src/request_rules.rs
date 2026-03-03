use std::collections::HashMap;

use base64::Engine;
use bifrost_core::{Protocol, ResolvedRules as CoreResolvedRules};
use bytes::Bytes;
use tracing::info;
use url::Url;

#[derive(Debug, Clone, Default)]
pub struct AppliedRules {
    pub forward_url: Option<String>,
    pub host: Option<String>,
    pub method: Option<String>,
    pub ua: Option<String>,
    pub referer: Option<String>,
    pub auth: Option<String>,
    pub req_headers: Vec<(String, String)>,
    pub req_cookies: Vec<(String, String)>,
    pub req_del_cookies: Vec<String>,
    pub delete_req_headers: Vec<String>,
    pub url_params: Vec<(String, String)>,
    pub url_replace: Vec<(String, String)>,
    pub req_body: Option<Bytes>,
    pub req_prepend: Option<Bytes>,
    pub req_append: Option<Bytes>,
    pub req_replace: Vec<(String, String)>,
}

pub struct AppliedRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Bytes>,
}

pub fn build_applied_rules(core_rules: &CoreResolvedRules) -> AppliedRules {
    let mut applied = AppliedRules::default();

    for rule in &core_rules.rules {
        match rule.rule.protocol {
            Protocol::Http | Protocol::Https | Protocol::Ws | Protocol::Wss => {
                if applied.forward_url.is_none() {
                    let scheme = match rule.rule.protocol {
                        Protocol::Http => "http",
                        Protocol::Https => "https",
                        Protocol::Ws => "ws",
                        Protocol::Wss => "wss",
                        _ => continue,
                    };
                    let value = rule.resolved_value.trim_end_matches('/');
                    let forward_url = if value.contains("://") {
                        value.to_string()
                    } else {
                        format!("{}://{}", scheme, value)
                    };
                    applied.forward_url = Some(forward_url);
                }
            }
            Protocol::Host | Protocol::XHost => {
                if applied.host.is_none() {
                    applied.host = Some(rule.resolved_value.clone());
                }
            }
            Protocol::Method => {
                if applied.method.is_none() {
                    applied.method = Some(rule.resolved_value.clone());
                }
            }
            Protocol::Ua => {
                if applied.ua.is_none() {
                    applied.ua = Some(rule.resolved_value.clone());
                }
            }
            Protocol::Referer => {
                if applied.referer.is_none() {
                    applied.referer = Some(rule.resolved_value.clone());
                }
            }
            Protocol::Auth => {
                if applied.auth.is_none() {
                    applied.auth = Some(rule.resolved_value.clone());
                }
            }
            Protocol::ReqHeaders => {
                if let Some((key, value)) = parse_header_value(&rule.resolved_value) {
                    applied.req_headers.push((key, value));
                }
            }
            Protocol::ReqCookies => {
                if let Some((key, value)) = parse_cookie_value(&rule.resolved_value) {
                    applied.req_cookies.push((key, value));
                }
            }
            Protocol::Delete => {
                let headers = parse_delete_headers(&rule.resolved_value);
                applied.delete_req_headers.extend(headers);
            }
            Protocol::UrlParams => {
                if let Some((key, value)) = parse_url_param(&rule.resolved_value) {
                    applied.url_params.push((key, value));
                }
            }
            Protocol::UrlReplace => {
                if let Some((from, to)) = parse_url_replace(&rule.resolved_value) {
                    applied.url_replace.push((from, to));
                }
            }
            Protocol::ReqBody => {
                if applied.req_body.is_none() {
                    let content = extract_inline_content(&rule.resolved_value);
                    applied.req_body = Some(Bytes::from(content));
                }
            }
            Protocol::ReqPrepend => {
                if applied.req_prepend.is_none() {
                    let content = extract_inline_content(&rule.resolved_value);
                    applied.req_prepend = Some(Bytes::from(content));
                }
            }
            Protocol::ReqAppend => {
                if applied.req_append.is_none() {
                    let content = extract_inline_content(&rule.resolved_value);
                    applied.req_append = Some(Bytes::from(content));
                }
            }
            Protocol::ReqReplace => {
                if let Some((from, to)) = parse_replace_value(&rule.resolved_value) {
                    applied.req_replace.push((from, to));
                }
            }
            _ => {}
        }
    }

    applied
}

pub fn apply_host_rule(original_url: &str, host: Option<&str>) -> Result<String, String> {
    let Some(new_host) = host else {
        return Ok(original_url.to_string());
    };

    let mut url = Url::parse(original_url).map_err(|e| format!("Invalid URL: {}", e))?;

    if let Some((host_part, port_part)) = new_host.split_once(':') {
        url.set_host(Some(host_part)).map_err(|e| e.to_string())?;
        if let Ok(port) = port_part.parse::<u16>() {
            url.set_port(Some(port)).map_err(|_| "Invalid port")?;
        }
    } else {
        url.set_host(Some(new_host)).map_err(|e| e.to_string())?;
        let _ = url.set_port(None);
    }

    Ok(url.to_string())
}

pub fn apply_url_rules(url: &str, rules: &AppliedRules) -> String {
    let mut result = url.to_string();

    for (from, to) in &rules.url_replace {
        result = result.replace(from, to);
    }

    if !rules.url_params.is_empty() {
        let mut parsed = match Url::parse(&result) {
            Ok(u) => u,
            Err(_) => return result,
        };

        let mut query_pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        for (key, value) in &rules.url_params {
            if let Some(existing) = query_pairs.iter_mut().find(|(k, _)| k == key) {
                existing.1 = value.clone();
            } else {
                query_pairs.push((key.clone(), value.clone()));
            }
        }

        if query_pairs.is_empty() {
            parsed.set_query(None);
        } else {
            let query_string: String = query_pairs
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            parsed.set_query(Some(&query_string));
        }

        result = parsed.to_string();
    }

    result
}

pub fn apply_all_request_rules(
    original_url: &str,
    original_method: &str,
    original_headers: &[(String, String)],
    original_body: Option<&[u8]>,
    applied_rules: &AppliedRules,
    verbose_logging: bool,
) -> Result<AppliedRequest, String> {
    let final_method = applied_rules
        .method
        .clone()
        .unwrap_or_else(|| original_method.to_string());

    if verbose_logging && applied_rules.method.is_some() {
        info!(
            "[REPLAY_RULES] Method: {} -> {}",
            original_method, final_method
        );
    }

    let base_url = if let Some(ref forward_url) = applied_rules.forward_url {
        let original_parsed =
            Url::parse(original_url).map_err(|e| format!("Invalid original URL: {}", e))?;
        let forward_parsed =
            Url::parse(forward_url).map_err(|e| format!("Invalid forward URL: {}", e))?;

        let mut new_url = forward_parsed.clone();
        let original_path = original_parsed.path();
        let forward_path = forward_parsed.path().trim_end_matches('/');
        let combined_path = if forward_path.is_empty() {
            original_path.to_string()
        } else if original_path == "/" {
            forward_path.to_string()
        } else {
            format!("{}{}", forward_path, original_path)
        };
        new_url.set_path(&combined_path);
        new_url.set_query(original_parsed.query());

        if verbose_logging {
            info!(
                "[REPLAY_RULES] Forward URL: {} -> {}",
                original_url, new_url
            );
        }
        new_url.to_string()
    } else {
        original_url.to_string()
    };

    let url_after_host = apply_host_rule(&base_url, applied_rules.host.as_deref())?;

    if verbose_logging && applied_rules.host.is_some() {
        info!("[REPLAY_RULES] Host: {} -> {}", base_url, url_after_host);
    }

    let final_url = apply_url_rules(&url_after_host, applied_rules);

    if verbose_logging && final_url != url_after_host {
        info!("[REPLAY_RULES] URL: {} -> {}", url_after_host, final_url);
    }

    let mut final_headers = apply_header_rules(original_headers, applied_rules, verbose_logging);

    if let Some(ref ua) = applied_rules.ua {
        apply_single_header(&mut final_headers, "user-agent", ua, verbose_logging);
    }

    if let Some(ref referer) = applied_rules.referer {
        apply_single_header(&mut final_headers, "referer", referer, verbose_logging);
    }

    if let Some(ref auth) = applied_rules.auth {
        let encoded = base64::engine::general_purpose::STANDARD.encode(auth);
        let header_value = format!("Basic {}", encoded);
        apply_single_header(
            &mut final_headers,
            "authorization",
            &header_value,
            verbose_logging,
        );
    }

    let final_body = apply_body_rules(original_body, applied_rules, verbose_logging);

    Ok(AppliedRequest {
        url: final_url,
        method: final_method,
        headers: final_headers,
        body: final_body,
    })
}

fn apply_header_rules(
    original_headers: &[(String, String)],
    rules: &AppliedRules,
    verbose_logging: bool,
) -> Vec<(String, String)> {
    let mut headers: Vec<(String, String)> = original_headers.to_vec();

    for header_name in &rules.delete_req_headers {
        let lower_name = header_name.to_lowercase();
        let before_len = headers.len();
        headers.retain(|(k, _)| k.to_lowercase() != lower_name);
        if verbose_logging && headers.len() < before_len {
            info!("[REPLAY_RULES] Deleted header: {}", header_name);
        }
    }

    for (name, value) in &rules.req_headers {
        let lower_name = name.to_lowercase();
        let mut found = false;
        for (k, v) in headers.iter_mut() {
            if k.to_lowercase() == lower_name {
                if verbose_logging {
                    info!(
                        "[REPLAY_RULES] Header {} : \"{}\" -> \"{}\"",
                        name, v, value
                    );
                }
                *v = value.clone();
                found = true;
                break;
            }
        }
        if !found {
            if verbose_logging {
                info!("[REPLAY_RULES] Header {} : (none) -> \"{}\"", name, value);
            }
            headers.push((name.clone(), value.clone()));
        }
    }

    if !rules.req_cookies.is_empty() || !rules.req_del_cookies.is_empty() {
        apply_cookie_rules(&mut headers, rules, verbose_logging);
    }

    headers
}

fn apply_single_header(
    headers: &mut Vec<(String, String)>,
    header_name: &str,
    value: &str,
    verbose_logging: bool,
) {
    let lower_name = header_name.to_lowercase();
    for (k, v) in headers.iter_mut() {
        if k.to_lowercase() == lower_name {
            if verbose_logging {
                info!(
                    "[REPLAY_RULES] {} : \"{}\" -> \"{}\"",
                    header_name, v, value
                );
            }
            *v = value.to_string();
            return;
        }
    }
    if verbose_logging {
        info!("[REPLAY_RULES] {} : (none) -> \"{}\"", header_name, value);
    }
    headers.push((header_name.to_string(), value.to_string()));
}

fn apply_cookie_rules(
    headers: &mut Vec<(String, String)>,
    rules: &AppliedRules,
    verbose_logging: bool,
) {
    let cookie_header = headers
        .iter()
        .find(|(k, _)| k.to_lowercase() == "cookie")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();

    let mut cookies: HashMap<String, String> = cookie_header
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
        .collect();

    for del_name in &rules.req_del_cookies {
        if cookies.remove(del_name).is_some() && verbose_logging {
            info!("[REPLAY_RULES] Cookie {} : deleted", del_name);
        }
    }

    for (name, value) in &rules.req_cookies {
        let old_value = cookies.insert(name.clone(), value.clone());
        if verbose_logging {
            if let Some(old) = old_value {
                info!(
                    "[REPLAY_RULES] Cookie {} : \"{}\" -> \"{}\"",
                    name, old, value
                );
            } else {
                info!("[REPLAY_RULES] Cookie {} : (none) -> \"{}\"", name, value);
            }
        }
    }

    let new_cookie_value: String = cookies
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("; ");

    headers.retain(|(k, _)| k.to_lowercase() != "cookie");

    if !new_cookie_value.is_empty() {
        headers.push(("Cookie".to_string(), new_cookie_value));
    }
}

fn apply_body_rules(
    original_body: Option<&[u8]>,
    rules: &AppliedRules,
    verbose_logging: bool,
) -> Option<Bytes> {
    if rules.req_body.is_some() {
        if verbose_logging {
            info!("[REPLAY_RULES] Request body replaced by rule");
        }
        return rules.req_body.clone();
    }

    let mut body = original_body.map(|b| b.to_vec()).unwrap_or_default();

    if !rules.req_replace.is_empty() {
        let mut body_str = String::from_utf8_lossy(&body).to_string();
        for (from, to) in &rules.req_replace {
            if body_str.contains(from) {
                body_str = body_str.replace(from, to);
                if verbose_logging {
                    info!("[REPLAY_RULES] Body replace: \"{}\" -> \"{}\"", from, to);
                }
            }
        }
        body = body_str.into_bytes();
    }

    if let Some(ref prepend) = rules.req_prepend {
        if verbose_logging {
            info!("[REPLAY_RULES] Prepending {} bytes to body", prepend.len());
        }
        let mut new_body = prepend.to_vec();
        new_body.extend_from_slice(&body);
        body = new_body;
    }

    if let Some(ref append) = rules.req_append {
        if verbose_logging {
            info!("[REPLAY_RULES] Appending {} bytes to body", append.len());
        }
        body.extend_from_slice(append);
    }

    if body.is_empty() {
        None
    } else {
        Some(Bytes::from(body))
    }
}

fn parse_header_value(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();

    if let Some(pos) = trimmed.find(':') {
        let key = trimmed[..pos].trim().to_string();
        let val = trimmed[pos + 1..].trim().to_string();
        if !key.is_empty() {
            return Some((key, val));
        }
    }

    if let Some(pos) = trimmed.find('=') {
        let key = trimmed[..pos].trim().to_string();
        let val = trimmed[pos + 1..].trim().to_string();
        if !key.is_empty() {
            return Some((key, val));
        }
    }

    None
}

fn parse_cookie_value(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();

    if let Some(pos) = trimmed.find('=') {
        let key = trimmed[..pos].trim().to_string();
        let val = trimmed[pos + 1..].trim().to_string();
        if !key.is_empty() {
            return Some((key, val));
        }
    }

    None
}

fn parse_delete_headers(value: &str) -> Vec<String> {
    value
        .split([',', '\n'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_url_param(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    if let Some(pos) = trimmed.find('=') {
        let key = trimmed[..pos].trim().to_string();
        let val = trimmed[pos + 1..].trim().to_string();
        if !key.is_empty() {
            return Some((key, val));
        }
    }
    None
}

fn parse_url_replace(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    if parts.len() == 2 {
        return Some((parts[0].to_string(), parts[1].to_string()));
    }
    if let Some(pos) = trimmed.find('=') {
        let from = trimmed[..pos].trim().to_string();
        let to = trimmed[pos + 1..].trim().to_string();
        if !from.is_empty() {
            return Some((from, to));
        }
    }
    None
}

fn parse_replace_value(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    if parts.len() == 2 {
        return Some((parts[0].to_string(), parts[1].to_string()));
    }
    None
}

fn extract_inline_content(value: &str) -> String {
    if value.starts_with('{') && value.ends_with('}') && value.len() > 1 {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_host_rule_basic() {
        let result = apply_host_rule("http://example.com/path", Some("newhost.com")).unwrap();
        assert_eq!(result, "http://newhost.com/path");
    }

    #[test]
    fn test_apply_host_rule_with_port() {
        let result = apply_host_rule("http://example.com/path", Some("newhost.com:8080")).unwrap();
        assert_eq!(result, "http://newhost.com:8080/path");
    }

    #[test]
    fn test_apply_host_rule_https() {
        let result = apply_host_rule("https://example.com/path", Some("newhost.com")).unwrap();
        assert_eq!(result, "https://newhost.com/path");
    }

    #[test]
    fn test_apply_host_rule_none() {
        let result = apply_host_rule("http://example.com/path", None).unwrap();
        assert_eq!(result, "http://example.com/path");
    }

    #[test]
    fn test_apply_url_rules_replace() {
        let rules = AppliedRules {
            url_replace: vec![("/old/".to_string(), "/new/".to_string())],
            ..Default::default()
        };
        let result = apply_url_rules("http://example.com/old/path", &rules);
        assert_eq!(result, "http://example.com/new/path");
    }

    #[test]
    fn test_apply_url_rules_params() {
        let rules = AppliedRules {
            url_params: vec![("foo".to_string(), "bar".to_string())],
            ..Default::default()
        };
        let result = apply_url_rules("http://example.com/path", &rules);
        assert!(result.contains("foo=bar"));
    }

    #[test]
    fn test_parse_header_value_colon() {
        let result = parse_header_value("Content-Type: application/json");
        assert_eq!(
            result,
            Some(("Content-Type".to_string(), "application/json".to_string()))
        );
    }

    #[test]
    fn test_parse_header_value_equals() {
        let result = parse_header_value("X-Custom=value");
        assert_eq!(result, Some(("X-Custom".to_string(), "value".to_string())));
    }

    #[test]
    fn test_extract_inline_content() {
        assert_eq!(extract_inline_content("{hello}"), "hello");
        assert_eq!(extract_inline_content("hello"), "hello");
        assert_eq!(extract_inline_content("{}"), "");
    }
}

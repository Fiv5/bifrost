use hyper::Uri;
use tracing::debug;
use url::Url;

use crate::server::ResolvedRules;
use crate::utils::logging::RequestContext;

pub fn apply_url_params(uri: &Uri, rules: &ResolvedRules) -> Uri {
    if rules.url_params.is_empty() && rules.delete_url_params.is_empty() {
        return uri.clone();
    }

    let uri_str = uri.to_string();
    let base_url = if uri_str.starts_with('/') {
        format!("http://localhost{}", uri_str)
    } else if !uri_str.contains("://") {
        format!("http://{}", uri_str)
    } else {
        uri_str
    };

    let Ok(mut url) = Url::parse(&base_url) else {
        return uri.clone();
    };

    let mut query_pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();

    if !rules.delete_url_params.is_empty() {
        query_pairs.retain(|(key, _)| !rules.delete_url_params.iter().any(|delete| delete == key));
    }

    for (key, value) in &rules.url_params {
        query_pairs.retain(|(existing_key, _)| existing_key != key);
        query_pairs.push((key.clone(), value.clone()));
    }

    if query_pairs.is_empty() {
        url.set_query(None);
    } else {
        let query = query_pairs
            .into_iter()
            .map(|(key, value)| {
                format!(
                    "{}={}",
                    urlencoding::encode(&key),
                    urlencoding::encode(&value)
                )
            })
            .collect::<Vec<_>>()
            .join("&");
        url.set_query(Some(&query));
    }

    let new_uri_str = if uri.to_string().starts_with('/') {
        match url.query() {
            Some(query) if !query.is_empty() => format!("{}?{}", url.path(), query),
            _ => url.path().to_string(),
        }
    } else {
        url.to_string()
    };

    new_uri_str.parse().unwrap_or_else(|_| uri.clone())
}

pub fn apply_url_replace(
    uri: &Uri,
    rules: &ResolvedRules,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Uri {
    if rules.url_replace.is_empty() && rules.url_replace_regex.is_empty() {
        return uri.clone();
    }

    let mut path = uri.path().to_string();
    let query = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();

    for (from, to) in &rules.url_replace {
        if path.contains(from.as_str()) {
            path = path.replace(from.as_str(), to.as_str());
            if verbose_logging {
                debug!("[{}] [URL_REPLACE] {} -> {}", ctx.id_str(), from, to);
            }
        }
    }

    for rule in &rules.url_replace_regex {
        let updated = if rule.global {
            rule.pattern
                .replace_all(&path, rule.replacement.as_str())
                .to_string()
        } else {
            rule.pattern
                .replace(&path, rule.replacement.as_str())
                .to_string()
        };

        if updated != path {
            if verbose_logging {
                debug!(
                    "[{}] [URL_REPLACE_REGEX] {} -> {}",
                    ctx.id_str(),
                    rule.pattern.as_str(),
                    rule.replacement
                );
            }
            path = updated;
        }
    }

    let new_uri_str = format!("{}{}", path, query);
    new_uri_str.parse().unwrap_or_else(|_| uri.clone())
}

pub fn apply_url_rules(
    uri: &Uri,
    rules: &ResolvedRules,
    verbose_logging: bool,
    ctx: &RequestContext,
) -> Uri {
    let uri = apply_url_params(uri, rules);
    apply_url_replace(&uri, rules, verbose_logging, ctx)
}

pub fn build_redirect_uri(base_uri: &Uri, redirect_target: &str) -> Option<String> {
    if redirect_target.starts_with("http://") || redirect_target.starts_with("https://") {
        return Some(redirect_target.to_string());
    }

    let scheme = base_uri.scheme_str().unwrap_or("http");
    let host = base_uri.host()?;
    let port = base_uri.port_u16();

    let host_port = if let Some(p) = port {
        if (scheme == "http" && p == 80) || (scheme == "https" && p == 443) {
            host.to_string()
        } else {
            format!("{}:{}", host, p)
        }
    } else {
        host.to_string()
    };

    if redirect_target.starts_with('/') {
        Some(format!("{}://{}{}", scheme, host_port, redirect_target))
    } else {
        let path = base_uri.path();
        let base_path = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        Some(format!(
            "{}://{}{}/{}",
            scheme, host_port, base_path, redirect_target
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_ctx() -> RequestContext {
        RequestContext::new()
    }

    #[test]
    fn test_apply_url_params_empty() {
        let uri: Uri = "/api/test".parse().unwrap();
        let rules = ResolvedRules::default();

        let result = apply_url_params(&uri, &rules);
        assert_eq!(result.to_string(), "/api/test");
    }

    #[test]
    fn test_apply_url_params_add_new() {
        let uri: Uri = "/api/test".parse().unwrap();
        let rules = ResolvedRules {
            url_params: vec![("key".to_string(), "value".to_string())],
            ..Default::default()
        };

        let result = apply_url_params(&uri, &rules);
        assert!(result.to_string().contains("key=value"));
    }

    #[test]
    fn test_apply_url_params_with_existing_query() {
        let uri: Uri = "/api/test?existing=1".parse().unwrap();
        let rules = ResolvedRules {
            url_params: vec![("new".to_string(), "2".to_string())],
            ..Default::default()
        };

        let result = apply_url_params(&uri, &rules);
        let result_str = result.to_string();
        assert!(result_str.contains("existing=1"));
        assert!(result_str.contains("new=2"));
    }

    #[test]
    fn test_apply_url_replace_path() {
        let uri: Uri = "/api/v1/users".parse().unwrap();
        let rules = ResolvedRules {
            url_replace: vec![("/v1/".to_string(), "/v2/".to_string())],
            ..Default::default()
        };

        let result = apply_url_replace(&uri, &rules, false, &mock_ctx());
        assert_eq!(result.path(), "/api/v2/users");
    }

    #[test]
    fn test_apply_url_replace_multiple() {
        let uri: Uri = "/old/path/old".parse().unwrap();
        let rules = ResolvedRules {
            url_replace: vec![("old".to_string(), "new".to_string())],
            ..Default::default()
        };

        let result = apply_url_replace(&uri, &rules, false, &mock_ctx());
        assert_eq!(result.path(), "/new/path/new");
    }

    #[test]
    fn test_apply_url_replace_preserves_query() {
        let uri: Uri = "/api/v1/users?page=1".parse().unwrap();
        let rules = ResolvedRules {
            url_replace: vec![("/v1/".to_string(), "/v2/".to_string())],
            ..Default::default()
        };

        let result = apply_url_replace(&uri, &rules, false, &mock_ctx());
        assert_eq!(result.path(), "/api/v2/users");
        assert_eq!(result.query(), Some("page=1"));
    }

    #[test]
    fn test_build_redirect_uri_absolute() {
        let base: Uri = "http://example.com/path".parse().unwrap();
        let result = build_redirect_uri(&base, "https://other.com/new");
        assert_eq!(result, Some("https://other.com/new".to_string()));
    }

    #[test]
    fn test_build_redirect_uri_absolute_path() {
        let base: Uri = "http://example.com/old/path".parse().unwrap();
        let result = build_redirect_uri(&base, "/new/path");
        assert_eq!(result, Some("http://example.com/new/path".to_string()));
    }

    #[test]
    fn test_build_redirect_uri_relative() {
        let base: Uri = "http://example.com/old/path".parse().unwrap();
        let result = build_redirect_uri(&base, "newfile.html");
        assert_eq!(
            result,
            Some("http://example.com/old/newfile.html".to_string())
        );
    }

    #[test]
    fn test_build_redirect_uri_with_port() {
        let base: Uri = "http://example.com:8080/path".parse().unwrap();
        let result = build_redirect_uri(&base, "/new");
        assert_eq!(result, Some("http://example.com:8080/new".to_string()));
    }

    #[test]
    fn test_build_redirect_uri_default_port() {
        let base: Uri = "http://example.com:80/path".parse().unwrap();
        let result = build_redirect_uri(&base, "/new");
        assert_eq!(result, Some("http://example.com/new".to_string()));
    }
}

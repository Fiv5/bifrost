use crate::protocol::Protocol;
use crate::rule::filter::Filter;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::context::RequestContext;
use super::template::TemplateEngine;
use super::types::Rule;

const DEFAULT_CACHE_CAPACITY: usize = 1000;

#[derive(Debug, Clone)]
pub struct ResolvedRule {
    pub rule: Rule,
    pub captures: Option<Vec<String>>,
    pub resolved_value: String,
}

impl ResolvedRule {
    pub fn new(
        rule: Rule,
        captures: Option<Vec<String>>,
        ctx: &RequestContext,
        values: &HashMap<String, String>,
    ) -> Self {
        let resolved_value =
            TemplateEngine::expand_with_context(&rule.value, ctx, captures.as_deref(), values);

        Self {
            rule,
            captures,
            resolved_value,
        }
    }

    pub fn new_simple(
        rule: Rule,
        captures: Option<Vec<String>>,
        values: &HashMap<String, String>,
    ) -> Self {
        let ctx = RequestContext::new();
        Self::new(rule, captures, &ctx, values)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedRules {
    pub rules: Vec<ResolvedRule>,
    by_protocol: HashMap<Protocol, Vec<usize>>,
}

impl ResolvedRules {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            by_protocol: HashMap::new(),
        }
    }

    pub fn add(&mut self, resolved: ResolvedRule) {
        let idx = self.rules.len();
        let protocol = resolved.rule.protocol;
        self.rules.push(resolved);
        self.by_protocol.entry(protocol).or_default().push(idx);
    }

    pub fn get_by_protocol(&self, protocol: Protocol) -> Vec<&ResolvedRule> {
        self.by_protocol
            .get(&protocol)
            .map(|indices| indices.iter().map(|&i| &self.rules[i]).collect())
            .unwrap_or_default()
    }

    pub fn has_protocol(&self, protocol: Protocol) -> bool {
        self.by_protocol.contains_key(&protocol)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }
}

struct LruCache {
    capacity: usize,
    cache: HashMap<String, (ResolvedRules, u64)>,
    counter: u64,
}

impl LruCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
            counter: 0,
        }
    }

    fn get(&mut self, key: &str) -> Option<ResolvedRules> {
        if let Some((value, access_time)) = self.cache.get_mut(key) {
            self.counter += 1;
            *access_time = self.counter;
            Some(value.clone())
        } else {
            None
        }
    }

    fn insert(&mut self, key: String, value: ResolvedRules) {
        if self.cache.len() >= self.capacity {
            self.evict_lru();
        }
        self.counter += 1;
        self.cache.insert(key, (value, self.counter));
    }

    fn evict_lru(&mut self) {
        if let Some(lru_key) = self
            .cache
            .iter()
            .min_by_key(|(_, (_, access_time))| access_time)
            .map(|(k, _)| k.clone())
        {
            self.cache.remove(&lru_key);
        }
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.counter = 0;
    }
}

pub struct RulesResolver {
    rules: Vec<Rule>,
    values: HashMap<String, String>,
    cache: Arc<RwLock<LruCache>>,
    cache_enabled: bool,
}

impl RulesResolver {
    pub fn new(rules: Vec<Rule>) -> Self {
        let mut sorted_rules = rules;
        sorted_rules.sort_by_key(|b| std::cmp::Reverse(b.priority()));

        Self {
            rules: sorted_rules,
            values: HashMap::new(),
            cache: Arc::new(RwLock::new(LruCache::new(DEFAULT_CACHE_CAPACITY))),
            cache_enabled: true,
        }
    }

    pub fn with_values(mut self, values: HashMap<String, String>) -> Self {
        self.values = values;
        self
    }

    pub fn with_cache_capacity(self, capacity: usize) -> Self {
        *self.cache.write().unwrap() = LruCache::new(capacity);
        self
    }

    pub fn disable_cache(mut self) -> Self {
        self.cache_enabled = false;
        self
    }

    pub fn set_value(&mut self, key: String, value: String) {
        self.values.insert(key, value);
        self.clear_cache();
    }

    pub fn add_rule(&mut self, rule: Rule) {
        let priority = rule.priority();
        let pos = self
            .rules
            .binary_search_by(|r| priority.cmp(&r.priority()))
            .unwrap_or_else(|e| e);
        self.rules.insert(pos, rule);
        self.clear_cache();
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    pub fn resolve(&self, ctx: &RequestContext) -> ResolvedRules {
        let cache_key = format!("{}|{}|{}|{}", ctx.url, ctx.host, ctx.path, ctx.method);

        if self.cache_enabled {
            if let Ok(mut cache) = self.cache.write() {
                if let Some(cached) = cache.get(&cache_key) {
                    return cached;
                }
            }
        }

        let mut result = ResolvedRules::new();
        let mut matched_protocols: HashMap<Protocol, bool> = HashMap::new();

        for rule in &self.rules {
            if rule.is_disabled() {
                continue;
            }

            if rule.is_negated() {
                let match_result = rule.matcher.matches(&ctx.url, &ctx.host, &ctx.path);
                if match_result.matched {
                    matched_protocols.insert(rule.protocol, true);
                }
                continue;
            }

            if !rule.protocol.is_multi_match() && matched_protocols.contains_key(&rule.protocol) {
                continue;
            }

            let match_result = rule.matcher.matches(&ctx.url, &ctx.host, &ctx.path);
            if !match_result.matched {
                continue;
            }

            if !rule.include_filters.is_empty()
                && !Self::matches_any_filter(&rule.include_filters, ctx)
            {
                continue;
            }

            if Self::matches_any_filter(&rule.exclude_filters, ctx) {
                continue;
            }

            let resolved =
                ResolvedRule::new(rule.clone(), match_result.captures, ctx, &self.values);
            result.add(resolved);

            if !rule.protocol.is_multi_match() {
                matched_protocols.insert(rule.protocol, true);
            }
        }

        if self.cache_enabled {
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(cache_key, result.clone());
            }
        }

        result
    }

    fn matches_any_filter(filters: &[Filter], ctx: &RequestContext) -> bool {
        filters.iter().any(|f| Self::matches_filter(f, ctx))
    }

    fn matches_filter(filter: &Filter, ctx: &RequestContext) -> bool {
        match filter {
            Filter::Method(methods) => {
                let req_method = ctx.method.to_uppercase();
                methods.iter().any(|m| m.to_uppercase() == req_method)
            }
            Filter::StatusCode(range) => {
                if let Some(status) = ctx.status_code {
                    range.matches(status)
                } else {
                    false
                }
            }
            Filter::Path(matcher) => matcher.matches(&ctx.path),
            Filter::HeaderExists(name) => ctx.req_headers.contains_key(&name.to_lowercase()),
            Filter::HeaderMatch {
                name,
                pattern,
                is_request,
            } => {
                let headers = if *is_request {
                    Some(&ctx.req_headers)
                } else {
                    ctx.res_headers.as_ref()
                };
                if let Some(headers) = headers {
                    if let Some(value) = headers.get(&name.to_lowercase()) {
                        return pattern.is_match(value);
                    }
                }
                false
            }
            Filter::ClientIp(matcher) => matcher.matches(&ctx.client_ip),
            Filter::Body(_regex) => false,
            Filter::Custom(_key, _value) => true,
        }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::WildcardMatcher;
    use crate::rule::filter::{parse_filter, LineProps};
    use std::sync::Arc;

    fn create_test_rule(pattern: &str, protocol: Protocol, value: &str) -> Rule {
        let matcher = Arc::new(WildcardMatcher::new(pattern).unwrap());
        Rule::new(
            pattern.to_string(),
            matcher,
            protocol,
            value.to_string(),
            format!("{} {}://{}", pattern, protocol.to_str(), value),
        )
    }

    fn create_test_rule_with_filters(
        pattern: &str,
        protocol: Protocol,
        value: &str,
        include_filters: Vec<Filter>,
        exclude_filters: Vec<Filter>,
    ) -> Rule {
        let matcher = Arc::new(WildcardMatcher::new(pattern).unwrap());
        Rule::new(
            pattern.to_string(),
            matcher,
            protocol,
            value.to_string(),
            format!("{} {}://{}", pattern, protocol.to_str(), value),
        )
        .with_include_filters(include_filters)
        .with_exclude_filters(exclude_filters)
    }

    fn create_test_context(url: &str, host: &str, path: &str) -> RequestContext {
        RequestContext::builder()
            .url(url)
            .host(host)
            .hostname(host)
            .path(path)
            .pathname(path)
            .build()
    }

    #[test]
    fn test_request_context_new() {
        let ctx = create_test_context("http://example.com/path", "example.com", "/path");
        assert_eq!(ctx.url, "http://example.com/path");
        assert_eq!(ctx.host, "example.com");
        assert_eq!(ctx.path, "/path");
    }

    #[test]
    fn test_resolved_rules_new() {
        let result = ResolvedRules::new();
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_resolved_rules_add() {
        let mut result = ResolvedRules::new();
        let rule = create_test_rule("*.example.com", Protocol::Host, "127.0.0.1");
        let resolved = ResolvedRule::new_simple(rule, None, &HashMap::new());
        result.add(resolved);

        assert!(!result.is_empty());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_resolved_rules_get_by_protocol() {
        let mut result = ResolvedRules::new();

        let rule1 = create_test_rule("*.example.com", Protocol::Host, "127.0.0.1");
        let rule2 = create_test_rule("*.api.com", Protocol::Proxy, "proxy:8080");

        result.add(ResolvedRule::new_simple(rule1, None, &HashMap::new()));
        result.add(ResolvedRule::new_simple(rule2, None, &HashMap::new()));

        let host_rules = result.get_by_protocol(Protocol::Host);
        assert_eq!(host_rules.len(), 1);

        let proxy_rules = result.get_by_protocol(Protocol::Proxy);
        assert_eq!(proxy_rules.len(), 1);

        let ignore_rules = result.get_by_protocol(Protocol::Ignore);
        assert!(ignore_rules.is_empty());
    }

    #[test]
    fn test_resolved_rules_has_protocol() {
        let mut result = ResolvedRules::new();
        let rule = create_test_rule("*.example.com", Protocol::Host, "127.0.0.1");
        result.add(ResolvedRule::new_simple(rule, None, &HashMap::new()));

        assert!(result.has_protocol(Protocol::Host));
        assert!(!result.has_protocol(Protocol::Proxy));
    }

    #[test]
    fn test_rules_resolver_new() {
        let rules = vec![
            create_test_rule("*.example.com", Protocol::Host, "127.0.0.1"),
            create_test_rule("example.com", Protocol::Host, "127.0.0.2"),
        ];
        let resolver = RulesResolver::new(rules);
        assert_eq!(resolver.rule_count(), 2);
    }

    #[test]
    fn test_rules_resolver_priority_sorting() {
        let rules = vec![
            create_test_rule("*.example.com", Protocol::Host, "127.0.0.1"),
            create_test_rule("example.com", Protocol::Host, "127.0.0.2"),
        ];
        let resolver = RulesResolver::new(rules);
        assert!(resolver.rules()[0].priority() >= resolver.rules()[1].priority());
    }

    #[test]
    fn test_rules_resolver_resolve() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert!(result.has_protocol(Protocol::Host));
    }

    #[test]
    fn test_rules_resolver_no_match() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.other.com/path", "www.other.com", "/path");

        let result = resolver.resolve(&ctx);
        assert!(result.is_empty());
    }

    #[test]
    fn test_rules_resolver_with_values() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "${target}",
        )];

        let mut values = HashMap::new();
        values.insert("target".to_string(), "127.0.0.1".to_string());

        let resolver = RulesResolver::new(rules).with_values(values);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "127.0.0.1");
    }

    #[test]
    fn test_rules_resolver_add_rule() {
        let mut resolver = RulesResolver::new(vec![]);
        assert_eq!(resolver.rule_count(), 0);

        resolver.add_rule(create_test_rule(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
        ));
        assert_eq!(resolver.rule_count(), 1);
    }

    #[test]
    fn test_rules_resolver_set_value() {
        let mut resolver = RulesResolver::new(vec![]);
        resolver.set_value("key".to_string(), "value".to_string());
        assert_eq!(resolver.values.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_rules_resolver_cache() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result1 = resolver.resolve(&ctx);
        let result2 = resolver.resolve(&ctx);

        assert_eq!(result1.len(), result2.len());
    }

    #[test]
    fn test_rules_resolver_disable_cache() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
        )];
        let resolver = RulesResolver::new(rules).disable_cache();

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_rules_resolver_clear_cache() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let _ = resolver.resolve(&ctx);
        resolver.clear_cache();

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = LruCache::new(2);
        cache.insert("key1".to_string(), ResolvedRules::new());
        cache.insert("key2".to_string(), ResolvedRules::new());
        let _ = cache.get("key1");
        cache.insert("key3".to_string(), ResolvedRules::new());

        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_none());
        assert!(cache.get("key3").is_some());
    }

    #[test]
    fn test_multi_match_protocol() {
        let rules = vec![
            create_test_rule("*.example.com", Protocol::ReqHeaders, "header1=value1"),
            create_test_rule("*.example.com", Protocol::ReqHeaders, "header2=value2"),
        ];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_single_match_protocol() {
        let rules = vec![
            create_test_rule("*.example.com", Protocol::Host, "127.0.0.1"),
            create_test_rule("*.example.com", Protocol::Host, "127.0.0.2"),
        ];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_builtin_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::Host,
            "host-${hostname}",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "host-www.example.com");
    }

    #[test]
    fn test_url_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::ResBody,
            "${url}",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context(
            "http://www.example.com/api/test",
            "www.example.com",
            "/api/test",
        );

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result.rules[0].resolved_value,
            "http://www.example.com/api/test"
        );
    }

    #[test]
    fn test_path_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::ResBody,
            "path=${path}",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = create_test_context(
            "http://www.example.com/api/test?foo=bar",
            "www.example.com",
            "/api/test?foo=bar",
        );

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "path=/api/test?foo=bar");
    }

    #[test]
    fn test_method_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::ResBody,
            "method=${method}",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .method("POST")
            .build();

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "method=POST");
    }

    #[test]
    fn test_client_ip_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::ResBody,
            "client=${clientIp}",
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .client_ip("192.168.1.100")
            .build();

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "client=192.168.1.100");
    }

    #[test]
    fn test_header_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::ResBody,
            "auth=${reqHeaders.authorization}",
        )];
        let resolver = RulesResolver::new(rules);

        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), "Bearer token123".to_string());

        let ctx = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .req_headers(headers)
            .build();

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "auth=Bearer token123");
    }

    #[test]
    fn test_cookie_variable_expansion() {
        let rules = vec![create_test_rule(
            "*.example.com",
            Protocol::ResBody,
            "session=${reqCookies.session_id}",
        )];
        let resolver = RulesResolver::new(rules);

        let mut cookies = HashMap::new();
        cookies.insert("session_id".to_string(), "abc123".to_string());

        let ctx = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .req_cookies(cookies)
            .build();

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "session=abc123");
    }

    #[test]
    fn test_include_filter_method() {
        let include_filters = vec![parse_filter("m:GET").unwrap()];
        let rules = vec![create_test_rule_with_filters(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
            include_filters,
            vec![],
        )];
        let resolver = RulesResolver::new(rules);

        let ctx_get = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .method("GET")
            .build();

        let result = resolver.resolve(&ctx_get);
        assert_eq!(result.len(), 1);

        let ctx_post = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .method("POST")
            .build();

        let result = resolver.resolve(&ctx_post);
        assert!(result.is_empty());
    }

    #[test]
    fn test_exclude_filter_path() {
        let exclude_filters = vec![parse_filter("/admin/").unwrap()];
        let rules = vec![create_test_rule_with_filters(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
            vec![],
            exclude_filters,
        )];
        let resolver = RulesResolver::new(rules);

        let ctx_api = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .build();

        let result = resolver.resolve(&ctx_api);
        assert_eq!(result.len(), 1);

        let ctx_admin = RequestContext::builder()
            .url("http://www.example.com/admin/users")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/admin/users")
            .pathname("/admin/users")
            .build();

        let result = resolver.resolve(&ctx_admin);
        assert!(result.is_empty());
    }

    #[test]
    fn test_combined_include_exclude_filters() {
        let include_filters = vec![parse_filter("m:GET,POST").unwrap()];
        let exclude_filters = vec![parse_filter("/health/").unwrap()];
        let rules = vec![create_test_rule_with_filters(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
            include_filters,
            exclude_filters,
        )];
        let resolver = RulesResolver::new(rules);

        let ctx = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .method("GET")
            .build();

        let result = resolver.resolve(&ctx);
        assert_eq!(result.len(), 1);

        let ctx_health = RequestContext::builder()
            .url("http://www.example.com/health/")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/health/")
            .pathname("/health/")
            .method("GET")
            .build();

        let result = resolver.resolve(&ctx_health);
        assert!(result.is_empty());
    }

    #[test]
    fn test_include_filter_header_exists() {
        let include_filters = vec![parse_filter("h:X-Custom-Header").unwrap()];
        let rules = vec![create_test_rule_with_filters(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
            include_filters,
            vec![],
        )];
        let resolver = RulesResolver::new(rules).disable_cache();

        let ctx_with_header = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .header("X-Custom-Header", "value")
            .build();

        let result = resolver.resolve(&ctx_with_header);
        assert_eq!(result.len(), 1);

        let ctx_without_header = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .build();

        let result = resolver.resolve(&ctx_without_header);
        assert!(result.is_empty());
    }

    #[test]
    fn test_include_filter_client_ip() {
        let include_filters = vec![parse_filter("i:192.168.0.0/16").unwrap()];
        let rules = vec![create_test_rule_with_filters(
            "*.example.com",
            Protocol::Host,
            "127.0.0.1",
            include_filters,
            vec![],
        )];
        let resolver = RulesResolver::new(rules).disable_cache();

        let ctx_match = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .client_ip("192.168.1.100")
            .build();

        let result = resolver.resolve(&ctx_match);
        assert_eq!(result.len(), 1);

        let ctx_no_match = RequestContext::builder()
            .url("http://www.example.com/api")
            .host("www.example.com")
            .hostname("www.example.com")
            .path("/api")
            .pathname("/api")
            .client_ip("10.0.0.1")
            .build();

        let result = resolver.resolve(&ctx_no_match);
        assert!(result.is_empty());
    }

    #[test]
    fn test_disabled_rule() {
        let matcher = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule = Rule::new(
            "*.example.com".to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        )
        .with_line_props(LineProps {
            important: false,
            disabled: true,
        });

        let resolver = RulesResolver::new(vec![rule]);

        let ctx = create_test_context("http://www.example.com/path", "www.example.com", "/path");

        let result = resolver.resolve(&ctx);
        assert!(result.is_empty());
    }

    #[test]
    fn test_important_priority_ordering() {
        let matcher1 = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule1 = Rule::new(
            "*.example.com".to_string(),
            matcher1,
            Protocol::Host,
            "127.0.0.1".to_string(),
            "*.example.com host://127.0.0.1".to_string(),
        );

        let matcher2 = Arc::new(WildcardMatcher::new("*.example.com").unwrap());
        let rule2 = Rule::new(
            "*.example.com".to_string(),
            matcher2,
            Protocol::Host,
            "127.0.0.2".to_string(),
            "*.example.com host://127.0.0.2".to_string(),
        )
        .with_line_props(LineProps {
            important: true,
            disabled: false,
        });

        let resolver = RulesResolver::new(vec![rule1, rule2]);

        assert!(resolver.rules()[0].line_props.important);
        assert!(resolver.rules()[0].priority() > resolver.rules()[1].priority());
    }
}

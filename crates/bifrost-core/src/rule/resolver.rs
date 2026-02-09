use crate::protocol::Protocol;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::rule::Rule;
use super::template::TemplateEngine;

const DEFAULT_CACHE_CAPACITY: usize = 1000;

pub struct Request {
    pub url: String,
    pub host: String,
    pub path: String,
}

impl Request {
    pub fn new(url: String, host: String, path: String) -> Self {
        Self { url, host, path }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRule {
    pub rule: Rule,
    pub captures: Option<Vec<String>>,
    pub resolved_value: String,
}

impl ResolvedRule {
    pub fn new(rule: Rule, captures: Option<Vec<String>>, values: &HashMap<String, String>) -> Self {
        let resolved_value = if captures.is_some() || !values.is_empty() {
            TemplateEngine::expand(&rule.value, captures.as_deref(), values)
        } else {
            rule.value.clone()
        };

        Self {
            rule,
            captures,
            resolved_value,
        }
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
        sorted_rules.sort_by(|a, b| b.priority().cmp(&a.priority()));

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

    pub fn resolve(&self, req: &Request) -> ResolvedRules {
        let cache_key = format!("{}|{}|{}", req.url, req.host, req.path);

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
            if rule.is_negated() {
                let match_result = rule.matcher.matches(&req.url, &req.host, &req.path);
                if match_result.matched {
                    matched_protocols.insert(rule.protocol, true);
                }
                continue;
            }

            if !rule.protocol.is_multi_match() {
                if matched_protocols.contains_key(&rule.protocol) {
                    continue;
                }
            }

            let match_result = rule.matcher.matches(&req.url, &req.host, &req.path);
            if match_result.matched {
                let resolved = ResolvedRule::new(rule.clone(), match_result.captures, &self.values);
                result.add(resolved);

                if !rule.protocol.is_multi_match() {
                    matched_protocols.insert(rule.protocol, true);
                }
            }
        }

        if self.cache_enabled {
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(cache_key, result.clone());
            }
        }

        result
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

    #[test]
    fn test_request_new() {
        let req = Request::new(
            "http://example.com/path".to_string(),
            "example.com".to_string(),
            "/path".to_string(),
        );
        assert_eq!(req.url, "http://example.com/path");
        assert_eq!(req.host, "example.com");
        assert_eq!(req.path, "/path");
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
        let resolved = ResolvedRule::new(rule, None, &HashMap::new());
        result.add(resolved);

        assert!(!result.is_empty());
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_resolved_rules_get_by_protocol() {
        let mut result = ResolvedRules::new();

        let rule1 = create_test_rule("*.example.com", Protocol::Host, "127.0.0.1");
        let rule2 = create_test_rule("*.api.com", Protocol::Proxy, "proxy:8080");

        result.add(ResolvedRule::new(rule1, None, &HashMap::new()));
        result.add(ResolvedRule::new(rule2, None, &HashMap::new()));

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
        result.add(ResolvedRule::new(rule, None, &HashMap::new()));

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

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let result = resolver.resolve(&req);
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

        let req = Request::new(
            "http://www.other.com/path".to_string(),
            "www.other.com".to_string(),
            "/path".to_string(),
        );

        let result = resolver.resolve(&req);
        assert!(result.is_empty());
    }

    #[test]
    fn test_rules_resolver_with_values() {
        let rules = vec![create_test_rule("*.example.com", Protocol::Host, "${target}")];

        let mut values = HashMap::new();
        values.insert("target".to_string(), "127.0.0.1".to_string());

        let resolver = RulesResolver::new(rules).with_values(values);

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let result = resolver.resolve(&req);
        assert_eq!(result.len(), 1);
        assert_eq!(result.rules[0].resolved_value, "127.0.0.1");
    }

    #[test]
    fn test_rules_resolver_add_rule() {
        let mut resolver = RulesResolver::new(vec![]);
        assert_eq!(resolver.rule_count(), 0);

        resolver.add_rule(create_test_rule("*.example.com", Protocol::Host, "127.0.0.1"));
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

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let result1 = resolver.resolve(&req);
        let result2 = resolver.resolve(&req);

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

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let result = resolver.resolve(&req);
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

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let _ = resolver.resolve(&req);
        resolver.clear_cache();

        let result = resolver.resolve(&req);
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

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let result = resolver.resolve(&req);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_single_match_protocol() {
        let rules = vec![
            create_test_rule("*.example.com", Protocol::Host, "127.0.0.1"),
            create_test_rule("*.example.com", Protocol::Host, "127.0.0.2"),
        ];
        let resolver = RulesResolver::new(rules);

        let req = Request::new(
            "http://www.example.com/path".to_string(),
            "www.example.com".to_string(),
            "/path".to_string(),
        );

        let result = resolver.resolve(&req);
        assert_eq!(result.len(), 1);
    }
}

use std::collections::HashMap;

use super::rule::Rule;

#[derive(Debug, Clone)]
pub struct RuleGroup {
    pub name: String,
    pub rules: Vec<Rule>,
    pub enabled: bool,
    pub order: i32,
}

impl RuleGroup {
    pub fn new(name: String) -> Self {
        Self {
            name,
            rules: Vec::new(),
            enabled: true,
            order: 0,
        }
    }

    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn add_rules(&mut self, rules: Vec<Rule>) {
        self.rules.extend(rules);
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }
}

pub struct RuleGroupManager {
    groups: HashMap<String, RuleGroup>,
    order_counter: i32,
}

impl RuleGroupManager {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
            order_counter: 0,
        }
    }

    pub fn add_group(&mut self, name: String) -> &mut RuleGroup {
        let order = self.order_counter;
        self.order_counter += 1;

        self.groups
            .entry(name.clone())
            .or_insert_with(|| RuleGroup::new(name).with_order(order))
    }

    pub fn get_group(&self, name: &str) -> Option<&RuleGroup> {
        self.groups.get(name)
    }

    pub fn get_group_mut(&mut self, name: &str) -> Option<&mut RuleGroup> {
        self.groups.get_mut(name)
    }

    pub fn remove_group(&mut self, name: &str) -> Option<RuleGroup> {
        self.groups.remove(name)
    }

    pub fn enable_group(&mut self, name: &str) -> bool {
        if let Some(group) = self.groups.get_mut(name) {
            group.enable();
            true
        } else {
            false
        }
    }

    pub fn disable_group(&mut self, name: &str) -> bool {
        if let Some(group) = self.groups.get_mut(name) {
            group.disable();
            true
        } else {
            false
        }
    }

    pub fn toggle_group(&mut self, name: &str) -> bool {
        if let Some(group) = self.groups.get_mut(name) {
            group.toggle();
            true
        } else {
            false
        }
    }

    pub fn set_group_order(&mut self, name: &str, order: i32) -> bool {
        if let Some(group) = self.groups.get_mut(name) {
            group.order = order;
            true
        } else {
            false
        }
    }

    pub fn get_enabled_rules(&self) -> Vec<Rule> {
        let mut groups: Vec<_> = self.groups.values().filter(|g| g.enabled).collect();
        groups.sort_by_key(|g| g.order);

        groups
            .into_iter()
            .flat_map(|g| g.rules.clone())
            .collect()
    }

    pub fn get_all_rules(&self) -> Vec<Rule> {
        let mut groups: Vec<_> = self.groups.values().collect();
        groups.sort_by_key(|g| g.order);

        groups
            .into_iter()
            .flat_map(|g| g.rules.clone())
            .collect()
    }

    pub fn group_names(&self) -> Vec<&str> {
        let mut groups: Vec<_> = self.groups.values().collect();
        groups.sort_by_key(|g| g.order);
        groups.iter().map(|g| g.name.as_str()).collect()
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn enabled_group_count(&self) -> usize {
        self.groups.values().filter(|g| g.enabled).count()
    }

    pub fn total_rule_count(&self) -> usize {
        self.groups.values().map(|g| g.rule_count()).sum()
    }

    pub fn enabled_rule_count(&self) -> usize {
        self.groups
            .values()
            .filter(|g| g.enabled)
            .map(|g| g.rule_count())
            .sum()
    }
}

impl Default for RuleGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::WildcardMatcher;
    use crate::protocol::Protocol;
    use std::sync::Arc;

    fn create_test_rule(pattern: &str) -> Rule {
        let matcher = Arc::new(WildcardMatcher::new(pattern).unwrap());
        Rule::new(
            pattern.to_string(),
            matcher,
            Protocol::Host,
            "127.0.0.1".to_string(),
            format!("{} host://127.0.0.1", pattern),
        )
    }

    #[test]
    fn test_rule_group_new() {
        let group = RuleGroup::new("test".to_string());
        assert_eq!(group.name, "test");
        assert!(group.enabled);
        assert_eq!(group.order, 0);
        assert!(group.rules.is_empty());
    }

    #[test]
    fn test_rule_group_with_order() {
        let group = RuleGroup::new("test".to_string()).with_order(10);
        assert_eq!(group.order, 10);
    }

    #[test]
    fn test_rule_group_with_enabled() {
        let group = RuleGroup::new("test".to_string()).with_enabled(false);
        assert!(!group.enabled);
    }

    #[test]
    fn test_rule_group_add_rule() {
        let mut group = RuleGroup::new("test".to_string());
        group.add_rule(create_test_rule("*.example.com"));
        assert_eq!(group.rule_count(), 1);
    }

    #[test]
    fn test_rule_group_add_rules() {
        let mut group = RuleGroup::new("test".to_string());
        let rules = vec![
            create_test_rule("*.example.com"),
            create_test_rule("*.api.com"),
        ];
        group.add_rules(rules);
        assert_eq!(group.rule_count(), 2);
    }

    #[test]
    fn test_rule_group_enable_disable() {
        let mut group = RuleGroup::new("test".to_string());

        group.disable();
        assert!(!group.is_enabled());

        group.enable();
        assert!(group.is_enabled());
    }

    #[test]
    fn test_rule_group_toggle() {
        let mut group = RuleGroup::new("test".to_string());
        assert!(group.is_enabled());

        group.toggle();
        assert!(!group.is_enabled());

        group.toggle();
        assert!(group.is_enabled());
    }

    #[test]
    fn test_rule_group_clear() {
        let mut group = RuleGroup::new("test".to_string());
        group.add_rule(create_test_rule("*.example.com"));
        assert_eq!(group.rule_count(), 1);

        group.clear();
        assert_eq!(group.rule_count(), 0);
    }

    #[test]
    fn test_rule_group_manager_new() {
        let manager = RuleGroupManager::new();
        assert_eq!(manager.group_count(), 0);
    }

    #[test]
    fn test_rule_group_manager_add_group() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());
        assert_eq!(manager.group_count(), 1);
    }

    #[test]
    fn test_rule_group_manager_get_group() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());

        let group = manager.get_group("group1");
        assert!(group.is_some());
        assert_eq!(group.unwrap().name, "group1");

        assert!(manager.get_group("nonexistent").is_none());
    }

    #[test]
    fn test_rule_group_manager_get_group_mut() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());

        if let Some(group) = manager.get_group_mut("group1") {
            group.add_rule(create_test_rule("*.example.com"));
        }

        assert_eq!(manager.get_group("group1").unwrap().rule_count(), 1);
    }

    #[test]
    fn test_rule_group_manager_remove_group() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());
        assert_eq!(manager.group_count(), 1);

        let removed = manager.remove_group("group1");
        assert!(removed.is_some());
        assert_eq!(manager.group_count(), 0);
    }

    #[test]
    fn test_rule_group_manager_enable_disable_group() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());

        assert!(manager.disable_group("group1"));
        assert!(!manager.get_group("group1").unwrap().enabled);

        assert!(manager.enable_group("group1"));
        assert!(manager.get_group("group1").unwrap().enabled);

        assert!(!manager.enable_group("nonexistent"));
    }

    #[test]
    fn test_rule_group_manager_toggle_group() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());

        assert!(manager.toggle_group("group1"));
        assert!(!manager.get_group("group1").unwrap().enabled);

        assert!(manager.toggle_group("group1"));
        assert!(manager.get_group("group1").unwrap().enabled);
    }

    #[test]
    fn test_rule_group_manager_set_group_order() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());

        assert!(manager.set_group_order("group1", 10));
        assert_eq!(manager.get_group("group1").unwrap().order, 10);

        assert!(!manager.set_group_order("nonexistent", 20));
    }

    #[test]
    fn test_rule_group_manager_get_enabled_rules() {
        let mut manager = RuleGroupManager::new();

        {
            let group1 = manager.add_group("group1".to_string());
            group1.add_rule(create_test_rule("*.example.com"));
        }

        {
            let group2 = manager.add_group("group2".to_string());
            group2.add_rule(create_test_rule("*.api.com"));
            group2.disable();
        }

        let rules = manager.get_enabled_rules();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_rule_group_manager_get_all_rules() {
        let mut manager = RuleGroupManager::new();

        {
            let group1 = manager.add_group("group1".to_string());
            group1.add_rule(create_test_rule("*.example.com"));
        }

        {
            let group2 = manager.add_group("group2".to_string());
            group2.add_rule(create_test_rule("*.api.com"));
            group2.disable();
        }

        let rules = manager.get_all_rules();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn test_rule_group_manager_group_names() {
        let mut manager = RuleGroupManager::new();
        manager.add_group("group1".to_string());
        manager.add_group("group2".to_string());

        let names = manager.group_names();
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_rule_group_manager_counts() {
        let mut manager = RuleGroupManager::new();

        {
            let group1 = manager.add_group("group1".to_string());
            group1.add_rule(create_test_rule("*.example.com"));
            group1.add_rule(create_test_rule("*.test.com"));
        }

        {
            let group2 = manager.add_group("group2".to_string());
            group2.add_rule(create_test_rule("*.api.com"));
            group2.disable();
        }

        assert_eq!(manager.group_count(), 2);
        assert_eq!(manager.enabled_group_count(), 1);
        assert_eq!(manager.total_rule_count(), 3);
        assert_eq!(manager.enabled_rule_count(), 2);
    }

    #[test]
    fn test_rule_group_manager_order_preservation() {
        let mut manager = RuleGroupManager::new();

        manager.add_group("group3".to_string());
        manager.add_group("group1".to_string());
        manager.add_group("group2".to_string());

        manager.set_group_order("group1", 1);
        manager.set_group_order("group2", 2);
        manager.set_group_order("group3", 3);

        let names = manager.group_names();
        assert_eq!(names, vec!["group1", "group2", "group3"]);
    }

    #[test]
    fn test_rule_group_manager_default() {
        let manager = RuleGroupManager::default();
        assert_eq!(manager.group_count(), 0);
    }
}

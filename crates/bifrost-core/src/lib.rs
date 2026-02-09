pub mod error;
pub mod logging;
pub mod matcher;
pub mod protocol;
pub mod rule;

pub use error::{Result, BifrostError};
pub use logging::init_logging;
pub use matcher::{
    DomainMatcher, IpMatcher, MatchResult, Matcher, RegexMatcher, WildcardMatcher,
    factory::parse_pattern,
};
pub use protocol::*;
pub use rule::{
    parse_line, parse_rules, ResolvedRule, ResolvedRules, Rule, RuleGroup, RuleGroupManager,
    RuleParser, RulesResolver, TemplateEngine, TemplateValue,
};

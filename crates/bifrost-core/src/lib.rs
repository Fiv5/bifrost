pub mod access_control;
pub mod error;
pub mod logging;
pub mod matcher;
pub mod protocol;
pub mod rule;

pub use access_control::{AccessControlConfig, AccessDecision, AccessMode, ClientAccessControl};
pub use error::{BifrostError, Result};
pub use logging::init_logging;
pub use matcher::{
    factory::parse_pattern, DomainMatcher, IpMatcher, MatchResult, Matcher, RegexMatcher,
    WildcardMatcher,
};
pub use protocol::*;
pub use rule::{
    parse_line, parse_rules, RequestContext, RequestContextBuilder, ResolvedRule, ResolvedRules,
    Rule, RuleGroup, RuleGroupManager, RuleParser, RulesResolver, TemplateEngine,
};

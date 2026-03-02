pub mod access_control;
pub mod error;
pub mod logging;
pub mod matcher;
pub mod panic_handler;
pub mod protocol;
pub mod rule;
pub mod syntax;
pub mod system_proxy;

pub use access_control::{
    AccessControlConfig, AccessDecision, AccessMode, ClientAccessControl, PendingAuth,
};
pub use error::{BifrostError, Result};
pub use logging::{init_logging, init_logging_with_config, LogConfig, LogGuard, LogOutput};
pub use matcher::{
    factory::parse_pattern, DomainMatcher, IpMatcher, MatchResult, Matcher, RegexMatcher,
    WildcardMatcher,
};
pub use panic_handler::{install_panic_hook, spawn_with_panic_guard};
pub use protocol::*;
pub use rule::{
    create_shared_store, parse_line, parse_rules, parse_rules_tolerant, validate_rules,
    validate_rules_with_context, CompositeValueStore, MemoryValueStore, ParseError,
    ParseErrorSeverity, ParseResult, RequestContext, RequestContextBuilder, ResolvedRule,
    ResolvedRules, Rule, RuleGroup, RuleGroupManager, RuleParser, RulesResolver, ScriptReference,
    SharedValueStore, TemplateEngine, ValidationResult, ValueStore, VariableInfo,
};
pub use syntax::{
    get_all_protocols, get_filter_value_specs, get_pattern_types, get_syntax_info,
    get_template_variables, validate_filter_value, FilterValidationError, PatternInfo,
    ProtocolInfo, ProtocolValueSpec, SyntaxInfo, TemplateVariableInfo, ValueHint,
};
pub use system_proxy::{ProxyBackup, SystemProxyManager};

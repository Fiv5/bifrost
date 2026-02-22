pub mod access_control;
pub mod error;
pub mod logging;
pub mod matcher;
pub mod panic_handler;
pub mod protocol;
pub mod rule;
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
    create_shared_store, parse_line, parse_rules, CompositeValueStore, MemoryValueStore,
    RequestContext, RequestContextBuilder, ResolvedRule, ResolvedRules, Rule, RuleGroup,
    RuleGroupManager, RuleParser, RulesResolver, SharedValueStore, TemplateEngine, ValueStore,
};
pub use system_proxy::{ProxyBackup, SystemProxyManager};

mod config;
mod config_manager;
mod data_dir;
mod rules;
mod state;
mod unified_config;
mod values;

#[deprecated(note = "Use unified_config::AccessConfig instead")]
pub use config::AccessConfig as LegacyAccessConfig;
#[deprecated(note = "Use UnifiedConfig instead")]
pub use config::BifrostConfig as LegacyBifrostConfig;
#[deprecated(note = "Use unified_config::SystemProxyConfig instead")]
pub use config::SystemProxyConfig as LegacySystemProxyConfig;
#[deprecated(note = "Use unified_config::TrafficConfig instead")]
pub use config::TrafficConfig as LegacyTrafficConfig;

pub use config::{AccessConfig, BifrostConfig, SystemProxyConfig, TrafficConfig};
pub use config_manager::{ConfigChangeEvent, ConfigManager, SharedConfigManager};
pub use data_dir::{data_dir, set_data_dir};
pub use rules::{RuleFile, RulesStorage};
pub use state::{RuntimeState, StateManager};
pub use unified_config::{
    AccessConfig as NewAccessConfig, AccessConfigUpdate, PathsConfig, ProxySettings, ServerConfig,
    SocksAuthConfig, SystemProxyConfig as NewSystemProxyConfig, SystemProxyConfigUpdate, TlsConfig,
    TlsConfigUpdate, TrafficConfig as NewTrafficConfig, UnifiedConfig,
};
pub use values::{ValueEntry, ValuesStorage};

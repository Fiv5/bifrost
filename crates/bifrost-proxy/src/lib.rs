pub mod dns;
#[cfg(feature = "http3")]
pub mod http3;
pub mod protocol;
mod proxy;
mod server;
pub mod transform;
mod unified;
pub mod utils;

#[cfg(feature = "http3")]
pub use http3::*;
pub use transform::{decompress_body, get_content_encoding};

pub use bifrost_core::{AccessControlConfig, AccessDecision, AccessMode, ClientAccessControl};
pub use dns::DnsResolver;
pub use protocol::{
    ContentType, DetectionResult, Priority, ProtocolHandler, ProtocolRegistry, ProxyContext,
    TransportProtocol,
};
pub use proxy::*;
pub use server::*;
pub use transform::{apply_req_rules, format_cookie_header, parse_cookie_string};
pub use transform::{apply_res_rules, format_set_cookie, parse_set_cookie, SetCookieOptions};
pub use unified::*;
pub use utils::logging::*;
pub use utils::process_info::{
    format_client_info, resolve_client_process, ClientProcess, ProcessResolver, PROCESS_RESOLVER,
};

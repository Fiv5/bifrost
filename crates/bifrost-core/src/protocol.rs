use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    // 高级功能
    G,
    Style,
    Rule,
    Pipe,

    // 基础路由
    Host,
    XHost,
    Proxy,
    Pac,
    InternalProxy,
    Https2HttpProxy,
    Http2HttpsProxy,
    Redirect,
    LocationHref,
    File,
    Tpl,
    RawFile,

    // 过滤控制
    Filter,
    Ignore,
    Enable,
    Disable,
    Delete,

    // 请求修改
    ReqHeaders,
    ReqBody,
    ReqPrepend,
    ReqAppend,
    ReqCookies,
    ReqCors,
    ReqDelay,
    ReqSpeed,
    ReqType,
    ReqCharset,
    ReqReplace,
    ReqWrite,
    ReqWriteRaw,
    Method,
    Auth,
    Ua,
    Referer,
    UrlParams,
    Params,

    // 响应修改
    ResHeaders,
    ResBody,
    ResPrepend,
    ResAppend,
    ResCookies,
    ResCors,
    ResDelay,
    ResSpeed,
    ResType,
    ResCharset,
    ResReplace,
    ResWrite,
    ResWriteRaw,
    ReplaceStatus,
    StatusCode,
    Cache,
    Attachment,
    ResponseFor,
    ForwardedFor,
    Trailers,
    ResMerge,
    HeaderReplace,

    // 内容注入
    HtmlAppend,
    HtmlPrepend,
    HtmlBody,
    JsAppend,
    JsPrepend,
    JsBody,
    CssAppend,
    CssPrepend,
    CssBody,

    // URL处理
    UrlReplace,

    // 脚本插件
    Plugin,
    RulesFile,
    ResScript,
    FrameScript,
    Log,
    Weinre,

    // 高级功能
    SniCallback,
    Cipher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolCategory {
    Request,
    Response,
    Both,
    Control,
}

pub const MULTI_MATCH_PROTOCOLS: &[Protocol] = &[
    Protocol::G,
    Protocol::Ignore,
    Protocol::Enable,
    Protocol::Filter,
    Protocol::Disable,
    Protocol::Plugin,
    Protocol::Delete,
    Protocol::Style,
    Protocol::Cipher,
    Protocol::Trailers,
    Protocol::UrlParams,
    Protocol::Params,
    Protocol::HeaderReplace,
    Protocol::ReqHeaders,
    Protocol::ResHeaders,
    Protocol::ReqCors,
    Protocol::ResCors,
    Protocol::ReqCookies,
    Protocol::ResCookies,
    Protocol::ReqReplace,
    Protocol::UrlReplace,
    Protocol::ResReplace,
    Protocol::ResMerge,
    Protocol::ReqBody,
    Protocol::ReqPrepend,
    Protocol::ResPrepend,
    Protocol::ReqAppend,
    Protocol::ResAppend,
    Protocol::ResBody,
    Protocol::HtmlAppend,
    Protocol::JsAppend,
    Protocol::CssAppend,
    Protocol::HtmlBody,
    Protocol::JsBody,
    Protocol::CssBody,
    Protocol::HtmlPrepend,
    Protocol::JsPrepend,
    Protocol::CssPrepend,
    Protocol::RulesFile,
    Protocol::ResScript,
];

pub fn protocol_aliases() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    map.insert("ruleFile", "rulesFile");
    map.insert("ruleScript", "rulesFile");
    map.insert("rulesScript", "rulesFile");
    map.insert("reqScript", "rulesFile");
    map.insert("reqRules", "rulesFile");
    map.insert("resRules", "resScript");
    map.insert("pathReplace", "urlReplace");
    map.insert("download", "attachment");
    map.insert("skip", "ignore");
    map.insert("http-proxy", "proxy");
    map.insert("xhttp-proxy", "xproxy");
    map.insert("status", "statusCode");
    map.insert("hosts", "host");
    map.insert("html", "htmlAppend");
    map.insert("js", "jsAppend");
    map.insert("reqMerge", "params");
    map.insert("tlsOptions", "cipher");
    map.insert("css", "cssAppend");
    map.insert("excludeFilter", "filter");
    map.insert("includeFilter", "filter");
    map.insert("P", "G");
    map
}

lazy_static::lazy_static! {
    pub static ref PROTOCOL_ALIASES: HashMap<&'static str, &'static str> = protocol_aliases();
}

const PURE_RES_PROTOCOLS: &[Protocol] = &[
    Protocol::ReplaceStatus,
    Protocol::StatusCode,
    Protocol::Cache,
    Protocol::Attachment,
    Protocol::ResMerge,
    Protocol::ResDelay,
    Protocol::ResSpeed,
    Protocol::ResType,
    Protocol::ResCharset,
    Protocol::ResCookies,
    Protocol::ResCors,
    Protocol::ResHeaders,
    Protocol::Trailers,
    Protocol::ResPrepend,
    Protocol::ResBody,
    Protocol::ResAppend,
    Protocol::ResReplace,
    Protocol::ResWrite,
    Protocol::ResWriteRaw,
    Protocol::CssAppend,
    Protocol::HtmlAppend,
    Protocol::JsAppend,
    Protocol::CssBody,
    Protocol::HtmlBody,
    Protocol::JsBody,
    Protocol::CssPrepend,
    Protocol::HtmlPrepend,
    Protocol::JsPrepend,
    Protocol::ResponseFor,
];

impl Protocol {
    pub fn parse(s: &str) -> Option<Protocol> {
        let resolved = Self::resolve_alias(s);
        match resolved {
            "G" => Some(Protocol::G),
            "style" => Some(Protocol::Style),
            "host" => Some(Protocol::Host),
            "xhost" => Some(Protocol::XHost),
            "rule" => Some(Protocol::Rule),
            "pipe" => Some(Protocol::Pipe),
            "weinre" => Some(Protocol::Weinre),
            "proxy" => Some(Protocol::Proxy),
            "https2http-proxy" => Some(Protocol::Https2HttpProxy),
            "http2https-proxy" => Some(Protocol::Http2HttpsProxy),
            "internal-proxy" => Some(Protocol::InternalProxy),
            "pac" => Some(Protocol::Pac),
            "redirect" => Some(Protocol::Redirect),
            "locationHref" => Some(Protocol::LocationHref),
            "file" => Some(Protocol::File),
            "tpl" => Some(Protocol::Tpl),
            "rawfile" => Some(Protocol::RawFile),
            "filter" => Some(Protocol::Filter),
            "ignore" => Some(Protocol::Ignore),
            "enable" => Some(Protocol::Enable),
            "disable" => Some(Protocol::Disable),
            "delete" => Some(Protocol::Delete),
            "log" => Some(Protocol::Log),
            "plugin" => Some(Protocol::Plugin),
            "referer" => Some(Protocol::Referer),
            "auth" => Some(Protocol::Auth),
            "ua" => Some(Protocol::Ua),
            "urlParams" => Some(Protocol::UrlParams),
            "params" => Some(Protocol::Params),
            "resMerge" => Some(Protocol::ResMerge),
            "replaceStatus" => Some(Protocol::ReplaceStatus),
            "statusCode" => Some(Protocol::StatusCode),
            "method" => Some(Protocol::Method),
            "cache" => Some(Protocol::Cache),
            "attachment" => Some(Protocol::Attachment),
            "forwardedFor" => Some(Protocol::ForwardedFor),
            "responseFor" => Some(Protocol::ResponseFor),
            "rulesFile" => Some(Protocol::RulesFile),
            "resScript" => Some(Protocol::ResScript),
            "frameScript" => Some(Protocol::FrameScript),
            "reqDelay" => Some(Protocol::ReqDelay),
            "resDelay" => Some(Protocol::ResDelay),
            "headerReplace" => Some(Protocol::HeaderReplace),
            "reqSpeed" => Some(Protocol::ReqSpeed),
            "resSpeed" => Some(Protocol::ResSpeed),
            "reqType" => Some(Protocol::ReqType),
            "resType" => Some(Protocol::ResType),
            "reqCharset" => Some(Protocol::ReqCharset),
            "resCharset" => Some(Protocol::ResCharset),
            "reqCookies" => Some(Protocol::ReqCookies),
            "resCookies" => Some(Protocol::ResCookies),
            "reqCors" => Some(Protocol::ReqCors),
            "resCors" => Some(Protocol::ResCors),
            "reqHeaders" => Some(Protocol::ReqHeaders),
            "resHeaders" => Some(Protocol::ResHeaders),
            "trailers" => Some(Protocol::Trailers),
            "reqPrepend" => Some(Protocol::ReqPrepend),
            "resPrepend" => Some(Protocol::ResPrepend),
            "reqBody" => Some(Protocol::ReqBody),
            "resBody" => Some(Protocol::ResBody),
            "reqAppend" => Some(Protocol::ReqAppend),
            "resAppend" => Some(Protocol::ResAppend),
            "urlReplace" => Some(Protocol::UrlReplace),
            "reqReplace" => Some(Protocol::ReqReplace),
            "resReplace" => Some(Protocol::ResReplace),
            "reqWrite" => Some(Protocol::ReqWrite),
            "resWrite" => Some(Protocol::ResWrite),
            "reqWriteRaw" => Some(Protocol::ReqWriteRaw),
            "resWriteRaw" => Some(Protocol::ResWriteRaw),
            "cssAppend" => Some(Protocol::CssAppend),
            "htmlAppend" => Some(Protocol::HtmlAppend),
            "jsAppend" => Some(Protocol::JsAppend),
            "cssBody" => Some(Protocol::CssBody),
            "htmlBody" => Some(Protocol::HtmlBody),
            "jsBody" => Some(Protocol::JsBody),
            "cssPrepend" => Some(Protocol::CssPrepend),
            "htmlPrepend" => Some(Protocol::HtmlPrepend),
            "jsPrepend" => Some(Protocol::JsPrepend),
            "cipher" => Some(Protocol::Cipher),
            "sniCallback" => Some(Protocol::SniCallback),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Protocol::G => "G",
            Protocol::Style => "style",
            Protocol::Host => "host",
            Protocol::XHost => "xhost",
            Protocol::Rule => "rule",
            Protocol::Pipe => "pipe",
            Protocol::Weinre => "weinre",
            Protocol::Proxy => "proxy",
            Protocol::Https2HttpProxy => "https2http-proxy",
            Protocol::Http2HttpsProxy => "http2https-proxy",
            Protocol::InternalProxy => "internal-proxy",
            Protocol::Pac => "pac",
            Protocol::Redirect => "redirect",
            Protocol::LocationHref => "locationHref",
            Protocol::File => "file",
            Protocol::Tpl => "tpl",
            Protocol::RawFile => "rawfile",
            Protocol::Filter => "filter",
            Protocol::Ignore => "ignore",
            Protocol::Enable => "enable",
            Protocol::Disable => "disable",
            Protocol::Delete => "delete",
            Protocol::Log => "log",
            Protocol::Plugin => "plugin",
            Protocol::Referer => "referer",
            Protocol::Auth => "auth",
            Protocol::Ua => "ua",
            Protocol::UrlParams => "urlParams",
            Protocol::Params => "params",
            Protocol::ResMerge => "resMerge",
            Protocol::ReplaceStatus => "replaceStatus",
            Protocol::StatusCode => "statusCode",
            Protocol::Method => "method",
            Protocol::Cache => "cache",
            Protocol::Attachment => "attachment",
            Protocol::ForwardedFor => "forwardedFor",
            Protocol::ResponseFor => "responseFor",
            Protocol::RulesFile => "rulesFile",
            Protocol::ResScript => "resScript",
            Protocol::FrameScript => "frameScript",
            Protocol::ReqDelay => "reqDelay",
            Protocol::ResDelay => "resDelay",
            Protocol::HeaderReplace => "headerReplace",
            Protocol::ReqSpeed => "reqSpeed",
            Protocol::ResSpeed => "resSpeed",
            Protocol::ReqType => "reqType",
            Protocol::ResType => "resType",
            Protocol::ReqCharset => "reqCharset",
            Protocol::ResCharset => "resCharset",
            Protocol::ReqCookies => "reqCookies",
            Protocol::ResCookies => "resCookies",
            Protocol::ReqCors => "reqCors",
            Protocol::ResCors => "resCors",
            Protocol::ReqHeaders => "reqHeaders",
            Protocol::ResHeaders => "resHeaders",
            Protocol::Trailers => "trailers",
            Protocol::ReqPrepend => "reqPrepend",
            Protocol::ResPrepend => "resPrepend",
            Protocol::ReqBody => "reqBody",
            Protocol::ResBody => "resBody",
            Protocol::ReqAppend => "reqAppend",
            Protocol::ResAppend => "resAppend",
            Protocol::UrlReplace => "urlReplace",
            Protocol::ReqReplace => "reqReplace",
            Protocol::ResReplace => "resReplace",
            Protocol::ReqWrite => "reqWrite",
            Protocol::ResWrite => "resWrite",
            Protocol::ReqWriteRaw => "reqWriteRaw",
            Protocol::ResWriteRaw => "resWriteRaw",
            Protocol::CssAppend => "cssAppend",
            Protocol::HtmlAppend => "htmlAppend",
            Protocol::JsAppend => "jsAppend",
            Protocol::CssBody => "cssBody",
            Protocol::HtmlBody => "htmlBody",
            Protocol::JsBody => "jsBody",
            Protocol::CssPrepend => "cssPrepend",
            Protocol::HtmlPrepend => "htmlPrepend",
            Protocol::JsPrepend => "jsPrepend",
            Protocol::Cipher => "cipher",
            Protocol::SniCallback => "sniCallback",
        }
    }

    pub fn category(&self) -> ProtocolCategory {
        match self {
            Protocol::Filter
            | Protocol::Ignore
            | Protocol::Enable
            | Protocol::Disable
            | Protocol::Delete
            | Protocol::G
            | Protocol::Style
            | Protocol::Plugin
            | Protocol::Log
            | Protocol::Weinre => ProtocolCategory::Control,

            Protocol::ReplaceStatus
            | Protocol::StatusCode
            | Protocol::Cache
            | Protocol::Attachment
            | Protocol::ResMerge
            | Protocol::ResDelay
            | Protocol::ResSpeed
            | Protocol::ResType
            | Protocol::ResCharset
            | Protocol::ResCookies
            | Protocol::ResCors
            | Protocol::ResHeaders
            | Protocol::Trailers
            | Protocol::ResPrepend
            | Protocol::ResBody
            | Protocol::ResAppend
            | Protocol::ResReplace
            | Protocol::ResWrite
            | Protocol::ResWriteRaw
            | Protocol::CssAppend
            | Protocol::HtmlAppend
            | Protocol::JsAppend
            | Protocol::CssBody
            | Protocol::HtmlBody
            | Protocol::JsBody
            | Protocol::CssPrepend
            | Protocol::HtmlPrepend
            | Protocol::JsPrepend
            | Protocol::ResponseFor
            | Protocol::ResScript
            | Protocol::FrameScript => ProtocolCategory::Response,

            Protocol::ReqHeaders
            | Protocol::ReqBody
            | Protocol::ReqPrepend
            | Protocol::ReqAppend
            | Protocol::ReqCookies
            | Protocol::ReqCors
            | Protocol::ReqDelay
            | Protocol::ReqSpeed
            | Protocol::ReqType
            | Protocol::ReqCharset
            | Protocol::ReqReplace
            | Protocol::ReqWrite
            | Protocol::ReqWriteRaw
            | Protocol::Method
            | Protocol::Auth
            | Protocol::Ua
            | Protocol::Referer
            | Protocol::UrlParams
            | Protocol::Params
            | Protocol::RulesFile => ProtocolCategory::Request,

            Protocol::Host
            | Protocol::XHost
            | Protocol::Proxy
            | Protocol::Pac
            | Protocol::InternalProxy
            | Protocol::Https2HttpProxy
            | Protocol::Http2HttpsProxy
            | Protocol::Redirect
            | Protocol::LocationHref
            | Protocol::File
            | Protocol::Tpl
            | Protocol::RawFile
            | Protocol::Rule
            | Protocol::Pipe
            | Protocol::HeaderReplace
            | Protocol::UrlReplace
            | Protocol::SniCallback
            | Protocol::Cipher
            | Protocol::ForwardedFor => ProtocolCategory::Both,
        }
    }

    pub fn is_multi_match(&self) -> bool {
        MULTI_MATCH_PROTOCOLS.contains(self)
    }

    pub fn resolve_alias(name: &str) -> &str {
        PROTOCOL_ALIASES.get(name).copied().unwrap_or(name)
    }

    pub fn is_res_protocol(&self) -> bool {
        PURE_RES_PROTOCOLS.contains(self)
            || matches!(
                self,
                Protocol::Filter
                    | Protocol::Enable
                    | Protocol::Disable
                    | Protocol::Ignore
                    | Protocol::Style
                    | Protocol::Delete
                    | Protocol::HeaderReplace
            )
    }

    pub fn is_req_protocol(&self) -> bool {
        !PURE_RES_PROTOCOLS.contains(self)
    }

    pub fn all() -> &'static [Protocol] {
        &ALL_PROTOCOLS
    }
}

impl FromStr for Protocol {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Protocol::parse(s).ok_or(())
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

pub const ALL_PROTOCOLS: [Protocol; 74] = [
    Protocol::G,
    Protocol::Style,
    Protocol::Host,
    Protocol::Rule,
    Protocol::Pipe,
    Protocol::Weinre,
    Protocol::Proxy,
    Protocol::Https2HttpProxy,
    Protocol::Http2HttpsProxy,
    Protocol::InternalProxy,
    Protocol::Pac,
    Protocol::Filter,
    Protocol::Ignore,
    Protocol::Enable,
    Protocol::Disable,
    Protocol::Delete,
    Protocol::Log,
    Protocol::Plugin,
    Protocol::Referer,
    Protocol::Auth,
    Protocol::Ua,
    Protocol::UrlParams,
    Protocol::Params,
    Protocol::ResMerge,
    Protocol::ReplaceStatus,
    Protocol::StatusCode,
    Protocol::Method,
    Protocol::Cache,
    Protocol::Attachment,
    Protocol::ForwardedFor,
    Protocol::ResponseFor,
    Protocol::RulesFile,
    Protocol::ResScript,
    Protocol::FrameScript,
    Protocol::ReqDelay,
    Protocol::ResDelay,
    Protocol::HeaderReplace,
    Protocol::ReqSpeed,
    Protocol::ResSpeed,
    Protocol::ReqType,
    Protocol::ResType,
    Protocol::ReqCharset,
    Protocol::ResCharset,
    Protocol::ReqCookies,
    Protocol::ResCookies,
    Protocol::ReqCors,
    Protocol::ResCors,
    Protocol::ReqHeaders,
    Protocol::ResHeaders,
    Protocol::Trailers,
    Protocol::ReqPrepend,
    Protocol::ResPrepend,
    Protocol::ReqBody,
    Protocol::ResBody,
    Protocol::ReqAppend,
    Protocol::ResAppend,
    Protocol::UrlReplace,
    Protocol::ReqReplace,
    Protocol::ResReplace,
    Protocol::ReqWrite,
    Protocol::ResWrite,
    Protocol::ReqWriteRaw,
    Protocol::ResWriteRaw,
    Protocol::CssAppend,
    Protocol::HtmlAppend,
    Protocol::JsAppend,
    Protocol::CssBody,
    Protocol::HtmlBody,
    Protocol::JsBody,
    Protocol::CssPrepend,
    Protocol::HtmlPrepend,
    Protocol::JsPrepend,
    Protocol::Cipher,
    Protocol::SniCallback,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_count() {
        assert_eq!(ALL_PROTOCOLS.len(), 74);
    }

    #[test]
    fn test_all_protocols_parse() {
        let protocol_names = [
            "G",
            "style",
            "host",
            "rule",
            "pipe",
            "weinre",
            "proxy",
            "https2http-proxy",
            "http2https-proxy",
            "internal-proxy",
            "pac",
            "filter",
            "ignore",
            "enable",
            "disable",
            "delete",
            "log",
            "plugin",
            "referer",
            "auth",
            "ua",
            "urlParams",
            "params",
            "resMerge",
            "replaceStatus",
            "statusCode",
            "method",
            "cache",
            "attachment",
            "forwardedFor",
            "responseFor",
            "rulesFile",
            "resScript",
            "frameScript",
            "reqDelay",
            "resDelay",
            "headerReplace",
            "reqSpeed",
            "resSpeed",
            "reqType",
            "resType",
            "reqCharset",
            "resCharset",
            "reqCookies",
            "resCookies",
            "reqCors",
            "resCors",
            "reqHeaders",
            "resHeaders",
            "trailers",
            "reqPrepend",
            "resPrepend",
            "reqBody",
            "resBody",
            "reqAppend",
            "resAppend",
            "urlReplace",
            "reqReplace",
            "resReplace",
            "reqWrite",
            "resWrite",
            "reqWriteRaw",
            "resWriteRaw",
            "cssAppend",
            "htmlAppend",
            "jsAppend",
            "cssBody",
            "htmlBody",
            "jsBody",
            "cssPrepend",
            "htmlPrepend",
            "jsPrepend",
            "cipher",
            "sniCallback",
        ];

        for name in &protocol_names {
            let result = Protocol::parse(name);
            assert!(result.is_some(), "Failed to parse protocol: {}", name);
        }

        assert_eq!(protocol_names.len(), 74);
    }

    #[test]
    fn test_protocol_roundtrip() {
        for protocol in ALL_PROTOCOLS.iter() {
            let name = protocol.to_str();
            let parsed = Protocol::parse(name);
            assert_eq!(parsed, Some(*protocol), "Roundtrip failed for: {}", name);
        }
    }

    #[test]
    fn test_alias_resolution() {
        assert_eq!(Protocol::resolve_alias("ruleFile"), "rulesFile");
        assert_eq!(Protocol::resolve_alias("ruleScript"), "rulesFile");
        assert_eq!(Protocol::resolve_alias("rulesScript"), "rulesFile");
        assert_eq!(Protocol::resolve_alias("reqScript"), "rulesFile");
        assert_eq!(Protocol::resolve_alias("reqRules"), "rulesFile");
        assert_eq!(Protocol::resolve_alias("resRules"), "resScript");
        assert_eq!(Protocol::resolve_alias("pathReplace"), "urlReplace");
        assert_eq!(Protocol::resolve_alias("download"), "attachment");
        assert_eq!(Protocol::resolve_alias("skip"), "ignore");
        assert_eq!(Protocol::resolve_alias("http-proxy"), "proxy");
        assert_eq!(Protocol::resolve_alias("xhttp-proxy"), "xproxy");
        assert_eq!(Protocol::resolve_alias("status"), "statusCode");
        assert_eq!(Protocol::resolve_alias("hosts"), "host");
        assert_eq!(Protocol::resolve_alias("xhost"), "xhost");
        assert_eq!(Protocol::resolve_alias("html"), "htmlAppend");
        assert_eq!(Protocol::resolve_alias("js"), "jsAppend");
        assert_eq!(Protocol::resolve_alias("reqMerge"), "params");
        assert_eq!(Protocol::resolve_alias("tlsOptions"), "cipher");
        assert_eq!(Protocol::resolve_alias("css"), "cssAppend");
        assert_eq!(Protocol::resolve_alias("excludeFilter"), "filter");
        assert_eq!(Protocol::resolve_alias("includeFilter"), "filter");
        assert_eq!(Protocol::resolve_alias("P"), "G");
    }

    #[test]
    fn test_alias_parse() {
        let resolved = Protocol::resolve_alias("hosts");
        assert_eq!(Protocol::parse(resolved), Some(Protocol::Host));

        let resolved = Protocol::resolve_alias("skip");
        assert_eq!(Protocol::parse(resolved), Some(Protocol::Ignore));

        let resolved = Protocol::resolve_alias("download");
        assert_eq!(Protocol::parse(resolved), Some(Protocol::Attachment));

        let resolved = Protocol::resolve_alias("html");
        assert_eq!(Protocol::parse(resolved), Some(Protocol::HtmlAppend));

        let resolved = Protocol::resolve_alias("css");
        assert_eq!(Protocol::parse(resolved), Some(Protocol::CssAppend));

        let resolved = Protocol::resolve_alias("js");
        assert_eq!(Protocol::parse(resolved), Some(Protocol::JsAppend));
    }

    #[test]
    fn test_multi_match_protocols() {
        assert!(Protocol::G.is_multi_match());
        assert!(Protocol::Ignore.is_multi_match());
        assert!(Protocol::Enable.is_multi_match());
        assert!(Protocol::Filter.is_multi_match());
        assert!(Protocol::Disable.is_multi_match());
        assert!(Protocol::Plugin.is_multi_match());
        assert!(Protocol::Delete.is_multi_match());
        assert!(Protocol::Style.is_multi_match());
        assert!(Protocol::Cipher.is_multi_match());
        assert!(Protocol::Trailers.is_multi_match());
        assert!(Protocol::UrlParams.is_multi_match());
        assert!(Protocol::Params.is_multi_match());
        assert!(Protocol::HeaderReplace.is_multi_match());
        assert!(Protocol::ReqHeaders.is_multi_match());
        assert!(Protocol::ResHeaders.is_multi_match());
        assert!(Protocol::ReqCors.is_multi_match());
        assert!(Protocol::ResCors.is_multi_match());
        assert!(Protocol::ReqCookies.is_multi_match());
        assert!(Protocol::ResCookies.is_multi_match());
        assert!(Protocol::ReqReplace.is_multi_match());
        assert!(Protocol::UrlReplace.is_multi_match());
        assert!(Protocol::ResReplace.is_multi_match());
        assert!(Protocol::ResMerge.is_multi_match());
        assert!(Protocol::ReqBody.is_multi_match());
        assert!(Protocol::ReqPrepend.is_multi_match());
        assert!(Protocol::ResPrepend.is_multi_match());
        assert!(Protocol::ReqAppend.is_multi_match());
        assert!(Protocol::ResAppend.is_multi_match());
        assert!(Protocol::ResBody.is_multi_match());
        assert!(Protocol::HtmlAppend.is_multi_match());
        assert!(Protocol::JsAppend.is_multi_match());
        assert!(Protocol::CssAppend.is_multi_match());
        assert!(Protocol::HtmlBody.is_multi_match());
        assert!(Protocol::JsBody.is_multi_match());
        assert!(Protocol::CssBody.is_multi_match());
        assert!(Protocol::HtmlPrepend.is_multi_match());
        assert!(Protocol::JsPrepend.is_multi_match());
        assert!(Protocol::CssPrepend.is_multi_match());
        assert!(Protocol::RulesFile.is_multi_match());
        assert!(Protocol::ResScript.is_multi_match());

        assert!(!Protocol::Host.is_multi_match());
        assert!(!Protocol::Proxy.is_multi_match());
        assert!(!Protocol::Pac.is_multi_match());
        assert!(!Protocol::Method.is_multi_match());
        assert!(!Protocol::Auth.is_multi_match());
    }

    #[test]
    fn test_protocol_category_control() {
        assert_eq!(Protocol::Filter.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Ignore.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Enable.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Disable.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Delete.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::G.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Style.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Plugin.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Log.category(), ProtocolCategory::Control);
        assert_eq!(Protocol::Weinre.category(), ProtocolCategory::Control);
    }

    #[test]
    fn test_protocol_category_request() {
        assert_eq!(Protocol::ReqHeaders.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqBody.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqPrepend.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqAppend.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqCookies.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqCors.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqDelay.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqSpeed.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqType.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqCharset.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqReplace.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqWrite.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::ReqWriteRaw.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::Method.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::Auth.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::Ua.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::Referer.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::UrlParams.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::Params.category(), ProtocolCategory::Request);
        assert_eq!(Protocol::RulesFile.category(), ProtocolCategory::Request);
    }

    #[test]
    fn test_protocol_category_response() {
        assert_eq!(Protocol::ResHeaders.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResBody.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResPrepend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResAppend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResCookies.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResCors.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResDelay.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResSpeed.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResType.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResCharset.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResReplace.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResWrite.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResWriteRaw.category(), ProtocolCategory::Response);
        assert_eq!(
            Protocol::ReplaceStatus.category(),
            ProtocolCategory::Response
        );
        assert_eq!(Protocol::Cache.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::Attachment.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResponseFor.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::Trailers.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResMerge.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::HtmlAppend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::HtmlPrepend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::HtmlBody.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::JsAppend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::JsPrepend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::JsBody.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::CssAppend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::CssPrepend.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::CssBody.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::ResScript.category(), ProtocolCategory::Response);
        assert_eq!(Protocol::FrameScript.category(), ProtocolCategory::Response);
    }

    #[test]
    fn test_protocol_category_both() {
        assert_eq!(Protocol::Host.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Proxy.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Pac.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::InternalProxy.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Https2HttpProxy.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Http2HttpsProxy.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Rule.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Pipe.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::HeaderReplace.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::UrlReplace.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::SniCallback.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::Cipher.category(), ProtocolCategory::Both);
        assert_eq!(Protocol::ForwardedFor.category(), ProtocolCategory::Both);
    }

    #[test]
    fn test_is_res_protocol() {
        assert!(Protocol::ResHeaders.is_res_protocol());
        assert!(Protocol::ResBody.is_res_protocol());
        assert!(Protocol::ReplaceStatus.is_res_protocol());
        assert!(Protocol::Cache.is_res_protocol());
        assert!(Protocol::Filter.is_res_protocol());
        assert!(Protocol::Ignore.is_res_protocol());
        assert!(Protocol::Enable.is_res_protocol());
        assert!(Protocol::Disable.is_res_protocol());
        assert!(Protocol::Style.is_res_protocol());
        assert!(Protocol::Delete.is_res_protocol());
        assert!(Protocol::HeaderReplace.is_res_protocol());
    }

    #[test]
    fn test_is_req_protocol() {
        assert!(Protocol::ReqHeaders.is_req_protocol());
        assert!(Protocol::ReqBody.is_req_protocol());
        assert!(Protocol::Method.is_req_protocol());
        assert!(Protocol::Auth.is_req_protocol());
        assert!(Protocol::Ua.is_req_protocol());
        assert!(Protocol::Host.is_req_protocol());
        assert!(Protocol::Proxy.is_req_protocol());
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(format!("{}", Protocol::Host), "host");
        assert_eq!(format!("{}", Protocol::Proxy), "proxy");
        assert_eq!(format!("{}", Protocol::ReqHeaders), "reqHeaders");
        assert_eq!(format!("{}", Protocol::ResHeaders), "resHeaders");
        assert_eq!(format!("{}", Protocol::G), "G");
    }

    #[test]
    fn test_protocol_from_str_trait() {
        let result: Result<Protocol, _> = "host".parse();
        assert_eq!(result, Ok(Protocol::Host));

        let result: Result<Protocol, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_protocol_returns_none() {
        assert!(Protocol::parse("unknown").is_none());
        assert!(Protocol::parse("").is_none());
        assert!(Protocol::parse("HOST").is_none());
    }

    #[test]
    fn test_all_protocols_function() {
        let all = Protocol::all();
        assert_eq!(all.len(), 74);
        assert!(all.contains(&Protocol::Host));
        assert!(all.contains(&Protocol::Proxy));
        assert!(all.contains(&Protocol::G));
    }

    #[test]
    fn test_protocol_aliases_count() {
        assert_eq!(PROTOCOL_ALIASES.len(), 21);
    }

    #[test]
    fn test_multi_match_protocols_count() {
        assert_eq!(MULTI_MATCH_PROTOCOLS.len(), 40);
    }
}

mod detect;
mod h2;
mod http1;
mod pool;
pub mod quic;
mod sse;
mod stream;
mod websocket;

pub use detect::*;
pub use h2::*;
pub use http1::*;
pub use pool::*;
pub use quic::QuicPacketDetector;
pub use sse::*;
pub use stream::*;
pub use websocket::*;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bifrost_core::Result;
use tokio::net::TcpStream;

use crate::server::RulesResolver;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u8);

impl Priority {
    pub const HIGHEST: Priority = Priority(0);
    pub const HIGH: Priority = Priority(25);
    pub const NORMAL: Priority = Priority(50);
    pub const LOW: Priority = Priority(75);
    pub const LOWEST: Priority = Priority(100);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionResult {
    Match(Priority),
    NotMatch,
    NeedMoreData(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocol {
    Http1,
    Http2,
    WebSocket,
    Sse,
    Grpc,
    Socks5,
    Socks4,
    Tls,
    Raw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    Json,
    Html,
    Xml,
    PlainText,
    EventStream,
    OctetStream,
    FormData,
    FormUrlEncoded,
    Other,
}

impl ContentType {
    pub fn from_header(value: &str) -> Self {
        let lower = value.to_lowercase();
        if lower.contains("application/json") {
            ContentType::Json
        } else if lower.contains("text/html") {
            ContentType::Html
        } else if lower.contains("text/xml") || lower.contains("application/xml") {
            ContentType::Xml
        } else if lower.contains("text/plain") {
            ContentType::PlainText
        } else if lower.contains("text/event-stream") {
            ContentType::EventStream
        } else if lower.contains("application/octet-stream") {
            ContentType::OctetStream
        } else if lower.contains("multipart/form-data") {
            ContentType::FormData
        } else if lower.contains("application/x-www-form-urlencoded") {
            ContentType::FormUrlEncoded
        } else {
            ContentType::Other
        }
    }
}

pub struct ProxyContext {
    pub rules: Arc<dyn RulesResolver>,
    pub enable_tls_interception: bool,
    pub detected_protocol: Option<TransportProtocol>,
}

impl ProxyContext {
    pub fn new(rules: Arc<dyn RulesResolver>) -> Self {
        Self {
            rules,
            enable_tls_interception: false,
            detected_protocol: None,
        }
    }

    pub fn with_tls_interception(mut self, enabled: bool) -> Self {
        self.enable_tls_interception = enabled;
        self
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait ProtocolHandler: Send + Sync {
    fn name(&self) -> &'static str;

    fn detect(&self, data: &[u8]) -> DetectionResult;

    fn handle(
        &self,
        stream: TcpStream,
        ctx: ProxyContext,
        initial_data: Option<Vec<u8>>,
    ) -> BoxFuture<'_, Result<()>>;

    fn priority(&self) -> Priority {
        Priority::NORMAL
    }
}

pub struct ProtocolRegistry {
    handlers: Vec<Arc<dyn ProtocolHandler>>,
}

impl ProtocolRegistry {
    pub fn new() -> Self {
        Self { handlers: vec![] }
    }

    pub fn register(&mut self, handler: Arc<dyn ProtocolHandler>) {
        self.handlers.push(handler);
        self.handlers
            .sort_by_key(|h| std::cmp::Reverse(h.priority()));
    }

    pub fn detect(&self, data: &[u8]) -> Option<(Arc<dyn ProtocolHandler>, Priority)> {
        let mut best_match: Option<(Arc<dyn ProtocolHandler>, Priority)> = None;

        for handler in &self.handlers {
            match handler.detect(data) {
                DetectionResult::Match(priority) => {
                    if best_match.is_none()
                        || priority
                            < best_match
                                .as_ref()
                                .map(|(_, p)| *p)
                                .unwrap_or(Priority::LOWEST)
                    {
                        best_match = Some((Arc::clone(handler), priority));
                    }
                }
                DetectionResult::NotMatch => continue,
                DetectionResult::NeedMoreData(_) => continue,
            }
        }

        best_match
    }

    pub fn handlers(&self) -> &[Arc<dyn ProtocolHandler>] {
        &self.handlers
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::HIGHEST < Priority::HIGH);
        assert!(Priority::HIGH < Priority::NORMAL);
        assert!(Priority::NORMAL < Priority::LOW);
        assert!(Priority::LOW < Priority::LOWEST);
    }

    #[test]
    fn test_content_type_from_header() {
        assert_eq!(
            ContentType::from_header("application/json"),
            ContentType::Json
        );
        assert_eq!(
            ContentType::from_header("application/json; charset=utf-8"),
            ContentType::Json
        );
        assert_eq!(ContentType::from_header("text/html"), ContentType::Html);
        assert_eq!(
            ContentType::from_header("text/event-stream"),
            ContentType::EventStream
        );
        assert_eq!(
            ContentType::from_header("text/plain"),
            ContentType::PlainText
        );
        assert_eq!(
            ContentType::from_header("multipart/form-data"),
            ContentType::FormData
        );
        assert_eq!(
            ContentType::from_header("application/x-www-form-urlencoded"),
            ContentType::FormUrlEncoded
        );
        assert_eq!(
            ContentType::from_header("application/unknown"),
            ContentType::Other
        );
    }

    #[test]
    fn test_detection_result() {
        let match_result = DetectionResult::Match(Priority::HIGH);
        let not_match = DetectionResult::NotMatch;
        let need_more = DetectionResult::NeedMoreData(10);

        assert_eq!(match_result, DetectionResult::Match(Priority::HIGH));
        assert_eq!(not_match, DetectionResult::NotMatch);
        assert_eq!(need_more, DetectionResult::NeedMoreData(10));
    }

    #[test]
    fn test_protocol_registry() {
        let registry = ProtocolRegistry::new();
        assert!(registry.handlers().is_empty());
    }
}

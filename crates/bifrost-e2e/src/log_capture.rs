use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

#[derive(Debug, Clone)]
pub struct RuleExecutionLog {
    pub timestamp: Instant,
    pub url: String,
    pub protocol: String,
    pub value: String,
    pub message: String,
}

#[derive(Default)]
struct LogVisitor {
    url: Option<String>,
    protocol: Option<String>,
    value: Option<String>,
    message: Option<String>,
}

impl Visit for LogVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{:?}", value);
        match field.name() {
            "url" => self.url = Some(value_str.trim_matches('"').to_string()),
            "protocol" => self.protocol = Some(value_str.trim_matches('"').to_string()),
            "value" => self.value = Some(value_str.trim_matches('"').to_string()),
            "message" => self.message = Some(value_str.trim_matches('"').to_string()),
            _ => {}
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "url" => self.url = Some(value.to_string()),
            "protocol" => self.protocol = Some(value.to_string()),
            "value" => self.value = Some(value.to_string()),
            "message" => self.message = Some(value.to_string()),
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct LogCapture {
    logs: Arc<RwLock<Vec<RuleExecutionLog>>>,
    raw_logs: Arc<RwLock<Vec<String>>>,
}

impl Default for LogCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl LogCapture {
    pub fn new() -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            raw_logs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn get_logs(&self) -> Vec<RuleExecutionLog> {
        self.logs.read().clone()
    }

    pub fn get_raw_logs(&self) -> Vec<String> {
        self.raw_logs.read().clone()
    }

    pub fn clear(&self) {
        self.logs.write().clear();
        self.raw_logs.write().clear();
    }

    pub fn filter_by_protocol(&self, protocol: &str) -> Vec<RuleExecutionLog> {
        self.logs
            .read()
            .iter()
            .filter(|log| log.protocol.to_lowercase() == protocol.to_lowercase())
            .cloned()
            .collect()
    }

    pub fn filter_by_url_contains(&self, substring: &str) -> Vec<RuleExecutionLog> {
        self.logs
            .read()
            .iter()
            .filter(|log| log.url.contains(substring))
            .cloned()
            .collect()
    }

    pub fn assert_rule_applied(&self, url_pattern: &str, protocol: &str) -> Result<(), String> {
        let logs = self.logs.read();
        for log in logs.iter() {
            if log.url.contains(url_pattern)
                && log.protocol.to_lowercase() == protocol.to_lowercase()
            {
                return Ok(());
            }
        }

        let available: Vec<String> = logs
            .iter()
            .map(|l| format!("{}:{}", l.protocol, l.url))
            .collect();

        Err(format!(
            "Rule '{}' for protocol '{}' was not applied. Available logs: {:?}",
            url_pattern, protocol, available
        ))
    }

    pub fn assert_log_contains(&self, substring: &str) -> Result<(), String> {
        let raw_logs = self.raw_logs.read();
        for log in raw_logs.iter() {
            if log.contains(substring) {
                return Ok(());
            }
        }

        Err(format!(
            "No log containing '{}' found. Total logs: {}",
            substring,
            raw_logs.len()
        ))
    }

    pub fn print_all_logs(&self) {
        println!("\n=== Captured Logs ===");
        for (i, log) in self.raw_logs.read().iter().enumerate() {
            println!("[{}] {}", i, log);
        }
        println!("=== End Logs ===\n");
    }

    fn record_log(&self, log: RuleExecutionLog) {
        self.logs.write().push(log);
    }

    fn record_raw(&self, message: String) {
        self.raw_logs.write().push(message);
    }
}

pub struct LogCaptureLayer {
    capture: LogCapture,
}

impl LogCaptureLayer {
    pub fn new(capture: LogCapture) -> Self {
        Self { capture }
    }
}

impl<S> Layer<S> for LogCaptureLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        let message = visitor.message.clone().unwrap_or_default();
        let raw_log = format!("[{}] {} - {}", metadata.level(), metadata.target(), message);

        self.capture.record_raw(raw_log);

        if message.contains("matched:") || message.contains("RULES") {
            let log = RuleExecutionLog {
                timestamp: Instant::now(),
                url: visitor.url.unwrap_or_default(),
                protocol: visitor.protocol.unwrap_or_default(),
                value: visitor.value.unwrap_or_default(),
                message,
            };
            self.capture.record_log(log);
        }
    }
}

pub fn init_log_capture() -> LogCapture {
    let capture = LogCapture::new();
    let layer = LogCaptureLayer::new(capture.clone());

    use tracing_subscriber::prelude::*;
    let _ = tracing_subscriber::registry().with(layer).try_init();

    capture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_capture_basic() {
        let capture = LogCapture::new();
        assert!(capture.get_logs().is_empty());

        capture.record_log(RuleExecutionLog {
            timestamp: Instant::now(),
            url: "http://example.com".to_string(),
            protocol: "host".to_string(),
            value: "127.0.0.1".to_string(),
            message: "rule applied".to_string(),
        });

        assert_eq!(capture.get_logs().len(), 1);
    }

    #[test]
    fn test_log_capture_filter() {
        let capture = LogCapture::new();

        capture.record_log(RuleExecutionLog {
            timestamp: Instant::now(),
            url: "http://example.com".to_string(),
            protocol: "host".to_string(),
            value: "127.0.0.1".to_string(),
            message: "rule applied".to_string(),
        });

        capture.record_log(RuleExecutionLog {
            timestamp: Instant::now(),
            url: "http://example.com".to_string(),
            protocol: "reqHeaders".to_string(),
            value: "X-Test: 1".to_string(),
            message: "rule applied".to_string(),
        });

        let host_logs = capture.filter_by_protocol("host");
        assert_eq!(host_logs.len(), 1);

        let header_logs = capture.filter_by_protocol("reqHeaders");
        assert_eq!(header_logs.len(), 1);
    }

    #[test]
    fn test_assert_rule_applied() {
        let capture = LogCapture::new();

        capture.record_log(RuleExecutionLog {
            timestamp: Instant::now(),
            url: "http://baidu.com/test".to_string(),
            protocol: "host".to_string(),
            value: "127.0.0.1".to_string(),
            message: "rule applied".to_string(),
        });

        assert!(capture.assert_rule_applied("baidu.com", "host").is_ok());
        assert!(capture.assert_rule_applied("google.com", "host").is_err());
        assert!(capture
            .assert_rule_applied("baidu.com", "reqHeaders")
            .is_err());
    }
}

use std::path::PathBuf;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::{BifrostError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LogOutput {
    Console,
    File,
}

impl LogOutput {
    pub fn parse(s: &str) -> Vec<LogOutput> {
        s.split(',')
            .filter_map(|part| match part.trim().to_lowercase().as_str() {
                "console" => Some(LogOutput::Console),
                "file" => Some(LogOutput::File),
                "both" => None,
                _ => None,
            })
            .collect::<Vec<_>>()
            .into_iter()
            .chain(if s.to_lowercase().contains("both") {
                vec![LogOutput::Console, LogOutput::File]
            } else {
                vec![]
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct LogConfig {
    pub level: String,
    pub outputs: Vec<LogOutput>,
    pub log_dir: PathBuf,
    pub retention_days: u32,
    pub file_prefix: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            outputs: vec![LogOutput::Console, LogOutput::File],
            log_dir: PathBuf::from("."),
            retention_days: 7,
            file_prefix: "bifrost".to_string(),
        }
    }
}

impl LogConfig {
    pub fn new(level: String, log_dir: PathBuf) -> Self {
        Self {
            level,
            log_dir,
            ..Default::default()
        }
    }

    pub fn with_outputs(mut self, outputs: Vec<LogOutput>) -> Self {
        self.outputs = outputs;
        self
    }

    pub fn with_retention_days(mut self, days: u32) -> Self {
        self.retention_days = days;
        self
    }
}

pub struct LogGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

fn build_env_filter(level: &str) -> Result<EnvFilter> {
    if std::env::var("RUST_LOG").is_ok() {
        Ok(EnvFilter::from_default_env())
    } else {
        EnvFilter::try_new(level)
            .map_err(|e| BifrostError::Config(format!("Invalid log level '{}': {}", level, e)))
    }
}

fn cleanup_old_logs(log_dir: &std::path::Path, prefix: &str, retention_days: u32) -> Result<()> {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Failed to read log directory for cleanup: {}", e);
            return Ok(());
        }
    };

    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);

    for entry in entries.flatten() {
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(prefix) && name.ends_with(".log") {
                if let Some(date_str) = extract_date_from_filename(name, prefix) {
                    if let Ok(file_date) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                    {
                        if file_date < cutoff.date_naive() {
                            tracing::info!("Removing old log file: {}", name);
                            if let Err(e) = std::fs::remove_file(&path) {
                                tracing::warn!("Failed to remove old log file {}: {}", name, e);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn extract_date_from_filename(filename: &str, prefix: &str) -> Option<String> {
    let without_prefix = filename.strip_prefix(prefix)?;
    let without_dot = without_prefix.strip_prefix('.')?;
    let date_part = without_dot.strip_suffix(".log")?;
    Some(date_part.to_string())
}

pub fn init_logging(level: &str) -> Result<()> {
    let filter = build_env_filter(level)?;

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .try_init()
        .map_err(|e| BifrostError::Config(format!("Failed to initialize logging: {}", e)))?;

    Ok(())
}

pub fn init_logging_with_config(config: &LogConfig) -> Result<LogGuard> {
    let filter = build_env_filter(&config.level)?;

    let has_console = config.outputs.contains(&LogOutput::Console);
    let has_file = config.outputs.contains(&LogOutput::File);

    let mut file_guard: Option<tracing_appender::non_blocking::WorkerGuard> = None;

    if has_file {
        std::fs::create_dir_all(&config.log_dir).map_err(|e| {
            BifrostError::Config(format!(
                "Failed to create log directory '{}': {}",
                config.log_dir.display(),
                e
            ))
        })?;
    }

    match (has_console, has_file) {
        (true, true) => {
            let file_appender = RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix(&config.file_prefix)
                .filename_suffix("log")
                .max_log_files(config.retention_days as usize)
                .build(&config.log_dir)
                .map_err(|e| {
                    BifrostError::Config(format!("Failed to create file appender: {}", e))
                })?;

            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            file_guard = Some(guard);

            let console_layer = fmt::layer()
                .with_target(true)
                .with_file(true)
                .with_line_number(true);

            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(console_layer)
                .with(file_layer)
                .try_init()
                .map_err(|e| {
                    BifrostError::Config(format!("Failed to initialize logging: {}", e))
                })?;

            if let Err(e) =
                cleanup_old_logs(&config.log_dir, &config.file_prefix, config.retention_days)
            {
                tracing::warn!("Failed to cleanup old logs: {}", e);
            }
        }
        (true, false) => {
            let console_layer = fmt::layer()
                .with_target(true)
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(console_layer)
                .try_init()
                .map_err(|e| {
                    BifrostError::Config(format!("Failed to initialize logging: {}", e))
                })?;
        }
        (false, true) => {
            let file_appender = RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix(&config.file_prefix)
                .filename_suffix("log")
                .max_log_files(config.retention_days as usize)
                .build(&config.log_dir)
                .map_err(|e| {
                    BifrostError::Config(format!("Failed to create file appender: {}", e))
                })?;

            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            file_guard = Some(guard);

            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(file_layer)
                .try_init()
                .map_err(|e| {
                    BifrostError::Config(format!("Failed to initialize logging: {}", e))
                })?;

            if let Err(e) =
                cleanup_old_logs(&config.log_dir, &config.file_prefix, config.retention_days)
            {
                tracing::warn!("Failed to cleanup old logs: {}", e);
            }
        }
        (false, false) => {
            return Err(BifrostError::Config(
                "At least one log output (console or file) must be specified".to_string(),
            ));
        }
    }

    Ok(LogGuard {
        _file_guard: file_guard,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_output_parse() {
        let outputs = LogOutput::parse("console");
        assert!(outputs.contains(&LogOutput::Console));
        assert!(!outputs.contains(&LogOutput::File));

        let outputs = LogOutput::parse("file");
        assert!(!outputs.contains(&LogOutput::Console));
        assert!(outputs.contains(&LogOutput::File));

        let outputs = LogOutput::parse("console,file");
        assert!(outputs.contains(&LogOutput::Console));
        assert!(outputs.contains(&LogOutput::File));

        let outputs = LogOutput::parse("both");
        assert!(outputs.contains(&LogOutput::Console));
        assert!(outputs.contains(&LogOutput::File));
    }

    #[test]
    fn test_extract_date_from_filename() {
        assert_eq!(
            extract_date_from_filename("bifrost.2026-02-22.log", "bifrost"),
            Some("2026-02-22".to_string())
        );
        assert_eq!(
            extract_date_from_filename("bifrost.2026-01-01.log", "bifrost"),
            Some("2026-01-01".to_string())
        );
        assert_eq!(
            extract_date_from_filename("other.2026-02-22.log", "bifrost"),
            None
        );
        assert_eq!(extract_date_from_filename("bifrost.log", "bifrost"), None);
    }
}

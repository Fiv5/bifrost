use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Plugin not running: {0}")]
    NotRunning(String),

    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Plugin configuration error: {0}")]
    Config(String),

    #[error("Plugin communication error: {0}")]
    Communication(String),

    #[error("Plugin hook error: {0}")]
    Hook(String),

    #[error("Plugin timeout: {0}")]
    Timeout(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] hyper::Error),

    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] hyper::http::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, PluginError>;

impl PluginError {
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            PluginError::Communication(_) | PluginError::Timeout(_) | PluginError::Io(_)
        )
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, PluginError::NotFound(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PluginError::NotFound("test-plugin".to_string());
        assert_eq!(err.to_string(), "Plugin not found: test-plugin");
    }

    #[test]
    fn test_error_is_retriable() {
        assert!(PluginError::Communication("error".to_string()).is_retriable());
        assert!(PluginError::Timeout("error".to_string()).is_retriable());
        assert!(!PluginError::NotFound("error".to_string()).is_retriable());
        assert!(!PluginError::Config("error".to_string()).is_retriable());
    }

    #[test]
    fn test_error_is_not_found() {
        assert!(PluginError::NotFound("test".to_string()).is_not_found());
        assert!(!PluginError::NotRunning("test".to_string()).is_not_found());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let plugin_err: PluginError = io_err.into();
        assert!(matches!(plugin_err, PluginError::Io(_)));
        assert!(plugin_err.is_retriable());
    }

    #[test]
    fn test_json_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let plugin_err: PluginError = json_err.into();
        assert!(matches!(plugin_err, PluginError::Json(_)));
    }
}

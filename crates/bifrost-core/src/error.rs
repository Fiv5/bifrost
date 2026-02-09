use std::fmt;

#[derive(Debug)]
pub enum BifrostError {
    Config(String),
    Parse(String),
    Rule(String),
    Proxy(String),
    Plugin(String),
    Tls(String),
    Io(std::io::Error),
    Network(String),
    NotFound(String),
    Storage(String),
}

impl fmt::Display for BifrostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BifrostError::Config(msg) => write!(f, "Config error: {}", msg),
            BifrostError::Parse(msg) => write!(f, "Parse error: {}", msg),
            BifrostError::Rule(msg) => write!(f, "Rule error: {}", msg),
            BifrostError::Proxy(msg) => write!(f, "Proxy error: {}", msg),
            BifrostError::Plugin(msg) => write!(f, "Plugin error: {}", msg),
            BifrostError::Tls(msg) => write!(f, "TLS error: {}", msg),
            BifrostError::Io(err) => write!(f, "IO error: {}", err),
            BifrostError::Network(msg) => write!(f, "Network error: {}", msg),
            BifrostError::NotFound(msg) => write!(f, "Not found: {}", msg),
            BifrostError::Storage(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for BifrostError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BifrostError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BifrostError {
    fn from(err: std::io::Error) -> Self {
        BifrostError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, BifrostError>;

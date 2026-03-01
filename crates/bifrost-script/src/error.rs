use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScriptError {
    #[error("Script not found: {0}")]
    NotFound(String),

    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Script timeout after {0}ms")]
    Timeout(u64),

    #[error("Script syntax error: {0}")]
    SyntaxError(String),

    #[error("Script runtime error: {0}")]
    RuntimeError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid script name: {0}")]
    InvalidName(String),

    #[error("QuickJS error: {0}")]
    QuickJsError(String),
}

pub type Result<T> = std::result::Result<T, ScriptError>;

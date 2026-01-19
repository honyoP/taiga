use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaigaError {
    #[error("Task #{0} not found")]
    TaskNotFound(u32),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Daemon error: {0}")]
    Daemon(String),
}

impl From<confy::ConfyError> for TaigaError {
    fn from(err: confy::ConfyError) -> Self {
        TaigaError::Config(err.to_string())
    }
}

impl From<serde_json::Error> for TaigaError {
    fn from(err: serde_json::Error) -> Self {
        TaigaError::Parse(err.to_string())
    }
}

impl From<std::num::ParseIntError> for TaigaError {
    fn from(err: std::num::ParseIntError) -> Self {
        TaigaError::Parse(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, TaigaError>;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaigaError {
    #[error("Task #{0} not found")]
    TaskNotFound(u32),

    #[error("Configuration error: {message}")]
    Config {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("IO error: {context}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Parse error: {message}")]
    Parse {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("IPC error: {message}")]
    Ipc {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Daemon error: {message}")]
    Daemon {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Plugin error: {message}")]
    Plugin {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },
}

impl TaigaError {
    /// Create a config error with a message
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            source: None,
        }
    }

    /// Create a config error with source
    pub fn config_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Config {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a parse error with a message
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Parse {
            message: message.into(),
            source: None,
        }
    }

    /// Create a parse error with source
    pub fn parse_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Parse {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create an IPC error with a message
    pub fn ipc(message: impl Into<String>) -> Self {
        Self::Ipc {
            message: message.into(),
            source: None,
        }
    }

    /// Create an IPC error with source
    pub fn ipc_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Ipc {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a daemon error with a message
    pub fn daemon(message: impl Into<String>) -> Self {
        Self::Daemon {
            message: message.into(),
            source: None,
        }
    }

    /// Create a daemon error with source
    pub fn daemon_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Daemon {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a plugin error with a message
    pub fn plugin(message: impl Into<String>) -> Self {
        Self::Plugin {
            message: message.into(),
            source: None,
        }
    }

    /// Create a plugin error with source
    pub fn plugin_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::Plugin {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a validation error
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create an IO error with context
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

impl From<std::io::Error> for TaigaError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            context: "IO operation failed".to_string(),
            source: err,
        }
    }
}

impl From<confy::ConfyError> for TaigaError {
    fn from(err: confy::ConfyError) -> Self {
        Self::config_with_source("Failed to load configuration", err)
    }
}

impl From<serde_json::Error> for TaigaError {
    fn from(err: serde_json::Error) -> Self {
        Self::parse_with_source("JSON parsing failed", err)
    }
}

impl From<std::num::ParseIntError> for TaigaError {
    fn from(err: std::num::ParseIntError) -> Self {
        Self::parse_with_source("Integer parsing failed", err)
    }
}

pub type Result<T> = std::result::Result<T, TaigaError>;

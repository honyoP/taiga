//! Core error types for Taiga domain logic
//!
//! These errors represent domain-level failures, not I/O or CLI errors.

use thiserror::Error;

/// Core domain errors
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Task #{0} not found")]
    TaskNotFound(u32),

    #[error("Parse error: {message}")]
    Parse {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },
}

impl CoreError {
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

    /// Create a validation error
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }
}

/// Result type for core operations
pub type Result<T> = std::result::Result<T, CoreError>;

//! Taiga Core - Pure domain logic for task management
//!
//! This crate contains no I/O operations. All persistence
//! is handled by adapters in consuming crates.

pub mod date;
pub mod error;
pub mod filter;
pub mod task;

pub use error::{CoreError, Result};
pub use filter::{TaskFilter, TaskSort};
pub use task::{Task, TaskCollection, TaskId};

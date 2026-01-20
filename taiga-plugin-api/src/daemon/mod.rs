//! Daemon infrastructure for plugin development
//!
//! This module provides utilities for creating daemon-based plugins with IPC communication.
//! It offers three levels of abstraction:
//!
//! 1. **Low-level utilities** (`socket`, `ipc`) - Direct control over sockets and IPC
//! 2. **Mid-level client helper** (`client`) - Automatic retry-with-autospawn pattern
//! 3. **High-level traits** (future) - Structured daemon framework
//!
//! # Example: Using the client helper
//!
//! ```rust,ignore
//! use taiga_plugin_api::daemon::{
//!     client::{DaemonClientConfig, send_command_with_autospawn},
//!     ipc::DaemonSpawnConfig,
//! };
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize)]
//! struct MyCommand { action: String }
//!
//! #[derive(Deserialize)]
//! struct MyResponse { result: String }
//!
//! async fn send_command() -> Result<MyResponse, PluginError> {
//!     let config = DaemonClientConfig::new(
//!         "/tmp/my-plugin.sock",
//!         DaemonSpawnConfig::new("my-plugin", "daemon"),
//!     );
//!
//!     let cmd = MyCommand { action: "test".into() };
//!     send_command_with_autospawn(&config, &cmd).await
//! }
//! ```

pub mod socket;
pub mod ipc;
pub mod client;

// Re-export commonly used types
pub use socket::{create_listener, connect, cleanup_socket};
pub use ipc::{send_message, receive_message, spawn_daemon_process, DaemonSpawnConfig};
pub use client::{send_command_with_autospawn, DaemonClientConfig};

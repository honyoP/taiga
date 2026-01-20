//! IPC message utilities for daemon-based plugins
//!
//! Provides generic serialization patterns and daemon process management.

use interprocess::local_socket::tokio::Stream;
use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use crate::PluginError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Send a serialized message over an IPC stream
pub async fn send_message<T: Serialize>(
    stream: &mut Stream,
    msg: &T,
) -> Result<(), PluginError> {
    let bytes = serde_json::to_vec(msg)?;
    stream.write_all(&bytes).await?;
    Ok(())
}

/// Receive and deserialize a message from an IPC stream
///
/// # Arguments
/// * `stream` - The IPC stream to read from
/// * `buffer_size` - Size of the buffer to allocate for reading
pub async fn receive_message<T: for<'de> Deserialize<'de>>(
    stream: &mut Stream,
    buffer_size: usize,
) -> Result<T, PluginError> {
    let mut buffer = vec![0u8; buffer_size];
    let n = stream.read(&mut buffer).await?;

    if n == 0 {
        return Err(PluginError::ipc_connection(
            "Connection closed without response",
        ));
    }

    let msg: T = serde_json::from_slice(&buffer[0..n])?;
    Ok(msg)
}

/// Configuration for spawning a daemon process
#[derive(Debug, Clone)]
pub struct DaemonSpawnConfig {
    /// Plugin name (used as first argument)
    pub plugin_name: String,
    /// Daemon command (used as second argument)
    pub daemon_command: String,
    /// Additional arguments to pass to the daemon
    pub additional_args: Vec<String>,
}

impl DaemonSpawnConfig {
    /// Create a new daemon spawn configuration
    pub fn new(plugin_name: impl Into<String>, daemon_command: impl Into<String>) -> Self {
        Self {
            plugin_name: plugin_name.into(),
            daemon_command: daemon_command.into(),
            additional_args: Vec::new(),
        }
    }

    /// Add additional arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.additional_args = args;
        self
    }
}

/// Spawn the daemon as a detached background process
///
/// # Arguments
/// * `config` - Configuration for the daemon process
///
/// Returns the spawned child process handle, or an error if spawning fails.
pub fn spawn_daemon_process(config: &DaemonSpawnConfig) -> Result<Child, std::io::Error> {
    let current_exe = std::env::current_exe()?;

    let mut args = vec![config.plugin_name.clone(), config.daemon_command.clone()];
    args.extend(config.additional_args.clone());

    Command::new(&current_exe)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to spawn daemon process '{}': {}",
                    current_exe.display(),
                    e
                ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_spawn_config_creation() {
        let config = DaemonSpawnConfig::new("test", "daemon");
        assert_eq!(config.plugin_name, "test");
        assert_eq!(config.daemon_command, "daemon");
        assert!(config.additional_args.is_empty());
    }

    #[test]
    fn test_daemon_spawn_config_with_args() {
        let config = DaemonSpawnConfig::new("test", "daemon")
            .with_args(vec!["--verbose".to_string()]);
        assert_eq!(config.additional_args.len(), 1);
    }
}

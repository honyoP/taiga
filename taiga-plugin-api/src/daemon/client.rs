//! Generic client helper for daemon-based plugins
//!
//! Provides a retry-with-autospawn pattern for connecting to daemon processes.

use super::ipc::{receive_message, send_message, spawn_daemon_process, DaemonSpawnConfig};
use super::socket;
use crate::PluginError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the daemon client
#[derive(Debug, Clone)]
pub struct DaemonClientConfig {
    /// Path to the socket file
    pub socket_path: String,
    /// Configuration for spawning the daemon
    pub daemon_spawn: DaemonSpawnConfig,
    /// Time to wait after starting daemon before retrying connection (milliseconds)
    pub startup_wait_ms: u64,
    /// Buffer size for IPC messages
    pub buffer_size: usize,
}

impl DaemonClientConfig {
    /// Create a new daemon client configuration
    pub fn new(
        socket_path: impl Into<String>,
        daemon_spawn: DaemonSpawnConfig,
    ) -> Self {
        Self {
            socket_path: socket_path.into(),
            daemon_spawn,
            startup_wait_ms: 500,
            buffer_size: 1024,
        }
    }

    /// Set the startup wait time
    pub fn with_startup_wait(mut self, wait_ms: u64) -> Self {
        self.startup_wait_ms = wait_ms;
        self
    }

    /// Set the buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

/// Send a command to the daemon with automatic spawning if not running
///
/// This implements the retry-with-autospawn pattern:
/// 1. Try to connect to the daemon
/// 2. If connection fails, spawn the daemon
/// 3. Wait for startup, then retry connection
/// 4. Send command and receive response
///
/// # Type Parameters
/// * `Cmd` - The command type (must be serializable)
/// * `Resp` - The response type (must be deserializable)
///
/// # Arguments
/// * `config` - Configuration for the daemon client
/// * `command` - The command to send
///
/// # Returns
/// The response from the daemon, or an error if communication fails
pub async fn send_command_with_autospawn<Cmd, Resp>(
    config: &DaemonClientConfig,
    command: &Cmd,
) -> Result<Resp, PluginError>
where
    Cmd: Serialize,
    Resp: for<'de> Deserialize<'de>,
{
    let stream_result = socket::connect(&config.socket_path).await;

    let mut stream = match stream_result {
        Ok(s) => s,
        Err(conn_err) => {
            // Daemon not running, try to start it
            println!("Daemon not running. Starting it...");
            spawn_daemon_process(&config.daemon_spawn)
                .map_err(PluginError::daemon_not_running_with_source)?;

            // Wait for daemon to start
            tokio::time::sleep(Duration::from_millis(config.startup_wait_ms)).await;

            // Retry connection
            socket::connect(&config.socket_path).await.map_err(|_| {
                // Daemon was started but we still can't connect - include original error
                PluginError::ipc_connection_with_source(
                    "Failed to connect after starting daemon",
                    conn_err,
                )
            })?
        }
    };

    // Send command and receive response
    send_message(&mut stream, command).await?;
    let resp: Resp = receive_message(&mut stream, config.buffer_size).await?;

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_client_config_creation() {
        let spawn_config = DaemonSpawnConfig::new("test", "daemon");
        let client_config = DaemonClientConfig::new("/tmp/test.sock", spawn_config);
        assert_eq!(client_config.socket_path, "/tmp/test.sock");
        assert_eq!(client_config.startup_wait_ms, 500);
        assert_eq!(client_config.buffer_size, 1024);
    }

    #[test]
    fn test_daemon_client_config_with_options() {
        let spawn_config = DaemonSpawnConfig::new("test", "daemon");
        let client_config = DaemonClientConfig::new("/tmp/test.sock", spawn_config)
            .with_startup_wait(1000)
            .with_buffer_size(2048);
        assert_eq!(client_config.startup_wait_ms, 1000);
        assert_eq!(client_config.buffer_size, 2048);
    }
}

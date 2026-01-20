//! Generic daemon framework traits and runner
//!
//! Provides a structured way to implement daemon-based plugins with minimal boilerplate.
//!
//! # Example
//!
//! ```rust,ignore
//! use taiga_plugin_api::daemon::traits::{DaemonHandler, DaemonConfig, run_daemon_loop};
//! use async_trait::async_trait;
//!
//! struct MyDaemon {
//!     counter: u32,
//! }
//!
//! #[async_trait]
//! impl DaemonHandler for MyDaemon {
//!     type Command = MyCommand;
//!     type Response = MyResponse;
//!
//!     async fn handle_command(&mut self, cmd: Self::Command) -> Self::Response {
//!         match cmd {
//!             MyCommand::Increment => {
//!                 self.counter += 1;
//!                 MyResponse::Ok(self.counter)
//!             }
//!         }
//!     }
//!
//!     async fn on_tick(&mut self) {
//!         // Called periodically
//!     }
//! }
//!
//! // In your daemon entry point:
//! let config = DaemonConfig::new("/tmp/my-plugin.sock");
//! run_daemon_loop(config, MyDaemon { counter: 0 }).await?;
//! ```

use super::ipc::{receive_message, send_message};
use super::socket;
use crate::PluginError;
use async_trait::async_trait;
use interprocess::local_socket::tokio::Stream as LocalSocketStream;
use interprocess::local_socket::traits::tokio::Listener as _;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;

/// Configuration for running a daemon
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Path to the socket file
    pub socket_path: String,
    /// Interval between tick calls (in seconds)
    pub tick_interval_secs: u64,
    /// Buffer size for IPC messages
    pub buffer_size: usize,
}

impl DaemonConfig {
    /// Create a new daemon configuration with the given socket path
    pub fn new(socket_path: impl Into<String>) -> Self {
        Self {
            socket_path: socket_path.into(),
            tick_interval_secs: 1,
            buffer_size: 1024,
        }
    }

    /// Set the tick interval
    pub fn with_tick_interval(mut self, secs: u64) -> Self {
        self.tick_interval_secs = secs;
        self
    }

    /// Set the buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

/// Result of handling a command
#[derive(Debug, Clone)]
pub enum HandleResult<R> {
    /// Send the response and continue
    Response(R),
    /// Send the response and shut down the daemon
    Shutdown(R),
}

impl<R> HandleResult<R> {
    /// Create a response that continues the daemon
    pub fn response(r: R) -> Self {
        HandleResult::Response(r)
    }

    /// Create a response that shuts down the daemon
    pub fn shutdown(r: R) -> Self {
        HandleResult::Shutdown(r)
    }
}

/// Trait for implementing daemon command handlers
///
/// Implement this trait to define how your daemon handles commands and periodic ticks.
#[async_trait]
pub trait DaemonHandler: Send + Sync + 'static {
    /// The command type this daemon accepts
    type Command: for<'de> Deserialize<'de> + Send;

    /// The response type this daemon returns
    type Response: Serialize + Send + Sync;

    /// Handle an incoming command
    ///
    /// This method is called whenever a client sends a command.
    /// Return `HandleResult::Response` to continue running, or
    /// `HandleResult::Shutdown` to stop the daemon after sending the response.
    async fn handle_command(&mut self, cmd: Self::Command) -> HandleResult<Self::Response>;

    /// Called periodically based on the tick interval
    ///
    /// Use this for background tasks like checking timers, cleaning up resources, etc.
    /// The default implementation does nothing.
    async fn on_tick(&mut self) {}

    /// Called when the daemon starts, before accepting connections
    ///
    /// Use this for initialization that needs to happen after the socket is created.
    /// The default implementation does nothing.
    fn on_start(&mut self) {}

    /// Called when the daemon is shutting down
    ///
    /// Use this for cleanup tasks.
    /// The default implementation does nothing.
    fn on_shutdown(&mut self) {}
}

/// Run the daemon event loop
///
/// This function:
/// 1. Sets up the socket listener
/// 2. Calls `on_start` on the handler
/// 3. Runs the main event loop, handling:
///    - Incoming client connections
///    - Periodic tick events
/// 4. Calls `on_shutdown` when the daemon exits
///
/// # Arguments
/// * `config` - Daemon configuration (socket path, intervals, etc.)
/// * `handler` - The daemon handler implementing the business logic
///
/// # Returns
/// Returns an error if socket setup fails or a fatal error occurs.
pub async fn run_daemon_loop<H>(
    config: DaemonConfig,
    handler: H,
) -> Result<(), PluginError>
where
    H: DaemonHandler,
{
    // Clean up any existing socket
    socket::cleanup_socket(&config.socket_path);

    // Create listener
    let listener = socket::create_listener(&config.socket_path)?;

    println!("Daemon listening at: {}", config.socket_path);

    // Wrap handler in Arc<Mutex> for shared access
    let handler = Arc::new(Mutex::new(handler));

    // Call on_start
    {
        let mut h = handler.lock().await;
        h.on_start();
    }

    // Create tick interval
    let mut interval = time::interval(Duration::from_secs(config.tick_interval_secs));

    // Track if we should shut down
    let should_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

    loop {
        // Check if we should shut down
        if should_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        tokio::select! {
            _ = interval.tick() => {
                let mut h = handler.lock().await;
                h.on_tick().await;
            }

            result = listener.accept() => {
                match result {
                    Ok(mut stream) => {
                        let handler_clone = handler.clone();
                        let buffer_size = config.buffer_size;
                        let shutdown_flag = should_shutdown.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(
                                &mut stream,
                                handler_clone,
                                buffer_size,
                                shutdown_flag,
                            ).await {
                                eprintln!("Error handling client: {}", e);
                            }
                        });
                    }
                    Err(e) => eprintln!("Connection error: {}", e),
                }
            }
        }
    }

    // Call on_shutdown
    {
        let mut h = handler.lock().await;
        h.on_shutdown();
    }

    // Clean up socket
    socket::cleanup_socket(&config.socket_path);

    Ok(())
}

async fn handle_connection<H>(
    stream: &mut LocalSocketStream,
    handler: Arc<Mutex<H>>,
    buffer_size: usize,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), PluginError>
where
    H: DaemonHandler,
{
    // Receive command
    let cmd: H::Command = match receive_message(stream, buffer_size).await {
        Ok(cmd) => cmd,
        Err(PluginError::IpcConnection { message, .. }) if message.contains("closed") => {
            // Client disconnected without sending
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    // Handle command
    let result = {
        let mut h = handler.lock().await;
        h.handle_command(cmd).await
    };

    // Send response and check for shutdown
    match result {
        HandleResult::Response(response) => {
            send_message(stream, &response).await?;
        }
        HandleResult::Shutdown(response) => {
            send_message(stream, &response).await?;
            shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_config_creation() {
        let config = DaemonConfig::new("/tmp/test.sock");
        assert_eq!(config.socket_path, "/tmp/test.sock");
        assert_eq!(config.tick_interval_secs, 1);
        assert_eq!(config.buffer_size, 1024);
    }

    #[test]
    fn test_daemon_config_with_options() {
        let config = DaemonConfig::new("/tmp/test.sock")
            .with_tick_interval(5)
            .with_buffer_size(2048);
        assert_eq!(config.tick_interval_secs, 5);
        assert_eq!(config.buffer_size, 2048);
    }
}

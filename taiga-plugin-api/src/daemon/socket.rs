//! Cross-platform socket utilities for daemon-based plugins
//!
//! Consolidates platform-specific socket handling for both Unix and Windows.

use interprocess::local_socket::traits::tokio::Stream as _;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, ToFsName, ToNsName,
    tokio::Listener, tokio::Stream,
};
use crate::PluginError;

/// Create a local socket listener at the given path
///
/// On Unix, this creates a filesystem socket.
/// On Windows, this creates a named pipe.
pub fn create_listener(path: &str) -> Result<Listener, PluginError> {
    let listener = if cfg!(windows) {
        let name = path
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| PluginError::ipc_connection(format!("Invalid socket name: {}", e)))?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| PluginError::ipc_connection(format!("Failed to create listener: {}", e)))?
    } else {
        let name = path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| PluginError::ipc_connection(format!("Invalid socket name: {}", e)))?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| PluginError::ipc_connection(format!("Failed to create listener: {}", e)))?
    };
    Ok(listener)
}

/// Connect to a local socket at the given path
///
/// On Unix, this connects to a filesystem socket.
/// On Windows, this connects to a named pipe.
pub async fn connect(path: &str) -> Result<Stream, PluginError> {
    let stream = if cfg!(windows) {
        let name = path
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| PluginError::ipc_connection(format!("Invalid socket name: {}", e)))?;
        Stream::connect(name)
            .await
            .map_err(|e| PluginError::ipc_connection(format!("Connection failed: {}", e)))?
    } else {
        let name = path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| PluginError::ipc_connection(format!("Invalid socket name: {}", e)))?;
        Stream::connect(name)
            .await
            .map_err(|e| PluginError::ipc_connection(format!("Connection failed: {}", e)))?
    };
    Ok(stream)
}

/// Clean up a socket file on Unix systems
///
/// On Windows, named pipes don't need explicit cleanup.
pub fn cleanup_socket(path: &str) {
    if !cfg!(windows) && std::fs::metadata(path).is_ok() {
        std::fs::remove_file(path).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_nonexistent_socket() {
        // Should not panic when socket doesn't exist
        cleanup_socket("/nonexistent/socket/path");
    }
}

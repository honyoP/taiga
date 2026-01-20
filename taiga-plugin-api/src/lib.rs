//! Taiga Plugin API
//!
//! This crate provides the core types and traits needed to create plugins for Taiga.
//!
//! # Creating a Plugin
//!
//! 1. Create a new crate with `crate-type = ["cdylib"]`
//! 2. Implement the `Plugin` trait
//! 3. Export the plugin using `export_plugin!` macro
//!
//! # Example
//!
//! ```rust,ignore
//! use taiga_plugin_api::{Plugin, PluginContext, CommandDef, CommandResult, export_plugin};
//!
//! pub struct MyPlugin;
//!
//! impl Plugin for MyPlugin {
//!     fn name(&self) -> &str { "my-plugin" }
//!     fn version(&self) -> &str { "0.1.0" }
//!     fn description(&self) -> &str { "My awesome plugin" }
//!
//!     fn commands(&self) -> Vec<CommandDef> {
//!         vec![CommandDef::new("greet", "Says hello")]
//!     }
//!
//!     fn execute(&self, cmd: &str, args: &[String], _ctx: &mut PluginContext) -> PluginResult<CommandResult> {
//!         match cmd {
//!             "greet" => Ok(CommandResult::Success(Some("Hello!".into()))),
//!             _ => Ok(CommandResult::Error(format!("Unknown command: {}", cmd))),
//!         }
//!     }
//! }
//!
//! export_plugin!(MyPlugin);
//! ```

pub mod daemon;

use thiserror::Error;

/// Plugin-specific errors
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid argument '{arg}': {message}")]
    InvalidArg { arg: String, message: String },

    #[error("Argument '{arg}' out of range: {value} (expected {min}-{max})")]
    ArgOutOfRange {
        arg: String,
        value: i64,
        min: i64,
        max: i64,
    },

    #[error("IPC connection error: {message}")]
    IpcConnection {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Daemon not running and could not be started")]
    DaemonNotRunning {
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Plugin error: {0}")]
    Other(String),
}

impl PluginError {
    /// Create an invalid argument error
    pub fn invalid_arg(arg: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidArg {
            arg: arg.into(),
            message: message.into(),
        }
    }

    /// Create an argument out of range error
    pub fn arg_out_of_range(arg: impl Into<String>, value: i64, min: i64, max: i64) -> Self {
        Self::ArgOutOfRange {
            arg: arg.into(),
            value,
            min,
            max,
        }
    }

    /// Create an IPC connection error
    pub fn ipc_connection(message: impl Into<String>) -> Self {
        Self::IpcConnection {
            message: message.into(),
            source: None,
        }
    }

    /// Create an IPC connection error with source
    pub fn ipc_connection_with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::IpcConnection {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a daemon not running error
    pub fn daemon_not_running() -> Self {
        Self::DaemonNotRunning { source: None }
    }

    /// Create a daemon not running error with source
    pub fn daemon_not_running_with_source(
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::DaemonNotRunning {
            source: Some(Box::new(source)),
        }
    }
}

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Metadata about a plugin command
#[derive(Debug, Clone)]
pub struct CommandDef {
    /// Command name (e.g., "start", "status")
    pub name: String,
    /// Short description shown in help
    pub description: String,
    /// Usage string (e.g., "<FOCUS> <BREAK> <CYCLES>")
    pub usage: Option<String>,
    /// Argument definitions for help text
    pub args: Vec<ArgDef>,
}

impl CommandDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            usage: None,
            args: Vec::new(),
        }
    }

    pub fn with_usage(mut self, usage: impl Into<String>) -> Self {
        self.usage = Some(usage.into());
        self
    }

    pub fn with_arg(mut self, arg: ArgDef) -> Self {
        self.args.push(arg);
        self
    }
}

/// Definition of a command argument
#[derive(Debug, Clone)]
pub struct ArgDef {
    pub name: String,
    pub description: String,
    pub required: bool,
}

impl ArgDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            required: true,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// Context passed to plugins during execution
/// Provides access to shared resources
#[derive(Debug)]
pub struct PluginContext {
    /// Path to the data directory
    pub data_dir: std::path::PathBuf,
    /// Additional context data (plugin-specific)
    pub extra: std::collections::HashMap<String, String>,
    /// Plugin configuration as JSON string (deserialized by plugin)
    pub config_json: Option<String>,
}

impl PluginContext {
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        Self {
            data_dir,
            extra: std::collections::HashMap::new(),
            config_json: None,
        }
    }

    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    pub fn with_config(mut self, config_json: impl Into<String>) -> Self {
        self.config_json = Some(config_json.into());
        self
    }

    /// Deserialize configuration from JSON
    /// Returns default if no config is set or parsing fails
    pub fn get_config<T: serde::de::DeserializeOwned + Default>(&self) -> T {
        self.config_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default()
    }

    /// Try to deserialize configuration, returning an error on failure
    pub fn try_get_config<T: serde::de::DeserializeOwned>(&self) -> PluginResult<Option<T>> {
        match &self.config_json {
            Some(json) => Ok(Some(serde_json::from_str(json)?)),
            None => Ok(None),
        }
    }
}

/// Result of a plugin command execution
#[derive(Debug)]
pub enum CommandResult {
    /// Command completed successfully with optional message
    Success(Option<String>),
    /// Command failed with error message
    Error(String),
    /// Command started async work (message describes what's happening)
    Async(String),
}

/// The main Plugin trait that all plugins must implement
pub trait Plugin: Send + Sync {
    /// Returns the plugin's unique name (used as CLI subcommand)
    fn name(&self) -> &str;

    /// Returns the plugin's version string
    fn version(&self) -> &str;

    /// Returns a short description of the plugin
    fn description(&self) -> &str;

    /// Returns the list of commands this plugin provides
    fn commands(&self) -> Vec<CommandDef>;

    /// Execute a command with the given arguments
    fn execute(
        &self,
        command: &str,
        args: &[String],
        ctx: &mut PluginContext,
    ) -> PluginResult<CommandResult>;

    /// Called when the plugin is loaded (optional initialization)
    fn on_load(&self) -> PluginResult<()> {
        Ok(())
    }

    /// Called when the plugin is unloaded (optional cleanup)
    fn on_unload(&self) -> PluginResult<()> {
        Ok(())
    }
}

/// Trait for async plugin operations
#[async_trait::async_trait]
pub trait AsyncPlugin: Plugin {
    /// Execute a command asynchronously
    async fn execute_async(
        &self,
        command: &str,
        args: &[String],
        ctx: &mut PluginContext,
    ) -> PluginResult<CommandResult>;
}

/// Raw plugin data for FFI - contains pointer and vtable as separate values
#[repr(C)]
pub struct RawPlugin {
    pub data: *mut (),
    pub vtable: *const (),
}

// Safety: RawPlugin is just pointers, the actual safety is managed by the Plugin trait bounds
unsafe impl Send for RawPlugin {}
unsafe impl Sync for RawPlugin {}

impl RawPlugin {
    /// Create a RawPlugin from a boxed trait object
    ///
    /// # Safety
    /// The returned RawPlugin must be converted back using `into_boxed()`
    pub fn from_boxed(plugin: Box<dyn Plugin>) -> Self {
        let raw: *mut dyn Plugin = Box::into_raw(plugin);
        unsafe {
            let parts: (*mut (), *const ()) = std::mem::transmute(raw);
            Self {
                data: parts.0,
                vtable: parts.1,
            }
        }
    }

    /// Convert back to a boxed trait object
    ///
    /// # Safety
    /// Must only be called once with a RawPlugin from `from_boxed()`
    pub unsafe fn into_boxed(self) -> Box<dyn Plugin> {
        unsafe {
            let raw: *mut dyn Plugin = std::mem::transmute((self.data, self.vtable));
            Box::from_raw(raw)
        }
    }

    /// Check if the plugin pointer is null
    pub fn is_null(&self) -> bool {
        self.data.is_null()
    }
}

/// Plugin entry point function type
pub type PluginCreateFn = unsafe extern "C" fn() -> RawPlugin;

/// Plugin destruction function type
pub type PluginDestroyFn = unsafe extern "C" fn(RawPlugin);

/// Macro to export a plugin from a cdylib crate
///
/// # Example
/// ```rust,ignore
/// use taiga_plugin_api::{Plugin, export_plugin};
///
/// struct MyPlugin;
/// impl Plugin for MyPlugin { /* ... */ }
///
/// export_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($plugin_type:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn taiga_plugin_create() -> $crate::RawPlugin {
            let plugin: Box<dyn $crate::Plugin> = Box::new(<$plugin_type>::new());
            $crate::RawPlugin::from_boxed(plugin)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn taiga_plugin_destroy(plugin: $crate::RawPlugin) {
            unsafe {
                let _ = plugin.into_boxed();
                // Box is dropped here, calling destructor
            }
        }
    };
}

/// Plugin metadata for discovery
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub commands: Vec<CommandDef>,
}

impl PluginInfo {
    pub fn from_plugin(plugin: &dyn Plugin) -> Self {
        Self {
            name: plugin.name().to_string(),
            version: plugin.version().to_string(),
            description: plugin.description().to_string(),
            commands: plugin.commands(),
        }
    }
}

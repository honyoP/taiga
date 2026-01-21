use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::Library;

use crate::error::{CliError, Result};
use crate::plugin::{CommandResult, Plugin, PluginContext, PluginCreateFn, PluginInfo, RawPlugin};

/// Holds a dynamically loaded plugin and its library handle
struct DynamicPlugin {
    plugin: Box<dyn Plugin>,
    #[allow(dead_code)]
    library: Library,
}

/// Manages plugin discovery, loading, and command dispatch
pub struct PluginManager {
    /// Static plugins (compiled into the binary)
    static_plugins: HashMap<String, Arc<dyn Plugin>>,
    /// Dynamically loaded plugins
    dynamic_plugins: HashMap<String, DynamicPlugin>,
    /// Plugin search paths
    plugin_paths: Vec<PathBuf>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new() -> Self {
        Self {
            static_plugins: HashMap::new(),
            dynamic_plugins: HashMap::new(),
            plugin_paths: Vec::new(),
        }
    }

    /// Add a directory to search for plugins
    pub fn add_plugin_path(&mut self, path: impl Into<PathBuf>) {
        self.plugin_paths.push(path.into());
    }

    /// Register a static plugin (compiled into the binary)
    pub fn register_static(&mut self, plugin: Arc<dyn Plugin>) -> Result<()> {
        let name = plugin.name().to_string();

        if self.static_plugins.contains_key(&name) || self.dynamic_plugins.contains_key(&name) {
            return Err(CliError::plugin(format!(
                "Plugin '{}' is already registered",
                name
            )));
        }

        plugin
            .on_load()
            .map_err(|e| CliError::plugin(format!("Plugin load error: {}", e)))?;
        self.static_plugins.insert(name, plugin);
        Ok(())
    }

    /// Load a dynamic plugin from a library file
    ///
    /// # Safety
    /// This function loads and executes code from an external library.
    /// Only load plugins from trusted sources.
    pub fn load_dynamic(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();

        let library = unsafe {
            Library::new(path).map_err(|e| {
                CliError::plugin(format!("Failed to load plugin from {:?}: {}", path, e))
            })?
        };

        let create_fn: libloading::Symbol<PluginCreateFn> = unsafe {
            library.get(b"taiga_plugin_create").map_err(|e| {
                CliError::plugin(format!(
                    "Plugin {:?} missing 'taiga_plugin_create' symbol: {}",
                    path, e
                ))
            })?
        };

        let plugin: Box<dyn Plugin> = unsafe {
            let raw: RawPlugin = create_fn();
            if raw.is_null() {
                return Err(CliError::plugin(format!(
                    "Plugin {:?} returned null from create function",
                    path
                )));
            }
            raw.into_boxed()
        };

        let name = plugin.name().to_string();

        if self.static_plugins.contains_key(&name) || self.dynamic_plugins.contains_key(&name) {
            return Err(CliError::plugin(format!(
                "Plugin '{}' is already registered",
                name
            )));
        }

        // Skip on_load() during discovery - it's called lazily when first used
        // This avoids FFI issues with error types crossing boundaries

        self.dynamic_plugins
            .insert(name, DynamicPlugin { plugin, library });

        Ok(())
    }

    /// Discover and load all plugins from registered paths
    pub fn discover_plugins(&mut self) -> Result<Vec<String>> {
        let mut loaded = Vec::new();

        for path in self.plugin_paths.clone() {
            if !path.exists() {
                continue;
            }

            let entries = std::fs::read_dir(&path).map_err(|e| {
                CliError::plugin(format!("Failed to read plugin directory {:?}: {}", path, e))
            })?;

            for entry in entries.flatten() {
                let file_path = entry.path();

                let extension = file_path.extension().and_then(|e| e.to_str());
                let is_plugin = match extension {
                    Some("so") => cfg!(target_os = "linux"),
                    Some("dylib") => cfg!(target_os = "macos"),
                    Some("dll") => cfg!(target_os = "windows"),
                    _ => false,
                };

                if is_plugin {
                    // Check file name to avoid loading duplicate plugins
                    let file_name = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");

                    // Skip if we've already loaded a plugin with similar name
                    let plugin_base_name = file_name
                        .strip_prefix("lib")
                        .unwrap_or(file_name)
                        .strip_suffix(".so")
                        .or_else(|| file_name.strip_suffix(".dylib"))
                        .or_else(|| file_name.strip_suffix(".dll"))
                        .unwrap_or(file_name);

                    // Check if already loaded
                    let already_loaded =
                        loaded.iter().any(|p: &String| p.contains(plugin_base_name));
                    if already_loaded {
                        continue;
                    }

                    match self.load_dynamic(&file_path) {
                        Ok(()) => {
                            loaded.push(file_path.display().to_string());
                        }
                        Err(e) => {
                            // Only warn if it's not a duplicate registration
                            let err_str = format!("{}", e);
                            if !err_str.contains("already registered") {
                                eprintln!("Warning: Failed to load plugin {:?}: {}", file_path, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Get a plugin by name
    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        if let Some(plugin) = self.static_plugins.get(name) {
            return Some(plugin.as_ref());
        }
        if let Some(dynamic) = self.dynamic_plugins.get(name) {
            return Some(dynamic.plugin.as_ref());
        }
        None
    }

    /// Get all registered plugins
    pub fn plugins(&self) -> Vec<&dyn Plugin> {
        let mut plugins: Vec<&dyn Plugin> =
            self.static_plugins.values().map(|p| p.as_ref()).collect();

        plugins.extend(self.dynamic_plugins.values().map(|d| d.plugin.as_ref()));

        plugins
    }

    /// Get plugin info for all registered plugins
    pub fn plugin_infos(&self) -> Vec<PluginInfo> {
        self.plugins()
            .iter()
            .map(|p| PluginInfo::from_plugin(*p))
            .collect()
    }

    /// Check if a plugin exists
    pub fn has_plugin(&self, name: &str) -> bool {
        self.static_plugins.contains_key(name) || self.dynamic_plugins.contains_key(name)
    }

    /// Check if a command is handled by a plugin
    pub fn has_command(&self, plugin_name: &str, command: &str) -> bool {
        if let Some(plugin) = self.get(plugin_name) {
            return plugin.commands().iter().any(|c| c.name == command);
        }
        false
    }

    /// Execute a plugin command
    pub fn execute(
        &self,
        plugin_name: &str,
        command: &str,
        args: &[String],
        ctx: &mut PluginContext,
    ) -> Result<CommandResult> {
        let plugin = self
            .get(plugin_name)
            .ok_or_else(|| CliError::plugin(format!("Plugin '{}' not found", plugin_name)))?;

        plugin
            .execute(command, args, ctx)
            .map_err(|e| CliError::plugin(format!("Command execution failed: {}", e)))
    }

    /// Unload all plugins (called on shutdown)
    pub fn unload_all(&mut self) -> Result<()> {
        // Skip on_unload() calls to avoid FFI issues with error types
        // The plugins will be dropped automatically when the library is unloaded
        self.static_plugins.clear();
        self.dynamic_plugins.clear();
        Ok(())
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        if let Err(e) = self.unload_all() {
            eprintln!("Error during plugin manager cleanup: {}", e);
        }
    }
}

//! Taiga TUI Plugin
//!
//! A terminal user interface for Taiga task manager.

mod app;
mod ui;
mod task_storage;

use taiga_plugin_api::{
    CommandDef, CommandResult, Plugin, PluginContext, PluginResult,
    export_plugin,
};

pub struct TuiPlugin;

impl TuiPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TuiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TuiPlugin {
    fn name(&self) -> &str {
        "tui"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn description(&self) -> &str {
        "Terminal user interface for Taiga"
    }

    fn commands(&self) -> Vec<CommandDef> {
        vec![
            CommandDef::new("run", "Launch the TUI interface"),
        ]
    }

    fn execute(
        &self,
        command: &str,
        _args: &[String],
        ctx: &mut PluginContext,
    ) -> PluginResult<CommandResult> {
        match command {
            "run" => {
                match app::run_tui(ctx) {
                    Ok(()) => Ok(CommandResult::Success(None)),
                    Err(e) => Ok(CommandResult::Error(format!("TUI error: {}", e))),
                }
            }
            _ => Ok(CommandResult::Error(format!(
                "Unknown command: {}",
                command
            ))),
        }
    }
}

export_plugin!(TuiPlugin);

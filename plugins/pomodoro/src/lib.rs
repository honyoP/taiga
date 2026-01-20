//! Taiga Pomodoro Plugin
//!
//! A Pomodoro timer plugin for Taiga task manager.
//!
//! ## Commands
//!
//! - `start <FOCUS> <BREAK> <CYCLES>` - Start a pomodoro session
//! - `status` - Show current timer status
//! - `stop` - Stop the current timer
//! - `pause` - Pause the timer
//! - `resume` - Resume a paused timer
//! - `kill` - Kill the daemon process

mod audio;
mod break_window;
mod client;
mod config;
mod daemon;
mod ipc;

use taiga_plugin_api::{
    ArgDef, CommandDef, CommandResult, Plugin, PluginContext, PluginError, PluginResult,
    export_plugin,
};

/// Validation constants for pomodoro timer arguments
const MIN_FOCUS_MINUTES: u64 = 1;
const MAX_FOCUS_MINUTES: u64 = 480; // 8 hours
const MIN_BREAK_MINUTES: u64 = 1;
const MAX_BREAK_MINUTES: u64 = 120; // 2 hours
const MIN_CYCLES: u32 = 1;
const MAX_CYCLES: u32 = 100;

pub struct PomoPlugin {
    runtime: tokio::runtime::Runtime,
}

impl PomoPlugin {
    pub fn new() -> Self {
        // Runtime creation is unlikely to fail in practice, but if it does,
        // we create a minimal runtime. The error will surface on first use.
        let runtime = tokio::runtime::Runtime::new()
            .unwrap_or_else(|_| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create even minimal tokio runtime")
            });
        Self { runtime }
    }

    /// Validate focus duration in minutes
    fn validate_focus(value: u64) -> PluginResult<u64> {
        if !(MIN_FOCUS_MINUTES..=MAX_FOCUS_MINUTES).contains(&value) {
            return Err(PluginError::arg_out_of_range(
                "FOCUS",
                value as i64,
                MIN_FOCUS_MINUTES as i64,
                MAX_FOCUS_MINUTES as i64,
            ));
        }
        Ok(value)
    }

    /// Validate break duration in minutes
    fn validate_break(value: u64) -> PluginResult<u64> {
        if !(MIN_BREAK_MINUTES..=MAX_BREAK_MINUTES).contains(&value) {
            return Err(PluginError::arg_out_of_range(
                "BREAK",
                value as i64,
                MIN_BREAK_MINUTES as i64,
                MAX_BREAK_MINUTES as i64,
            ));
        }
        Ok(value)
    }

    /// Validate number of cycles
    fn validate_cycles(value: u32) -> PluginResult<u32> {
        if !(MIN_CYCLES..=MAX_CYCLES).contains(&value) {
            return Err(PluginError::arg_out_of_range(
                "CYCLES",
                value as i64,
                MIN_CYCLES as i64,
                MAX_CYCLES as i64,
            ));
        }
        Ok(value)
    }
}

impl Default for PomoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PomoPlugin {
    fn name(&self) -> &str {
        "pomo"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn description(&self) -> &str {
        "Pomodoro timer for focused work sessions"
    }

    fn commands(&self) -> Vec<CommandDef> {
        vec![
            CommandDef::new("start", "Start a new pomodoro session")
                .with_usage("<FOCUS> <BREAK> <CYCLES> [--no-gui] [--no-sound]")
                .with_arg(ArgDef::new("FOCUS", "Focus duration in minutes"))
                .with_arg(ArgDef::new("BREAK", "Break duration in minutes"))
                .with_arg(ArgDef::new("CYCLES", "Number of focus cycles"))
                .with_arg(ArgDef::new("--no-gui", "Disable break window GUI"))
                .with_arg(ArgDef::new("--no-sound", "Disable sound alerts")),
            CommandDef::new("status", "Show current timer status"),
            CommandDef::new("stop", "Stop the current timer"),
            CommandDef::new("pause", "Pause the running timer"),
            CommandDef::new("resume", "Resume a paused timer"),
            CommandDef::new("kill", "Kill the daemon process"),
            CommandDef::new("daemon", "Run as daemon (internal)"),
            CommandDef::new("break-window", "Show break window (internal)")
                .with_arg(ArgDef::new("--duration", "Break duration in seconds"))
                .with_arg(ArgDef::new("--long", "Long break"))
                .with_arg(ArgDef::new("--sound", "Play sounds")),
        ]
    }

    fn execute(
        &self,
        command: &str,
        args: &[String],
        _ctx: &mut PluginContext,
    ) -> PluginResult<CommandResult> {
        match command {
            "start" => {
                if args.len() < 3 {
                    return Ok(CommandResult::Error(
                        "Usage: pomo start <FOCUS> <BREAK> <CYCLES> [--no-gui] [--no-sound]"
                            .to_string(),
                    ));
                }

                let focus: u64 = args[0]
                    .parse()
                    .map_err(|_| PluginError::invalid_arg("FOCUS", "must be a number"))?;
                let focus = Self::validate_focus(focus)?;

                let break_time: u64 = args[1]
                    .parse()
                    .map_err(|_| PluginError::invalid_arg("BREAK", "must be a number"))?;
                let break_time = Self::validate_break(break_time)?;

                let cycles: u32 = args[2]
                    .parse()
                    .map_err(|_| PluginError::invalid_arg("CYCLES", "must be a number"))?;
                let cycles = Self::validate_cycles(cycles)?;

                // Parse optional flags
                let no_gui = args.iter().any(|a| a == "--no-gui");
                let no_sound = args.iter().any(|a| a == "--no-sound");

                let result = self.runtime.block_on(async {
                    client::send_command(ipc::DaemonCommand::Start {
                        task_id: 0,
                        focus_len: focus,
                        break_len: break_time,
                        cycles,
                        no_gui,
                        no_sound,
                    })
                    .await
                })?;

                Ok(CommandResult::Success(Some(format!("{:?}", result))))
            }

            "status" => {
                let result = self
                    .runtime
                    .block_on(async { client::send_command(ipc::DaemonCommand::Status).await })?;

                match result {
                    ipc::DaemonResponse::Status {
                        remaining_secs,
                        is_running,
                        mode,
                        cycles_left,
                        task_id: _,
                    } => {
                        let status = if is_running { "Running" } else { "Paused" };
                        let mins = remaining_secs / 60;
                        let secs = remaining_secs % 60;
                        Ok(CommandResult::Success(Some(format!(
                            "Status: {} | Mode: {:?} | Time: {}:{:02} | Cycles left: {}",
                            status, mode, mins, secs, cycles_left
                        ))))
                    }
                    other => Ok(CommandResult::Success(Some(format!("{:?}", other)))),
                }
            }

            "stop" => {
                let result = self
                    .runtime
                    .block_on(async { client::send_command(ipc::DaemonCommand::Stop).await })?;

                Ok(CommandResult::Success(Some(format!("{:?}", result))))
            }

            "pause" => {
                let result = self
                    .runtime
                    .block_on(async { client::send_command(ipc::DaemonCommand::Pause).await })?;

                Ok(CommandResult::Success(Some(format!("{:?}", result))))
            }

            "resume" => {
                let result = self
                    .runtime
                    .block_on(async { client::send_command(ipc::DaemonCommand::Resume).await })?;

                Ok(CommandResult::Success(Some(format!("{:?}", result))))
            }

            "kill" => {
                let result = self
                    .runtime
                    .block_on(async { client::send_command(ipc::DaemonCommand::Kill).await })?;

                Ok(CommandResult::Success(Some(format!("{:?}", result))))
            }

            "daemon" => {
                // Run daemon mode - this blocks
                self.runtime
                    .block_on(async { daemon::run_daemon().await })?;

                Ok(CommandResult::Success(None))
            }

            "break-window" => {
                // Parse arguments for internal break window command
                let mut duration_secs: u64 = 5 * 60; // default 5 min
                let mut is_long_break = false;
                let mut play_sound = false;

                let mut i = 0;
                while i < args.len() {
                    match args[i].as_str() {
                        "--duration" => {
                            if i + 1 < args.len() {
                                duration_secs = args[i + 1].parse().unwrap_or(5 * 60);
                                i += 1;
                            }
                        }
                        "--long" => is_long_break = true,
                        "--sound" => play_sound = true,
                        _ => {}
                    }
                    i += 1;
                }

                // Run break window - blocks until closed
                let completed = break_window::run_break_window_command(duration_secs, is_long_break, play_sound);

                // Exit with code 0 for normal completion, 1 for skipped
                std::process::exit(if completed { 0 } else { 1 });
            }

            _ => Ok(CommandResult::Error(format!(
                "Unknown command: {}",
                command
            ))),
        }
    }
}

// Export the plugin for dynamic loading
export_plugin!(PomoPlugin);

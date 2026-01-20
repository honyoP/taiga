//! Break timer GUI window
//!
//! Shows a small always-on-top window with countdown timer during breaks.

use crate::audio;
use crate::config::{PomoConfig, UiSettings};
use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Configuration for the break window
#[derive(Clone)]
pub struct BreakConfig {
    pub duration_secs: u64,
    pub is_long_break: bool,
    pub play_sound: bool,
}

impl Default for BreakConfig {
    fn default() -> Self {
        Self {
            duration_secs: 5 * 60, // 5 minutes
            is_long_break: false,
            play_sound: true,
        }
    }
}

/// Shared state for communicating with the break window
pub struct BreakWindowState {
    pub should_close: AtomicBool,
    pub break_skipped: AtomicBool,
}

impl Default for BreakWindowState {
    fn default() -> Self {
        Self {
            should_close: AtomicBool::new(false),
            break_skipped: AtomicBool::new(false),
        }
    }
}

/// The break window application
struct BreakWindowApp {
    config: BreakConfig,
    ui_settings: UiSettings,
    start_time: Instant,
    state: Arc<BreakWindowState>,
    sound_played_at_end: bool,
}

impl BreakWindowApp {
    fn new(config: BreakConfig, state: Arc<BreakWindowState>) -> Self {
        // Play break start sound
        if config.play_sound {
            audio::play_break_start_alert();
        }

        let ui_settings = PomoConfig::default().ui;

        Self {
            config,
            ui_settings,
            start_time: Instant::now(),
            state,
            sound_played_at_end: false,
        }
    }

    fn remaining_secs(&self) -> u64 {
        let elapsed = self.start_time.elapsed().as_secs();
        self.config.duration_secs.saturating_sub(elapsed)
    }

    fn is_break_over(&self) -> bool {
        self.remaining_secs() == 0
    }

    fn format_time(secs: u64) -> String {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}

impl eframe::App for BreakWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if we should close
        if self.state.should_close.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let remaining = self.remaining_secs();

        // Check if break is over
        if self.is_break_over() {
            if !self.sound_played_at_end && self.config.play_sound {
                audio::play_break_end_alert();
                self.sound_played_at_end = true;
            }

            // Show "break over" message briefly, then close
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.heading("Break finished!");
                    ui.add_space(10.0);
                    ui.label("Time to get back to work!");
                    ui.add_space(20.0);
                    if ui.button("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            // Auto-close after configured delay
            let auto_close_delay = self.ui_settings.auto_close_delay_secs;
            ctx.request_repaint_after(Duration::from_secs(auto_close_delay));
            if self.start_time.elapsed().as_secs() > self.config.duration_secs + auto_close_delay {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }

        // Update window title with time
        let title = format!("Break Time - {}", Self::format_time(remaining));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(15.0);

                // Break type label
                let break_type = if self.config.is_long_break {
                    "Long Break"
                } else {
                    "Short Break"
                };
                ui.label(
                    egui::RichText::new(break_type)
                        .size(self.ui_settings.label_font_size)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(5.0);

                // Main message
                ui.label(egui::RichText::new("Take a break!").size(self.ui_settings.message_font_size));

                ui.add_space(10.0);

                // Countdown timer - large and prominent
                let [r, g, b] = self.ui_settings.timer_color;
                ui.label(
                    egui::RichText::new(Self::format_time(remaining))
                        .size(self.ui_settings.countdown_font_size)
                        .strong()
                        .color(egui::Color32::from_rgb(r, g, b)),
                );

                ui.add_space(5.0);
                ui.label("remaining");

                ui.add_space(15.0);

                // Skip break button
                if ui.button("Skip Break").clicked() {
                    self.state.break_skipped.store(true, Ordering::Relaxed);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });

        // Request repaint every second for countdown
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

/// Show the break window
///
/// This function blocks until the window is closed.
/// Returns true if break completed normally, false if skipped.
pub fn show_break_window(config: BreakConfig) -> bool {
    let ui_settings = PomoConfig::default().ui;
    let state = Arc::new(BreakWindowState::default());
    let state_clone = state.clone();

    let window_size = [ui_settings.window_width, ui_settings.window_height];

    // Calculate window position (bottom-right with margin)
    // We'll set initial position, eframe will handle the rest
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(window_size)
            .with_min_inner_size(window_size)
            .with_max_inner_size(window_size)
            .with_resizable(false)
            .with_always_on_top()
            .with_decorations(true)
            .with_title_shown(true)
            .with_titlebar_shown(true)
            .with_close_button(true)
            .with_minimize_button(false)
            .with_maximize_button(false),
        centered: false,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Break Time",
        options,
        Box::new(move |_cc| Ok(Box::new(BreakWindowApp::new(config, state_clone)))),
    );

    if let Err(e) = result {
        eprintln!("Warning: Failed to show break window: {}", e);
        return true; // Assume break completed
    }

    !state.break_skipped.load(Ordering::Relaxed)
}

/// Spawn break window as a separate process (for daemon use)
///
/// This spawns the break window as a child process, which gets its own main thread
/// and proper GUI access. Returns a handle to wait for the process.
pub fn spawn_break_window_process(config: BreakConfig) -> Option<std::process::Child> {
    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("Failed to get current exe: {}", e);
            return None;
        }
    };

    let mut cmd = std::process::Command::new(current_exe);
    cmd.arg("pomo").arg("break-window");
    cmd.arg("--duration").arg(config.duration_secs.to_string());

    if config.is_long_break {
        cmd.arg("--long");
    }
    if config.play_sound {
        cmd.arg("--sound");
    }

    match cmd.spawn() {
        Ok(child) => Some(child),
        Err(e) => {
            eprintln!("Failed to spawn break window process: {}", e);
            None
        }
    }
}

/// Run the break window (called from break-window subcommand)
pub fn run_break_window_command(duration_secs: u64, is_long_break: bool, play_sound: bool) -> bool {
    let config = BreakConfig {
        duration_secs,
        is_long_break,
        play_sound,
    };
    show_break_window(config)
}

//! Pomodoro daemon implementation using the generic daemon framework

use crate::break_window::{self, BreakConfig};
use crate::config::PomoConfig;
use crate::ipc::{DaemonCommand, DaemonResponse, PomoMode, get_socket_path};
use async_trait::async_trait;
use notify_rust::Notification;
use taiga_plugin_api::daemon::traits::{DaemonConfig, DaemonHandler, HandleResult, run_daemon_loop};
use taiga_plugin_api::PluginError;
use tokio::time::{Duration, Instant};

struct TimerConfig {
    focus_duration: Duration,
    break_duration: Duration,
    long_break_duration: Duration,
    show_gui: bool,
    play_sound: bool,
}

struct PomodoroDaemon {
    mode: PomoMode,
    end_time: Option<Instant>,
    cycles_remaining: u32,
    total_cycles: u32,
    completed_pomodoros: u32,
    timer_config: Option<TimerConfig>,
    task_id: Option<u32>,
    paused_duration: Option<Duration>,
    break_window_process: Option<std::process::Child>,
    pomo_config: PomoConfig,
}

impl PomodoroDaemon {
    fn new() -> Self {
        Self {
            mode: PomoMode::Idle,
            end_time: None,
            cycles_remaining: 0,
            total_cycles: 0,
            completed_pomodoros: 0,
            timer_config: None,
            task_id: None,
            paused_duration: None,
            break_window_process: None,
            pomo_config: PomoConfig::default(),
        }
    }

    fn handle_timer_transition(&mut self) {
        let timer_config = match self.timer_config.as_ref() {
            Some(c) => c,
            None => return,
        };

        match self.mode {
            PomoMode::Focus => {
                self.cycles_remaining -= 1;
                self.completed_pomodoros += 1;

                if self.cycles_remaining > 0 {
                    let pomodoros_before_long_break =
                        self.pomo_config.timer.pomodoros_before_long_break;
                    let is_long_break =
                        self.completed_pomodoros.is_multiple_of(pomodoros_before_long_break);

                    let break_duration = if is_long_break {
                        timer_config.long_break_duration
                    } else {
                        timer_config.break_duration
                    };

                    if timer_config.show_gui {
                        let break_config = BreakConfig {
                            duration_secs: break_duration.as_secs(),
                            is_long_break,
                            play_sound: timer_config.play_sound,
                        };

                        self.mode = PomoMode::Break;
                        self.end_time = None;

                        if let Some(child) = break_window::spawn_break_window_process(break_config) {
                            self.break_window_process = Some(child);
                        } else {
                            let break_type = if is_long_break { "long" } else { "short" };
                            Notification::new()
                                .summary("Taiga Pomodoro")
                                .body(&format!(
                                    "Focus complete! Take a {} break ({} min). [GUI failed to launch]",
                                    break_type,
                                    break_duration.as_secs() / 60
                                ))
                                .show()
                                .ok();

                            if timer_config.play_sound {
                                crate::audio::play_break_start_alert();
                            }

                            self.end_time = Some(Instant::now() + break_duration);
                        }
                    } else {
                        let break_type = if is_long_break { "long" } else { "short" };
                        Notification::new()
                            .summary("Taiga Pomodoro")
                            .body(&format!(
                                "Focus complete! Take a {} break ({} min).",
                                break_type,
                                break_duration.as_secs() / 60
                            ))
                            .show()
                            .ok();

                        if timer_config.play_sound {
                            crate::audio::play_break_start_alert();
                        }

                        self.mode = PomoMode::Break;
                        self.end_time = Some(Instant::now() + break_duration);
                    }
                } else {
                    Notification::new()
                        .summary("Taiga Pomodoro")
                        .body(&format!(
                            "All {} Pomodoros finished! Great work!",
                            self.total_cycles
                        ))
                        .show()
                        .ok();

                    if timer_config.play_sound {
                        crate::audio::play_break_end_alert();
                    }

                    self.reset();
                }
            }
            PomoMode::Break => {
                Notification::new()
                    .summary("Taiga Pomodoro")
                    .body("Break over! Back to work.")
                    .show()
                    .ok();

                if let Some(cfg) = &self.timer_config {
                    if cfg.play_sound {
                        crate::audio::play_break_end_alert();
                    }
                }

                self.mode = PomoMode::Focus;
                if let Some(cfg) = &self.timer_config {
                    self.end_time = Some(Instant::now() + cfg.focus_duration);
                }
            }
            PomoMode::Idle => {
                self.reset();
            }
        }
    }

    fn handle_break_finished(&mut self) {
        Notification::new()
            .summary("Taiga Pomodoro")
            .body("Break over! Starting next focus session.")
            .show()
            .ok();

        self.mode = PomoMode::Focus;
        if let Some(cfg) = &self.timer_config {
            self.end_time = Some(Instant::now() + cfg.focus_duration);
        }
    }

    fn reset(&mut self) {
        self.mode = PomoMode::Idle;
        self.end_time = None;
        self.paused_duration = None;
        self.cycles_remaining = 0;
        self.completed_pomodoros = 0;
    }
}

#[async_trait]
impl DaemonHandler for PomodoroDaemon {
    type Command = DaemonCommand;
    type Response = DaemonResponse;

    fn on_start(&mut self) {
        println!("Pomodoro daemon started");
    }

    async fn on_tick(&mut self) {
        // Check if break window process finished
        if let Some(mut child) = self.break_window_process.take() {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    self.handle_break_finished();
                }
                Ok(None) => {
                    self.break_window_process = Some(child);
                }
                Err(e) => {
                    eprintln!("Error checking break window process: {}", e);
                    self.handle_break_finished();
                }
            }
        }

        // Check timer expiration
        if let Some(end_time) = self.end_time {
            if Instant::now() >= end_time {
                self.handle_timer_transition();
            }
        }
    }

    async fn handle_command(&mut self, cmd: DaemonCommand) -> HandleResult<DaemonResponse> {
        let response = match cmd {
            DaemonCommand::Start {
                task_id: _,
                focus_len,
                break_len,
                cycles,
                no_gui,
                no_sound,
            } => {
                let focus_dur = Duration::from_secs(focus_len * 60);
                let break_dur = Duration::from_secs(break_len * 60);
                let long_break_dur =
                    Duration::from_secs(self.pomo_config.timer.long_break_minutes * 60);

                self.timer_config = Some(TimerConfig {
                    focus_duration: focus_dur,
                    break_duration: break_dur,
                    long_break_duration: long_break_dur,
                    show_gui: !no_gui,
                    play_sound: !no_sound,
                });

                self.cycles_remaining = cycles;
                self.total_cycles = cycles;
                self.completed_pomodoros = 0;
                self.mode = PomoMode::Focus;
                self.end_time = Some(Instant::now() + focus_dur);
                self.paused_duration = None;

                DaemonResponse::Ok(format!(
                    "Started: {}m Focus, {}m Break ({} cycles){}{}",
                    focus_len,
                    break_len,
                    cycles,
                    if no_gui { " [no-gui]" } else { "" },
                    if no_sound { " [no-sound]" } else { "" }
                ))
            }
            DaemonCommand::Stop => {
                self.end_time = None;
                self.task_id = None;
                self.mode = PomoMode::Idle;
                self.completed_pomodoros = 0;
                DaemonResponse::Ok("Timer stopped".to_string())
            }
            DaemonCommand::Status => {
                if let Some(end) = self.end_time {
                    let rem = end.saturating_duration_since(Instant::now()).as_secs();
                    DaemonResponse::Status {
                        remaining_secs: rem,
                        is_running: true,
                        mode: self.mode,
                        cycles_left: self.cycles_remaining,
                        task_id: self.task_id,
                    }
                } else if self.break_window_process.is_some() {
                    DaemonResponse::Status {
                        remaining_secs: 0,
                        is_running: true,
                        mode: PomoMode::Break,
                        cycles_left: self.cycles_remaining,
                        task_id: self.task_id,
                    }
                } else if let Some(dur) = self.paused_duration {
                    DaemonResponse::Status {
                        remaining_secs: dur.as_secs(),
                        is_running: false,
                        mode: self.mode,
                        cycles_left: self.cycles_remaining,
                        task_id: self.task_id,
                    }
                } else {
                    DaemonResponse::Status {
                        remaining_secs: 0,
                        is_running: false,
                        mode: PomoMode::Idle,
                        cycles_left: 0,
                        task_id: self.task_id,
                    }
                }
            }
            DaemonCommand::Pause => {
                if let Some(end) = self.end_time {
                    let remaining = end.saturating_duration_since(Instant::now());
                    self.paused_duration = Some(remaining);
                    self.end_time = None;
                    DaemonResponse::Ok(format!("Paused with {}s remaining", remaining.as_secs()))
                } else {
                    DaemonResponse::Error("Timer is not running".to_string())
                }
            }
            DaemonCommand::Resume => {
                if let Some(duration) = self.paused_duration {
                    self.end_time = Some(Instant::now() + duration);
                    self.paused_duration = None;
                    DaemonResponse::Ok("Timer resumed".to_string())
                } else {
                    DaemonResponse::Error("No paused timer found".to_string())
                }
            }
            DaemonCommand::Kill => {
                return HandleResult::shutdown(DaemonResponse::Ok("Daemon shutting down.".into()));
            }
            DaemonCommand::Ping => DaemonResponse::Pong,
        };

        HandleResult::response(response)
    }
}

pub async fn run_daemon() -> Result<(), PluginError> {
    let pomo_config = PomoConfig::default();
    let socket_path = get_socket_path();

    let config = DaemonConfig::new(&socket_path)
        .with_tick_interval(pomo_config.timing.tick_interval_secs)
        .with_buffer_size(pomo_config.timing.ipc_buffer_size);

    println!("Pomodoro daemon starting...");

    run_daemon_loop(config, PomodoroDaemon::new()).await
}

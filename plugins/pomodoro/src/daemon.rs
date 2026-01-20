use crate::break_window::{self, BreakConfig};
use crate::config::PomoConfig;
use crate::ipc::{DaemonCommand, DaemonResponse, PomoMode, get_socket_path};
use taiga_plugin_api::daemon::ipc::{receive_message, send_message};
use taiga_plugin_api::daemon::socket;
use interprocess::local_socket::tokio::Stream as LocalSocketStream;
use interprocess::local_socket::traits::tokio::Listener as _;
use notify_rust::Notification;
use std::sync::Arc;
use taiga_plugin_api::PluginError;
use tokio::sync::Mutex;
use tokio::time::{self, Duration, Instant};

struct TimerConfig {
    focus_duration: Duration,
    break_duration: Duration,
    long_break_duration: Duration,
    /// Whether to show GUI break window
    show_gui: bool,
    /// Whether to play sounds
    play_sound: bool,
}

struct TimerState {
    mode: PomoMode,
    end_time: Option<Instant>,
    cycles_remaining: u32,
    /// Total cycles at start (to track when to do long break)
    total_cycles: u32,
    /// Completed pomodoros in this session
    completed_pomodoros: u32,
    timer_config: Option<TimerConfig>,
    task_id: Option<u32>,
    paused_duration: Option<Duration>,
    /// Handle to the break window process (if running)
    break_window_process: Option<std::process::Child>,
    /// Global plugin configuration
    pomo_config: PomoConfig,
}

pub async fn run_daemon() -> Result<(), PluginError> {
    let pomo_config = PomoConfig::default();

    println!("Pomodoro daemon starting...");

    let socket_path = get_socket_path();

    // Clean up any existing socket file
    socket::cleanup_socket(&socket_path);

    let listener = socket::create_listener(&socket_path)?;

    println!("Pomodoro daemon listening at: {}", socket_path);

    let state = Arc::new(Mutex::new(TimerState {
        mode: PomoMode::Idle,
        end_time: None,
        cycles_remaining: 0,
        total_cycles: 0,
        completed_pomodoros: 0,
        timer_config: None,
        task_id: None,
        paused_duration: None,
        break_window_process: None,
        pomo_config,
    }));

    let tick_interval = {
        let locked = state.lock().await;
        locked.pomo_config.timing.tick_interval_secs
    };
    let mut interval = time::interval(Duration::from_secs(tick_interval));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let mut locked_state = state.lock().await;

                // Check if break window process finished
                if let Some(mut child) = locked_state.break_window_process.take() {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            // Process exited - exit code 0 means normal completion, 1 means skipped
                            let completed_normally = status.success();
                            handle_break_finished(&mut locked_state, completed_normally);
                        }
                        Ok(None) => {
                            // Still running, put it back
                            locked_state.break_window_process = Some(child);
                        }
                        Err(e) => {
                            eprintln!("Error checking break window process: {}", e);
                            handle_break_finished(&mut locked_state, true);
                        }
                    }
                }

                // Check timer expiration
                if let Some(end_time) = locked_state.end_time && Instant::now() >= end_time {
                    handle_timer_transition(&mut locked_state);
                }
            }

            result = listener.accept() => {
                match result {
                    Ok(mut stream) => {
                        let state_clone = state.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(&mut stream, state_clone).await {
                                eprintln!("Error handling client: {}", e);
                            }
                        });
                    }
                    Err(e) => eprintln!("Connection error: {}", e),
                }
            }
        }
    }
}

async fn handle_connection(
    stream: &mut LocalSocketStream,
    state: Arc<Mutex<TimerState>>,
) -> Result<(), PluginError> {
    let buffer_size = {
        let locked = state.lock().await;
        locked.pomo_config.timing.ipc_buffer_size
    };

    let req: DaemonCommand = match receive_message(stream, buffer_size).await {
        Ok(cmd) => cmd,
        Err(PluginError::IpcConnection { message, .. }) if message.contains("closed") => {
            // Client disconnected without sending, which is OK
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    let response = {
        let mut locked = state.lock().await;
        match req {
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
                    Duration::from_secs(locked.pomo_config.timer.long_break_minutes * 60);

                locked.timer_config = Some(TimerConfig {
                    focus_duration: focus_dur,
                    break_duration: break_dur,
                    long_break_duration: long_break_dur,
                    show_gui: !no_gui,
                    play_sound: !no_sound,
                });

                locked.cycles_remaining = cycles;
                locked.total_cycles = cycles;
                locked.completed_pomodoros = 0;
                locked.mode = PomoMode::Focus;
                locked.end_time = Some(Instant::now() + focus_dur);
                locked.paused_duration = None;

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
                locked.end_time = None;
                locked.task_id = None;
                locked.mode = PomoMode::Idle;
                locked.completed_pomodoros = 0;
                DaemonResponse::Ok("Timer stopped".to_string())
            }
            DaemonCommand::Status => {
                if let Some(end) = locked.end_time {
                    let rem = end.saturating_duration_since(Instant::now()).as_secs();
                    DaemonResponse::Status {
                        remaining_secs: rem,
                        is_running: true,
                        mode: locked.mode,
                        cycles_left: locked.cycles_remaining,
                        task_id: locked.task_id,
                    }
                } else if locked.break_window_process.is_some() {
                    // Break window is showing
                    DaemonResponse::Status {
                        remaining_secs: 0,
                        is_running: true,
                        mode: PomoMode::Break,
                        cycles_left: locked.cycles_remaining,
                        task_id: locked.task_id,
                    }
                } else if let Some(dur) = locked.paused_duration {
                    DaemonResponse::Status {
                        remaining_secs: dur.as_secs(),
                        is_running: false,
                        mode: locked.mode,
                        cycles_left: locked.cycles_remaining,
                        task_id: locked.task_id,
                    }
                } else {
                    DaemonResponse::Status {
                        remaining_secs: 0,
                        is_running: false,
                        mode: PomoMode::Idle,
                        cycles_left: 0,
                        task_id: locked.task_id,
                    }
                }
            }
            DaemonCommand::Pause => {
                if let Some(end) = locked.end_time {
                    let remaining = end.saturating_duration_since(Instant::now());
                    locked.paused_duration = Some(remaining);
                    locked.end_time = None;
                    DaemonResponse::Ok(format!("Paused with {}s remaining", remaining.as_secs()))
                } else {
                    DaemonResponse::Error("Timer is not running".to_string())
                }
            }
            DaemonCommand::Resume => {
                if let Some(duration) = locked.paused_duration {
                    locked.end_time = Some(Instant::now() + duration);
                    locked.paused_duration = None;
                    DaemonResponse::Ok("Timer resumed".to_string())
                } else {
                    DaemonResponse::Error("No paused timer found".to_string())
                }
            }
            DaemonCommand::Kill => {
                let response = DaemonResponse::Ok("Daemon shutting down.".into());
                let _ = send_message(stream, &response).await;
                std::process::exit(0);
            }
            DaemonCommand::Ping => DaemonResponse::Pong,
        }
    };

    send_message(stream, &response).await?;

    Ok(())
}

fn handle_timer_transition(state: &mut TimerState) {
    let timer_config = match state.timer_config.as_ref() {
        Some(c) => c,
        None => return,
    };

    match state.mode {
        PomoMode::Focus => {
            // Focus session complete
            state.cycles_remaining -= 1;
            state.completed_pomodoros += 1;

            if state.cycles_remaining > 0 {
                // Determine if this should be a long break
                let pomodoros_before_long_break =
                    state.pomo_config.timer.pomodoros_before_long_break;
                let is_long_break =
                    state.completed_pomodoros.is_multiple_of(pomodoros_before_long_break);

                let break_duration = if is_long_break {
                    timer_config.long_break_duration
                } else {
                    timer_config.break_duration
                };

                if timer_config.show_gui {
                    // Launch break window as separate process (handles its own timing)
                    let break_config = BreakConfig {
                        duration_secs: break_duration.as_secs(),
                        is_long_break,
                        play_sound: timer_config.play_sound,
                    };

                    state.mode = PomoMode::Break;
                    state.end_time = None; // Window handles timing

                    // Spawn as process - if this fails, fall back to notification-only mode
                    if let Some(child) = break_window::spawn_break_window_process(break_config) {
                        state.break_window_process = Some(child);
                    } else {
                        // Fallback: show notification and use timer
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

                        state.end_time = Some(Instant::now() + break_duration);
                    }
                } else {
                    // No GUI - just use notification and timer
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

                    state.mode = PomoMode::Break;
                    state.end_time = Some(Instant::now() + break_duration);
                }
            } else {
                // All cycles complete
                Notification::new()
                    .summary("Taiga Pomodoro")
                    .body(&format!(
                        "All {} Pomodoros finished! Great work!",
                        state.total_cycles
                    ))
                    .show()
                    .ok();

                if timer_config.play_sound {
                    crate::audio::play_break_end_alert();
                }

                reset_state(state);
            }
        }
        PomoMode::Break => {
            // Break complete (only hit when not using GUI)
            Notification::new()
                .summary("Taiga Pomodoro")
                .body("Break over! Back to work.")
                .show()
                .ok();

            if timer_config.play_sound {
                crate::audio::play_break_end_alert();
            }

            state.mode = PomoMode::Focus;
            if let Some(cfg) = &state.timer_config {
                state.end_time = Some(Instant::now() + cfg.focus_duration);
            }
        }
        PomoMode::Idle => {
            reset_state(state);
        }
    }
}

fn handle_break_finished(state: &mut TimerState, _completed_normally: bool) {
    // Break window closed, transition to next focus session
    Notification::new()
        .summary("Taiga Pomodoro")
        .body("Break over! Starting next focus session.")
        .show()
        .ok();

    state.mode = PomoMode::Focus;
    if let Some(cfg) = &state.timer_config {
        state.end_time = Some(Instant::now() + cfg.focus_duration);
    }
}

fn reset_state(state: &mut TimerState) {
    state.mode = PomoMode::Idle;
    state.end_time = None;
    state.paused_duration = None;
    state.cycles_remaining = 0;
    state.completed_pomodoros = 0;
}

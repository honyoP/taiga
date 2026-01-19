use crate::error::{Result, TaigaError};
use crate::ipc::{DaemonCommand, DaemonResponse, get_socket_path};
use interprocess::local_socket::traits::tokio::Listener as _;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, ToFsName, ToNsName,
    tokio::Stream as LocalSocketStream,
};
use notify_rust::Notification;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::{self, Duration, Instant};

struct TimerConfig {
    focus_duration: Duration,
    break_duration: Duration,
}

struct TimerState {
    mode: crate::ipc::PomoMode,
    end_time: Option<Instant>,
    cycles_remaining: u32,
    config: Option<TimerConfig>,
    task_id: Option<u32>,
    paused_duration: Option<Duration>,
}

pub async fn run_daemon() -> Result<()> {
    println!("Daemon starting...");

    let socket_path = get_socket_path();

    if !cfg!(windows) && std::fs::metadata(&socket_path).is_ok() {
        println!("Removing old socket file...");
        std::fs::remove_file(&socket_path).ok();
    }

    let listener = if cfg!(windows) {
        let name = socket_path
            .as_str()
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| TaigaError::Ipc(e.to_string()))?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| TaigaError::Daemon(e.to_string()))?
    } else {
        let name = socket_path
            .as_str()
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| TaigaError::Ipc(e.to_string()))?;
        ListenerOptions::new()
            .name(name)
            .create_tokio()
            .map_err(|e| TaigaError::Daemon(e.to_string()))?
    };

    println!("Daemon listening at: {}", socket_path);

    let state = Arc::new(Mutex::new(TimerState {
        mode: crate::ipc::PomoMode::Idle,
        end_time: None,
        cycles_remaining: 0,
        config: None,
        task_id: None,
        paused_duration: None,
    }));

    let mut interval = time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let mut locked_state = state.lock().await;
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
) -> Result<()> {
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer).await?;

    if n == 0 {
        return Ok(());
    }

    let req: DaemonCommand = serde_json::from_slice(&buffer[0..n])?;

    let response = {
        let mut locked = state.lock().await;
        match req {
            DaemonCommand::Start {
                task_id: _,
                focus_len,
                break_len,
                cycles,
            } => {
                let focus_dur = Duration::from_secs(focus_len * 60);
                let break_dur = Duration::from_secs(break_len * 60);

                locked.config = Some(TimerConfig {
                    focus_duration: focus_dur,
                    break_duration: break_dur,
                });

                locked.cycles_remaining = cycles;
                locked.mode = crate::ipc::PomoMode::Focus;
                locked.end_time = Some(Instant::now() + focus_dur);
                locked.paused_duration = None;

                DaemonResponse::Ok(format!(
                    "Started: {}m Focus, {}m Break ({} cycles)",
                    focus_len, break_len, cycles
                ))
            }
            DaemonCommand::Stop => {
                locked.end_time = None;
                locked.task_id = None;
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
                        mode: crate::ipc::PomoMode::Idle,
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
                let _ = stream
                    .write_all(&serde_json::to_vec(&DaemonResponse::Ok(
                        "Daemon killing itself.".into(),
                    ))?)
                    .await;
                std::process::exit(0);
            }
            DaemonCommand::Ping => DaemonResponse::Pong,
        }
    };

    let resp_bytes = serde_json::to_vec(&response)?;
    stream.write_all(&resp_bytes).await?;

    Ok(())
}

fn handle_timer_transition(state: &mut TimerState) {
    let config = state.config.as_ref().unwrap();

    match state.mode {
        crate::ipc::PomoMode::Focus => {
            state.cycles_remaining -= 1;

            if state.cycles_remaining > 0 {
                Notification::new()
                    .summary("Taiga")
                    .body("Focus complete! Take a break.")
                    .show()
                    .ok();

                state.mode = crate::ipc::PomoMode::Break;
                state.end_time = Some(Instant::now() + config.break_duration);
            } else {
                Notification::new()
                    .summary("Taiga")
                    .body("All Pomodoros finished! Great work.")
                    .show()
                    .ok();

                reset_state(state);
            }
        }
        crate::ipc::PomoMode::Break => {
            Notification::new()
                .summary("Taiga")
                .body("Break over! Back to work.")
                .show()
                .ok();

            state.mode = crate::ipc::PomoMode::Focus;
            state.end_time = Some(Instant::now() + config.focus_duration);
        }
        crate::ipc::PomoMode::Idle => {
            reset_state(state);
        }
    }
}

fn reset_state(state: &mut TimerState) {
    state.mode = crate::ipc::PomoMode::Idle;
    state.end_time = None;
    state.paused_duration = None;
    state.cycles_remaining = 0;
}

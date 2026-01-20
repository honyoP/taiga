use directories::BaseDirs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum PomoMode {
    Focus,
    Break,
    Idle,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonCommand {
    Start {
        task_id: u32,
        focus_len: u64,
        break_len: u64,
        cycles: u32,
        no_gui: bool,
        no_sound: bool,
    },
    Status,
    Stop,
    Pause,
    Resume,
    Ping,
    Kill,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DaemonResponse {
    Ok(String),
    Error(String),
    Status {
        remaining_secs: u64,
        is_running: bool,
        mode: PomoMode,
        cycles_left: u32,
        task_id: Option<u32>,
    },
    Pong,
}

pub fn get_socket_path() -> String {
    if cfg!(windows) {
        String::from(r"\\.\pipe\taiga-pomo-daemon")
    } else {
        let base = BaseDirs::new().unwrap();
        let path = base
            .runtime_dir()
            .unwrap_or_else(|| base.cache_dir())
            .join("taiga-pomo.sock");
        path.to_string_lossy().to_string()
    }
}

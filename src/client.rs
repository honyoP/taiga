use crate::error::{Result, TaigaError};
use crate::ipc::{DaemonCommand, DaemonResponse, get_socket_path};
use interprocess::local_socket::traits::tokio::Stream as _;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ToFsName, ToNsName, tokio::Stream,
};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn send_command(cmd: DaemonCommand) -> Result<DaemonResponse> {
    let socket_path = get_socket_path();

    let stream_result = connect_to_daemon(&socket_path).await;

    let mut stream = match stream_result {
        Ok(s) => s,
        Err(_) => {
            println!("Daemon not running. Starting it...");
            spawn_daemon()?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            connect_to_daemon(&socket_path).await?
        }
    };

    let req_bytes = serde_json::to_vec(&cmd)?;
    stream.write_all(&req_bytes).await?;

    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer).await?;

    if n == 0 {
        return Err(TaigaError::Ipc(
            "Daemon closed connection without response".to_string(),
        ));
    }

    let resp: DaemonResponse = serde_json::from_slice(&buffer[0..n])?;

    Ok(resp)
}

async fn connect_to_daemon(path: &str) -> Result<Stream> {
    let stream = if cfg!(windows) {
        let name = path
            .to_ns_name::<GenericNamespaced>()
            .map_err(|e| TaigaError::Ipc(e.to_string()))?;
        Stream::connect(name)
            .await
            .map_err(|e| TaigaError::Ipc(e.to_string()))?
    } else {
        let name = path
            .to_fs_name::<GenericFilePath>()
            .map_err(|e| TaigaError::Ipc(e.to_string()))?;
        Stream::connect(name)
            .await
            .map_err(|e| TaigaError::Ipc(e.to_string()))?
    };
    Ok(stream)
}

fn spawn_daemon() -> Result<()> {
    let current_exe = std::env::current_exe()?;
    Command::new(current_exe)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

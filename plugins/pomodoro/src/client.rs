use crate::config::PomoConfig;
use crate::ipc::{DaemonCommand, DaemonResponse, get_socket_path};
use taiga_plugin_api::daemon::client::{DaemonClientConfig, send_command_with_autospawn};
use taiga_plugin_api::daemon::ipc::DaemonSpawnConfig;
use taiga_plugin_api::PluginError;

pub async fn send_command(cmd: DaemonCommand) -> Result<DaemonResponse, PluginError> {
    let config = PomoConfig::default();
    let socket_path = get_socket_path();

    let client_config = DaemonClientConfig::new(
        socket_path,
        DaemonSpawnConfig::new("pomo", "daemon"),
    )
    .with_startup_wait(config.timing.daemon_startup_wait_ms)
    .with_buffer_size(config.timing.ipc_buffer_size);

    send_command_with_autospawn(&client_config, &cmd).await
}

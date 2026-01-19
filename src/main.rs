use std::path::PathBuf;

use chrono::Local;
use chrono_english::{Dialect, parse_date_string};
use clap::Parser;

use crate::cli::{Cli, Commands, PomoCommands, WhenCommand};
use crate::error::{Result, TaigaError};
use crate::task::TaskRepository;

mod cli;
mod client;
mod config;
mod daemon;
mod error;
mod ipc;
mod task;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Daemon = cli.command {
        daemon::run_daemon().await?;
        return Ok(());
    }

    let cfg: config::Config = confy::load("taiga", None)?;
    let mut tasks_file_path = PathBuf::from(&cfg.data_directory);
    tasks_file_path.push(&cfg.task_filename);
    let mut repo = TaskRepository::load_from_file(&tasks_file_path)?;

    match cli.command {
        Commands::Add { title, when } => {
            let title_str = title.join(" ");
            let parsed_time = match when {
                Some(WhenCommand::When { scheduled }) => {
                    let date_str = scheduled.join(" ");
                    parse_date_string(&date_str, Local::now(), Dialect::Us).ok()
                }
                None => None,
            };

            repo.add(title_str, parsed_time);
            repo.save_to_file(&tasks_file_path)?;
            println!("Task saved.");
        }
        Commands::List { state } => {
            let tasks = repo.list_all();

            if tasks.is_empty() {
                println!("No tasks found.");
            } else {
                for task in tasks {
                    let should_show = match state.as_str() {
                        "open" => !task.is_complete,
                        "done" => task.is_complete,
                        _ => true,
                    };

                    if should_show {
                        print!("{}", task.to_md_line());
                    }
                }
            }
        }
        Commands::Check { id } => match repo.get_mut(id) {
            Some(task) => {
                task.is_complete = !task.is_complete;
                let status = if task.is_complete { "done" } else { "open" };
                println!("Marked task #{} as {}: {}", task.id, status, task.title);
                repo.save_to_file(&tasks_file_path)?;
            }
            None => {
                return Err(TaigaError::TaskNotFound(id));
            }
        },
        Commands::Remove { id } => match repo.remove(id) {
            Some(removed_task) => {
                println!("Removed: {}", removed_task.title);
                repo.save_to_file(&tasks_file_path)?;
            }
            None => {
                return Err(TaigaError::TaskNotFound(id));
            }
        },
        Commands::Pomo { action } => match action {
            PomoCommands::Start {
                focus,
                break_time,
                cycles,
            } => {
                let resp = client::send_command(ipc::DaemonCommand::Start {
                    task_id: 0,
                    focus_len: focus,
                    break_len: break_time,
                    cycles,
                })
                .await?;
                println!("{:?}", resp);
            }
            PomoCommands::Status => {
                let resp = client::send_command(ipc::DaemonCommand::Status).await?;
                println!("{:?}", resp);
            }
            PomoCommands::Stop => {
                client::send_command(ipc::DaemonCommand::Stop).await?;
                println!("Timer stopped.");
            }
            PomoCommands::Pause => {
                let resp = client::send_command(ipc::DaemonCommand::Pause).await?;
                println!("{:?}", resp);
            }
            PomoCommands::Resume => {
                let resp = client::send_command(ipc::DaemonCommand::Resume).await?;
                println!("{:?}", resp);
            }
            PomoCommands::Kill => {
                let resp = client::send_command(ipc::DaemonCommand::Kill).await?;
                println!("{:?}", resp);
            }
        },
        Commands::Daemon => unreachable!(),
    }

    Ok(())
}

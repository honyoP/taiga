use std::path::PathBuf;

use chrono::Local;
use chrono_english::{Dialect, parse_date_string};

use crate::task::TaskRepository;

mod cli;
mod client;
mod config;
mod daemon;
mod ipc;
mod task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = cli::cli().get_matches();

    if matches.subcommand_matches("daemon").is_some() {
        daemon::run_daemon().await?;
        return Ok(());
    }

    let cfg: config::Config = confy::load("taiga", None)?;
    let mut tasks_file_path = PathBuf::from(&cfg.data_directory);
    tasks_file_path.push(&cfg.task_filename);
    let mut repo = TaskRepository::load_from_file(&tasks_file_path)?;

    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            let title = sub_matches.get_one::<String>("TITLE").expect("required");

            println!("it hits the ADD");
            let parsed_time = match sub_matches.subcommand() {
                Some(("when", when_matches)) => {
                    println!("it hits the when");
                    let date_str = when_matches
                        .get_one::<String>("SCHEDULED")
                        .expect("required");
                    parse_date_string(date_str, Local::now(), Dialect::Us).ok()
                }
                _ => None,
            };

            println!("Adding {}", title);
            repo.add(title.to_string(), parsed_time);
            repo.save_to_file(&tasks_file_path)?;
            println!("Task saved.");
        }
        Some(("list", sub_matches)) => {
            let state = sub_matches
                .get_one::<String>("STATE")
                .map(|s| s.as_str())
                .unwrap_or("all");
            println!("Listing tasks [{}]", state);

            let tasks = repo.list_all();

            if tasks.is_empty() {
                println!("No tasks found.");
            } else {
                for task in tasks {
                    let should_show = match state {
                        "open" => !task.is_complete,
                        "done" => task.is_complete,
                        _ => true,
                    };

                    if should_show {
                        println!("{}", task.to_md_line());
                    }
                }
            }
        }

        Some(("check", sub_matches)) => {
            let id = *sub_matches.get_one::<u32>("ID").expect("required");

            match repo.get_mut(id) {
                Some(task) => {
                    if task.is_complete {
                        task.is_complete = false;
                        println!("Marked task #{} as open: {}", task.id, task.title);
                    } else {
                        task.is_complete = true;
                        println!("Marked task #{} as done: {}", task.id, task.title);
                    }

                    repo.save_to_file(&tasks_file_path)?;
                }
                None => {
                    println!("Error: Task #{} not found.", id);
                }
            }
        }

        Some(("remove", sub_matches)) => {
            let id = *sub_matches.get_one::<u32>("ID").expect("required");
            println!("Removing {}", id);

            match repo.remove(id) {
                Some(removed_task) => {
                    println!("Removed: {}", removed_task.title);
                    repo.save_to_file(&tasks_file_path)?;
                }
                None => {
                    println!("Error: Task with ID {} not found.", id);
                }
            }
        }

        Some(("pomo", sub_matches)) => {
            match sub_matches.subcommand() {
                Some(("start", start_matches)) => {
                    //start pomo
                    let focus_input: u64 =
                        start_matches.get_one::<String>("FOCUS").unwrap().parse()?;
                    let break_input: u64 =
                        start_matches.get_one::<String>("BREAK").unwrap().parse()?;
                    let cycles_input: u32 =
                        start_matches.get_one::<String>("CYCLES").unwrap().parse()?;
                    // IPC Call
                    let resp = client::send_command(ipc::DaemonCommand::Start {
                        task_id: 0, // Pass real ID if you have it
                        focus_len: focus_input,
                        break_len: break_input,
                        cycles: cycles_input,
                    })
                    .await?;
                    println!("{:?}", resp);
                }
                // TODO: Prettify all Pomo outputs
                Some(("status", _status_matches)) => {
                    let resp = client::send_command(ipc::DaemonCommand::Status).await?;
                    println!("{:?}", resp);
                }
                Some(("stop", _stop_matches)) => {
                    client::send_command(ipc::DaemonCommand::Stop).await?;
                    println!("Timer stopped.");
                }
                Some(("pause", _pause_matches)) => {
                    let resp = client::send_command(ipc::DaemonCommand::Pause).await?;
                    println!("{:?}", resp);
                }
                Some(("resume", _resume_matches)) => {
                    let resp = client::send_command(ipc::DaemonCommand::Resume).await?;
                    println!("{:?}", resp);
                }
                Some(("kill", _kill_matches)) => {
                    let resp = client::send_command(ipc::DaemonCommand::Kill).await?;
                    println!("{:?}", resp);
                }
                _ => {
                    println!("No command issued!");
                }
            }
        }
        _ => unreachable!(),
    };

    Ok(())
}

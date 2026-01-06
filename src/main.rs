use std::path::PathBuf;

use chrono::Local;
use chrono_english::{Dialect, parse_date_string};

use crate::task::TaskRepository;

mod cli;
mod config;
mod task;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg: config::Config = confy::load("taiga", None)?;
    let mut tasks_file_path = PathBuf::from(&cfg.data_directory);
    tasks_file_path.push(&cfg.task_filename);
    let mut repo = TaskRepository::load_from_file(&tasks_file_path)?;

    let matches = cli::cli().get_matches();
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
                    task.is_complete = true;
                    println!("Marked task #{} as done: {}", task.id, task.title);

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
        _ => unreachable!(),
    };

    Ok(())
}

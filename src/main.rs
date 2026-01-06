use std::path::PathBuf;

use chrono::Local;
use chrono_english::{Dialect, parse_date_string};

mod cli;
mod config;
mod task;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg: config::Config = confy::load("taiga", None)?;
    let mut task_path = PathBuf::from(&cfg.data_directory);
    task_path.push(&cfg.task_filename);
    println!("task path: {}", task_path.to_string_lossy());

    //let tasks = load_tasks(&task_path)?;

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
            task::add_task(&task_path, title.to_string(), parsed_time)?;
        }
        Some(("list", sub_matches)) => {
            /* filtering by state
                        println!(
                            "Listing Tasks: {}\n",
                            sub_matches.get_one::<String>("STATE").expect("required")
                        );
            */
            let tasks = task::load_tasks(&task_path)?;
            println!("{}", tasks);
        }
        Some(("remove", sub_matches)) => {
            println!(
                "Removing {}",
                sub_matches.get_one::<String>("TASK").expect("required")
            );
        }
        _ => unreachable!(),
    };

    Ok(())
}

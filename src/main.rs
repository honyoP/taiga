use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use chrono::{DateTime, Local};
use chrono_english::{Dialect, parse_date_string};

mod cli;
mod config;

pub struct Task {
    pub title: String,
    pub is_complete: bool,
    pub scheduled: Option<DateTime<Local>>,
    pub location: Option<String>,
}

impl Task {
    pub fn new(title: String) -> Self {
        Self {
            title,
            is_complete: false,
            scheduled: None,
            location: None,
        }
    }

    pub fn scheduled(mut self, date: Option<DateTime<Local>>) -> Self {
        self.scheduled = date;
        self
    }

    pub fn location(mut self, loc: Option<String>) -> Self {
        self.location = loc;
        self
    }

    pub fn to_md_line(&self) -> String {
        let check_mark = if self.is_complete { "x" } else { " " };
        match &self.scheduled {
            Some(dt) => format!(
                "- [{}] {} (Scheduled: {})\n",
                check_mark,
                self.title,
                dt.format("%Y-%m-%d")
            ),
            None => format!("- [{}] {}\n", check_mark, self.title,),
        }
    }
}

pub struct ParsedTask {
    pub title: String,
    pub scheduled: Option<DateTime<Local>>,
    pub location: Option<String>,
}

fn load_tasks(file_path: &PathBuf) -> io::Result<String> {
    let mut file = OpenOptions::new().read(true).write(true).open(file_path)?;

    let mut content = String::new();
    file.read_to_string(&mut content)?;

    Ok(content)
}

fn add_task(
    file_path: &PathBuf,
    task_title: String,
    task_schedule: Option<DateTime<Local>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let new_task = Task::new(task_title).scheduled(task_schedule);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)?;

    file.write_all(new_task.to_md_line().as_bytes())?;

    println!("Task added successfully!");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg: config::Config = confy::load("taiginator", None)?;
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
            add_task(&task_path, title.to_string(), parsed_time);
        }
        Some(("list", sub_matches)) => {
            /* filtering by state
                        println!(
                            "Listing Tasks: {}\n",
                            sub_matches.get_one::<String>("STATE").expect("required")
                        );
            */
            let tasks = load_tasks(&task_path)?;
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

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::error::{Result, TaigaError};

static TASK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[ID:(\d+)\] - \[(.)\] (.*?)(?: \(Scheduled: (.*)\))?$")
        .expect("Invalid regex pattern")
});

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub is_complete: bool,
    pub scheduled: Option<DateTime<Local>>,
}

impl Task {
    pub fn new(title: String) -> Self {
        Self {
            id: 0,
            title,
            is_complete: false,
            scheduled: None,
        }
    }

    pub fn scheduled(mut self, date: Option<DateTime<Local>>) -> Self {
        self.scheduled = date;
        self
    }

    pub fn to_md_line(&self) -> String {
        let check_mark = if self.is_complete { "x" } else { " " };
        match &self.scheduled {
            Some(dt) => format!(
                "[ID:{}] - [{}] {} (Scheduled: {})\n",
                self.id,
                check_mark,
                self.title,
                dt.format("%Y-%m-%d")
            ),
            None => format!("[ID:{}] - [{}] {}\n", self.id, check_mark, self.title,),
        }
    }

    pub fn from_md_line(line: &str) -> Result<Self> {
        let caps = TASK_REGEX
            .captures(line)
            .ok_or_else(|| TaigaError::Parse(format!("Invalid task format: {}", line)))?;

        let id = caps
            .get(1)
            .ok_or_else(|| TaigaError::Parse("Missing task ID".to_string()))?
            .as_str()
            .parse::<u32>()
            .map_err(|e| TaigaError::Parse(format!("Invalid task ID: {}", e)))?;

        let is_complete = caps
            .get(2)
            .ok_or_else(|| TaigaError::Parse("Missing completion status".to_string()))?
            .as_str()
            == "x";

        let title = caps
            .get(3)
            .ok_or_else(|| TaigaError::Parse("Missing task title".to_string()))?
            .as_str()
            .to_string();

        let scheduled = match caps.get(4) {
            Some(m) => {
                let date_str = m.as_str();
                NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .and_then(|dt| Local.from_local_datetime(&dt).single())
            }
            None => None,
        };

        Ok(Task {
            id,
            title,
            is_complete,
            scheduled,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TaskRepository {
    pub tasks: HashMap<u32, Task>,
    pub next_id: u32,
}

impl TaskRepository {
    pub fn new() -> Self {
        TaskRepository {
            tasks: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn add(&mut self, title: String, scheduled: Option<DateTime<Local>>) {
        let id = self.next_id;

        let task = Task {
            id,
            title,
            is_complete: false,
            scheduled,
        };

        self.tasks.insert(id, task);
        self.next_id += 1;
    }

    pub fn get(&self, id: u32) -> Option<&Task> {
        self.tasks.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    pub fn remove(&mut self, id: u32) -> Option<Task> {
        self.tasks.remove(&id)
    }

    pub fn list_all(&self) -> Vec<&Task> {
        let mut list: Vec<&Task> = self.tasks.values().collect();
        list.sort_by_key(|t| t.id);
        list
    }

    pub fn load_from_file(file_path: &PathBuf) -> Result<TaskRepository> {
        let mut repo = TaskRepository::new();

        if !file_path.exists() {
            return Ok(repo);
        }

        let file = std::fs::File::open(file_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line: String = line?;
            if line.trim().is_empty() {
                continue;
            }

            match Task::from_md_line(&line) {
                Ok(task) => {
                    if task.id >= repo.next_id {
                        repo.next_id = task.id + 1;
                    }
                    repo.tasks.insert(task.id, task);
                }
                Err(e) => {
                    eprintln!("Warning: Skipping invalid task line: {}", e);
                }
            }
        }

        Ok(repo)
    }

    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        for task in self.list_all() {
            writeln!(file, "{}", task.to_md_line())?;
        }

        Ok(())
    }
}

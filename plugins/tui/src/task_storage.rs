//! Task storage for TUI plugin
//!
//! Handles loading and saving tasks from the markdown file.

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub is_complete: bool,
    pub scheduled: Option<DateTime<Local>>,
}

impl Task {
    pub fn to_md_line(&self) -> String {
        let status = if self.is_complete { "x" } else { " " };
        let schedule = self
            .scheduled
            .map(|dt| format!(" (Scheduled: {})", dt.format("%Y-%m-%d")))
            .unwrap_or_default();

        format!("[ID:{}] - [{}] {}{}", self.id, status, self.title, schedule)
    }

    pub fn from_md_line(line: &str) -> Option<Self> {
        let re = Regex::new(
            r"^\[ID:(\d+)\]\s*-\s*\[([ xX])\]\s*(.+?)(?:\s*\(Scheduled:\s*(\d{4}-\d{2}-\d{2})\))?$",
        )
        .ok()?;

        let caps = re.captures(line.trim())?;

        let id: u32 = caps.get(1)?.as_str().parse().ok()?;
        let is_complete = caps.get(2)?.as_str().to_lowercase() == "x";
        let title = caps.get(3)?.as_str().trim().to_string();

        let scheduled = caps.get(4).and_then(|m| {
            NaiveDate::parse_from_str(m.as_str(), "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .and_then(|dt| Local.from_local_datetime(&dt).single())
        });

        Some(Task {
            id,
            title,
            is_complete,
            scheduled,
        })
    }
}

pub struct TaskStorage {
    pub tasks: HashMap<u32, Task>,
    pub file_path: PathBuf,
    next_id: u32,
}

impl TaskStorage {
    pub fn new(data_dir: &PathBuf, task_filename: &str) -> Self {
        let mut file_path = data_dir.clone();
        file_path.push(task_filename);

        Self {
            tasks: HashMap::new(),
            file_path,
            next_id: 1,
        }
    }

    pub fn load(&mut self) -> Result<(), String> {
        if !self.file_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Failed to read tasks file: {}", e))?;

        self.tasks.clear();
        let mut max_id = 0u32;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(task) = Task::from_md_line(line) {
                if task.id > max_id {
                    max_id = task.id;
                }
                self.tasks.insert(task.id, task);
            }
        }

        self.next_id = max_id + 1;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        // Create backup first
        if self.file_path.exists() {
            let backup_path = self.file_path.with_extension("md.bak");
            fs::copy(&self.file_path, &backup_path)
                .map_err(|e| format!("Failed to create backup: {}", e))?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.file_path)
            .map_err(|e| format!("Failed to open tasks file: {}", e))?;

        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by_key(|t| t.id);

        for task in tasks {
            writeln!(file, "{}", task.to_md_line())
                .map_err(|e| format!("Failed to write task: {}", e))?;
        }

        Ok(())
    }

    pub fn add_task(&mut self, title: String, scheduled: Option<DateTime<Local>>) -> u32 {
        let id = self.find_next_id();
        let task = Task {
            id,
            title,
            is_complete: false,
            scheduled,
        };
        self.tasks.insert(id, task);
        self.update_next_id();
        id
    }

    pub fn remove_task(&mut self, id: u32) -> Option<Task> {
        self.tasks.remove(&id)
    }

    pub fn toggle_task(&mut self, id: u32) -> Option<bool> {
        self.tasks.get_mut(&id).map(|task| {
            task.is_complete = !task.is_complete;
            task.is_complete
        })
    }

    pub fn update_task(
        &mut self,
        id: u32,
        title: Option<String>,
        scheduled: Option<Option<DateTime<Local>>>,
    ) {
        if let Some(task) = self.tasks.get_mut(&id) {
            if let Some(new_title) = title {
                task.title = new_title;
            }
            if let Some(new_scheduled) = scheduled {
                task.scheduled = new_scheduled;
            }
        }
    }

    pub fn get_tasks_sorted(&self) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.tasks.values().collect();
        tasks.sort_by_key(|t| t.id);
        tasks
    }

    pub fn clear_completed(&mut self) -> usize {
        let to_remove: Vec<u32> = self
            .tasks
            .iter()
            .filter(|(_, task)| task.is_complete)
            .map(|(id, _)| *id)
            .collect();

        let count = to_remove.len();
        for id in to_remove {
            self.tasks.remove(&id);
        }
        count
    }

    fn find_next_id(&self) -> u32 {
        for id in 1..=self.next_id {
            if !self.tasks.contains_key(&id) {
                return id;
            }
        }
        self.next_id
    }

    fn update_next_id(&mut self) {
        if let Some(&max_id) = self.tasks.keys().max() {
            self.next_id = max_id + 1;
        } else {
            self.next_id = 1;
        }
    }
}

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::error::{Result, TaigaError};

// Regex pattern is validated at compile time - invalid patterns are programming errors
static TASK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[ID:(\d+)\] - \[(.)\] (.*?)(?: \(Scheduled: (.*)\))?$")
        .expect("Invalid regex pattern - this is a compile-time constant")
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
            .ok_or_else(|| TaigaError::parse(format!("Invalid task format: {}", line)))?;

        let id = caps
            .get(1)
            .ok_or_else(|| TaigaError::parse("Missing task ID"))?
            .as_str()
            .parse::<u32>()
            .map_err(|e| TaigaError::parse_with_source("Invalid task ID", e))?;

        let is_complete = caps
            .get(2)
            .ok_or_else(|| TaigaError::parse("Missing completion status"))?
            .as_str()
            == "x";

        let title = caps
            .get(3)
            .ok_or_else(|| TaigaError::parse("Missing task title"))?
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
        let id = self.find_next_id();

        let task = Task {
            id,
            title,
            is_complete: false,
            scheduled,
        };

        self.tasks.insert(id, task);
        self.update_next_id();
    }

    /// Find the next available ID (reuses gaps)
    fn find_next_id(&self) -> u32 {
        // Start from 1 and find first gap
        for id in 1..=self.next_id {
            if !self.tasks.contains_key(&id) {
                return id;
            }
        }
        self.next_id
    }

    /// Update next_id to be one more than the maximum used ID
    fn update_next_id(&mut self) {
        if let Some(&max_id) = self.tasks.keys().max() {
            self.next_id = max_id + 1;
        } else {
            self.next_id = 1;
        }
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
        // Create backup before saving
        self.create_backup(path)?;

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

    /// Create a backup of the tasks file
    fn create_backup(&self, path: &PathBuf) -> Result<()> {
        if !path.exists() {
            return Ok(()); // Nothing to backup
        }

        let backup_path = path.with_extension("md.bak");
        std::fs::copy(path, &backup_path)?;
        Ok(())
    }

    /// Recover tasks from backup file
    pub fn recover_from_backup(path: &PathBuf) -> Result<TaskRepository> {
        let backup_path = path.with_extension("md.bak");

        if !backup_path.exists() {
            return Err(TaigaError::parse("Backup file not found"));
        }

        TaskRepository::load_from_file(&backup_path)
    }

    /// Reindex all tasks to sequential IDs starting from 1
    pub fn reindex(&mut self) {
        let mut tasks: Vec<Task> = self.tasks.drain().map(|(_, t)| t).collect();
        tasks.sort_by_key(|t| t.id);

        for (new_id, task) in tasks.into_iter().enumerate() {
            let mut task = task;
            task.id = (new_id + 1) as u32;
            self.tasks.insert(task.id, task);
        }

        self.update_next_id();
    }

    /// Remove all checked/completed tasks
    pub fn remove_checked(&mut self) -> usize {
        let to_remove: Vec<u32> = self.tasks
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

    /// Get tasks filtered and sorted
    pub fn get_filtered_sorted(
        &self,
        filter_checked: Option<bool>,
        filter_scheduled: Option<bool>,
        filter_overdue: bool,
        search_term: Option<&str>,
        sort_by: &str,
        reverse: bool,
    ) -> Vec<&Task> {
        let today = Local::now().date_naive();

        let mut tasks: Vec<&Task> = self.tasks
            .values()
            .filter(|task| {
                // Filter by completion status
                if let Some(checked) = filter_checked {
                    if task.is_complete != checked {
                        return false;
                    }
                }

                // Filter by scheduled status
                if let Some(has_schedule) = filter_scheduled {
                    if task.scheduled.is_some() != has_schedule {
                        return false;
                    }
                }

                // Filter overdue
                if filter_overdue {
                    if let Some(dt) = task.scheduled {
                        if dt.date_naive() >= today || task.is_complete {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }

                // Filter by search term
                if let Some(term) = search_term {
                    if !task.title.to_lowercase().contains(&term.to_lowercase()) {
                        return false;
                    }
                }

                true
            })
            .collect();

        // Sort tasks
        match sort_by {
            "id" => tasks.sort_by_key(|t| t.id),
            "date" => tasks.sort_by(|a, b| {
                match (&a.scheduled, &b.scheduled) {
                    (Some(a_dt), Some(b_dt)) => a_dt.cmp(b_dt),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.id.cmp(&b.id),
                }
            }),
            "name" => tasks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
            "status" => tasks.sort_by_key(|t| (t.is_complete, t.id)),
            _ => tasks.sort_by_key(|t| t.id),
        }

        if reverse {
            tasks.reverse();
        }

        tasks
    }

    /// Count overdue tasks
    pub fn count_overdue(&self) -> usize {
        let today = Local::now().date_naive();
        self.tasks
            .values()
            .filter(|task| {
                if let Some(dt) = task.scheduled {
                    dt.date_naive() < today && !task.is_complete
                } else {
                    false
                }
            })
            .count()
    }

    /// Count completed tasks
    pub fn count_completed(&self) -> usize {
        self.tasks.values().filter(|task| task.is_complete).count()
    }
}

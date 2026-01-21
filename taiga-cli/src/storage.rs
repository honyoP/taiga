//! Markdown file storage adapter for TaskCollection
//!
//! Handles persistence of tasks to markdown files.

use chrono::{Local, NaiveDate, TimeZone};
use regex::Regex;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use taiga_core::{Task, TaskCollection};

use crate::error::{CliError, Result};

// Regex pattern is validated at compile time - invalid patterns are programming errors
static TASK_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[ID:(\d+)\] - \[(.)\] (.*?)(?: \(Scheduled: (.*)\))?$")
        .expect("Invalid regex pattern - this is a compile-time constant")
});

/// Markdown storage adapter
pub struct MarkdownStorage {
    path: PathBuf,
}

impl MarkdownStorage {
    /// Create a new storage adapter for the given path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the storage path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load tasks from the markdown file
    pub fn load(&self) -> Result<TaskCollection> {
        let mut collection = TaskCollection::new();

        if !self.path.exists() {
            return Ok(collection);
        }

        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line: String = line?;
            if line.trim().is_empty() {
                continue;
            }

            match parse_task_line(&line) {
                Ok(task) => {
                    collection.insert(task);
                }
                Err(e) => {
                    eprintln!("Warning: Skipping invalid task line: {}", e);
                }
            }
        }

        Ok(collection)
    }

    /// Save tasks to the markdown file
    pub fn save(&self, collection: &TaskCollection) -> Result<()> {
        // Create backup before saving
        self.backup()?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.path)?;

        for task in collection.list_all() {
            writeln!(file, "{}", format_task_line(task))?;
        }

        Ok(())
    }

    /// Create a backup of the tasks file
    pub fn backup(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(()); // Nothing to backup
        }

        let backup_path = self.path.with_extension("md.bak");
        std::fs::copy(&self.path, &backup_path)?;
        Ok(())
    }

    /// Recover tasks from backup file
    pub fn recover(&self) -> Result<TaskCollection> {
        let backup_path = self.path.with_extension("md.bak");

        if !backup_path.exists() {
            return Err(CliError::storage("Backup file not found"));
        }

        let backup_storage = MarkdownStorage::new(backup_path);
        backup_storage.load()
    }

    /// Check if backup exists
    pub fn backup_exists(&self) -> bool {
        self.path.with_extension("md.bak").exists()
    }
}

/// Parse a markdown line into a Task
fn parse_task_line(line: &str) -> Result<Task> {
    let caps = TASK_REGEX
        .captures(line)
        .ok_or_else(|| CliError::parse(format!("Invalid task format: {}", line)))?;

    let id = caps
        .get(1)
        .ok_or_else(|| CliError::parse("Missing task ID"))?
        .as_str()
        .parse::<u32>()
        .map_err(|e| CliError::parse_with_source("Invalid task ID", e))?;

    let is_complete = caps
        .get(2)
        .ok_or_else(|| CliError::parse("Missing completion status"))?
        .as_str()
        == "x";

    let title = caps
        .get(3)
        .ok_or_else(|| CliError::parse("Missing task title"))?
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

    Ok(Task::new(title)
        .with_id(id)
        .with_complete(is_complete)
        .with_scheduled(scheduled))
}

/// Format a Task as a markdown line
fn format_task_line(task: &Task) -> String {
    let check_mark = if task.is_complete { "x" } else { " " };
    match &task.scheduled {
        Some(dt) => format!(
            "[ID:{}] - [{}] {} (Scheduled: {})",
            task.id,
            check_mark,
            task.title,
            dt.format("%Y-%m-%d")
        ),
        None => format!("[ID:{}] - [{}] {}", task.id, check_mark, task.title),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task_line_simple() {
        let line = "[ID:1] - [ ] Simple task";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.id, 1);
        assert_eq!(task.title, "Simple task");
        assert!(!task.is_complete);
        assert!(task.scheduled.is_none());
    }

    #[test]
    fn test_parse_task_line_completed() {
        let line = "[ID:2] - [x] Completed task";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.id, 2);
        assert!(task.is_complete);
    }

    #[test]
    fn test_parse_task_line_scheduled() {
        let line = "[ID:3] - [ ] Scheduled task (Scheduled: 2026-01-25)";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.id, 3);
        assert!(task.scheduled.is_some());
        assert_eq!(
            task.scheduled.unwrap().date_naive(),
            NaiveDate::from_ymd_opt(2026, 1, 25).unwrap()
        );
    }

    #[test]
    fn test_format_task_line_simple() {
        let task = Task::new("Simple task").with_id(1);
        let line = format_task_line(&task);

        assert_eq!(line, "[ID:1] - [ ] Simple task");
    }

    #[test]
    fn test_format_task_line_completed() {
        let task = Task::new("Done").with_id(2).with_complete(true);
        let line = format_task_line(&task);

        assert_eq!(line, "[ID:2] - [x] Done");
    }

    #[test]
    fn test_roundtrip() {
        let original = Task::new("Test task").with_id(5).with_complete(true);

        let line = format_task_line(&original);
        let parsed = parse_task_line(&line).unwrap();

        assert_eq!(original.id, parsed.id);
        assert_eq!(original.title, parsed.title);
        assert_eq!(original.is_complete, parsed.is_complete);
    }
}

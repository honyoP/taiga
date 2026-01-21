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

// Category header pattern: ## Category Name
static CATEGORY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^##\s+(.+)$").expect("Invalid category regex pattern")
});

// Tag pattern: #word (alphanumeric and underscores)
static TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#(\w+)").expect("Invalid tag regex pattern")
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

        let mut current_category: Option<String> = None;

        for line in reader.lines() {
            let line: String = line?;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            // Check for category header
            if let Some(caps) = CATEGORY_REGEX.captures(trimmed) {
                let cat_name = caps.get(1).map(|m| m.as_str().trim().to_string());
                // "Uncategorized" header maps to None
                current_category = cat_name.filter(|s| s.to_lowercase() != "uncategorized");
                continue;
            }

            match parse_task_line(&line, current_category.clone()) {
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

        // Group tasks by category
        let mut categorized: std::collections::BTreeMap<Option<String>, Vec<&Task>> =
            std::collections::BTreeMap::new();

        for task in collection.list_all() {
            categorized
                .entry(task.category.clone())
                .or_default()
                .push(task);
        }

        // Sort categories: named categories alphabetically, then Uncategorized (None) last
        let mut categories: Vec<Option<String>> = categorized.keys().cloned().collect();
        categories.sort_by(|a, b| match (a, b) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(a_cat), Some(b_cat)) => a_cat.to_lowercase().cmp(&b_cat.to_lowercase()),
        });

        let mut first_category = true;
        for category in categories {
            // Write category header
            if !first_category {
                writeln!(file)?;
            }
            first_category = false;

            let header_name = category.as_deref().unwrap_or("Uncategorized");
            writeln!(file, "## {}", header_name)?;

            // Write tasks in this category
            if let Some(tasks) = categorized.get(&category) {
                for task in tasks {
                    writeln!(file, "{}", format_task_line(task))?;
                }
            }
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
fn parse_task_line(line: &str, category: Option<String>) -> Result<Task> {
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

    let raw_title = caps
        .get(3)
        .ok_or_else(|| CliError::parse("Missing task title"))?
        .as_str();

    // Extract tags from title
    let tags: Vec<String> = TAG_REGEX
        .captures_iter(raw_title)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();

    // Remove tags from title to get clean title
    let title = TAG_REGEX.replace_all(raw_title, "").trim().to_string();

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
        .with_scheduled(scheduled)
        .with_category(category)
        .with_tags(tags))
}

/// Format a Task as a markdown line
fn format_task_line(task: &Task) -> String {
    let check_mark = if task.is_complete { "x" } else { " " };

    // Build tags string
    let tags_str = if task.tags.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            task.tags
                .iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };

    match &task.scheduled {
        Some(dt) => format!(
            "[ID:{}] - [{}] {}{} (Scheduled: {})",
            task.id,
            check_mark,
            task.title,
            tags_str,
            dt.format("%Y-%m-%d")
        ),
        None => format!("[ID:{}] - [{}] {}{}", task.id, check_mark, task.title, tags_str),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task_line_simple() {
        let line = "[ID:1] - [ ] Simple task";
        let task = parse_task_line(line, None).unwrap();

        assert_eq!(task.id, 1);
        assert_eq!(task.title, "Simple task");
        assert!(!task.is_complete);
        assert!(task.scheduled.is_none());
        assert!(task.category.is_none());
        assert!(task.tags.is_empty());
    }

    #[test]
    fn test_parse_task_line_completed() {
        let line = "[ID:2] - [x] Completed task";
        let task = parse_task_line(line, None).unwrap();

        assert_eq!(task.id, 2);
        assert!(task.is_complete);
    }

    #[test]
    fn test_parse_task_line_scheduled() {
        let line = "[ID:3] - [ ] Scheduled task (Scheduled: 2026-01-25)";
        let task = parse_task_line(line, None).unwrap();

        assert_eq!(task.id, 3);
        assert!(task.scheduled.is_some());
        assert_eq!(
            task.scheduled.unwrap().date_naive(),
            NaiveDate::from_ymd_opt(2026, 1, 25).unwrap()
        );
    }

    #[test]
    fn test_parse_task_line_with_tags() {
        let line = "[ID:4] - [ ] Complete report #urgent #finance";
        let task = parse_task_line(line, Some("Work".to_string())).unwrap();

        assert_eq!(task.id, 4);
        assert_eq!(task.title, "Complete report");
        assert_eq!(task.category, Some("Work".to_string()));
        assert_eq!(task.tags, vec!["urgent", "finance"]);
    }

    #[test]
    fn test_parse_task_line_with_tags_and_schedule() {
        let line = "[ID:5] - [ ] Buy groceries #shopping (Scheduled: 2026-01-25)";
        let task = parse_task_line(line, Some("Personal".to_string())).unwrap();

        assert_eq!(task.id, 5);
        assert_eq!(task.title, "Buy groceries");
        assert_eq!(task.category, Some("Personal".to_string()));
        assert_eq!(task.tags, vec!["shopping"]);
        assert!(task.scheduled.is_some());
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
    fn test_format_task_line_with_tags() {
        let task = Task::new("Complete report")
            .with_id(3)
            .with_tags(vec!["urgent".to_string(), "finance".to_string()]);
        let line = format_task_line(&task);

        assert_eq!(line, "[ID:3] - [ ] Complete report #urgent #finance");
    }

    #[test]
    fn test_roundtrip() {
        let original = Task::new("Test task").with_id(5).with_complete(true);

        let line = format_task_line(&original);
        let parsed = parse_task_line(&line, None).unwrap();

        assert_eq!(original.id, parsed.id);
        assert_eq!(original.title, parsed.title);
        assert_eq!(original.is_complete, parsed.is_complete);
    }

    #[test]
    fn test_roundtrip_with_tags() {
        let original = Task::new("Test task")
            .with_id(6)
            .with_tags(vec!["urgent".to_string(), "work".to_string()])
            .with_category(Some("Work".to_string()));

        let line = format_task_line(&original);
        let parsed = parse_task_line(&line, original.category.clone()).unwrap();

        assert_eq!(original.id, parsed.id);
        assert_eq!(original.title, parsed.title);
        assert_eq!(original.tags, parsed.tags);
        assert_eq!(original.category, parsed.category);
    }
}

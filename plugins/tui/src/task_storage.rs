//! Task storage for TUI plugin
//!
//! Handles loading and saving tasks from the markdown file.

use chrono::{DateTime, Local, NaiveDate, TimeZone};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::LazyLock;

// Category header pattern: ## Category Name
static CATEGORY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^##\s+(.+)$").expect("Invalid category regex pattern")
});

// Tag pattern: #word (alphanumeric and underscores)
static TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#(\w+)").expect("Invalid tag regex pattern")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub is_complete: bool,
    pub scheduled: Option<DateTime<Local>>,
    pub category: Option<String>,
    pub tags: Vec<String>,
}

impl Task {
    pub fn to_md_line(&self) -> String {
        let status = if self.is_complete { "x" } else { " " };

        // Build tags string
        let tags_str = if self.tags.is_empty() {
            String::new()
        } else {
            format!(
                " {}",
                self.tags
                    .iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };

        let schedule = self
            .scheduled
            .map(|dt| format!(" (Scheduled: {})", dt.format("%Y-%m-%d")))
            .unwrap_or_default();

        format!("[ID:{}] - [{}] {}{}{}", self.id, status, self.title, tags_str, schedule)
    }

    pub fn from_md_line(line: &str, category: Option<String>) -> Option<Self> {
        let re = Regex::new(
            r"^\[ID:(\d+)\]\s*-\s*\[([ xX])\]\s*(.+?)(?:\s*\(Scheduled:\s*(\d{4}-\d{2}-\d{2})\))?$",
        )
        .ok()?;

        let caps = re.captures(line.trim())?;

        let id: u32 = caps.get(1)?.as_str().parse().ok()?;
        let is_complete = caps.get(2)?.as_str().to_lowercase() == "x";
        let raw_title = caps.get(3)?.as_str().trim();

        // Extract tags from title
        let tags: Vec<String> = TAG_REGEX
            .captures_iter(raw_title)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect();

        // Remove tags from title to get clean title
        let title = TAG_REGEX.replace_all(raw_title, "").trim().to_string();

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
            category,
            tags,
        })
    }

    /// Add a tag to this task
    pub fn add_tag(&mut self, tag: &str) {
        let tag = tag.trim_start_matches('#').to_string();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remove a tag from this task
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        let tag = tag.trim_start_matches('#');
        if let Some(pos) = self.tags.iter().position(|t| t == tag) {
            self.tags.remove(pos);
            true
        } else {
            false
        }
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
        let mut current_category: Option<String> = None;

        for line in content.lines() {
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

            if let Some(task) = Task::from_md_line(line, current_category.clone()) {
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

        // Group tasks by category
        let mut categorized: BTreeMap<Option<String>, Vec<&Task>> = BTreeMap::new();

        let mut all_tasks: Vec<&Task> = self.tasks.values().collect();
        all_tasks.sort_by_key(|t| t.id);

        for task in all_tasks {
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
                writeln!(file).map_err(|e| format!("Failed to write newline: {}", e))?;
            }
            first_category = false;

            let header_name = category.as_deref().unwrap_or("Uncategorized");
            writeln!(file, "## {}", header_name)
                .map_err(|e| format!("Failed to write category header: {}", e))?;

            // Write tasks in this category
            if let Some(tasks) = categorized.get(&category) {
                for task in tasks {
                    writeln!(file, "{}", task.to_md_line())
                        .map_err(|e| format!("Failed to write task: {}", e))?;
                }
            }
        }

        Ok(())
    }

    pub fn add_task(&mut self, title: String, scheduled: Option<DateTime<Local>>) -> u32 {
        self.add_task_with_category_tags(title, scheduled, None, Vec::new())
    }

    pub fn add_task_with_category_tags(
        &mut self,
        title: String,
        scheduled: Option<DateTime<Local>>,
        category: Option<String>,
        tags: Vec<String>,
    ) -> u32 {
        let id = self.find_next_id();
        let task = Task {
            id,
            title,
            is_complete: false,
            scheduled,
            category,
            tags,
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

    /// Get unique categories sorted alphabetically
    pub fn get_categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self
            .tasks
            .values()
            .filter_map(|t| t.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        categories
    }

    /// Get all unique tags sorted alphabetically
    pub fn get_all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .tasks
            .values()
            .flat_map(|t| t.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Get count of tasks in a specific category (None = uncategorized)
    pub fn count_in_category(&self, category: Option<&str>) -> usize {
        self.tasks
            .values()
            .filter(|t| t.category.as_deref() == category)
            .count()
    }

    /// Get count of tasks with a specific tag
    pub fn count_with_tag(&self, tag: &str) -> usize {
        self.tasks
            .values()
            .filter(|t| t.tags.iter().any(|t_tag| t_tag == tag))
            .count()
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

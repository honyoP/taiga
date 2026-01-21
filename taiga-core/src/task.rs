//! Task domain model
//!
//! Pure domain logic for task management with no I/O operations.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{CoreError, Result};

/// Newtype wrapper for task IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u32);

impl From<u32> for TaskId {
    fn from(id: u32) -> Self {
        TaskId(id)
    }
}

impl From<TaskId> for u32 {
    fn from(id: TaskId) -> Self {
        id.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single task
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub is_complete: bool,
    pub scheduled: Option<DateTime<Local>>,
    /// Category this task belongs to (None = "Uncategorized")
    pub category: Option<String>,
    /// Tags associated with this task (without # prefix)
    pub tags: Vec<String>,
}

impl Task {
    /// Create a new task with the given title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: 0,
            title: title.into(),
            is_complete: false,
            scheduled: None,
            category: None,
            tags: Vec::new(),
        }
    }

    /// Builder method to set scheduled date
    pub fn with_scheduled(mut self, date: Option<DateTime<Local>>) -> Self {
        self.scheduled = date;
        self
    }

    /// Builder method to set task ID
    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    /// Builder method to set completion status
    pub fn with_complete(mut self, complete: bool) -> Self {
        self.is_complete = complete;
        self
    }

    /// Builder method to set category
    pub fn with_category(mut self, category: Option<String>) -> Self {
        self.category = category;
        self
    }

    /// Builder method to set tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
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

    /// Toggle completion status
    pub fn toggle_complete(&mut self) {
        self.is_complete = !self.is_complete;
    }

    /// Check if task is overdue
    pub fn is_overdue(&self) -> bool {
        if let Some(dt) = self.scheduled {
            dt.date_naive() < Local::now().date_naive() && !self.is_complete
        } else {
            false
        }
    }
}

/// In-memory collection of tasks
///
/// This is a pure domain model with no I/O operations.
/// Persistence is handled by storage adapters in consuming crates.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct TaskCollection {
    pub tasks: HashMap<u32, Task>,
    pub next_id: u32,
}

impl TaskCollection {
    /// Create a new empty task collection
    pub fn new() -> Self {
        TaskCollection {
            tasks: HashMap::new(),
            next_id: 1,
        }
    }

    /// Add a new task with the given title and optional scheduled date
    pub fn add(&mut self, title: impl Into<String>, scheduled: Option<DateTime<Local>>) -> u32 {
        self.add_with_category_tags(title, scheduled, None, Vec::new())
    }

    /// Add a new task with category and tags
    pub fn add_with_category_tags(
        &mut self,
        title: impl Into<String>,
        scheduled: Option<DateTime<Local>>,
        category: Option<String>,
        tags: Vec<String>,
    ) -> u32 {
        let id = self.find_next_id();

        let task = Task {
            id,
            title: title.into(),
            is_complete: false,
            scheduled,
            category,
            tags,
        };

        self.tasks.insert(id, task);
        self.update_next_id();
        id
    }

    /// Add an existing task to the collection
    pub fn insert(&mut self, task: Task) {
        if task.id >= self.next_id {
            self.next_id = task.id + 1;
        }
        self.tasks.insert(task.id, task);
    }

    /// Find the next available ID (reuses gaps)
    fn find_next_id(&self) -> u32 {
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

    /// Get a task by ID
    pub fn get(&self, id: u32) -> Option<&Task> {
        self.tasks.get(&id)
    }

    /// Get a mutable reference to a task by ID
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Task> {
        self.tasks.get_mut(&id)
    }

    /// Remove a task by ID
    pub fn remove(&mut self, id: u32) -> Option<Task> {
        self.tasks.remove(&id)
    }

    /// Get all tasks sorted by ID
    pub fn list_all(&self) -> Vec<&Task> {
        let mut list: Vec<&Task> = self.tasks.values().collect();
        list.sort_by_key(|t| t.id);
        list
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

    /// Remove all checked/completed tasks, returns count of removed tasks
    pub fn remove_checked(&mut self) -> usize {
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

    /// Count total tasks
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Check if collection is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Count overdue tasks
    pub fn count_overdue(&self) -> usize {
        self.tasks.values().filter(|task| task.is_overdue()).count()
    }

    /// Count completed tasks
    pub fn count_completed(&self) -> usize {
        self.tasks.values().filter(|task| task.is_complete).count()
    }

    /// Get or return error if task not found
    pub fn get_or_err(&self, id: u32) -> Result<&Task> {
        self.get(id).ok_or(CoreError::TaskNotFound(id))
    }

    /// Get mutable or return error if task not found
    pub fn get_mut_or_err(&mut self, id: u32) -> Result<&mut Task> {
        self.get_mut(id).ok_or(CoreError::TaskNotFound(id))
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

    /// Move a task to a different category
    pub fn move_to_category(&mut self, id: u32, category: Option<String>) -> Result<()> {
        let task = self.get_mut_or_err(id)?;
        task.category = category;
        Ok(())
    }

    /// Get tasks in a specific category (None = uncategorized)
    pub fn tasks_in_category(&self, category: Option<&str>) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| t.category.as_deref() == category)
            .collect()
    }

    /// Get tasks with a specific tag
    pub fn tasks_with_tag(&self, tag: &str) -> Vec<&Task> {
        self.tasks
            .values()
            .filter(|t| t.tags.iter().any(|t_tag| t_tag == tag))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("Test task")
            .with_id(1)
            .with_complete(false);

        assert_eq!(task.id, 1);
        assert_eq!(task.title, "Test task");
        assert!(!task.is_complete);
        assert!(task.scheduled.is_none());
    }

    #[test]
    fn test_task_toggle() {
        let mut task = Task::new("Test");
        assert!(!task.is_complete);

        task.toggle_complete();
        assert!(task.is_complete);

        task.toggle_complete();
        assert!(!task.is_complete);
    }

    #[test]
    fn test_collection_add() {
        let mut collection = TaskCollection::new();

        let id1 = collection.add("Task 1", None);
        let id2 = collection.add("Task 2", None);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(collection.len(), 2);
    }

    #[test]
    fn test_collection_id_reuse() {
        let mut collection = TaskCollection::new();

        collection.add("Task 1", None);
        let id2 = collection.add("Task 2", None);
        collection.add("Task 3", None);

        // Remove task 2
        collection.remove(id2);

        // Next task should reuse ID 2
        let id_new = collection.add("Task 4", None);
        assert_eq!(id_new, 2);
    }

    #[test]
    fn test_collection_reindex() {
        let mut collection = TaskCollection::new();

        collection.add("Task 1", None);
        collection.add("Task 2", None);
        collection.add("Task 3", None);

        // Remove task 2 to create a gap
        collection.remove(2);

        // Reindex
        collection.reindex();

        // Should now have IDs 1 and 2
        let tasks = collection.list_all();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, 1);
        assert_eq!(tasks[1].id, 2);
    }

    #[test]
    fn test_remove_checked() {
        let mut collection = TaskCollection::new();

        collection.add("Task 1", None);
        collection.add("Task 2", None);
        collection.add("Task 3", None);

        // Mark tasks 1 and 3 as complete
        collection.get_mut(1).unwrap().is_complete = true;
        collection.get_mut(3).unwrap().is_complete = true;

        let removed = collection.remove_checked();

        assert_eq!(removed, 2);
        assert_eq!(collection.len(), 1);
        assert!(collection.get(2).is_some());
    }
}

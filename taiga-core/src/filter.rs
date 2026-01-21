//! Task filtering and sorting logic
//!
//! Provides a builder-style API for filtering and sorting tasks.

use chrono::Local;

use crate::task::Task;

/// Sort order for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskSort {
    #[default]
    Id,
    Date,
    Name,
    Status,
}

impl TaskSort {
    /// Create from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "date" => Self::Date,
            "name" => Self::Name,
            "status" => Self::Status,
            _ => Self::Id,
        }
    }
}

/// Builder for filtering tasks
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    /// Filter by completion status (Some(true) = completed, Some(false) = incomplete)
    pub checked: Option<bool>,
    /// Filter by scheduled status (Some(true) = has date, Some(false) = no date)
    pub scheduled: Option<bool>,
    /// Filter to only show overdue tasks
    pub overdue: bool,
    /// Search term for title (case-insensitive)
    pub search: Option<String>,
    /// Sort order
    pub sort: TaskSort,
    /// Reverse sort order
    pub reverse: bool,
    /// Filter by category (Some(Some("Work")) = in "Work", Some(None) = uncategorized)
    pub category: Option<Option<String>>,
    /// Filter by tags (all must match)
    pub tags: Vec<String>,
}

impl TaskFilter {
    /// Create a new filter with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter to only show completed tasks
    pub fn completed(mut self) -> Self {
        self.checked = Some(true);
        self
    }

    /// Filter to only show incomplete tasks
    pub fn incomplete(mut self) -> Self {
        self.checked = Some(false);
        self
    }

    /// Set completion filter
    pub fn with_checked(mut self, checked: Option<bool>) -> Self {
        self.checked = checked;
        self
    }

    /// Filter to only show scheduled tasks
    pub fn with_schedule(mut self) -> Self {
        self.scheduled = Some(true);
        self
    }

    /// Filter to only show unscheduled tasks
    pub fn without_schedule(mut self) -> Self {
        self.scheduled = Some(false);
        self
    }

    /// Set scheduled filter
    pub fn with_scheduled(mut self, scheduled: Option<bool>) -> Self {
        self.scheduled = scheduled;
        self
    }

    /// Filter to only show overdue tasks
    pub fn overdue_only(mut self) -> Self {
        self.overdue = true;
        self
    }

    /// Set overdue filter
    pub fn with_overdue(mut self, overdue: bool) -> Self {
        self.overdue = overdue;
        self
    }

    /// Filter by search term
    pub fn search(mut self, term: impl Into<String>) -> Self {
        self.search = Some(term.into());
        self
    }

    /// Set search term
    pub fn with_search(mut self, term: Option<String>) -> Self {
        self.search = term;
        self
    }

    /// Sort by given field
    pub fn sort_by(mut self, sort: TaskSort) -> Self {
        self.sort = sort;
        self
    }

    /// Reverse sort order
    pub fn reversed(mut self) -> Self {
        self.reverse = true;
        self
    }

    /// Set reverse flag
    pub fn with_reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    /// Filter by exact category
    pub fn in_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(Some(category.into()));
        self
    }

    /// Filter to uncategorized tasks only
    pub fn uncategorized(mut self) -> Self {
        self.category = Some(None);
        self
    }

    /// Set category filter
    pub fn with_category(mut self, category: Option<Option<String>>) -> Self {
        self.category = category;
        self
    }

    /// Filter by tag (must have this tag)
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set tags filter (all must match)
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Check if a task matches this filter
    pub fn matches(&self, task: &Task) -> bool {
        let today = Local::now().date_naive();

        // Filter by completion status
        if let Some(checked) = self.checked {
            if task.is_complete != checked {
                return false;
            }
        }

        // Filter by scheduled status
        if let Some(has_schedule) = self.scheduled {
            if task.scheduled.is_some() != has_schedule {
                return false;
            }
        }

        // Filter overdue
        if self.overdue {
            if let Some(dt) = task.scheduled {
                if dt.date_naive() >= today || task.is_complete {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Filter by search term
        if let Some(term) = &self.search {
            if !task.title.to_lowercase().contains(&term.to_lowercase()) {
                return false;
            }
        }

        // Filter by category
        if let Some(ref cat_filter) = self.category {
            if task.category.as_ref() != cat_filter.as_ref() {
                return false;
            }
        }

        // Filter by tags (all must match)
        for tag in &self.tags {
            if !task.tags.iter().any(|t| t == tag) {
                return false;
            }
        }

        true
    }

    /// Apply filter and sort to a collection of tasks
    pub fn apply<'a>(&self, tasks: impl Iterator<Item = &'a Task>) -> Vec<&'a Task> {
        let mut filtered: Vec<&Task> = tasks.filter(|t| self.matches(t)).collect();

        // Sort tasks
        match self.sort {
            TaskSort::Id => filtered.sort_by_key(|t| t.id),
            TaskSort::Date => filtered.sort_by(|a, b| match (&a.scheduled, &b.scheduled) {
                (Some(a_dt), Some(b_dt)) => a_dt.cmp(b_dt),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.id.cmp(&b.id),
            }),
            TaskSort::Name => {
                filtered.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }
            TaskSort::Status => filtered.sort_by_key(|t| (t.is_complete, t.id)),
        }

        if self.reverse {
            filtered.reverse();
        }

        filtered
    }
}

/// Extension trait for TaskCollection to support filtering
pub trait FilterExt {
    /// Get tasks filtered and sorted according to the filter
    fn get_filtered(&self, filter: &TaskFilter) -> Vec<&Task>;

    /// Get tasks with legacy parameters (for backwards compatibility)
    fn get_filtered_sorted(
        &self,
        filter_checked: Option<bool>,
        filter_scheduled: Option<bool>,
        filter_overdue: bool,
        search_term: Option<&str>,
        sort_by: &str,
        reverse: bool,
    ) -> Vec<&Task>;
}

impl FilterExt for crate::task::TaskCollection {
    fn get_filtered(&self, filter: &TaskFilter) -> Vec<&Task> {
        filter.apply(self.tasks.values())
    }

    fn get_filtered_sorted(
        &self,
        filter_checked: Option<bool>,
        filter_scheduled: Option<bool>,
        filter_overdue: bool,
        search_term: Option<&str>,
        sort_by: &str,
        reverse: bool,
    ) -> Vec<&Task> {
        let filter = TaskFilter::new()
            .with_checked(filter_checked)
            .with_scheduled(filter_scheduled)
            .with_overdue(filter_overdue)
            .with_search(search_term.map(|s| s.to_string()))
            .sort_by(TaskSort::from_str(sort_by))
            .with_reverse(reverse);

        self.get_filtered(&filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskCollection;

    #[test]
    fn test_filter_completed() {
        let mut collection = TaskCollection::new();
        collection.add("Task 1", None);
        collection.add("Task 2", None);
        collection.get_mut(1).unwrap().is_complete = true;

        let filter = TaskFilter::new().completed();
        let results = collection.get_filtered(&filter);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_filter_search() {
        let mut collection = TaskCollection::new();
        collection.add("Buy groceries", None);
        collection.add("Call mom", None);
        collection.add("Buy present", None);

        let filter = TaskFilter::new().search("buy");
        let results = collection.get_filtered(&filter);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_sort_by_name() {
        let mut collection = TaskCollection::new();
        collection.add("Zebra", None);
        collection.add("Apple", None);
        collection.add("Mango", None);

        let filter = TaskFilter::new().sort_by(TaskSort::Name);
        let results = collection.get_filtered(&filter);

        assert_eq!(results[0].title, "Apple");
        assert_eq!(results[1].title, "Mango");
        assert_eq!(results[2].title, "Zebra");
    }

    #[test]
    fn test_reverse_sort() {
        let mut collection = TaskCollection::new();
        collection.add("Task 1", None);
        collection.add("Task 2", None);
        collection.add("Task 3", None);

        let filter = TaskFilter::new().reversed();
        let results = collection.get_filtered(&filter);

        assert_eq!(results[0].id, 3);
        assert_eq!(results[1].id, 2);
        assert_eq!(results[2].id, 1);
    }
}

//! Task display formatting module
//!
//! Handles colored output and different view modes for tasks

use chrono::Local;
use colored::*;

use taiga_core::date::format_date_human;
use taiga_core::Task;

/// Display mode for task list
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayMode {
    /// Minimal one-line format
    Compact,
    /// Full info with formatted dates
    Detailed,
    /// Balanced view with clear status indicators (default)
    Default,
}

/// Check if terminal supports colors
pub fn supports_color() -> bool {
    atty::is(atty::Stream::Stdout)
}

/// Format a task for display
pub fn format_task(task: &Task, mode: DisplayMode, use_color: bool) -> String {
    let today = Local::now().date_naive();

    let checkbox = if task.is_complete { "[✓]" } else { "[ ]" };

    let status_info = match &task.scheduled {
        Some(dt) => {
            let date = dt.date_naive();
            let diff_days = date.signed_duration_since(today).num_days();

            let date_str = match mode {
                DisplayMode::Compact => format_date_human(date, false),
                _ => format_date_human(date, true),
            };

            // Determine color based on status
            if use_color {
                if task.is_complete {
                    format!("({})", date_str).green().to_string()
                } else if diff_days < 0 {
                    format!("({})", date_str).red().bold().to_string()
                } else if diff_days <= 1 {
                    format!("({})", date_str).yellow().to_string()
                } else {
                    format!("({})", date_str).normal().to_string()
                }
            } else {
                format!("({})", date_str)
            }
        }
        None => String::new(),
    };

    let title = if use_color && task.is_complete {
        task.title.green().to_string()
    } else {
        task.title.clone()
    };

    // Format tags
    let tags_str = if task.tags.is_empty() {
        String::new()
    } else if use_color {
        format!(
            " {}",
            task.tags
                .iter()
                .map(|t| format!("#{}", t).magenta().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )
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

    match mode {
        DisplayMode::Compact => {
            format!("{} [{}] {}{}", checkbox, task.id, title, tags_str)
        }
        DisplayMode::Detailed => {
            let mut parts = vec![
                format!("{} [ID: {}]", checkbox, task.id),
                format!("Title: {}{}", title, tags_str),
            ];

            if let Some(cat) = &task.category {
                parts.push(format!("Category: {}", cat));
            }

            if !task.tags.is_empty() {
                parts.push(format!("Tags: {}", task.tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")));
            }

            if let Some(dt) = &task.scheduled {
                parts.push(format!(
                    "Scheduled: {} {}",
                    dt.format("%Y-%m-%d"),
                    status_info
                ));
            } else {
                parts.push("Scheduled: (none)".to_string());
            }

            parts.push(format!(
                "Status: {}",
                if task.is_complete {
                    "Complete"
                } else {
                    "Incomplete"
                }
            ));
            parts.join("\n  ")
        }
        DisplayMode::Default => {
            let id_str = if use_color {
                format!("[{}]", task.id).cyan().to_string()
            } else {
                format!("[{}]", task.id)
            };

            if status_info.is_empty() {
                format!("{} {} {}{}", checkbox, id_str, title, tags_str)
            } else {
                format!("{} {} {}{} {}", checkbox, id_str, title, tags_str, status_info)
            }
        }
    }
}

/// Format a summary line for task list
pub fn format_summary(total: usize, completed: usize, overdue: usize, use_color: bool) -> String {
    let parts = vec![
        format!("{} total", total),
        if use_color {
            format!("{} done", completed).green().to_string()
        } else {
            format!("{} done", completed)
        },
        if overdue > 0 {
            if use_color {
                format!("{} overdue", overdue).red().to_string()
            } else {
                format!("{} overdue", overdue)
            }
        } else {
            String::new()
        },
    ];

    let summary: Vec<&str> = parts.iter().filter(|s| !s.is_empty()).map(|s| s.as_str()).collect();

    format!("[{}]", summary.join(" | "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_task_compact() {
        let task = Task::new("Test task").with_id(1);

        let output = format_task(&task, DisplayMode::Compact, false);
        assert!(output.contains("[ ]"));
        assert!(output.contains("[1]"));
        assert!(output.contains("Test task"));
    }

    #[test]
    fn test_format_task_completed() {
        let task = Task::new("Done task").with_id(2).with_complete(true);

        let output = format_task(&task, DisplayMode::Default, false);
        assert!(output.contains("[✓]"));
    }

    #[test]
    fn test_format_summary() {
        let summary = format_summary(10, 5, 2, false);
        assert!(summary.contains("10 total"));
        assert!(summary.contains("5 done"));
        assert!(summary.contains("2 overdue"));
    }
}

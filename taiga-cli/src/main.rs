use std::io::{self, Write};
use std::path::PathBuf;

use chrono::{Local, TimeZone};
use clap::Parser;

use taiga_core::date::parse_date;
use taiga_core::filter::FilterExt;

use crate::cli::{Cli, Commands, SortBy};
use crate::display::{format_summary, format_task, supports_color, DisplayMode};
use crate::error::{CliError, Result};
use crate::plugin::{CommandResult, PluginContext};
use crate::plugin_manager::PluginManager;
use crate::storage::MarkdownStorage;

mod cli;
mod config;
mod display;
mod error;
mod plugin;
mod plugin_manager;
mod storage;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let cfg: config::Config = confy::load("taiga", None)?;
    let mut tasks_file_path = PathBuf::from(&cfg.data_directory);
    tasks_file_path.push(&cfg.task_filename);

    let storage = MarkdownStorage::new(&tasks_file_path);

    // Initialize plugin manager
    let mut plugin_manager = PluginManager::new();

    // Add plugin search paths
    let data_plugins_dir = PathBuf::from(&cfg.data_directory).join("plugins");
    plugin_manager.add_plugin_path(&data_plugins_dir);

    // Check for plugins next to executable
    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        plugin_manager.add_plugin_path(exe_dir.join("plugins"));
    }

    // Check for plugins in project target directory (for development)
    let dev_plugin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug");
    plugin_manager.add_plugin_path(&dev_plugin_path);

    // Discover and load plugins (silently)
    if let Err(e) = plugin_manager.discover_plugins() {
        eprintln!("Warning: Error discovering plugins: {}", e);
    }

    // Create plugin context with task filename
    let mut plugin_ctx = PluginContext::new(PathBuf::from(&cfg.data_directory))
        .with_extra("task_filename", &cfg.task_filename);

    match cli.command {
        Commands::Add { title, on, date } => {
            let mut collection = storage.load()?;
            let title_str = title.join(" ");

            // Use --on or --date (--on takes precedence)
            let date_str = on.or(date);
            let scheduled = if let Some(date_input) = date_str {
                Some(
                    parse_date(&date_input)?
                        .and_hms_opt(0, 0, 0)
                        .and_then(|dt| Local.from_local_datetime(&dt).single())
                        .ok_or_else(|| CliError::parse("Invalid date/time"))?,
                )
            } else {
                None
            };

            collection.add(title_str.clone(), scheduled);
            storage.save(&collection)?;

            if let Some(dt) = scheduled {
                println!(
                    "Task added: {} (scheduled: {})",
                    title_str,
                    dt.format("%Y-%m-%d")
                );
            } else {
                println!("Task added: {}", title_str);
            }
        }

        Commands::List {
            checked,
            unchecked,
            scheduled,
            unscheduled,
            overdue,
            search,
            sort,
            reverse,
            compact,
            detailed,
            no_color,
        } => {
            let collection = storage.load()?;

            // Determine filters
            let filter_checked = if checked {
                Some(true)
            } else if unchecked {
                Some(false)
            } else {
                None
            };

            let filter_scheduled = if scheduled {
                Some(true)
            } else if unscheduled {
                Some(false)
            } else {
                None
            };

            // Get sort key
            let sort_key = match sort {
                SortBy::Id => "id",
                SortBy::Date => "date",
                SortBy::Name => "name",
                SortBy::Status => "status",
            };

            let tasks = collection.get_filtered_sorted(
                filter_checked,
                filter_scheduled,
                overdue,
                search.as_deref(),
                sort_key,
                reverse,
            );

            if tasks.is_empty() {
                println!("No tasks found.");
            } else {
                // Determine display mode
                let mode = if compact {
                    DisplayMode::Compact
                } else if detailed {
                    DisplayMode::Detailed
                } else {
                    DisplayMode::Default
                };

                let use_color = !no_color && supports_color();

                for task in &tasks {
                    println!("{}", format_task(task, mode, use_color));
                }

                // Show summary
                println!();
                let summary = format_summary(
                    tasks.len(),
                    tasks.iter().filter(|t| t.is_complete).count(),
                    tasks.iter().filter(|t| t.is_overdue()).count(),
                    use_color,
                );
                println!("{}", summary);
            }
        }

        Commands::Check { id } => {
            let mut collection = storage.load()?;
            let task = collection.get_mut_or_err(id)?;
            task.toggle_complete();
            let status = if task.is_complete { "done" } else { "open" };
            println!("Marked task #{} as {}: {}", task.id, status, task.title);
            storage.save(&collection)?;
        }

        Commands::Remove { id } => {
            let mut collection = storage.load()?;
            let removed_task = collection
                .remove(id)
                .ok_or(CliError::TaskNotFound(id))?;
            println!("Removed: {}", removed_task.title);
            storage.save(&collection)?;
        }

        Commands::Edit { id, name, date } => {
            if name.is_none() && date.is_none() {
                return Err(CliError::validation(
                    "edit",
                    "At least one of --name or --date must be provided",
                ));
            }

            let mut collection = storage.load()?;
            let task = collection.get_mut_or_err(id)?;

            if let Some(new_name) = name {
                task.title = new_name;
            }

            if let Some(date_str) = date {
                if date_str.to_lowercase() == "none" {
                    task.scheduled = None;
                } else {
                    let date = parse_date(&date_str)?;
                    task.scheduled = date
                        .and_hms_opt(0, 0, 0)
                        .and_then(|dt| Local.from_local_datetime(&dt).single());
                }
            }

            println!("Updated task #{}: {}", task.id, task.title);
            if let Some(dt) = &task.scheduled {
                println!("  Scheduled: {}", dt.format("%Y-%m-%d"));
            }
            storage.save(&collection)?;
        }

        Commands::Reschedule { id, date } => {
            let mut collection = storage.load()?;
            let date_str = date.join(" ");

            let task = collection.get_mut_or_err(id)?;

            if date_str.to_lowercase() == "none" {
                task.scheduled = None;
                println!("Cleared schedule for task #{}: {}", task.id, task.title);
            } else {
                let parsed_date = parse_date(&date_str)?;
                task.scheduled = parsed_date
                    .and_hms_opt(0, 0, 0)
                    .and_then(|dt| Local.from_local_datetime(&dt).single());

                println!(
                    "Rescheduled task #{} to {}: {}",
                    task.id,
                    parsed_date.format("%Y-%m-%d"),
                    task.title
                );
            }
            storage.save(&collection)?;
        }

        Commands::Rename { id, name } => {
            let mut collection = storage.load()?;
            let new_name = name.join(" ");

            let task = collection.get_mut_or_err(id)?;
            let old_name = task.title.clone();
            task.title = new_name.clone();
            println!("Renamed task #{}:", id);
            println!("  From: {}", old_name);
            println!("  To:   {}", new_name);
            storage.save(&collection)?;
        }

        Commands::Clear { checked, force } => {
            if !checked {
                return Err(CliError::validation(
                    "clear",
                    "Use --checked to remove completed tasks",
                ));
            }

            let mut collection = storage.load()?;
            let count = collection.count_completed();

            if count == 0 {
                println!("No completed tasks to remove.");
                return Ok(());
            }

            if !force && !confirm(&format!("Remove {} completed task(s)?", count))? {
                println!("Cancelled.");
                return Ok(());
            }

            let removed = collection.remove_checked();
            storage.save(&collection)?;
            println!("Removed {} completed task(s).", removed);
        }

        Commands::Recover { force } => {
            if !storage.backup_exists() {
                return Err(CliError::parse("No backup file found"));
            }

            if !force && !confirm("Restore tasks from backup? Current tasks will be replaced.")? {
                println!("Cancelled.");
                return Ok(());
            }

            let backup_collection = storage.recover()?;
            storage.save(&backup_collection)?;
            println!("Recovered {} tasks from backup.", backup_collection.len());
        }

        Commands::Reindex { force } => {
            let mut collection = storage.load()?;

            if !force
                && !confirm("Renumber all task IDs sequentially? This cannot be undone.")?
            {
                println!("Cancelled.");
                return Ok(());
            }

            collection.reindex();
            storage.save(&collection)?;
            println!("Reindexed {} tasks.", collection.len());
        }

        Commands::Plugins => {
            let plugins = plugin_manager.plugin_infos();
            if plugins.is_empty() {
                println!("No plugins loaded.");
                println!("\nPlugin search paths:");
                println!("  - {}", data_plugins_dir.display());
                if let Ok(exe_path) = std::env::current_exe()
                    && let Some(exe_dir) = exe_path.parent()
                {
                    println!("  - {}", exe_dir.join("plugins").display());
                }
            } else {
                println!("Loaded plugins:\n");
                for info in plugins {
                    println!("  {} v{}", info.name, info.version);
                    println!("    {}", info.description);
                    println!("    Commands:");
                    for cmd in &info.commands {
                        let usage = cmd.usage.as_deref().unwrap_or("");
                        println!("      {} {} - {}", cmd.name, usage, cmd.description);
                    }
                    println!();
                }
            }
        }

        Commands::External(args) => {
            if args.is_empty() {
                return Err(CliError::plugin("No command specified".to_string()));
            }

            let plugin_name = &args[0];
            let (command, cmd_args) = if args.len() > 1 {
                (&args[1], &args[2..])
            } else {
                // If only plugin name given, try "help" or show error
                return Err(CliError::plugin(format!(
                    "Usage: taiga {} <command> [args...]",
                    plugin_name
                )));
            };

            if !plugin_manager.has_plugin(plugin_name) {
                return Err(CliError::plugin(format!(
                    "Unknown command or plugin: '{}'\n\nRun 'taiga --help' for usage.",
                    plugin_name
                )));
            }

            let result = plugin_manager.execute(plugin_name, command, cmd_args, &mut plugin_ctx)?;

            match result {
                CommandResult::Success(Some(msg)) => println!("{}", msg),
                CommandResult::Success(None) => {}
                CommandResult::Error(msg) => {
                    return Err(CliError::plugin(msg));
                }
                CommandResult::Async(msg) => println!("{}", msg),
            }
        }
    }

    Ok(())
}

/// Ask user for confirmation
fn confirm(prompt: &str) -> Result<bool> {
    print!("{} [y/N] ", prompt);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "y")
}

//! TUI Application state and event handling

use crate::task_storage::{Task, TaskStorage};
use crate::ui;
use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use taiga_plugin_api::PluginContext;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterMode {
    All,
    Unchecked,
    Checked,
    Scheduled,
    Overdue,
}

impl FilterMode {
    pub fn next(&self) -> Self {
        match self {
            FilterMode::All => FilterMode::Unchecked,
            FilterMode::Unchecked => FilterMode::Checked,
            FilterMode::Checked => FilterMode::Scheduled,
            FilterMode::Scheduled => FilterMode::Overdue,
            FilterMode::Overdue => FilterMode::All,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            FilterMode::All => "All",
            FilterMode::Unchecked => "Incomplete",
            FilterMode::Checked => "Complete",
            FilterMode::Scheduled => "Scheduled",
            FilterMode::Overdue => "Overdue",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortMode {
    Id,
    Date,
    Name,
    Status,
}

impl SortMode {
    pub fn next(&self) -> Self {
        match self {
            SortMode::Id => SortMode::Date,
            SortMode::Date => SortMode::Name,
            SortMode::Name => SortMode::Status,
            SortMode::Status => SortMode::Id,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            SortMode::Id => "ID",
            SortMode::Date => "Date",
            SortMode::Name => "Name",
            SortMode::Status => "Status",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogMode {
    None,
    AddTask { name: String, date: String },
    EditTask { id: u32, name: String, date: String, field: usize },
    DeleteConfirm { id: u32 },
    ClearConfirm,
    Help,
    MoveCategory { task_id: u32, categories: Vec<String>, selected: usize },
    AddTag { task_id: u32, input: String },
    RemoveTag { task_id: u32, tags: Vec<String>, selected: usize },
}

/// Sidebar section selection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarSection {
    Categories,
    Tags,
}

pub struct App {
    pub storage: TaskStorage,
    pub selected_index: usize,
    pub filter_mode: FilterMode,
    pub sort_mode: SortMode,
    pub search_query: String,
    pub is_searching: bool,
    pub dialog: DialogMode,
    pub should_quit: bool,
    pub error_message: Option<String>,
    filtered_tasks: Vec<u32>,
    // Sidebar state
    pub categories: Vec<String>,
    pub all_tags: Vec<String>,
    pub selected_category: Option<Option<String>>, // None = "All", Some(None) = "Uncategorized", Some(Some("X")) = category X
    pub selected_tag_filter: Option<String>,
    pub sidebar_focused: bool,
    pub sidebar_section: SidebarSection,
    pub sidebar_selection: usize,
}

impl App {
    pub fn new(data_dir: PathBuf, task_filename: &str) -> Self {
        let storage = TaskStorage::new(&data_dir, task_filename);
        Self {
            storage,
            selected_index: 0,
            filter_mode: FilterMode::All,
            sort_mode: SortMode::Id,
            search_query: String::new(),
            is_searching: false,
            dialog: DialogMode::None,
            should_quit: false,
            error_message: None,
            filtered_tasks: Vec::new(),
            categories: Vec::new(),
            all_tags: Vec::new(),
            selected_category: None,
            selected_tag_filter: None,
            sidebar_focused: false,
            sidebar_section: SidebarSection::Categories,
            sidebar_selection: 0,
        }
    }

    pub fn load_tasks(&mut self) -> Result<(), String> {
        self.storage.load()?;
        self.update_categories_tags();
        self.update_filtered_tasks();
        Ok(())
    }

    /// Update the category and tag lists from storage
    pub fn update_categories_tags(&mut self) {
        self.categories = self.storage.get_categories();
        self.all_tags = self.storage.get_all_tags();
    }

    pub fn save_tasks(&mut self) -> Result<(), String> {
        self.storage.save()
    }

    pub fn update_filtered_tasks(&mut self) {
        let today = Local::now().date_naive();
        let search_lower = self.search_query.to_lowercase();

        let mut tasks: Vec<&Task> = self.storage.tasks.values()
            .filter(|task| {
                // Apply search filter
                if !self.search_query.is_empty() {
                    if !task.title.to_lowercase().contains(&search_lower) {
                        return false;
                    }
                }

                // Apply category filter
                if let Some(ref cat_filter) = self.selected_category {
                    if task.category.as_ref() != cat_filter.as_ref() {
                        return false;
                    }
                }

                // Apply tag filter
                if let Some(ref tag_filter) = self.selected_tag_filter {
                    if !task.tags.iter().any(|t| t == tag_filter) {
                        return false;
                    }
                }

                // Apply filter mode
                match self.filter_mode {
                    FilterMode::All => true,
                    FilterMode::Unchecked => !task.is_complete,
                    FilterMode::Checked => task.is_complete,
                    FilterMode::Scheduled => task.scheduled.is_some(),
                    FilterMode::Overdue => {
                        if let Some(dt) = task.scheduled {
                            dt.date_naive() < today && !task.is_complete
                        } else {
                            false
                        }
                    }
                }
            })
            .collect();

        // Sort tasks
        match self.sort_mode {
            SortMode::Id => tasks.sort_by_key(|t| t.id),
            SortMode::Date => tasks.sort_by(|a, b| {
                match (&a.scheduled, &b.scheduled) {
                    (Some(a_dt), Some(b_dt)) => a_dt.cmp(b_dt),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.id.cmp(&b.id),
                }
            }),
            SortMode::Name => tasks.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
            SortMode::Status => tasks.sort_by_key(|t| (t.is_complete, t.id)),
        }

        self.filtered_tasks = tasks.iter().map(|t| t.id).collect();

        // Adjust selection if needed
        if self.selected_index >= self.filtered_tasks.len() && !self.filtered_tasks.is_empty() {
            self.selected_index = self.filtered_tasks.len() - 1;
        }
    }

    pub fn get_visible_tasks(&self) -> Vec<&Task> {
        self.filtered_tasks.iter()
            .filter_map(|id| self.storage.tasks.get(id))
            .collect()
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.filtered_tasks.get(self.selected_index)
            .and_then(|id| self.storage.tasks.get(id))
    }

    pub fn selected_task_id(&self) -> Option<u32> {
        self.filtered_tasks.get(self.selected_index).copied()
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.filtered_tasks.len();
        if len == 0 {
            return;
        }

        let new_index = if delta < 0 {
            self.selected_index.saturating_sub((-delta) as usize)
        } else {
            (self.selected_index + delta as usize).min(len - 1)
        };

        self.selected_index = new_index;
    }

    pub fn toggle_selected(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.storage.toggle_task(id);
            self.update_filtered_tasks();
            if let Err(e) = self.save_tasks() {
                self.error_message = Some(format!("Failed to save: {}", e));
            }
        }
    }

    pub fn delete_selected(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.storage.remove_task(id);
            self.update_filtered_tasks();
            if let Err(e) = self.save_tasks() {
                self.error_message = Some(format!("Failed to save: {}", e));
            }
        }
        self.dialog = DialogMode::None;
    }

    pub fn add_task(&mut self, name: String, date_str: String) {
        if name.trim().is_empty() {
            self.error_message = Some("Task name cannot be empty".to_string());
            return;
        }

        let scheduled = parse_date_input(&date_str);
        self.storage.add_task(name, scheduled);
        self.update_filtered_tasks();

        if let Err(e) = self.save_tasks() {
            self.error_message = Some(format!("Failed to save: {}", e));
        }
        self.dialog = DialogMode::None;
    }

    pub fn update_task(&mut self, id: u32, name: String, date_str: String) {
        if name.trim().is_empty() {
            self.error_message = Some("Task name cannot be empty".to_string());
            return;
        }

        let scheduled = if date_str.trim().is_empty() || date_str.to_lowercase() == "none" {
            Some(None)
        } else {
            Some(parse_date_input(&date_str))
        };

        self.storage.update_task(id, Some(name), scheduled);
        self.update_filtered_tasks();

        if let Err(e) = self.save_tasks() {
            self.error_message = Some(format!("Failed to save: {}", e));
        }
        self.dialog = DialogMode::None;
    }

    pub fn clear_completed(&mut self) {
        let count = self.storage.clear_completed();
        self.update_filtered_tasks();

        if let Err(e) = self.save_tasks() {
            self.error_message = Some(format!("Failed to save: {}", e));
        } else if count > 0 {
            self.error_message = Some(format!("Cleared {} completed task(s)", count));
        }
        self.dialog = DialogMode::None;
    }

    pub fn cycle_filter(&mut self) {
        self.filter_mode = self.filter_mode.next();
        self.update_filtered_tasks();
    }

    pub fn cycle_sort(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.update_filtered_tasks();
    }

    pub fn start_search(&mut self) {
        self.is_searching = true;
    }

    pub fn end_search(&mut self) {
        self.is_searching = false;
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.is_searching = false;
        self.update_filtered_tasks();
    }

    pub fn refresh(&mut self) {
        if let Err(e) = self.load_tasks() {
            self.error_message = Some(format!("Failed to reload: {}", e));
        }
    }

    pub fn task_count(&self) -> usize {
        self.storage.tasks.len()
    }

    pub fn completed_count(&self) -> usize {
        self.storage.tasks.values().filter(|t| t.is_complete).count()
    }

    pub fn overdue_count(&self) -> usize {
        let today = Local::now().date_naive();
        self.storage.tasks.values()
            .filter(|t| {
                if let Some(dt) = t.scheduled {
                    dt.date_naive() < today && !t.is_complete
                } else {
                    false
                }
            })
            .count()
    }

    // Sidebar navigation methods

    /// Toggle focus between sidebar and task list
    pub fn toggle_sidebar_focus(&mut self) {
        self.sidebar_focused = !self.sidebar_focused;
        if self.sidebar_focused {
            self.sidebar_selection = 0;
        }
    }

    /// Move sidebar selection up or down
    pub fn move_sidebar_selection(&mut self, delta: i32) {
        let max_items = match self.sidebar_section {
            SidebarSection::Categories => self.categories.len() + 2, // +2 for "All" and "Uncategorized"
            SidebarSection::Tags => self.all_tags.len() + 1, // +1 for "All"
        };

        if max_items == 0 {
            return;
        }

        let new_selection = if delta < 0 {
            self.sidebar_selection.saturating_sub((-delta) as usize)
        } else {
            (self.sidebar_selection + delta as usize).min(max_items - 1)
        };

        self.sidebar_selection = new_selection;
    }

    /// Switch between Categories and Tags sections in sidebar
    pub fn toggle_sidebar_section(&mut self) {
        self.sidebar_section = match self.sidebar_section {
            SidebarSection::Categories => SidebarSection::Tags,
            SidebarSection::Tags => SidebarSection::Categories,
        };
        self.sidebar_selection = 0;
    }

    /// Select the current sidebar item (apply filter)
    pub fn select_sidebar_item(&mut self) {
        match self.sidebar_section {
            SidebarSection::Categories => {
                // 0 = "All", 1 = "Uncategorized", 2+ = specific category
                if self.sidebar_selection == 0 {
                    self.selected_category = None;
                } else if self.sidebar_selection == 1 {
                    self.selected_category = Some(None); // Uncategorized
                } else {
                    let idx = self.sidebar_selection - 2;
                    if idx < self.categories.len() {
                        self.selected_category = Some(Some(self.categories[idx].clone()));
                    }
                }
                self.selected_tag_filter = None;
            }
            SidebarSection::Tags => {
                // 0 = "All", 1+ = specific tag
                if self.sidebar_selection == 0 {
                    self.selected_tag_filter = None;
                } else {
                    let idx = self.sidebar_selection - 1;
                    if idx < self.all_tags.len() {
                        self.selected_tag_filter = Some(self.all_tags[idx].clone());
                    }
                }
            }
        }
        self.selected_index = 0;
        self.update_filtered_tasks();
    }

    /// Open move category dialog for selected task
    pub fn open_move_category_dialog(&mut self) {
        if let Some(id) = self.selected_task_id() {
            let mut categories = self.categories.clone();
            categories.insert(0, "Uncategorized".to_string());
            self.dialog = DialogMode::MoveCategory {
                task_id: id,
                categories,
                selected: 0,
            };
        }
    }

    /// Open add tag dialog for selected task
    pub fn open_add_tag_dialog(&mut self) {
        if let Some(id) = self.selected_task_id() {
            self.dialog = DialogMode::AddTag {
                task_id: id,
                input: String::new(),
            };
        }
    }

    /// Open remove tag dialog for selected task
    pub fn open_remove_tag_dialog(&mut self) {
        if let Some(task) = self.selected_task() {
            if !task.tags.is_empty() {
                let tags = task.tags.clone();
                let task_id = task.id;
                self.dialog = DialogMode::RemoveTag {
                    task_id,
                    tags,
                    selected: 0,
                };
            }
        }
    }

    /// Move task to category
    pub fn move_task_to_category(&mut self, task_id: u32, category: Option<String>) {
        if let Some(task) = self.storage.tasks.get_mut(&task_id) {
            task.category = category;
            self.update_categories_tags();
            self.update_filtered_tasks();
            if let Err(e) = self.save_tasks() {
                self.error_message = Some(format!("Failed to save: {}", e));
            }
        }
        self.dialog = DialogMode::None;
    }

    /// Add tag to task
    pub fn add_tag_to_task(&mut self, task_id: u32, tag: String) {
        if let Some(task) = self.storage.tasks.get_mut(&task_id) {
            task.add_tag(&tag);
            self.update_categories_tags();
            self.update_filtered_tasks();
            if let Err(e) = self.save_tasks() {
                self.error_message = Some(format!("Failed to save: {}", e));
            }
        }
        self.dialog = DialogMode::None;
    }

    /// Remove tag from task
    pub fn remove_tag_from_task(&mut self, task_id: u32, tag: String) {
        if let Some(task) = self.storage.tasks.get_mut(&task_id) {
            task.remove_tag(&tag);
            self.update_categories_tags();
            self.update_filtered_tasks();
            if let Err(e) = self.save_tasks() {
                self.error_message = Some(format!("Failed to save: {}", e));
            }
        }
        self.dialog = DialogMode::None;
    }
}

fn parse_date_input(input: &str) -> Option<DateTime<Local>> {
    let input = input.trim().to_lowercase();

    if input.is_empty() || input == "none" {
        return None;
    }

    // Try relative dates
    let today = Local::now().date_naive();
    let date = match input.as_str() {
        "today" => Some(today),
        "tomorrow" => today.succ_opt(),
        _ => None,
    };

    if let Some(d) = date {
        return d.and_hms_opt(0, 0, 0)
            .and_then(|dt| Local.from_local_datetime(&dt).single());
    }

    // Try ISO format
    if let Ok(d) = NaiveDate::parse_from_str(&input, "%Y-%m-%d") {
        return d.and_hms_opt(0, 0, 0)
            .and_then(|dt| Local.from_local_datetime(&dt).single());
    }

    // Try other formats
    let formats = ["%b %d", "%B %d", "%m/%d"];
    for fmt in &formats {
        if let Ok(mut d) = NaiveDate::parse_from_str(&input, fmt) {
            d = d.with_year(today.year()).unwrap_or(d);
            if d < today {
                d = d.with_year(today.year() + 1).unwrap_or(d);
            }
            return d.and_hms_opt(0, 0, 0)
                .and_then(|dt| Local.from_local_datetime(&dt).single());
        }
    }

    None
}

pub fn run_tui(ctx: &mut PluginContext) -> Result<(), String> {
    // Setup terminal
    enable_raw_mode().map_err(|e| format!("Failed to enable raw mode: {}", e))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| format!("Failed to setup terminal: {}", e))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| format!("Failed to create terminal: {}", e))?;

    // Get task filename from context, default to "taiginator.md" (same as main app)
    let task_filename = ctx.extra
        .get("task_filename")
        .map(|s| s.as_str())
        .unwrap_or("taiginator.md");

    // Create app state
    let mut app = App::new(ctx.data_dir.clone(), task_filename);
    if let Err(e) = app.load_tasks() {
        // Continue anyway, just show the error
        app.error_message = Some(format!("Failed to load tasks: {}", e));
    }

    // Main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).ok();
    terminal.show_cursor().ok();

    result
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), String> {
    loop {
        terminal.draw(|f| ui::draw(f, app))
            .map_err(|e| format!("Failed to draw: {}", e))?;

        if let Event::Key(key) = event::read()
            .map_err(|e| format!("Failed to read event: {}", e))?
        {
            // Handle dialog input first
            if app.dialog != DialogMode::None {
                handle_dialog_input(app, key.code, key.modifiers);
                continue;
            }

            // Handle search input
            if app.is_searching {
                match key.code {
                    KeyCode::Esc => app.clear_search(),
                    KeyCode::Enter => app.end_search(),
                    KeyCode::Backspace => {
                        app.search_query.pop();
                        app.update_filtered_tasks();
                    }
                    KeyCode::Char(c) => {
                        app.search_query.push(c);
                        app.update_filtered_tasks();
                    }
                    _ => {}
                }
                continue;
            }

            // Handle sidebar-focused input
            if app.sidebar_focused {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                        app.toggle_sidebar_focus();
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.move_sidebar_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => app.move_sidebar_selection(1),
                    KeyCode::Char(' ') | KeyCode::Enter => app.select_sidebar_item(),
                    KeyCode::Char('h') | KeyCode::Left => app.toggle_sidebar_section(),
                    KeyCode::Char('?') => app.dialog = DialogMode::Help,
                    _ => {}
                }
            } else {
                // Handle normal input (task list focused)
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
                    }
                    KeyCode::Tab | KeyCode::Left | KeyCode::Char('h') => {
                        app.toggle_sidebar_focus();
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::Char('g') => app.selected_index = 0,
                    KeyCode::Char('G') => {
                        let len = app.filtered_tasks.len();
                        if len > 0 {
                            app.selected_index = len - 1;
                        }
                    }
                    KeyCode::Home => app.selected_index = 0,
                    KeyCode::End => {
                        let len = app.filtered_tasks.len();
                        if len > 0 {
                            app.selected_index = len - 1;
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Char('x') => {
                        app.toggle_selected();
                    }
                    KeyCode::Char('a') => {
                        app.dialog = DialogMode::AddTask {
                            name: String::new(),
                            date: String::new(),
                        };
                    }
                    KeyCode::Char('e') => {
                        if let Some(task) = app.selected_task() {
                            app.dialog = DialogMode::EditTask {
                                id: task.id,
                                name: task.title.clone(),
                                date: task.scheduled
                                    .map(|d| d.format("%Y-%m-%d").to_string())
                                    .unwrap_or_default(),
                                field: 0,
                            };
                        }
                    }
                    KeyCode::Char('d') | KeyCode::Delete => {
                        if let Some(id) = app.selected_task_id() {
                            app.dialog = DialogMode::DeleteConfirm { id };
                        }
                    }
                    KeyCode::Char('c') => {
                        if app.completed_count() > 0 {
                            app.dialog = DialogMode::ClearConfirm;
                        }
                    }
                    KeyCode::Char('m') => app.open_move_category_dialog(),
                    KeyCode::Char('t') => app.open_add_tag_dialog(),
                    KeyCode::Char('T') => app.open_remove_tag_dialog(),
                    KeyCode::Char('f') => app.cycle_filter(),
                    KeyCode::Char('s') => app.cycle_sort(),
                    KeyCode::Char('/') => app.start_search(),
                    KeyCode::Char('r') | KeyCode::F(5) => app.refresh(),
                    KeyCode::Char('?') => app.dialog = DialogMode::Help,
                    _ => {}
                }
            }

            // Clear error after any input
            app.error_message = None;
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_dialog_input(app: &mut App, key: KeyCode, _modifiers: KeyModifiers) {
    match &mut app.dialog {
        DialogMode::AddTask { name, date } => {
            match key {
                KeyCode::Esc => app.dialog = DialogMode::None,
                KeyCode::Enter => {
                    let n = name.clone();
                    let d = date.clone();
                    app.add_task(n, d);
                }
                KeyCode::Tab => {
                    // Toggle between name and date fields (simple implementation)
                }
                KeyCode::Backspace => {
                    name.pop();
                }
                KeyCode::Char(c) => {
                    name.push(c);
                }
                _ => {}
            }
        }
        DialogMode::EditTask { id, name, date, field } => {
            match key {
                KeyCode::Esc => app.dialog = DialogMode::None,
                KeyCode::Enter => {
                    let i = *id;
                    let n = name.clone();
                    let d = date.clone();
                    app.update_task(i, n, d);
                }
                KeyCode::Tab => {
                    *field = (*field + 1) % 2;
                }
                KeyCode::Backspace => {
                    if *field == 0 {
                        name.pop();
                    } else {
                        date.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if *field == 0 {
                        name.push(c);
                    } else {
                        date.push(c);
                    }
                }
                _ => {}
            }
        }
        DialogMode::DeleteConfirm { id: _ } => {
            match key {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    app.delete_selected();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.dialog = DialogMode::None;
                }
                _ => {}
            }
        }
        DialogMode::ClearConfirm => {
            match key {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    app.clear_completed();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.dialog = DialogMode::None;
                }
                _ => {}
            }
        }
        DialogMode::Help => {
            match key {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Enter | KeyCode::Char('q') => {
                    app.dialog = DialogMode::None;
                }
                _ => {}
            }
        }
        DialogMode::MoveCategory { task_id, categories, selected } => {
            match key {
                KeyCode::Esc => app.dialog = DialogMode::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if *selected < categories.len() - 1 {
                        *selected += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let task_id = *task_id;
                    let category = if *selected == 0 {
                        None // "Uncategorized"
                    } else {
                        Some(categories[*selected].clone())
                    };
                    app.move_task_to_category(task_id, category);
                }
                _ => {}
            }
        }
        DialogMode::AddTag { task_id, input } => {
            match key {
                KeyCode::Esc => app.dialog = DialogMode::None,
                KeyCode::Enter => {
                    if !input.trim().is_empty() {
                        let task_id = *task_id;
                        let tag = input.trim().to_string();
                        app.add_tag_to_task(task_id, tag);
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if c.is_alphanumeric() || c == '_' => {
                    input.push(c);
                }
                _ => {}
            }
        }
        DialogMode::RemoveTag { task_id, tags, selected } => {
            match key {
                KeyCode::Esc => app.dialog = DialogMode::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if *selected < tags.len() - 1 {
                        *selected += 1;
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    let task_id = *task_id;
                    let tag = tags[*selected].clone();
                    app.remove_tag_from_task(task_id, tag);
                }
                _ => {}
            }
        }
        DialogMode::None => {}
    }
}

//! TUI rendering module

use crate::app::{App, DialogMode, SidebarSection};
use crate::task_storage::Task;
use chrono::Local;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    // Main horizontal layout: sidebar + content
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Length(22),  // Sidebar
            Constraint::Min(40),     // Content
        ])
        .split(f.area());

    draw_sidebar(f, app, main_chunks[0]);

    // Content area: task list + status + controls
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),    // Task list
            Constraint::Length(3), // Status bar
            Constraint::Length(3), // Controls
        ])
        .split(main_chunks[1]);

    draw_task_list(f, app, content_chunks[0]);
    draw_status_bar(f, app, content_chunks[1]);
    draw_controls(f, app, content_chunks[2]);

    // Draw dialogs on top
    match &app.dialog {
        DialogMode::None => {}
        DialogMode::AddTask { name, date: _ } => {
            draw_add_dialog(f, name);
        }
        DialogMode::EditTask { id: _, name, date, field } => {
            draw_edit_dialog(f, name, date, *field);
        }
        DialogMode::DeleteConfirm { id } => {
            if let Some(task) = app.storage.tasks.get(id) {
                draw_confirm_dialog(f, &format!("Delete task #{}?", id), &task.title);
            }
        }
        DialogMode::ClearConfirm => {
            let count = app.completed_count();
            draw_confirm_dialog(
                f,
                "Clear completed tasks?",
                &format!("{} task(s) will be removed", count),
            );
        }
        DialogMode::Help => {
            draw_help_dialog(f);
        }
        DialogMode::MoveCategory { task_id: _, categories, selected } => {
            draw_move_category_dialog(f, categories, *selected);
        }
        DialogMode::AddTag { task_id: _, input } => {
            draw_add_tag_dialog(f, input);
        }
        DialogMode::RemoveTag { task_id: _, tags, selected } => {
            draw_remove_tag_dialog(f, tags, *selected);
        }
    }

    // Draw error message if any
    if let Some(msg) = &app.error_message {
        draw_error_message(f, msg);
    }
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),  // Categories
            Constraint::Percentage(40),  // Tags
        ])
        .split(area);

    // Draw categories section
    let category_border_style = if app.sidebar_focused && app.sidebar_section == SidebarSection::Categories {
        Style::default().fg(Color::Yellow)
    } else if app.sidebar_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut category_items: Vec<ListItem> = vec![
        // "All" option
        {
            let is_selected = app.sidebar_focused
                && app.sidebar_section == SidebarSection::Categories
                && app.sidebar_selection == 0;
            let is_active = app.selected_category.is_none();
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            let total = app.storage.tasks.len();
            ListItem::new(format!("All ({})", total)).style(style)
        },
        // "Uncategorized" option
        {
            let is_selected = app.sidebar_focused
                && app.sidebar_section == SidebarSection::Categories
                && app.sidebar_selection == 1;
            let is_active = app.selected_category == Some(None);
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            let count = app.storage.count_in_category(None);
            ListItem::new(format!("Uncategorized ({})", count)).style(style)
        },
    ];

    // Add specific categories
    for (i, cat) in app.categories.iter().enumerate() {
        let is_selected = app.sidebar_focused
            && app.sidebar_section == SidebarSection::Categories
            && app.sidebar_selection == i + 2;
        let is_active = app.selected_category == Some(Some(cat.clone()));
        let style = if is_selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let count = app.storage.count_in_category(Some(cat));
        category_items.push(ListItem::new(format!("{} ({})", cat, count)).style(style));
    }

    let categories_list = List::new(category_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Categories ")
                .border_style(category_border_style),
        );

    f.render_widget(categories_list, chunks[0]);

    // Draw tags section
    let tags_border_style = if app.sidebar_focused && app.sidebar_section == SidebarSection::Tags {
        Style::default().fg(Color::Yellow)
    } else if app.sidebar_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut tag_items: Vec<ListItem> = vec![
        // "All" option
        {
            let is_selected = app.sidebar_focused
                && app.sidebar_section == SidebarSection::Tags
                && app.sidebar_selection == 0;
            let is_active = app.selected_tag_filter.is_none();
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new("All").style(style)
        },
    ];

    // Add specific tags
    for (i, tag) in app.all_tags.iter().enumerate() {
        let is_selected = app.sidebar_focused
            && app.sidebar_section == SidebarSection::Tags
            && app.sidebar_selection == i + 1;
        let is_active = app.selected_tag_filter.as_ref() == Some(tag);
        let style = if is_selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Magenta)
        };
        let count = app.storage.count_with_tag(tag);
        tag_items.push(ListItem::new(format!("#{} ({})", tag, count)).style(style));
    }

    let tags_list = List::new(tag_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tags ")
                .border_style(tags_border_style),
        );

    f.render_widget(tags_list, chunks[1]);
}

fn draw_task_list(f: &mut Frame, app: &App, area: Rect) {
    let tasks = app.get_visible_tasks();
    let today = Local::now().date_naive();

    let items: Vec<ListItem> = tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let is_selected = i == app.selected_index;
            create_task_item(task, is_selected, today)
        })
        .collect();

    let title = if app.is_searching {
        format!(" Taiga Tasks - Search: {} ", app.search_query)
    } else {
        format!(
            " Taiga Tasks ({} | Sort: {}) ",
            app.filter_mode.as_str(),
            app.sort_mode.as_str()
        )
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_alignment(Alignment::Left)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_widget(list, area);
}

fn create_task_item(task: &Task, is_selected: bool, today: chrono::NaiveDate) -> ListItem<'static> {
    let checkbox = if task.is_complete { "[✓]" } else { "[ ]" };

    let date_info = task.scheduled.map(|dt| {
        let date = dt.date_naive();
        let diff = date.signed_duration_since(today).num_days();

        let date_str = if diff == 0 {
            "Today".to_string()
        } else if diff == 1 {
            "Tomorrow".to_string()
        } else if diff < 0 {
            format!("Overdue ({} days)", -diff)
        } else if diff <= 7 {
            date.format("%a").to_string()
        } else {
            date.format("%b %d").to_string()
        };

        (date_str, diff)
    });

    let mut spans = vec![
        Span::styled(
            format!("{} ", checkbox),
            if task.is_complete {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            },
        ),
        Span::styled(
            format!("[{}] ", task.id),
            Style::default().fg(Color::Cyan),
        ),
    ];

    // Title with styling
    let title_style = if task.is_complete {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::CROSSED_OUT)
    } else if is_selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    spans.push(Span::styled(task.title.clone(), title_style));

    // Tags in magenta
    for tag in &task.tags {
        spans.push(Span::styled(
            format!(" #{}", tag),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Date info
    if let Some((date_str, diff)) = date_info {
        let date_style = if task.is_complete {
            Style::default().fg(Color::Green)
        } else if diff < 0 {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if diff <= 1 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        spans.push(Span::styled(format!(" ({})", date_str), date_style));
    }

    let line = Line::from(spans);

    let style = if is_selected {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    ListItem::new(line).style(style)
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let total = app.task_count();
    let done = app.completed_count();
    let overdue = app.overdue_count();

    let status = vec![
        Span::raw(" "),
        Span::styled(format!("{} total", total), Style::default()),
        Span::raw(" | "),
        Span::styled(format!("{} done", done), Style::default().fg(Color::Green)),
        Span::raw(" | "),
        if overdue > 0 {
            Span::styled(format!("{} overdue", overdue), Style::default().fg(Color::Red))
        } else {
            Span::styled("0 overdue", Style::default().fg(Color::DarkGray))
        },
    ];

    let paragraph = Paragraph::new(Line::from(status))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_controls(f: &mut Frame, app: &App, area: Rect) {
    let controls = if app.is_searching {
        vec![
            Span::styled(" Type to search", Style::default().fg(Color::Yellow)),
            Span::raw(" | "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Confirm "),
            Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Clear "),
        ]
    } else {
        vec![
            Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Nav "),
            Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Toggle "),
            Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Add "),
            Span::styled("e", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Edit "),
            Span::styled("d", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Del "),
            Span::styled("/", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Search "),
            Span::styled("f", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Filter "),
            Span::styled("s", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Sort "),
            Span::styled("?", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Help "),
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(":Quit"),
        ]
    };

    let paragraph = Paragraph::new(Line::from(controls))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Controls ")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

fn draw_add_dialog(f: &mut Frame, name: &str) {
    let area = centered_rect(50, 30, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Add New Task ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let input = Paragraph::new(format!("Name: {}_", name))
        .style(Style::default())
        .wrap(Wrap { trim: false });

    f.render_widget(input, chunks[0]);

    let hint = Paragraph::new("Enter to save, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(hint, chunks[1]);
}

fn draw_edit_dialog(f: &mut Frame, name: &str, date: &str, field: usize) {
    let area = centered_rect(50, 40, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Edit Task ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let name_style = if field == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let date_style = if field == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let name_cursor = if field == 0 { "_" } else { "" };
    let date_cursor = if field == 1 { "_" } else { "" };

    let name_input = Paragraph::new(format!("Name: {}{}", name, name_cursor))
        .style(name_style)
        .wrap(Wrap { trim: false });

    f.render_widget(name_input, chunks[0]);

    let date_input = Paragraph::new(format!("Date: {}{} (Tab to switch)", date, date_cursor))
        .style(date_style)
        .wrap(Wrap { trim: false });

    f.render_widget(date_input, chunks[1]);

    let hint = Paragraph::new("Enter to save, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(hint, chunks[2]);
}

fn draw_confirm_dialog(f: &mut Frame, title: &str, message: &str) {
    let area = centered_rect(40, 25, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(inner);

    let msg = Paragraph::new(message)
        .style(Style::default())
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    f.render_widget(msg, chunks[0]);

    let hint = Paragraph::new("Y to confirm, N/Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(hint, chunks[1]);
}

fn draw_help_dialog(f: &mut Frame) {
    let area = centered_rect(65, 80, f.area());

    f.render_widget(Clear, area);

    let help_text = vec![
        Line::from(vec![Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  ↑/↓ or j/k  Move selection"),
        Line::from("  Tab/h/l     Toggle sidebar/task list focus"),
        Line::from("  g/G         Go to top/bottom"),
        Line::from("  Home/End    Go to top/bottom"),
        Line::from(""),
        Line::from(vec![Span::styled("Task Actions", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  Space/Enter Toggle completion"),
        Line::from("  a           Add new task"),
        Line::from("  e           Edit selected task"),
        Line::from("  d/Delete    Delete selected task"),
        Line::from("  c           Clear completed tasks"),
        Line::from(""),
        Line::from(vec![Span::styled("Categories & Tags", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  m           Move task to category"),
        Line::from("  t           Add tag to task"),
        Line::from("  T           Remove tag from task"),
        Line::from(""),
        Line::from(vec![Span::styled("View Controls", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  f           Cycle filter mode"),
        Line::from("  s           Cycle sort mode"),
        Line::from("  /           Search tasks"),
        Line::from("  r/F5        Refresh from file"),
        Line::from(""),
        Line::from(vec![Span::styled("General", Style::default().add_modifier(Modifier::BOLD))]),
        Line::from("  ?           Toggle this help"),
        Line::from("  q/Esc       Quit"),
        Line::from(""),
        Line::from(vec![Span::styled("Press any key to close", Style::default().fg(Color::DarkGray))]),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_error_message(f: &mut Frame, message: &str) {
    let area = Rect {
        x: 2,
        y: f.area().height - 2,
        width: f.area().width - 4,
        height: 1,
    };

    let msg = Paragraph::new(message)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);

    f.render_widget(msg, area);
}

fn draw_move_category_dialog(f: &mut Frame, categories: &[String], selected: usize) {
    let area = centered_rect(40, 50, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Move to Category ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let style = if i == selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(cat.as_str()).style(style)
        })
        .collect();

    let list = List::new(items);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(inner);

    f.render_widget(list, chunks[0]);

    let hint = Paragraph::new("↑/↓ to select, Enter to confirm, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(hint, chunks[1]);
}

fn draw_add_tag_dialog(f: &mut Frame, input: &str) {
    let area = centered_rect(40, 25, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Add Tag ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(3), Constraint::Length(2)])
        .split(inner);

    let input_text = Paragraph::new(format!("Tag: {}_", input))
        .style(Style::default())
        .wrap(Wrap { trim: false });

    f.render_widget(input_text, chunks[0]);

    let hint = Paragraph::new("Enter to confirm, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(hint, chunks[1]);
}

fn draw_remove_tag_dialog(f: &mut Frame, tags: &[String], selected: usize) {
    let area = centered_rect(40, 40, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Remove Tag ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = tags
        .iter()
        .enumerate()
        .map(|(i, tag)| {
            let style = if i == selected {
                Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Magenta)
            };
            ListItem::new(format!("#{}", tag)).style(style)
        })
        .collect();

    let list = List::new(items);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(inner);

    f.render_widget(list, chunks[0]);

    let hint = Paragraph::new("↑/↓ to select, Enter to remove, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    f.render_widget(hint, chunks[1]);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

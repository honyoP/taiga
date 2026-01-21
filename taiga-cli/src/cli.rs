use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "taiga")]
#[command(about = concat!(
    ">>=====================================<<\n",
    "|| /__  ___/                           ||\n",
    "||   / /   ___     ( )  ___      ___   ||\n",
    "||  / /  //   ) ) / / //   ) ) //   ) )||\n",
    "|| / /  //   / / / / ((___/ / //   / / ||\n",
    "||/ /  ((___( ( / /   //__   ((___( (  ||\n",
    ">>=====================================<<\n",
    "~A task organizer from a mentally deficit monkey~"
))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(long_version = concat!(
    "v",
    env!("CARGO_PKG_VERSION"),
    "\nCodeName: ",
    env!("CODENAME")
))]
#[command(allow_external_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SortBy {
    Id,
    Date,
    Name,
    Status,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Adds a task
    Add {
        #[arg(required = true, num_args = 1..)]
        title: Vec<String>,
        /// Schedule the task on a specific date
        #[arg(long, short = 'd', value_name = "DATE")]
        on: Option<String>,
        /// Schedule the task on a specific date (alias for --on)
        #[arg(long, value_name = "DATE")]
        date: Option<String>,
        /// Category for the task
        #[arg(long, short = 'C', value_name = "CATEGORY")]
        category: Option<String>,
        /// Tags for the task (can be specified multiple times)
        #[arg(long, short = 't', value_name = "TAG")]
        tag: Vec<String>,
    },

    /// Lists tasks with filtering and sorting
    List {
        /// Show only checked/completed tasks
        #[arg(long)]
        checked: bool,
        /// Show only unchecked/incomplete tasks
        #[arg(long)]
        unchecked: bool,
        /// Show only tasks with scheduled dates
        #[arg(long)]
        scheduled: bool,
        /// Show only tasks without scheduled dates
        #[arg(long)]
        unscheduled: bool,
        /// Show only overdue tasks
        #[arg(long)]
        overdue: bool,
        /// Filter tasks containing text (case-insensitive)
        #[arg(long, short = 's', value_name = "TERM")]
        search: Option<String>,
        /// Sort tasks by field
        #[arg(long, value_enum, default_value = "id")]
        sort: SortBy,
        /// Reverse sort order
        #[arg(long, short = 'r')]
        reverse: bool,
        /// Use compact one-line format
        #[arg(long, short = 'c')]
        compact: bool,
        /// Use detailed format with full info
        #[arg(long)]
        detailed: bool,
        /// Disable colors
        #[arg(long)]
        no_color: bool,
        /// Filter by category (use 'none' for uncategorized)
        #[arg(long, short = 'C', value_name = "CATEGORY")]
        category: Option<String>,
        /// Filter by tag (can be specified multiple times, all must match)
        #[arg(long, short = 't', value_name = "TAG")]
        tag: Vec<String>,
    },

    /// Toggles task completion status
    Check {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
    },

    /// Removes a task
    Remove {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
    },

    /// Edit a task's name and/or scheduled date
    Edit {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
        /// New task name
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
        /// New scheduled date (use 'none' to clear)
        #[arg(long, value_name = "DATE")]
        date: Option<String>,
    },

    /// Reschedule a task (change only the date)
    Reschedule {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
        /// New date (use 'none' to clear)
        #[arg(required = true, num_args = 1..)]
        date: Vec<String>,
    },

    /// Rename a task (change only the name)
    Rename {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
        #[arg(required = true, num_args = 1..)]
        name: Vec<String>,
    },

    /// Clear completed tasks
    Clear {
        /// Remove only checked/completed tasks
        #[arg(long)]
        checked: bool,
        /// Skip confirmation prompt
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Recover tasks from backup file
    Recover {
        /// Skip confirmation prompt
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Renumber all tasks sequentially
    Reindex {
        /// Skip confirmation prompt
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// List loaded plugins
    Plugins,

    /// Move a task to a different category
    Move {
        /// Task ID to move
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
        /// Target category (use 'none' for uncategorized)
        category: String,
    },

    /// Add or remove tags from a task
    Tag {
        /// Task ID to modify
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
        /// Tag action (add or remove)
        #[command(subcommand)]
        action: TagAction,
    },

    /// List all categories
    Categories,

    /// List all tags
    Tags,

    /// External command (handled by plugins)
    #[command(external_subcommand)]
    External(Vec<String>),
}

/// Tag subcommands
#[derive(Subcommand)]
pub enum TagAction {
    /// Add a tag to a task
    Add {
        /// Tag to add (without # prefix)
        tag: String,
    },
    /// Remove a tag from a task
    Remove {
        /// Tag to remove (without # prefix)
        tag: String,
    },
}

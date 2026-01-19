use clap::{Parser, Subcommand};

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
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Adds a task
    #[command(subcommand_precedence_over_arg = true)]
    Add {
        #[arg(required = true, num_args = 1..)]
        title: Vec<String>,
        #[command(subcommand)]
        when: Option<WhenCommand>,
    },
    /// Lists tasks
    List {
        #[arg(default_value = "all")]
        state: String,
    },
    /// Checks task completed
    Check {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
    },
    /// Removes a task
    Remove {
        #[arg(value_parser = clap::value_parser!(u32))]
        id: u32,
    },
    /// Pomodoro manager
    Pomo {
        #[command(subcommand)]
        action: PomoCommands,
    },
    #[command(hide = true)]
    Daemon,
}

#[derive(Subcommand)]
pub enum WhenCommand {
    /// Schedules a task
    When {
        #[arg(num_args = 1..)]
        scheduled: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum PomoCommands {
    /// Starts new pomodoro session
    Start {
        /// How long should focus session last (minutes)
        focus: u64,
        /// How long should break session last (minutes)
        break_time: u64,
        /// How many cycles of focus times to repeat
        cycles: u32,
    },
    /// Shows status of running session
    Status,
    /// Stops running pomodoro session
    Stop,
    /// Pauses running pomodoro session
    Pause,
    /// Resumes paused pomodoro session
    Resume,
    /// Kills daemon
    Kill,
}

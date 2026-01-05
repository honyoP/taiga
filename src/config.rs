use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub data_directory: String,
    pub task_filename: String,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        let default_path = dirs::home_dir()
            .map(|mut p| {
                p.push("taiginator");
                p
            })
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            data_directory: default_path.to_string_lossy().to_string(),
            task_filename: "taiginator.md".to_string(),
        }
    }
}

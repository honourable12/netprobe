use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub poll_interval_secs: u64,
    pub signal_threshold: u32,
    pub moving_average_window: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_secs: 15,
            signal_threshold: 70,
            moving_average_window: 3,
        }
    }
}

pub fn load_config() -> Config {
    let path = Path::new("config.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
    }
    
    let default = Config::default();
    let _ = fs::write(path, serde_json::to_string_pretty(&default).unwrap());
    default
}

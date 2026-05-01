use std::fs;
use std::path::PathBuf;

use crate::types::Task;

fn data_path() -> PathBuf {
    let base = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zen");
    fs::create_dir_all(&base).ok();
    base.join("tasks.json")
}

pub fn load() -> Vec<Task> {
    let path = data_path();
    if !path.exists() {
        return Vec::new();
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(tasks: &[Task]) {
    let path = data_path();
    let raw = serde_json::to_string_pretty(tasks).unwrap_or_default();
    fs::write(path, raw).ok();
}

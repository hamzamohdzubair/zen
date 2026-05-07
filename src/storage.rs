use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::Task;

#[derive(Serialize, Deserialize, Default)]
struct AppState {
    tasks: Vec<Task>,
}

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

    if let Ok(state) = serde_json::from_str::<AppState>(&raw) {
        return state.tasks;
    }

    // Legacy: plain task array
    serde_json::from_str::<Vec<Task>>(&raw).unwrap_or_default()
}

pub fn save(tasks: &[Task]) {
    // Keep archive in sync: every task that exists in main view exists in archive.
    crate::archive::sync(tasks);

    let path = data_path();
    let state = AppState { tasks: tasks.to_vec() };
    let raw = serde_json::to_string_pretty(&state).unwrap_or_default();
    fs::write(path, raw).ok();
}

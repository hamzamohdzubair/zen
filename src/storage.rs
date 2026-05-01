use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::Task;

#[derive(Serialize, Deserialize, Default)]
struct AppState {
    tasks: Vec<Task>,
    #[serde(default)]
    projects: Vec<Option<String>>,
}

fn data_path() -> PathBuf {
    let base = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zen");
    fs::create_dir_all(&base).ok();
    base.join("tasks.json")
}

pub fn load() -> (Vec<Task>, [Option<String>; 10]) {
    let path = data_path();
    if !path.exists() {
        return (Vec::new(), Default::default());
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();

    if let Ok(state) = serde_json::from_str::<AppState>(&raw) {
        let mut projects: [Option<String>; 10] = Default::default();
        for (i, p) in state.projects.into_iter().enumerate().take(10) {
            projects[i] = p;
        }
        return (state.tasks, projects);
    }

    // Legacy: plain task array
    let tasks = serde_json::from_str::<Vec<Task>>(&raw).unwrap_or_default();
    (tasks, Default::default())
}

pub fn save(tasks: &[Task], projects: &[Option<String>; 10]) {
    let path = data_path();
    let state = AppState {
        tasks: tasks.to_vec(),
        projects: projects.to_vec(),
    };
    let raw = serde_json::to_string_pretty(&state).unwrap_or_default();
    fs::write(path, raw).ok();
}

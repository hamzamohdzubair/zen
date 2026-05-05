use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::Task;

#[derive(Serialize, Deserialize, Default)]
struct ArchiveStore {
    tasks: Vec<Task>,
}

fn archive_path() -> PathBuf {
    let base = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zen");
    fs::create_dir_all(&base).ok();
    base.join("archive.json")
}

pub fn append_tasks(tasks: &[Task]) {
    if tasks.is_empty() {
        return;
    }
    let path = archive_path();
    let mut store: ArchiveStore = if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    } else {
        ArchiveStore::default()
    };
    store.tasks.extend_from_slice(tasks);
    if let Ok(raw) = serde_json::to_string_pretty(&store) {
        fs::write(path, raw).ok();
    }
}

#[allow(dead_code)]
pub fn load() -> Vec<Task> {
    let path = archive_path();
    if !path.exists() {
        return Vec::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<ArchiveStore>(&raw).ok())
        .map(|s| s.tasks)
        .unwrap_or_default()
}

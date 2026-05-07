use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

fn load_store() -> ArchiveStore {
    let path = archive_path();
    if !path.exists() {
        return ArchiveStore::default();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_store(store: &ArchiveStore) {
    if let Ok(raw) = serde_json::to_string_pretty(store) {
        fs::write(archive_path(), raw).ok();
    }
}

/// Upsert `current_tasks` into the archive: update existing entries by ID,
/// append new ones. Tasks previously in archive but absent from `current_tasks`
/// are kept unchanged (the archive can only grow).
pub fn sync(current_tasks: &[Task]) {
    if current_tasks.is_empty() {
        return;
    }
    let mut store = load_store();
    let index: HashMap<Uuid, usize> = store.tasks.iter()
        .enumerate()
        .map(|(i, t)| (t.id, i))
        .collect();
    for task in current_tasks {
        if let Some(&pos) = index.get(&task.id) {
            store.tasks[pos] = task.clone();
        } else {
            store.tasks.push(task.clone());
        }
    }
    save_store(&store);
}

/// Append `tasks` to the archive without updating existing entries.
/// Used when tasks are removed from the main view (their last known state
/// should already be in the archive from a previous sync).
pub fn append_tasks(tasks: &[Task]) {
    if tasks.is_empty() {
        return;
    }
    let mut store = load_store();
    store.tasks.extend_from_slice(tasks);
    save_store(&store);
}

/// Load all tasks from the archive.
pub fn load() -> Vec<Task> {
    load_store().tasks
}

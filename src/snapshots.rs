use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::Task;

#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub taken_at: DateTime<Utc>,
    pub tasks: Vec<Task>,
    pub projects: [Option<String>; 10],
    pub active_slots: [bool; 10],
    pub show_unc: bool,
    pub collapsed: Vec<Uuid>,
}

pub struct SnapEntry {
    pub label: String,
    pub path: PathBuf,
}

pub struct SnapViewerData {
    pub tasks: Vec<Task>,
    pub projects: [Option<String>; 10],
    pub collapsed: HashSet<Uuid>,
    pub scroll_offset: usize,
    pub label: String,
}

pub struct SnapPopupState {
    pub entries: Vec<SnapEntry>,
    pub cursor: usize,
    pub viewer: Option<SnapViewerData>,
}

pub fn snaps_dir() -> PathBuf {
    let base = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zen")
        .join("snapshots");
    fs::create_dir_all(&base).ok();
    base
}

pub fn save_snapshot(snap: &Snapshot) -> Option<PathBuf> {
    let filename = snap.taken_at.format("%Y-%m-%d_%H-%M-%S").to_string() + ".json";
    let path = snaps_dir().join(filename);
    let raw = serde_json::to_string_pretty(snap).ok()?;
    fs::write(&path, raw).ok()?;
    Some(path)
}

pub fn list_snapshots() -> Vec<(DateTime<Utc>, PathBuf)> {
    let dir = snaps_dir();
    let mut results: Vec<(DateTime<Utc>, PathBuf)> = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return results;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if let Ok(ndt) = NaiveDateTime::parse_from_str(stem, "%Y-%m-%d_%H-%M-%S") {
            let dt = DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc);
            results.push((dt, path));
        }
    }
    results.sort_by_key(|(dt, _)| *dt);
    results
}

pub fn load_snapshot(path: &PathBuf) -> Option<Snapshot> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

impl SnapPopupState {
    pub fn load() -> Self {
        let mut all = list_snapshots();
        all.reverse();
        let entries = all
            .into_iter()
            .take(10)
            .map(|(dt, path)| SnapEntry {
                label: dt.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string(),
                path,
            })
            .collect();
        SnapPopupState { entries, cursor: 0, viewer: None }
    }

    pub fn open_viewer(&mut self) {
        let Some(entry) = self.entries.get(self.cursor) else { return };
        if let Some(snap) = load_snapshot(&entry.path) {
            let collapsed = snap.collapsed.iter().copied().collect();
            self.viewer = Some(SnapViewerData {
                tasks: snap.tasks,
                projects: snap.projects,
                collapsed,
                scroll_offset: 0,
                label: entry.label.clone(),
            });
        }
    }

    pub fn close_viewer(&mut self) {
        self.viewer = None;
    }

    pub fn viewer_scroll_down(&mut self) {
        if let Some(ref mut v) = self.viewer {
            v.scroll_offset += 1;
        }
    }

    pub fn viewer_scroll_up(&mut self) {
        if let Some(ref mut v) = self.viewer {
            v.scroll_offset = v.scroll_offset.saturating_sub(1);
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.entries.len() {
            self.cursor += 1;
        }
    }
}

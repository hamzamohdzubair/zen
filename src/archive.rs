use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{Status, Task};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveTask {
    pub id: Uuid,
    pub title: String,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// None for ancestor-context tasks that are still active.
    pub completed_at: Option<DateTime<Utc>>,
}

type MonthData = BTreeMap<String, Vec<ArchiveTask>>;

fn archive_dir() -> PathBuf {
    let base = dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zen")
        .join("archive");
    fs::create_dir_all(&base).ok();
    base
}

fn month_path(year: i32, month: u32) -> PathBuf {
    let dir = archive_dir().join(format!("{}", year));
    fs::create_dir_all(&dir).ok();
    dir.join(format!("{:02}.json", month))
}

fn load_month(year: i32, month: u32) -> MonthData {
    let path = month_path(year, month);
    if !path.exists() {
        return BTreeMap::new();
    }
    let raw = fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_month(year: i32, month: u32, data: &MonthData) {
    let path = month_path(year, month);
    if let Ok(raw) = serde_json::to_string_pretty(data) {
        fs::write(path, raw).ok();
    }
}

fn task_completed_at(task: &Task) -> DateTime<Utc> {
    task.transitions
        .iter()
        .filter(|t| t.to == Status::Done)
        .last()
        .map(|t| t.at)
        .unwrap_or(task.created_at)
}

/// Archive a hidden subtree.
/// `hidden` = tasks being hidden (all Done, get a completed_at).
/// `ancestors` = still-active parents up to root (no completed_at, context only).
pub fn archive_subtree(hidden: &[Task], ancestors: &[Task]) {
    if hidden.is_empty() {
        return;
    }

    // Build the full set of ArchiveTasks to write.
    let mut archive_tasks: Vec<ArchiveTask> = hidden
        .iter()
        .map(|t| ArchiveTask {
            id: t.id,
            title: t.title.clone(),
            parent_id: t.parent_id,
            created_at: t.created_at,
            completed_at: Some(task_completed_at(t)),
        })
        .collect();

    for t in ancestors {
        archive_tasks.push(ArchiveTask {
            id: t.id,
            title: t.title.clone(),
            parent_id: t.parent_id,
            created_at: t.created_at,
            completed_at: None,
        });
    }

    // Event dates are driven by hidden tasks only.
    let mut event_dates: HashSet<NaiveDate> = HashSet::new();
    for t in hidden {
        event_dates.insert(t.created_at.date_naive());
        event_dates.insert(task_completed_at(t).date_naive());
    }

    let mut by_month: HashMap<(i32, u32), Vec<NaiveDate>> = HashMap::new();
    for date in &event_dates {
        by_month.entry((date.year(), date.month())).or_default().push(*date);
    }

    for ((year, month), dates) in by_month {
        let mut month_data = load_month(year, month);
        for date in dates {
            let key = date.format("%Y-%m-%d").to_string();
            let snapshot = month_data.entry(key).or_default();
            for at in &archive_tasks {
                let alive = at.created_at.date_naive() <= date
                    && at.completed_at.map(|c| date <= c.date_naive()).unwrap_or(true);
                if alive && !snapshot.iter().any(|t| t.id == at.id) {
                    snapshot.push(at.clone());
                }
            }
        }
        save_month(year, month, &month_data);
    }
}

pub fn available_dates_in_month(year: i32, month: u32) -> HashSet<NaiveDate> {
    load_month(year, month)
        .keys()
        .filter_map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .collect()
}

pub fn load_day_snapshot(date: NaiveDate) -> Vec<ArchiveTask> {
    let data = load_month(date.year(), date.month());
    let key = date.format("%Y-%m-%d").to_string();
    data.get(&key).cloned().unwrap_or_default()
}

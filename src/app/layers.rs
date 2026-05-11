use super::*;
use super::{ArchiveBrowserState, ArchiveView};
use chrono::Datelike;
use uuid::Uuid;

impl App {
    /// Auto-surface any Snoozed tasks whose timer has expired.
    pub fn check_snooze_timers(&mut self) {
        let now = chrono::Utc::now().timestamp();
        let expired: Vec<Uuid> = self.tasks.iter()
            .filter_map(|t| {
                if let Layer::Snoozed { expires_at } = t.layer {
                    if now >= expires_at { Some(t.id) } else { None }
                } else {
                    None
                }
            })
            .collect();
        for id in expired {
            self.unsnooze_task(id);
        }
    }

    /// Move `id` and its entire subtree to Snoozed with the given expiry timestamp.
    pub fn snooze_task(&mut self, id: Uuid, expires_at: i64) {
        self.push_undo();
        self.apply_layer_recursive(id, Layer::Snoozed { expires_at });
        self.status_message = Some("Snoozed".into());
    }

    /// Move `id` and its entire subtree back to Active (surface from snooze).
    pub fn unsnooze_task(&mut self, id: Uuid) {
        self.apply_layer_recursive(id, Layer::Active);
    }

    /// Archive `id` and its entire subtree: write to archive files, remove from task list.
    pub fn hide_task(&mut self, id: Uuid) {
        self.push_undo();

        let subtree_ids = self.collect_subtree_ids(id);
        let subtree_tasks: Vec<crate::types::Task> = subtree_ids
            .iter()
            .filter_map(|&sid| self.task_ref(sid).cloned())
            .collect();

        let ancestor_tasks: Vec<crate::types::Task> = self.collect_ancestors(id);

        crate::archive::archive_subtree(&subtree_tasks, &ancestor_tasks);

        // Remove from parent's children list so the parent stays consistent.
        let parent_id = self.task_ref(id).and_then(|t| t.parent_id);
        if let Some(pid) = parent_id {
            if let Some(parent) = self.task_mut(pid) {
                parent.children.retain(|&cid| cid != id);
            }
        }

        self.tasks.retain(|t| !subtree_ids.contains(&t.id));
        self.clamp_all_cursors();
        self.status_message = Some("Hidden".into());
    }

    fn collect_subtree_ids(&self, id: Uuid) -> std::collections::HashSet<Uuid> {
        let mut ids = std::collections::HashSet::new();
        self.collect_subtree_recursive(id, &mut ids);
        ids
    }

    fn collect_subtree_recursive(&self, id: Uuid, ids: &mut std::collections::HashSet<Uuid>) {
        ids.insert(id);
        if let Some(task) = self.task_ref(id) {
            for &cid in &task.children {
                self.collect_subtree_recursive(cid, ids);
            }
        }
    }

    fn collect_ancestors(&self, id: Uuid) -> Vec<crate::types::Task> {
        let mut ancestors = Vec::new();
        let mut current = id;
        while let Some(pid) = self.task_ref(current).and_then(|t| t.parent_id) {
            if let Some(parent) = self.task_ref(pid) {
                ancestors.push(parent.clone());
            }
            current = pid;
        }
        ancestors
    }

    /// True if any direct child of this task is Snoozed.
    pub fn has_snoozed_children(&self, task: &crate::types::Task) -> bool {
        task.children.iter().any(|&cid| {
            self.task_ref(cid)
                .map(|c| matches!(c.layer, Layer::Snoozed { .. }))
                .unwrap_or(false)
        })
    }

    /// True if task is Active and has at least one Snoozed direct child.
    pub fn is_clocked(&self, task: &crate::types::Task) -> bool {
        matches!(task.layer, Layer::Active) && self.has_snoozed_children(task)
    }

    /// Begin early-expire confirmation for a clocked task.
    pub fn begin_expire_snooze(&mut self, id: Uuid) {
        // Find earliest expiry among snoozed children.
        let min_expiry = self.task_ref(id)
            .map(|t| t.children.clone())
            .unwrap_or_default()
            .iter()
            .filter_map(|&cid| {
                self.task_ref(cid).and_then(|c| {
                    if let Layer::Snoozed { expires_at } = c.layer {
                        Some(expires_at)
                    } else {
                        None
                    }
                })
            })
            .min();

        let due_str = if let Some(exp) = min_expiry {
            let secs = (exp - chrono::Utc::now().timestamp()).max(0);
            format_duration(secs)
        } else {
            "soon".into()
        };

        self.pending_confirm = Some(PendingConfirm::ExpireSnooze(id));
        self.status_message = Some(
            format!("Due in {}. Expire snooze early? Enter / Esc", due_str)
        );
    }

    /// Surface all Snoozed direct children of the given task.
    pub fn confirm_expire_snooze(&mut self, id: Uuid) {
        self.push_undo();
        let children: Vec<Uuid> = self.task_ref(id)
            .map(|t| t.children.clone())
            .unwrap_or_default();
        for cid in children {
            if self.task_ref(cid).map(|c| matches!(c.layer, Layer::Snoozed { .. })).unwrap_or(false) {
                self.apply_layer_recursive(cid, Layer::Active);
            }
        }
        self.pending_confirm = None;
        self.status_message = None;
    }

    pub(super) fn apply_layer_recursive(&mut self, id: Uuid, layer: Layer) {
        let children: Vec<Uuid> = self.task_ref(id).map(|t| t.children.clone()).unwrap_or_default();
        if let Some(task) = self.task_mut(id) {
            task.layer = layer.clone();
        }
        for cid in children {
            self.apply_layer_recursive(cid, layer.clone());
        }
    }

    /// Parse a duration string like "2h", "3d", "1w" into a Unix timestamp seconds from now.
    pub fn parse_duration_to_expiry(input: &str) -> Option<i64> {
        let input = input.trim();
        if input.is_empty() { return None; }
        let (num_str, unit) = input.split_at(input.len() - 1);
        let n: i64 = num_str.trim().parse().ok()?;
        if n <= 0 { return None; }
        let secs = match unit {
            "h" => n * 3600,
            "d" => n * 86400,
            "w" => n * 7 * 86400,
            _ => return None,
        };
        Some(chrono::Utc::now().timestamp() + secs)
    }

    /// Begin snooze mode: store the task id and wait for duration input.
    pub fn begin_snooze(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            self.snooze_input = Some(SnoozeInputState { task_id: id, input: String::new() });
            self.mode = Mode::SnoozeInput;
            self.status_message = Some("Snooze duration: e.g. 2h  3d  1w  (Esc to cancel)".into());
        }
    }

    /// Commit the snooze after duration input.
    pub fn commit_snooze(&mut self) {
        if let Some(state) = self.snooze_input.take() {
            if let Some(expires_at) = Self::parse_duration_to_expiry(&state.input) {
                self.snooze_task(state.task_id, expires_at);
            } else {
                self.status_message = Some("Invalid duration — use e.g. 2h, 3d, 1w".into());
            }
        }
        self.mode = Mode::Normal;
    }

    // ── Archive browser ──────────────────────────────────────────────────────

    pub fn open_archive_browser(&mut self) {
        use chrono::{Datelike, Utc};
        let today = Utc::now().date_naive();
        let year = today.year();
        let month = today.month();
        let available_dates = crate::archive::available_dates_in_month(year, month);
        self.archive_browser = Some(ArchiveBrowserState {
            view: ArchiveView::Calendar,
            year,
            month,
            selected_day: today.day(),
            available_dates,
            day_tasks: vec![],
            day_scroll: 0,
        });
        self.mode = Mode::ArchiveBrowser;
    }

    pub fn close_archive_browser(&mut self) {
        self.archive_browser = None;
        self.mode = Mode::Normal;
    }

    pub fn archive_prev_day(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            if ab.selected_day > 1 {
                ab.selected_day -= 1;
            } else {
                if ab.month == 1 { ab.year -= 1; ab.month = 12; } else { ab.month -= 1; }
                ab.available_dates = crate::archive::available_dates_in_month(ab.year, ab.month);
                ab.selected_day = days_in_month(ab.year, ab.month);
            }
        }
    }

    pub fn archive_next_day(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            let max = days_in_month(ab.year, ab.month);
            if ab.selected_day < max {
                ab.selected_day += 1;
            } else {
                if ab.month == 12 { ab.year += 1; ab.month = 1; } else { ab.month += 1; }
                ab.available_dates = crate::archive::available_dates_in_month(ab.year, ab.month);
                ab.selected_day = 1;
            }
        }
    }

    pub fn archive_prev_week(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            if ab.selected_day > 7 {
                ab.selected_day -= 7;
            } else {
                let carry = ab.selected_day;
                if ab.month == 1 { ab.year -= 1; ab.month = 12; } else { ab.month -= 1; }
                ab.available_dates = crate::archive::available_dates_in_month(ab.year, ab.month);
                let max = days_in_month(ab.year, ab.month);
                ab.selected_day = max.saturating_sub(7u32.saturating_sub(carry));
            }
        }
    }

    pub fn archive_next_week(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            let max = days_in_month(ab.year, ab.month);
            if ab.selected_day + 7 <= max {
                ab.selected_day += 7;
            } else {
                let overflow = ab.selected_day + 7 - max;
                if ab.month == 12 { ab.year += 1; ab.month = 1; } else { ab.month += 1; }
                ab.available_dates = crate::archive::available_dates_in_month(ab.year, ab.month);
                let new_max = days_in_month(ab.year, ab.month);
                ab.selected_day = overflow.min(new_max);
            }
        }
    }

    pub fn archive_prev_month(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            if ab.month == 1 { ab.year -= 1; ab.month = 12; } else { ab.month -= 1; }
            ab.available_dates = crate::archive::available_dates_in_month(ab.year, ab.month);
            ab.selected_day = ab.selected_day.min(days_in_month(ab.year, ab.month));
        }
    }

    pub fn archive_next_month(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            if ab.month == 12 { ab.year += 1; ab.month = 1; } else { ab.month += 1; }
            ab.available_dates = crate::archive::available_dates_in_month(ab.year, ab.month);
            ab.selected_day = ab.selected_day.min(days_in_month(ab.year, ab.month));
        }
    }

    pub fn archive_open_day(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            if let Some(date) = chrono::NaiveDate::from_ymd_opt(ab.year, ab.month, ab.selected_day) {
                if ab.available_dates.contains(&date) {
                    ab.day_tasks = crate::archive::load_day_snapshot(date);
                    ab.day_scroll = 0;
                    ab.view = ArchiveView::Day;
                }
            }
        }
    }

    pub fn archive_back_to_calendar(&mut self) {
        if let Some(ref mut ab) = self.archive_browser {
            ab.view = ArchiveView::Calendar;
            ab.day_tasks = vec![];
            ab.day_scroll = 0;
        }
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap().pred_opt().unwrap().day()
}

fn format_duration(secs: i64) -> String {
    if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

use super::*;
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

    /// Permanently hide `id` and its subtree from the main view.
    /// Tasks remain in tasks.json with layer=Hidden.
    pub fn archive_task(&mut self, id: Uuid) {
        self.push_undo();
        self.apply_layer_recursive(id, Layer::Hidden);
        self.clamp_all_cursors();
        self.status_message = Some("Archived".into());
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
        let tasks = self.tasks.clone();
        self.archive_browser = Some(ArchiveBrowserState { tasks, scroll_offset: 0 });
        self.mode = Mode::ArchiveBrowser;
    }

    pub fn close_archive_browser(&mut self) {
        self.archive_browser = None;
        self.mode = Mode::Normal;
    }
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

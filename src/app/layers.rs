use super::*;
use uuid::Uuid;

impl App {
    /// Check all Background tasks and surface any whose timer has expired.
    pub fn check_background_timers(&mut self) {
        let now = chrono::Utc::now().timestamp();
        let expired: Vec<Uuid> = self.tasks.iter()
            .filter_map(|t| {
                if let Layer::Background { expires_at } = t.layer {
                    if now >= expires_at { Some(t.id) } else { None }
                } else {
                    None
                }
            })
            .collect();
        for id in expired {
            self.surface_task(id);
        }
    }

    /// Move `id` and its entire subtree to Background with the given expiry timestamp.
    pub fn submerge_task(&mut self, id: Uuid, expires_at: i64) {
        self.push_undo();
        self.apply_layer_recursive(id, Layer::Background { expires_at });
        self.status_message = Some("Submerged to background".into());
    }

    /// Move `id` and its entire subtree to Archive.
    pub fn bury_task(&mut self, id: Uuid) {
        self.push_undo();
        self.apply_layer_recursive(id, Layer::Archive);
        self.status_message = Some("Archived".into());
    }

    /// Move `id` and its entire subtree to Foreground (surface from background).
    pub fn surface_task(&mut self, id: Uuid) {
        self.apply_layer_recursive(id, Layer::Foreground);
    }

    /// Move `id` and its entire subtree to Foreground (restore from archive).
    pub fn restore_task(&mut self, id: Uuid) {
        self.push_undo();
        self.apply_layer_recursive(id, Layer::Foreground);
        self.status_message = Some("Restored to foreground".into());
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

    /// Parse a duration string like "2h", "3d", "1w" into seconds from now.
    /// Returns None if the string is invalid.
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

    /// Begin submerge mode: store the task id and wait for duration input.
    pub fn begin_submerge(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            self.submerge_input = Some(SubmergeInputState { task_id: id, input: String::new() });
            self.mode = Mode::SubmergeInput;
            self.status_message = Some("Submerge duration: e.g. 2h  3d  1w  (Esc to cancel)".into());
        }
    }

    /// Commit the submerge after duration input.
    pub fn commit_submerge(&mut self) {
        if let Some(state) = self.submerge_input.take() {
            if let Some(expires_at) = Self::parse_duration_to_expiry(&state.input) {
                self.submerge_task(state.task_id, expires_at);
            } else {
                self.status_message = Some("Invalid duration — use e.g. 2h, 3d, 1w".into());
            }
        }
        self.mode = Mode::Normal;
    }

    /// Switch to the given layer and reset peek state.
    pub fn set_active_layer(&mut self, layer: ActiveLayer) {
        self.active_layer = layer;
        self.peek_state = PeekState::Hidden;
        self.peek_held = false;
        self.cursor = [0, 0, 0];
        self.focused_col = Column::Todo;
        self.tui_scroll_offset = 0;
        self.clamp_all_cursors();
    }

    /// Count tasks in the given layer.
    pub fn count_in_layer(&self, layer: ActiveLayer) -> usize {
        self.tasks.iter().filter(|t| match layer {
            ActiveLayer::Foreground => matches!(t.layer, Layer::Foreground),
            ActiveLayer::Background => matches!(t.layer, Layer::Background { .. }),
            ActiveLayer::Archive => matches!(t.layer, Layer::Archive),
        }).count()
    }
}

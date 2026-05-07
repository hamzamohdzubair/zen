use super::*;
use uuid::Uuid;

impl App {
    pub fn enter_visual(&mut self) {
        self.visual_anchor_id = self.selected_task_id(self.focused_col);
        self.mode = Mode::Visual;
    }

    pub fn exit_visual(&mut self) {
        self.visual_anchor_id = None;
        self.mode = Mode::Normal;
    }

    /// Set `status` on every task in `ids`, propagating upward for each.
    pub fn visual_apply_status(&mut self, ids: &[Uuid], status: Status) {
        for &id in ids {
            if let Some(task) = self.task_mut(id) {
                task.transition_to(status.clone());
            }
            self.propagate_status_up(id);
        }
        let new_col = match status {
            Status::Todo => Column::Todo,
            Status::Doing => Column::Doing,
            Status::Done => Column::Done,
        };
        self.focused_col = new_col;
        self.clamp_all_cursors();
    }

    /// Move the selected block one position up (past the preceding sibling).
    /// All `ids` must share the same parent and be contiguous in the children list.
    pub fn visual_shift_up(&mut self, ids: &[Uuid]) {
        if ids.is_empty() { return; }
        let cursor_id = self.selected_task_id(self.focused_col);
        let common_parent = self.task_ref(ids[0]).and_then(|t| t.parent_id);
        if ids.iter().any(|&id| self.task_ref(id).and_then(|t| t.parent_id) != common_parent) {
            return;
        }
        match common_parent {
            Some(pid) => {
                let children = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
                let mut positions: Vec<usize> = ids.iter()
                    .filter_map(|&id| children.iter().position(|&c| c == id))
                    .collect();
                if positions.len() != ids.len() { return; }
                positions.sort_unstable();
                for w in positions.windows(2) { if w[1] != w[0] + 1 { return; } }
                let first = positions[0];
                if first == 0 { return; }
                let last = *positions.last().unwrap();
                if let Some(parent) = self.task_mut(pid) {
                    // Remove the sibling just above the block and re-insert it after the block.
                    let moving = parent.children.remove(first - 1);
                    parent.children.insert(last, moving);
                }
            }
            None => {
                let root_ids: Vec<Uuid> = self.tasks.iter()
                    .filter(|t| t.parent_id.is_none()).map(|t| t.id).collect();
                let mut positions: Vec<usize> = ids.iter()
                    .filter_map(|&id| root_ids.iter().position(|&r| r == id))
                    .collect();
                if positions.len() != ids.len() { return; }
                positions.sort_unstable();
                for w in positions.windows(2) { if w[1] != w[0] + 1 { return; } }
                let first = positions[0];
                if first == 0 { return; }
                let last = *positions.last().unwrap();
                let prev_root = root_ids[first - 1];
                let last_root = root_ids[last];
                let prev_idx = self.tasks.iter().position(|t| t.id == prev_root).unwrap();
                let last_idx = self.tasks.iter().position(|t| t.id == last_root).unwrap();
                // Remove prev_root and insert it after last_root.
                let task = self.tasks.remove(prev_idx);
                self.tasks.insert(last_idx, task);
            }
        }
        let col = self.focused_col;
        if let Some(cid) = cursor_id {
            let visible = self.visible_tasks_for(col);
            if let Some(pos) = visible.iter().position(|t| t.id == cid) {
                self.cursor[Self::col_index(col)] = pos;
            }
        }
    }

    /// Move the selected block one position down (past the following sibling).
    /// All `ids` must share the same parent and be contiguous in the children list.
    pub fn visual_shift_down(&mut self, ids: &[Uuid]) {
        if ids.is_empty() { return; }
        let cursor_id = self.selected_task_id(self.focused_col);
        let common_parent = self.task_ref(ids[0]).and_then(|t| t.parent_id);
        if ids.iter().any(|&id| self.task_ref(id).and_then(|t| t.parent_id) != common_parent) {
            return;
        }
        match common_parent {
            Some(pid) => {
                let children = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
                let mut positions: Vec<usize> = ids.iter()
                    .filter_map(|&id| children.iter().position(|&c| c == id))
                    .collect();
                if positions.len() != ids.len() { return; }
                positions.sort_unstable();
                for w in positions.windows(2) { if w[1] != w[0] + 1 { return; } }
                let last = *positions.last().unwrap();
                if last + 1 >= children.len() { return; }
                let first = positions[0];
                if let Some(parent) = self.task_mut(pid) {
                    // Remove the sibling just below the block and re-insert it before the block.
                    let moving = parent.children.remove(last + 1);
                    parent.children.insert(first, moving);
                }
            }
            None => {
                let root_ids: Vec<Uuid> = self.tasks.iter()
                    .filter(|t| t.parent_id.is_none()).map(|t| t.id).collect();
                let mut positions: Vec<usize> = ids.iter()
                    .filter_map(|&id| root_ids.iter().position(|&r| r == id))
                    .collect();
                if positions.len() != ids.len() { return; }
                positions.sort_unstable();
                for w in positions.windows(2) { if w[1] != w[0] + 1 { return; } }
                let last = *positions.last().unwrap();
                if last + 1 >= root_ids.len() { return; }
                let first = positions[0];
                let next_root = root_ids[last + 1];
                let first_root = root_ids[first];
                let next_idx = self.tasks.iter().position(|t| t.id == next_root).unwrap();
                let first_idx = self.tasks.iter().position(|t| t.id == first_root).unwrap();
                // Remove next_root and insert it before first_root.
                let task = self.tasks.remove(next_idx);
                self.tasks.insert(first_idx, task);
            }
        }
        let col = self.focused_col;
        if let Some(cid) = cursor_id {
            let visible = self.visible_tasks_for(col);
            if let Some(pos) = visible.iter().position(|t| t.id == cid) {
                self.cursor[Self::col_index(col)] = pos;
            }
        }
    }

    /// Delete all tasks in `ids` (skipping any already removed), then clamp cursors.
    pub fn visual_delete(&mut self, ids: Vec<Uuid>) {
        let count = ids.len();
        for id in ids {
            if self.task_ref(id).is_some() {
                self.delete_task(id);
            }
        }
        self.clamp_all_cursors();
        self.status_message = Some(format!("Deleted {} tasks", count));
    }
}

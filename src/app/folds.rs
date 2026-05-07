use super::*;
use uuid::Uuid;

impl App {
    pub fn fold_selected(&mut self) {
        if let Some(id) = self.selected_task_id(self.focused_col) {
            self.collapsed.insert(id);
        }
    }

    pub fn toggle_fold_selected(&mut self) {
        if let Some(id) = self.selected_task_id(self.focused_col) {
            if self.collapsed.contains(&id) {
                self.collapsed.remove(&id);
            } else {
                self.collapsed.insert(id);
            }
        }
    }

    /// Collapse every task that has visible children.
    pub fn fold_all(&mut self) {
        let ids: Vec<Uuid> = self.tasks.iter()
            .filter(|t| self.task_visible(t) && !t.children.is_empty())
            .map(|t| t.id)
            .collect();
        for id in ids {
            self.collapsed.insert(id);
        }
    }

    /// Expand every task.
    pub fn unfold_all(&mut self) {
        self.collapsed.clear();
    }

    /// Fold all, then unfold only the path to the first leaf of the first global root.
    pub fn fold_focus_global(&mut self) {
        let roots = self.visible_roots();
        if let Some(&first_root) = roots.first() {
            self.fold_all();
            let leaf = self.first_leaf_of(first_root);
            self.unfold_path_to(leaf);
            let rows = self.build_visible_rows();
            let pos = rows.iter().position(|&id| id == leaf).unwrap_or(0);
            self.tui_scroll_offset = pos.saturating_sub(2);
            self.navigate_to_id(leaf);
        }
    }

    /// Fold all, then unfold only the path to the first leaf of the current root.
    pub fn fold_focus_local(&mut self) {
        let current_root = self.selected_task_id(self.focused_col)
            .map(|id| self.root_task_id(id));
        if let Some(root) = current_root {
            self.fold_all();
            let leaf = self.first_leaf_of(root);
            self.unfold_path_to(leaf);
            let rows = self.build_visible_rows();
            let pos = rows.iter().position(|&id| id == leaf).unwrap_or(0);
            self.tui_scroll_offset = pos.saturating_sub(2);
            self.navigate_to_id(leaf);
        }
    }

    /// Collapse the current root, then apply zl-logic on the next root (wraps).
    pub fn cycle_leaf_next(&mut self) {
        let roots = self.visible_roots();
        if roots.is_empty() { return; }
        let current_root = self.selected_task_id(self.focused_col)
            .map(|id| self.root_task_id(id));
        let next_root = if let Some(root) = current_root {
            self.collapsed.insert(root);
            let pos = roots.iter().position(|&id| id == root).unwrap_or(0);
            roots[(pos + 1) % roots.len()]
        } else {
            roots[0]
        };
        let leaf = self.first_leaf_of(next_root);
        self.unfold_path_to(leaf);
        let rows = self.build_visible_rows();
        let pos = rows.iter().position(|&id| id == leaf).unwrap_or(0);
        self.tui_scroll_offset = pos.saturating_sub(2);
        self.navigate_to_id(leaf);
    }

    /// Collapse the current root, then apply zl-logic on the previous root (wraps).
    pub fn cycle_leaf_prev(&mut self) {
        let roots = self.visible_roots();
        if roots.is_empty() { return; }
        let current_root = self.selected_task_id(self.focused_col)
            .map(|id| self.root_task_id(id));
        let prev_root = if let Some(root) = current_root {
            self.collapsed.insert(root);
            let pos = roots.iter().position(|&id| id == root).unwrap_or(0);
            roots[(pos + roots.len() - 1) % roots.len()]
        } else {
            roots[roots.len() - 1]
        };
        let leaf = self.first_leaf_of(prev_root);
        self.unfold_path_to(leaf);
        let rows = self.build_visible_rows();
        let pos = rows.iter().position(|&id| id == leaf).unwrap_or(0);
        self.tui_scroll_offset = pos.saturating_sub(2);
        self.navigate_to_id(leaf);
    }

    /// Returns the ordered list of visible (non-collapsed) task IDs in DFS order.
    pub(super) fn build_visible_rows(&self) -> Vec<Uuid> {
        let roots: Vec<Uuid> = self.tasks.iter()
            .filter(|t| self.task_visible(t) && t.parent_id.is_none())
            .map(|t| t.id)
            .collect();
        let mut result = Vec::new();
        for root in roots {
            self.collect_visible_dfs(root, &mut result);
        }
        result
    }

    pub(super) fn collect_visible_dfs(&self, id: Uuid, out: &mut Vec<Uuid>) {
        out.push(id);
        if self.collapsed.contains(&id) { return; }
        if let Some(task) = self.task_ref(id) {
            for &child_id in &task.children {
                if self.task_ref(child_id).map(|t| self.task_visible(t)).unwrap_or(false) {
                    self.collect_visible_dfs(child_id, out);
                }
            }
        }
    }

    fn visible_roots(&self) -> Vec<Uuid> {
        self.tasks.iter()
            .filter(|t| self.task_visible(t) && t.parent_id.is_none() && t.status != crate::types::Status::Done)
            .map(|t| t.id)
            .collect()
    }

    /// Follow first-child chain (skipping Done tasks) until hitting a task with no undone visible children.
    fn first_leaf_of(&self, id: Uuid) -> Uuid {
        let mut current = id;
        loop {
            let first_undone_child = self.task_ref(current)
                .and_then(|t| t.children.iter().copied()
                    .find(|&cid| self.task_ref(cid)
                        .map(|c| self.task_visible(c) && c.status != crate::types::Status::Done)
                        .unwrap_or(false)));
            match first_undone_child {
                Some(child) => current = child,
                None => return current,
            }
        }
    }

    /// Remove every ancestor (and the task itself) from the collapsed set.
    fn unfold_path_to(&mut self, id: Uuid) {
        let mut path = vec![id];
        let mut current = id;
        while let Some(pid) = self.task_ref(current).and_then(|t| t.parent_id) {
            path.push(pid);
            current = pid;
        }
        for ancestor in path {
            self.collapsed.remove(&ancestor);
        }
    }
}

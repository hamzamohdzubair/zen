use super::*;
use uuid::Uuid;

impl App {
    /// Move selected task one visual row up in the DFS tree, reparenting if needed.
    pub fn tree_swap_up(&mut self) {
        self.push_undo();
        let Some(task_id) = self.selected_task_id(self.focus) else { return; };
        let task_parent = self.task_ref(task_id).and_then(|t| t.parent_id);

        match task_parent {
            Some(pid) => {
                let children = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
                let pos = match children.iter().position(|&c| c == task_id) {
                    Some(p) => p,
                    None => return,
                };
                if pos == 0 { return; }
                if let Some(parent) = self.task_mut(pid) {
                    parent.children.swap(pos, pos - 1);
                }
            }
            None => {
                // Root-level: find previous root sibling in self.tasks order
                let root_ids: Vec<Uuid> = self.tasks.iter()
                    .filter(|t| t.parent_id.is_none())
                    .map(|t| t.id)
                    .collect();
                let pos = match root_ids.iter().position(|&id| id == task_id) {
                    Some(p) => p,
                    None => return,
                };
                if pos == 0 { return; }
                let prev_id = root_ids[pos - 1];
                let cur_idx = self.tasks.iter().position(|t| t.id == task_id).unwrap();
                let prev_idx = self.tasks.iter().position(|t| t.id == prev_id).unwrap();
                self.tasks.swap(cur_idx, prev_idx);
            }
        }

        let col = self.focus;
        let visible = self.visible_tasks_for(col);
        if let Some(pos) = visible.iter().position(|t| t.id == task_id) {
            self.cursor[Self::status_index(col)] = pos;
        }
    }

    /// Move selected task one visual row down in the DFS tree (past its full subtree), reparenting if needed.
    pub fn tree_swap_down(&mut self) {
        self.push_undo();
        let dfs = self.dfs_visible_ids();
        let Some(task_id) = self.selected_task_id(self.focus) else { return; };
        let i = match dfs.iter().position(|&id| id == task_id) {
            Some(p) => p,
            None => return,
        };

        // Find first DFS row after task's full subtree
        let mut subtree_end = i + 1;
        while subtree_end < dfs.len() && self.is_descendant_of(dfs[subtree_end], task_id) {
            subtree_end += 1;
        }
        if subtree_end >= dfs.len() { return; }

        let next_id = dfs[subtree_end];
        let task_parent = self.task_ref(task_id).and_then(|t| t.parent_id);
        let next_parent = self.task_ref(next_id).and_then(|t| t.parent_id);

        if task_parent != next_parent {
            return;
        }
        // Same parent: simple sibling swap
        if let Some(pid) = task_parent {
            if let Some(parent) = self.task_mut(pid) {
                let pos = parent.children.iter().position(|&c| c == task_id).unwrap();
                parent.children.swap(pos, pos + 1);
            }
        } else {
            let cur_idx = self.tasks.iter().position(|t| t.id == task_id).unwrap();
            let next_idx = self.tasks.iter().position(|t| t.id == next_id).unwrap();
            self.tasks.swap(cur_idx, next_idx);
        }

        let col = self.focus;
        let visible = self.visible_tasks_for(col);
        if let Some(pos) = visible.iter().position(|t| t.id == task_id) {
            self.cursor[Self::status_index(col)] = pos;
        }
    }

    pub(super) fn task_depth(&self, id: Uuid) -> usize {
        let mut depth = 0;
        let mut current = id;
        while let Some(pid) = self.task_ref(current).and_then(|t| t.parent_id) {
            depth += 1;
            current = pid;
        }
        depth
    }

    pub(super) fn ancestor_at_depth(&self, id: Uuid, target_depth: usize) -> Option<Uuid> {
        let mut current = id;
        let mut depth = self.task_depth(id);
        while depth > target_depth {
            current = self.task_ref(current).and_then(|t| t.parent_id)?;
            depth -= 1;
        }
        (depth == target_depth).then_some(current)
    }

    pub fn make_child(&mut self) {
        self.push_undo();
        let child_id = match self.selected_task_id(self.focus) {
            Some(id) => id,
            None => return,
        };
        let rows = self.build_visible_rows();
        let cur_pos = match rows.iter().position(|&id| id == child_id) {
            Some(p) if p > 0 => p,
            _ => return,
        };
        let above_id = rows[cur_pos - 1];

        let child_depth = self.task_depth(child_id);
        let above_depth = self.task_depth(above_id);

        if above_depth < child_depth {
            return;
        }
        let Some(new_parent_id) = self.ancestor_at_depth(above_id, child_depth) else {
            return;
        };

        let old_parent_id = self.task_ref(child_id).and_then(|t| t.parent_id);
        if old_parent_id != Some(new_parent_id) {
            if let Some(old_pid) = old_parent_id {
                if let Some(old_parent) = self.task_mut(old_pid) {
                    old_parent.children.retain(|&c| c != child_id);
                }
            }
            if let Some(child) = self.task_mut(child_id) {
                child.parent_id = Some(new_parent_id);
            }
            if let Some(parent) = self.task_mut(new_parent_id) {
                if !parent.children.contains(&child_id) {
                    parent.children.push(child_id);
                }
            }
        }
        self.status_message = Some("Made child of task above".into());
    }

    pub fn make_root(&mut self) {
        self.push_undo();
        let col = self.focus;
        if let Some(id) = self.selected_task_id(col) {
            let old_parent_id = match self.task_ref(id).and_then(|t| t.parent_id) {
                Some(pid) => pid,
                None => return, // already root
            };
            let grandparent_id = self.task_ref(old_parent_id).and_then(|t| t.parent_id);

            if let Some(old_parent) = self.task_mut(old_parent_id) {
                old_parent.children.retain(|&c| c != id);
            }
            if let Some(task) = self.task_mut(id) {
                task.parent_id = grandparent_id;
            }
            if let Some(gpid) = grandparent_id {
                if let Some(gp) = self.task_mut(gpid) {
                    if !gp.children.contains(&id) {
                        gp.children.push(id);
                    }
                }
            }
            let msg = if grandparent_id.is_some() { "Promoted one level up" } else { "Promoted to root task" };
            self.status_message = Some(msg.into());
        }
    }
}

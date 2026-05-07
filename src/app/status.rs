use super::*;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl App {
    /// Propagate derived status upward from `child_id`'s parent all the way to the root.
    /// Each ancestor adopts the most-urgent status of its direct children:
    /// Todo if any child is Todo, else Doing if any child is Doing, else Done.
    pub(super) fn propagate_status_up(&mut self, child_id: Uuid) {
        let mut current = self.task_ref(child_id).and_then(|t| t.parent_id);
        while let Some(pid) = current {
            let children: Vec<Uuid> = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
            let derived = if children.iter().any(|&cid| self.task_ref(cid).map(|c| c.status == Status::Todo).unwrap_or(false)) {
                Status::Todo
            } else if children.iter().any(|&cid| self.task_ref(cid).map(|c| c.status == Status::Doing).unwrap_or(false)) {
                Status::Doing
            } else {
                Status::Done
            };
            if self.task_ref(pid).map(|t| t.status != derived).unwrap_or(false) {
                if let Some(parent) = self.task_mut(pid) {
                    parent.transition_to(derived);
                }
                current = self.task_ref(pid).and_then(|t| t.parent_id);
            } else {
                break;
            }
        }
    }

    /// Toggle the selected task's status between Doing and Todo (tree view).
    pub fn tree_toggle_doing(&mut self) {
        let Some(id) = self.selected_task_id(self.focused_col) else { return; };
        let current = self.task_ref(id).map(|t| t.status.clone()).unwrap_or(Status::Todo);
        let new_status = if current == Status::Doing { Status::Todo } else { Status::Doing };
        self.tree_set_status(id, new_status);
    }

    /// Toggle the selected task's status between Done and Todo (tree view).
    /// When marking Done, all descendants are also marked Done.
    pub fn tree_toggle_done(&mut self) {
        let Some(id) = self.selected_task_id(self.focused_col) else { return; };
        let current = self.task_ref(id).map(|t| t.status.clone()).unwrap_or(Status::Todo);
        let new_status = if current == Status::Done { Status::Todo } else { Status::Done };
        if new_status == Status::Done {
            self.cascade_status_down(id, Status::Done);
        }
        self.tree_set_status(id, new_status);
    }

    pub(super) fn cascade_status_down(&mut self, id: Uuid, status: Status) {
        let children: Vec<Uuid> = self.task_ref(id).map(|t| t.children.clone()).unwrap_or_default();
        for cid in children {
            if let Some(task) = self.task_mut(cid) {
                task.transition_to(status);
            }
            self.cascade_status_down(cid, status);
        }
    }

    pub(super) fn tree_set_status(&mut self, id: Uuid, new_status: Status) {
        let old_col = self.focused_col;
        let new_col = match new_status {
            Status::Todo => Column::Todo,
            Status::Doing => Column::Doing,
            Status::Done => Column::Done,
        };
        if let Some(task) = self.task_mut(id) {
            task.transition_to(new_status);
        }
        self.propagate_status_up(id);
        self.clamp_cursor(old_col);
        if let Some(pos) = self.visible_tasks_for(new_col).iter().position(|t| t.id == id) {
            self.cursor[Self::col_index(new_col)] = pos;
        }
        self.focused_col = new_col;
    }

    /// Returns all visible task IDs in DFS preorder (same order as the tree display).
    pub(super) fn dfs_visible_ids(&self) -> Vec<Uuid> {
        let visible_ids: HashSet<Uuid> = self.tasks.iter()
            .filter(|t| self.task_visible(t))
            .map(|t| t.id)
            .collect();

        let tasks_by_id: HashMap<Uuid, &Task> = self.tasks.iter()
            .map(|t| (t.id, t))
            .collect();

        fn visit(id: Uuid, tasks_by_id: &HashMap<Uuid, &Task>, visible_ids: &HashSet<Uuid>, result: &mut Vec<Uuid>) {
            result.push(id);
            if let Some(task) = tasks_by_id.get(&id) {
                for &cid in &task.children {
                    if visible_ids.contains(&cid) {
                        visit(cid, tasks_by_id, visible_ids, result);
                    }
                }
            }
        }

        let mut result = Vec::new();
        for task in &self.tasks {
            if !visible_ids.contains(&task.id) { continue; }
            if task.parent_id.map(|pid| !visible_ids.contains(&pid)).unwrap_or(true) {
                visit(task.id, &tasks_by_id, &visible_ids, &mut result);
            }
        }
        result
    }

    pub fn first_visible_leaf_id(&self) -> Option<Uuid> {
        self.dfs_visible_ids()
            .into_iter()
            .find(|&id| self.task_ref(id).map(|t| self.is_leaf_task(t)).unwrap_or(false))
    }

    pub(super) fn is_descendant_of(&self, id: Uuid, ancestor_id: Uuid) -> bool {
        let mut current = id;
        loop {
            match self.task_ref(current).and_then(|t| t.parent_id) {
                None => return false,
                Some(pid) if pid == ancestor_id => return true,
                Some(pid) => current = pid,
            }
        }
    }
}

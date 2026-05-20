use super::*;
use std::time::Instant;
use uuid::Uuid;

impl App {
    pub fn push_undo(&mut self) {
        self.undo_stack.push(self.tasks.clone());
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack.push(self.tasks.clone());
            self.tasks = prev;
            self.clamp_all_cursors();
            self.status_message = Some("Undo".into());
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.tasks.clone());
            self.tasks = next;
            self.clamp_all_cursors();
            self.status_message = Some("Redo".into());
        }
    }

    /// Delete a task by ID and all its descendants, removing it from the tree.
    pub fn delete_task(&mut self, id: Uuid) {
        // Record DFS position before deletion so we can navigate to the nearest task after.
        let dfs_before = self.build_visible_rows();
        let del_pos = dfs_before.iter().position(|&r| r == id).unwrap_or(0);

        let parent_id = self.task_ref(id).and_then(|t| t.parent_id);
        if let Some(pid) = parent_id {
            if let Some(parent) = self.task_mut(pid) {
                parent.children.retain(|&c| c != id);
            }
        }
        let mut to_delete = vec![id];
        let mut i = 0;
        while i < to_delete.len() {
            let current = to_delete[i];
            let children: Vec<Uuid> = self
                .task_ref(current)
                .map(|t| t.children.clone())
                .unwrap_or_default();
            to_delete.extend(children);
            i += 1;
        }
        self.tasks.retain(|t| !to_delete.contains(&t.id));

        // Navigate to the nearest remaining task (same index or the one before it).
        let dfs_after = self.build_visible_rows();
        if let Some(&target_id) = dfs_after.get(del_pos).or_else(|| dfs_after.last()) {
            self.navigate_to_id(target_id);
        } else {
            self.clamp_all_cursors();
        }

        self.status_message = Some("Deleted".into());
    }

    /// Handle the `gg` double-keypress pattern. Returns true on the second press.
    pub fn consume_gg(&mut self) -> bool {
        let now = Instant::now();
        let double = self.last_g_press
            .take()
            .map(|t| t.elapsed().as_millis() < 500)
            .unwrap_or(false);
        if !double { self.last_g_press = Some(now); }
        double
    }

    /// Handle the `dd` double-keypress pattern. Returns true if the delete fired.
    pub fn try_delete_dd(&mut self) -> bool {
        let now = Instant::now();
        let double = self.last_d_press
            .take()
            .map(|t| t.elapsed().as_millis() < 500)
            .unwrap_or(false);
        if double {
            self.push_undo();
            if let Some(id) = self.selected_task_id(self.focus) {
                self.delete_task(id);
            }
            true
        } else {
            self.last_d_press = Some(now);
            self.status_message = Some("d again to delete".into());
            false
        }
    }

    pub fn begin_insert_after(&mut self) {
        let col = self.focus;
        let current_id = self.selected_task_id(col);
        let (parent_id, position) = if let Some(id) = current_id {
            let task = self.task_ref(id).unwrap();
            (task.parent_id, InsertPosition::AfterSibling(id))
        } else {
            (None, InsertPosition::AtBeginning)
        };
        self.insert = Some(InsertState {
            title: String::new(),
            parent_id,
            status: Status::Todo,
            position,
        });
        self.mode = Mode::Insert;
    }

    pub fn begin_insert_before(&mut self) {
        let col = self.focus;
        let current_id = match self.selected_task_id(col) {
            Some(id) => id,
            None => return,
        };
        let task = self.task_ref(current_id).unwrap();
        let parent_id = task.parent_id;

        let position = if let Some(pid) = parent_id {
            let children = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
            let pos = children.iter().position(|&c| c == current_id).unwrap_or(0);
            if pos > 0 {
                InsertPosition::AfterSibling(children[pos - 1])
            } else {
                InsertPosition::AfterParent(pid)
            }
        } else {
            // Search all tasks (any status) to find the nearest root above current_id.
            // Filtering by visible_tasks_for(col) would miss roots with a different status.
            let current_pos = self.tasks.iter().position(|t| t.id == current_id).unwrap_or(0);
            let prev_root = self.tasks[..current_pos].iter().rev()
                .find(|t| t.parent_id.is_none())
                .map(|t| t.id);
            match prev_root {
                Some(prev_id) => InsertPosition::AfterSibling(prev_id),
                None => InsertPosition::AtBeginning,
            }
        };

        self.insert = Some(InsertState {
            title: String::new(),
            parent_id,
            status: Status::Todo,
            position,
        });
        self.mode = Mode::Insert;
    }

    pub fn commit_insert(&mut self) {
        if let Some(state) = self.insert.take() {
            if state.title.trim().is_empty() {
                self.mode = Mode::Normal;
                return;
            }
            let effective_status = if let Some(pid) = state.parent_id {
                if self.task_ref(pid).map(|p| p.status == Status::Doing).unwrap_or(false) {
                    Status::Doing
                } else {
                    state.status.clone()
                }
            } else {
                state.status.clone()
            };
            let mut task = Task::new(state.title.trim().to_string(), effective_status.clone());
            task.parent_id = state.parent_id;
            let task_id = task.id;
            let status = effective_status;

            match state.parent_id {
                None => {
                    let pos = match &state.position {
                        InsertPosition::AtBeginning => 0,
                        InsertPosition::AfterSibling(after_id) => {
                            self.tasks.iter().position(|t| t.id == *after_id)
                                .map(|i| i + 1)
                                .unwrap_or(self.tasks.len())
                        }
                        InsertPosition::AfterParent(_) => self.tasks.len(),
                    };
                    self.tasks.insert(pos, task);
                }
                Some(pid) => {
                    let task_insert_pos = match &state.position {
                        InsertPosition::AtBeginning => {
                            self.tasks.iter().position(|t| t.parent_id == Some(pid))
                                .unwrap_or(self.tasks.len())
                        }
                        InsertPosition::AfterSibling(after_id) => {
                            let after = *after_id;
                            self.tasks.iter().position(|t| t.id == after)
                                .map(|i| i + 1)
                                .unwrap_or(self.tasks.len())
                        }
                        InsertPosition::AfterParent(parent_id) => {
                            let p = *parent_id;
                            self.tasks.iter().position(|t| t.id == p)
                                .map(|i| i + 1)
                                .unwrap_or(self.tasks.len())
                        }
                    };
                    let child_pos = match &state.position {
                        InsertPosition::AtBeginning => 0,
                        InsertPosition::AfterSibling(after_id) => {
                            let after = *after_id;
                            self.task_ref(pid)
                                .and_then(|p| p.children.iter().position(|&c| c == after))
                                .map(|i| i + 1)
                                .unwrap_or_else(|| {
                                    self.task_ref(pid).map(|p| p.children.len()).unwrap_or(0)
                                })
                        }
                        InsertPosition::AfterParent(_) => 0,
                    };
                    self.tasks.insert(task_insert_pos, task);
                    if let Some(parent) = self.task_mut(pid) {
                        parent.children.insert(child_pos, task_id);
                    }
                    // Ensure the parent isn't ghost-collapsed (in collapsed but not visibly
                    // showing ▸ because it previously had no children).
                    self.collapsed.remove(&pid);
                }
            }

            // Propagate first so column membership is stable before we set the cursor.
            self.propagate_status_up(task_id);

            let visible = self.visible_tasks_for(status);
            if let Some(new_pos) = visible.iter().position(|t| t.id == task_id) {
                self.cursor[Self::status_index(status)] = new_pos;
            }
            self.focus = status;
        }
        self.mode = Mode::Normal;
    }

    pub fn begin_edit(&mut self, cursor_at_end: bool) {
        let col = self.focus;
        if let Some(id) = self.selected_task_id(col) {
            if let Some(task) = self.task_ref(id) {
                let cursor_pos = if cursor_at_end { task.title.chars().count() } else { 0 };
                self.edit = Some(EditState {
                    task_id: id,
                    title: task.title.clone(),
                    cursor_pos,
                });
                self.mode = Mode::Insert;
            }
        }
    }

    pub fn begin_edit_at_percent(&mut self, percent: usize) {
        let col = self.focus;
        if let Some(id) = self.selected_task_id(col) {
            if let Some(task) = self.task_ref(id) {
                let len = task.title.chars().count();
                let cursor_pos = (len * percent / 100).min(len);
                self.edit = Some(EditState {
                    task_id: id,
                    title: task.title.clone(),
                    cursor_pos,
                });
                self.mode = Mode::Insert;
            }
        }
    }

    pub fn commit_edit(&mut self) {
        if let Some(state) = self.edit.take() {
            if let Some(task) = self.task_mut(state.task_id) {
                task.title = state.title.trim().to_string();
            }
        }
        self.mode = Mode::Normal;
    }

    pub fn begin_bulk_insert(&mut self) {
        if self.selected_task_id(self.focus).is_none() {
            return;
        }
        self.bulk_insert = Some(BulkInsertState {
            step: BulkInsertStep::Num,
            num_input: String::new(),
            num: 0,
            prefix_input: String::new(),
        });
        self.mode = Mode::BulkInsert;
    }

    pub fn commit_bulk_insert(&mut self) {
        let state = match self.bulk_insert.take() {
            Some(s) => s,
            None => {
                self.mode = Mode::Normal;
                return;
            }
        };

        let parent_id = match self.selected_task_id(self.focus) {
            Some(id) => id,
            None => {
                self.mode = Mode::Normal;
                return;
            }
        };

        let prefix = state.prefix_input.trim().to_string();
        if prefix.is_empty() || state.num == 0 {
            self.mode = Mode::Normal;
            return;
        }

        let last_child_tasks_pos = self
            .task_ref(parent_id)
            .map(|p| p.children.clone())
            .unwrap_or_default()
            .iter()
            .filter_map(|&cid| self.tasks.iter().rposition(|t| t.id == cid))
            .max();

        let mut insert_pos = match last_child_tasks_pos {
            Some(pos) => pos + 1,
            None => self
                .tasks
                .iter()
                .position(|t| t.id == parent_id)
                .map(|i| i + 1)
                .unwrap_or(self.tasks.len()),
        };

        let mut new_child_ids = Vec::new();
        for i in 1..=state.num {
            let title = format!("{} {}", prefix, i);
            let mut task = Task::new(title, Status::Todo);
            task.parent_id = Some(parent_id);
            let task_id = task.id;
            self.tasks.insert(insert_pos, task);
            insert_pos += 1;
            new_child_ids.push(task_id);
        }

        let first_child_id = new_child_ids.first().copied();
        if let Some(parent) = self.task_mut(parent_id) {
            parent.children.extend(new_child_ids);
        }
        if let Some(cid) = first_child_id {
            self.propagate_status_up(cid);
        }

        self.mode = Mode::Normal;
        self.status_message = Some(format!("Created {} tasks", state.num));
    }

    pub(super) fn root_task_id(&self, id: Uuid) -> Uuid {
        let mut current = id;
        while let Some(parent_id) = self.task_ref(current).and_then(|t| t.parent_id) {
            current = parent_id;
        }
        current
    }

    pub fn indent_insert(&mut self) {
        let state = match self.insert.as_ref() {
            Some(s) => s,
            None => return,
        };
        let above_id = match &state.position {
            InsertPosition::AfterSibling(id) => *id,
            _ => return,
        };
        let status = state.status.clone();
        let children = match self.task_ref(above_id) {
            Some(t) => t.children.clone(),
            None => return,
        };
        let last_child = children.iter()
            .rev()
            .find(|&&cid| self.task_ref(cid).map(|c| c.status == status).unwrap_or(false))
            .copied();
        let state = self.insert.as_mut().unwrap();
        state.parent_id = Some(above_id);
        state.position = if let Some(lc) = last_child {
            InsertPosition::AfterSibling(lc)
        } else {
            InsertPosition::AfterParent(above_id)
        };
    }

    pub fn unindent_insert(&mut self) {
        let state = match self.insert.as_ref() {
            Some(s) => s,
            None => return,
        };
        let parent_id = match state.parent_id {
            Some(pid) => pid,
            None => return,
        };
        let grandparent_id = self.task_ref(parent_id).and_then(|t| t.parent_id);
        let state = self.insert.as_mut().unwrap();
        state.parent_id = grandparent_id;
        state.position = InsertPosition::AfterSibling(parent_id);
    }
}

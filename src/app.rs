use std::collections::HashSet;
use std::time::Instant;
use uuid::Uuid;

use crate::types::{Status, Task};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Edit,
    Move,
    ProjectEdit,
    Confirm(ConfirmAction),
    Help,
    BulkInsert,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BulkInsertStep {
    Num,
    Prefix,
}

pub struct BulkInsertState {
    pub step: BulkInsertStep,
    pub num_input: String,
    pub num: usize,
    pub prefix_input: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    DeleteTask(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Column {
    Todo,
    Doing,
    Done,
}

impl Column {
    pub fn status(&self) -> Status {
        match self {
            Column::Todo => Status::Todo,
            Column::Doing => Status::Doing,
            Column::Done => Status::Done,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Column::Todo => Column::Doing,
            Column::Doing => Column::Done,
            Column::Done => Column::Done,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Column::Todo => Column::Todo,
            Column::Doing => Column::Todo,
            Column::Done => Column::Doing,
        }
    }
}

#[derive(Debug, Clone)]
pub enum InsertPosition {
    AtBeginning,
    AfterSibling(Uuid),
    AfterParent(Uuid),
}

pub struct InsertState {
    pub title: String,
    pub project: String,
    pub parent_id: Option<Uuid>,
    pub status: Status,
    pub position: InsertPosition,
}

pub struct EditState {
    pub task_id: Uuid,
    pub title: String,
    pub description: String,
}

pub struct MoveState {
    pub task_id: Uuid,
}

pub struct ProjectEditState {
    pub slot: usize,
    pub input: String,
}

pub struct App {
    pub tasks: Vec<Task>,
    /// 10 named project slots. Index 0 = key '1', ..., index 8 = key '9', index 9 = key '0'.
    pub projects: [Option<String>; 10],
    pub active_slots: [bool; 10],
    pub show_unc: bool,
    pub mode: Mode,
    pub focused_col: Column,
    pub cursor: [usize; 3],
    pub insert: Option<InsertState>,
    pub edit: Option<EditState>,
    pub move_state: Option<MoveState>,
    pub project_edit: Option<ProjectEditState>,
    pub bulk_insert: Option<BulkInsertState>,
    pub status_message: Option<String>,
    pub last_digit_press: Option<(usize, Instant)>,
    pub last_unc_press: Option<Instant>,
}

impl App {
    pub fn new(tasks: Vec<Task>, projects: [Option<String>; 10]) -> Self {
        Self {
            tasks,
            projects,
            active_slots: [true; 10],
            show_unc: true,
            mode: Mode::Normal,
            focused_col: Column::Todo,
            cursor: [0, 0, 0],
            insert: None,
            edit: None,
            move_state: None,
            project_edit: None,
            bulk_insert: None,
            status_message: None,
            last_digit_press: None,
            last_unc_press: None,
        }
    }

    pub fn col_index(col: Column) -> usize {
        match col {
            Column::Todo => 0,
            Column::Doing => 1,
            Column::Done => 2,
        }
    }

    pub fn slot_for_project(&self, project: &str) -> Option<usize> {
        if project.is_empty() {
            return None;
        }
        self.projects.iter().position(|p| p.as_deref() == Some(project))
    }

    pub fn is_unc(&self, task: &Task) -> bool {
        task.project.is_empty() || self.slot_for_project(&task.project).is_none()
    }

    pub fn task_visible(&self, task: &Task) -> bool {
        if self.is_unc(task) {
            self.show_unc
        } else {
            self.active_slots[self.slot_for_project(&task.project).unwrap()]
        }
    }

    /// A leaf task is Todo/Doing with no direct children that are also Todo/Doing.
    pub fn is_leaf_task(&self, task: &Task) -> bool {
        if !matches!(task.status, Status::Todo | Status::Doing) {
            return false;
        }
        !task.children.iter().any(|&cid| {
            self.task_ref(cid)
                .map(|c| matches!(c.status, Status::Todo | Status::Doing))
                .unwrap_or(false)
        })
    }

    pub fn doable_count_for_slot(&self, slot: usize) -> usize {
        let name = match &self.projects[slot] {
            Some(n) => n.as_str(),
            None => return 0,
        };
        self.tasks.iter().filter(|t| t.project == name && self.is_leaf_task(t)).count()
    }

    pub fn unc_doable_count(&self) -> usize {
        self.tasks.iter().filter(|t| self.is_unc(t) && self.is_leaf_task(t)).count()
    }

    pub fn has_unc_tasks(&self) -> bool {
        self.tasks.iter().any(|t| self.is_unc(t))
    }

    pub fn visible_tasks_for(&self, col: Column) -> Vec<&Task> {
        let status = col.status();
        let in_col: HashSet<Uuid> = self.tasks.iter()
            .filter(|t| t.status == status && self.task_visible(t))
            .map(|t| t.id)
            .collect();

        let mut result: Vec<&Task> = Vec::new();
        let mut added: HashSet<Uuid> = HashSet::new();

        for task in &self.tasks {
            if !in_col.contains(&task.id) || added.contains(&task.id) {
                continue;
            }
            if let Some(pid) = task.parent_id {
                if in_col.contains(&pid) {
                    continue;
                }
            }
            self.add_subtree(task.id, &in_col, &mut result, &mut added);
        }

        result
    }

    fn add_subtree<'a>(&'a self, id: Uuid, in_col: &HashSet<Uuid>, result: &mut Vec<&'a Task>, added: &mut HashSet<Uuid>) {
        if added.contains(&id) {
            return;
        }
        let children = match self.task_ref(id) {
            Some(task) => {
                result.push(task);
                added.insert(id);
                task.children.clone()
            }
            None => return,
        };
        for cid in children {
            if in_col.contains(&cid) {
                self.add_subtree(cid, in_col, result, added);
            }
        }
    }

    pub fn cursor_for(&self, col: Column) -> usize {
        self.cursor[Self::col_index(col)]
    }

    pub fn clamp_cursor(&mut self, col: Column) {
        let len = self.visible_tasks_for(col).len();
        let i = Self::col_index(col);
        if len == 0 {
            self.cursor[i] = 0;
        } else if self.cursor[i] >= len {
            self.cursor[i] = len - 1;
        }
    }

    pub fn clamp_all_cursors(&mut self) {
        for col in [Column::Todo, Column::Doing, Column::Done] {
            self.clamp_cursor(col);
        }
    }

    pub fn selected_task_id(&self, col: Column) -> Option<Uuid> {
        let tasks = self.visible_tasks_for(col);
        let cur = self.cursor_for(col);
        tasks.get(cur).map(|t| t.id)
    }

    pub fn task_mut(&mut self, id: Uuid) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn task_ref(&self, id: Uuid) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    // ── Navigation ──────────────────────────────────────────────────────────

    pub fn move_cursor_up(&mut self) {
        let i = Self::col_index(self.focused_col);
        if self.cursor[i] > 0 {
            self.cursor[i] -= 1;
        }
    }

    pub fn move_cursor_down(&mut self) {
        let col = self.focused_col;
        let len = self.visible_tasks_for(col).len();
        let i = Self::col_index(col);
        if self.cursor[i] + 1 < len {
            self.cursor[i] += 1;
        }
    }

    pub fn focus_next_col(&mut self) {
        self.focused_col = self.focused_col.next();
    }

    pub fn focus_prev_col(&mut self) {
        self.focused_col = self.focused_col.prev();
    }

    // ── Task movement ────────────────────────────────────────────────────────

    pub fn move_selected_right(&mut self) {
        let col = self.focused_col;
        let dest = col.next();
        if let Some(id) = self.selected_task_id(col) {
            let parent_id = self.task_ref(id).and_then(|t| t.parent_id);
            let new_status = dest.status();
            let src_status = col.status();
            if let Some(task) = self.task_mut(id) {
                if task.status != new_status {
                    task.transition_to(new_status.clone());
                }
            }
            if let Some(pid) = parent_id {
                if self.task_ref(pid).map(|t| t.status == src_status).unwrap_or(false) {
                    let children: Vec<Uuid> = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
                    let remaining = children.iter().any(|&cid| self.task_ref(cid).map(|c| c.status == src_status).unwrap_or(false));
                    if !remaining {
                        if let Some(parent) = self.task_mut(pid) {
                            parent.transition_to(new_status.clone());
                        }
                    }
                }
            }
            self.clamp_cursor(col);
            if let Some(pos) = self.visible_tasks_for(dest).iter().position(|t| t.id == id) {
                self.cursor[Self::col_index(dest)] = pos;
            }
            self.focused_col = dest;
        }
    }

    pub fn move_selected_left(&mut self) {
        let col = self.focused_col;
        let dest = col.prev();
        if let Some(id) = self.selected_task_id(col) {
            let parent_id = self.task_ref(id).and_then(|t| t.parent_id);
            let new_status = dest.status();
            let src_status = col.status();
            if let Some(task) = self.task_mut(id) {
                if task.status != new_status {
                    task.transition_to(new_status.clone());
                }
            }
            if let Some(pid) = parent_id {
                if self.task_ref(pid).map(|t| t.status == src_status).unwrap_or(false) {
                    let children: Vec<Uuid> = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
                    let remaining = children.iter().any(|&cid| self.task_ref(cid).map(|c| c.status == src_status).unwrap_or(false));
                    if !remaining {
                        if let Some(parent) = self.task_mut(pid) {
                            parent.transition_to(new_status.clone());
                        }
                    }
                }
            }
            self.clamp_cursor(col);
            if let Some(pos) = self.visible_tasks_for(dest).iter().position(|t| t.id == id) {
                self.cursor[Self::col_index(dest)] = pos;
            }
            self.focused_col = dest;
        }
    }

    pub fn swap_up(&mut self) {
        let col = self.focused_col;
        let visible: Vec<Uuid> = self.visible_tasks_for(col).iter().map(|t| t.id).collect();
        let cur = self.cursor_for(col);
        if cur == 0 || visible.is_empty() {
            return;
        }
        let a = visible[cur];
        let group_start = self.group_start(&visible, cur - 1);
        let first = visible[group_start];
        let ai = self.tasks.iter().position(|t| t.id == a).unwrap();
        let bi = self.tasks.iter().position(|t| t.id == first).unwrap();
        if bi < ai {
            self.tasks[bi..=ai].rotate_right(1);
        }
        self.cursor[Self::col_index(col)] = group_start;
    }

    pub fn swap_down(&mut self) {
        let col = self.focused_col;
        let visible: Vec<Uuid> = self.visible_tasks_for(col).iter().map(|t| t.id).collect();
        let cur = self.cursor_for(col);
        if cur + 1 >= visible.len() {
            return;
        }
        let a = visible[cur];
        let group_end = self.group_end(&visible, cur + 1);
        let last = visible[group_end];
        let ai = self.tasks.iter().position(|t| t.id == a).unwrap();
        let bi = self.tasks.iter().position(|t| t.id == last).unwrap();
        if ai < bi {
            self.tasks[ai..=bi].rotate_left(1);
        }
        self.cursor[Self::col_index(col)] = group_end;
    }

    fn group_end(&self, visible: &[Uuid], start: usize) -> usize {
        let id = visible[start];
        let task = match self.task_ref(id) {
            Some(t) => t,
            None => return start,
        };

        if let Some(pid) = task.parent_id {
            if !visible.contains(&pid) {
                let mut end = start;
                while end + 1 < visible.len() {
                    let next_pid = self.task_ref(visible[end + 1]).and_then(|t| t.parent_id);
                    if next_pid == Some(pid) { end += 1; } else { break; }
                }
                return end;
            }
        }

        let mut end = start;
        while end + 1 < visible.len() {
            let next_pid = self.task_ref(visible[end + 1]).and_then(|t| t.parent_id);
            if next_pid == Some(id) { end += 1; } else { break; }
        }
        end
    }

    fn group_start(&self, visible: &[Uuid], end: usize) -> usize {
        let id = visible[end];
        let task = match self.task_ref(id) {
            Some(t) => t,
            None => return end,
        };

        if let Some(pid) = task.parent_id {
            if let Some(parent_vis) = visible.iter().position(|&v| v == pid) {
                return parent_vis;
            }
            let mut start = end;
            while start > 0 {
                let prev_pid = self.task_ref(visible[start - 1]).and_then(|t| t.parent_id);
                if prev_pid == Some(pid) { start -= 1; } else { break; }
            }
            return start;
        }

        end
    }

    // ── Parent/child ─────────────────────────────────────────────────────────

    fn task_depth(&self, id: Uuid) -> usize {
        let mut depth = 0;
        let mut current = id;
        while let Some(pid) = self.task_ref(current).and_then(|t| t.parent_id) {
            depth += 1;
            current = pid;
        }
        depth
    }

    fn ancestor_at_depth(&self, id: Uuid, target_depth: usize) -> Option<Uuid> {
        let mut current = id;
        let mut depth = self.task_depth(id);
        while depth > target_depth {
            current = self.task_ref(current).and_then(|t| t.parent_id)?;
            depth -= 1;
        }
        (depth == target_depth).then_some(current)
    }

    pub fn make_child(&mut self) {
        let col = self.focused_col;
        let visible: Vec<Uuid> = self.visible_tasks_for(col).iter().map(|t| t.id).collect();
        let cur = self.cursor_for(col);
        if cur == 0 || visible.is_empty() {
            return;
        }
        let child_id = visible[cur];
        let above_id = visible[cur - 1];

        let child_depth = self.task_depth(child_id);
        let above_depth = self.task_depth(above_id);

        // Increase depth by exactly one level: find the ancestor of the task above
        // at the current task's depth. This prevents skipping indentation levels.
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

            // Project reconciliation: the whole family must share one project.
            let parent_project = self.task_ref(new_parent_id).map(|t| t.project.clone()).unwrap_or_default();
            let child_project = self.task_ref(child_id).map(|t| t.project.clone()).unwrap_or_default();
            if !parent_project.is_empty() {
                // Parent (or parent's family) has a project — child's subtree inherits it.
                self.set_project_recursive(child_id, parent_project);
            } else if !child_project.is_empty() {
                // Parent is unc — the whole parent family adopts the child's project.
                let root_id = self.root_task_id(new_parent_id);
                self.set_project_recursive(root_id, child_project);
            }
        }
        self.status_message = Some("Made child of task above".into());
    }

    pub fn make_root(&mut self) {
        let col = self.focused_col;
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

    // ── Delete ───────────────────────────────────────────────────────────────

    pub fn delete_selected(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            self.mode = Mode::Confirm(ConfirmAction::DeleteTask(id));
        }
    }

    pub fn confirm_delete(&mut self, id: Uuid) {
        let parent_id = self.task_ref(id).and_then(|t| t.parent_id);
        if let Some(pid) = parent_id {
            if let Some(parent) = self.task_mut(pid) {
                parent.children.retain(|&c| c != id);
            }
        }
        let children: Vec<Uuid> = self
            .task_ref(id)
            .map(|t| t.children.clone())
            .unwrap_or_default();
        for cid in children {
            if let Some(child) = self.task_mut(cid) {
                child.parent_id = None;
            }
        }
        self.tasks.retain(|t| t.id != id);
        self.clamp_cursor(self.focused_col);
        self.mode = Mode::Normal;
        self.status_message = Some("Task deleted".into());
    }

    // ── Insert ───────────────────────────────────────────────────────────────

    /// Returns the project name to use for a new task given the current filter state.
    /// If exactly one project slot is active, use that project; otherwise use empty (unc).
    pub fn default_project_for_insert(&self) -> String {
        let active: Vec<usize> = (0..10)
            .filter(|&i| self.active_slots[i] && self.projects[i].is_some())
            .collect();
        if active.len() == 1 {
            self.projects[active[0]].clone().unwrap_or_default()
        } else {
            String::new()
        }
    }

    pub fn begin_insert_after(&mut self) {
        let col = self.focused_col;
        let current_id = self.selected_task_id(col);
        let (parent_id, project, position) = if let Some(id) = current_id {
            let task = self.task_ref(id).unwrap();
            let project = if task.parent_id.is_some() {
                task.project.clone()
            } else {
                self.default_project_for_insert()
            };
            (task.parent_id, project, InsertPosition::AfterSibling(id))
        } else {
            let project = self.default_project_for_insert();
            (None, project, InsertPosition::AtBeginning)
        };
        self.insert = Some(InsertState {
            title: String::new(),
            project,
            parent_id,
            status: col.status(),
            position,
        });
        self.mode = Mode::Insert;
    }

    pub fn begin_insert_todo_end(&mut self) {
        let todo_roots: Vec<Uuid> = self.visible_tasks_for(Column::Todo)
            .iter()
            .filter(|t| t.parent_id.is_none())
            .map(|t| t.id)
            .collect();
        let position = if let Some(&last_id) = todo_roots.last() {
            InsertPosition::AfterSibling(last_id)
        } else {
            InsertPosition::AtBeginning
        };
        let project = self.default_project_for_insert();
        self.focused_col = Column::Todo;
        self.insert = Some(InsertState {
            title: String::new(),
            project,
            parent_id: None,
            status: Status::Todo,
            position,
        });
        self.mode = Mode::Insert;
    }

    pub fn begin_insert_before(&mut self) {
        let col = self.focused_col;
        let current_id = match self.selected_task_id(col) {
            Some(id) => id,
            None => return,
        };
        let task = self.task_ref(current_id).unwrap();
        let parent_id = task.parent_id;
        let project = if parent_id.is_some() {
            task.project.clone()
        } else {
            self.default_project_for_insert()
        };

        let position = if let Some(pid) = parent_id {
            let children = self.task_ref(pid).map(|p| p.children.clone()).unwrap_or_default();
            let pos = children.iter().position(|&c| c == current_id).unwrap_or(0);
            if pos > 0 {
                InsertPosition::AfterSibling(children[pos - 1])
            } else {
                InsertPosition::AfterParent(pid)
            }
        } else {
            let visible_roots: Vec<Uuid> = self.visible_tasks_for(col).iter()
                .filter(|t| t.parent_id.is_none())
                .map(|t| t.id)
                .collect();
            let pos = visible_roots.iter().position(|&id| id == current_id).unwrap_or(0);
            if pos > 0 {
                InsertPosition::AfterSibling(visible_roots[pos - 1])
            } else {
                InsertPosition::AtBeginning
            }
        };

        self.insert = Some(InsertState {
            title: String::new(),
            project,
            parent_id,
            status: col.status(),
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
            let mut task = Task::new(state.title.trim().to_string(), state.project.clone(), state.status.clone());
            task.parent_id = state.parent_id;
            let task_id = task.id;
            let status = state.status.clone();

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
                }
            }

            let col = match status {
                Status::Todo => Column::Todo,
                Status::Doing => Column::Doing,
                Status::Done => Column::Done,
            };
            let visible = self.visible_tasks_for(col);
            if let Some(new_pos) = visible.iter().position(|t| t.id == task_id) {
                self.cursor[Self::col_index(col)] = new_pos;
            }
        }
        self.mode = Mode::Normal;
    }

    // ── Edit ─────────────────────────────────────────────────────────────────

    pub fn begin_edit(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            if let Some(task) = self.task_ref(id) {
                self.edit = Some(EditState {
                    task_id: id,
                    title: task.title.clone(),
                    description: task.description.clone().unwrap_or_default(),
                });
                self.mode = Mode::Edit;
            }
        }
    }

    pub fn commit_edit(&mut self) {
        if let Some(state) = self.edit.take() {
            if let Some(task) = self.task_mut(state.task_id) {
                task.title = state.title.trim().to_string();
                task.description = if state.description.is_empty() {
                    None
                } else {
                    Some(state.description)
                };
            }
        }
        self.mode = Mode::Normal;
    }

    // ── Project filter ────────────────────────────────────────────────────────

    pub fn toggle_slot(&mut self, slot: usize) {
        let now = Instant::now();
        let is_double = self.last_digit_press
            .map(|(s, t)| s == slot && now.duration_since(t).as_millis() < 500)
            .unwrap_or(false);

        if is_double {
            for i in 0..10 {
                self.active_slots[i] = i == slot;
            }
            self.show_unc = false;
            self.last_digit_press = None;
        } else {
            self.active_slots[slot] = !self.active_slots[slot];
            self.last_digit_press = Some((slot, now));
        }
        self.clamp_all_cursors();
    }

    pub fn toggle_unc(&mut self) {
        let now = Instant::now();
        let is_double = self.last_unc_press
            .map(|t| now.duration_since(t).as_millis() < 500)
            .unwrap_or(false);

        if is_double {
            for i in 0..10 {
                self.active_slots[i] = false;
            }
            self.show_unc = true;
            self.last_unc_press = None;
        } else {
            self.show_unc = !self.show_unc;
            self.last_unc_press = Some(now);
        }
        self.clamp_all_cursors();
    }

    pub fn enable_all(&mut self) {
        self.active_slots = [true; 10];
        self.show_unc = true;
        self.clamp_all_cursors();
    }

    pub fn disable_all(&mut self) {
        self.active_slots = [false; 10];
        self.show_unc = false;
        self.clamp_all_cursors();
    }

    // ── Project management ────────────────────────────────────────────────────

    pub fn begin_project_edit(&mut self) {
        let input = self.projects[0].clone().unwrap_or_default();
        self.project_edit = Some(ProjectEditState { slot: 0, input });
        self.mode = Mode::ProjectEdit;
    }

    /// Save current slot input and move by delta (-1 or +1).
    pub fn project_edit_navigate(&mut self, delta: i32) {
        if let Some(ref mut pe) = self.project_edit {
            let name = pe.input.trim().to_string();
            self.projects[pe.slot] = if name.is_empty() { None } else { Some(name) };
            let new_slot = ((pe.slot as i32 + delta).rem_euclid(10)) as usize;
            pe.slot = new_slot;
            pe.input = self.projects[new_slot].clone().unwrap_or_default();
        }
    }

    pub fn commit_project_edit(&mut self) {
        if let Some(pe) = self.project_edit.take() {
            let name = pe.input.trim().to_string();
            self.projects[pe.slot] = if name.is_empty() { None } else { Some(name) };
        }
        self.mode = Mode::Normal;
    }

    // ── Move task to project ──────────────────────────────────────────────────

    pub fn begin_move_project(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            let root_id = self.root_task_id(id);
            self.move_state = Some(MoveState { task_id: root_id });
            self.mode = Mode::Move;
        }
    }

    pub fn move_to_slot(&mut self, slot: usize) {
        if let Some(ms) = self.move_state.take() {
            if let Some(name) = self.projects[slot].clone() {
                self.set_project_recursive(ms.task_id, name);
            } else {
                // Empty slot → assign to unc (empty project)
                self.set_project_recursive(ms.task_id, String::new());
            }
        }
        self.mode = Mode::Normal;
    }

    // ── Bulk insert ───────────────────────────────────────────────────────────

    pub fn begin_bulk_insert(&mut self) {
        if self.selected_task_id(self.focused_col).is_none() {
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

        let parent_id = match self.selected_task_id(self.focused_col) {
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

        let project = self.task_ref(parent_id).map(|t| t.project.clone()).unwrap_or_default();

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
            let mut task = Task::new(title, project.clone(), Status::Todo);
            task.parent_id = Some(parent_id);
            let task_id = task.id;
            self.tasks.insert(insert_pos, task);
            insert_pos += 1;
            new_child_ids.push(task_id);
        }

        if let Some(parent) = self.task_mut(parent_id) {
            parent.children.extend(new_child_ids);
        }

        self.mode = Mode::Normal;
        self.status_message = Some(format!("Created {} tasks", state.num));
    }

    pub fn undone_leaf_count(&self, id: Uuid) -> usize {
        let task = match self.task_ref(id) {
            Some(t) => t,
            None => return 0,
        };
        if task.children.is_empty() {
            (task.status != Status::Done) as usize
        } else {
            task.children.clone().iter().map(|&cid| self.undone_leaf_count(cid)).sum()
        }
    }

    fn root_task_id(&self, id: Uuid) -> Uuid {
        let mut current = id;
        while let Some(parent_id) = self.task_ref(current).and_then(|t| t.parent_id) {
            current = parent_id;
        }
        current
    }

    pub fn set_project_recursive(&mut self, id: Uuid, project: String) {
        let children = self.task_ref(id).map(|t| t.children.clone()).unwrap_or_default();
        if let Some(task) = self.task_mut(id) {
            task.project = project.clone();
        }
        for child_id in children {
            self.set_project_recursive(child_id, project.clone());
        }
    }

    // ── Insert indent ────────────────────────────────────────────────────────

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
        let (new_project, children) = match self.task_ref(above_id) {
            Some(t) => (t.project.clone(), t.children.clone()),
            None => return,
        };
        let last_child = children.iter()
            .rev()
            .find(|&&cid| self.task_ref(cid).map(|c| c.status == status).unwrap_or(false))
            .copied();
        let state = self.insert.as_mut().unwrap();
        state.parent_id = Some(above_id);
        state.project = new_project;
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
        let (grandparent_id, project) = self.task_ref(parent_id)
            .map(|t| (t.parent_id, t.project.clone()))
            .unwrap_or((None, String::new()));
        let state = self.insert.as_mut().unwrap();
        state.parent_id = grandparent_id;
        state.project = project;
        state.position = InsertPosition::AfterSibling(parent_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Status, Task};

    fn no_projects() -> [Option<String>; 10] {
        Default::default()
    }

    fn with_projects(names: &[&str]) -> [Option<String>; 10] {
        let mut p: [Option<String>; 10] = Default::default();
        for (i, name) in names.iter().enumerate().take(10) {
            p[i] = Some((*name).to_string());
        }
        p
    }

    fn task(title: &str, project: &str, status: Status) -> Task {
        Task::new(title.into(), project.into(), status)
    }

    fn empty_app() -> App {
        App::new(vec![], no_projects())
    }

    // ── Column ──────────────────────────────────────────────────────────────────

    #[test]
    fn column_next_clamps_at_done() {
        assert_eq!(Column::Todo.next(), Column::Doing);
        assert_eq!(Column::Doing.next(), Column::Done);
        assert_eq!(Column::Done.next(), Column::Done);
    }

    #[test]
    fn column_prev_clamps_at_todo() {
        assert_eq!(Column::Done.prev(), Column::Doing);
        assert_eq!(Column::Doing.prev(), Column::Todo);
        assert_eq!(Column::Todo.prev(), Column::Todo);
    }

    #[test]
    fn column_status_maps_correctly() {
        assert_eq!(Column::Todo.status(), Status::Todo);
        assert_eq!(Column::Doing.status(), Status::Doing);
        assert_eq!(Column::Done.status(), Status::Done);
    }

    // ── is_unc / slot_for_project ────────────────────────────────────────────

    #[test]
    fn is_unc_empty_project() {
        let app = App::new(vec![], with_projects(&["work"]));
        let t = task("x", "", Status::Todo);
        assert!(app.is_unc(&t));
    }

    #[test]
    fn is_unc_no_slot_match() {
        let app = App::new(vec![], with_projects(&["work"]));
        let t = task("x", "personal", Status::Todo);
        assert!(app.is_unc(&t));
    }

    #[test]
    fn is_unc_false_when_slot_matches() {
        let app = App::new(vec![], with_projects(&["work"]));
        let t = task("x", "work", Status::Todo);
        assert!(!app.is_unc(&t));
    }

    // ── task_visible ────────────────────────────────────────────────────────────

    #[test]
    fn task_visible_unc_respects_show_unc() {
        let mut app = App::new(vec![], no_projects());
        let t = task("x", "", Status::Todo);
        app.show_unc = true;
        assert!(app.task_visible(&t));
        app.show_unc = false;
        assert!(!app.task_visible(&t));
    }

    #[test]
    fn task_visible_slot_respects_active_slots() {
        let mut app = App::new(vec![], with_projects(&["work"]));
        let t = task("x", "work", Status::Todo);
        app.active_slots[0] = true;
        assert!(app.task_visible(&t));
        app.active_slots[0] = false;
        assert!(!app.task_visible(&t));
    }

    // ── is_leaf_task ─────────────────────────────────────────────────────────

    #[test]
    fn is_leaf_task_done_task_is_not_leaf() {
        let app = empty_app();
        let t = task("x", "", Status::Done);
        assert!(!app.is_leaf_task(&t));
    }

    #[test]
    fn is_leaf_task_todo_with_no_children_is_leaf() {
        let app = empty_app();
        let t = task("x", "", Status::Todo);
        assert!(app.is_leaf_task(&t));
    }

    #[test]
    fn is_leaf_task_false_when_has_active_child() {
        let mut parent = task("parent", "", Status::Todo);
        let child = task("child", "", Status::Todo);
        parent.children.push(child.id);
        let app = App::new(vec![parent.clone(), child], no_projects());
        assert!(!app.is_leaf_task(&parent));
    }

    // ── visible_tasks_for ─────────────────────────────────────────────────────

    #[test]
    fn visible_tasks_for_returns_matching_visible_tasks() {
        let t1 = task("a", "", Status::Todo);
        let t2 = task("b", "", Status::Doing);
        let app = App::new(vec![t1.clone(), t2.clone()], no_projects());
        let todo = app.visible_tasks_for(Column::Todo);
        assert_eq!(todo.len(), 1);
        assert_eq!(todo[0].id, t1.id);
    }

    #[test]
    fn visible_tasks_for_respects_show_unc() {
        let t = task("a", "", Status::Todo);
        let mut app = App::new(vec![t], no_projects());
        app.show_unc = false;
        assert!(app.visible_tasks_for(Column::Todo).is_empty());
    }

    #[test]
    fn visible_tasks_for_orders_children_after_parent() {
        let mut parent = task("parent", "", Status::Todo);
        let child = task("child", "", Status::Todo);
        parent.children.push(child.id);
        let child_id = child.id;
        let parent_id = parent.id;
        let mut child2 = child.clone();
        child2.parent_id = Some(parent_id);
        let app = App::new(vec![parent, child2], no_projects());
        let visible = app.visible_tasks_for(Column::Todo);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].id, parent_id);
        assert_eq!(visible[1].id, child_id);
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    #[test]
    fn move_cursor_up_clamps_at_zero() {
        let mut app = App::new(vec![task("x", "", Status::Todo)], no_projects());
        app.cursor[0] = 0;
        app.move_cursor_up();
        assert_eq!(app.cursor[0], 0);
    }

    #[test]
    fn move_cursor_down_clamps_at_last() {
        let t1 = task("a", "", Status::Todo);
        let t2 = task("b", "", Status::Todo);
        let mut app = App::new(vec![t1, t2], no_projects());
        app.cursor[0] = 1;
        app.move_cursor_down();
        assert_eq!(app.cursor[0], 1);
    }

    #[test]
    fn focus_next_prev_col() {
        let mut app = empty_app();
        assert_eq!(app.focused_col, Column::Todo);
        app.focus_next_col();
        assert_eq!(app.focused_col, Column::Doing);
        app.focus_next_col();
        assert_eq!(app.focused_col, Column::Done);
        app.focus_next_col(); // clamps
        assert_eq!(app.focused_col, Column::Done);
        app.focus_prev_col();
        assert_eq!(app.focused_col, Column::Doing);
    }

    // ── Task movement ─────────────────────────────────────────────────────────

    #[test]
    fn move_selected_right_changes_status() {
        let t = task("x", "", Status::Todo);
        let id = t.id;
        let mut app = App::new(vec![t], no_projects());
        app.focused_col = Column::Todo;
        app.move_selected_right();
        let moved = app.task_ref(id).unwrap();
        assert_eq!(moved.status, Status::Doing);
        assert_eq!(app.focused_col, Column::Doing);
    }

    #[test]
    fn move_selected_left_changes_status() {
        let t = task("x", "", Status::Doing);
        let id = t.id;
        let mut app = App::new(vec![t], no_projects());
        app.focused_col = Column::Doing;
        app.move_selected_left();
        let moved = app.task_ref(id).unwrap();
        assert_eq!(moved.status, Status::Todo);
    }

    #[test]
    fn move_selected_right_at_done_is_noop() {
        let t = task("x", "", Status::Done);
        let id = t.id;
        let mut app = App::new(vec![t], no_projects());
        app.focused_col = Column::Done;
        app.move_selected_right();
        assert_eq!(app.task_ref(id).unwrap().status, Status::Done);
    }

    // ── make_child / make_root ────────────────────────────────────────────────

    #[test]
    fn make_child_increments_depth_by_one_when_above_is_deeper() {
        // Structure: root1 → child_of_root1, then root2 at cursor.
        // Pressing `>` on root2 should make it a sibling of child_of_root1
        // (child of root1), NOT a grandchild (child of child_of_root1).
        let root1 = task("root1", "", Status::Todo);
        let mut child_of_root1 = task("child_of_root1", "", Status::Todo);
        let root2 = task("root2", "", Status::Todo);
        let root1_id = root1.id;
        let child_id = child_of_root1.id;
        let root2_id = root2.id;
        child_of_root1.parent_id = Some(root1_id);
        let mut app = App::new(vec![root1, child_of_root1, root2], no_projects());
        // Manually wire root1.children so visible_tasks_for orders correctly
        app.task_mut(root1_id).unwrap().children.push(child_id);
        app.focused_col = Column::Todo;
        // visible order: root1 (0), child_of_root1 (1), root2 (2)
        app.cursor[0] = 2;
        app.make_child();
        // root2 should now be a child of root1 (depth 1), not child_of_root1 (depth 2)
        assert_eq!(app.task_ref(root2_id).unwrap().parent_id, Some(root1_id));
        assert!(app.task_ref(root1_id).unwrap().children.contains(&root2_id));
        assert!(!app.task_ref(child_id).unwrap().children.contains(&root2_id));
    }

    #[test]
    fn make_child_links_parent_and_child() {
        let parent = task("parent", "", Status::Todo);
        let child = task("child", "", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        let mut app = App::new(vec![parent, child], no_projects());
        app.focused_col = Column::Todo;
        app.cursor[0] = 1; // select child (second visible task)
        app.make_child();
        assert_eq!(app.task_ref(child_id).unwrap().parent_id, Some(parent_id));
        assert!(app.task_ref(parent_id).unwrap().children.contains(&child_id));
    }

    #[test]
    fn make_root_promotes_depth1_to_root() {
        let mut parent = task("parent", "", Status::Todo);
        let mut child = task("child", "", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![parent, child], no_projects());
        app.focused_col = Column::Todo;
        app.cursor[0] = 1;
        app.make_root();
        assert!(app.task_ref(child_id).unwrap().parent_id.is_none());
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&child_id));
    }

    #[test]
    fn make_root_decrements_depth_by_one_not_to_root() {
        // grandparent → parent → child (depth 2)
        // pressing `<` on child should make it depth 1 (child of grandparent), not root
        let mut grandparent = task("grandparent", "", Status::Todo);
        let mut parent = task("parent", "", Status::Todo);
        let mut child = task("child", "", Status::Todo);
        let gp_id = grandparent.id;
        let parent_id = parent.id;
        let child_id = child.id;
        parent.parent_id = Some(gp_id);
        grandparent.children.push(parent_id);
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![grandparent, parent, child], no_projects());
        app.focused_col = Column::Todo;
        // visible order: grandparent(0), parent(1), child(2)
        app.cursor[0] = 2;
        app.make_root();
        assert_eq!(app.task_ref(child_id).unwrap().parent_id, Some(gp_id));
        assert!(app.task_ref(gp_id).unwrap().children.contains(&child_id));
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&child_id));
    }

    // ── make_child project inheritance ───────────────────────────────────────

    #[test]
    fn make_child_unc_inherits_parent_project() {
        let parent = task("parent", "work", Status::Todo);
        let child = task("child", "", Status::Todo);
        let child_id = child.id;
        let mut app = App::new(vec![parent, child], no_projects());
        app.focused_col = Column::Todo;
        app.cursor[0] = 1;
        app.make_child();
        assert_eq!(app.task_ref(child_id).unwrap().project, "work");
    }

    #[test]
    fn make_child_unc_parent_inherits_child_project() {
        let parent = task("parent", "", Status::Todo);
        let child = task("child", "work", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        let mut app = App::new(vec![parent, child], no_projects());
        app.focused_col = Column::Todo;
        app.cursor[0] = 1;
        app.make_child();
        assert_eq!(app.task_ref(parent_id).unwrap().project, "work");
        assert_eq!(app.task_ref(child_id).unwrap().project, "work");
    }

    #[test]
    fn make_child_parent_project_takes_precedence_over_child() {
        let parent = task("parent", "work", Status::Todo);
        let child = task("child", "personal", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        let mut app = App::new(vec![parent, child], no_projects());
        app.focused_col = Column::Todo;
        app.cursor[0] = 1;
        app.make_child();
        assert_eq!(app.task_ref(parent_id).unwrap().project, "work");
        assert_eq!(app.task_ref(child_id).unwrap().project, "work");
    }

    // ── confirm_delete ─────────────────────────────────────────────────────────

    #[test]
    fn confirm_delete_removes_task_and_orphans_children() {
        let mut parent = task("parent", "", Status::Todo);
        let mut child = task("child", "", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![parent, child], no_projects());
        app.confirm_delete(parent_id);
        assert!(app.task_ref(parent_id).is_none());
        assert!(app.task_ref(child_id).unwrap().parent_id.is_none());
    }

    // ── toggle_slot / enable_all / disable_all ────────────────────────────────

    #[test]
    fn toggle_slot_flips_slot() {
        let mut app = App::new(vec![], with_projects(&["work"]));
        assert!(app.active_slots[0]);
        app.toggle_slot(0);
        assert!(!app.active_slots[0]);
        app.toggle_slot(0);
        assert!(app.active_slots[0]);
    }

    #[test]
    fn enable_all_resets_everything() {
        let mut app = App::new(vec![], with_projects(&["work"]));
        app.active_slots = [false; 10];
        app.show_unc = false;
        app.enable_all();
        assert!(app.active_slots.iter().all(|&s| s));
        assert!(app.show_unc);
    }

    #[test]
    fn disable_all_hides_everything() {
        let mut app = empty_app();
        app.disable_all();
        assert!(app.active_slots.iter().all(|&s| !s));
        assert!(!app.show_unc);
    }

    // ── default_project_for_insert ────────────────────────────────────────────

    #[test]
    fn default_project_empty_when_multiple_active() {
        let app = App::new(vec![], with_projects(&["work", "home"]));
        assert_eq!(app.default_project_for_insert(), "");
    }

    #[test]
    fn default_project_uses_sole_active_slot() {
        let mut app = App::new(vec![], with_projects(&["work", "home"]));
        app.active_slots = [false; 10];
        app.active_slots[1] = true;
        assert_eq!(app.default_project_for_insert(), "home");
    }

    // ── commit_insert ─────────────────────────────────────────────────────────

    #[test]
    fn commit_insert_adds_task() {
        let mut app = empty_app();
        app.begin_insert_after();
        app.insert.as_mut().unwrap().title = "new task".into();
        app.commit_insert();
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].title, "new task");
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn commit_insert_empty_title_aborts() {
        let mut app = empty_app();
        app.begin_insert_after();
        // title stays empty
        app.commit_insert();
        assert!(app.tasks.is_empty());
        assert_eq!(app.mode, Mode::Normal);
    }

    // ── set_project_recursive ─────────────────────────────────────────────────

    #[test]
    fn set_project_recursive_updates_tree() {
        let mut parent = task("parent", "old", Status::Todo);
        let mut child = task("child", "old", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![parent, child], no_projects());
        app.set_project_recursive(parent_id, "new".into());
        assert_eq!(app.task_ref(parent_id).unwrap().project, "new");
        assert_eq!(app.task_ref(child_id).unwrap().project, "new");
    }

    // ── has_unc_tasks / unc_doable_count ──────────────────────────────────────

    #[test]
    fn has_unc_tasks_detects_unclassified() {
        let mut app = empty_app();
        assert!(!app.has_unc_tasks());
        app.tasks.push(task("x", "", Status::Todo));
        assert!(app.has_unc_tasks());
    }

    #[test]
    fn undone_leaf_count_sums_non_done_leaves() {
        let mut root = task("root", "", Status::Todo);
        let mut child_a = task("child_a", "", Status::Todo);
        let mut child_b = task("child_b", "", Status::Doing);
        let mut child_c = task("child_c", "", Status::Done);
        let root_id = root.id;
        let a_id = child_a.id;
        let b_id = child_b.id;
        let c_id = child_c.id;
        child_a.parent_id = Some(root_id);
        child_b.parent_id = Some(root_id);
        child_c.parent_id = Some(root_id);
        root.children.extend([a_id, b_id, c_id]);
        let app = App::new(vec![root, child_a, child_b, child_c], no_projects());
        // child_a (todo) + child_b (doing) = 2 undone leaves; child_c (done) excluded
        assert_eq!(app.undone_leaf_count(root_id), 2);
    }

    #[test]
    fn undone_leaf_count_is_none_for_leaf_task() {
        let leaf = task("leaf", "", Status::Todo);
        let leaf_id = leaf.id;
        let app = App::new(vec![leaf], no_projects());
        // Leaf tasks return 1 from the recursive helper (they count themselves),
        // but draw_card only calls undone_leaf_count when children is non-empty.
        // The value when called on a plain leaf is 1 (itself, undone).
        assert_eq!(app.undone_leaf_count(leaf_id), 1);
    }

    #[test]
    fn unc_doable_count_counts_leaf_unc_tasks() {
        let mut parent = task("parent", "", Status::Todo);
        let mut child = task("child", "", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let app = App::new(vec![parent, child], no_projects());
        // Only the leaf (child) should be counted
        assert_eq!(app.unc_doable_count(), 1);
    }
}

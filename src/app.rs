use std::collections::HashSet;
use uuid::Uuid;

use crate::types::{Status, Task, collect_projects, project_matches, segment_boundary_matches};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Edit,
    Filter,
    Move,
    Confirm(ConfirmAction),
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

pub struct FilterState {
    pub input: String,
}

pub struct MoveState {
    pub task_id: Uuid,
    pub target_input: String,
    pub suggestion_cursor: Option<usize>,
    pub suggestion_query: Option<String>,
}

pub struct App {
    pub tasks: Vec<Task>,
    pub mode: Mode,
    pub focused_col: Column,
    pub cursor: [usize; 3],
    pub active_projects: Vec<String>,
    pub insert: Option<InsertState>,
    pub edit: Option<EditState>,
    pub filter: Option<FilterState>,
    pub move_state: Option<MoveState>,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks,
            mode: Mode::Normal,
            focused_col: Column::Todo,
            cursor: [0, 0, 0],
            active_projects: Vec::new(),
            insert: None,
            edit: None,
            filter: None,
            move_state: None,
            status_message: None,
        }
    }

    pub fn col_index(col: Column) -> usize {
        match col {
            Column::Todo => 0,
            Column::Doing => 1,
            Column::Done => 2,
        }
    }

    pub fn visible_tasks_for(&self, col: Column) -> Vec<&Task> {
        let status = col.status();
        let in_col: HashSet<Uuid> = self.tasks.iter()
            .filter(|t| {
                t.status == status
                    && (self.active_projects.is_empty()
                        || self.active_projects.iter().any(|f| project_matches(&t.project, f)))
            })
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

    /// Returns the last visible index of the group that starts at `start`.
    /// A group is an orphan-sibling set (consecutive items sharing the same ghost parent)
    /// or a root task followed by all its direct children.
    fn group_end(&self, visible: &[Uuid], start: usize) -> usize {
        let id = visible[start];
        let task = match self.task_ref(id) {
            Some(t) => t,
            None => return start,
        };

        if let Some(pid) = task.parent_id {
            if !visible.contains(&pid) {
                // Ghost parent: extend while consecutive siblings share the same parent
                let mut end = start;
                while end + 1 < visible.len() {
                    let next_pid = self.task_ref(visible[end + 1]).and_then(|t| t.parent_id);
                    if next_pid == Some(pid) { end += 1; } else { break; }
                }
                return end;
            }
        }

        // Root task: extend while the next item is a direct child of this task
        let mut end = start;
        while end + 1 < visible.len() {
            let next_pid = self.task_ref(visible[end + 1]).and_then(|t| t.parent_id);
            if next_pid == Some(id) { end += 1; } else { break; }
        }
        end
    }

    /// Returns the first visible index of the group that ends at `end`.
    fn group_start(&self, visible: &[Uuid], end: usize) -> usize {
        let id = visible[end];
        let task = match self.task_ref(id) {
            Some(t) => t,
            None => return end,
        };

        if let Some(pid) = task.parent_id {
            // Visible parent: the whole parent+children block is the group
            if let Some(parent_vis) = visible.iter().position(|&v| v == pid) {
                return parent_vis;
            }
            // Ghost parent: extend backwards while consecutive siblings share the same parent
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

    pub fn make_child(&mut self) {
        let col = self.focused_col;
        let visible: Vec<Uuid> = self.visible_tasks_for(col).iter().map(|t| t.id).collect();
        let cur = self.cursor_for(col);
        if cur == 0 || visible.is_empty() {
            return;
        }
        let child_id = visible[cur];
        let parent_id = visible[cur - 1];
        if let Some(child) = self.task_mut(child_id) {
            child.parent_id = Some(parent_id);
        }
        if let Some(parent) = self.task_mut(parent_id) {
            if !parent.children.contains(&child_id) {
                parent.children.push(child_id);
            }
        }
        self.status_message = Some("Made child of task above".into());
    }

    pub fn make_root(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            let old_parent = self.task_ref(id).and_then(|t| t.parent_id);
            if let Some(task) = self.task_mut(id) {
                task.parent_id = None;
            }
            if let Some(parent_id) = old_parent {
                if let Some(parent) = self.task_mut(parent_id) {
                    parent.children.retain(|&c| c != id);
                }
            }
            self.status_message = Some("Promoted to root task".into());
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

    /// 'o' — create sibling after current card (or new top-level if column empty)
    pub fn begin_insert_after(&mut self) {
        let col = self.focused_col;
        let current_id = self.selected_task_id(col);
        let (parent_id, project, position) = if let Some(id) = current_id {
            let task = self.task_ref(id).unwrap();
            (task.parent_id, task.project.clone(), InsertPosition::AfterSibling(id))
        } else {
            let project = self.active_projects.first().cloned().unwrap_or_else(|| "none".into());
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

    /// 'O' — create sibling before current card
    pub fn begin_insert_before(&mut self) {
        let col = self.focused_col;
        let current_id = match self.selected_task_id(col) {
            Some(id) => id,
            None => return,
        };
        let task = self.task_ref(current_id).unwrap();
        let parent_id = task.parent_id;
        let project = task.project.clone();

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

            // Move cursor to the newly created card
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

    pub fn begin_move_project(&mut self) {
        let col = self.focused_col;
        if let Some(id) = self.selected_task_id(col) {
            let root_id = self.root_task_id(id);
            let project = self.task_ref(root_id).map(|t| t.project.clone()).unwrap_or_default();
            self.move_state = Some(MoveState {
                task_id: root_id,
                target_input: project,
                suggestion_cursor: None,
                suggestion_query: None,
            });
            self.mode = Mode::Move;
        }
    }

    fn root_task_id(&self, id: Uuid) -> Uuid {
        let mut current = id;
        while let Some(parent_id) = self.task_ref(current).and_then(|t| t.parent_id) {
            current = parent_id;
        }
        current
    }

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

    // ── Filter ───────────────────────────────────────────────────────────────

    pub fn begin_filter(&mut self) {
        self.filter = Some(FilterState { input: String::new() });
        self.mode = Mode::Filter;
    }

    pub fn commit_filter(&mut self) {
        if let Some(fs) = self.filter.take() {
            if fs.input.is_empty() {
                self.active_projects.clear();
            } else {
                self.active_projects = self.all_projects()
                    .into_iter()
                    .filter(|p| !segment_boundary_matches(p, &fs.input).is_empty())
                    .collect();
            }
        }
        self.mode = Mode::Normal;
        for col in [Column::Todo, Column::Doing, Column::Done] {
            self.clamp_cursor(col);
        }
    }

    pub fn clear_filter(&mut self) {
        self.active_projects.clear();
        self.filter = None;
        self.mode = Mode::Normal;
    }

    pub fn all_projects(&self) -> Vec<String> {
        collect_projects(&self.tasks)
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

    pub fn move_suggestions(&self) -> Vec<String> {
        let ms = match self.move_state.as_ref() {
            Some(ms) => ms,
            None => return Vec::new(),
        };
        let query = ms.suggestion_query.as_deref().unwrap_or(&ms.target_input);
        self.all_projects()
            .into_iter()
            .filter(|p| p.to_lowercase().starts_with(&query.to_lowercase()))
            .collect()
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

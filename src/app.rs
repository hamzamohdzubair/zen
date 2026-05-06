use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

use crate::types::{Status, Task};

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Move,
    ProjectEdit,
    Help,
    BulkInsert,
    Visual,
    SnapBrowser,
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


#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    Tree,
    Board,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KanbanSort {
    /// Projects ordered by age of their oldest visible leaf task; within each project, DFS tree order.
    Age,
    /// Projects ordered by slot priority (1–9 then 0); within each project, DFS tree order.
    Project,
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
    pub cursor_pos: usize,
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
    pub kanban_sort: KanbanSort,
    /// Saved kanban filter state, restored when exiting planning mode.
    pub saved_slots: [bool; 10],
    pub saved_show_unc: bool,
    /// Last tree filter state, restored when entering tree from kanban via Tab/Backspace.
    pub last_tree_slots: [bool; 10],
    pub last_tree_show_unc: bool,
    /// A non-first-leaf task forced into the Todo kanban column when the user pressed
    /// Enter on it in tree view. Cleared when returning to tree.
    pub forced_todo_task_id: Option<Uuid>,
    pub mode: Mode,
    pub view_mode: ViewMode,
    pub focused_col: Column,
    pub cursor: [usize; 3],
    pub tui_scroll_offset: usize,
    pub insert: Option<InsertState>,
    pub edit: Option<EditState>,
    pub move_state: Option<MoveState>,
    pub project_edit: Option<ProjectEditState>,
    pub bulk_insert: Option<BulkInsertState>,
    pub status_message: Option<String>,
    pub last_d_press: Option<Instant>,
    pub last_g_press: Option<Instant>,
    pub collapsed: HashSet<Uuid>,
    pub undo_stack: Vec<Vec<Task>>,
    pub redo_stack: Vec<Vec<Task>>,
    /// Explicit ordering for the Doing kanban column, independent of tree structure.
    /// Empty means fall back to DFS order. Populated on first K/J swap.
    pub doing_order: Vec<Uuid>,
    /// Anchor task for visual (multi-select) mode. The selection spans from this task
    /// to the current cursor position in DFS row order.
    pub visual_anchor_id: Option<Uuid>,
    /// True while waiting for y/n confirmation before discarding unsaved insert/edit input.
    pub discard_confirm: bool,
    /// Which of the three flag pills (0-indexed) are currently toggled on.
    pub flag_active: [bool; 3],
    /// The flag index (0-indexed) that 'f' will apply. Set to the most recently activated flag.
    pub active_highlight: Option<usize>,
    /// True while waiting for Enter/Esc confirmation before clearing all highlights of a flag.
    pub flag_clear_confirm: bool,
    /// True while waiting for confirmation before archiving all Done tasks.
    pub archive_done_confirm: bool,
    /// State for the in-TUI snapshot browser popup.
    pub snap_popup: Option<crate::snapshots::SnapPopupState>,
}

impl App {
    pub fn new(tasks: Vec<Task>, projects: [Option<String>; 10]) -> Self {
        // Default tree view to first defined project; inbox if no projects exist yet.
        let (last_tree_slots, last_tree_show_unc) = match projects.iter().position(|p| p.is_some()) {
            Some(slot) => { let mut a = [false; 10]; a[slot] = true; (a, false) }
            None => ([false; 10], true),
        };
        Self {
            tasks,
            projects,
            active_slots: [true; 10],
            show_unc: true,
            kanban_sort: KanbanSort::Age,
            saved_slots: [true; 10],
            saved_show_unc: true,
            last_tree_slots,
            last_tree_show_unc,
            forced_todo_task_id: None,
            mode: Mode::Normal,
            view_mode: ViewMode::Tree,
            focused_col: Column::Todo,
            cursor: [0, 0, 0],
            tui_scroll_offset: 0,
            insert: None,
            edit: None,
            move_state: None,
            project_edit: None,
            bulk_insert: None,
            status_message: None,
            last_d_press: None,
            last_g_press: None,
            collapsed: HashSet::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            doing_order: Vec::new(),
            visual_anchor_id: None,
            discard_confirm: false,
            flag_active: [false; 3],
            active_highlight: None,
            flag_clear_confirm: false,
            archive_done_confirm: false,
            snap_popup: None,
        }
    }

    /// Enter planning (tree) mode for the project of the currently selected task,
    /// placing the tree cursor on that same task.
    pub fn enter_planning_for_selected(&mut self) {
        let Some(task_id) = self.selected_task_id(self.focused_col) else { return; };
        let Some((project, task_status)) = self.task_ref(task_id)
            .map(|t| (t.project.clone(), t.status.clone())) else { return; };

        self.saved_slots = self.active_slots;
        self.saved_show_unc = self.show_unc;

        self.active_slots = [false; 10];
        if let Some(slot) = self.slot_for_project(&project) {
            self.active_slots[slot] = true;
            self.show_unc = false;
        } else {
            self.show_unc = true;
        }

        self.view_mode = ViewMode::Tree;
        self.cursor = [0, 0, 0];
        self.tui_scroll_offset = 0;

        // Set focused_col and cursor to the task's actual column and position.
        let col = match task_status {
            Status::Todo => Column::Todo,
            Status::Doing => Column::Doing,
            Status::Done => Column::Done,
        };
        self.focused_col = col;
        if let Some(pos) = self.visible_tasks_for(col).iter().position(|t| t.id == task_id) {
            self.cursor[Self::col_index(col)] = pos;
        }
        self.clamp_all_cursors();
    }

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

    /// Exit planning (tree) mode, restoring the saved kanban filter state.
    pub fn exit_planning(&mut self) {
        self.last_tree_slots = self.active_slots;
        self.last_tree_show_unc = self.show_unc;
        self.active_slots = self.saved_slots;
        self.show_unc = self.saved_show_unc;
        self.view_mode = ViewMode::Board;
        self.clamp_all_cursors();
    }

    // ── Visual (multi-select) mode ───────────────────────────────────────────

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

    /// Enter kanban (board) mode for the currently selected task in tree view,
    /// restoring saved filter state and placing the board cursor on that task.
    /// If the selected task is a Todo leaf that wouldn't normally appear on the board,
    /// it is forced into the Todo column for this kanban session.
    pub fn enter_kanban_for_selected(&mut self) {
        let task_info = self.selected_task_id(self.focused_col)
            .and_then(|id| self.task_ref(id).map(|t| (id, t.status.clone())));

        // Save tree filter so Tab/Backspace in kanban can return here
        self.last_tree_slots = self.active_slots;
        self.last_tree_show_unc = self.show_unc;

        // Force a non-first-leaf Todo task into the kanban Todo column
        if let Some((id, Status::Todo)) = &task_info {
            let id = *id;
            if self.task_ref(id).map(|t| t.children.is_empty()).unwrap_or(false)
                && !self.would_appear_in_todo_board_normally(id)
            {
                self.forced_todo_task_id = Some(id);
            }
        }

        self.active_slots = self.saved_slots;
        self.show_unc = self.saved_show_unc;
        self.view_mode = ViewMode::Board;

        if let Some((id, status)) = task_info {
            let col = match status {
                Status::Todo => Column::Todo,
                Status::Doing => Column::Doing,
                Status::Done => Column::Done,
            };
            self.focused_col = col;
            if let Some(pos) = self.board_tasks_for(col).iter().position(|t| t.id == id) {
                self.cursor[Self::col_index(col)] = pos;
            }
        }

        self.clamp_all_cursors();
    }

    /// Enter tree mode restoring the last tree filter state (saved by exit_planning or
    /// enter_kanban_for_selected). Used by Tab/Backspace in kanban.
    /// If a forced Todo task was in play and has since moved to Doing, positions the
    /// tree cursor on it so it is treated as the current task.
    pub fn enter_planning_for_last_project(&mut self) {
        self.saved_slots = self.active_slots;
        self.saved_show_unc = self.show_unc;

        self.active_slots = self.last_tree_slots;
        self.show_unc = self.last_tree_show_unc;

        self.view_mode = ViewMode::Tree;
        self.cursor = [0, 0, 0];
        self.tui_scroll_offset = 0;

        let forced_id = self.forced_todo_task_id.take();
        if let Some(id) = forced_id {
            if let Some(task) = self.task_ref(id) {
                let col = match task.status {
                    Status::Todo => Column::Todo,
                    Status::Doing => Column::Doing,
                    Status::Done => Column::Done,
                };
                self.focused_col = col;
                if let Some(pos) = self.visible_tasks_for(col).iter().position(|t| t.id == id) {
                    self.cursor[Self::col_index(col)] = pos;
                }
            } else {
                self.focused_col = Column::Todo;
            }
        } else {
            self.focused_col = Column::Todo;
        }

        self.clamp_all_cursors();
    }

    /// Returns true if this Todo leaf task would appear in the Todo kanban column
    /// under normal (non-forced) conditions.
    fn would_appear_in_todo_board_normally(&self, task_id: Uuid) -> bool {
        let task = match self.task_ref(task_id) {
            Some(t) => t,
            None => return false,
        };
        if task.status != Status::Todo || !task.children.is_empty() {
            return false;
        }
        let project = task.project.clone();
        // Project is hidden when it already has a Doing leaf task
        if self.tasks.iter().any(|t| t.project == project && t.status == Status::Doing && t.children.is_empty()) {
            return false;
        }
        // Only the first leaf in DFS order is shown per project
        let dfs_pos = self.dfs_all_map();
        let first_id = self.tasks.iter()
            .filter(|t| t.project == project && t.status == Status::Todo && t.children.is_empty())
            .min_by_key(|t| dfs_pos.get(&t.id).copied().unwrap_or(usize::MAX))
            .map(|t| t.id);
        first_id == Some(task_id)
    }

    /// Cycle the active project in planning mode (exclusive single-project selection).
    pub fn cycle_project(&mut self, delta: i32) {
        // Build ordered list: None = INBOX (always present), Some(slot) = named project
        let mut items: Vec<Option<usize>> = Vec::new();
        items.push(None);
        for slot in 0..10usize {
            if self.projects[slot].is_some() {
                items.push(Some(slot));
            }
        }
        if items.is_empty() { return; }

        let current = if self.show_unc && self.active_slots.iter().all(|&a| !a) {
            items.iter().position(|i| i.is_none()).unwrap_or(0)
        } else if let Some(slot) = self.active_slots.iter().position(|&a| a) {
            items.iter().position(|i| *i == Some(slot)).unwrap_or(0)
        } else {
            0
        };

        let new_pos = (current as i32 + delta).rem_euclid(items.len() as i32) as usize;

        self.active_slots = [false; 10];
        match items[new_pos] {
            None => { self.show_unc = true; }
            Some(slot) => { self.show_unc = false; self.active_slots[slot] = true; }
        }
        self.cursor = [0, 0, 0];
        self.focused_col = Column::Todo;
        self.tui_scroll_offset = 0;
        self.clamp_all_cursors();
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

    pub fn board_tasks_for(&self, col: Column) -> Vec<&Task> {
        let mut tasks: Vec<&Task> = self.all_col_tasks(col)
            .into_iter()
            .filter(|t| t.children.is_empty())
            .collect();

        match col {
            Column::Done => {
                tasks.sort_by(|a, b| {
                    let ta = a.transitions.iter().filter(|tr| tr.to == Status::Done).last().map(|tr| tr.at).unwrap_or(a.created_at);
                    let tb = b.transitions.iter().filter(|tr| tr.to == Status::Done).last().map(|tr| tr.at).unwrap_or(b.created_at);
                    tb.cmp(&ta)
                });
            }
            Column::Doing => {
                let dfs_pos = self.dfs_all_map();
                tasks.sort_by_key(|t| dfs_pos.get(&t.id).copied().unwrap_or(usize::MAX));
                if !self.doing_order.is_empty() {
                    let ordered: Vec<Uuid> = self.doing_order.iter()
                        .filter(|&&id| tasks.iter().any(|t| t.id == id))
                        .copied()
                        .collect();
                    tasks.sort_by_key(|t| {
                        ordered.iter().position(|&id| id == t.id)
                            .unwrap_or(ordered.len() + dfs_pos.get(&t.id).copied().unwrap_or(usize::MAX))
                    });
                }
            }
            Column::Todo => {
                let dfs_pos = self.dfs_all_map();
                let project_key: HashMap<String, u64> = match self.kanban_sort {
                    KanbanSort::Age => {
                        let mut map: HashMap<String, u64> = HashMap::new();
                        for t in &tasks {
                            let age = t.created_at.timestamp_millis() as u64;
                            map.entry(t.project.clone()).and_modify(|v| *v = (*v).min(age)).or_insert(age);
                        }
                        map
                    }
                    KanbanSort::Project => tasks.iter()
                        .map(|t| {
                            let key = self.slot_for_project(&t.project).map(|s| s as u64).unwrap_or(u64::MAX);
                            (t.project.clone(), key)
                        })
                        .collect(),
                };
                tasks.sort_by_key(|t| {
                    let pk = project_key.get(&t.project).copied().unwrap_or(u64::MAX);
                    let tree_key = dfs_pos.get(&t.id).copied().unwrap_or(usize::MAX) as u64;
                    (pk, tree_key)
                });
                // Always show only first leaf per project; skip projects that already have a Doing task.
                // Exception: a forced task is always shown and replaces the normal first leaf for its project.
                let doing_projects: HashSet<&str> = self.tasks.iter()
                    .filter(|t| t.status == Status::Doing && t.children.is_empty())
                    .map(|t| t.project.as_str())
                    .collect();
                let mut seen: HashSet<String> = HashSet::new();
                let forced_id = self.forced_todo_task_id;
                // Pre-mark forced task's project so its natural first leaf is skipped
                if let Some(fid) = forced_id {
                    if let Some(ft) = self.task_ref(fid) {
                        if ft.status == Status::Todo && ft.children.is_empty() {
                            seen.insert(ft.project.clone());
                        }
                    }
                }
                tasks.retain(|t| {
                    if forced_id == Some(t.id) {
                        return true; // always keep the forced task regardless of project rules
                    }
                    !doing_projects.contains(t.project.as_str()) && seen.insert(t.project.clone())
                });
            }
        }
        tasks
    }

    /// Returns all tasks for a column in tree DFS order, ignoring project visibility filters.
    fn all_col_tasks(&self, col: Column) -> Vec<&Task> {
        let status = col.status();
        let in_col: HashSet<Uuid> = self.tasks.iter()
            .filter(|t| t.status == status)
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

    /// Returns DFS preorder position for every task, ignoring project visibility filters.
    fn dfs_all_map(&self) -> HashMap<Uuid, usize> {
        let tasks_by_id: HashMap<Uuid, &Task> = self.tasks.iter().map(|t| (t.id, t)).collect();
        let mut result: Vec<Uuid> = Vec::new();

        fn visit(id: Uuid, tasks_by_id: &HashMap<Uuid, &Task>, result: &mut Vec<Uuid>) {
            result.push(id);
            if let Some(task) = tasks_by_id.get(&id) {
                for &cid in &task.children {
                    if tasks_by_id.contains_key(&cid) {
                        visit(cid, tasks_by_id, result);
                    }
                }
            }
        }

        for task in &self.tasks {
            if task.parent_id.is_none() {
                visit(task.id, &tasks_by_id, &mut result);
            }
        }

        result.into_iter().enumerate().map(|(i, id)| (id, i)).collect()
    }

    /// Enter tree view for the given project slot, with cursor on the first kanban-visible task.
    pub fn enter_planning_for_slot_key(&mut self, slot: usize) {
        let Some(project_name) = self.projects[slot].clone() else { return; };

        let first_task_id = self.board_tasks_for(Column::Todo)
            .into_iter()
            .find(|t| t.project == project_name)
            .map(|t| t.id);

        self.saved_slots = self.active_slots;
        self.saved_show_unc = self.show_unc;
        self.active_slots = [false; 10];
        self.active_slots[slot] = true;
        self.show_unc = false;

        self.view_mode = ViewMode::Tree;
        self.focused_col = Column::Todo;
        self.cursor = [0, 0, 0];
        self.tui_scroll_offset = 0;

        if let Some(task_id) = first_task_id {
            if let Some(pos) = self.visible_tasks_for(Column::Todo).iter().position(|t| t.id == task_id) {
                self.cursor[Self::col_index(Column::Todo)] = pos;
            }
        }

        self.clamp_all_cursors();
    }

    /// Enter tree view for INBOX, with cursor on the first kanban-visible INBOX task.
    pub fn enter_planning_for_inbox_tree(&mut self) {
        let first_task_id = self.board_tasks_for(Column::Todo)
            .into_iter()
            .find(|t| self.is_unc(t))
            .map(|t| t.id);

        self.saved_slots = self.active_slots;
        self.saved_show_unc = self.show_unc;
        self.active_slots = [false; 10];
        self.show_unc = true;

        self.view_mode = ViewMode::Tree;
        self.focused_col = Column::Todo;
        self.cursor = [0, 0, 0];
        self.tui_scroll_offset = 0;

        if let Some(task_id) = first_task_id {
            if let Some(pos) = self.visible_tasks_for(Column::Todo).iter().position(|t| t.id == task_id) {
                self.cursor[Self::col_index(Column::Todo)] = pos;
            }
        }

        self.clamp_all_cursors();
    }

    pub fn cycle_sort(&mut self) {
        self.kanban_sort = match self.kanban_sort {
            KanbanSort::Age => KanbanSort::Project,
            KanbanSort::Project => KanbanSort::Age,
        };
        self.clamp_all_cursors();
    }


    /// Sync doing_order to match the current set of Doing leaf tasks, preserving
    /// any existing user-defined order and appending new tasks in DFS order.
    fn sync_doing_order(&mut self) {
        let dfs_pos = self.dfs_all_map();
        let mut current: Vec<Uuid> = self.tasks.iter()
            .filter(|t| t.status == Status::Doing && t.children.is_empty())
            .map(|t| t.id)
            .collect();
        current.sort_by_key(|id| dfs_pos.get(id).copied().unwrap_or(usize::MAX));

        self.doing_order.retain(|id| current.contains(id));
        for id in &current {
            if !self.doing_order.contains(id) {
                self.doing_order.push(*id);
            }
        }
    }

    /// Move the selected Doing task up (delta=-1) or down (delta=1) in the Doing column.
    pub fn kanban_doing_swap(&mut self, delta: i32) {
        let col = Column::Doing;
        let cur = self.cursor[Self::col_index(col)];
        let tasks = self.board_tasks_for(col);
        let len = tasks.len();
        if len == 0 { return; }
        let other = if delta < 0 {
            if cur == 0 { return; }
            cur - 1
        } else {
            if cur + 1 >= len { return; }
            cur + 1
        };
        let id_a = tasks[cur].id;
        let id_b = tasks[other].id;
        drop(tasks);

        self.sync_doing_order();
        if let (Some(pos_a), Some(pos_b)) = (
            self.doing_order.iter().position(|&id| id == id_a),
            self.doing_order.iter().position(|&id| id == id_b),
        ) {
            self.doing_order.swap(pos_a, pos_b);
        }
        self.cursor[Self::col_index(col)] = other;
    }

    fn nav_tasks_for(&self, col: Column) -> Vec<&Task> {
        match self.view_mode {
            ViewMode::Board => self.board_tasks_for(col),
            ViewMode::Tree => self.visible_tasks_for(col),
        }
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
        let len = self.nav_tasks_for(col).len();
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
        let tasks = self.nav_tasks_for(col);
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
        let len = self.nav_tasks_for(col).len();
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

    /// Propagate derived status upward from `child_id`'s parent all the way to the root.
    /// Each ancestor adopts the most-urgent status of its direct children:
    /// Todo if any child is Todo, else Doing if any child is Doing, else Done.
    fn propagate_status_up(&mut self, child_id: Uuid) {
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

    pub fn move_selected_right(&mut self) {
        let dest = self.focused_col.next();
        self.move_selected_to(dest);
    }

    pub fn move_selected_left(&mut self) {
        let dest = self.focused_col.prev();
        self.move_selected_to(dest);
    }

    fn move_selected_to(&mut self, dest: Column) {
        let col = self.focused_col;
        if dest == col { return; }
        let Some(id) = self.selected_task_id(col) else { return; };
        let new_status = dest.status();
        if let Some(task) = self.task_mut(id) {
            task.transition_to(new_status);
        }
        self.propagate_status_up(id);
        self.clamp_cursor(col);
        if let Some(pos) = self.nav_tasks_for(dest).iter().position(|t| t.id == id) {
            self.cursor[Self::col_index(dest)] = pos;
        }
        self.focused_col = dest;
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

    fn cascade_status_down(&mut self, id: Uuid, status: Status) {
        let children: Vec<Uuid> = self.task_ref(id).map(|t| t.children.clone()).unwrap_or_default();
        for cid in children {
            if let Some(task) = self.task_mut(cid) {
                task.transition_to(status);
            }
            self.cascade_status_down(cid, status);
        }
    }

    fn tree_set_status(&mut self, id: Uuid, new_status: Status) {
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
    fn dfs_visible_ids(&self) -> Vec<Uuid> {
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

    fn is_descendant_of(&self, id: Uuid, ancestor_id: Uuid) -> bool {
        let mut current = id;
        loop {
            match self.task_ref(current).and_then(|t| t.parent_id) {
                None => return false,
                Some(pid) if pid == ancestor_id => return true,
                Some(pid) => current = pid,
            }
        }
    }

    /// Move selected task one visual row up in the DFS tree, reparenting if needed.
    pub fn tree_swap_up(&mut self) {
        self.push_undo();
        let Some(task_id) = self.selected_task_id(self.focused_col) else { return; };
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

        let col = self.focused_col;
        let visible = self.visible_tasks_for(col);
        if let Some(pos) = visible.iter().position(|t| t.id == task_id) {
            self.cursor[Self::col_index(col)] = pos;
        }
    }

    /// Move selected task one visual row down in the DFS tree (past its full subtree), reparenting if needed.
    pub fn tree_swap_down(&mut self) {
        self.push_undo();
        let dfs = self.dfs_visible_ids();
        let Some(task_id) = self.selected_task_id(self.focused_col) else { return; };
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

        let col = self.focused_col;
        let visible = self.visible_tasks_for(col);
        if let Some(pos) = visible.iter().position(|t| t.id == task_id) {
            self.cursor[Self::col_index(col)] = pos;
        }
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
        self.push_undo();
        let col = self.focused_col;
        let visible: Vec<Uuid> = self.nav_tasks_for(col).iter().map(|t| t.id).collect();
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
        self.push_undo();
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

    // ── Undo ─────────────────────────────────────────────────────────────────

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

    // ── Delete ───────────────────────────────────────────────────────────────

    /// Delete a task by ID: orphans its children and removes it from the tree.
    pub fn delete_task(&mut self, id: Uuid) {
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
            if let Some(id) = self.selected_task_id(self.focused_col) {
                self.delete_task(id);
            }
            true
        } else {
            self.last_d_press = Some(now);
            self.status_message = Some("d again to delete".into());
            false
        }
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
        let status = Status::Todo;
        self.insert = Some(InsertState {
            title: String::new(),
            project,
            parent_id,
            status,
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

        let status = Status::Todo;
        self.insert = Some(InsertState {
            title: String::new(),
            project,
            parent_id,
            status,
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
            // Propagate up so a newly added Todo task un-Dones any completed ancestors
            self.propagate_status_up(task_id);
        }
        self.mode = Mode::Normal;
    }

    // ── Edit ─────────────────────────────────────────────────────────────────

    pub fn begin_edit(&mut self, cursor_at_end: bool) {
        let col = self.focused_col;
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
        let col = self.focused_col;
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

    // ── Project filter ────────────────────────────────────────────────────────

    /// In tree mode: exclusively switch to a single project by slot number.
    pub fn select_project_slot(&mut self, slot: usize) {
        if self.projects[slot].is_none() { return; }
        self.active_slots = [false; 10];
        self.active_slots[slot] = true;
        self.show_unc = false;
        self.cursor = [0, 0, 0];
        self.focused_col = Column::Todo;
        self.tui_scroll_offset = 0;
        self.clamp_all_cursors();
    }

    /// In tree mode: exclusively switch to INBOX.
    pub fn select_inbox(&mut self) {
        self.active_slots = [false; 10];
        self.show_unc = true;
        self.cursor = [0, 0, 0];
        self.focused_col = Column::Todo;
        self.tui_scroll_offset = 0;
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

    // ── Flag highlights ──────────────────────────────────────────────────────

    pub fn toggle_flag_pill(&mut self, idx: usize) {
        self.flag_active[idx] = !self.flag_active[idx];
        if self.flag_active[idx] {
            self.active_highlight = Some(idx);
        } else {
            self.active_highlight = self.flag_active.iter().position(|&x| x);
        }
    }

    /// Toggle the active flag on the currently selected task. Returns true if a change was made.
    pub fn flag_selected_task(&mut self) -> bool {
        let Some(flag_idx) = self.active_highlight else { return false; };
        let col = self.focused_col;
        let Some(id) = self.selected_task_id(col) else { return false; };
        if let Some(task) = self.task_mut(id) {
            task.flags ^= 1u8 << flag_idx;
            true
        } else {
            false
        }
    }

    pub fn begin_flag_clear(&mut self) {
        let active: Vec<usize> = (0..3).filter(|&i| self.flag_active[i]).collect();
        if active.is_empty() { return; }
        let nums: String = active.iter().map(|&i| (i + 1).to_string()).collect::<Vec<_>>().join(", ");
        self.flag_clear_confirm = true;
        self.status_message = Some(format!(
            "Clear flag {} highlights? (Enter to confirm, Esc to cancel)", nums
        ));
    }

    pub fn confirm_flag_clear(&mut self) {
        for i in 0..3 {
            if self.flag_active[i] {
                let mask = !(1u8 << i);
                for task in &mut self.tasks {
                    task.flags &= mask;
                }
            }
        }
        self.flag_clear_confirm = false;
        self.status_message = None;
    }

    pub fn cancel_flag_clear(&mut self) {
        self.flag_clear_confirm = false;
        self.status_message = None;
    }

    // ── Snapshots ─────────────────────────────────────────────────────────────

    pub fn to_snapshot(&self) -> crate::snapshots::Snapshot {
        crate::snapshots::Snapshot {
            taken_at: chrono::Utc::now(),
            tasks: self.tasks.clone(),
            projects: self.projects.clone(),
            active_slots: self.active_slots,
            show_unc: self.show_unc,
            collapsed: self.collapsed.iter().copied().collect(),
        }
    }

    pub fn open_snap_browser(&mut self) {
        self.snap_popup = Some(crate::snapshots::SnapPopupState::load());
        self.mode = Mode::SnapBrowser;
    }

    pub fn close_snap_browser(&mut self) {
        self.snap_popup = None;
        self.mode = Mode::Normal;
    }

    // ── Archive Done ──────────────────────────────────────────────────────────

    pub fn begin_archive_done(&mut self) {
        let project_label = if self.show_unc && self.active_slots.iter().all(|&a| !a) {
            "INBOX".to_string()
        } else {
            self.active_slots.iter().enumerate()
                .find(|&(_, &on)| on)
                .and_then(|(i, _)| self.projects[i].clone())
                .unwrap_or_else(|| "current project".to_string())
        };
        self.archive_done_confirm = true;
        self.status_message = Some(format!(
            "Archive Done tasks in {}? (Enter to confirm, Esc to cancel)",
            project_label
        ));
    }

    pub fn cancel_archive_done(&mut self) {
        self.archive_done_confirm = false;
        self.status_message = None;
    }

    /// Returns Done tasks in the currently visible project whose entire subtree is also Done.
    /// Scoped to the active project/inbox so Ctrl+R never touches other projects.
    pub fn collect_done_for_archive(&self) -> Vec<Task> {
        let tasks_by_id: HashMap<Uuid, &Task> =
            self.tasks.iter().map(|t| (t.id, t)).collect();

        fn fully_done(id: Uuid, tasks_by_id: &HashMap<Uuid, &Task>) -> bool {
            match tasks_by_id.get(&id) {
                Some(t) => {
                    t.status == Status::Done
                        && t.children.iter().all(|&cid| fully_done(cid, tasks_by_id))
                }
                None => true,
            }
        }

        self.tasks
            .iter()
            .filter(|t| self.task_visible(t) && fully_done(t.id, &tasks_by_id))
            .cloned()
            .collect()
    }

    /// Removes the given task IDs from active state after they have been written
    /// to the archive. Cleans up children lists and orphaned parent refs.
    pub fn remove_archived_tasks(&mut self, archived_ids: &HashSet<Uuid>) {
        if archived_ids.is_empty() {
            return;
        }
        self.tasks.retain(|t| !archived_ids.contains(&t.id));
        for task in &mut self.tasks {
            task.children.retain(|cid| !archived_ids.contains(cid));
            if let Some(pid) = task.parent_id {
                if archived_ids.contains(&pid) {
                    task.parent_id = None;
                }
            }
        }
        self.doing_order.retain(|id| !archived_ids.contains(id));
        self.collapsed.retain(|id| !archived_ids.contains(id));
        self.clamp_all_cursors();
        self.status_message = Some(format!(
            "Archived {} task{}",
            archived_ids.len(),
            if archived_ids.len() == 1 { "" } else { "s" }
        ));
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

    #[test]
    fn parent_demotes_when_child_returns_from_done_with_done_sibling() {
        // Regression: if a parent was promoted to Done after its last Doing child
        // moved to Done, and that child is moved back while a Done sibling exists,
        // the parent must demote to reflect the child's new status.
        let mut parent = task("parent", "", Status::Todo);
        let mut c1 = task("c1", "", Status::Todo);
        let mut c2 = task("c2", "", Status::Done); // already Done before parent promoted
        let pid = parent.id;
        let c1_id = c1.id;
        let c2_id = c2.id;
        c1.parent_id = Some(pid);
        c2.parent_id = Some(pid);
        parent.children = vec![c1_id, c2_id];
        let mut app = App::new(vec![parent, c1, c2], no_projects());
        // Use Board view so only leaf tasks appear in each column (mirrors kanban).
        app.view_mode = ViewMode::Board;

        // Move c1 to Done (Todo → Doing → Done), which auto-promotes parent to Done.
        app.focused_col = Column::Todo;
        app.move_selected_right(); // c1: Todo → Doing, parent follows
        app.focused_col = Column::Doing;
        app.move_selected_right(); // c1: Doing → Done, parent follows → Done
        assert_eq!(app.task_ref(pid).unwrap().status, Status::Done);

        // Move c1 back to Doing — parent must demote even though c2 is still Done.
        app.focused_col = Column::Done;
        app.move_selected_left(); // c1: Done → Doing
        assert_eq!(app.task_ref(c1_id).unwrap().status, Status::Doing);
        assert_eq!(app.task_ref(c2_id).unwrap().status, Status::Done);
        assert_eq!(app.task_ref(pid).unwrap().status, Status::Doing,
            "parent should be Doing when c1 is Doing even though c2 is Done");
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

    // ── delete_task ─────────────────────────────────────────────────────────────

    #[test]
    fn delete_task_removes_task_and_orphans_children() {
        let mut parent = task("parent", "", Status::Todo);
        let mut child = task("child", "", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![parent, child], no_projects());
        app.delete_task(parent_id);
        assert!(app.task_ref(parent_id).is_none());
        assert!(app.task_ref(child_id).unwrap().parent_id.is_none());
    }

    // ── enable_all / disable_all ────────────────────────────────────────────────

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

    #[test]
    fn commit_insert_propagates_todo_up_through_done_ancestors() {
        // Bug: adding a new child to a completed (Done) family left parents as Done.
        let mut parent = task("parent", "", Status::Done);
        let done_child = task("done_child", "", Status::Done);
        let parent_id = parent.id;
        let _done_child_id = done_child.id;
        let mut done_child2 = done_child.clone();
        done_child2.parent_id = Some(parent_id);
        parent.children.push(done_child2.id);
        let mut app = App::new(vec![parent, done_child2], no_projects());
        app.focused_col = Column::Done;
        // Cursor on done_child inside the Done family
        app.cursor[App::col_index(Column::Done)] = 0;
        app.begin_insert_after();
        app.insert.as_mut().unwrap().title = "new subtask".into();
        // Ensure the insert will be a child of parent_id
        app.insert.as_mut().unwrap().parent_id = Some(parent_id);
        app.commit_insert();
        // Parent must now reflect the new Todo child and become Todo itself
        assert_eq!(app.task_ref(parent_id).unwrap().status, Status::Todo,
            "parent should demote to Todo when a new Todo child is added to a Done family");
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

    // ── unc_doable_count ──────────────────────────────────────────────────────

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

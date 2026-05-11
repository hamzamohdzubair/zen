use std::collections::HashSet;
use std::time::Instant;
use uuid::Uuid;

use crate::types::{Layer, Status, Task};

mod layers;
mod folds;
mod visual;
mod status;
mod tree;
mod editing;
mod meta;
mod search;

/// Waiting-for-confirmation variants.
#[derive(Debug, Clone)]
pub enum PendingConfirm {
    /// backspace on a clocked task — confirm to surface snoozed children early.
    ExpireSnooze(Uuid),
}

pub enum ArchiveView {
    Calendar,
    Day,
}

pub struct ArchiveBrowserState {
    pub view: ArchiveView,
    pub year: i32,
    pub month: u32,
    pub selected_day: u32,
    pub available_dates: std::collections::HashSet<chrono::NaiveDate>,
    pub day_tasks: Vec<crate::archive::ArchiveTask>,
    pub day_scroll: usize,
    /// Active date-jump input (YYYY-MM-DD). Some while the user is typing.
    pub date_jump_input: Option<String>,
}

pub struct SearchState {
    pub query: String,
    /// Task IDs in DFS order (ignoring collapse) that contain the query.
    pub matches: Vec<Uuid>,
    /// Index into `matches` for the currently highlighted match.
    pub match_idx: usize,
    /// Snapshot of `collapsed` taken when search began. Restored on every jump
    /// so only the current match's ancestors stay unfolded.
    pub original_collapsed: std::collections::HashSet<Uuid>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Help,
    BulkInsert,
    Visual,
    /// Waiting for a snooze duration string before hiding the selected task.
    SnoozeInput,
    /// Typing a search query.
    Search,
    /// Calendar-based archive browser.
    ArchiveBrowser,
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
}

#[derive(Debug, Clone)]
pub enum InsertPosition {
    AtBeginning,
    AfterSibling(Uuid),
    AfterParent(Uuid),
}

pub struct InsertState {
    pub title: String,
    pub parent_id: Option<Uuid>,
    pub status: Status,
    pub position: InsertPosition,
}

pub struct EditState {
    pub task_id: Uuid,
    pub title: String,
    pub cursor_pos: usize,
}

pub struct SnoozeInputState {
    pub task_id: Uuid,
    /// Raw text typed so far (e.g. "3d", "2h", "1w").
    pub input: String,
}

pub struct App {
    pub tasks: Vec<Task>,
    /// Active search state. Some while a search query is live (typing or post-Enter navigation).
    pub search: Option<SearchState>,
    pub mode: Mode,
    pub focused_col: Column,
    pub cursor: [usize; 3],
    pub tui_scroll_offset: usize,
    pub insert: Option<InsertState>,
    pub edit: Option<EditState>,
    pub bulk_insert: Option<BulkInsertState>,
    pub snooze_input: Option<SnoozeInputState>,
    pub pending_confirm: Option<PendingConfirm>,
    pub archive_browser: Option<ArchiveBrowserState>,
    pub status_message: Option<String>,
    pub last_d_press: Option<Instant>,
    pub last_g_press: Option<Instant>,
    pub last_z_press: Option<Instant>,
    pub collapsed: HashSet<Uuid>,
    pub undo_stack: Vec<Vec<Task>>,
    pub redo_stack: Vec<Vec<Task>>,
    /// Anchor task for visual (multi-select) mode.
    pub visual_anchor_id: Option<Uuid>,
    /// True while waiting for y/n confirmation before discarding unsaved insert/edit input.
    pub discard_confirm: bool,
    /// Which of the three flag pills (0-indexed) are currently toggled on.
    pub flag_active: [bool; 3],
    /// The flag index (0-indexed) that 'f' will apply.
    pub active_highlight: Option<usize>,
    /// True while waiting for Enter/Esc confirmation before clearing all highlights of a flag.
    pub flag_clear_confirm: bool,
}

impl App {
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks,
            search: None,
            mode: Mode::Normal,
            focused_col: Column::Todo,
            cursor: [0, 0, 0],
            tui_scroll_offset: 0,
            insert: None,
            edit: None,
            bulk_insert: None,
            snooze_input: None,
            pending_confirm: None,
            archive_browser: None,
            status_message: None,
            last_d_press: None,
            last_g_press: None,
            last_z_press: None,
            collapsed: HashSet::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            visual_anchor_id: None,
            discard_confirm: false,
            flag_active: [false; 3],
            active_highlight: None,
            flag_clear_confirm: false,
        }
    }

    pub fn col_index(col: Column) -> usize {
        match col {
            Column::Todo => 0,
            Column::Doing => 1,
            Column::Done => 2,
        }
    }

    pub fn task_visible(&self, task: &Task) -> bool {
        matches!(task.layer, Layer::Active)
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

    /// Navigate the cursor to the task with the given id, updating focused_col and cursor[].
    pub fn navigate_to_id(&mut self, id: Uuid) {
        if let Some(task) = self.task_ref(id) {
            let col = match task.status {
                Status::Todo  => Column::Todo,
                Status::Doing => Column::Doing,
                Status::Done  => Column::Done,
            };
            self.focused_col = col;
            let tasks = self.visible_tasks_for(col);
            if let Some(pos) = tasks.iter().position(|t| t.id == id) {
                self.cursor[Self::col_index(col)] = pos;
            }
        }
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Status, Task};

    fn task(title: &str, status: Status) -> Task {
        Task::new(title.into(), status)
    }

    fn empty_app() -> App { App::new(vec![]) }

    // ── layer visibility ────────────────────────────────────────────────────────

    #[test]
    fn task_visible_active_task_shown() {
        let app = empty_app();
        let t = task("x", Status::Todo);
        assert!(app.task_visible(&t)); // default layer is Active
    }

    #[test]
    fn task_visible_snoozed_task_hidden() {
        let app = empty_app();
        let mut t = task("x", Status::Todo);
        t.layer = Layer::Snoozed { expires_at: i64::MAX };
        assert!(!app.task_visible(&t));
    }

    // ── layer operations ─────────────────────────────────────────────────────

    #[test]
    fn snooze_task_moves_to_snoozed() {
        let t = task("x", Status::Todo);
        let id = t.id;
        let expires = chrono::Utc::now().timestamp() + 3600;
        let mut app = App::new(vec![t]);
        app.snooze_task(id, expires);
        assert!(matches!(app.task_ref(id).unwrap().layer, Layer::Snoozed { .. }));
    }

    #[test]
    fn unsnooze_task_restores_to_active() {
        let mut t = task("x", Status::Todo);
        t.layer = Layer::Snoozed { expires_at: i64::MAX };
        let id = t.id;
        let mut app = App::new(vec![t]);
        app.unsnooze_task(id);
        assert!(matches!(app.task_ref(id).unwrap().layer, Layer::Active));
    }

    #[test]
    fn parse_duration_handles_hours_days_weeks() {
        let base = chrono::Utc::now().timestamp();
        let h = App::parse_duration_to_expiry("2h").unwrap();
        assert!((h - base - 7200).abs() < 5);
        let d = App::parse_duration_to_expiry("3d").unwrap();
        assert!((d - base - 3 * 86400).abs() < 5);
        let w = App::parse_duration_to_expiry("1w").unwrap();
        assert!((w - base - 7 * 86400).abs() < 5);
    }

    #[test]
    fn parse_duration_rejects_invalid() {
        assert!(App::parse_duration_to_expiry("").is_none());
        assert!(App::parse_duration_to_expiry("abc").is_none());
        assert!(App::parse_duration_to_expiry("0d").is_none());
    }

    // ── status toggles ────────────────────────────────────────────────────────

    #[test]
    fn tree_toggle_doing_sets_and_clears() {
        let t = task("x", Status::Todo);
        let id = t.id;
        let mut app = App::new(vec![t]);
        app.tree_toggle_doing();
        assert_eq!(app.task_ref(id).unwrap().status, Status::Doing);
        app.tree_toggle_doing();
        assert_eq!(app.task_ref(id).unwrap().status, Status::Todo);
    }

    #[test]
    fn tree_toggle_done_sets_and_clears() {
        let t = task("x", Status::Todo);
        let id = t.id;
        let mut app = App::new(vec![t]);
        app.tree_toggle_done();
        assert_eq!(app.task_ref(id).unwrap().status, Status::Done);
        app.tree_toggle_done();
        assert_eq!(app.task_ref(id).unwrap().status, Status::Todo);
    }

    #[test]
    fn done_child_auto_promotes_parent() {
        let mut parent = task("parent", Status::Todo);
        let mut child = task("child", Status::Todo);
        let pid = parent.id;
        let cid = child.id;
        child.parent_id = Some(pid);
        parent.children.push(cid);
        let mut app = App::new(vec![parent, child]);
        app.cursor[App::col_index(Column::Todo)] = 1;
        app.tree_toggle_done();
        assert_eq!(app.task_ref(cid).unwrap().status, Status::Done);
        assert_eq!(app.task_ref(pid).unwrap().status, Status::Done);
    }

    // ── make_child / make_root ────────────────────────────────────────────────

    #[test]
    fn make_child_increments_depth_by_one_when_above_is_deeper() {
        let root1 = task("root1", Status::Todo);
        let mut child_of_root1 = task("child_of_root1", Status::Todo);
        let root2 = task("root2", Status::Todo);
        let root1_id = root1.id;
        let child_id = child_of_root1.id;
        let root2_id = root2.id;
        child_of_root1.parent_id = Some(root1_id);
        let mut app = App::new(vec![root1, child_of_root1, root2]);
        app.task_mut(root1_id).unwrap().children.push(child_id);
        app.focused_col = Column::Todo;
        app.cursor[0] = 2;
        app.make_child();
        assert_eq!(app.task_ref(root2_id).unwrap().parent_id, Some(root1_id));
        assert!(app.task_ref(root1_id).unwrap().children.contains(&root2_id));
        assert!(!app.task_ref(child_id).unwrap().children.contains(&root2_id));
    }

    #[test]
    fn make_child_links_parent_and_child() {
        let parent = task("parent", Status::Todo);
        let child = task("child", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        let mut app = App::new(vec![parent, child]);
        app.focused_col = Column::Todo;
        app.cursor[0] = 1;
        app.make_child();
        assert_eq!(app.task_ref(child_id).unwrap().parent_id, Some(parent_id));
        assert!(app.task_ref(parent_id).unwrap().children.contains(&child_id));
    }

    #[test]
    fn make_root_promotes_depth1_to_root() {
        let mut parent = task("parent", Status::Todo);
        let mut child = task("child", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![parent, child]);
        app.focused_col = Column::Todo;
        app.cursor[0] = 1;
        app.make_root();
        assert!(app.task_ref(child_id).unwrap().parent_id.is_none());
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&child_id));
    }

    #[test]
    fn make_root_decrements_depth_by_one_not_to_root() {
        let mut grandparent = task("grandparent", Status::Todo);
        let mut parent = task("parent", Status::Todo);
        let mut child = task("child", Status::Todo);
        let gp_id = grandparent.id;
        let parent_id = parent.id;
        let child_id = child.id;
        parent.parent_id = Some(gp_id);
        grandparent.children.push(parent_id);
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![grandparent, parent, child]);
        app.focused_col = Column::Todo;
        app.cursor[0] = 2;
        app.make_root();
        assert_eq!(app.task_ref(child_id).unwrap().parent_id, Some(gp_id));
        assert!(app.task_ref(gp_id).unwrap().children.contains(&child_id));
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&child_id));
    }

    #[test]
    fn make_root_carries_children_with_promoted_task() {
        // grandparent -> parent -> task -> [child_a, child_b]
        // Promoting `task` should move it under grandparent while keeping
        // child_a and child_b under task (not stranded under parent).
        let mut grandparent = task("grandparent", Status::Todo);
        let mut parent = task("parent", Status::Todo);
        let mut t = task("task", Status::Todo);
        let mut child_a = task("child_a", Status::Todo);
        let mut child_b = task("child_b", Status::Todo);
        let gp_id = grandparent.id;
        let parent_id = parent.id;
        let task_id = t.id;
        let child_a_id = child_a.id;
        let child_b_id = child_b.id;
        parent.parent_id = Some(gp_id);
        grandparent.children.push(parent_id);
        t.parent_id = Some(parent_id);
        parent.children.push(task_id);
        child_a.parent_id = Some(task_id);
        child_b.parent_id = Some(task_id);
        t.children.push(child_a_id);
        t.children.push(child_b_id);
        let mut app = App::new(vec![grandparent, parent, t, child_a, child_b]);
        app.focused_col = Column::Todo;
        app.cursor[0] = 2; // task is at DFS index 2
        app.make_root();
        // task promoted to grandparent level
        assert_eq!(app.task_ref(task_id).unwrap().parent_id, Some(gp_id));
        assert!(app.task_ref(gp_id).unwrap().children.contains(&task_id));
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&task_id));
        // children stay under task, not stranded under parent
        assert!(app.task_ref(task_id).unwrap().children.contains(&child_a_id));
        assert!(app.task_ref(task_id).unwrap().children.contains(&child_b_id));
        assert_eq!(app.task_ref(child_a_id).unwrap().parent_id, Some(task_id));
        assert_eq!(app.task_ref(child_b_id).unwrap().parent_id, Some(task_id));
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&child_a_id));
        assert!(!app.task_ref(parent_id).unwrap().children.contains(&child_b_id));
    }

    // ── delete_task ─────────────────────────────────────────────────────────────

    #[test]
    fn delete_task_cascades_to_children() {
        let mut parent = task("parent", Status::Todo);
        let mut child = task("child", Status::Todo);
        let parent_id = parent.id;
        let child_id = child.id;
        child.parent_id = Some(parent_id);
        parent.children.push(child_id);
        let mut app = App::new(vec![parent, child]);
        app.delete_task(parent_id);
        assert!(app.task_ref(parent_id).is_none());
        assert!(app.task_ref(child_id).is_none());
    }

    #[test]
    fn delete_task_cascades_deeply() {
        let mut parent = task("parent", Status::Done);
        let mut child = task("child", Status::Done);
        let mut grandchild = task("grandchild", Status::Done);
        let pid = parent.id;
        let cid = child.id;
        let gcid = grandchild.id;
        grandchild.parent_id = Some(cid);
        child.children.push(gcid);
        child.parent_id = Some(pid);
        parent.children.push(cid);
        let mut app = App::new(vec![parent, child, grandchild]);
        app.delete_task(pid);
        assert!(app.task_ref(pid).is_none());
        assert!(app.task_ref(cid).is_none());
        assert!(app.task_ref(gcid).is_none());
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
        app.commit_insert();
        assert!(app.tasks.is_empty());
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn commit_insert_child_of_doing_inherits_doing_and_demotes_parent() {
        let parent = task("parent", Status::Doing);
        let parent_id = parent.id;
        let mut app = App::new(vec![parent]);
        app.focused_col = Column::Doing;
        app.cursor[App::col_index(Column::Doing)] = 0;
        app.begin_insert_after();
        app.insert.as_mut().unwrap().title = "subtask".into();
        app.insert.as_mut().unwrap().parent_id = Some(parent_id);
        app.commit_insert();
        let new_task = app.tasks.iter().find(|t| t.title == "subtask").unwrap();
        assert_eq!(new_task.status, Status::Doing, "child should inherit Doing");
        assert_eq!(app.task_ref(parent_id).unwrap().status, Status::Todo, "parent should demote to Todo");
    }

    #[test]
    fn commit_insert_propagates_todo_up_through_done_ancestors() {
        let mut parent = task("parent", Status::Done);
        let done_child = task("done_child", Status::Done);
        let parent_id = parent.id;
        let mut done_child2 = done_child.clone();
        done_child2.parent_id = Some(parent_id);
        parent.children.push(done_child2.id);
        let mut app = App::new(vec![parent, done_child2]);
        app.focused_col = Column::Done;
        app.cursor[App::col_index(Column::Done)] = 0;
        app.begin_insert_after();
        app.insert.as_mut().unwrap().title = "new subtask".into();
        app.insert.as_mut().unwrap().parent_id = Some(parent_id);
        app.commit_insert();
        assert_eq!(app.task_ref(parent_id).unwrap().status, Status::Todo,
            "parent should demote to Todo when a new Todo child is added to a Done family");
    }

}

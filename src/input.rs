use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, BulkInsertStep, Column, Mode, ViewMode};
use crate::types::Status;
use crate::ui::tui;

pub enum AppAction {
    Quit,
    Save,
    None,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> AppAction {
    match &app.mode.clone() {
        Mode::Normal => handle_normal(app, key),
        Mode::Insert => {
            if app.insert.is_some() {
                handle_insert(app, key)
            } else {
                handle_edit(app, key)
            }
        }
        Mode::Move => handle_move(app, key),
        Mode::ProjectEdit => handle_project_edit(app, key),
        Mode::Help => handle_help(app, key),
        Mode::BulkInsert => handle_bulk_insert(app, key),
        Mode::Visual => handle_visual_keys(app, key),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> AppAction {
    if app.flag_clear_confirm {
        match key.code {
            KeyCode::Enter => {
                app.confirm_flag_clear();
                return AppAction::Save;
            }
            _ => {
                app.cancel_flag_clear();
                return AppAction::None;
            }
        }
    }

    // Keys that work in both planning and action mode
    match key.code {
        KeyCode::Char('q') => return AppAction::Quit,

        KeyCode::Char('j') | KeyCode::Down => app.move_cursor_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_cursor_up(),

        KeyCode::Char(c @ '1'..='9') => {
            let slot = (c as u8 - b'1') as usize;
            match app.view_mode {
                ViewMode::Tree => app.select_project_slot(slot),
                ViewMode::Board => app.enter_planning_for_slot_key(slot),
            }
        }
        KeyCode::Char('0') => match app.view_mode {
            ViewMode::Tree => app.select_project_slot(9),
            ViewMode::Board => app.enter_planning_for_slot_key(9),
        },
        KeyCode::Char('`') => match app.view_mode {
            ViewMode::Tree => app.select_inbox(),
            ViewMode::Board => app.enter_planning_for_inbox_tree(),
        },
        KeyCode::Char('=') => app.enable_all(),
        KeyCode::Char('-') => app.disable_all(),
        KeyCode::Char('P') => app.begin_project_edit(),
        KeyCode::Char('?') => app.mode = Mode::Help,

        // Flag highlight keys — work in both tree and board view
        KeyCode::Char('!') => app.toggle_flag_pill(0),
        KeyCode::Char('@') => app.toggle_flag_pill(1),
        KeyCode::Char('#') => app.toggle_flag_pill(2),
        KeyCode::Char('f') => {
            if app.flag_selected_task() {
                return AppAction::Save;
            }
        }
        KeyCode::Char('F') => app.begin_flag_clear(),

        _ => return match app.view_mode {
            ViewMode::Board => handle_action_keys(app, key),
            ViewMode::Tree => handle_planning_keys(app, key),
        },
    }
    AppAction::None
}

/// Action mode (kanban): navigate columns, move tasks between columns, enter planning.
fn handle_action_keys(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Char('h') | KeyCode::Left => app.focus_prev_col(),
        KeyCode::Char('l') | KeyCode::Right => app.focus_next_col(),

        KeyCode::Char('L') => {
            app.move_selected_right();
            return AppAction::Save;
        }
        KeyCode::Char('H') => {
            app.move_selected_left();
            return AppAction::Save;
        }

        // Reorder within Doing column (only Doing — Done is immutable, Todo uses sort mode)
        KeyCode::Char('K') if app.focused_col == Column::Doing => {
            app.kanban_doing_swap(-1);
            return AppAction::Save;
        }
        KeyCode::Char('J') if app.focused_col == Column::Doing => {
            app.kanban_doing_swap(1);
            return AppAction::Save;
        }

        // Cycle cross-project sort order (Todo column only)
        KeyCode::Char('s') => app.cycle_sort(),

        // Enter planning mode for the selected task's project
        KeyCode::Enter => app.enter_planning_for_selected(),

        // Return to tree, restoring the last-visited tree project
        KeyCode::Tab | KeyCode::Backspace => app.enter_planning_for_last_project(),

        _ => {}
    }
    AppAction::None
}

/// Planning mode (tree): structural operations only; Left/Right cycle projects, v exits to kanban.
fn handle_planning_keys(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        // Cycle through projects
        KeyCode::Left | KeyCode::Char(',') => app.cycle_project(-1),
        KeyCode::Right | KeyCode::Char('.') => app.cycle_project(1),

        // Return to action mode (kanban), positioning cursor on the selected task
        KeyCode::Enter => app.enter_kanban_for_selected(),

        // Return to action mode (kanban)
        KeyCode::Tab | KeyCode::Backspace => app.exit_planning(),

        // Reorder within tree (sibling-aware)
        KeyCode::Char('K') => {
            app.tree_swap_up();
            return AppAction::Save;
        }
        KeyCode::Char('J') => {
            app.tree_swap_down();
            return AppAction::Save;
        }

        // Insert sibling after / before
        KeyCode::Char('o') => app.begin_insert_after(),
        KeyCode::Char('O') => app.begin_insert_before(),

        // Edit title — 'i' cursor at start, 'a' cursor at end
        KeyCode::Char('i') => app.begin_edit(false),
        KeyCode::Char('a') => app.begin_edit(true),

        // Delete (dd)
        KeyCode::Char('d') => {
            if app.try_delete_dd() {
                return AppAction::Save;
            }
        }

        // Undo / Redo
        KeyCode::Char('u') => {
            app.undo();
            return AppAction::Save;
        }
        KeyCode::Char('r') => {
            app.redo();
            return AppAction::Save;
        }

        // Relationships
        KeyCode::Char('>') => {
            app.make_child();
            return AppAction::Save;
        }
        KeyCode::Char('<') => {
            app.make_root();
            return AppAction::Save;
        }

        // Project assignment
        KeyCode::Char('m') => app.begin_move_project(),

        // Bulk insert children
        KeyCode::Char('A') => app.begin_bulk_insert(),

        // Toggle task status (Doing / Done)
        KeyCode::Char('s') => {
            app.tree_toggle_doing();
            return AppAction::Save;
        }
        KeyCode::Char('x') => {
            app.tree_toggle_done();
            return AppAction::Save;
        }

        // Enter visual (multi-select) mode
        KeyCode::Char('V') => app.enter_visual(),

        // Jump to first / last row
        KeyCode::Char('g') => {
            if app.consume_gg() {
                tui::tree_goto_first(app);
            }
        }
        KeyCode::Char('G') => tui::tree_goto_last(app),

        // Jump to prev / next root task
        KeyCode::Char('[') => tui::tree_jump_prev_root(app),
        KeyCode::Char(']') => tui::tree_jump_next_root(app),

        // Fold / unfold branch
        KeyCode::Char('h') => app.fold_selected(),
        KeyCode::Char('l') => app.toggle_fold_selected(),

        _ => {}
    }
    AppAction::None
}

fn handle_visual_keys(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc | KeyCode::Char('V') => app.exit_visual(),

        KeyCode::Char('K') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() {
                app.push_undo();
                app.visual_shift_up(&ids);
            }
            return AppAction::Save;
        }
        KeyCode::Char('J') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() {
                app.push_undo();
                app.visual_shift_down(&ids);
            }
            return AppAction::Save;
        }

        KeyCode::Char('d') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() {
                app.push_undo();
                app.visual_delete(ids);
            }
            app.exit_visual();
            return AppAction::Save;
        }

        KeyCode::Char('s') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() {
                let all_doing = ids.iter().all(|&id| {
                    app.task_ref(id).map(|t| t.status == Status::Doing).unwrap_or(false)
                });
                let new_status = if all_doing { Status::Todo } else { Status::Doing };
                app.push_undo();
                app.visual_apply_status(&ids, new_status);
            }
            app.exit_visual();
            return AppAction::Save;
        }

        KeyCode::Char('x') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() {
                let all_done = ids.iter().all(|&id| {
                    app.task_ref(id).map(|t| t.status == Status::Done).unwrap_or(false)
                });
                let new_status = if all_done { Status::Todo } else { Status::Done };
                app.push_undo();
                app.visual_apply_status(&ids, new_status);
            }
            app.exit_visual();
            return AppAction::Save;
        }

        // Extend selection to first / last row
        KeyCode::Char('g') => { if app.consume_gg() { tui::tree_goto_first(app); } }
        KeyCode::Char('G') => tui::tree_goto_last(app),

        // Extend selection to prev / next root
        KeyCode::Char('[') => tui::tree_jump_prev_root(app),
        KeyCode::Char(']') => tui::tree_jump_next_root(app),

        _ => {}
    }
    AppAction::None
}

fn handle_insert(app: &mut App, key: KeyEvent) -> AppAction {
    if app.insert.is_none() {
        return AppAction::None;
    }

    if app.discard_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.discard_confirm = false;
                app.insert = None;
                app.mode = Mode::Normal;
            }
            _ => {
                app.discard_confirm = false;
                app.status_message = None;
            }
        }
        return AppAction::None;
    }

    if let Some(ref mut state) = app.insert {
        match key.code {
            KeyCode::Esc => {
                if state.title.is_empty() {
                    app.insert = None;
                    app.mode = Mode::Normal;
                } else {
                    app.discard_confirm = true;
                    app.status_message = Some("Discard changes? (y/n)".to_string());
                }
                return AppAction::None;
            }
            KeyCode::Enter => {
                let _ = state;
                app.commit_insert();
                return AppAction::Save;
            }
            KeyCode::Tab => {
                let _ = state;
                app.indent_insert();
                return AppAction::None;
            }
            KeyCode::BackTab => {
                let _ = state;
                app.unindent_insert();
                return AppAction::None;
            }
            KeyCode::Backspace => {
                state.title.pop();
                return AppAction::None;
            }
            KeyCode::Char(c) => {
                state.title.push(c);
                return AppAction::None;
            }
            _ => {}
        }
    }
    AppAction::None
}

fn handle_edit(app: &mut App, key: KeyEvent) -> AppAction {
    if app.edit.is_none() {
        return AppAction::None;
    }

    if app.discard_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.discard_confirm = false;
                app.edit = None;
                app.mode = Mode::Normal;
            }
            _ => {
                app.discard_confirm = false;
                app.status_message = None;
            }
        }
        return AppAction::None;
    }

    if let Some(ref mut state) = app.edit {
        match key.code {
            KeyCode::Esc => {
                let title_changed = app.tasks.iter()
                    .find(|t| t.id == state.task_id)
                    .map(|t| t.title != state.title)
                    .unwrap_or(false);
                if title_changed {
                    app.discard_confirm = true;
                    app.status_message = Some("Discard changes? (y/n)".to_string());
                } else {
                    app.edit = None;
                    app.mode = Mode::Normal;
                }
                return AppAction::None;
            }
            KeyCode::Enter => {
                let _ = state;
                app.commit_edit();
                return AppAction::Save;
            }
            KeyCode::Left => {
                if state.cursor_pos > 0 {
                    state.cursor_pos -= 1;
                }
                return AppAction::None;
            }
            KeyCode::Right => {
                let len = state.title.chars().count();
                if state.cursor_pos < len {
                    state.cursor_pos += 1;
                }
                return AppAction::None;
            }
            KeyCode::Backspace => {
                if state.cursor_pos > 0 {
                    let byte_pos = state.title.char_indices()
                        .nth(state.cursor_pos - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    state.title.remove(byte_pos);
                    state.cursor_pos -= 1;
                }
                return AppAction::None;
            }
            KeyCode::Char(c) => {
                let byte_pos = state.title.char_indices()
                    .nth(state.cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(state.title.len());
                state.title.insert(byte_pos, c);
                state.cursor_pos += 1;
                return AppAction::None;
            }
            _ => {}
        }
    }
    AppAction::None
}

fn handle_move(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc => {
            app.move_state = None;
            app.mode = Mode::Normal;
        }
        KeyCode::Char(c @ '1'..='9') => {
            let slot = (c as u8 - b'1') as usize;
            app.move_to_slot(slot);
            return AppAction::Save;
        }
        KeyCode::Char('0') => {
            app.move_to_slot(9);
            return AppAction::Save;
        }
        _ => {}
    }
    AppAction::None
}

fn handle_project_edit(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc => {
            app.project_edit = None;
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            app.commit_project_edit();
            return AppAction::Save;
        }
        KeyCode::Left => {
            app.project_edit_navigate(-1);
        }
        KeyCode::Right => {
            app.project_edit_navigate(1);
        }
        KeyCode::Backspace => {
            if let Some(ref mut pe) = app.project_edit {
                pe.input.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut pe) = app.project_edit {
                pe.input.push(c);
            }
        }
        _ => {}
    }
    AppAction::None
}

fn handle_bulk_insert(app: &mut App, key: KeyEvent) -> AppAction {
    if let Some(ref mut state) = app.bulk_insert {
        match key.code {
            KeyCode::Esc => {
                app.bulk_insert = None;
                app.mode = Mode::Normal;
                return AppAction::None;
            }
            KeyCode::Enter => {
                match state.step {
                    BulkInsertStep::Num => {
                        if let Ok(n) = state.num_input.trim().parse::<usize>() {
                            if n > 0 {
                                state.num = n;
                                state.step = BulkInsertStep::Prefix;
                            }
                        }
                        return AppAction::None;
                    }
                    BulkInsertStep::Prefix => {
                        let _ = state;
                        app.commit_bulk_insert();
                        return AppAction::Save;
                    }
                }
            }
            KeyCode::Backspace => {
                match state.step {
                    BulkInsertStep::Num => { state.num_input.pop(); }
                    BulkInsertStep::Prefix => { state.prefix_input.pop(); }
                }
                return AppAction::None;
            }
            KeyCode::Char(c) => {
                match state.step {
                    BulkInsertStep::Num => {
                        if c.is_ascii_digit() {
                            state.num_input.push(c);
                        }
                    }
                    BulkInsertStep::Prefix => {
                        state.prefix_input.push(c);
                    }
                }
                return AppAction::None;
            }
            _ => {}
        }
    }
    AppAction::None
}

fn handle_help(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
    AppAction::None
}


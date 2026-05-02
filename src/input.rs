use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, BulkInsertStep, Column, Mode, ViewMode};

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
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> AppAction {
    // Keys that work in both planning and action mode
    match key.code {
        KeyCode::Char('q') => return AppAction::Quit,

        KeyCode::Char('j') | KeyCode::Down => app.move_cursor_down(),
        KeyCode::Char('k') | KeyCode::Up => app.move_cursor_up(),

        KeyCode::Char(c @ '1'..='9') => {
            let slot = (c as u8 - b'1') as usize;
            match app.view_mode {
                ViewMode::Tree => app.select_project_slot(slot),
                ViewMode::Board => app.toggle_slot(slot),
            }
        }
        KeyCode::Char('0') => match app.view_mode {
            ViewMode::Tree => app.select_project_slot(9),
            ViewMode::Board => app.toggle_slot(9),
        },
        KeyCode::Char('`') => match app.view_mode {
            ViewMode::Tree => app.select_inbox(),
            ViewMode::Board => app.toggle_inbox(),
        },
        KeyCode::Char('=') => app.enable_all(),
        KeyCode::Char('-') => app.disable_all(),
        KeyCode::Char('P') => app.begin_project_edit(),
        KeyCode::Char('?') => app.mode = Mode::Help,

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

        // Enter tree mode for the highest-priority enabled project
        KeyCode::Tab => app.enter_planning_by_priority(),

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
        KeyCode::Tab => app.exit_planning(),

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
        KeyCode::Char('o') => {
            if app.focused_col == Column::Todo {
                app.begin_insert_after();
            } else {
                app.begin_insert_todo_end();
            }
        }
        KeyCode::Char('O') => {
            if app.focused_col == Column::Todo {
                app.begin_insert_before();
            }
        }

        // Edit title — 'i' cursor at start, 'a' cursor at end
        KeyCode::Char('i') => app.begin_edit(false),
        KeyCode::Char('a') => app.begin_edit(true),

        // Delete (dd)
        KeyCode::Char('d') => {
            if app.try_delete_dd() {
                return AppAction::Save;
            }
        }

        // Undo
        KeyCode::Char('u') => {
            app.undo();
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

        // Fold / unfold branch
        KeyCode::Char('h') => app.fold_selected(),
        KeyCode::Char('l') => app.toggle_fold_selected(),

        _ => {}
    }
    AppAction::None
}

fn handle_insert(app: &mut App, key: KeyEvent) -> AppAction {
    if let Some(ref mut state) = app.insert {
        match key.code {
            KeyCode::Esc => {
                app.insert = None;
                app.mode = Mode::Normal;
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
    if let Some(ref mut state) = app.edit {
        match key.code {
            KeyCode::Esc => {
                app.edit = None;
                app.mode = Mode::Normal;
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


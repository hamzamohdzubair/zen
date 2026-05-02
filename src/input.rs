use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, BulkInsertStep, Column, ConfirmAction, Mode};

pub enum AppAction {
    Quit,
    Save,
    None,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> AppAction {
    match &app.mode.clone() {
        Mode::Normal => handle_normal(app, key),
        Mode::Insert => handle_insert(app, key),
        Mode::Edit => handle_edit(app, key),
        Mode::Move => handle_move(app, key),
        Mode::ProjectEdit => handle_project_edit(app, key),
        Mode::Confirm(action) => handle_confirm(app, key, action.clone()),
        Mode::Help => handle_help(app, key),
        Mode::BulkInsert => handle_bulk_insert(app, key),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Char('q') => return AppAction::Quit,

        // Navigation
        KeyCode::Char('h') | KeyCode::Left => app.focus_prev_col(),
        KeyCode::Char('l') | KeyCode::Right => app.focus_next_col(),
        KeyCode::Char('k') | KeyCode::Up => app.move_cursor_up(),
        KeyCode::Char('j') | KeyCode::Down => app.move_cursor_down(),

        // Move card between columns
        KeyCode::Char('L') => {
            app.move_selected_right();
            return AppAction::Save;
        }
        KeyCode::Char('H') => {
            app.move_selected_left();
            return AppAction::Save;
        }

        // Reorder within column
        KeyCode::Char('K') => {
            app.swap_up();
            return AppAction::Save;
        }
        KeyCode::Char('J') => {
            app.swap_down();
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

        // Edit
        KeyCode::Char('i') => app.begin_edit(),

        // Delete
        KeyCode::Char('d') => app.delete_selected(),

        // Make child of task above
        KeyCode::Char('>') => {
            app.make_child();
            return AppAction::Save;
        }
        // Promote to root
        KeyCode::Char('<') => {
            app.make_root();
            return AppAction::Save;
        }

        // Move card to project (wait for digit)
        KeyCode::Char('m') => app.begin_move_project(),

        // Project slot toggles (1-9, 0)
        KeyCode::Char(c @ '1'..='9') => {
            let slot = (c as u8 - b'1') as usize;
            app.toggle_slot(slot);
        }
        KeyCode::Char('0') => {
            app.toggle_slot(9);
        }

        // Toggle unclassified tasks
        KeyCode::Char('`') => {
            app.toggle_unc();
        }

        // Enable / disable all project pills
        KeyCode::Char('=') => app.enable_all(),
        KeyCode::Char('-') => app.disable_all(),

        // Bulk insert children
        KeyCode::Char('A') => app.begin_bulk_insert(),

        // Project slot management
        KeyCode::Char('P') => app.begin_project_edit(),

        // Toggle view (tree ↔ board)
        KeyCode::Char('v') => app.toggle_view(),

        // Help
        KeyCode::Char('?') => app.mode = Mode::Help,

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

fn handle_confirm(app: &mut App, key: KeyEvent, action: ConfirmAction) -> AppAction {
    match key.code {
        KeyCode::Enter => match action {
            ConfirmAction::DeleteTask(id) => {
                app.confirm_delete(id);
                return AppAction::Save;
            }
        },
        KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        _ => {}
    }
    AppAction::None
}

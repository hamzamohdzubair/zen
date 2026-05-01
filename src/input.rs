use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, ConfirmAction, Mode};

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
        Mode::Filter => handle_filter(app, key),
        Mode::Move => handle_move(app, key),
        Mode::Confirm(action) => handle_confirm(app, key, action.clone()),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        // Quit
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
        KeyCode::Char('o') => app.begin_insert_after(),
        KeyCode::Char('O') => app.begin_insert_before(),

        // Edit
        KeyCode::Char('i') => app.begin_edit(),

        // Delete
        KeyCode::Char('d') => app.delete_selected(),

        // Filter
        KeyCode::Char('f') => app.begin_filter(),
        KeyCode::Char('F') => {
            app.clear_filter();
            return AppAction::None;
        }

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

        // Move card to project
        KeyCode::Char('m') => app.begin_move_project(),

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

fn handle_filter(app: &mut App, key: KeyEvent) -> AppAction {
    if app.filter.is_none() {
        return AppAction::None;
    }
    match key.code {
        KeyCode::Esc => {
            app.filter = None;
            app.mode = Mode::Normal;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.clear_filter();
        }
        KeyCode::Enter => {
            app.commit_filter();
        }
        KeyCode::Backspace => {
            if let Some(ref mut state) = app.filter {
                state.input.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut state) = app.filter {
                state.input.push(c);
            }
        }
        _ => {}
    }
    AppAction::None
}

fn handle_move(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc => {
            app.move_state = None;
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            if let Some(ms) = app.move_state.take() {
                let target = ms.target_input.trim().to_string();
                if !target.is_empty() {
                    app.set_project_recursive(ms.task_id, target);
                }
            }
            app.mode = Mode::Normal;
            return AppAction::Save;
        }
        KeyCode::Tab => {
            let suggestions = app.move_suggestions();
            if let Some(ref mut ms) = app.move_state {
                if let Some(cursor) = ms.suggestion_cursor {
                    if let Some(s) = suggestions.get(cursor) {
                        ms.target_input = s.clone();
                    }
                    ms.suggestion_cursor = None;
                    ms.suggestion_query = None;
                } else if !suggestions.is_empty() {
                    ms.suggestion_query = Some(ms.target_input.clone());
                    ms.target_input = suggestions[0].clone();
                    ms.suggestion_cursor = Some(0);
                }
            }
        }
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let suggestions = app.move_suggestions();
            if let Some(ref mut ms) = app.move_state {
                if let Some(cursor) = ms.suggestion_cursor {
                    let next = (cursor + 1).min(suggestions.len().saturating_sub(1));
                    ms.suggestion_cursor = Some(next);
                    if let Some(s) = suggestions.get(next) {
                        ms.target_input = s.clone();
                    }
                }
            }
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let suggestions = app.move_suggestions();
            if let Some(ref mut ms) = app.move_state {
                if let Some(cursor) = ms.suggestion_cursor {
                    let prev = cursor.saturating_sub(1);
                    ms.suggestion_cursor = Some(prev);
                    if let Some(s) = suggestions.get(prev) {
                        ms.target_input = s.clone();
                    }
                }
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut ms) = app.move_state {
                ms.target_input.pop();
                ms.suggestion_cursor = None;
                ms.suggestion_query = None;
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut ms) = app.move_state {
                ms.target_input.push(c);
                ms.suggestion_cursor = None;
                ms.suggestion_query = None;
            }
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


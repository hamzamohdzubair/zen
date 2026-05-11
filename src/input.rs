use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{App, ArchiveView, BulkInsertStep, Mode, PendingConfirm};
use crate::types::Status;
use crate::ui::tui;

pub enum AppAction {
    Quit,
    Save,
    None,
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> AppAction {
    if matches!(app.mode, Mode::ArchiveBrowser) {
        return handle_archive_browser(app, key);
    }
    match &app.mode.clone() {
        Mode::Normal        => handle_normal(app, key),
        Mode::Insert => {
            if app.insert.is_some() { handle_insert(app, key) }
            else { handle_edit(app, key) }
        }
        Mode::Help          => handle_help(app, key),
        Mode::BulkInsert    => handle_bulk_insert(app, key),
        Mode::Visual        => handle_visual_keys(app, key),
        Mode::SnoozeInput   => handle_snooze_input(app, key),
        Mode::Search        => handle_search(app, key),
        Mode::ArchiveBrowser => AppAction::None,
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> AppAction {
    if app.flag_clear_confirm {
        match key.code {
            KeyCode::Enter => { app.confirm_flag_clear(); return AppAction::Save; }
            _ => { app.cancel_flag_clear(); return AppAction::None; }
        }
    }

    // PendingConfirm: Enter confirms, anything else cancels.
    if let Some(confirm) = app.pending_confirm.clone() {
        match key.code {
            KeyCode::Enter => {
                match confirm {
                    PendingConfirm::ExpireSnooze(id) => {
                        app.confirm_expire_snooze(id);
                        return AppAction::Save;
                    }
                }
            }
            _ => {
                app.pending_confirm = None;
                app.status_message = None;
            }
        }
        return AppAction::None;
    }

    match key.code {
        KeyCode::Char('q') => return AppAction::Quit,

        KeyCode::Esc => {
            // Clear active search when Esc is pressed in Normal mode.
            if app.search.is_some() {
                app.cancel_search();
            }
        }

        KeyCode::Char('j') | KeyCode::Down => app.move_cursor_down(),
        KeyCode::Char('k') | KeyCode::Up   => app.move_cursor_up(),

        KeyCode::Char('/') => { app.begin_search(); return AppAction::None; }

        KeyCode::Char('?') => app.mode = Mode::Help,

        // Flag highlight keys
        KeyCode::Char('!') => app.toggle_flag_pill(0),
        KeyCode::Char('@') => app.toggle_flag_pill(1),
        KeyCode::Char('#') => app.toggle_flag_pill(2),
        KeyCode::Char('f') => { if app.flag_selected_task() { return AppAction::Save; } }
        KeyCode::Char('F') => app.begin_flag_clear(),

        _ => return handle_tree_keys(app, key),
    }
    AppAction::None
}

/// Tree-mode structural keys.
fn handle_tree_keys(app: &mut App, key: KeyEvent) -> AppAction {
    // z-chord
    if let Some(z_at) = app.last_z_press.take() {
        if z_at.elapsed().as_millis() < 500 {
            return handle_z_chord(app, key);
        }
    }

    match key.code {
        // Reorder siblings
        KeyCode::Char('K') => { app.tree_swap_up();   return AppAction::Save; }
        KeyCode::Char('J') => { app.tree_swap_down(); return AppAction::Save; }

        // Insert sibling after / before
        KeyCode::Char('o') => app.begin_insert_after(),
        KeyCode::Char('O') => app.begin_insert_before(),

        // Edit title
        KeyCode::Char('I') => app.begin_edit(false),
        KeyCode::Char('A') => app.begin_edit(true),
        KeyCode::Char('i') => app.begin_edit_at_percent(25),
        KeyCode::Char('a') => app.begin_edit_at_percent(75),

        // Delete (dd)
        KeyCode::Char('d') => { if app.try_delete_dd() { return AppAction::Save; } }

        // Undo / Redo
        KeyCode::Char('u') => { app.undo(); return AppAction::Save; }
        KeyCode::Char('r') => { app.redo(); return AppAction::Save; }

        // backspace: g+backspace opens archive browser; otherwise context-sensitive hide
        KeyCode::Backspace => {
            if app.last_g_press.take().map(|t| t.elapsed().as_millis() < 500).unwrap_or(false) {
                app.open_archive_browser();
            } else {
                let col = app.focused_col;
                if let Some(id) = app.selected_task_id(col) {
                    if let Some(task) = app.task_ref(id) {
                        let status = task.status;
                        let clocked = app.is_clocked(task);
                        match (status, clocked) {
                            (_, true) => {
                                app.begin_expire_snooze(id);
                            }
                            (Status::Done, false) => {
                                app.hide_task(id);
                                return AppAction::Save;
                            }
                            (Status::Todo, false) => {
                                app.begin_snooze();
                            }
                            (Status::Doing, false) => {
                                app.status_message = Some(
                                    "Cannot archive or snooze a 'doing' task".into()
                                );
                            }
                        }
                    }
                }
            }
        }

        // Search navigation
        KeyCode::Char('n') => {
            if app.search.is_some() { app.search_next(); }
        }
        KeyCode::Char('N') => {
            if app.search.is_some() { app.search_prev(); }
        }

        // Status operations
        KeyCode::Char(' ') => { app.tree_toggle_doing(); return AppAction::Save; }
        KeyCode::Enter     => { app.tree_toggle_done();  return AppAction::Save; }

        // Relationships
        KeyCode::Char('>') => { app.make_child(); return AppAction::Save; }
        KeyCode::Char('<') => { app.make_root();  return AppAction::Save; }

        // Bulk insert children
        KeyCode::Char('M') => app.begin_bulk_insert(),

        // Enter visual (multi-select) mode
        KeyCode::Char('V') => app.enter_visual(),

        // Jump to first / last row
        KeyCode::Char('g') => {
            if app.consume_gg() {
                tui::tree_goto_first(app);
            } else {
                // Record g-press for g+backspace chord (archive browser).
                use std::time::Instant;
                app.last_g_press = Some(Instant::now());
            }
        }
        KeyCode::Char('G') => tui::tree_goto_last(app),

        // Fold / unfold branch
        KeyCode::Char('h') => app.fold_selected(),
        KeyCode::Char('l') => app.toggle_fold_selected(),

        // Begin z-chord (fold shortcuts)
        KeyCode::Char('z') => {
            use std::time::Instant;
            app.last_z_press = Some(Instant::now());
        }

        _ => {}
    }
    AppAction::None
}

/// Handle the second key of a z-chord (z+key fold shortcuts).
fn handle_z_chord(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Char('a') => app.toggle_fold_all(),
        KeyCode::Char('o') => app.fold_focus_current(),
        KeyCode::Char(' ') => app.jump_next_doing(),
        KeyCode::Char('g') => app.fold_focus_global(),
        KeyCode::Char('l') => app.fold_focus_local(),
        KeyCode::Char('.') => app.cycle_leaf_next(),
        KeyCode::Char(',') => app.cycle_leaf_prev(),
        _ => {}
    }
    AppAction::None
}

fn handle_visual_keys(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc | KeyCode::Char('V') => app.exit_visual(),

        KeyCode::Char('K') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() { app.push_undo(); app.visual_shift_up(&ids); }
            return AppAction::Save;
        }
        KeyCode::Char('J') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() { app.push_undo(); app.visual_shift_down(&ids); }
            return AppAction::Save;
        }

        KeyCode::Char('d') => {
            let ids = tui::visual_selected_ids(app);
            if !ids.is_empty() { app.push_undo(); app.visual_delete(ids); }
            app.exit_visual();
            return AppAction::Save;
        }

        KeyCode::Char(' ') => {
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

        KeyCode::Enter => {
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

        KeyCode::Char('g') => { if app.consume_gg() { tui::tree_goto_first(app); } }
        KeyCode::Char('G') => tui::tree_goto_last(app),

        _ => {}
    }
    AppAction::None
}

fn handle_snooze_input(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc => {
            app.snooze_input = None;
            app.mode = Mode::Normal;
            app.status_message = None;
        }
        KeyCode::Enter => {
            app.commit_snooze();
            return AppAction::Save;
        }
        KeyCode::Backspace => {
            if let Some(ref mut si) = app.snooze_input {
                si.input.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(ref mut si) = app.snooze_input {
                si.input.push(c);
            }
        }
        _ => {}
    }
    AppAction::None
}

fn handle_search(app: &mut App, key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc        => app.cancel_search(),
        KeyCode::Enter      => app.commit_search(),
        KeyCode::Backspace  => app.search_pop(),
        KeyCode::Char(c)    => app.search_push(c),
        _ => {}
    }
    AppAction::None
}

fn handle_insert(app: &mut App, key: KeyEvent) -> AppAction {
    if app.insert.is_none() { return AppAction::None; }

    if app.discard_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.discard_confirm = false;
                app.insert = None;
                app.mode = Mode::Normal;
            }
            _ => { app.discard_confirm = false; app.status_message = None; }
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
            KeyCode::Enter => { let _ = state; app.commit_insert(); return AppAction::Save; }
            KeyCode::Tab => { let _ = state; app.indent_insert(); return AppAction::None; }
            KeyCode::BackTab => { let _ = state; app.unindent_insert(); return AppAction::None; }
            KeyCode::Backspace => { state.title.pop(); return AppAction::None; }
            KeyCode::Char(c) => { state.title.push(c); return AppAction::None; }
            _ => {}
        }
    }
    AppAction::None
}

fn handle_edit(app: &mut App, key: KeyEvent) -> AppAction {
    if app.edit.is_none() { return AppAction::None; }

    if app.discard_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.discard_confirm = false;
                app.edit = None;
                app.mode = Mode::Normal;
            }
            _ => { app.discard_confirm = false; app.status_message = None; }
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
            KeyCode::Enter => { let _ = state; app.commit_edit(); return AppAction::Save; }
            KeyCode::Left => { if state.cursor_pos > 0 { state.cursor_pos -= 1; } return AppAction::None; }
            KeyCode::Right => {
                let len = state.title.chars().count();
                if state.cursor_pos < len { state.cursor_pos += 1; }
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

fn handle_bulk_insert(app: &mut App, key: KeyEvent) -> AppAction {
    if let Some(ref mut state) = app.bulk_insert {
        match key.code {
            KeyCode::Esc => { app.bulk_insert = None; app.mode = Mode::Normal; return AppAction::None; }
            KeyCode::Enter => {
                match state.step {
                    BulkInsertStep::Num => {
                        if let Ok(n) = state.num_input.trim().parse::<usize>() {
                            if n > 0 { state.num = n; state.step = BulkInsertStep::Prefix; }
                        }
                        return AppAction::None;
                    }
                    BulkInsertStep::Prefix => { let _ = state; app.commit_bulk_insert(); return AppAction::Save; }
                }
            }
            KeyCode::Backspace => {
                match state.step {
                    BulkInsertStep::Num    => { state.num_input.pop(); }
                    BulkInsertStep::Prefix => { state.prefix_input.pop(); }
                }
                return AppAction::None;
            }
            KeyCode::Char(c) => {
                match state.step {
                    BulkInsertStep::Num    => { if c.is_ascii_digit() { state.num_input.push(c); } }
                    BulkInsertStep::Prefix => { state.prefix_input.push(c); }
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
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => { app.mode = Mode::Normal; }
        _ => {}
    }
    AppAction::None
}

fn handle_archive_browser(app: &mut App, key: KeyEvent) -> AppAction {
    let date_jumping = app.archive_browser
        .as_ref()
        .map(|ab| ab.date_jump_input.is_some())
        .unwrap_or(false);

    if date_jumping {
        match key.code {
            KeyCode::Esc => app.archive_cancel_date_jump(),
            KeyCode::Enter => { app.archive_commit_date_jump(); }
            KeyCode::Backspace => {
                if let Some(ref mut ab) = app.archive_browser {
                    if let Some(ref mut s) = ab.date_jump_input { s.pop(); }
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut ab) = app.archive_browser {
                    if let Some(ref mut s) = ab.date_jump_input {
                        if s.len() < 10 { s.push(c); }
                    }
                }
            }
            _ => {}
        }
        return AppAction::None;
    }

    let in_day_view = app.archive_browser
        .as_ref()
        .map(|ab| matches!(ab.view, ArchiveView::Day))
        .unwrap_or(false);

    if in_day_view {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => app.archive_back_to_calendar(),
            KeyCode::Char('/') => app.archive_begin_date_jump(),
            KeyCode::Char('[') => app.archive_day_prev(),
            KeyCode::Char(']') => app.archive_day_next(),
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut ab) = app.archive_browser {
                    ab.day_scroll = ab.day_scroll.saturating_add(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut ab) = app.archive_browser {
                    ab.day_scroll = ab.day_scroll.saturating_sub(1);
                }
            }
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => app.close_archive_browser(),
            KeyCode::Char('/') => app.archive_begin_date_jump(),
            KeyCode::Char('h') | KeyCode::Left  => app.archive_prev_day(),
            KeyCode::Char('l') | KeyCode::Right => app.archive_next_day(),
            KeyCode::Char('k') | KeyCode::Up    => app.archive_prev_week(),
            KeyCode::Char('j') | KeyCode::Down  => app.archive_next_week(),
            KeyCode::Char('[')                   => app.archive_prev_month(),
            KeyCode::Char(']')                   => app.archive_next_month(),
            KeyCode::Enter                       => app.archive_open_day(),
            _ => {}
        }
    }
    AppAction::None
}

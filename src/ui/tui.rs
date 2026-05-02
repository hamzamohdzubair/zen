use std::collections::{HashMap, HashSet};

use uuid::Uuid;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Column, InsertPosition, Mode};
use crate::types::Status;
use super::{pill_span, slot_key_char, unc_pill_span};
use super::board::project_to_color;

const GUTTER_WIDTH: u16 = 10;

#[derive(Clone, Copy, PartialEq)]
pub enum RowKind {
    Next,
    Leaf,
    Doing,
    Done,
    ParentTodo,
}

pub struct TuiRow {
    pub id: Uuid,
    pub title: String,
    pub depth: usize,
    pub kind: RowKind,
}

pub fn build_tui_rows(app: &App) -> Vec<TuiRow> {
    let visible_ids: HashSet<Uuid> = app.tasks.iter()
        .filter(|t| app.task_visible(t))
        .map(|t| t.id)
        .collect();

    let tasks_by_id: HashMap<Uuid, &crate::types::Task> = app.tasks.iter()
        .map(|t| (t.id, t))
        .collect();

    let roots: Vec<Uuid> = app.tasks.iter()
        .filter(|t| {
            visible_ids.contains(&t.id)
                && t.parent_id.map(|pid| !visible_ids.contains(&pid)).unwrap_or(true)
        })
        .map(|t| t.id)
        .collect();

    let mut rows: Vec<TuiRow> = Vec::new();
    let mut first_next_found = false;

    fn visit(
        id: Uuid,
        depth: usize,
        visible_ids: &HashSet<Uuid>,
        tasks_by_id: &HashMap<Uuid, &crate::types::Task>,
        rows: &mut Vec<TuiRow>,
        first_next_found: &mut bool,
    ) {
        let task = match tasks_by_id.get(&id) {
            Some(t) => t,
            None => return,
        };

        let kind = match task.status {
            Status::Done => RowKind::Done,
            Status::Doing => RowKind::Doing,
            Status::Todo => {
                let has_active_children = task.children.iter().any(|&cid| {
                    visible_ids.contains(&cid)
                        && tasks_by_id.get(&cid)
                            .map(|c| matches!(c.status, Status::Todo | Status::Doing))
                            .unwrap_or(false)
                });
                if has_active_children {
                    RowKind::ParentTodo
                } else if !*first_next_found {
                    *first_next_found = true;
                    RowKind::Next
                } else {
                    RowKind::Leaf
                }
            }
        };

        rows.push(TuiRow { id, title: task.title.clone(), depth, kind });

        for &cid in &task.children {
            if visible_ids.contains(&cid) {
                visit(cid, depth + 1, visible_ids, tasks_by_id, rows, first_next_found);
            }
        }
    }

    for root_id in roots {
        visit(root_id, 0, &visible_ids, &tasks_by_id, &mut rows, &mut first_next_found);
    }

    rows
}

pub fn navigate_tree(app: &mut App, delta: i32) {
    let rows = build_tui_rows(app);
    if rows.is_empty() {
        return;
    }
    let selected_id = app.selected_task_id(app.focused_col);
    let current_pos = selected_id
        .and_then(|id| rows.iter().position(|r| r.id == id))
        .unwrap_or(0);
    let new_pos = (current_pos as i32 + delta).clamp(0, rows.len() as i32 - 1) as usize;
    let new_id = rows[new_pos].id;
    if let Some(new_task) = app.task_ref(new_id) {
        let new_col = match new_task.status {
            Status::Todo => Column::Todo,
            Status::Doing => Column::Doing,
            Status::Done => Column::Done,
        };
        app.focused_col = new_col;
        let col_tasks = app.visible_tasks_for(new_col);
        if let Some(pos) = col_tasks.iter().position(|t| t.id == new_id) {
            app.cursor[App::col_index(new_col)] = pos;
        }
    }
}

pub fn compute_scroll(app: &App, current_scroll: usize, task_area_height: usize) -> usize {
    let rows = build_tui_rows(app);
    let sel_id = app.selected_task_id(app.focused_col);
    let sel_idx = sel_id
        .and_then(|id| rows.iter().position(|r| r.id == id))
        .unwrap_or(0);
    if sel_idx < current_scroll {
        sel_idx
    } else if task_area_height > 0 && sel_idx >= current_scroll + task_area_height {
        sel_idx.saturating_sub(task_area_height.saturating_sub(1))
    } else {
        current_scroll
    }
}

fn inline_insert_row(app: &App) -> Option<(usize, TuiRow)> {
    let state = app.insert.as_ref()?;
    let rows = build_tui_rows(app);

    let insert_idx = match &state.position {
        InsertPosition::AtBeginning => 0,
        InsertPosition::AfterSibling(after_id) | InsertPosition::AfterParent(after_id) => {
            rows.iter().position(|r| r.id == *after_id)
                .map(|i| i + 1)
                .unwrap_or(rows.len())
        }
    };

    let parent_depth = state.parent_id
        .and_then(|pid| rows.iter().find(|r| r.id == pid))
        .map(|r| r.depth)
        .unwrap_or(0);

    let depth = if state.parent_id.is_some() { parent_depth + 1 } else { 0 };

    let title = format!("{}\u{2588}", state.title);
    let kind = match state.status {
        Status::Todo => RowKind::Leaf,
        Status::Doing => RowKind::Doing,
        Status::Done => RowKind::Done,
    };

    Some((insert_idx.min(rows.len()), TuiRow { id: Uuid::nil(), title, depth, kind }))
}

fn gutter_for(kind: RowKind) -> (&'static str, Style) {
    match kind {
        RowKind::Next => (
            "▶ NEXT    ",
            Style::default().fg(Color::Black).bg(Color::Indexed(46)),
        ),
        RowKind::Doing => (
            "  DOING   ",
            Style::default().fg(Color::Black).bg(Color::Indexed(214)),
        ),
        RowKind::Done => (
            "  DONE    ",
            Style::default().fg(Color::Indexed(242)).bg(Color::Indexed(236)),
        ),
        RowKind::Leaf | RowKind::ParentTodo => ("          ", Style::default()),
    }
}

fn title_style_for(kind: RowKind) -> Style {
    match kind {
        RowKind::Next => Style::default().fg(Color::Indexed(46)).add_modifier(Modifier::BOLD),
        RowKind::Doing => Style::default().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD),
        RowKind::Leaf => Style::default().fg(Color::Indexed(252)),
        RowKind::ParentTodo => Style::default().fg(Color::Indexed(242)).add_modifier(Modifier::DIM),
        RowKind::Done => Style::default()
            .fg(Color::Indexed(240))
            .add_modifier(Modifier::CROSSED_OUT),
    }
}

fn truncate_to(s: String, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s
    } else {
        s.chars().take(max_chars).collect()
    }
}

pub fn draw_tui(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);
    draw_pills(frame, app, chunks[1]);
    draw_task_area(frame, app, app.tui_scroll_offset, chunks[2]);
}

fn draw_header(frame: &mut Frame, _app: &App, area: Rect) {
    let sep = Style::default().fg(Color::Indexed(240));
    let spans = vec![
        Span::styled(" TUI ", Style::default().fg(Color::Black).bg(Color::Blue)),
        Span::styled("│", sep),
        Span::styled(
            "  j/k navigate  L/H move  i/o insert  < > indent  d delete  ? help  v board  q quit",
            Style::default().fg(Color::Indexed(242)),
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_pills(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();

    if app.has_unc_tasks() {
        spans.push(unc_pill_span(app.unc_doable_count(), app.show_unc));
    }

    for slot in 0..10 {
        if let Some(name) = &app.projects[slot] {
            let key = slot_key_char(slot);
            let count = app.doable_count_for_slot(slot);
            let color = project_to_color(name);
            spans.push(pill_span(key, name, count, app.active_slots[slot], color));
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_task_area(frame: &mut Frame, app: &App, scroll_offset: usize, area: Rect) {
    if area.height == 0 {
        return;
    }

    let mut rows = build_tui_rows(app);

    if matches!(app.mode, Mode::Insert) {
        if let Some((idx, inline_row)) = inline_insert_row(app) {
            rows.insert(idx, inline_row);
        }
    }

    let selected_id = app.selected_task_id(app.focused_col);

    let mut y = area.y;
    for row in rows.iter().skip(scroll_offset).take(area.height as usize) {
        let is_selected = selected_id == Some(row.id);

        let (gutter_text, gutter_style) = gutter_for(row.kind);
        frame.render_widget(
            Paragraph::new(Span::styled(gutter_text, gutter_style)),
            Rect { x: area.x, y, width: GUTTER_WIDTH, height: 1 },
        );

        let title_x = area.x + GUTTER_WIDTH;
        let title_width = area.width.saturating_sub(GUTTER_WIDTH);
        let title_area = Rect { x: title_x, y, width: title_width, height: 1 };

        let indent = "  ".repeat(row.depth);
        let connector = if row.depth > 0 { "╰─ " } else { "" };
        let full = format!("{}{}{}", indent, connector, row.title);
        let truncated = truncate_to(full, title_width as usize);

        let mut style = title_style_for(row.kind);
        if is_selected {
            style = style.bg(Color::Indexed(238));
        }

        frame.render_widget(Paragraph::new(Span::styled(truncated, style)), title_area);

        y += 1;
    }
}

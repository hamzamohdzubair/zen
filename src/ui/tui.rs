use std::collections::{HashMap, HashSet};

use uuid::Uuid;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Column, InsertPosition, Mode};
use crate::types::Status;

#[derive(Clone, Copy, PartialEq)]
pub enum RowKind {
    Todo,
    Doing,
    Done,
}

pub struct TuiRow {
    pub id: Uuid,
    pub title: String,
    pub depth: usize,
    pub kind: RowKind,
    /// Visual prefix including tree connectors, e.g. "│  ├─ "
    pub display_prefix: String,
    /// Prefix to pass down to children of this row, e.g. "│  │  "
    pub children_prefix: String,
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

    fn visit(
        id: Uuid,
        depth: usize,
        parent_children_prefix: &str,
        is_last: bool,
        visible_ids: &HashSet<Uuid>,
        tasks_by_id: &HashMap<Uuid, &crate::types::Task>,
        rows: &mut Vec<TuiRow>,
    ) {
        let task = match tasks_by_id.get(&id) {
            Some(t) => t,
            None => return,
        };

        let kind = match task.status {
            Status::Done => RowKind::Done,
            Status::Doing => RowKind::Doing,
            Status::Todo => RowKind::Todo,
        };

        let (display_prefix, children_prefix) = if depth == 0 {
            (String::new(), String::new())
        } else {
            let connector = if is_last { "╰─ " } else { "├─ " };
            let child_cont = if is_last { "   " } else { "│  " };
            (
                format!("{}{}", parent_children_prefix, connector),
                format!("{}{}", parent_children_prefix, child_cont),
            )
        };

        let visible_children: Vec<Uuid> = task.children.iter()
            .filter(|&&cid| visible_ids.contains(&cid))
            .copied()
            .collect();

        rows.push(TuiRow {
            id,
            title: task.title.clone(),
            depth,
            kind,
            display_prefix,
            children_prefix: children_prefix.clone(),
        });

        let n = visible_children.len();
        for (i, &cid) in visible_children.iter().enumerate() {
            visit(cid, depth + 1, &children_prefix, i == n - 1, visible_ids, tasks_by_id, rows);
        }
    }

    let n = roots.len();
    for (i, &root_id) in roots.iter().enumerate() {
        visit(root_id, 0, "", i == n - 1, &visible_ids, &tasks_by_id, &mut rows);
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
    let insert_idx = insert_idx.min(rows.len());

    let (depth, display_prefix, children_prefix) = if let Some(pid) = state.parent_id {
        let parent = rows.iter().find(|r| r.id == pid);
        let d = parent.map(|p| p.depth + 1).unwrap_or(1);
        let pcprefix = parent.map(|p| p.children_prefix.as_str()).unwrap_or("");
        let has_sibling_after = rows.get(insert_idx).map(|r| r.depth == d).unwrap_or(false);
        let connector = if has_sibling_after { "├─ " } else { "╰─ " };
        let child_cont = if has_sibling_after { "│  " } else { "   " };
        (
            d,
            format!("{}{}", pcprefix, connector),
            format!("{}{}", pcprefix, child_cont),
        )
    } else {
        (0, String::new(), String::new())
    };

    let title = format!("{}\u{2588}", state.title);
    let kind = match state.status {
        Status::Todo => RowKind::Todo,
        Status::Doing => RowKind::Doing,
        Status::Done => RowKind::Done,
    };

    Some((insert_idx, TuiRow { id: Uuid::nil(), title, depth, kind, display_prefix, children_prefix }))
}

fn title_style_for(kind: RowKind) -> Style {
    match kind {
        RowKind::Todo => Style::default().fg(Color::Indexed(252)),
        RowKind::Doing => Style::default().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD),
        RowKind::Done => Style::default()
            .fg(Color::Indexed(240))
            .add_modifier(Modifier::CROSSED_OUT),
    }
}

fn truncate_to(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

pub fn draw_tui(frame: &mut Frame, app: &App, area: Rect) {
    draw_task_area(frame, app, app.tui_scroll_offset, area);
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
    let meta_style = Style::default().fg(Color::Indexed(238));
    let num_style = Style::default().fg(Color::Indexed(245));

    // Pre-compute hierarchical labels (e.g. "1", "1.1", "1.2", "2") across all rows
    // so that scrolled-off rows still count correctly.
    let mut depth_counters: Vec<usize> = Vec::new();
    let num_labels: Vec<String> = rows.iter().map(|row| {
        if row.id == Uuid::nil() {
            return String::new();
        }
        let d = row.depth;
        if d < depth_counters.len() {
            depth_counters.truncate(d + 1);
            depth_counters[d] += 1;
        } else {
            depth_counters.push(1);
        }
        depth_counters.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(".")
    }).collect();

    let mut y = area.y;
    for (row_idx, row) in rows.iter().enumerate().skip(scroll_offset).take(area.height as usize) {
        let is_inline = row.id == Uuid::nil();

        let is_selected = selected_id == Some(row.id);
        let is_editing = app.mode == Mode::Edit
            && app.edit.as_ref().map(|es| es.task_id == row.id).unwrap_or(false);
        let bg = if is_editing {
            Some(Color::Rgb(30, 75, 45))
        } else if is_selected {
            Some(Color::Indexed(238))
        } else {
            None
        };
        let ms = if let Some(bg) = bg { meta_style.bg(bg) } else { meta_style };

        let num_str = if is_inline || num_labels[row_idx].is_empty() {
            String::new()
        } else {
            format!("{} ", num_labels[row_idx])
        };
        let prefix_chars = row.display_prefix.chars().count() + num_str.chars().count();
        let title_width = (area.width as usize).saturating_sub(prefix_chars);
        let raw_title = if is_editing {
            let es = app.edit.as_ref().unwrap();
            let mut chars: Vec<char> = es.title.chars().collect();
            chars.insert(es.cursor_pos.min(chars.len()), '\u{2588}');
            chars.into_iter().collect()
        } else {
            row.title.clone()
        };
        let title_text = truncate_to(&raw_title, title_width);

        let mut title_style = title_style_for(row.kind);
        if let Some(bg) = bg { title_style = title_style.bg(bg); }

        let mut spans: Vec<Span> = Vec::new();
        if !row.display_prefix.is_empty() {
            spans.push(Span::styled(row.display_prefix.clone(), ms));
        }
        if !num_str.is_empty() {
            let ns = if let Some(bg) = bg { num_style.bg(bg) } else { num_style };
            spans.push(Span::styled(num_str, ns));
        }
        spans.push(Span::styled(title_text, title_style));

        let para_style = if let Some(bg) = bg { Style::default().bg(bg) } else { Style::default() };
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(para_style),
            Rect { x: area.x, y, width: area.width, height: 1 },
        );

        y += 1;
    }
}

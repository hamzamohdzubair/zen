use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};
use uuid::Uuid;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::{App, ArchiveBrowserState, ArchiveView, Column, InsertPosition, Mode};
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
    pub is_collapsed: bool,
}

/// Returns the task IDs covered by the current visual selection (anchor → cursor, inclusive).
/// Ordered by DFS row position (top to bottom).
pub fn visual_selected_ids(app: &App) -> Vec<Uuid> {
    let anchor_id = match app.visual_anchor_id {
        Some(id) => id,
        None => return Vec::new(),
    };
    let rows = build_tui_rows(app);
    let anchor_pos = match rows.iter().position(|r| r.id == anchor_id) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let cursor_pos = app.selected_task_id(app.focused_col)
        .and_then(|id| rows.iter().position(|r| r.id == id))
        .unwrap_or(anchor_pos);
    let (lo, hi) = if anchor_pos <= cursor_pos {
        (anchor_pos, cursor_pos)
    } else {
        (cursor_pos, anchor_pos)
    };
    rows[lo..=hi].iter().map(|r| r.id).collect()
}

fn has_doing_descendant(id: Uuid, tasks_by_id: &HashMap<Uuid, &crate::types::Task>, visible_ids: &HashSet<Uuid>) -> bool {
    let children = match tasks_by_id.get(&id) {
        Some(t) => t.children.clone(),
        None => return false,
    };
    for cid in children {
        if !visible_ids.contains(&cid) { continue; }
        if tasks_by_id.get(&cid).map(|c| c.status == Status::Doing).unwrap_or(false) {
            return true;
        }
        if has_doing_descendant(cid, tasks_by_id, visible_ids) {
            return true;
        }
    }
    false
}

fn visit_task(
    id: Uuid,
    depth: usize,
    parent_children_prefix: &str,
    is_last: bool,
    visible_ids: &HashSet<Uuid>,
    collapsed: &HashSet<Uuid>,
    tasks_by_id: &HashMap<Uuid, &crate::types::Task>,
    rows: &mut Vec<TuiRow>,
) {
    let task = match tasks_by_id.get(&id) {
        Some(t) => t,
        None => return,
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

    let is_collapsed = collapsed.contains(&id) && !visible_children.is_empty();

    let kind = if is_collapsed && has_doing_descendant(id, tasks_by_id, visible_ids) {
        RowKind::Doing
    } else {
        match task.status {
            Status::Done => RowKind::Done,
            Status::Doing => RowKind::Doing,
            Status::Todo => RowKind::Todo,
        }
    };

    rows.push(TuiRow {
        id,
        title: task.title.clone(),
        depth,
        kind,
        display_prefix,
        children_prefix: children_prefix.clone(),
        is_collapsed,
    });

    if !is_collapsed {
        let n = visible_children.len();
        for (i, &cid) in visible_children.iter().enumerate() {
            visit_task(cid, depth + 1, &children_prefix, i == n - 1, visible_ids, collapsed, tasks_by_id, rows);
        }
    }
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
    let n = roots.len();
    for (i, &root_id) in roots.iter().enumerate() {
        visit_task(root_id, 0, "", i == n - 1, &visible_ids, &app.collapsed, &tasks_by_id, &mut rows);
    }
    rows
}


fn navigate_to_row(app: &mut App, rows: &[TuiRow], pos: usize) {
    let id = rows[pos].id;
    if let Some(task) = app.task_ref(id) {
        let col = match task.status {
            Status::Todo => Column::Todo,
            Status::Doing => Column::Doing,
            Status::Done => Column::Done,
        };
        app.focused_col = col;
        let col_tasks = app.visible_tasks_for(col);
        if let Some(p) = col_tasks.iter().position(|t| t.id == id) {
            app.cursor[App::col_index(col)] = p;
        }
    }
}

pub fn tree_goto_first(app: &mut App) {
    let rows = build_tui_rows(app);
    if !rows.is_empty() { navigate_to_row(app, &rows, 0); }
}

pub fn tree_goto_last(app: &mut App) {
    let rows = build_tui_rows(app);
    if let Some(last) = rows.len().checked_sub(1) { navigate_to_row(app, &rows, last); }
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
    const SCROLLOFF: usize = 6;
    let rows = build_tui_rows(app);
    let sel_idx = if app.insert.is_some() {
        // Inserting a new task: scroll to keep the inline input row visible.
        inline_insert_row(app).map(|(idx, _)| idx).unwrap_or(0)
    } else {
        app.selected_task_id(app.focused_col)
            .and_then(|id| rows.iter().position(|r| r.id == id))
            .unwrap_or(0)
    };
    if task_area_height == 0 {
        return 0;
    }
    // Clamp scroll so the cursor stays at least SCROLLOFF rows from each edge.
    let min_scroll = (sel_idx + SCROLLOFF + 1).saturating_sub(task_area_height);
    let max_scroll = sel_idx.saturating_sub(SCROLLOFF);
    if min_scroll > max_scroll {
        min_scroll
    } else {
        current_scroll.clamp(min_scroll, max_scroll)
    }
}

fn inline_insert_row(app: &App) -> Option<(usize, TuiRow)> {
    let state = app.insert.as_ref()?;
    let rows = build_tui_rows(app);

    let insert_idx = match &state.position {
        InsertPosition::AtBeginning => 0,
        InsertPosition::AfterParent(after_id) => {
            rows.iter().position(|r| r.id == *after_id)
                .map(|i| i + 1)
                .unwrap_or(rows.len())
        }
        InsertPosition::AfterSibling(after_id) => {
            // Skip past the sibling's full subtree so the insert row
            // appears after all descendants, not between parent and first child.
            if let Some(pos) = rows.iter().position(|r| r.id == *after_id) {
                let sibling_depth = rows[pos].depth;
                let mut end = pos + 1;
                while end < rows.len() && rows[end].depth > sibling_depth {
                    end += 1;
                }
                end
            } else {
                rows.len()
            }
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

    let title = state.title.clone();
    let kind = match state.status {
        Status::Todo => RowKind::Todo,
        Status::Doing => RowKind::Doing,
        Status::Done => RowKind::Done,
    };

    Some((insert_idx, TuiRow { id: Uuid::nil(), title, depth, kind, display_prefix, children_prefix, is_collapsed: false }))
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

/// Push the title as one or more styled spans, highlighting every occurrence of
/// `query` (case-insensitive). `is_current` highlights the first match more brightly.
fn push_title_spans(
    spans: &mut Vec<Span<'static>>,
    title: &str,
    base_style: Style,
    query: &str,
    is_current: bool,
) {
    if query.is_empty() {
        spans.push(Span::styled(title.to_owned(), base_style));
        return;
    }

    let title_chars: Vec<char> = title.chars().collect();
    let lower_title: Vec<char> = title.to_lowercase().chars().collect();
    let lower_query: Vec<char> = query.to_lowercase().chars().collect();
    let qlen = lower_query.len();
    let n = title_chars.len();

    let hl_current = Style::default().fg(Color::Black).bg(Color::Indexed(220)); // bright yellow
    let hl_other   = Style::default().fg(Color::Black).bg(Color::Indexed(58));  // dark olive

    let mut seg_start = 0;
    let mut first_match_done = false;
    let mut i = 0;
    while i + qlen <= n {
        if lower_title[i..i + qlen] == lower_query[..] {
            if i > seg_start {
                let plain: String = title_chars[seg_start..i].iter().collect();
                spans.push(Span::styled(plain, base_style));
            }
            let matched: String = title_chars[i..i + qlen].iter().collect();
            let hl = if is_current && !first_match_done {
                first_match_done = true;
                hl_current
            } else {
                hl_other
            };
            spans.push(Span::styled(matched, hl));
            i += qlen;
            seg_start = i;
        } else {
            i += 1;
        }
    }
    if seg_start < n {
        let tail: String = title_chars[seg_start..].iter().collect();
        spans.push(Span::styled(tail, base_style));
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

    let visual_ids: HashSet<Uuid> = if matches!(app.mode, Mode::Visual) {
        visual_selected_ids(app).into_iter().collect()
    } else {
        HashSet::new()
    };

    // Find which visible row represents the "next" leaf task.
    // If the leaf is hidden behind a fold, bubble up to its nearest visible ancestor.
    let next_row_id: Option<Uuid> = app.first_visible_leaf_id().and_then(|leaf_id| {
        let row_ids: HashSet<Uuid> = rows.iter().map(|r| r.id).collect();
        if row_ids.contains(&leaf_id) {
            return Some(leaf_id);
        }
        let tasks_by_id: HashMap<Uuid, &crate::types::Task> =
            app.tasks.iter().map(|t| (t.id, t)).collect();
        let mut cur = leaf_id;
        loop {
            let parent = tasks_by_id.get(&cur)?.parent_id?;
            if row_ids.contains(&parent) {
                return Some(parent);
            }
            cur = parent;
        }
    });

    // Pre-compute hierarchical labels (e.g. "1", "1.1", "1.2", "2") across all rows
    // so that scrolled-off rows still count correctly.
    let mut depth_counters: Vec<usize> = Vec::new();
    let num_labels: Vec<String> = rows.iter().map(|row| {
        let d = row.depth;
        if d < depth_counters.len() {
            depth_counters.truncate(d + 1);
            depth_counters[d] += 1;
        } else {
            depth_counters.push(1);
        }
        depth_counters.last().map(|n| n.to_string()).unwrap_or_default()
    }).collect();

    let tasks_by_id: HashMap<Uuid, &crate::types::Task> =
        app.tasks.iter().map(|t| (t.id, t)).collect();

    let mut y = area.y;
    for (row_idx, row) in rows.iter().enumerate().skip(scroll_offset).take(area.height as usize) {
        let is_inline = row.id == Uuid::nil();

        let is_selected = selected_id == Some(row.id);
        let is_editing = app.mode == Mode::Insert
            && app.edit.as_ref().map(|es| es.task_id == row.id).unwrap_or(false);

        let flag_bg = if !is_inline {
            let task_flags = app.tasks.iter().find(|t| t.id == row.id).map(|t| t.flags).unwrap_or(0);
            super::flag_bg_for_task(task_flags, &app.flag_active)
        } else {
            None
        };

        let bg = if is_editing {
            Some(Color::Green)
        } else if !visual_ids.is_empty() && visual_ids.contains(&row.id) {
            Some(Color::Indexed(25))  // blue for visual selection
        } else if is_selected {
            Some(Color::Indexed(238))
        } else {
            flag_bg
        };
        let ms = if let Some(bg) = bg { meta_style.bg(bg) } else { meta_style };

        let collapse_indicator = if row.is_collapsed { "▸" } else { "" };
        let num_str = if num_labels[row_idx].is_empty() {
            String::new()
        } else if row.is_collapsed {
            num_labels[row_idx].clone()
        } else {
            format!("{} ", num_labels[row_idx])
        };
        let prefix_chars = row.display_prefix.chars().count()
            + num_str.chars().count()
            + collapse_indicator.chars().count();

        // Progress bar (●/○): one circle per direct visible child.
        let progress_bar: Option<(String, String)> = if !is_inline && !is_editing {
            if let Some(task) = tasks_by_id.get(&row.id) {
                let total = task.children.iter()
                    .filter(|&&cid| tasks_by_id.get(&cid).map(|t| app.task_visible(*t)).unwrap_or(false))
                    .count();
                if total > 0 {
                    let done = task.children.iter()
                        .filter(|&&cid| tasks_by_id.get(&cid)
                            .map(|t| app.task_visible(*t) && t.status == Status::Done)
                            .unwrap_or(false))
                        .count();
                    Some(("●".repeat(done), "○".repeat(total - done)))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let bar_reserve = progress_bar.as_ref().map(|(f, e)| f.len() + e.len() + 1).unwrap_or(0);
        let title_width = (area.width as usize).saturating_sub(prefix_chars + bar_reserve);
        let raw_title = if is_editing {
            app.edit.as_ref().unwrap().title.clone()
        } else {
            row.title.clone()
        };
        let title_text = truncate_to(&raw_title, title_width);

        let mut title_style = title_style_for(row.kind);
        if is_editing {
            title_style = title_style.bg(Color::Green).fg(Color::Black);
        } else if let Some(bg_color) = bg {
            title_style = title_style.bg(bg_color);
            // When the flag background is active (not selection/visual/edit), adjust text contrast
            if flag_bg.is_some() && bg == flag_bg {
                title_style = match row.kind {
                    RowKind::Done => title_style.fg(Color::Indexed(236)), // darker for strikethrough
                    _ => title_style.fg(Color::Indexed(255)),             // lighter for active tasks
                };
            }
        }

        let mut spans: Vec<Span> = Vec::new();
        if !row.display_prefix.is_empty() {
            spans.push(Span::styled(row.display_prefix.clone(), ms));
        }
        if !num_str.is_empty() {
            let is_next = next_row_id == Some(row.id);
            let ns = if is_next {
                let s = Style::default().fg(Color::Indexed(77)).add_modifier(Modifier::BOLD);
                if let Some(bg) = bg { s.bg(bg) } else { s }
            } else if let Some(bg_color) = bg {
                if flag_bg.is_some() && bg == flag_bg {
                    Style::default().fg(Color::Indexed(238)).bg(bg_color)
                } else {
                    num_style.bg(bg_color)
                }
            } else {
                num_style
            };
            spans.push(Span::styled(num_str, ns));
        }
        if !collapse_indicator.is_empty() {
            let cs = Style::default().fg(Color::Indexed(110));
            let cs = if let Some(bg) = bg { cs.bg(bg) } else { cs };
            spans.push(Span::styled(collapse_indicator, cs));
        }

        // Render title with search highlights if a query is active.
        if let Some(ref s) = app.search {
            let is_current = s.matches.get(s.match_idx) == Some(&row.id);
            push_title_spans(&mut spans, &title_text, title_style, &s.query, is_current);
        } else {
            spans.push(Span::styled(title_text, title_style));
        }

        // Clock symbol for tasks whose children are snoozed.
        if !is_inline {
            if let Some(task) = tasks_by_id.get(&row.id) {
                if app.is_clocked(task) {
                    let cs = Style::default().fg(Color::Indexed(226));
                    let cs = if let Some(bg) = bg { cs.bg(bg) } else { cs };
                    spans.push(Span::styled(" ⏰", cs));
                }
            }
        }

        if let Some((filled, empty)) = progress_bar {
            let bar_style = Style::default().fg(Color::Indexed(110));
            let bar_style = if let Some(c) = bg { bar_style.bg(c) } else { bar_style };
            spans.push(Span::styled(format!(" {}{}", filled, empty), bar_style));
        }

        let para_style = if let Some(bg) = bg { Style::default().bg(bg) } else { Style::default() };
        frame.render_widget(
            Paragraph::new(Line::from(spans)).style(para_style),
            Rect { x: area.x, y, width: area.width, height: 1 },
        );

        // Place the terminal cursor (bar style) without touching the rendered text.
        if is_editing {
            if let Some(es) = &app.edit {
                let col = es.cursor_pos.min(es.title.chars().count()).min(title_width);
                let cx = area.x.saturating_add(prefix_chars as u16).saturating_add(col as u16);
                frame.set_cursor_position((cx, y));
            }
        } else if is_inline {
            if let Some(ins) = &app.insert {
                let col = ins.title.chars().count().min(title_width);
                let cx = area.x.saturating_add(prefix_chars as u16).saturating_add(col as u16);
                frame.set_cursor_position((cx, y));
            }
        }

        y += 1;
    }
}

pub fn draw_archive_browser(frame: &mut Frame, app: &App, area: Rect) {
    let Some(ref ab) = app.archive_browser else { return; };
    match ab.view {
        ArchiveView::Calendar => draw_archive_calendar(frame, ab, area),
        ArchiveView::Day      => draw_archive_day(frame, ab, area),
    }
}

fn draw_archive_calendar(frame: &mut Frame, ab: &ArchiveBrowserState, area: Rect) {
    let popup_w: u16 = 28;
    let popup_h: u16 = 13;
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    let popup = Rect { x, y, width: popup_w.min(area.width), height: popup_h.min(area.height) };

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Archive ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(232)));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut row_y = inner.y;

    // Month / year header
    let header = format!("{} {}", month_name(ab.month), ab.year);
    let hx = inner.x + (inner.width.saturating_sub(header.len() as u16)) / 2;
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            header,
            Style::default().fg(Color::Indexed(252)).add_modifier(Modifier::BOLD),
        )])),
        Rect { x: hx, y: row_y, width: inner.width, height: 1 },
    );
    row_y += 1;

    // Day-of-week labels
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "Mo Tu We Th Fr Sa Su",
            Style::default().fg(Color::Indexed(240)),
        )])),
        Rect { x: inner.x + 3, y: row_y, width: inner.width, height: 1 },
    );
    row_y += 1;

    // Calendar grid
    let first = NaiveDate::from_ymd_opt(ab.year, ab.month, 1).unwrap();
    let start_col = first.weekday().num_days_from_monday(); // 0=Mon
    let total_days = calendar_days_in_month(ab.year, ab.month);

    let mut day = 1u32;
    for _row in 0..6 {
        if day > total_days { break; }

        let mut spans: Vec<Span> = vec![Span::raw("   ")]; // 3-char left pad

        let leading = if day == 1 { start_col } else { 0 };
        for _ in 0..leading {
            spans.push(Span::raw("   ")); // blank slot
        }

        for col in leading..7 {
            if day > total_days { break; }

            let date = NaiveDate::from_ymd_opt(ab.year, ab.month, day).unwrap();
            let has_data = ab.available_dates.contains(&date);
            let is_selected = day == ab.selected_day;

            let style = if is_selected {
                Style::default().fg(Color::Black).bg(Color::Indexed(33)).add_modifier(Modifier::BOLD)
            } else if has_data {
                Style::default().fg(Color::Indexed(214)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Indexed(240))
            };

            spans.push(Span::styled(format!("{:2}", day), style));
            if col < 6 { spans.push(Span::raw(" ")); }

            day += 1;
        }

        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
        );
        row_y += 1;
    }

    // Help text at bottom of inner area
    let help_y = inner.y + inner.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " [/] month  Enter view  q close",
            Style::default().fg(Color::Indexed(238)),
        )])),
        Rect { x: inner.x, y: help_y, width: inner.width, height: 1 },
    );
}

fn draw_archive_day(frame: &mut Frame, ab: &ArchiveBrowserState, area: Rect) {
    frame.render_widget(Clear, area);

    let date_str = NaiveDate::from_ymd_opt(ab.year, ab.month, ab.selected_day)
        .map(|d| d.format("%B %-d, %Y").to_string())
        .unwrap_or_default();
    let title = format!(" {}  —  j/k scroll  q back ", date_str);

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(232)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let date = NaiveDate::from_ymd_opt(ab.year, ab.month, ab.selected_day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    let rows = build_archive_rows(&ab.day_tasks, date);

    let mut y = inner.y;
    for row in rows.iter().skip(ab.day_scroll).take(inner.height as usize) {
        let (prefix_style, title_style) = match row.event {
            ArchiveEvent::Completed => (
                Style::default().fg(Color::Indexed(238)),
                Style::default().fg(Color::Indexed(240)).add_modifier(Modifier::CROSSED_OUT),
            ),
            ArchiveEvent::Created => (
                Style::default().fg(Color::Indexed(240)),
                Style::default().fg(Color::Indexed(252)),
            ),
            ArchiveEvent::Context => (
                Style::default().fg(Color::Indexed(236)),
                Style::default().fg(Color::Indexed(238)),
            ),
        };

        let mut spans: Vec<Span> = Vec::new();
        if !row.prefix.is_empty() {
            spans.push(Span::styled(row.prefix.clone(), prefix_style));
        }
        spans.push(Span::styled(row.title.clone(), title_style));

        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect { x: inner.x, y, width: inner.width, height: 1 },
        );
        y += 1;
        if y >= inner.y + inner.height { break; }
    }
}

enum ArchiveEvent { Created, Completed, Context }

struct ArchiveDisplayRow {
    title: String,
    prefix: String,
    event: ArchiveEvent,
}

fn build_archive_rows(tasks: &[crate::archive::ArchiveTask], date: NaiveDate) -> Vec<ArchiveDisplayRow> {
    let by_id: HashMap<Uuid, &crate::archive::ArchiveTask> =
        tasks.iter().map(|t| (t.id, t)).collect();
    let ids: HashSet<Uuid> = tasks.iter().map(|t| t.id).collect();

    let mut roots: Vec<&crate::archive::ArchiveTask> = tasks
        .iter()
        .filter(|t| t.parent_id.map(|pid| !ids.contains(&pid)).unwrap_or(true))
        .collect();
    roots.sort_by_key(|t| t.created_at);

    let mut rows: Vec<ArchiveDisplayRow> = Vec::new();
    let n = roots.len();
    for (i, root) in roots.iter().enumerate() {
        visit_archive_task(root.id, 0, "", i == n - 1, &ids, &by_id, &mut rows, date);
    }
    rows
}

fn visit_archive_task(
    id: Uuid,
    depth: usize,
    parent_prefix: &str,
    is_last: bool,
    all_ids: &HashSet<Uuid>,
    by_id: &HashMap<Uuid, &crate::archive::ArchiveTask>,
    rows: &mut Vec<ArchiveDisplayRow>,
    date: NaiveDate,
) {
    let task = match by_id.get(&id) { Some(t) => t, None => return };

    let (display_prefix, children_prefix) = if depth == 0 {
        (String::new(), String::new())
    } else {
        let connector = if is_last { "╰─ " } else { "├─ " };
        let cont      = if is_last { "   " } else { "│  " };
        (format!("{}{}", parent_prefix, connector), format!("{}{}", parent_prefix, cont))
    };

    let event = if task.completed_at.map(|c| c.date_naive() == date).unwrap_or(false) {
        ArchiveEvent::Completed
    } else if task.created_at.date_naive() == date {
        ArchiveEvent::Created
    } else {
        ArchiveEvent::Context
    };

    rows.push(ArchiveDisplayRow { title: task.title.clone(), prefix: display_prefix, event });

    let mut children: Vec<&crate::archive::ArchiveTask> = by_id
        .values()
        .filter(|t| t.parent_id == Some(id) && all_ids.contains(&t.id))
        .copied()
        .collect();
    children.sort_by_key(|t| t.created_at);

    let n = children.len();
    for (i, child) in children.iter().enumerate() {
        visit_archive_task(
            child.id, depth + 1, &children_prefix, i == n - 1,
            all_ids, by_id, rows, date,
        );
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",  2 => "February", 3 => "March",
        4 => "April",    5 => "May",       6 => "June",
        7 => "July",     8 => "August",    9 => "September",
        10 => "October", 11 => "November", 12 => "December",
        _ => "?",
    }
}

fn calendar_days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    next.unwrap().pred_opt().unwrap().day()
}

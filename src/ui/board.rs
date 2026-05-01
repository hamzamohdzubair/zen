use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Column, InsertPosition, InsertState, Mode};
use crate::types::{Status, Task};

pub fn draw_board(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);

    for (col, rect) in [(Column::Todo, cols[0]), (Column::Doing, cols[2]), (Column::Done, cols[4])] {
        draw_column(frame, app, col, rect);
    }
}

enum DrawItem<'a> {
    Task { task: &'a Task, task_idx: usize },
    Inline,
}

fn compute_ghost_chain(task_id: Uuid, col_ids: &HashSet<Uuid>, app: &App) -> Vec<Uuid> {
    let mut chain = vec![];
    let mut current = task_id;
    loop {
        match app.task_ref(current).and_then(|t| t.parent_id) {
            None => break,
            Some(pid) if col_ids.contains(&pid) => break,
            Some(pid) => { chain.push(pid); current = pid; }
        }
    }
    chain.reverse();
    chain
}

fn is_descendant_of(task_id: Uuid, ancestor_id: Uuid, app: &App) -> bool {
    let mut current = task_id;
    while let Some(pid) = app.task_ref(current).and_then(|t| t.parent_id) {
        if pid == ancestor_id { return true; }
        current = pid;
    }
    false
}

fn inline_insert_index(app: &App, col: Column) -> Option<usize> {
    let state = app.insert.as_ref()?;
    let insert_col = match state.status {
        Status::Todo => Column::Todo,
        Status::Doing => Column::Doing,
        Status::Done => Column::Done,
    };
    if insert_col != col {
        return None;
    }

    let visible = app.visible_tasks_for(col);

    let idx = match &state.position {
        InsertPosition::AtBeginning => 0,
        InsertPosition::AfterSibling(after_id) => {
            if let Some(pos) = visible.iter().position(|t| t.id == *after_id) {
                let mut end = pos + 1;
                while end < visible.len() && is_descendant_of(visible[end].id, *after_id, app) {
                    end += 1;
                }
                end
            } else {
                visible.len()
            }
        }
        InsertPosition::AfterParent(parent_id) => {
            visible.iter().position(|t| t.id == *parent_id)
                .map(|i| i + 1)
                .unwrap_or(0)
        }
    };
    Some(idx)
}

fn draw_column(frame: &mut Frame, app: &App, col: Column, area: Rect) {
    let is_focused = app.focused_col == col;
    let tasks = app.visible_tasks_for(col);
    let count = tasks.iter().filter(|t| t.children.is_empty()).count();
    let cur = app.cursor_for(col);

    let col_ids: HashSet<Uuid> = tasks.iter().map(|t| t.id).collect();

    let ghost_chains: Vec<Vec<Uuid>> = tasks.iter()
        .map(|task| compute_ghost_chain(task.id, &col_ids, app))
        .collect();

    let mut root_count = 0usize;
    let mut child_counts: HashMap<Uuid, usize> = HashMap::new();
    let mut ghost_numbers: HashMap<Uuid, usize> = HashMap::new();
    let mut depth_map: HashMap<Uuid, usize> = HashMap::new();

    // Assign numbers to all ghost ancestors (first encounter wins, outermost first)
    for chain in &ghost_chains {
        for (i, &gid) in chain.iter().enumerate() {
            if ghost_numbers.contains_key(&gid) {
                continue;
            }
            let num = if i == 0 {
                root_count += 1;
                root_count
            } else {
                let parent_gid = chain[i - 1];
                let n = child_counts.entry(parent_gid).or_insert(0);
                *n += 1;
                *n
            };
            ghost_numbers.insert(gid, num);
        }
    }

    let task_numbers: Vec<usize> = tasks.iter().enumerate().map(|(i, task)| {
        let chain = &ghost_chains[i];
        let depth = if chain.is_empty() {
            match task.parent_id {
                None => 0,
                Some(pid) => depth_map.get(&pid).copied().unwrap_or(0) + 1,
            }
        } else {
            chain.len()
        };
        depth_map.insert(task.id, depth);
        if chain.is_empty() {
            match task.parent_id {
                None => { root_count += 1; root_count }
                Some(pid) => {
                    let n = child_counts.entry(pid).or_insert(0);
                    *n += 1;
                    *n
                }
            }
        } else {
            let immediate = *chain.last().unwrap();
            let n = child_counts.entry(immediate).or_insert(0);
            *n += 1;
            *n
        }
    }).collect();

    let header_style = if is_focused {
        Style::default().fg(Color::Indexed(253)).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(242))
    };
    let title = format!(" {} ({}) ", col.status().label(), count);
    frame.render_widget(
        Paragraph::new(Span::styled(title, header_style)),
        Rect { x: area.x, y: area.y, width: area.width, height: 1 },
    );

    let inner = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    let inline_idx = inline_insert_index(app, col);
    let mut items: Vec<DrawItem> = tasks.iter().enumerate()
        .map(|(i, t)| DrawItem::Task { task: t, task_idx: i })
        .collect();
    if let Some(idx) = inline_idx {
        items.insert(idx.min(items.len()), DrawItem::Inline);
    }

    let mut y = inner.y;
    let mut last_ghost_chain: Vec<Uuid> = vec![];

    for item in &items {
        if y >= inner.y + inner.height {
            break;
        }

        match item {
            DrawItem::Task { task, task_idx } => {
                let chain = &ghost_chains[*task_idx];
                let depth = depth_map.get(&task.id).copied().unwrap_or(0);

                if !chain.is_empty() {
                    let common = chain.iter().zip(last_ghost_chain.iter())
                        .take_while(|(a, b)| a == b)
                        .count();
                    for (ci, &gid) in chain.iter().enumerate().skip(common) {
                        if y >= inner.y + inner.height { break; }
                        if let Some(ancestor) = app.task_ref(gid) {
                            let ghost_num = ghost_numbers.get(&gid).copied().unwrap_or(0);
                            draw_ghost_card(frame, ancestor, ghost_num, ci, Rect { x: inner.x, y, width: inner.width, height: 1 });
                            y += 1;
                        }
                    }
                    last_ghost_chain = chain.clone();
                } else {
                    last_ghost_chain = vec![];
                }

                let selected = is_focused && *task_idx == cur;
                let number = task_numbers[*task_idx];

                let is_moving = matches!(app.mode, Mode::Move)
                    && app.move_state.as_ref().map(|ms| ms.task_id == task.id).unwrap_or(false);

                let inline_edit = if matches!(app.mode, Mode::Edit) {
                    app.edit.as_ref().and_then(|es| {
                        if es.task_id == task.id { Some(es.title.as_str()) } else { None }
                    })
                } else {
                    None
                };

                let leaf_count = if task.children.is_empty() {
                    None
                } else {
                    Some(app.undone_leaf_count(task.id))
                };
                draw_card(frame, task, selected, number, depth, is_moving, inline_edit, leaf_count,
                          Rect { x: inner.x, y, width: inner.width, height: 1 });
                y += 1;
            }

            DrawItem::Inline => {
                if let Some(state) = &app.insert {
                    let depth = match state.parent_id {
                        None => 0,
                        Some(pid) if !col_ids.contains(&pid) => {
                            compute_ghost_chain(pid, &col_ids, app).len() + 1
                        }
                        Some(pid) => depth_map.get(&pid).copied().unwrap_or(0) + 1,
                    };
                    draw_inline_card(frame, state, depth, Rect { x: inner.x, y, width: inner.width, height: 1 });
                    y += 1;
                }
            }
        }
    }
}

fn draw_card(
    frame: &mut Frame,
    task: &Task,
    selected: bool,
    number: usize,
    depth: usize,
    is_moving: bool,
    inline_edit: Option<&str>,
    leaf_count: Option<usize>,
    area: Rect,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let active = selected && inline_edit.is_none() && !is_moving;
    let bg = if inline_edit.is_some() {
        Color::Indexed(58)
    } else if is_moving {
        Color::Indexed(23)
    } else {
        project_to_color(&task.project)
    };
    let (fg, num_fg) = if inline_edit.is_some() || is_moving {
        (Color::White, Color::Indexed(246))
    } else if active {
        (Color::Indexed(253), Color::Indexed(253))
    } else {
        (Color::Black, Color::Black)
    };
    let bold = if active { Modifier::BOLD } else { Modifier::empty() };

    let mut spans: Vec<Span> = Vec::new();

    if depth > 0 {
        let prefix = format!("{}╰─", "  ".repeat(depth - 1));
        spans.push(Span::styled(prefix, Style::default().fg(num_fg).bg(bg)));
    }

    let num_str = format!("{} ", number);
    spans.push(Span::styled(num_str, Style::default().fg(num_fg).bg(bg)));

    let title_text = if let Some(input) = inline_edit {
        format!("{}█", input)
    } else {
        task.title.clone()
    };
    spans.push(Span::styled(title_text, Style::default().fg(fg).bg(bg).add_modifier(bold)));

    if let Some(n) = leaf_count {
        let count_str = format!(" ({})", n);
        spans.push(Span::styled(count_str, Style::default().fg(num_fg).bg(bg)));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}

fn draw_ghost_card(frame: &mut Frame, task: &Task, number: usize, depth: usize, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bg = Color::Indexed(234);
    let fg = Color::Indexed(239);
    let mut spans: Vec<Span> = Vec::new();
    if depth > 0 {
        let prefix = format!("{}╰─", "  ".repeat(depth - 1));
        spans.push(Span::styled(prefix, Style::default().fg(fg).bg(bg)));
    }
    spans.push(Span::styled(format!("{} ", number), Style::default().fg(fg).bg(bg)));
    spans.push(Span::styled(task.title.as_str(), Style::default().fg(fg).bg(bg).add_modifier(Modifier::DIM)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}

fn draw_inline_card(frame: &mut Frame, state: &InsertState, depth: usize, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let project_color = project_to_color(&state.project);
    let bg = project_color;
    let fg = Color::Black;
    let num_fg = Color::Indexed(238);

    let mut spans: Vec<Span> = Vec::new();
    if depth > 0 {
        let prefix = format!("{}╰─", "  ".repeat(depth - 1));
        spans.push(Span::styled(prefix, Style::default().fg(num_fg).bg(bg)));
    }
    let text = format!("{}█", state.title);
    spans.push(Span::styled(text, Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD)));

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}

pub fn project_to_color(project: &str) -> Color {
    if project.is_empty() {
        return Color::Indexed(102);
    }
    // 15 pastel-range 256-color indices — muted, clearly distinct hues, dark text readable on all
    // Each maps to a unique hue region; no two entries share the same hue family
    let palette: [u8; 15] = [
        74,  // rgb(95,175,215)  steel blue
        108, // rgb(135,175,135) sage green
        139, // rgb(175,135,175) mauve
        109, // rgb(135,175,175) slate teal
        174, // rgb(215,135,135) soft rose
        179, // rgb(215,175,95)  warm gold
        140, // rgb(175,135,215) lavender
        173, // rgb(215,135,95)  terracotta
        146, // rgb(175,175,215) periwinkle
        107, // rgb(135,175,95)  olive
        103, // rgb(135,135,175) dusty slate
        180, // rgb(215,175,135) warm tan
        115, // rgb(135,215,175) mint
        182, // rgb(215,175,215) orchid
        149, // rgb(175,215,95)  chartreuse
    ];
    let hash: usize = project.bytes().fold(5381usize, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(b as usize)
    });
    Color::Indexed(palette[hash % palette.len()])
}


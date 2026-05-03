use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::app::{App, Column, InsertPosition, InsertState, Mode};
use crate::types::{Status, Task};

fn task_key_char(app: &App, task: &Task) -> char {
    if app.is_unc(task) {
        '`'
    } else if let Some(slot) = app.slot_for_project(&task.project) {
        super::slot_key_char(slot)
    } else {
        '`'
    }
}

fn task_project_name<'a>(app: &'a App, task: &Task) -> Option<&'a str> {
    app.slot_for_project(&task.project)
        .and_then(|slot| app.projects[slot].as_deref())
}

pub fn draw_board(frame: &mut Frame, app: &App, area: Rect) {
    let n_projects = app.projects.iter().filter(|p| p.is_some()).count();
    let summary_h = if n_projects > 0 { n_projects as u16 + 1 } else { 0 }; // +1 for header row

    let (summary_area, board_area) = if summary_h > 0 && summary_h < area.height {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(summary_h), Constraint::Min(0)])
            .split(area);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, area)
    };

    if let Some(sa) = summary_area {
        draw_project_summary(frame, app, sa);
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(board_area);

    for (col, rect) in [(Column::Todo, cols[0]), (Column::Doing, cols[2]), (Column::Done, cols[4])] {
        draw_column(frame, app, col, rect);
    }
}

const BAR_W: usize = 12;

fn draw_project_summary(frame: &mut Frame, app: &App, area: Rect) {
    struct Row { key: char, name: String, rem: usize, pct: usize, pct_str: String }

    let mut rows: Vec<Row> = Vec::new();
    for slot in 0..10 {
        if let Some(name) = &app.projects[slot] {
            let rem = app.doable_count_for_slot(slot);
            let done: usize = app.tasks.iter()
                .filter(|t| &t.project == name && t.status == Status::Done && t.children.is_empty())
                .count();
            let total = rem + done;
            let pct = if total > 0 { done * 100 / total } else { 0 };
            let key = super::slot_key_char(slot);
            rows.push(Row { key, name: name.clone(), rem, pct, pct_str: format!("{}%", pct) });
        }
    }
    if rows.is_empty() { return; }

    // Column widths: data-driven, pct column minimum 4 to always fit "100%"
    // Name cell includes "k:" prefix, so its rendered width is name_w + 4 (" k:name ")
    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(1);
    let rem_w  = rows.iter().map(|r| r.rem.to_string().len()).max().unwrap_or(1);
    let pct_w  = rows.iter().map(|r| r.pct_str.len()).max().unwrap_or(4).max(4);

    let dim = Style::default().fg(Color::Indexed(240));

    // Header row — name cell padded to match data cell width (name_w + 4)
    if area.height >= 1 {
        let header = Line::from(vec![
            Span::styled(format!("  {:width$}  ", "", width = name_w), dim),
            Span::styled(format!(" {:>width$}", "r", width = rem_w), dim),
            Span::styled(format!("  {:>width$} ", "%", width = pct_w), dim),
        ]);
        frame.render_widget(
            Paragraph::new(header),
            Rect { x: area.x, y: area.y, width: area.width, height: 1 },
        );
    }

    // One data row per project
    // Layout: " {key}:{name} {rem}  {pct}  {bar} "
    let table_w = (4 + name_w + 1 + rem_w + 2 + pct_w + 2 + BAR_W + 1) as u16;
    for (i, row) in rows.iter().enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.y + area.height { break; }
        let color = project_to_color(&row.name);
        let filled = row.pct * BAR_W / 100;
        let empty  = BAR_W - filled;
        let data_row = Line::from(vec![
            Span::styled(
                format!(" {}:{:<width$} ", row.key, row.name, width = name_w),
                Style::default().fg(Color::Black).bg(color),
            ),
            Span::styled(
                format!(" {:>width$}", row.rem, width = rem_w),
                Style::default().fg(Color::Indexed(250)),
            ),
            Span::styled(
                format!("  {:>width$} ", row.pct_str, width = pct_w),
                Style::default().fg(Color::Indexed(246)),
            ),
            Span::styled(
                " ".to_string(),
                Style::default(),
            ),
            Span::styled(
                "█".repeat(filled),
                Style::default().fg(color),
            ),
            Span::styled(
                "░".repeat(empty),
                Style::default().fg(Color::Indexed(237)),
            ),
            Span::styled(" ".to_string(), Style::default()),
        ]);
        frame.render_widget(
            Paragraph::new(data_row),
            Rect { x: area.x, y, width: table_w.min(area.width), height: 1 },
        );
    }
}

enum DrawItem<'a> {
    Task { task: &'a Task, task_idx: usize },
    Inline,
}

fn wrap_height(prefix_chars: usize, content_chars: usize, width: u16) -> u16 {
    if width == 0 { return 1; }
    let w = width as usize;
    let total = prefix_chars + content_chars;
    if total <= w { return 1; }
    let overflow = total - w;
    (1 + (overflow + w - 1) / w) as u16
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

    let visible = app.board_tasks_for(col);

    let idx = match &state.position {
        InsertPosition::AtBeginning => 0,
        InsertPosition::AfterSibling(after_id) => {
            visible.iter().position(|t| t.id == *after_id)
                .map(|i| i + 1)
                .unwrap_or(visible.len())
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
    let tasks = app.board_tasks_for(col);
    let count = tasks.len();
    let cur = app.cursor_for(col);

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
    for item in &items {
        if y >= inner.y + inner.height {
            break;
        }

        match item {
            DrawItem::Task { task, task_idx } => {
                let selected = is_focused && *task_idx == cur;

                let is_moving = matches!(app.mode, Mode::Move)
                    && app.move_state.as_ref().map(|ms| ms.task_id == task.id).unwrap_or(false);

                let inline_edit = if matches!(app.mode, Mode::Insert) && app.edit.is_some() {
                    app.edit.as_ref().and_then(|es| {
                        if es.task_id == task.id { Some(es.title.as_str()) } else { None }
                    })
                } else {
                    None
                };

                let project_key = task_key_char(app, task);
                let project_name = task_project_name(app, task);

                let h = if inline_edit.is_some() {
                    1
                } else {
                    let content = task.title.chars().count();
                    let prefix_chars = project_name.map_or(3, |n| n.len() + 4);
                    wrap_height(prefix_chars, content, inner.width)
                }.min(inner.y + inner.height - y);

                draw_card(frame, task, project_key, project_name, selected, is_moving, inline_edit,
                          Rect { x: inner.x, y, width: inner.width, height: h });
                y += h;
            }

            DrawItem::Inline => {
                if let Some(state) = &app.insert {
                    draw_inline_card(frame, state, Rect { x: inner.x, y, width: inner.width, height: 1 });
                    y += 1;
                }
            }
        }
    }
}

fn draw_card(
    frame: &mut Frame,
    task: &Task,
    project_key: char,
    project_name: Option<&str>,
    selected: bool,
    is_moving: bool,
    inline_edit: Option<&str>,
    area: Rect,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let active = selected && inline_edit.is_none() && !is_moving;
    let bg = if inline_edit.is_some() {
        Color::Green
    } else if is_moving {
        Color::Indexed(23)
    } else {
        project_to_color(&task.project)
    };
    let fg = if is_moving {
        Color::White
    } else if inline_edit.is_some() {
        Color::Black
    } else if active {
        Color::Indexed(253)
    } else {
        Color::Black
    };
    let bold = if active { Modifier::BOLD } else { Modifier::empty() };

    let spans = if let Some(input) = inline_edit {
        let text = format!("{}█", input);
        vec![Span::styled(text, Style::default().fg(fg).bg(bg).add_modifier(bold))]
    } else {
        let prefix = match project_name {
            Some(name) => format!("{}:{}: ", project_key, name),
            None => format!("{}: ", project_key),
        };
        vec![
            Span::styled(prefix, Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD)),
            Span::styled(task.title.clone(), Style::default().fg(fg).bg(bg).add_modifier(bold)),
        ]
    };

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_inline_card(frame: &mut Frame, state: &InsertState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bg = Color::Green;
    let fg = Color::Black;

    let text = format!("{}█", state.title);
    let spans = vec![Span::styled(text, Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD))];

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)).wrap(Wrap { trim: false }),
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

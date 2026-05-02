use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

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

                let h = if inline_edit.is_some() {
                    1
                } else {
                    let content = task.title.chars().count();
                    wrap_height(0, content, inner.width)
                }.min(inner.y + inner.height - y);

                draw_card(frame, task, selected, is_moving, inline_edit,
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
        Color::Indexed(120)
    } else if is_moving {
        Color::Indexed(23)
    } else {
        project_to_color(&task.project)
    };
    let (fg, _) = if inline_edit.is_some() || is_moving {
        (Color::White, Color::Indexed(246))
    } else if active {
        (Color::Indexed(253), Color::Indexed(253))
    } else {
        (Color::Black, Color::Black)
    };
    let bold = if active { Modifier::BOLD } else { Modifier::empty() };

    let title_text = if let Some(input) = inline_edit {
        format!("{}█", input)
    } else {
        task.title.clone()
    };

    let spans = vec![Span::styled(title_text, Style::default().fg(fg).bg(bg).add_modifier(bold))];

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)).wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_inline_card(frame: &mut Frame, state: &InsertState, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let bg = Color::Indexed(120);
    let fg = Color::Black;

    let text = format!("{}█", state.title);
    let spans = vec![Span::styled(text, Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD))];

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

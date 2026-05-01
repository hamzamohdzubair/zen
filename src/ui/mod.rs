mod board;
mod filter;
mod move_popup;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, Mode};
use board::project_to_color;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    board::draw_board(frame, app, chunks[0]);
    draw_status(frame, app, chunks[1]);

    if let Mode::Filter = &app.mode {
        let popup = filter::popup_rect(app, area);
        filter::draw_filter_popup(frame, app, popup);
    }
    if let Mode::Move = &app.mode {
        if app.move_state.as_ref().map(|ms| ms.suggestion_cursor.is_some()).unwrap_or(false) {
            let popup = move_popup::popup_rect(app, area);
            move_popup::draw_move_popup(frame, app, popup);
        }
    }
}

const SEP: &str = "│";

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode_str = match &app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Edit => "EDIT",
        Mode::Filter => "FILTER",
        Mode::Move => "MOVE",
        Mode::Confirm(_) => "CONFIRM",
    };

    let sep_style = Style::default().fg(Color::Indexed(240));

    let mut spans = vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().fg(Color::Black).bg(mode_color(&app.mode)),
        ),
    ];

    // Inline prompt between mode pill and project pills
    match &app.mode {
        Mode::Move => {
            if let Some(ms) = &app.move_state {
                spans.push(Span::styled(SEP, sep_style));
                spans.push(Span::styled(
                    format!(" {}█ ", ms.target_input),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ));
            }
        }
        Mode::Confirm(action) => {
            use crate::app::ConfirmAction;
            spans.push(Span::styled(SEP, sep_style));
            let label = match action {
                ConfirmAction::DeleteTask(id) => {
                    let title = app.task_ref(*id)
                        .map(|t| t.title.as_str())
                        .unwrap_or("task");
                    format!(" delete \"{}\"?  Enter / Esc ", title)
                }
            };
            spans.push(Span::styled(label, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
        }
        _ => {}
    }

    spans.push(Span::styled(SEP, sep_style));

    // All projects with "none" pinned first, no spaces between pills
    let mut projects = app.all_projects();
    if let Some(idx) = projects.iter().position(|p| p == "none") {
        let none = projects.remove(idx);
        projects.insert(0, none);
    }

    for proj in &projects {
        let is_active = app.active_projects.is_empty() || app.active_projects.contains(proj);
        let color = project_to_color(proj);
        let pill_style = if is_active {
            Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(235))
        };
        spans.push(Span::styled(format!(" {} ", proj), pill_style));
    }

    if let Some(msg) = &app.status_message {
        spans.push(Span::styled(
            format!("  {}", msg),
            Style::default().fg(Color::DarkGray),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn mode_color(mode: &Mode) -> Color {
    match mode {
        Mode::Normal => Color::Blue,
        Mode::Insert => Color::Green,
        Mode::Edit => Color::Yellow,
        Mode::Filter => Color::Magenta,
        Mode::Move => Color::Cyan,
        Mode::Confirm(_) => Color::Red,
    }
}

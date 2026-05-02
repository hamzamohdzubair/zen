mod board;
mod help;
pub mod done;
pub mod stats;
pub mod tui;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, BulkInsertStep, KanbanSort, Mode, ViewMode};
use board::project_to_color;

pub fn pill_span(key: char, name: &str, count: usize, active: bool, color: Color) -> Span<'static> {
    let style = if active {
        Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(235))
    };
    Span::styled(format!(" {}:{} ({}) ", key, name, count), style)
}

pub fn unc_pill_span(count: usize, active: bool) -> Span<'static> {
    let style = if active {
        Style::default().fg(Color::Black).bg(Color::Indexed(102)).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(235))
    };
    Span::styled(format!(" `:unc({}) ", count), style)
}

pub fn slot_key_char(slot: usize) -> char {
    if slot == 9 { '0' } else { (b'1' + slot as u8) as char }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    match app.view_mode {
        ViewMode::Tree => tui::draw_tui(frame, app, chunks[0]),
        ViewMode::Board => board::draw_board(frame, app, chunks[0]),
    }
    draw_status(frame, app, chunks[1]);

    if matches!(app.mode, Mode::Help) {
        help::draw_help(frame, app.view_mode);
    }
}

const SEP: &str = "│";

pub fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode_str = match &app.mode {
        Mode::Normal => match app.view_mode {
            ViewMode::Tree => "PLAN",
            ViewMode::Board => "ACTION",
        },
        Mode::Insert => "INSERT",
        Mode::Edit => "EDIT",
        Mode::Move => "MOVE",
        Mode::ProjectEdit => "PROJ",
        Mode::Confirm(_) => "CONFIRM",
        Mode::Help => "HELP",
        Mode::BulkInsert => "BULK",
    };

    let sep_style = Style::default().fg(Color::Indexed(240));

    let mut spans = vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().fg(Color::Black).bg(mode_color_for(app)),
        ),
    ];

    // Sort pill — only shown in kanban mode
    if app.view_mode == ViewMode::Board && matches!(app.mode, Mode::Normal) {
        let (sort_label, sort_color) = match app.kanban_sort {
            KanbanSort::Age     => (" AGE ", Color::Indexed(67)),
            KanbanSort::Project => (" PRO ", Color::Indexed(64)),
        };
        spans.push(Span::styled(sort_label, Style::default().fg(Color::Black).bg(sort_color)));
    }

    match &app.mode {
        Mode::Move => {
            spans.push(Span::styled(SEP, sep_style));
            spans.push(Span::styled(
                " 0-9 ",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
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
        Mode::BulkInsert => {
            spans.push(Span::styled(SEP, sep_style));
            if let Some(ref bs) = app.bulk_insert {
                let label = match bs.step {
                    BulkInsertStep::Num => format!(" num: {}\u{2588} ", bs.num_input),
                    BulkInsertStep::Prefix => format!(" prefix: {}\u{2588} ", bs.prefix_input),
                };
                spans.push(Span::styled(label, Style::default().fg(Color::Indexed(208)).add_modifier(Modifier::BOLD)));
            }
        }
        _ => {}
    }

    spans.push(Span::styled(SEP, sep_style));

    if matches!(app.mode, Mode::ProjectEdit) {
        // Show all 10 slots with cursor on editing slot
        let pe = app.project_edit.as_ref().unwrap();
        for slot in 0..10 {
            let key = slot_key_char(slot);
            let (label, style) = if slot == pe.slot {
                let text = format!(" {}:{}\u{2588} ", key, pe.input);
                (text, Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                match &app.projects[slot] {
                    Some(name) => {
                        let color = project_to_color(name);
                        let text = format!(" {}:{} ", key, name);
                        (text, Style::default().fg(Color::Black).bg(color))
                    }
                    None => {
                        let text = format!(" {}: ", key);
                        (text, Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(235)))
                    }
                }
            };
            spans.push(Span::styled(label, style));
        }
    } else {
        // Show unc pill (leftmost) if there are any unc tasks
        if app.has_unc_tasks() {
            spans.push(unc_pill_span(app.unc_doable_count(), app.show_unc));
        }

        // Show named project pills for non-empty slots
        for slot in 0..10 {
            if let Some(name) = &app.projects[slot] {
                let key = slot_key_char(slot);
                let count = app.doable_count_for_slot(slot);
                let color = project_to_color(name);
                spans.push(pill_span(key, name, count, app.active_slots[slot], color));
            }
        }
    }

    if let Some(msg) = &app.status_message {
        spans.push(Span::styled(
            format!("  {}", msg),
            Style::default().fg(Color::DarkGray),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn mode_color_for(app: &App) -> Color {
    match &app.mode {
        Mode::Normal => match app.view_mode {
            ViewMode::Tree => Color::Indexed(33),   // blue — planning
            ViewMode::Board => Color::Indexed(208),  // orange — action
        },
        mode => mode_color(mode),
    }
}

fn mode_color(mode: &Mode) -> Color {
    match mode {
        Mode::Normal => Color::Blue,
        Mode::Insert => Color::Green,
        Mode::Edit => Color::Yellow,
        Mode::Move => Color::Cyan,
        Mode::ProjectEdit => Color::Magenta,
        Mode::Confirm(_) => Color::Red,
        Mode::Help => Color::Indexed(240),
        Mode::BulkInsert => Color::Indexed(208),
    }
}

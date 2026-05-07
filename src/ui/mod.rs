mod help;
pub mod done;
pub mod snaps;
pub mod stats;
pub mod tui;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{ActiveLayer, App, BulkInsertStep, Mode};
use snaps::draw_snap_popup;


/// Return the background color for flag `idx` (0-indexed).
pub fn flag_color(idx: usize) -> Color {
    match idx {
        0 => Color::Indexed(68),  // cornflower blue  #5f87d7
        1 => Color::Indexed(179), // warm gold        #d7af5f
        2 => Color::Indexed(175), // dusty rose       #d787af
        _ => Color::Reset,
    }
}

/// Return the highlight color if the task has any active flag, otherwise `None`.
pub fn flag_bg_for_task(task_flags: u8, flag_active: &[bool; 3]) -> Option<Color> {
    (0..3).find(|&i| flag_active[i] && (task_flags >> i) & 1 == 1)
          .map(flag_color)
}

pub fn flag_pill_span(idx: usize, active: bool) -> Span<'static> {
    let color = flag_color(idx);
    let style = if active {
        Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(235))
    };
    Span::styled(format!(" \u{2691}{} ", idx + 1), style)
}

/// Layer pill colors — very dark background tints, white text.
pub fn layer_color(layer: ActiveLayer) -> Color {
    match layer {
        ActiveLayer::Foreground => Color::Indexed(22),  // very dark green
        ActiveLayer::Background => Color::Indexed(58),  // very dark yellow/olive
        ActiveLayer::Archive    => Color::Indexed(52),  // very dark red
    }
}

fn layer_pill_span(label: &str, key: char, count: usize, active: bool, layer: ActiveLayer) -> Span<'static> {
    let bg = layer_color(layer);
    let style = if active {
        Style::default().fg(Color::Indexed(252)).bg(bg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Indexed(240)).bg(Color::Indexed(233))
    };
    Span::styled(format!(" {}:{} ({}) ", key, label, count), style)
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    draw_status(frame, app, chunks[0]);
    tui::draw_tui(frame, app, chunks[1]);

    if matches!(app.mode, Mode::Help) {
        help::draw_help(frame);
    }

    if let Some(ref mut popup) = app.snap_popup {
        draw_snap_popup(frame, popup);
    }
}

const SEP: &str = "│";

pub fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode_str = match &app.mode {
        Mode::Normal        => "PLAN",
        Mode::Insert        => "INSERT",
        Mode::Help          => "HELP",
        Mode::BulkInsert    => "BULK",
        Mode::Visual        => "VISUAL",
        Mode::SnapBrowser   => "SNAPS",
        Mode::SubmergeInput => "SNOOZE",
    };

    let sep_style = Style::default().fg(Color::Indexed(240));

    let mut spans = vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().fg(Color::Black).bg(mode_color_for(app)),
        ),
    ];

    match &app.mode {
        Mode::BulkInsert => {
            spans.push(Span::styled(SEP, sep_style));
            if let Some(ref bs) = app.bulk_insert {
                let label = match bs.step {
                    BulkInsertStep::Num    => format!(" num: {}\u{2588} ", bs.num_input),
                    BulkInsertStep::Prefix => format!(" prefix: {}\u{2588} ", bs.prefix_input),
                };
                spans.push(Span::styled(label, Style::default().fg(Color::Indexed(208)).add_modifier(Modifier::BOLD)));
            }
        }
        Mode::SubmergeInput => {
            spans.push(Span::styled(SEP, sep_style));
            if let Some(ref si) = app.submerge_input {
                spans.push(Span::styled(
                    format!(" {}\u{2588} ", si.input),
                    Style::default().fg(Color::Indexed(226)).add_modifier(Modifier::BOLD),
                ));
            }
        }
        _ => {}
    }

    spans.push(Span::styled(SEP, sep_style));

    // Layer pills
    let fg_count = app.count_in_layer(ActiveLayer::Foreground);
    let bg_count = app.count_in_layer(ActiveLayer::Background);
    let ar_count = app.count_in_layer(ActiveLayer::Archive);
    spans.push(layer_pill_span("FG", '1', fg_count, app.active_layer == ActiveLayer::Foreground, ActiveLayer::Foreground));
    spans.push(layer_pill_span("BG", '2', bg_count, app.active_layer == ActiveLayer::Background, ActiveLayer::Background));
    spans.push(layer_pill_span("ARC", '3', ar_count, app.active_layer == ActiveLayer::Archive, ActiveLayer::Archive));

    // Flag pills — visible in all modes except help
    if !matches!(app.mode, Mode::Help) {
        spans.push(Span::styled(SEP, sep_style));
        for i in 0..3 {
            spans.push(flag_pill_span(i, app.flag_active[i]));
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
        Mode::Normal => Color::Indexed(33),
        mode => mode_color(mode),
    }
}

fn mode_color(mode: &Mode) -> Color {
    match mode {
        Mode::Normal        => Color::Blue,
        Mode::Insert        => Color::Green,
        Mode::Help          => Color::Indexed(240),
        Mode::BulkInsert    => Color::Indexed(208),
        Mode::Visual        => Color::Indexed(25),
        Mode::SnapBrowser   => Color::Indexed(33),
        Mode::SubmergeInput => Color::Indexed(226),
    }
}

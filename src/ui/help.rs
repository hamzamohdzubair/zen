use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("j / k", "move cursor down / up"),
            ("] / [", "next / previous layer (FG → BG → ARC)"),
            ("1 / 2 / 3", "jump to Foreground / Background / Archive"),
        ],
    ),
    (
        "Task editing",
        &[
            ("o / O", "new task below / above"),
            ("I / A", "edit title at start / end"),
            ("i / a", "edit title at 25% / 75%"),
            ("d", "delete task"),
            ("K / J", "reorder task up / down"),
            ("> / <", "indent / promote task"),
            ("M", "bulk add children"),
        ],
    ),
    (
        "Layer operations",
        &[
            ("b", "submerge to background (prompt for duration)"),
            ("x", "bury to archive"),
            ("e", "emerge / surface from background"),
            ("r", "restore from archive (or redo in FG)"),
            ("p", "peek: toggle background ancestors visible"),
        ],
    ),
    (
        "Status",
        &[
            ("s", "toggle doing"),
            ("c", "toggle done"),
        ],
    ),
    (
        "Folds",
        &[
            ("z m", "fold all"),
            ("z r", "unfold all"),
            ("z g", "unfold first leaf path globally"),
            ("z l", "unfold first leaf path in current root"),
            ("z . / z ,", "cycle leaf focus to next / prev root"),
        ],
    ),
    (
        "General",
        &[
            ("F", "cycle flag highlights"),
            ("S", "save snapshot"),
            ("?", "open / close help"),
            ("q", "quit"),
        ],
    ),
];

pub fn draw_help(frame: &mut Frame) {
    let area = centered_rect(66, 85, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help  —  ? / Esc to close ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::Indexed(235)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (section, bindings) in SECTIONS {
        lines.push(Line::from(Span::styled(
            format!("  {}  ", section),
            Style::default()
                .fg(Color::Indexed(189))
                .bg(Color::Indexed(238))
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        for (key, desc) in *bindings {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:>14}  ", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(Color::Indexed(252))),
            ]));
        }
        lines.push(Line::from(""));
    }

    let half = lines.len().div_ceil(2);
    let left_lines = lines[..half].to_vec();
    let right_lines = lines[half..].to_vec();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(left_lines).style(Style::default().bg(Color::Indexed(235))),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(right_lines).style(Style::default().bg(Color::Indexed(235))),
        cols[1],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}

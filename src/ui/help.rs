use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

const SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("h / ←", "focus left column"),
            ("l / →", "focus right column"),
            ("k / ↑", "move cursor up"),
            ("j / ↓", "move cursor down"),
        ],
    ),
    (
        "Cards",
        &[
            ("o", "insert card below"),
            ("O", "insert card above"),
            ("i", "edit selected card"),
            ("d", "delete selected card"),
            ("H / L", "move card left / right"),
            ("K / J", "reorder card up / down"),
            ("> / <", "indent / promote card"),
            ("m", "assign card to project"),
        ],
    ),
    (
        "Projects",
        &[
            ("1-9 / 0", "toggle project filter"),
            ("=", "enable all projects"),
            ("-", "disable all projects"),
            ("`", "toggle unclassified"),
            ("P", "edit project slot names"),
        ],
    ),
    (
        "General",
        &[
            ("?", "open / close this help"),
            ("q", "quit"),
        ],
    ),
];

pub fn draw_help(frame: &mut Frame) {
    let area = centered_rect(62, 80, frame.area());

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
                .fg(Color::Black)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        for (key, desc) in *bindings {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:>12}  ", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(Color::Gray)),
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

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::app::ViewMode;

const PLANNING_SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("h / ←", "previous project"),
            ("l / →", "next project"),
            ("k / ↑", "move cursor up"),
            ("j / ↓", "move cursor down"),
        ],
    ),
    (
        "Planning",
        &[
            ("o", "new task below"),
            ("O", "new task above"),
            ("i", "edit task title"),
            ("d", "delete task"),
            ("K / J", "reorder task up / down"),
            ("> / <", "indent / promote task"),
            ("m, then 1-9", "assign task to project"),
            ("A", "bulk add children"),
        ],
    ),
    (
        "Insert Mode",
        &[
            ("Enter", "confirm and create task"),
            ("Esc", "cancel"),
            ("Tab", "indent  →  make child of task above"),
            ("Shift-Tab", "unindent  →  promote to parent level"),
        ],
    ),
    (
        "Projects",
        &[
            ("P", "edit project slots  (← → pick, Enter save)"),
            ("1-9 / 0", "toggle project filter"),
            ("=", "enable all filters"),
            ("-", "disable all filters"),
            ("`", "toggle unclassified tasks"),
        ],
    ),
    (
        "General",
        &[
            ("Tab", "back to action mode  (kanban)"),
            ("?", "open / close help"),
            ("q", "quit"),
        ],
    ),
];

const ACTION_SECTIONS: &[(&str, &[(&str, &str)])] = &[
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
        "Action",
        &[
            ("L", "advance task  →  (todo → doing → done)"),
            ("H", "revert task  ←  (done → doing → todo)"),
            ("Enter", "open project in planning mode  (tree)"),
        ],
    ),
    (
        "Projects",
        &[
            ("P", "edit project slots  (← → pick, Enter save)"),
            ("1-9 / 0", "toggle project filter"),
            ("=", "enable all filters"),
            ("-", "disable all filters"),
            ("`", "toggle unclassified tasks"),
        ],
    ),
    (
        "General",
        &[
            ("?", "open / close help"),
            ("q", "quit"),
        ],
    ),
];

pub fn draw_help(frame: &mut Frame, view_mode: ViewMode) {
    let sections = match view_mode {
        ViewMode::Tree => PLANNING_SECTIONS,
        ViewMode::Board => ACTION_SECTIONS,
    };

    let area = centered_rect(66, 85, frame.area());

    frame.render_widget(Clear, area);

    let title = match view_mode {
        ViewMode::Tree => " Planning Mode  —  ? / Esc to close ",
        ViewMode::Board => " Action Mode  —  ? / Esc to close ",
    };

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(Color::Indexed(235)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = vec![Line::from("")];

    for (section, bindings) in sections {
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

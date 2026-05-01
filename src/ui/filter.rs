use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};

use crate::app::App;
use crate::types::segment_boundary_matches;
use super::board::project_to_color;

pub fn draw_filter_popup(frame: &mut Frame, app: &App, area: Rect) {
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let query = app.filter.as_ref().map(|f| f.input.as_str()).unwrap_or("");
    let projects = app.all_projects();

    let items: Vec<ListItem> = projects
        .iter()
        .map(|p| {
            let color = project_to_color(p);
            let matches = segment_boundary_matches(p, query);
            if query.is_empty() {
                ListItem::new(Line::from(Span::styled(p.as_str(), Style::default().fg(color))))
            } else if let Some(&pos) = matches.first() {
                let qlen = query.len();
                let before = &p[..pos];
                let highlighted = &p[pos..pos + qlen];
                let after = &p[pos + qlen..];
                ListItem::new(Line::from(vec![
                    Span::styled(before, Style::default().fg(color)),
                    Span::styled(highlighted, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(after, Style::default().fg(color)),
                ]))
            } else {
                ListItem::new(Line::from(Span::styled(p.as_str(), Style::default().fg(Color::Indexed(240)))))
            }
        })
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    frame.render_widget(list, inner);
}

/// Compute a popup rect that snugly fits the project list.
pub fn popup_rect(app: &App, screen: Rect) -> Rect {
    let projects = app.all_projects();
    let max_name = projects.iter().map(|p| p.len()).max().unwrap_or(8) as u16;
    let width = (max_name + 4).min(screen.width);   // 2 borders + 2 inner padding
    let height = (projects.len() as u16 + 2).min(screen.height); // 2 borders
    let x = screen.x + screen.width.saturating_sub(width) / 2;
    let y = screen.y + screen.height.saturating_sub(height) / 2;
    Rect { x, y, width, height }
}

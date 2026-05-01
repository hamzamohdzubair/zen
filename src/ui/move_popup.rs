use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};

use crate::app::App;
use super::board::project_to_color;

pub fn draw_move_popup(frame: &mut Frame, app: &App, area: Rect) {
    let ms = match app.move_state.as_ref() {
        Some(ms) if ms.suggestion_cursor.is_some() => ms,
        _ => return,
    };

    let suggestions = app.move_suggestions();
    let cursor = ms.suggestion_cursor.unwrap_or(0);
    let query = ms.suggestion_query.as_deref().unwrap_or(&ms.target_input);

    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let color = project_to_color(p);
            let qlen = query.len().min(p.len());
            let before = &p[..qlen];
            let after = &p[qlen..];
            let selected = i == cursor;
            let bg = if selected { Color::Indexed(237) } else { Color::Reset };
            ListItem::new(Line::from(vec![
                Span::styled(before, Style::default().fg(Color::White).add_modifier(Modifier::BOLD).bg(bg)),
                Span::styled(after, Style::default().fg(color).bg(bg)),
            ]))
        })
        .collect();

    frame.render_widget(List::new(items), inner);
}

pub fn popup_rect(app: &App, screen: Rect) -> Rect {
    let ms = match app.move_state.as_ref() {
        Some(ms) => ms,
        None => return Rect::default(),
    };
    let suggestions = app.move_suggestions();
    let max_name = suggestions.iter().map(|p| p.len()).max()
        .unwrap_or(ms.target_input.len())
        .max(ms.target_input.len()) as u16;
    let width = (max_name + 4).max(16).min(screen.width / 2);
    // Cap at screen height minus status bar (1) minus borders (2)
    let list_h = (suggestions.len() as u16).min(screen.height.saturating_sub(3));
    let height = list_h + 2; // borders
    // Anchor just above the status bar (last line of screen)
    let x = screen.x;
    let y = screen.y + screen.height.saturating_sub(height + 1);
    Rect { x, y, width, height }
}

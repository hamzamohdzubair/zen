//! Interactive TUI for fuzzy search

use anyhow::{Context, Result};
use chrono::Utc;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::finder::{Finder, SearchResult};

/// Action to take after handling an event
enum AppAction {
    Continue,
    Edit(String), // Card ID to edit
    Exit,
}

/// TUI application for finding cards
pub struct FinderApp {
    finder: Finder,
    query: String,
    results: Vec<SearchResult>,
    selected_index: usize,
    list_state: ListState,
}

impl FinderApp {
    /// Create a new finder app with an initial query
    pub fn new(initial_query: &str) -> Result<Self> {
        let mut finder = Finder::new().context("Failed to create finder")?;

        let query = initial_query.to_string();
        let results = finder.search(&query);

        let mut list_state = ListState::default();
        if !results.is_empty() {
            list_state.select(Some(0));
        }

        Ok(Self {
            finder,
            query,
            results,
            selected_index: 0,
            list_state,
        })
    }

    /// Update the search query and re-run search
    fn update_query(&mut self, new_query: String) {
        self.query = new_query;
        self.results = self.finder.search(&self.query);
        self.selected_index = 0;
        if !self.results.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    /// Run the TUI event loop
    /// Returns Some(card_id) if user wants to edit, None if cancelled
    pub fn run(&mut self) -> Result<Option<String>> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).context("Failed to enter alternate screen")?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        terminal.hide_cursor().context("Failed to hide cursor")?;

        let result = self.run_event_loop(&mut terminal);

        // Cleanup
        terminal.show_cursor().context("Failed to show cursor")?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)
            .context("Failed to leave alternate screen")?;
        disable_raw_mode().context("Failed to disable raw mode")?;

        result
    }

    fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<Option<String>> {
        loop {
            terminal
                .draw(|f| self.render(f))
                .context("Failed to draw frame")?;

            if event::poll(std::time::Duration::from_millis(100))
                .context("Failed to poll events")?
            {
                if let Event::Key(key) = event::read().context("Failed to read event")? {
                    match self.handle_key_event(key) {
                        AppAction::Continue => {}
                        AppAction::Edit(card_id) => return Ok(Some(card_id)),
                        AppAction::Exit => return Ok(None),
                    }
                }
            }
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            // Ctrl+n: select next
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.select_next();
                AppAction::Continue
            }
            // Ctrl+p: select previous
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.select_prev();
                AppAction::Continue
            }
            // Ctrl+c: exit
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => AppAction::Exit,
            // Tab: select next
            KeyCode::Tab if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.select_next();
                AppAction::Continue
            }
            // Shift+Tab (BackTab): select previous
            KeyCode::BackTab => {
                self.select_prev();
                AppAction::Continue
            }
            // Character input - add to query (no modifiers or just shift)
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                let mut new_query = self.query.clone();
                new_query.push(c);
                self.update_query(new_query);
                AppAction::Continue
            }
            // Backspace - remove last character
            KeyCode::Backspace => {
                let mut new_query = self.query.clone();
                new_query.pop();
                self.update_query(new_query);
                AppAction::Continue
            }
            // Down arrow: select next
            KeyCode::Down => {
                self.select_next();
                AppAction::Continue
            }
            // Up arrow: select previous
            KeyCode::Up => {
                self.select_prev();
                AppAction::Continue
            }
            // Enter: edit selected card
            KeyCode::Enter => {
                if let Some(card) = self.results.get(self.selected_index) {
                    AppAction::Edit(card.card.id.clone())
                } else {
                    AppAction::Continue
                }
            }
            // ESC: exit
            KeyCode::Esc => AppAction::Exit,
            _ => AppAction::Continue,
        }
    }

    fn select_next(&mut self) {
        if self.results.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.results.len();
        self.list_state.select(Some(self.selected_index));
    }

    fn select_prev(&mut self) {
        if self.results.is_empty() {
            return;
        }
        if self.selected_index == 0 {
            self.selected_index = self.results.len() - 1;
        } else {
            self.selected_index -= 1;
        }
        self.list_state.select(Some(self.selected_index));
    }

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Use full screen
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Search box
                Constraint::Min(0),     // Results and preview
            ])
            .split(size);

        // Render search input
        self.render_search_input(frame, main_chunks[0]);

        // Split horizontally: 40% left (list), 60% right (preview + stats)
        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(main_chunks[1]);

        // Render results list
        self.render_list(frame, bottom_chunks[0]);

        // Split right side vertically: preview on top, stats on bottom
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60), // Preview
                Constraint::Percentage(40), // Stats
            ])
            .split(bottom_chunks[1]);

        // Render preview pane
        self.render_preview(frame, right_chunks[0]);

        // Render stats pane
        self.render_stats(frame, right_chunks[1]);
    }

    fn render_search_input(&self, frame: &mut Frame, area: Rect) {
        let search_text = format!("Search: {}", self.query);
        let search_widget = Paragraph::new(search_text)
            .block(
                Block::default()
                    .title(" Fuzzy Search (Type to filter, ↑↓/Tab/Ctrl+n/p: navigate, Enter: edit, ESC: quit) ")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Yellow));

        frame.render_widget(search_widget, area);
    }

    fn render_list(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .results
            .iter()
            .map(|result| {
                // Truncate question for display
                let question = if result.card.question.len() > 50 {
                    format!("{}...", &result.card.question[..47])
                } else {
                    result.card.question.clone()
                };

                let line = Line::from(Span::raw(question));

                ListItem::new(line)
            })
            .collect();

        let title = format!(" Results ({}) ", self.results.len());

        let list = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let preview_text = if let Some(result) = self.results.get(self.selected_index) {
            format!(
                "{}\n\n---\n\n{}",
                result.card.question, result.card.answer
            )
        } else {
            "No card selected".to_string()
        };

        let preview = Paragraph::new(preview_text)
            .block(Block::default().title(" Preview ").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(preview, area);
    }

    fn render_stats(&self, frame: &mut Frame, area: Rect) {
        if let Some(result) = self.results.get(self.selected_index) {
            // Get card stats from database
            if let Ok(conn) = crate::database::init_database() {
                if let Ok(stats) = crate::database::get_card_stats(&conn, &result.card.id) {
                    // Get last 10 review logs
                    let logs = crate::database::get_review_logs(&conn, &result.card.id)
                        .unwrap_or_default();

                    // Create rating history string with color-coded characters
                    let mut rating_spans = vec![];

                    // Pad with X if less than 10 reviews
                    let padding_count = 10usize.saturating_sub(logs.len());
                    for _ in 0..padding_count {
                        rating_spans.push(Span::styled("X", Style::default().fg(Color::DarkGray)));
                    }

                    // Take last 10 reviews
                    let recent_logs = if logs.len() > 10 {
                        &logs[logs.len() - 10..]
                    } else {
                        &logs[..]
                    };

                    for log in recent_logs {
                        let (ch, color) = match log.rating {
                            1 => ('A', Color::Red),          // Again
                            2 => ('H', Color::Yellow),       // Hard
                            3 => ('G', Color::Green),        // Good
                            4 => ('E', Color::Cyan),         // Easy
                            _ => ('?', Color::White),
                        };
                        rating_spans.push(Span::styled(
                            ch.to_string(),
                            Style::default().fg(color).add_modifier(Modifier::BOLD)
                        ));
                    }

                    // Calculate days until due
                    let now = Utc::now();
                    let days_until_due = stats.due_date.signed_duration_since(now).num_days();
                    let (due_str, due_color) = if days_until_due < 0 {
                        (format!("{} days ago", -days_until_due), Color::Red)
                    } else if days_until_due == 0 {
                        ("Today".to_string(), Color::Yellow)
                    } else {
                        (format!("{} days", days_until_due), Color::Green)
                    };

                    let mut lines = vec![
                        Line::from(vec![
                            Span::styled("Last 10 Ratings: ", Style::default().fg(Color::Cyan)),
                        ]),
                        Line::from(rating_spans),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled("Review Count: ", Style::default().fg(Color::Cyan)),
                            Span::styled(
                                stats.review_count.to_string(),
                                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                            ),
                        ]),
                        Line::from(vec![
                            Span::styled("Next Due: ", Style::default().fg(Color::Cyan)),
                            Span::styled(
                                due_str,
                                Style::default().fg(due_color).add_modifier(Modifier::BOLD)
                            ),
                        ]),
                    ];

                    // Add FSRS parameters if available
                    if let Some(stability) = stats.stability {
                        lines.push(Line::from(vec![
                            Span::styled("Stability: ", Style::default().fg(Color::Cyan)),
                            Span::styled(
                                format!("{:.1}", stability),
                                Style::default().fg(Color::White)
                            ),
                        ]));
                    }

                    if let Some(difficulty) = stats.difficulty {
                        lines.push(Line::from(vec![
                            Span::styled("Difficulty: ", Style::default().fg(Color::Cyan)),
                            Span::styled(
                                format!("{:.1}", difficulty),
                                Style::default().fg(Color::White)
                            ),
                        ]));
                    }

                    // Add rating breakdown
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("Rating Counts: ", Style::default().fg(Color::Cyan)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("A:", Style::default().fg(Color::Red)),
                        Span::raw(format!("{} ", stats.rating_counts[0])),
                        Span::styled("H:", Style::default().fg(Color::Yellow)),
                        Span::raw(format!("{} ", stats.rating_counts[1])),
                        Span::styled("G:", Style::default().fg(Color::Green)),
                        Span::raw(format!("{} ", stats.rating_counts[2])),
                        Span::styled("E:", Style::default().fg(Color::Cyan)),
                        Span::raw(format!("{}", stats.rating_counts[3])),
                    ]));

                    let paragraph = Paragraph::new(lines)
                        .block(Block::default().title(" Card Statistics ").borders(Borders::ALL))
                        .wrap(Wrap { trim: true });

                    frame.render_widget(paragraph, area);
                    return;
                }
            }

            // Fallback if stats not available
            let fallback = Paragraph::new("Statistics not available")
                .block(Block::default().title(" Card Statistics ").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            frame.render_widget(fallback, area);
        } else {
            let no_card = Paragraph::new("No card selected")
                .block(Block::default().title(" Card Statistics ").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            frame.render_widget(no_card, area);
        }
    }
}

impl Drop for FinderApp {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_app() -> FinderApp {
        // Create a test finder (requires database, so this might fail in CI)
        // For unit tests, we'll test the key methods directly
        let finder = Finder::new().unwrap_or_else(|_| {
            panic!("Failed to create finder - database not available in test environment")
        });

        FinderApp {
            finder,
            query: String::new(),
            results: vec![
                SearchResult {
                    card: crate::finder::SearchableCard::new(
                        "card-1".to_string(),
                        "Question 1".to_string(),
                        "Answer 1".to_string(),
                    ),
                    score: 100,
                },
                SearchResult {
                    card: crate::finder::SearchableCard::new(
                        "card-2".to_string(),
                        "Question 2".to_string(),
                        "Answer 2".to_string(),
                    ),
                    score: 90,
                },
                SearchResult {
                    card: crate::finder::SearchableCard::new(
                        "card-3".to_string(),
                        "Question 3".to_string(),
                        "Answer 3".to_string(),
                    ),
                    score: 80,
                },
            ],
            selected_index: 0,
            list_state: ListState::default().with_selected(Some(0)),
        }
    }

    #[test]
    fn test_select_navigation() {
        let mut app = create_test_app();

        // Test select_next
        app.select_next();
        assert_eq!(app.selected_index, 1);

        app.select_next();
        assert_eq!(app.selected_index, 2);

        // Wrap around to beginning
        app.select_next();
        assert_eq!(app.selected_index, 0);

        // Test select_prev
        app.select_prev();
        assert_eq!(app.selected_index, 2);

        app.select_prev();
        assert_eq!(app.selected_index, 1);

        app.select_prev();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_handle_key_event_exit() {
        let mut app = create_test_app();

        // Test ESC
        let result = app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(result, AppAction::Exit));

        // Test Ctrl+c
        let result = app.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(result, AppAction::Exit));
    }

    #[test]
    fn test_ctrl_navigation() {
        let mut app = create_test_app();
        let initial_index = app.selected_index;

        // Test Ctrl+n (next)
        app.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(app.selected_index, initial_index + 1);

        // Test Ctrl+p (previous)
        app.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
        assert_eq!(app.selected_index, initial_index);

        // Test Tab (next)
        app.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.selected_index, initial_index + 1);

        // Test BackTab / Shift+Tab (previous)
        app.handle_key_event(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.selected_index, initial_index);
    }

    #[test]
    fn test_handle_key_event_navigation_keys() {
        let mut app = create_test_app();
        let initial_index = app.selected_index;

        // Test Down arrow
        let result = app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(result, AppAction::Continue));
        assert_eq!(app.selected_index, initial_index + 1);

        // Reset
        app.selected_index = initial_index;

        // Test Up arrow (should wrap to end)
        let result = app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert!(matches!(result, AppAction::Continue));
        assert_eq!(app.selected_index, app.results.len() - 1);
    }

    #[test]
    fn test_handle_key_event_enter() {
        let mut app = create_test_app();

        // Test Enter
        let result = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, AppAction::Edit(_)));
    }

    #[test]
    fn test_empty_results() {
        let finder = Finder::new().unwrap_or_else(|_| {
            panic!("Failed to create finder - database not available in test environment")
        });

        let mut app = FinderApp {
            finder,
            query: String::new(),
            results: vec![],
            selected_index: 0,
            list_state: ListState::default(),
        };

        // Navigation should not panic on empty results
        app.select_next();
        app.select_prev();

        // Enter should return Continue, not Edit
        let result = app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(result, AppAction::Continue));
    }
}

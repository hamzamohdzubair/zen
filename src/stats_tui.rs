//! TUI interface for viewing card statistics and analytics

use anyhow::{Context, Result};
use chrono::Utc;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::database::{get_all_card_stats, get_system_stats, init_database, CardStats, SystemStats};

/// Sort order for card list
#[derive(Debug, Clone, PartialEq)]
enum SortBy {
    DueDate,
    ReviewCount,
    Created,
    CardId,
}

/// Application state
pub struct StatsApp {
    system_stats: SystemStats,
    card_stats: Vec<CardStats>,
    sort_by: SortBy,
    scroll_offset: usize,
}

impl StatsApp {
    /// Create a new stats app
    pub fn new() -> Result<Self> {
        let conn = init_database()?;
        let system_stats = get_system_stats(&conn)?;
        let mut card_stats = get_all_card_stats(&conn)?;

        // Default sort by due date
        card_stats.sort_by_key(|s| s.due_date);

        Ok(Self {
            system_stats,
            card_stats,
            sort_by: SortBy::DueDate,
            scroll_offset: 0,
        })
    }

    /// Run the TUI event loop
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Clear(ClearType::All))
            .context("Failed to enter alternate screen")?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        terminal.hide_cursor().context("Failed to hide cursor")?;
        terminal.clear().context("Failed to clear terminal")?;

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
    ) -> Result<()> {
        loop {
            terminal
                .draw(|f| self.render(f))
                .context("Failed to draw frame")?;

            if event::poll(std::time::Duration::from_millis(100))
                .context("Failed to poll events")?
            {
                if let Event::Key(key) = event::read().context("Failed to read event")? {
                    if self.handle_key_event(key) {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // Ctrl+C or ESC or Q to exit
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }
        if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
            return true;
        }

        match key.code {
            KeyCode::Char('d') => {
                self.sort_by = SortBy::DueDate;
                self.card_stats.sort_by_key(|s| s.due_date);
                self.scroll_offset = 0;
            }
            KeyCode::Char('r') => {
                self.sort_by = SortBy::ReviewCount;
                self.card_stats.sort_by(|a, b| b.review_count.cmp(&a.review_count));
                self.scroll_offset = 0;
            }
            KeyCode::Char('c') => {
                self.sort_by = SortBy::Created;
                self.card_stats.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                self.scroll_offset = 0;
            }
            KeyCode::Char('i') => {
                self.sort_by = SortBy::CardId;
                self.card_stats.sort_by(|a, b| a.card_id.cmp(&b.card_id));
                self.scroll_offset = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.scroll_offset < self.card_stats.len().saturating_sub(1) {
                    self.scroll_offset += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + 10).min(self.card_stats.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            _ => {}
        }

        false // Don't exit
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Split into sections: title, summary stats, card table, help
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(7),  // Summary stats
                Constraint::Min(10),    // Card table
                Constraint::Length(3),  // Help text
            ])
            .split(area);

        // Title
        self.render_title(frame, chunks[0]);

        // Summary stats
        self.render_summary(frame, chunks[1]);

        // Card table
        self.render_card_table(frame, chunks[2]);

        // Help text
        self.render_help(frame, chunks[3]);
    }

    fn render_title(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let title = Paragraph::new("Card Statistics")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(title, area);
    }

    fn render_summary(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let stats = &self.system_stats;

        let summary_text = vec![
            Line::from(vec![
                Span::styled("Total Cards: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    stats.total_cards.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                ),
                Span::raw("  "),
                Span::styled("Due Today: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    stats.due_today.to_string(),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                ),
                Span::raw("  "),
                Span::styled("Total Reviews: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    stats.total_reviews.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("New: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    stats.new_cards.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                ),
                Span::raw("  "),
                Span::styled("Learning: ", Style::default().fg(Color::Magenta)),
                Span::styled(
                    stats.learning_cards.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                ),
                Span::raw("  "),
                Span::styled("Mature: ", Style::default().fg(Color::Green)),
                Span::styled(
                    stats.mature_cards.to_string(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    format!("Average reviews per card: {:.1}",
                        if stats.total_cards > 0 {
                            stats.total_reviews as f64 / stats.total_cards as f64
                        } else {
                            0.0
                        }
                    ),
                    Style::default().fg(Color::DarkGray)
                ),
            ]),
        ];

        let summary = Paragraph::new(summary_text)
            .block(Block::default().title(" Summary ").borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        frame.render_widget(summary, area);
    }

    fn render_card_table(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let now = Utc::now();
        let conn = crate::database::init_database();

        // Header
        let header = Row::new(vec![
            Cell::from("Question").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from("Reviews").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from("Due").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from("Last 10").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from("Stability").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from("Difficulty").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]);

        // Calculate how many rows fit in the table area (subtract borders and header)
        let visible_rows = (area.height as usize).saturating_sub(3);

        // Get the visible slice of cards
        let end_idx = (self.scroll_offset + visible_rows).min(self.card_stats.len());
        let visible_cards = &self.card_stats[self.scroll_offset..end_idx];

        // Rows
        let rows: Vec<Row> = visible_cards
            .iter()
            .map(|stat| {
                // Format due date
                let due_diff = stat.due_date.signed_duration_since(now);
                let (due_str, due_color) = if due_diff.num_days() < 0 {
                    (format!("{}d ago", -due_diff.num_days()), Color::Red)
                } else if due_diff.num_days() == 0 {
                    ("Today".to_string(), Color::Yellow)
                } else if due_diff.num_days() < 7 {
                    (format!("{}d", due_diff.num_days()), Color::Green)
                } else {
                    (format!("{}d", due_diff.num_days()), Color::DarkGray)
                };

                // Get last 10 ratings with color coding
                let rating_history = if let Ok(ref conn) = conn {
                    if let Ok(logs) = crate::database::get_review_logs(conn, &stat.card_id) {
                        let mut spans = vec![];

                        // Pad with X if less than 10 reviews
                        let padding_count = 10usize.saturating_sub(logs.len());
                        for _ in 0..padding_count {
                            spans.push(Span::styled("X", Style::default().fg(Color::DarkGray)));
                        }

                        // Take last 10 reviews
                        let recent_logs = if logs.len() > 10 {
                            &logs[logs.len() - 10..]
                        } else {
                            &logs[..]
                        };

                        for log in recent_logs {
                            let (ch, color) = match log.rating {
                                1 => ('A', Color::Red),
                                2 => ('H', Color::Yellow),
                                3 => ('G', Color::Green),
                                4 => ('E', Color::Cyan),
                                _ => ('?', Color::White),
                            };
                            spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
                        }

                        Line::from(spans)
                    } else {
                        Line::from("XXXXXXXXXX")
                    }
                } else {
                    Line::from("XXXXXXXXXX")
                };

                // Get question text from storage
                let question_preview = if let Ok((question, _)) = crate::storage::read_card(&stat.card_id) {
                    if question.len() > 30 {
                        format!("{}...", &question[..27])
                    } else {
                        question
                    }
                } else {
                    "Error loading".to_string()
                };

                // Review count color
                let review_color = if stat.review_count == 0 {
                    Color::DarkGray
                } else if stat.review_count < 5 {
                    Color::Yellow
                } else {
                    Color::Green
                };

                Row::new(vec![
                    Cell::from(question_preview).style(Style::default().fg(Color::White)),
                    Cell::from(stat.review_count.to_string())
                        .style(Style::default().fg(review_color).add_modifier(Modifier::BOLD)),
                    Cell::from(due_str).style(Style::default().fg(due_color)),
                    Cell::from(rating_history),
                    Cell::from(
                        stat.stability
                            .map(|s| format!("{:.1}", s))
                            .unwrap_or_else(|| "-".to_string())
                    )
                    .style(Style::default().fg(Color::DarkGray)),
                    Cell::from(
                        stat.difficulty
                            .map(|d| format!("{:.1}", d))
                            .unwrap_or_else(|| "-".to_string())
                    )
                    .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let sort_indicator = match self.sort_by {
            SortBy::DueDate => " (sorted by Due Date)",
            SortBy::ReviewCount => " (sorted by Review Count)",
            SortBy::Created => " (sorted by Creation Date)",
            SortBy::CardId => " (sorted by Card ID)",
        };

        let title = format!(
            " Cards ({}/{}){}",
            self.scroll_offset + 1,
            self.card_stats.len().max(1),
            sort_indicator
        );

        let table = Table::new(
            rows,
            [
                Constraint::Length(30), // Question
                Constraint::Length(8),  // Reviews
                Constraint::Length(10), // Due
                Constraint::Length(10), // Last 10 ratings
                Constraint::Length(10), // Stability
                Constraint::Length(12), // Difficulty
            ],
        )
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .column_spacing(1);

        frame.render_widget(table, area);
    }

    fn render_help(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let help_text = Line::from(vec![
            Span::styled("[q/Esc] ", Style::default().fg(Color::Yellow)),
            Span::raw("Exit  "),
            Span::styled("[↑↓/jk] ", Style::default().fg(Color::Yellow)),
            Span::raw("Scroll  "),
            Span::styled("[d] ", Style::default().fg(Color::Yellow)),
            Span::raw("Sort by Due  "),
            Span::styled("[r] ", Style::default().fg(Color::Yellow)),
            Span::raw("Sort by Reviews  "),
            Span::styled("[c] ", Style::default().fg(Color::Yellow)),
            Span::raw("Sort by Created  "),
            Span::styled("[i] ", Style::default().fg(Color::Yellow)),
            Span::raw("Sort by ID"),
        ]);

        let help = Paragraph::new(help_text)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(help, area);
    }
}

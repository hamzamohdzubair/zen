//! TUI for displaying topic and keyword performance statistics

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::database::{get_keyword_performance_stats, get_keyword_stats, get_topic_performance_stats, get_topic_stats, init_database, KeywordStats, KeywordStatsData, TopicStats, TopicStatsData};
use chrono::{Datelike, Local, NaiveDate, Weekday};

/// State machine for the stats TUI
#[derive(Debug, Clone, PartialEq)]
enum StatsState {
    TopicPerformance,
    KeywordPerformance,
}

/// Stats TUI application
pub struct StatsApp {
    state: StatsState,
    topic_stats: Vec<TopicStatsData>,
    keyword_stats: Vec<KeywordStatsData>,
    topic_summary: TopicStats,
    keyword_summary: KeywordStats,
    review_dates: Vec<NaiveDate>,
    scroll_offset: usize,
}

impl StatsApp {
    pub fn new() -> Result<()> {
        let conn = init_database()?;
        let topic_stats = get_topic_performance_stats(&conn)?;
        let keyword_stats = get_keyword_performance_stats(&conn)?;
        let topic_summary = get_topic_stats(&conn)?;
        let keyword_summary = get_keyword_stats(&conn)?;
        let review_dates = crate::database::get_review_dates(&conn)?;

        let mut app = Self {
            state: StatsState::TopicPerformance,
            topic_stats,
            keyword_stats,
            topic_summary,
            keyword_summary,
            review_dates,
            scroll_offset: 0,
        };

        app.run()
    }

    fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_event_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Handle user input with 100ms polling
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        break;
                    }

                    if !self.handle_input(key.code)? {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, key: KeyCode) -> Result<bool> {
        match key {
            KeyCode::Char('q') => Ok(false), // Exit
            KeyCode::Tab => {
                // Toggle between screens
                self.state = match self.state {
                    StatsState::TopicPerformance => StatsState::KeywordPerformance,
                    StatsState::KeywordPerformance => StatsState::TopicPerformance,
                };
                self.scroll_offset = 0;
                Ok(true)
            }
            KeyCode::Up | KeyCode::Char('j') => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
                Ok(true)
            }
            KeyCode::Down | KeyCode::Char('l') => {
                let max_scroll = self.get_max_scroll();
                if self.scroll_offset < max_scroll {
                    self.scroll_offset += 1;
                }
                Ok(true)
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                Ok(true)
            }
            KeyCode::PageDown => {
                let max_scroll = self.get_max_scroll();
                self.scroll_offset = (self.scroll_offset + 10).min(max_scroll);
                Ok(true)
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                Ok(true)
            }
            KeyCode::End => {
                self.scroll_offset = self.get_max_scroll();
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    fn get_max_scroll(&self) -> usize {
        match self.state {
            StatsState::TopicPerformance => {
                if self.topic_stats.is_empty() {
                    0
                } else {
                    self.topic_stats.len().saturating_sub(1)
                }
            }
            StatsState::KeywordPerformance => {
                if self.keyword_stats.is_empty() {
                    0
                } else {
                    self.keyword_stats.len().saturating_sub(1)
                }
            }
        }
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Main vertical layout
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Min(10),    // Content (will be split horizontally)
                Constraint::Length(3),  // Legend
                Constraint::Length(2),  // Status
            ])
            .split(size);

        // Render title
        let title = match self.state {
            StatsState::TopicPerformance => "Topic Performance",
            StatsState::KeywordPerformance => "Keyword Performance",
        };
        self.render_title(f, main_chunks[0], title);

        // Split content area horizontally (main content on left, stats/calendar on right)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(60),     // Main content
                Constraint::Length(33),  // Stats table + calendar on right
            ])
            .split(main_chunks[1]);

        // Split right column vertically (stats table on top, calendar below)
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12),  // Stats table
                Constraint::Min(10),     // Calendar
            ])
            .split(content_chunks[1]);

        match self.state {
            StatsState::TopicPerformance => {
                self.render_topic_list(f, content_chunks[0]);
                self.render_topic_legend(f, main_chunks[2]);
            }
            StatsState::KeywordPerformance => {
                self.render_keyword_list(f, content_chunks[0]);
                self.render_keyword_legend(f, main_chunks[2]);
            }
        }

        // Render stats table and calendar on right side
        self.render_stats_table(f, right_chunks[0]);
        self.render_calendar(f, right_chunks[1]);

        self.render_status(f, main_chunks[3], "Tab: Switch View | q: Quit | ↑↓/jl: Scroll | PgUp/PgDn/Home/End");
    }

    fn render_title(&self, f: &mut Frame, area: Rect, title: &str) {
        let title_span = Span::styled(
            format!(" {} ", title),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        );

        let paragraph = Paragraph::new(Line::from(title_span))
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_stats_table(&self, f: &mut Frame, area: Rect) {
        let mut lines = Vec::new();

        // Header row
        lines.push(Line::from(vec![
            Span::styled(format!("{:14}", ""), Style::default()),
            Span::styled(format!("{:7}", "Topic"), Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:7}", "kw"), Style::default().add_modifier(Modifier::BOLD)),
        ]));

        lines.push(Line::from("─".repeat(28)));

        // Total row
        lines.push(Line::from(vec![
            Span::styled(format!("{:14}", "Total"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:7}", self.topic_summary.total)),
            Span::raw(format!("{:7}", self.keyword_summary.total_keywords)),
        ]));

        // Due Today row
        lines.push(Line::from(vec![
            Span::styled(format!("{:14}", "Due Today"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:7}", self.topic_summary.due_today)),
            Span::raw(format!("{:7}", self.keyword_summary.due_today)),
        ]));

        // Due This Week row
        lines.push(Line::from(vec![
            Span::styled(format!("{:14}", "Due Week"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:7}", self.topic_summary.due_week)),
            Span::raw(format!("{:7}", self.keyword_summary.due_week)),
        ]));

        lines.push(Line::from("─".repeat(28)));

        // Reviews row
        lines.push(Line::from(vec![
            Span::styled(format!("{:14}", "Reviews"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:14}", self.topic_summary.reviews_completed)),
        ]));

        // Average Score row
        let topic_avg_color = score_to_color(self.topic_summary.average_score);
        let keyword_avg_color = score_to_color(self.keyword_summary.average_score);

        lines.push(Line::from(vec![
            Span::styled(format!("{:14}", "Avg Score"), Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:6.1}%", self.topic_summary.average_score),
                Style::default().fg(topic_avg_color)
            ),
            Span::raw(" "),
            Span::styled(
                format!("{:6.1}%", self.keyword_summary.average_score),
                Style::default().fg(keyword_avg_color)
            ),
        ]));

        let block = Block::default()
            .title("Statistics")
            .borders(Borders::ALL)
            .style(Style::default());

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }

    fn render_topic_list(&self, f: &mut Frame, area: Rect) {
        if self.topic_stats.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default());

            let paragraph = Paragraph::new("No review data yet. Complete some reviews to see statistics.")
                .block(block)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
            return;
        }

        // Define column widths
        const COL_KEYWORDS: usize = 29;  // 28 chars + 1 space
        const COL_LAST: usize = 5;       // 5 chars for score
        const COL_AVG: usize = 5;        // 5 chars for score
        const COL_SPACING: usize = 2;    // 2 spaces between columns
        const INDENT_WIDTH: usize = COL_KEYWORDS + COL_LAST + COL_SPACING + COL_AVG + COL_SPACING;

        let mut lines = Vec::new();

        // Header row
        lines.push(Line::from(vec![
            Span::styled(format!("{:29}", "Keywords"), Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:5}", "Last"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(format!("{:5}", "Avg"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled("Recent Sessions", Style::default().add_modifier(Modifier::BOLD)),
        ]));

        lines.push(Line::from("─".repeat(area.width as usize - 2)));

        // Calculate visible window (each topic takes 4 lines: 1 header + 3 matrix rows)
        let content_height = (area.height as usize).saturating_sub(4); // Minus border and header
        let items_per_topic = 4;
        let visible_topics = content_height / items_per_topic;
        let start_idx = self.scroll_offset;
        let end_idx = (start_idx + visible_topics).min(self.topic_stats.len());

        for topic in &self.topic_stats[start_idx..end_idx] {
            // Format keywords (truncate if needed)
            let keywords_str = topic.keywords.join(", ");
            let keywords_display = if keywords_str.len() > 28 {
                format!("{:25}...", &keywords_str[..25])
            } else {
                format!("{:28}", keywords_str)
            };

            // Format scores
            let last_score = if let Some(score) = topic.last_session_score {
                format!("{:5.1}", score)
            } else {
                "  -  ".to_string()
            };

            let avg_score = format!("{:5.1}", topic.overall_average_score);

            // Color based on average score
            let score_color = score_to_color(topic.overall_average_score);

            // Build rating matrix as 3 lines (3 questions per column, 10 columns max)
            let matrix_lines = self.render_topic_rating_matrix(&topic.recent_sessions);

            // First line with all info
            let mut first_line_spans = vec![
                Span::raw(format!("{:29}", keywords_display)),
                Span::styled(format!("{:5}", last_score), Style::default().fg(score_color)),
                Span::raw("  "),
                Span::styled(format!("{:5}", avg_score), Style::default().fg(score_color)),
                Span::raw("  "),
            ];
            first_line_spans.extend(matrix_lines[0].clone());
            lines.push(Line::from(first_line_spans));

            // Second and third lines (indented to align with matrix)
            for i in 1..3 {
                let mut line_spans = vec![Span::raw(" ".repeat(INDENT_WIDTH))];
                line_spans.extend(matrix_lines[i].clone());
                lines.push(Line::from(line_spans));
            }

            // Add blank line between topics for readability
            lines.push(Line::from(""));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }

    fn render_topic_rating_matrix(&self, sessions: &[crate::database::ReviewSession]) -> [Vec<Span<'static>>; 3] {
        let mut lines: [Vec<Span>; 3] = [Vec::new(), Vec::new(), Vec::new()];

        const MATRIX_WIDTH: usize = 10; // 10 columns
        const ROWS: usize = 3; // 3 rows (questions)

        // Get up to 10 most recent sessions
        let sessions_to_show: Vec<_> = sessions.iter().take(MATRIX_WIDTH).collect();
        let num_sessions = sessions_to_show.len();

        // Calculate how many placeholder columns we need on the left
        let placeholder_cols = MATRIX_WIDTH - num_sessions;

        // Fill each row
        for row_idx in 0..ROWS {
            // Add placeholder dots on the left
            for col_idx in 0..placeholder_cols {
                lines[row_idx].push(Span::styled("·", Style::default().fg(Color::DarkGray)));
                // Add space after each column except the last
                if col_idx < placeholder_cols - 1 || num_sessions > 0 {
                    lines[row_idx].push(Span::raw(" "));
                }
            }

            // Add actual session data from the right (most recent on the right)
            for (session_idx, session) in sessions_to_show.iter().enumerate() {
                // Get the question for this row
                if row_idx < session.questions.len() {
                    lines[row_idx].push(get_colored_symbol(session.questions[row_idx].rating));
                } else {
                    // Session has fewer than 3 questions, use placeholder
                    lines[row_idx].push(Span::styled("·", Style::default().fg(Color::DarkGray)));
                }

                // Add space after each column except the last
                if session_idx < num_sessions - 1 {
                    lines[row_idx].push(Span::raw(" "));
                }
            }
        }

        lines
    }

    fn render_keyword_rating_matrix(&self, topic_reviews: &[crate::database::KeywordTopicReview]) -> [Vec<Span<'static>>; 3] {
        let mut lines: [Vec<Span>; 3] = [Vec::new(), Vec::new(), Vec::new()];

        const MATRIX_WIDTH: usize = 10; // 10 columns (topics)
        const ROWS: usize = 3; // 3 rows (questions)

        // Get up to 10 topics
        let topics_to_show: Vec<_> = topic_reviews.iter().take(MATRIX_WIDTH).collect();
        let num_topics = topics_to_show.len();

        // Calculate how many placeholder columns we need on the left
        let placeholder_cols = MATRIX_WIDTH - num_topics;

        // Fill each row
        for row_idx in 0..ROWS {
            // Add placeholder dots on the left
            for col_idx in 0..placeholder_cols {
                lines[row_idx].push(Span::styled("·", Style::default().fg(Color::DarkGray)));
                // Add space after each column except the last
                if col_idx < placeholder_cols - 1 || num_topics > 0 {
                    lines[row_idx].push(Span::raw(" "));
                }
            }

            // Add actual topic data from the right (most recent on the right)
            for (topic_idx, topic_review) in topics_to_show.iter().enumerate() {
                // Get the most recent session for this topic
                if let Some(session) = topic_review.recent_sessions.first() {
                    // Get the question for this row
                    if row_idx < session.questions.len() {
                        lines[row_idx].push(get_colored_symbol(session.questions[row_idx].rating));
                    } else {
                        // Session has fewer than 3 questions, use placeholder
                        lines[row_idx].push(Span::styled("·", Style::default().fg(Color::DarkGray)));
                    }
                } else {
                    // No sessions for this topic
                    lines[row_idx].push(Span::styled("·", Style::default().fg(Color::DarkGray)));
                }

                // Add space after each column except the last
                if topic_idx < num_topics - 1 {
                    lines[row_idx].push(Span::raw(" "));
                }
            }
        }

        lines
    }

    fn render_topic_legend(&self, f: &mut Frame, area: Rect) {
        let text = "✓=Easy ≥90  -=Good/Hard 60-89  ✗=Again <60  ·=No data";

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::DarkGray));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_keyword_list(&self, f: &mut Frame, area: Rect) {
        if self.keyword_stats.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .style(Style::default());

            let paragraph = Paragraph::new("No review data yet. Complete some reviews to see statistics.")
                .block(block)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
            return;
        }

        // Define column widths
        const COL_KEYWORD: usize = 29;   // 28 chars + 1 space
        const COL_TOPICS: usize = 7;     // 6 chars + 1 space
        const COL_AVG: usize = 5;        // 5 chars for score
        const COL_SPACING: usize = 2;    // 2 spaces between columns
        const INDENT_WIDTH: usize = COL_KEYWORD + COL_TOPICS + COL_AVG + COL_SPACING;

        let mut lines = Vec::new();

        // Header row
        lines.push(Line::from(vec![
            Span::styled(format!("{:29}", "Keyword"), Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:7}", "Topics"), Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:5}", "Avg"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled("Performance by Topic", Style::default().add_modifier(Modifier::BOLD)),
        ]));

        lines.push(Line::from("─".repeat(area.width as usize - 2)));

        // Calculate visible window (each keyword takes 4 lines: 1 header + 3 matrix rows)
        let content_height = (area.height as usize).saturating_sub(4);
        let items_per_keyword = 4;
        let visible_keywords = content_height / items_per_keyword;
        let start_idx = self.scroll_offset;
        let end_idx = (start_idx + visible_keywords).min(self.keyword_stats.len());

        for keyword in &self.keyword_stats[start_idx..end_idx] {
            // Format keyword (truncate if needed)
            let keyword_display = if keyword.keyword.len() > 28 {
                format!("{:25}...", &keyword.keyword[..25])
            } else {
                format!("{:28}", keyword.keyword)
            };

            let topics_display = format!("{:6}", keyword.topic_count);
            let avg_display = format!("{:5.1}", keyword.average_score);
            let score_color = score_to_color(keyword.average_score);

            // Build rating matrix showing performance across topics
            let matrix_lines = self.render_keyword_rating_matrix(&keyword.topic_reviews);

            // First line with all info
            let mut first_line_spans = vec![
                Span::raw(format!("{:29}", keyword_display)),
                Span::raw(format!("{:7}", topics_display)),
                Span::styled(format!("{:5}", avg_display), Style::default().fg(score_color)),
                Span::raw("  "),
            ];
            first_line_spans.extend(matrix_lines[0].clone());
            lines.push(Line::from(first_line_spans));

            // Second and third lines (indented to align with matrix)
            for i in 1..3 {
                let mut line_spans = vec![Span::raw(" ".repeat(INDENT_WIDTH))];
                line_spans.extend(matrix_lines[i].clone());
                lines.push(Line::from(line_spans));
            }

            // Add blank line between keywords for readability
            lines.push(Line::from(""));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }

    fn render_keyword_legend(&self, f: &mut Frame, area: Rect) {
        let text = "✓=Easy ≥90  -=Good/Hard 60-89  ✗=Again <60  ·=No data";

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::DarkGray));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_status(&self, f: &mut Frame, area: Rect, instructions: &str) {
        let paragraph = Paragraph::new(instructions)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_calendar(&self, f: &mut Frame, area: Rect) {
        let mut lines = Vec::new();

        // Get current month info
        let today = Local::now().naive_local().date();
        let year = today.year();
        let month = today.month();

        // Month header
        let month_name = match month {
            1 => "January", 2 => "February", 3 => "March", 4 => "April",
            5 => "May", 6 => "June", 7 => "July", 8 => "August",
            9 => "September", 10 => "October", 11 => "November", 12 => "December",
            _ => "Unknown",
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} {}", month_name, year),
                Style::default().add_modifier(Modifier::BOLD)
            ),
        ]));
        lines.push(Line::from(""));

        // Day headers (Su Mo Tu We Th Fr Sa)
        lines.push(Line::from(vec![
            Span::styled("Su Mo Tu We Th Fr Sa", Style::default().add_modifier(Modifier::BOLD)),
        ]));

        // Get first day of month
        let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        let first_weekday = first_day.weekday();

        // Calculate number of days in month
        let days_in_month = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
        }.signed_duration_since(first_day).num_days() as u32;

        // Build calendar grid
        let mut current_line_spans = Vec::new();

        // Add leading spaces for first week
        let leading_spaces = match first_weekday {
            Weekday::Sun => 0,
            Weekday::Mon => 1,
            Weekday::Tue => 2,
            Weekday::Wed => 3,
            Weekday::Thu => 4,
            Weekday::Fri => 5,
            Weekday::Sat => 6,
        };

        for _ in 0..leading_spaces {
            current_line_spans.push(Span::raw("   "));
        }

        // Add days
        for day in 1..=days_in_month {
            let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            let is_today = date == today;
            let has_review = self.review_dates.contains(&date);
            let is_past = date < today;

            // Determine color coding (no symbols, just colored day numbers)
            let (text, style) = if is_today {
                (format!("{:2}", day), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
            } else if has_review {
                (format!("{:2}", day), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else if is_past {
                (format!("{:2}", day), Style::default().fg(Color::Red))
            } else {
                (format!("{:2}", day), Style::default().fg(Color::DarkGray))
            };

            current_line_spans.push(Span::styled(text, style));

            // Add space or newline
            let current_weekday = date.weekday();
            if current_weekday == Weekday::Sat || day == days_in_month {
                lines.push(Line::from(current_line_spans.clone()));
                current_line_spans.clear();
            } else {
                current_line_spans.push(Span::raw(" "));
            }
        }

        // Add legend
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Bold+Underline", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
            Span::raw("=Today  "),
            Span::styled("Bold", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("=Reviewed  "),
            Span::styled("Red", Style::default().fg(Color::Red)),
            Span::raw("=Missed"),
        ]));

        let block = Block::default()
            .title("Review Calendar")
            .borders(Borders::ALL)
            .style(Style::default());

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    }
}

/// Map score to color (standardized with rating boundaries)
fn score_to_color(score: f64) -> Color {
    if score >= 90.0 {
        Color::Green  // Easy
    } else if score >= 70.0 {
        Color::Yellow  // Good
    } else if score >= 60.0 {
        Color::Yellow  // Hard
    } else {
        Color::Red  // Again
    }
}

/// Get colored symbol for rating
fn get_colored_symbol(rating: u8) -> Span<'static> {
    match rating {
        4 => Span::styled("✓", Style::default().fg(Color::Green)),   // Easy (≥90)
        3 => Span::styled("-", Style::default().fg(Color::Yellow)),  // Good (70-89)
        2 => Span::styled("-", Style::default().fg(Color::Yellow)),  // Hard (60-69)
        1 => Span::styled("✗", Style::default().fg(Color::Red)),     // Again (<60)
        _ => Span::raw(" "),
    }
}


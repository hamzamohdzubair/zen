//! TUI interface for review sessions

use anyhow::{Context, Result};
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
use std::collections::HashSet;
use std::io;

use crate::bert_score::BertScorer;
use crate::review::{format_interval, ReviewSession};

/// Common English stopwords to filter out
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has", "he",
    "in", "is", "it", "its", "of", "on", "that", "the", "to", "was", "will", "with",
    "i", "you", "we", "they", "them", "this", "these", "those", "can", "should",
    "would", "could", "may", "might", "must", "shall", "or", "but", "not", "no",
    "yes", "have", "had", "do", "does", "did", "what", "where", "when", "why", "how",
];

/// Extract keywords from text (lowercase words, excluding stopwords and punctuation)
fn extract_keywords(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split_whitespace()
        .map(|word| {
            // Remove punctuation from word
            word.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|word| !word.is_empty() && !STOPWORDS.contains(&word.as_str()))
        .collect()
}

/// Calculate keyword score: (matching keywords / keywords in correct answer)
fn calculate_keyword_score(user_answer: &str, correct_answer: &str) -> f64 {
    let user_keywords = extract_keywords(user_answer);
    let correct_keywords = extract_keywords(correct_answer);

    if correct_keywords.is_empty() {
        return 0.0;
    }

    let matching = user_keywords.intersection(&correct_keywords).count();
    matching as f64 / correct_keywords.len() as f64
}

/// Create styled spans for text with keyword highlighting
fn highlight_keywords(text: &str, keywords_to_highlight: &HashSet<String>, base_color: Color) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        let word_clean = word.chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();

        let is_keyword = keywords_to_highlight.contains(&word_clean);
        let is_last = i == words.len() - 1;

        if is_keyword {
            // Push any accumulated text first
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text.clone(), Style::default().fg(base_color)));
                current_text.clear();
            }
            // Add highlighted keyword (without trailing space in the highlight)
            spans.push(Span::styled(
                word.to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray),
            ));
            // Add space after keyword if not the last word
            if !is_last {
                spans.push(Span::styled(" ", Style::default().fg(base_color)));
            }
        } else {
            current_text.push_str(word);
            if !is_last {
                current_text.push(' ');
            }
        }
    }

    // Push any remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, Style::default().fg(base_color)));
    }

    spans
}

/// Application state
#[derive(Debug, Clone, PartialEq)]
enum ReviewState {
    ShowingQuestion,
    InputtingAnswer,
    ShowingAnswer,
    Complete,
}

/// Action to take after handling an event
enum AppAction {
    Continue,
    StartInput,
    RevealAnswer,
    SubmitRating(u8),
    Exit,
}

/// TUI application for review sessions
pub struct ReviewApp {
    session: ReviewSession,
    state: ReviewState,
    user_input: Vec<String>, // Lines of user input
    current_line: String,     // Current line being typed
    bert_scorer: Option<BertScorer>, // Option for graceful fallback
}

impl ReviewApp {
    /// Create a new review app
    pub fn new() -> Result<Self> {
        let session = ReviewSession::new()?;

        // Try to initialize BERT scorer, but don't fail if it errors
        let bert_scorer = match BertScorer::new() {
            Ok(scorer) => Some(scorer),
            Err(e) => {
                eprintln!("Warning: Could not load BERT model: {}", e);
                eprintln!("BERT scoring will be unavailable. The app will continue with keyword scoring only.");
                None
            }
        };

        Ok(Self {
            session,
            state: ReviewState::ShowingQuestion,
            user_input: Vec::new(),
            current_line: String::new(),
            bert_scorer,
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
                    match self.handle_key_event(key) {
                        AppAction::Continue => {}
                        AppAction::StartInput => {
                            self.state = ReviewState::InputtingAnswer;
                            self.user_input.clear();
                            self.current_line.clear();
                        }
                        AppAction::RevealAnswer => {
                            self.state = ReviewState::ShowingAnswer;
                        }
                        AppAction::SubmitRating(rating) => {
                            self.session.submit_rating(rating)?;
                            if self.session.is_complete() {
                                self.state = ReviewState::Complete;
                            } else {
                                self.state = ReviewState::ShowingQuestion;
                                self.user_input.clear();
                                self.current_line.clear();
                            }
                        }
                        AppAction::Exit => {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        // Ctrl+C always exits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return AppAction::Exit;
        }

        // ESC always exits
        if key.code == KeyCode::Esc {
            return AppAction::Exit;
        }

        match self.state {
            ReviewState::ShowingQuestion => match key.code {
                KeyCode::Char(' ') | KeyCode::Enter => AppAction::StartInput,
                _ => AppAction::Continue,
            },
            ReviewState::InputtingAnswer => {
                match key.code {
                    KeyCode::Char(c) => {
                        self.current_line.push(c);
                        AppAction::Continue
                    }
                    KeyCode::Backspace => {
                        self.current_line.pop();
                        AppAction::Continue
                    }
                    KeyCode::Enter => {
                        // Check if current line is empty and we have previous input
                        if self.current_line.trim().is_empty() && !self.user_input.is_empty() {
                            // Empty line after content - reveal answer
                            AppAction::RevealAnswer
                        } else {
                            // Add current line to input and start new line
                            self.user_input.push(self.current_line.clone());
                            self.current_line.clear();
                            AppAction::Continue
                        }
                    }
                    _ => AppAction::Continue,
                }
            }
            ReviewState::ShowingAnswer => match key.code {
                KeyCode::Char('1') => AppAction::SubmitRating(1),
                KeyCode::Char('2') => AppAction::SubmitRating(2),
                KeyCode::Char('3') => AppAction::SubmitRating(3),
                KeyCode::Char('4') => AppAction::SubmitRating(4),
                _ => AppAction::Continue,
            },
            ReviewState::Complete => AppAction::Exit,
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        match self.state {
            ReviewState::ShowingQuestion => self.render_question(frame, size),
            ReviewState::InputtingAnswer => self.render_input(frame, size),
            ReviewState::ShowingAnswer => self.render_answer(frame, size),
            ReviewState::Complete => self.render_summary(frame, size),
        }
    }

    fn render_input(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let card = self.session.current_card().expect("No current card");
        let (current, total) = self.session.progress();

        // Split into title, question, input area, and hint
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(7),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        // Title bar
        let title = format!(" Review Session - Card {}/{} ", current, total);
        let title_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let title_para = Paragraph::new("").block(title_block);
        frame.render_widget(title_para, chunks[0]);

        // Question
        let question_text = vec![
            Line::from(vec![Span::styled(
                "Question:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(card.question.clone()),
        ];

        let question = Paragraph::new(question_text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(question, chunks[1]);

        // User input area
        let mut input_lines = vec![
            Line::from(vec![Span::styled(
                "Your Answer:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ];

        // Show previous lines
        for line in &self.user_input {
            input_lines.push(Line::from(line.clone()));
        }

        // Show current line with cursor
        let cursor_line = format!("{}\u{2588}", self.current_line); // Block cursor
        input_lines.push(Line::from(cursor_line));

        let input_widget = Paragraph::new(input_lines)
            .block(Block::default().borders(Borders::ALL).border_style(
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ))
            .wrap(Wrap { trim: false });

        frame.render_widget(input_widget, chunks[2]);

        // Hint
        let hint = Paragraph::new("(Type your answer. Press Enter twice to reveal actual answer)")
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));

        frame.render_widget(hint, chunks[3]);
    }

    fn render_question(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let card = self.session.current_card().expect("No current card");
        let (current, total) = self.session.progress();

        // Split into title and content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        // Title bar
        let title = format!(" Review Session - Card {}/{} ", current, total);
        let title_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let title_para = Paragraph::new("").block(title_block);
        frame.render_widget(title_para, chunks[0]);

        // Content area
        let content_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(chunks[1]);

        // Question
        let question_text = vec![
            Line::from(vec![Span::styled(
                "Question:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(card.question.clone()),
        ];

        let question = Paragraph::new(question_text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(question, content_chunks[0]);

        // Hint
        let hint = Paragraph::new("(Press Space or Enter to start typing your answer)")
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));

        frame.render_widget(hint, content_chunks[1]);
    }

    fn render_answer(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let card = self.session.current_card().expect("No current card");
        let (current, total) = self.session.progress();

        // Get interval previews
        let preview = self
            .session
            .preview_next_intervals()
            .expect("Failed to preview intervals");

        // Split into title, question, side-by-side comparison, scores, and rating bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Title
                Constraint::Length(5),  // Question
                Constraint::Min(0),     // Side-by-side comparison
                Constraint::Length(5),  // Scores table (keyword, BERT, etc.)
                Constraint::Length(5),  // Rating bar
            ])
            .split(area);

        // Title bar
        let title = format!(" Review Session - Card {}/{} ", current, total);
        let title_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let title_para = Paragraph::new("").block(title_block);
        frame.render_widget(title_para, chunks[0]);

        // Question
        let question_text = vec![
            Line::from(vec![Span::styled(
                "Question:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(card.question.clone()),
        ];

        let question = Paragraph::new(question_text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(question, chunks[1]);

        // Calculate keyword score and find matching keywords
        let user_answer_text = self.user_input.join("\n");
        let keyword_score = calculate_keyword_score(&user_answer_text, &card.answer);
        let user_keywords = extract_keywords(&user_answer_text);
        let correct_keywords = extract_keywords(&card.answer);
        let matching_keywords: HashSet<String> = user_keywords.intersection(&correct_keywords).cloned().collect();

        // Side-by-side comparison
        let comparison_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        // User's answer with keyword highlighting
        let user_answer_lines: Vec<Line> = user_answer_text
            .lines()
            .map(|line| {
                let spans = highlight_keywords(line, &matching_keywords, Color::White);
                Line::from(spans)
            })
            .collect();

        let user_answer = Paragraph::new(user_answer_lines)
            .block(
                Block::default()
                    .title(" Your Answer ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(user_answer, comparison_chunks[0]);

        // Actual answer with keyword highlighting
        let actual_answer_lines: Vec<Line> = card
            .answer
            .lines()
            .map(|line| {
                let spans = highlight_keywords(line, &matching_keywords, Color::White);
                Line::from(spans)
            })
            .collect();

        let actual_answer = Paragraph::new(actual_answer_lines)
            .block(
                Block::default()
                    .title(" Actual Answer ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(actual_answer, comparison_chunks[1]);

        // Scores section (using centered Table widget)
        let keyword_score_percentage = (keyword_score * 100.0) as u32;
        let keyword_score_color = match keyword_score_percentage {
            80..=100 => Color::Green,
            50..=79 => Color::Yellow,
            _ => Color::Red,
        };

        // Calculate BERT score if scorer is available
        let (bert_score_text, bert_score_color, bert_details) = if let Some(scorer) = &mut self.bert_scorer {
            match scorer.calculate_score(&user_answer_text, &card.answer) {
                Ok(score) => {
                    let percentage = (score * 100.0) as u32;
                    let color = match percentage {
                        80..=100 => Color::Green,
                        50..=79 => Color::Yellow,
                        _ => Color::Red,
                    };
                    (format!("{}%", percentage), color, "Semantic similarity".to_string())
                }
                Err(_e) => {
                    // Don't print error in render loop - it gets called many times per second
                    ("N/A".to_string(), Color::DarkGray, "Calculation failed".to_string())
                }
            }
        } else {
            ("N/A".to_string(), Color::DarkGray, "Model not loaded".to_string())
        };

        // Center the scores table horizontally
        let scores_horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ])
            .split(chunks[3]);

        let header = Row::new(vec![
            Cell::from(Line::from("Metric").alignment(Alignment::Center))
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from(Line::from("Score").alignment(Alignment::Center))
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Cell::from(Line::from("Details").alignment(Alignment::Center))
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]);

        let rows = vec![
            Row::new(vec![
                Cell::from(Line::from("Keyword").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::White)),
                Cell::from(Line::from(format!("{}%", keyword_score_percentage)).alignment(Alignment::Center))
                    .style(Style::default().fg(keyword_score_color).add_modifier(Modifier::BOLD)),
                Cell::from(Line::from(format!("{}/{} keywords", matching_keywords.len(), correct_keywords.len())).alignment(Alignment::Center))
                    .style(Style::default().fg(Color::DarkGray)),
            ]),
            Row::new(vec![
                Cell::from(Line::from("BERT").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::White)),
                Cell::from(Line::from(bert_score_text.clone()).alignment(Alignment::Center))
                    .style(Style::default().fg(bert_score_color).add_modifier(
                        if bert_score_text == "N/A" { Modifier::ITALIC } else { Modifier::BOLD }
                    )),
                Cell::from(Line::from(bert_details.clone()).alignment(Alignment::Center))
                    .style(Style::default().fg(Color::DarkGray).add_modifier(
                        if bert_details == "Model not loaded" || bert_details == "Calculation failed" {
                            Modifier::ITALIC
                        } else {
                            Modifier::empty()
                        }
                    )),
            ]),
        ];

        let scores_table = Table::new(
            rows,
            [Constraint::Percentage(33), Constraint::Percentage(33), Constraint::Percentage(34)],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Similarity Scores "))
        .column_spacing(1);

        frame.render_widget(scores_table, scores_horizontal[1]);

        // Rating bar (using centered Table widget)
        // Center the rating table horizontally
        let rating_horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(chunks[4]);

        let rating_rows = vec![
            Row::new(vec![
                Cell::from(Line::from("[1]").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Red)),
                Cell::from(Line::from("[2]").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Yellow)),
                Cell::from(Line::from("[3]").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Green)),
                Cell::from(Line::from("[4]").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Cyan)),
            ]),
            Row::new(vec![
                Cell::from(Line::from("Again").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Red)),
                Cell::from(Line::from("Hard").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Yellow)),
                Cell::from(Line::from("Good").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Green)),
                Cell::from(Line::from("Easy").alignment(Alignment::Center))
                    .style(Style::default().fg(Color::Cyan)),
            ]),
            Row::new(vec![
                Cell::from(Line::from(format_interval(preview.again_days)).alignment(Alignment::Center))
                    .style(Style::default().fg(Color::White)),
                Cell::from(Line::from(format_interval(preview.hard_days)).alignment(Alignment::Center))
                    .style(Style::default().fg(Color::White)),
                Cell::from(Line::from(format_interval(preview.good_days)).alignment(Alignment::Center))
                    .style(Style::default().fg(Color::White)),
                Cell::from(Line::from(format_interval(preview.easy_days)).alignment(Alignment::Center))
                    .style(Style::default().fg(Color::White)),
            ]),
        ];

        let rating_table = Table::new(
            rating_rows,
            [
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ],
        )
        .block(Block::default().borders(Borders::ALL))
        .column_spacing(1);

        frame.render_widget(rating_table, rating_horizontal[1]);
    }

    fn render_summary(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let summary = self.session.summary();

        // Center the summary box
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Length(15),
                Constraint::Percentage(30),
            ])
            .split(area);

        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(vertical_chunks[1]);

        let summary_area = horizontal_chunks[1];

        let summary_text = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("Cards Reviewed: {}", summary.total_reviewed),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Again:  ", Style::default().fg(Color::Red)),
                Span::raw(format!("{}", summary.again_count)),
            ]),
            Line::from(vec![
                Span::styled("Hard:   ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}", summary.hard_count)),
            ]),
            Line::from(vec![
                Span::styled("Good:   ", Style::default().fg(Color::Green)),
                Span::raw(format!("{}", summary.good_count)),
            ]),
            Line::from(vec![
                Span::styled("Easy:   ", Style::default().fg(Color::Cyan)),
                Span::raw(format!("{}", summary.easy_count)),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press any key to exit...",
                Style::default().fg(Color::DarkGray),
            )]),
        ];

        let summary_widget = Paragraph::new(summary_text)
            .block(
                Block::default()
                    .title(" Review Session Complete! ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(summary_widget, summary_area);
    }
}

impl Drop for ReviewApp {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_state_transitions() {
        assert_eq!(ReviewState::ShowingQuestion, ReviewState::ShowingQuestion);
        assert_ne!(ReviewState::ShowingQuestion, ReviewState::ShowingAnswer);
    }
}

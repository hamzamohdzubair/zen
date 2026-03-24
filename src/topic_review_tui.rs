//! TUI for topic review sessions

use anyhow::{Context, Result};
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

use crate::config::Config;
use crate::llm_evaluator::{AnswerEvaluator, QuestionGenerator};
use crate::topic_review::{QuestionData, TopicReviewSession};

/// State machine for the review TUI
#[derive(Debug, Clone, PartialEq)]
enum ReviewState {
    ShowingTopic,
    GeneratingQuestion(usize),  // Question number 1, 2, or 3
    ShowingQuestion(usize),
    InputtingAnswer(usize),
    EvaluatingAnswer(usize),
    ShowingFeedback(usize),
    ShowingResults,
    Complete,
}

/// Topic review TUI application
pub struct TopicReviewApp {
    session: TopicReviewSession,
    state: ReviewState,
    questions: [Option<String>; 3],
    answers: [Option<String>; 3],
    scores: [Option<f64>; 3],
    feedback: [Option<String>; 3],
    ideal_answers: [Option<String>; 3],
    current_input_lines: Vec<String>,
    current_line: String,
    llm_evaluator: Box<dyn AnswerEvaluator>,
    llm_generator: Box<dyn QuestionGenerator>,
    status_message: String,
    /// All questions asked in this session (across all topics) to avoid overlap
    session_questions: Vec<String>,
}

impl TopicReviewApp {
    pub fn new() -> Result<()> {
        // Load LLM config
        let config = Config::load()?;
        let llm_config = config
            .llm
            .context("No LLM configuration found. Please set up ~/.zen/config.toml")?;

        // Create evaluator and generator (two instances of same LLM client)
        let evaluator = crate::llm_evaluator::create_evaluator(&llm_config)?;
        let generator: Box<dyn QuestionGenerator> = Box::new(crate::llm_evaluator::GroqEvaluator::new(
            &llm_config.api_key,
            &llm_config.model,
        )?);

        // Create session
        let session = TopicReviewSession::new()?;

        let mut app = Self {
            session,
            state: ReviewState::ShowingTopic,
            questions: [None, None, None],
            answers: [None, None, None],
            scores: [None, None, None],
            feedback: [None, None, None],
            ideal_answers: [None, None, None],
            current_input_lines: Vec::new(),
            current_line: String::new(),
            llm_evaluator: evaluator,
            llm_generator: generator,
            status_message: String::new(),
            session_questions: Vec::new(),
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

            // Handle auto-transitions (for LLM calls)
            let next_state = match &self.state {
                ReviewState::ShowingTopic => {
                    Some(ReviewState::GeneratingQuestion(0))
                }
                ReviewState::GeneratingQuestion(q_num) => {
                    let q = *q_num;
                    if let Err(e) = self.generate_question(q) {
                        self.status_message = format!("Error: {}", e);
                        None
                    } else {
                        Some(ReviewState::ShowingQuestion(q))
                    }
                }
                ReviewState::EvaluatingAnswer(q_num) => {
                    let q = *q_num;
                    if let Err(e) = self.evaluate_answer(q) {
                        self.status_message = format!("Error: {}", e);
                        None
                    } else {
                        Some(ReviewState::ShowingFeedback(q))
                    }
                }
                _ => None,
            };

            if let Some(state) = next_state {
                self.state = state;
                continue;
            }

            // Handle user input
            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        break;
                    }

                    match self.handle_input(key.code) {
                        Ok(should_continue) => {
                            if !should_continue {
                                break;
                            }
                        }
                        Err(e) => {
                            self.status_message = format!("Error: {}", e);
                        }
                    }
                }
            }

            if self.state == ReviewState::Complete {
                break;
            }
        }

        Ok(())
    }

    fn handle_input(&mut self, key: KeyCode) -> Result<bool> {
        match &self.state {
            ReviewState::ShowingQuestion(q_num) => {
                if key == KeyCode::Char('s') {
                    // Skip question - mark as 0% and evaluate to show ideal answer
                    self.answers[*q_num] = Some("(Skipped)".to_string());
                    self.state = ReviewState::EvaluatingAnswer(*q_num);
                } else if matches!(key, KeyCode::Char(' ') | KeyCode::Enter) {
                    // Space or Enter to start typing answer
                    self.state = ReviewState::InputtingAnswer(*q_num);
                    self.current_input_lines.clear();
                    self.current_line.clear();
                }
            }
            ReviewState::InputtingAnswer(q_num) => {
                match key {
                    KeyCode::Char(c) => {
                        self.current_line.push(c);
                    }
                    KeyCode::Backspace => {
                        if self.current_line.is_empty() && !self.current_input_lines.is_empty() {
                            // Move to previous line
                            self.current_line = self.current_input_lines.pop().unwrap();
                        } else {
                            self.current_line.pop();
                        }
                    }
                    KeyCode::Enter => {
                        // Check if this is second Enter (submit)
                        if self.current_line.is_empty() && !self.current_input_lines.is_empty() {
                            // Submit answer
                            let answer = self.current_input_lines.join("\n");
                            self.answers[*q_num] = Some(answer);
                            self.state = ReviewState::EvaluatingAnswer(*q_num);
                        } else {
                            // New line
                            self.current_input_lines.push(self.current_line.clone());
                            self.current_line.clear();
                        }
                    }
                    _ => {}
                }
            }
            ReviewState::ShowingFeedback(q_num) => {
                // Space to continue
                if key == KeyCode::Char(' ') {
                    if *q_num < 2 {
                        // Next question
                        self.state = ReviewState::GeneratingQuestion(*q_num + 1);
                    } else {
                        // All 3 questions done, show results
                        self.submit_review()?;
                        self.state = ReviewState::ShowingResults;
                    }
                }
            }
            ReviewState::ShowingResults => {
                // Space to next topic or complete
                if key == KeyCode::Char(' ') {
                    if self.session.is_complete() {
                        self.state = ReviewState::Complete;
                    } else {
                        // Reset for next topic
                        self.questions = [None, None, None];
                        self.answers = [None, None, None];
                        self.scores = [None, None, None];
                        self.feedback = [None, None, None];
                        self.ideal_answers = [None, None, None];
                        self.current_input_lines.clear();
                        self.current_line.clear();
                        self.state = ReviewState::ShowingTopic;
                    }
                }
            }
            ReviewState::Complete => {
                return Ok(false);
            }
            _ => {}
        }

        Ok(true)
    }

    fn generate_question(&mut self, q_num: usize) -> Result<()> {
        self.status_message = format!("Generating question {}...", q_num + 1);

        let topic = self.session.current_topic().context("No current topic")?;

        // Get previous questions for this topic from database (only "Easy" rated ones)
        let mut previous_questions = self.session.get_previous_questions()?;

        // Add all questions from the current session (across all topics) to avoid overlap
        previous_questions.extend(self.session_questions.iter().cloned());

        let question = self.llm_generator.generate_question(&topic.keywords, &previous_questions)?;

        // Track this question in the session to avoid duplicates across topics
        self.session_questions.push(question.clone());
        self.questions[q_num] = Some(question);
        self.status_message.clear();

        Ok(())
    }

    fn evaluate_answer(&mut self, q_num: usize) -> Result<()> {
        self.status_message = "Evaluating answer...".to_string();

        let question = self.questions[q_num]
            .as_ref()
            .context("No question")?;
        let answer = self.answers[q_num]
            .as_ref()
            .context("No answer")?;

        let evaluation = self.llm_evaluator.evaluate_answer(question, answer)?;

        // If answer was skipped, force score to 0
        let final_score = if answer == "(Skipped)" {
            Some(0.0)
        } else {
            evaluation.score
        };

        self.scores[q_num] = final_score;
        self.feedback[q_num] = if answer == "(Skipped)" {
            Some("Question skipped".to_string())
        } else {
            evaluation.feedback
        };
        self.ideal_answers[q_num] = Some(evaluation.ideal_answer);
        self.status_message.clear();

        Ok(())
    }

    fn submit_review(&mut self) -> Result<()> {
        // Calculate average score (including 0% for skipped questions)
        let scores: Vec<f64> = self.scores.iter().filter_map(|&s| s).collect();
        let average_score = if scores.is_empty() {
            0.0
        } else {
            scores.iter().sum::<f64>() / scores.len() as f64
        };

        // Create question data
        let mut questions_data = Vec::new();
        for i in 0..3 {
            if let (Some(question), Some(answer), Some(score), feedback) = (
                &self.questions[i],
                &self.answers[i],
                self.scores[i],
                &self.feedback[i],
            ) {
                questions_data.push(QuestionData {
                    question: question.clone(),
                    user_answer: answer.clone(),
                    score,
                    feedback: feedback.clone(),
                });
            }
        }

        self.session.submit_review(average_score, questions_data)?;

        Ok(())
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Main layout - vertical split
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),  // Topic display
                Constraint::Min(10),    // Content area
                Constraint::Length(3),  // Rating conversion table
                Constraint::Length(3),  // Status/instructions
            ])
            .split(size);

        // Render topic section
        self.render_topic_section(f, main_chunks[0]);

        // Content area - horizontal split (main content left, feedback right)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),  // Main content (left)
                Constraint::Percentage(40),  // Feedback boxes (right)
            ])
            .split(main_chunks[1]);

        // Render main content based on state
        match &self.state {
            ReviewState::ShowingTopic | ReviewState::GeneratingQuestion(_) => {
                self.render_loading(f, content_chunks[0]);
            }
            ReviewState::ShowingQuestion(q_num) | ReviewState::InputtingAnswer(q_num) => {
                self.render_question_and_answer(f, content_chunks[0], *q_num);
            }
            ReviewState::EvaluatingAnswer(_) => {
                self.render_loading(f, content_chunks[0]);
            }
            ReviewState::ShowingFeedback(q_num) => {
                self.render_feedback(f, content_chunks[0], *q_num);
            }
            ReviewState::ShowingResults => {
                self.render_results(f, content_chunks[0]);
            }
            ReviewState::Complete => {
                self.render_complete(f, content_chunks[0]);
            }
        }

        // Render feedback boxes on right side
        self.render_feedback_boxes(f, content_chunks[1]);

        // Render rating conversion table
        self.render_rating_table(f, main_chunks[2]);

        // Render status
        self.render_status(f, main_chunks[3]);
    }

    fn render_topic_section(&self, f: &mut Frame, area: Rect) {
        if let Some(topic) = self.session.current_topic() {
            let (current, total) = self.session.progress();
            let title = format!("Topic {} of {}", current, total);

            let keywords_text = topic.keywords.join(", ");

            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan));

            let paragraph = Paragraph::new(keywords_text)
                .block(block)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        }
    }

    fn render_loading(&self, f: &mut Frame, area: Rect) {
        let msg = if !self.status_message.is_empty() {
            &self.status_message
        } else {
            "Loading..."
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default());

        let paragraph = Paragraph::new(msg)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_question_and_answer(&self, f: &mut Frame, area: Rect, q_num: usize) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Question section
        if let Some(question) = &self.questions[q_num] {
            let block = Block::default()
                .title(format!("Question {} of 3", q_num + 1))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow));

            let paragraph = Paragraph::new(question.as_str())
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[0]);
        }

        // Answer section
        let is_inputting = matches!(self.state, ReviewState::InputtingAnswer(_));
        let title = if is_inputting {
            "Your Answer (Press Enter twice to submit)"
        } else {
            "Your Answer (Press Space/Enter to start typing)"
        };

        let answer_text = if is_inputting {
            let mut lines = self.current_input_lines.clone();
            lines.push(format!("{}█", self.current_line));
            lines.join("\n")
        } else if let Some(answer) = &self.answers[q_num] {
            answer.clone()
        } else {
            String::new()
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(answer_text)
            .block(block)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, chunks[1]);
    }

    fn render_feedback(&self, f: &mut Frame, area: Rect, q_num: usize) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),  // Question
                Constraint::Length(6),  // Your answer
                Constraint::Min(5),     // Ideal answer
            ])
            .split(area);

        // Question
        if let Some(question) = &self.questions[q_num] {
            let block = Block::default()
                .title(format!("Question {} of 3", q_num + 1))
                .borders(Borders::ALL);

            let paragraph = Paragraph::new(question.as_str())
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[0]);
        }

        // Your answer
        if let Some(answer) = &self.answers[q_num] {
            let block = Block::default()
                .title("Your Answer")
                .borders(Borders::ALL);

            let paragraph = Paragraph::new(answer.as_str())
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[1]);
        }

        // Ideal answer (100% scoring answer)
        if let Some(ideal_answer) = &self.ideal_answers[q_num] {
            if !ideal_answer.is_empty() {
                let block = Block::default()
                    .title("100% Answer")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Cyan));

                let paragraph = Paragraph::new(ideal_answer.as_str())
                    .block(block)
                    .wrap(Wrap { trim: true });

                f.render_widget(paragraph, chunks[2]);
            }
        }
    }

    fn render_results(&self, f: &mut Frame, area: Rect) {
        let mut lines = Vec::new();

        // Summary of all 3 questions
        for i in 0..3 {
            if let Some(score) = self.scores[i] {
                let rating = if score >= 90.0 {
                    "Easy"
                } else if score >= 70.0 {
                    "Good"
                } else if score >= 60.0 {
                    "Hard"
                } else {
                    "Again"
                };

                let color = if score >= 90.0 {
                    Color::Cyan
                } else if score >= 70.0 {
                    Color::Green
                } else if score >= 60.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };

                lines.push(Line::from(vec![
                    Span::raw(format!("Question {}: ", i + 1)),
                    Span::styled(
                        format!("{:.0}/100 ({})", score, rating),
                        Style::default().fg(color),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));

        // Calculate average
        let scores: Vec<f64> = self.scores.iter().filter_map(|&s| s).collect();
        let average_score = scores.iter().sum::<f64>() / scores.len() as f64;
        let final_rating = crate::topic_review::score_to_rating(average_score);
        let rating_text = match final_rating {
            4 => "Easy",
            3 => "Good",
            2 => "Hard",
            1 => "Again",
            _ => "Unknown",
        };

        lines.push(Line::from(vec![
            Span::styled("Average: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:.1}%", average_score),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Final Rating: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                rating_text,
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));

        let block = Block::default()
            .title("Review Complete")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_complete(&self, f: &mut Frame, area: Rect) {
        let (_, total) = self.session.progress();

        let text = format!("All {} topics reviewed!\n\nGreat work!", total);

        let block = Block::default()
            .title("Session Complete")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_rating_table(&self, f: &mut Frame, area: Rect) {
        let text = "Rating Conversion: 90%+ → Easy  |  70-90% → Good  |  60-70% → Hard  |  <60% → Again";

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::DarkGray));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_feedback_boxes(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),  // Current feedback
                Constraint::Percentage(50),  // Cumulative report
            ])
            .split(area);

        // Current question feedback
        let current_q_num = match &self.state {
            ReviewState::ShowingQuestion(q) | ReviewState::InputtingAnswer(q) |
            ReviewState::EvaluatingAnswer(q) | ReviewState::ShowingFeedback(q) => Some(*q),
            _ => None,
        };

        if let Some(q_num) = current_q_num {
            let mut lines = Vec::new();

            if let Some(score) = self.scores[q_num] {
                let rating_text = if score >= 90.0 {
                    "Easy"
                } else if score >= 70.0 {
                    "Good"
                } else if score >= 60.0 {
                    "Hard"
                } else {
                    "Again"
                };

                let score_color = if score >= 90.0 {
                    Color::Cyan
                } else if score >= 70.0 {
                    Color::Green
                } else if score >= 60.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };

                lines.push(Line::from(vec![
                    Span::styled("Score: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{:.0}/100", score),
                        Style::default().fg(score_color).add_modifier(Modifier::BOLD),
                    ),
                ]));

                lines.push(Line::from(vec![
                    Span::styled("Rating: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        rating_text,
                        Style::default().fg(score_color).add_modifier(Modifier::BOLD),
                    ),
                ]));

                lines.push(Line::from(""));

                if let Some(feedback) = &self.feedback[q_num] {
                    lines.push(Line::from(Span::styled("Feedback:", Style::default().add_modifier(Modifier::BOLD))));
                    lines.push(Line::from(feedback.as_str()));
                }
            } else {
                lines.push(Line::from("Answer the question to see feedback"));
            }

            let block = Block::default()
                .title(format!("Current (Q{})", q_num + 1))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow));

            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[0]);
        } else {
            let block = Block::default()
                .title("Current Feedback")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::DarkGray));

            let paragraph = Paragraph::new("No active question")
                .block(block)
                .alignment(Alignment::Center);

            f.render_widget(paragraph, chunks[0]);
        }

        // Cumulative report card
        let mut report_lines = Vec::new();

        for i in 0..3 {
            if let Some(score) = self.scores[i] {
                let rating = if score >= 90.0 {
                    "Easy"
                } else if score >= 70.0 {
                    "Good"
                } else if score >= 60.0 {
                    "Hard"
                } else {
                    "Again"
                };

                let color = if score >= 90.0 {
                    Color::Cyan
                } else if score >= 70.0 {
                    Color::Green
                } else if score >= 60.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };

                report_lines.push(Line::from(vec![
                    Span::raw(format!("Q{}: ", i + 1)),
                    Span::styled(
                        format!("{:.0}%", score),
                        Style::default().fg(color),
                    ),
                    Span::raw(format!(" ({})", rating)),
                ]));
            } else {
                report_lines.push(Line::from(format!("Q{}: --", i + 1)));
            }
        }

        // Calculate running average
        let answered_scores: Vec<f64> = self.scores.iter().filter_map(|&s| s).collect();
        if !answered_scores.is_empty() {
            let avg = answered_scores.iter().sum::<f64>() / answered_scores.len() as f64;
            report_lines.push(Line::from(""));
            report_lines.push(Line::from(vec![
                Span::styled("Average: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.1}%", avg),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]));

            let rating = if avg >= 90.0 {
                "Easy"
            } else if avg >= 70.0 {
                "Good"
            } else if avg >= 60.0 {
                "Hard"
            } else {
                "Again"
            };

            report_lines.push(Line::from(vec![
                Span::styled("Rating: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    rating,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        let block = Block::default()
            .title("Report Card")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(report_lines)
            .block(block)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, chunks[1]);
    }

    fn render_status(&self, f: &mut Frame, area: Rect) {
        let instruction = match &self.state {
            ReviewState::ShowingQuestion(_) => "Space/Enter: Answer | S: Skip (0%)",
            ReviewState::InputtingAnswer(_) => "Enter twice to submit | Ctrl+C to quit",
            ReviewState::ShowingFeedback(_) => "Press Space to continue",
            ReviewState::ShowingResults => "Press Space to continue",
            ReviewState::Complete => "Press any key to exit",
            _ => "Loading...",
        };

        let paragraph = Paragraph::new(instruction)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }
}

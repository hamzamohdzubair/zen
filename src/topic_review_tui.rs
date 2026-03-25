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
use rand::seq::SliceRandom;

use crate::config::Config;
use crate::database;
use crate::llm_evaluator::{AnswerEvaluator, QuestionGenerator};
use crate::topic_review::TopicReviewSession;

/// State machine for the review TUI
#[derive(Debug, Clone, PartialEq)]
enum ReviewState {
    SelectingNextTopic,
    GeneratingQuestion,
    ShowingQuestion,
    InputtingAnswer,
    EvaluatingAnswer,
    ShowingFeedback,
    ShowingResults,
    Complete,
}

/// Status of a single question for a topic
#[derive(Debug, Clone, Copy, PartialEq)]
enum QuestionStatus {
    Pending,       // Not yet asked
    Correct,       // Answered with score >= 60%
    Incorrect,     // Answered with score < 60%
}

/// Topic progress tracking
#[derive(Debug, Clone)]
struct TopicProgress {
    topic_id: String,
    topic_index: usize,
    keywords: Vec<String>,
    questions_status: [QuestionStatus; 3],  // 3 questions per topic
    scores: [Option<f64>; 3],
    current_question_num: usize,  // Which question (0-2) we're on
    review_log_id: Option<i64>,   // Single review_log for all 3 questions
}

/// Current question being shown
#[derive(Debug, Clone)]
struct CurrentQuestion {
    topic_index: usize,
    question_num: usize,
    question_text: String,
    answer: Option<String>,
    score: Option<f64>,
    feedback: Option<String>,
    ideal_answer: Option<String>,
}

/// Topic review TUI application
pub struct TopicReviewApp {
    session: TopicReviewSession,
    state: ReviewState,
    topic_progress: Vec<TopicProgress>,  // Track all topics and their question status
    current_question: Option<CurrentQuestion>,
    current_input_lines: Vec<String>,
    current_line: String,
    llm_evaluator: Box<dyn AnswerEvaluator>,
    llm_generator: Box<dyn QuestionGenerator>,
    status_message: String,
    /// Shuffled order of topic indices for random selection
    topic_order: Vec<usize>,
    /// Current position in topic_order
    topic_order_index: usize,
}

impl TopicReviewApp {
    pub fn new() -> Result<()> {
        // Load LLM config
        let config = Config::load()?;
        let llm_config = config
            .llm
            .context("No LLM configuration found. Please set up ~/.zen/config.toml")?;

        // Get web search config if available
        let web_search = config.web_search.clone();

        // Create evaluator and generator
        let evaluator = crate::llm_evaluator::create_evaluator(&llm_config, web_search.clone())?;
        let generator = crate::llm_evaluator::create_question_generator(&llm_config, web_search)?;

        // Create session
        let session = TopicReviewSession::new()?;
        let conn = database::init_database()?;

        // Initialize topic progress
        let mut topic_progress = Vec::new();
        let mut seen_keyword_sets = std::collections::HashSet::new();

        // DEBUG: Log all due topics to understand duplicates
        eprintln!("DEBUG: Due topics count: {}", session.due_topics.len());
        for (idx, topic) in session.due_topics.iter().enumerate() {
            eprintln!("DEBUG: Topic[{}]: id={}, keywords={}", idx, topic.topic_id, topic.keywords.join(","));
        }

        for (topic_index, topic) in session.due_topics.iter().enumerate() {
            // Deduplicate by keywords - skip if we've seen this keyword set already
            let keyword_key = topic.keywords.join(",");
            if seen_keyword_sets.contains(&keyword_key) {
                eprintln!("DEBUG: Skipping duplicate topic with keywords: {}", keyword_key);
                continue;
            }
            seen_keyword_sets.insert(keyword_key);

            // Check how many questions were already answered today for this topic
            let today_count = database::get_today_question_count(&conn, &topic.topic_id)?;
            let today_scores = database::get_today_question_scores(&conn, &topic.topic_id)?;

            let mut questions_status = [QuestionStatus::Pending, QuestionStatus::Pending, QuestionStatus::Pending];
            let mut scores = [None, None, None];
            let mut current_question_num = 0;

            // Mark already answered questions as completed and load their scores
            if today_count > 0 {
                for i in 0..today_count.min(3) {
                    if let Some(&score) = today_scores.get(i) {
                        scores[i] = Some(score);
                        questions_status[i] = if score >= 60.0 {
                            QuestionStatus::Correct
                        } else {
                            QuestionStatus::Incorrect
                        };
                    } else {
                        questions_status[i] = QuestionStatus::Correct;  // Fallback
                    }
                    current_question_num = i + 1;
                }
            }

            // Only include topics that have remaining questions
            if current_question_num < 3 {
                topic_progress.push(TopicProgress {
                    topic_id: topic.topic_id.clone(),
                    topic_index,
                    keywords: topic.keywords.clone(),
                    questions_status,
                    scores,
                    current_question_num,
                    review_log_id: None,  // Will be created when first question is answered
                });
            }
        }

        if topic_progress.is_empty() {
            anyhow::bail!("All topics have been completed for today!");
        }

        // Create random topic order for interleaving
        let mut topic_order: Vec<usize> = (0..topic_progress.len()).collect();
        let mut rng = rand::thread_rng();
        topic_order.shuffle(&mut rng);

        let mut app = Self {
            session,
            state: ReviewState::SelectingNextTopic,
            topic_progress,
            current_question: None,
            current_input_lines: Vec::new(),
            current_line: String::new(),
            llm_evaluator: evaluator,
            llm_generator: generator,
            status_message: String::new(),
            topic_order,
            topic_order_index: 0,
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
                ReviewState::SelectingNextTopic => {
                    if let Err(e) = self.select_next_topic() {
                        self.status_message = format!("Error: {}", e);
                        None
                    } else if self.current_question.is_some() {
                        Some(ReviewState::GeneratingQuestion)
                    } else {
                        // All topics completed
                        Some(ReviewState::ShowingResults)
                    }
                }
                ReviewState::GeneratingQuestion => {
                    if let Err(e) = self.generate_question() {
                        self.status_message = format!("Error: {}", e);
                        None
                    } else {
                        Some(ReviewState::ShowingQuestion)
                    }
                }
                ReviewState::EvaluatingAnswer => {
                    if let Err(e) = self.evaluate_answer() {
                        self.status_message = format!("Error: {}", e);
                        None
                    } else {
                        Some(ReviewState::ShowingFeedback)
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
            ReviewState::ShowingQuestion => {
                if key == KeyCode::Char('s') {
                    // Skip question - mark as 0% and evaluate to show ideal answer
                    if let Some(ref mut q) = self.current_question {
                        q.answer = Some("(Skipped)".to_string());
                        self.state = ReviewState::EvaluatingAnswer;
                    }
                } else if matches!(key, KeyCode::Char(' ') | KeyCode::Enter) {
                    // Space or Enter to start typing answer
                    self.state = ReviewState::InputtingAnswer;
                    self.current_input_lines.clear();
                    self.current_line.clear();
                }
            }
            ReviewState::InputtingAnswer => {
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
                            if let Some(ref mut q) = self.current_question {
                                q.answer = Some(answer);
                                self.state = ReviewState::EvaluatingAnswer;
                            }
                        } else {
                            // New line
                            self.current_input_lines.push(self.current_line.clone());
                            self.current_line.clear();
                        }
                    }
                    _ => {}
                }
            }
            ReviewState::ShowingFeedback => {
                // Space to continue to next question
                if key == KeyCode::Char(' ') {
                    self.state = ReviewState::SelectingNextTopic;
                }
            }
            ReviewState::ShowingResults => {
                // Space to complete
                if key == KeyCode::Char(' ') {
                    self.state = ReviewState::Complete;
                }
            }
            ReviewState::Complete => {
                return Ok(false);
            }
            _ => {}
        }

        Ok(true)
    }

    fn select_next_topic(&mut self) -> Result<()> {
        // Find next topic that needs questions
        loop {
            if self.topic_order_index >= self.topic_order.len() {
                // Completed one round through all topics, check if any need more questions
                let has_remaining = self.topic_progress.iter().any(|tp| {
                    tp.questions_status.iter().any(|s| *s == QuestionStatus::Pending)
                });

                if !has_remaining {
                    // All topics completed!
                    self.current_question = None;
                    return Ok(());
                }

                // Reset to beginning and reshuffle
                self.topic_order_index = 0;
                let mut rng = rand::thread_rng();
                self.topic_order.shuffle(&mut rng);
            }

            let progress_index = self.topic_order[self.topic_order_index];
            let topic_prog = &self.topic_progress[progress_index];

            // Check if this topic has pending questions
            if topic_prog.current_question_num < 3 {
                // Found a topic that needs a question
                self.current_question = Some(CurrentQuestion {
                    topic_index: topic_prog.topic_index,
                    question_num: topic_prog.current_question_num,
                    question_text: String::new(),  // Will be filled by generate_question
                    answer: None,
                    score: None,
                    feedback: None,
                    ideal_answer: None,
                });
                return Ok(());
            }

            self.topic_order_index += 1;
        }
    }

    fn generate_question(&mut self) -> Result<()> {
        let current_q = self.current_question.as_ref().context("No current question")?;
        let topic_prog = &self.topic_progress.iter()
            .find(|tp| tp.topic_index == current_q.topic_index)
            .context("Topic not found")?;

        self.status_message = format!("Generating question for topic...");

        // Get previous questions for this topic from database (historical, rating=4 only)
        let conn = database::init_database()?;
        let mut previous_questions = database::get_topic_previous_questions(&conn, &topic_prog.topic_id, 10)?;

        // Add ALL questions asked today (across all topics) to avoid repetition within the same day
        // This ensures a "session" is defined as the entire day, not just one run
        let today_questions = database::get_all_questions_asked_today(&conn)?;
        previous_questions.extend(today_questions);

        let question_text = self.llm_generator.generate_question(&topic_prog.keywords, &previous_questions)?;

        // Update current question
        if let Some(ref mut q) = self.current_question {
            q.question_text = question_text;
        }

        self.status_message.clear();
        Ok(())
    }

    fn evaluate_answer(&mut self) -> Result<()> {
        self.status_message = "Evaluating answer...".to_string();

        // Extract data from current_question first
        let (question_text, answer, topic_index, question_num) = {
            let current_q = self.current_question.as_ref().context("No current question")?;
            (
                current_q.question_text.clone(),
                current_q.answer.clone().context("No answer")?,
                current_q.topic_index,
                current_q.question_num,
            )
        };

        let evaluation = self.llm_evaluator.evaluate_answer(&question_text, &answer)?;

        // If answer was skipped, force score to 0
        let final_score = if answer == "(Skipped)" {
            0.0
        } else {
            evaluation.score.unwrap_or(0.0)
        };

        let feedback = if answer == "(Skipped)" {
            Some("Question skipped".to_string())
        } else {
            evaluation.feedback.clone()
        };

        // Update current question with results
        if let Some(ref mut q) = self.current_question {
            q.score = Some(final_score);
            q.feedback = feedback.clone();
            q.ideal_answer = Some(evaluation.ideal_answer);
        }

        // **IMMEDIATELY SAVE TO DATABASE**
        let conn = database::init_database()?;

        let topic_prog = self.topic_progress.iter_mut()
            .find(|tp| tp.topic_index == topic_index)
            .context("Topic not found")?;

        // Create review_log if this is the first question for this topic
        if topic_prog.review_log_id.is_none() {
            let review_log_id = database::create_topic_review_session(&conn, &topic_prog.topic_id)?;
            topic_prog.review_log_id = Some(review_log_id);
        }

        let review_log_id = topic_prog.review_log_id.unwrap();

        // Insert this question's log
        database::insert_topic_question_log(
            &conn,
            review_log_id,
            (question_num + 1) as i32,
            &question_text,
            &answer,
            final_score,
            feedback.as_deref(),
        )?;

        // Update topic progress
        let status = if final_score >= 60.0 {
            QuestionStatus::Correct
        } else {
            QuestionStatus::Incorrect
        };
        topic_prog.questions_status[question_num] = status;
        topic_prog.scores[question_num] = Some(final_score);
        topic_prog.current_question_num += 1;

        // Check if this topic is now complete (all 3 questions answered)
        if topic_prog.current_question_num >= 3 {
            // Calculate average score for this topic
            let scores: Vec<f64> = topic_prog.scores.iter().filter_map(|&s| s).collect();
            let average_score = scores.iter().sum::<f64>() / scores.len() as f64;

            // Update FSRS schedule
            let questions_data: Vec<crate::topic_review::QuestionData> = scores.iter().enumerate().map(|(i, &score)| {
                crate::topic_review::QuestionData {
                    question: format!("Question {}", i + 1),
                    user_answer: String::new(),
                    score,
                    feedback: None,
                }
            }).collect();

            self.session.current_index = topic_prog.topic_index;
            self.session.submit_review(average_score, questions_data)?;

            // Update the review_log with final values
            let rating = crate::topic_review::score_to_rating(average_score);
            database::update_review_log(&conn, review_log_id, rating, 0.0, 0.0, average_score)?;
        }

        // Move to next topic in rotation
        self.topic_order_index += 1;

        self.status_message.clear();
        Ok(())
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Main layout - vertical split
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),  // Topic display
                Constraint::Length(4),  // Progress bar
                Constraint::Min(10),    // Content area
                Constraint::Length(3),  // Rating conversion table
                Constraint::Length(3),  // Status/instructions
            ])
            .split(size);

        // Render topic section
        self.render_topic_section(f, main_chunks[0]);

        // Render progress bar
        self.render_progress_bar(f, main_chunks[1]);

        // Content area - horizontal split (main content left, feedback right)
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60),  // Main content (left)
                Constraint::Percentage(40),  // Feedback boxes (right)
            ])
            .split(main_chunks[2]);

        // Render main content based on state
        match &self.state {
            ReviewState::SelectingNextTopic | ReviewState::GeneratingQuestion | ReviewState::EvaluatingAnswer => {
                self.render_loading(f, content_chunks[0]);
            }
            ReviewState::ShowingQuestion | ReviewState::InputtingAnswer => {
                self.render_question_and_answer(f, content_chunks[0]);
            }
            ReviewState::ShowingFeedback => {
                self.render_feedback(f, content_chunks[0]);
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
        self.render_rating_table(f, main_chunks[3]);

        // Render status
        self.render_status(f, main_chunks[4]);
    }

    fn render_topic_section(&self, f: &mut Frame, area: Rect) {
        let (title, keywords_text) = if let Some(current_q) = &self.current_question {
            let topic_prog = self.topic_progress.iter()
                .find(|tp| tp.topic_index == current_q.topic_index);

            if let Some(tp) = topic_prog {
                let title = format!(
                    "Current Topic (Question {}/3)",
                    current_q.question_num + 1
                );
                (title, tp.keywords.join(", "))
            } else {
                ("Current Topic".to_string(), String::new())
            }
        } else {
            (format!("Reviewing {} Topics", self.topic_progress.len()), "Starting session...".to_string())
        };

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

    fn render_question_and_answer(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(area);

        // Question section
        if let Some(question) = &self.current_question {
            let block = Block::default()
                .title("Question")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Yellow));

            let paragraph = Paragraph::new(question.question_text.as_str())
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[0]);
        }

        // Answer section
        let is_inputting = matches!(self.state, ReviewState::InputtingAnswer);
        let title = if is_inputting {
            "Your Answer (Press Enter twice to submit)"
        } else {
            "Your Answer (Press Space/Enter to start typing, S to skip)"
        };

        let answer_text = if is_inputting {
            let mut lines = self.current_input_lines.clone();
            lines.push(format!("{}█", self.current_line));
            lines.join("\n")
        } else if let Some(question) = &self.current_question {
            question.answer.clone().unwrap_or_default()
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

    fn render_feedback(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),  // Question
                Constraint::Length(6),  // Your answer
                Constraint::Min(5),     // Ideal answer
            ])
            .split(area);

        if let Some(question) = &self.current_question {
            // Question
            let block = Block::default()
                .title("Question")
                .borders(Borders::ALL);

            let paragraph = Paragraph::new(question.question_text.as_str())
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, chunks[0]);

            // Your answer
            if let Some(answer) = &question.answer {
                let block = Block::default()
                    .title("Your Answer")
                    .borders(Borders::ALL);

                let paragraph = Paragraph::new(answer.as_str())
                    .block(block)
                    .wrap(Wrap { trim: true });

                f.render_widget(paragraph, chunks[1]);
            }

            // Ideal answer (100% scoring answer)
            if let Some(ideal_answer) = &question.ideal_answer {
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
    }

    fn render_results(&self, f: &mut Frame, area: Rect) {
        let mut lines = Vec::new();

        lines.push(Line::from(Span::styled(
            "Review Complete!",
            Style::default().add_modifier(Modifier::BOLD).fg(Color::Green),
        )));
        lines.push(Line::from(""));

        // Show each topic's result
        for topic_prog in &self.topic_progress {
            let scores: Vec<f64> = topic_prog.scores.iter().filter_map(|&s| s).collect();
            if !scores.is_empty() {
                let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
                let rating = if avg_score >= 90.0 {
                    "Easy"
                } else if avg_score >= 70.0 {
                    "Good"
                } else if avg_score >= 60.0 {
                    "Hard"
                } else {
                    "Again"
                };

                let color = if avg_score >= 90.0 {
                    Color::Cyan
                } else if avg_score >= 70.0 {
                    Color::Green
                } else if avg_score >= 60.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };

                let keywords_short = topic_prog.keywords.join(", ");
                let keywords_display = if keywords_short.len() > 40 {
                    format!("{}...", &keywords_short[..37])
                } else {
                    keywords_short
                };

                lines.push(Line::from(vec![
                    Span::raw(format!("{}: ", keywords_display)),
                    Span::styled(
                        format!("{:.0}% ({})", avg_score, rating),
                        Style::default().fg(color),
                    ),
                ]));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Press Space to exit"));

        let block = Block::default()
            .title("Session Complete")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_complete(&self, f: &mut Frame, area: Rect) {
        let total_answered: usize = self.topic_progress.iter()
            .map(|tp| tp.questions_status.iter().filter(|&&s| s != QuestionStatus::Pending).count())
            .sum();

        let text = format!(
            "Session ended!\n\n{} questions answered\n\nGreat work!",
            total_answered
        );

        let block = Block::default()
            .title("Session Complete")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }

    fn render_progress_bar(&self, f: &mut Frame, area: Rect) {
        let total_questions: usize = self.topic_progress.iter().map(|tp| 3 - tp.current_question_num).sum();
        let total_questions = total_questions + self.topic_progress.iter()
            .map(|tp| tp.current_question_num)
            .sum::<usize>();

        let answered_questions: usize = self.topic_progress.iter()
            .map(|tp| tp.questions_status.iter().filter(|&&s| s != QuestionStatus::Pending).count())
            .sum();

        let percentage = if total_questions > 0 {
            (answered_questions as f64 / total_questions as f64 * 100.0) as usize
        } else {
            0
        };

        // Build progress bar text
        let bar_width = 30;
        let filled = (bar_width * percentage) / 100;
        let empty = bar_width - filled;

        let bar = format!(
            "[{}{}] {}%",
            "=".repeat(filled),
            " ".repeat(empty),
            percentage
        );

        let lines = vec![
            Line::from(vec![
                Span::styled("Progress: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{}/{} questions answered  ", answered_questions, total_questions)),
                Span::styled(bar, Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("Topics: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!("{} topics remaining", self.topic_progress.len())),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Blue));

        let paragraph = Paragraph::new(lines)
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
                Constraint::Percentage(50),  // Topic report card
            ])
            .split(area);

        // Current question feedback
        if let Some(current_q) = &self.current_question {
            let mut lines = Vec::new();

            if let Some(score) = current_q.score {
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

                if let Some(feedback) = &current_q.feedback {
                    lines.push(Line::from(Span::styled("Feedback:", Style::default().add_modifier(Modifier::BOLD))));
                    // Limit feedback to avoid overflow
                    let feedback_short = if feedback.len() > 150 {
                        format!("{}...", &feedback[..147])
                    } else {
                        feedback.clone()
                    };
                    lines.push(Line::from(feedback_short));
                }
            } else {
                lines.push(Line::from("Answer the question to see feedback"));
            }

            let block = Block::default()
                .title("Current Feedback")
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

        // Topic report card with dots - formatted as table
        let mut report_lines = Vec::new();

        report_lines.push(Line::from(Span::styled(
            "Topics Progress:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        report_lines.push(Line::from(""));

        // Table header (aligned with data columns)
        report_lines.push(Line::from(vec![
            Span::styled(format!("{:<40}", "Topic"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled("Status", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled("Avg", Style::default().add_modifier(Modifier::BOLD)),
        ]));
        report_lines.push(Line::from("─".repeat(55)));

        // Show each topic with 3 dots (○ pending, ✓ correct, ✗ incorrect)
        for topic_prog in &self.topic_progress {
            let mut line_spans = Vec::new();

            // Column 1: Keywords (truncate to 40 chars)
            let keywords_str = topic_prog.keywords.join(",");
            let keywords_display = if keywords_str.len() > 40 {
                format!("{:.37}...", keywords_str)
            } else {
                format!("{:<40}", keywords_str)
            };
            line_spans.push(Span::raw(keywords_display));
            line_spans.push(Span::raw("  "));

            // Column 2: Status dots (○/✓/-/✗) based on score rating
            for (idx, status) in topic_prog.questions_status.iter().enumerate() {
                let (symbol, color) = if *status == QuestionStatus::Pending {
                    ("○", Color::DarkGray)
                } else if let Some(score) = topic_prog.scores[idx] {
                    // Show symbol based on actual rating
                    if score >= 90.0 {
                        ("✓", Color::Green)  // Easy
                    } else if score >= 60.0 {
                        ("-", Color::Yellow)  // Good/Hard
                    } else {
                        ("✗", Color::Red)  // Again
                    }
                } else {
                    ("○", Color::DarkGray)  // No score yet
                };
                line_spans.push(Span::styled(format!("{} ", symbol), Style::default().fg(color)));
            }

            line_spans.push(Span::raw("  "));

            // Column 3: Average score
            let scores: Vec<f64> = topic_prog.scores.iter().filter_map(|&s| s).collect();
            if !scores.is_empty() {
                let avg = scores.iter().sum::<f64>() / scores.len() as f64;
                line_spans.push(Span::styled(
                    format!("{:>3.0}%", avg),
                    Style::default().fg(Color::Cyan),
                ));
            } else {
                line_spans.push(Span::raw("  -- "));
            }

            report_lines.push(Line::from(line_spans));
        }

        // Add cumulative session average as a table row
        let all_scores: Vec<f64> = self.topic_progress.iter()
            .flat_map(|tp| tp.scores.iter().filter_map(|&s| s))
            .collect();

        if !all_scores.is_empty() {
            let session_avg = all_scores.iter().sum::<f64>() / all_scores.len() as f64;
            report_lines.push(Line::from("─".repeat(55)));

            let mut summary_spans = Vec::new();
            summary_spans.push(Span::styled(
                format!("{:<40}", "Session Total"),
                Style::default().add_modifier(Modifier::BOLD)
            ));
            summary_spans.push(Span::raw("  "));

            // Show session-level status summary (count of each rating)
            // Build status summary as fixed-width string to align with status dots above (6 chars)
            let easy_count = all_scores.iter().filter(|&&s| s >= 90.0).count();
            let good_hard_count = all_scores.iter().filter(|&&s| s >= 60.0 && s < 90.0).count();
            let again_count = all_scores.iter().filter(|&&s| s < 60.0).count();

            let mut status_parts = Vec::new();
            if easy_count > 0 {
                status_parts.push((format!("{}✓", easy_count), Color::Green));
            }
            if good_hard_count > 0 {
                status_parts.push((format!("{}-", good_hard_count), Color::Yellow));
            }
            if again_count > 0 {
                status_parts.push((format!("{}✗", again_count), Color::Red));
            }

            // Create status string with proper spacing (6 chars total to match "○ ○ ○ ")
            let mut status_str = String::new();
            for (i, (text, color)) in status_parts.iter().enumerate() {
                if i > 0 {
                    status_str.push(' ');
                }
                summary_spans.push(Span::styled(text.clone(), Style::default().fg(*color)));
                if i < status_parts.len() - 1 {
                    summary_spans.push(Span::raw(" "));
                }
            }

            // Pad to ensure alignment (6 chars for status column)
            // Each status dot takes 2 chars ("○ "), so 3 dots = 6 chars
            // Calculate current width and pad accordingly
            let current_width: usize = status_parts.iter().map(|(s, _)| s.len()).sum::<usize>()
                + status_parts.len().saturating_sub(1); // spaces between parts
            let padding_needed = if current_width < 6 { 6 - current_width } else { 0 };
            summary_spans.push(Span::raw(" ".repeat(padding_needed)));

            summary_spans.push(Span::raw("  "));
            summary_spans.push(Span::styled(
                format!("{:>3.0}%", session_avg),
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
            ));

            report_lines.push(Line::from(summary_spans));
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
            ReviewState::SelectingNextTopic | ReviewState::GeneratingQuestion | ReviewState::EvaluatingAnswer => "Loading...",
            ReviewState::ShowingQuestion => "Space/Enter: Answer | S: Skip (0%) | Ctrl+C: Quit (progress saved)",
            ReviewState::InputtingAnswer => "Enter twice to submit | Ctrl+C: Quit (progress saved)",
            ReviewState::ShowingFeedback => "Press Space to continue | Ctrl+C: Quit (progress saved)",
            ReviewState::ShowingResults => "Press Space to exit",
            ReviewState::Complete => "Press any key to exit",
        };

        let paragraph = Paragraph::new(instruction)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);

        f.render_widget(paragraph, area);
    }
}

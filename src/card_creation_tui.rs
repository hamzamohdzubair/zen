//! TUI interface for creating flashcards with optional LLM evaluation

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use crate::config::{Config, LLMConfig};
use crate::llm_evaluator::{create_evaluator, AnswerEvaluation};

#[derive(Debug, Clone, PartialEq)]
enum AppMode {
    Edit,   // Editing boxes
    Normal, // Showing commands, waiting for action
    ConfigSetup, // Entering API key
}

#[derive(Debug, Clone, PartialEq)]
enum FocusedBox {
    Question,
    UserAnswer,
    LLMAnswer,
}

#[derive(Debug, Clone, PartialEq)]
enum AppAction {
    Continue,
    SaveUserAnswer,
    SaveLLMAnswer,
    StartLLMEval,
    EnterEditMode,
    Exit,
    SaveWithoutChoice, // When only one answer exists
}

pub struct CardCreationApp {
    mode: AppMode,
    focused_box: FocusedBox,

    // Input buffers
    question: Vec<String>,
    user_answer: Vec<String>,
    llm_answer: Option<Vec<String>>,

    current_line: String,

    // State
    llm_evaluating: bool,
    llm_error: Option<String>,
    config: Option<LLMConfig>,
    evaluation_result: Option<AnswerEvaluation>,

    // For config setup
    api_key_buffer: String,
}

impl CardCreationApp {
    pub fn new() -> Result<Self> {
        // Load config if exists
        let config = Config::load()?.llm;

        Ok(Self {
            mode: AppMode::Edit,
            focused_box: FocusedBox::Question,
            question: vec![],
            user_answer: vec![],
            llm_answer: None,
            current_line: String::new(),
            llm_evaluating: false,
            llm_error: None,
            config,
            evaluation_result: None,
            api_key_buffer: String::new(),
        })
    }

    /// Run TUI and return (question, answer) if saved
    pub fn run(&mut self) -> Result<Option<(String, String)>> {
        // Setup terminal
        enable_raw_mode().context("Failed to enable raw mode")?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)
            .context("Failed to enter alternate screen")?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

        terminal.clear().context("Failed to clear terminal")?;

        let result = self.run_event_loop(&mut terminal);

        // Cleanup
        execute!(terminal.backend_mut(), LeaveAlternateScreen)
            .context("Failed to leave alternate screen")?;
        disable_raw_mode().context("Failed to disable raw mode")?;

        result
    }

    fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<Option<(String, String)>> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    let action = self.handle_key_event(key);

                    match action {
                        AppAction::Exit => return Ok(None),
                        AppAction::SaveUserAnswer => {
                            let question = self.question.join("\n");
                            let answer = self.user_answer.join("\n");
                            if !question.trim().is_empty() && !answer.trim().is_empty() {
                                return Ok(Some((question, answer)));
                            }
                        }
                        AppAction::SaveLLMAnswer => {
                            if let Some(ref llm_ans) = self.llm_answer {
                                let question = self.question.join("\n");
                                let answer = llm_ans.join("\n");
                                if !question.trim().is_empty() && !answer.trim().is_empty() {
                                    return Ok(Some((question, answer)));
                                }
                            }
                        }
                        AppAction::SaveWithoutChoice => {
                            // Save whichever answer exists
                            let question = self.question.join("\n");
                            if !question.trim().is_empty() {
                                if let Some(ref llm_ans) = self.llm_answer {
                                    let answer = llm_ans.join("\n");
                                    if !answer.trim().is_empty() {
                                        return Ok(Some((question, answer)));
                                    }
                                } else if !self.user_answer.is_empty() {
                                    let answer = self.user_answer.join("\n");
                                    if !answer.trim().is_empty() {
                                        return Ok(Some((question, answer)));
                                    }
                                }
                            }
                        }
                        AppAction::StartLLMEval => {
                            if let Err(e) = self.start_llm_evaluation() {
                                self.llm_error = Some(format!("Error: {}", e));
                            }
                        }
                        AppAction::EnterEditMode => {
                            self.mode = AppMode::Edit;
                        }
                        AppAction::Continue => {}
                    }
                }
            }
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        // Ctrl+C always exits
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return AppAction::Exit;
        }

        match self.mode {
            AppMode::Edit => self.handle_edit_mode(key),
            AppMode::Normal => self.handle_normal_mode(key),
            AppMode::ConfigSetup => self.handle_config_setup_mode(key),
        }
    }

    fn handle_edit_mode(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Char(c) => {
                self.current_line.push(c);
                AppAction::Continue
            }
            KeyCode::Backspace => {
                self.current_line.pop();
                AppAction::Continue
            }
            KeyCode::Tab => {
                // Save current line only if it has content
                if !self.current_line.is_empty() {
                    self.save_current_line();
                }

                // Cycle through boxes (only Tab cycles)
                self.focused_box = match self.focused_box {
                    FocusedBox::Question => FocusedBox::UserAnswer,
                    FocusedBox::UserAnswer => {
                        if self.llm_answer.is_some() {
                            FocusedBox::LLMAnswer
                        } else {
                            FocusedBox::Question
                        }
                    }
                    FocusedBox::LLMAnswer => FocusedBox::Question,
                };

                AppAction::Continue
            }
            KeyCode::Enter => {
                // Enter ONLY adds a new line, never changes focus
                self.save_current_line();
                AppAction::Continue
            }
            KeyCode::Esc => {
                // Save current line and exit edit mode
                self.save_current_line();
                self.mode = AppMode::Normal;
                AppAction::Continue
            }
            _ => AppAction::Continue,
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Char('e') => AppAction::EnterEditMode,
            KeyCode::Char('l') => {
                // Check if we have config
                if self.config.is_none() {
                    // Enter config setup mode
                    self.mode = AppMode::ConfigSetup;
                    AppAction::Continue
                } else {
                    AppAction::StartLLMEval
                }
            }
            KeyCode::Char('L') => {
                // Save LLM answer if it exists
                if self.llm_answer.is_some() {
                    AppAction::SaveLLMAnswer
                } else {
                    AppAction::Continue
                }
            }
            KeyCode::Char('S') => {
                // Save user answer if it exists
                if !self.user_answer.is_empty() {
                    AppAction::SaveUserAnswer
                } else {
                    AppAction::Continue
                }
            }
            KeyCode::Char('s') => {
                // Save whichever answer exists
                AppAction::SaveWithoutChoice
            }
            KeyCode::Esc => AppAction::Exit,
            _ => AppAction::Continue,
        }
    }

    fn handle_config_setup_mode(&mut self, key: KeyEvent) -> AppAction {
        match key.code {
            KeyCode::Char(c) => {
                self.api_key_buffer.push(c);
                AppAction::Continue
            }
            KeyCode::Backspace => {
                self.api_key_buffer.pop();
                AppAction::Continue
            }
            KeyCode::Enter => {
                // Save config and try evaluation
                let api_key = self.api_key_buffer.clone();
                if !api_key.trim().is_empty() {
                    let config = Config {
                        llm: Some(LLMConfig {
                            provider: "groq".to_string(),
                            api_key: api_key.trim().to_string(),
                            model: "llama-3.3-70b-versatile".to_string(),
                        }),
                    };

                    // Save config
                    if let Err(e) = config.save() {
                        self.llm_error = Some(format!("Failed to save config: {}", e));
                        self.mode = AppMode::Normal;
                        return AppAction::Continue;
                    }

                    // Update our config
                    self.config = config.llm;
                    self.api_key_buffer.clear();
                    self.mode = AppMode::Normal;

                    // Start evaluation
                    return AppAction::StartLLMEval;
                }

                self.mode = AppMode::Normal;
                AppAction::Continue
            }
            KeyCode::Esc => {
                self.api_key_buffer.clear();
                self.mode = AppMode::Normal;
                AppAction::Continue
            }
            _ => AppAction::Continue,
        }
    }

    fn save_current_line(&mut self) {
        if !self.current_line.is_empty() || matches!(self.focused_box, FocusedBox::Question | FocusedBox::UserAnswer) {
            match self.focused_box {
                FocusedBox::Question => {
                    self.question.push(self.current_line.clone());
                }
                FocusedBox::UserAnswer => {
                    self.user_answer.push(self.current_line.clone());
                }
                FocusedBox::LLMAnswer => {
                    if let Some(ref mut llm_ans) = self.llm_answer {
                        llm_ans.push(self.current_line.clone());
                    }
                }
            }
        }
        self.current_line.clear();
    }

    fn start_llm_evaluation(&mut self) -> Result<()> {
        // Check if we have question and user answer
        let question = self.question.join("\n");
        let user_answer = self.user_answer.join("\n");

        if question.trim().is_empty() || user_answer.trim().is_empty() {
            anyhow::bail!("Need both question and answer to evaluate");
        }

        // Check config
        let config = self.config.as_ref()
            .context("No LLM configuration")?;

        // Set evaluating state
        self.llm_evaluating = true;
        self.llm_error = None;

        // Call LLM
        let evaluator = create_evaluator(config)?;
        let evaluation = evaluator.evaluate_answer(&question, &user_answer)?;

        // Split ideal answer into lines
        self.llm_answer = Some(evaluation.ideal_answer.lines().map(|s| s.to_string()).collect());
        self.evaluation_result = Some(evaluation);
        self.llm_evaluating = false;

        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Split screen: main content on left, stats on right
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70),  // Main content
                Constraint::Percentage(30),  // Stats/Info panel
            ])
            .split(size);

        // Left side: Question and answers
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),  // Question box (smaller)
                Constraint::Min(8),     // User answer box
                Constraint::Min(8),     // LLM answer box
                Constraint::Length(3),  // Command box (smaller)
            ])
            .split(main_chunks[0]);

        // Right side: Model info and score
        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),  // Model info (same as question box)
                Constraint::Min(6),     // Score and stats
            ])
            .split(main_chunks[1]);

        // Render all components
        self.render_question_box(frame, left_chunks[0]);
        self.render_user_answer_box(frame, left_chunks[1]);
        self.render_llm_answer_box(frame, left_chunks[2]);
        self.render_command_box(frame, left_chunks[3]);

        self.render_model_info(frame, right_chunks[0]);
        self.render_score_info(frame, right_chunks[1]);
    }

    fn render_question_box(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let border_style = if self.mode == AppMode::Edit && self.focused_box == FocusedBox::Question {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let mut lines = vec![];

        for line in &self.question {
            lines.push(Line::from(line.clone()));
        }

        // Show current line if focused
        if self.mode == AppMode::Edit && self.focused_box == FocusedBox::Question {
            lines.push(Line::from(format!("{}█", self.current_line)));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Question | Create New Card ")
                    .borders(Borders::ALL)
                    .border_style(border_style)
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_user_answer_box(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let border_style = if self.mode == AppMode::Edit && self.focused_box == FocusedBox::UserAnswer {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::White)
        };

        let mut lines = vec![];

        for line in &self.user_answer {
            lines.push(Line::from(line.clone()));
        }

        // Show current line if focused
        if self.mode == AppMode::Edit && self.focused_box == FocusedBox::UserAnswer {
            lines.push(Line::from(format!("{}█", self.current_line)));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Your Answer ")
                    .borders(Borders::ALL)
                    .border_style(border_style)
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_llm_answer_box(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let border_style = if self.mode == AppMode::Edit && self.focused_box == FocusedBox::LLMAnswer {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default().fg(Color::White)
        };

        let mut lines = vec![];

        if self.llm_evaluating {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "⏳ Evaluating...",
                Style::default().fg(Color::Yellow),
            )]));
        } else if let Some(ref llm_ans) = self.llm_answer {
            for line in llm_ans {
                lines.push(Line::from(line.clone()));
            }

            // Show current line if focused
            if self.mode == AppMode::Edit && self.focused_box == FocusedBox::LLMAnswer {
                lines.push(Line::from(format!("{}█", self.current_line)));
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" LLM Answer (100% Score) ")
                    .borders(Borders::ALL)
                    .border_style(border_style)
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_model_info(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mut lines = vec![];

        if let Some(ref config) = self.config {
            lines.push(Line::from(vec![
                Span::styled("Provider: ", Style::default().fg(Color::Gray)),
                Span::styled(&config.provider, Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Model: ", Style::default().fg(Color::Gray)),
                Span::styled(&config.model, Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Location: ", Style::default().fg(Color::Gray)),
                Span::styled("Cloud", Style::default().fg(Color::Yellow)),
            ]));
        } else {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "No LLM configured",
                Style::default().fg(Color::Red),
            )]));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Model Info ")
                    .borders(Borders::ALL)
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_score_info(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let mut lines = vec![];

        if let Some(ref eval) = self.evaluation_result {
            // Score - large and prominent
            if let Some(score) = eval.score {
                let score_color = if score >= 80.0 {
                    Color::Green
                } else if score >= 60.0 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                lines.push(Line::from(vec![
                    Span::styled("Your Score: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{:.0}/100", score), Style::default().fg(score_color).add_modifier(Modifier::BOLD)),
                ]));
            }

            // Stats
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Stats:",
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD),
            )]));
            if let Some(tokens) = eval.tokens_used {
                lines.push(Line::from(vec![
                    Span::styled("  Tokens: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{}", tokens), Style::default().fg(Color::White)),
                ]));
            }
            if let Some(time) = eval.response_time_ms {
                lines.push(Line::from(vec![
                    Span::styled("  Time: ", Style::default().fg(Color::Gray)),
                    Span::styled(format!("{}ms", time), Style::default().fg(Color::White)),
                ]));
            }
        } else if self.llm_evaluating {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Evaluating...",
                Style::default().fg(Color::Yellow),
            )]));
        } else {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "No evaluation yet",
                Style::default().fg(Color::Gray),
            )]));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Evaluation ")
                    .borders(Borders::ALL)
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_command_box(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let text = match self.mode {
            AppMode::Edit => {
                "[Tab] Cycle  [Enter] New line  [ESC] Exit edit".to_string()
            }
            AppMode::Normal => {
                if let Some(ref error) = self.llm_error {
                    error.clone()
                } else {
                    let has_user_answer = !self.user_answer.is_empty();
                    let has_llm_answer = self.llm_answer.is_some();

                    let mut commands = vec!["[e] Edit".to_string()];

                    if has_user_answer {
                        commands.push("[l] LLM Eval".to_string());
                    }

                    if has_llm_answer {
                        commands.push("[L] Save LLM".to_string());
                    }

                    if has_user_answer {
                        commands.push("[S] Save User".to_string());
                    }

                    if has_user_answer || has_llm_answer {
                        commands.push("[s] Save".to_string());
                    }

                    commands.push("[ESC] Cancel".to_string());

                    commands.join("  ")
                }
            }
            AppMode::ConfigSetup => {
                format!("Groq API key: {}█  [Enter] Save  [ESC] Cancel", self.api_key_buffer)
            }
        };

        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}

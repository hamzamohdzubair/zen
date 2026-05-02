mod app;
mod input;
mod storage;
mod types;
mod ui;

use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::{App, ViewMode};
use input::{AppAction, handle_key};
use ui::done::DoneApp;
use ui::stats::StatsApp;
use ui::tui;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Launch the kanban board
    Kanban,
    /// Launch the tree TUI
    Tui,
    /// Browse all completed tasks
    Done,
    /// Show project statistics
    Stats,
    /// Export tasks as CSV to stdout
    Export {
        #[command(subcommand)]
        filter: Option<ExportFilter>,
    },
}

#[derive(Subcommand)]
enum ExportFilter {
    /// Export only done tasks as CSV
    Done {
        /// Output format (csv is the only supported format)
        format: Option<String>,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Kanban => run_kanban(),
        Command::Tui => run_tui(),
        Command::Done => run_done(),
        Command::Stats => run_stats(),
        Command::Export { filter: None } => run_export_all(),
        Command::Export { filter: Some(ExportFilter::Done { .. }) } => run_export_done(),
    }
}

fn run_kanban() -> io::Result<()> {
    run_main_tui(ViewMode::Board)
}

fn run_tui() -> io::Result<()> {
    run_main_tui(ViewMode::Tree)
}

fn run_main_tui(initial_view: ViewMode) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tasks, projects) = storage::load();
    let mut app = App::new(tasks, projects);
    app.view_mode = initial_view;

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.status_message = None;

                // In tree view, j/k drive tree navigation and scroll
                if app.view_mode == ViewMode::Tree {
                    let task_area_height =
                        (terminal.size()?.height as usize).saturating_sub(3);
                    match key.code {
                        KeyCode::Char('j') | KeyCode::Down => {
                            tui::navigate_tree(&mut app, 1);
                            app.tui_scroll_offset =
                                tui::compute_scroll(&app, app.tui_scroll_offset, task_area_height);
                            continue;
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            tui::navigate_tree(&mut app, -1);
                            app.tui_scroll_offset =
                                tui::compute_scroll(&app, app.tui_scroll_offset, task_area_height);
                            continue;
                        }
                        _ => {}
                    }
                }

                match handle_key(&mut app, key) {
                    AppAction::Quit => break,
                    AppAction::Save => storage::save(&app.tasks, &app.projects),
                    AppAction::None => {}
                }

                if app.view_mode == ViewMode::Tree {
                    let task_area_height =
                        (terminal.size()?.height as usize).saturating_sub(3);
                    app.tui_scroll_offset =
                        tui::compute_scroll(&app, app.tui_scroll_offset, task_area_height);
                }
            }
        }
    }

    cleanup_terminal(terminal)
}

fn run_done() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tasks, projects) = storage::load();
    let mut app = DoneApp::new(tasks, projects);

    loop {
        terminal.draw(|f| ui::done::draw_done(f, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                    KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                    KeyCode::Char('s') => app.cycle_sort(),
                    KeyCode::Char('`') => app.toggle_unc(),
                    KeyCode::Char(c @ '1'..='9') => app.toggle_slot((c as u8 - b'1') as usize),
                    KeyCode::Char('0') => app.toggle_slot(9),
                    _ => {}
                }
            }
        }
    }

    cleanup_terminal(terminal)
}

fn run_stats() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tasks, projects) = storage::load();
    let mut app = StatsApp::new(tasks, projects);

    loop {
        terminal.draw(|f| ui::stats::draw_stats(f, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => app.zoom_out(),
                    KeyCode::Enter => app.zoom_in(),
                    KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                    KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                    _ => {}
                }
            }
        }
    }

    cleanup_terminal(terminal)
}

fn cleanup_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_export_all() -> io::Result<()> {
    let (tasks, _) = storage::load();
    println!("id,title,project,status,created_at,time_in_todo_s,time_in_doing_s,time_in_done_s");
    for task in &tasks {
        use types::Status;
        println!(
            "{},{:?},{},{:?},{},{},{},{}",
            task.id,
            task.title,
            task.project,
            task.status,
            task.created_at.format("%Y-%m-%dT%H:%M:%SZ"),
            task.time_in(&Status::Todo),
            task.time_in(&Status::Doing),
            task.time_in(&Status::Done),
        );
    }
    Ok(())
}

fn run_export_done() -> io::Result<()> {
    use types::Status;
    use ui::done::{completed_at, elapsed_to_done};

    let (tasks, _) = storage::load();
    println!("id,title,project,parent_id,created_at,completed_at,elapsed_s,time_in_todo_s,time_in_doing_s");
    for task in tasks.iter().filter(|t| t.status == Status::Done) {
        let completed = completed_at(task)
            .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default();
        let elapsed = elapsed_to_done(task).unwrap_or(0);
        let parent = task.parent_id.map(|id| id.to_string()).unwrap_or_default();
        println!(
            "{},{:?},{},{},{},{},{},{},{}",
            task.id,
            task.title,
            task.project,
            parent,
            task.created_at.format("%Y-%m-%dT%H:%M:%SZ"),
            completed,
            elapsed,
            task.time_in(&Status::Todo),
            task.time_in(&Status::Doing),
        );
    }
    Ok(())
}

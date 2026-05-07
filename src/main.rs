mod app;
mod input;
mod snapshots;
mod storage;
mod types;
mod ui;

use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::{App, Mode};
use input::{AppAction, handle_key};
use ui::snaps::{SnapsApp, compute_browser_scroll};
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
    /// Launch the tree TUI
    Tui,
    /// Show project statistics
    Stats,
    /// Browse saved tree snapshots
    Snaps,
    /// Export tasks as CSV to stdout
    Export,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Tui => run_tui(),
        Command::Stats => run_stats(),
        Command::Snaps => run_snaps(),
        Command::Export => run_export_all(),
    }
}

fn run_tui() -> io::Result<()> {
    run_main_tui()
}

fn run_main_tui() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, cursor::Hide, cursor::SetCursorStyle::SteadyBlock)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tasks = storage::load();
    let mut app = App::new(tasks);
    app.fold_all();

    loop {
        app.check_snooze_timers();
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.status_message = None;

                if matches!(app.mode, Mode::Normal | Mode::Visual) {
                    let task_area_height =
                        (terminal.size()?.height as usize).saturating_sub(1);
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
                    AppAction::Save => storage::save(&app.tasks),
                    AppAction::Snapshot => {
                        let snap = app.to_snapshot();
                        app.status_message = Some(match snapshots::save_snapshot(&snap) {
                            Some(_) => "Snapshot saved".into(),
                            None => "Snapshot failed".into(),
                        });
                    }
                    AppAction::None => {}
                }

                let task_area_height = (terminal.size()?.height as usize).saturating_sub(1);
                app.tui_scroll_offset =
                    tui::compute_scroll(&app, app.tui_scroll_offset, task_area_height);
            }
        }
    }

    cleanup_terminal(terminal)
}

fn run_stats() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tasks = storage::load();
    let mut app = StatsApp::new(tasks);

    loop {
        terminal.draw(|f| ui::stats::draw_stats(f, &mut app))?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    _ => {}
                }
            }
        }
    }

    cleanup_terminal(terminal)
}

fn run_snaps() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = SnapsApp::new();

    loop {
        terminal.draw(|f| ui::snaps::draw_snaps(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.status_message = None;
                let h = terminal.size()?.height as usize;
                let area_height = h.saturating_sub(2);

                if app.viewer.is_some() {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.close_viewer(),
                        KeyCode::Char('j') | KeyCode::Down => app.viewer_scroll_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.viewer_scroll_up(),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('j') | KeyCode::Down => {
                            app.move_down();
                            app.scroll_offset =
                                compute_browser_scroll(app.cursor, app.scroll_offset, area_height);
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            app.move_up();
                            app.scroll_offset =
                                compute_browser_scroll(app.cursor, app.scroll_offset, area_height);
                        }
                        KeyCode::Char('l') | KeyCode::Enter | KeyCode::Right => {
                            app.toggle_or_open();
                            app.scroll_offset =
                                compute_browser_scroll(app.cursor, app.scroll_offset, area_height);
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            app.collapse_current();
                            app.scroll_offset =
                                compute_browser_scroll(app.cursor, app.scroll_offset, area_height);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    cleanup_terminal(terminal)
}

fn cleanup_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture, cursor::SetCursorStyle::DefaultUserShape, cursor::Show)?;
    Ok(())
}

fn run_export_all() -> io::Result<()> {
    let tasks = storage::load();
    println!("id,title,status,created_at,time_in_todo_s,time_in_doing_s,time_in_done_s");
    for task in &tasks {
        use types::Status;
        println!(
            "{},{:?},{:?},{},{},{},{}",
            task.id,
            task.title,
            task.status,
            task.created_at.format("%Y-%m-%dT%H:%M:%SZ"),
            task.time_in(&Status::Todo),
            task.time_in(&Status::Doing),
            task.time_in(&Status::Done),
        );
    }
    Ok(())
}


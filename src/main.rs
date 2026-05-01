mod app;
mod input;
mod storage;
mod types;
mod ui;

use std::io;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;
use input::{AppAction, handle_key};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Launch the TUI
    Tui,
    /// Export tasks as CSV to stdout
    Export,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Tui => run_tui(),
        Command::Export => run_export(),
    }
}

fn run_tui() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tasks = storage::load();
    let mut app = App::new(tasks);

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Clear status message on any key
                app.status_message = None;
                match handle_key(&mut app, key) {
                    AppAction::Quit => break,
                    AppAction::Save => storage::save(&app.tasks),
                    AppAction::None => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_export() -> io::Result<()> {
    let tasks = storage::load();
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

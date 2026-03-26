use clap::{Parser, Subcommand};
use anyhow::Result;

mod api_client;
mod auth;
mod commands;

#[derive(Parser)]
#[command(name = "zen")]
#[command(about = "AI-powered spaced repetition learning", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Login to your forgetmeifyoucan account
    Login {
        /// Your email address
        #[arg(short, long)]
        email: Option<String>,
    },

    /// Logout from your account
    Logout,

    /// Start a review session
    Start,

    /// View your statistics
    Stats,

    /// List all topics
    List {
        /// Show only due topics
        #[arg(short, long)]
        due: bool,
    },

    /// Add a new topic
    Add {
        /// Comma-separated keywords (e.g., "Rust, Programming, Web")
        keywords: Vec<String>,
    },

    /// Delete a topic
    Delete {
        /// Topic ID to delete
        topic_id: String,
    },

    /// Show account information
    Me,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Login { email } => commands::login(email),
        Commands::Logout => commands::logout(),
        Commands::Start => commands::start_review(),
        Commands::Stats => commands::show_stats(),
        Commands::List { due } => commands::list_topics(due),
        Commands::Add { keywords } => commands::add_topic(keywords),
        Commands::Delete { topic_id } => commands::delete_topic(&topic_id),
        Commands::Me => commands::show_me(),
    }
}

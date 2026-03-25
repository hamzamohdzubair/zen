use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zen")]
#[command(about = "A topic-based spaced repetition CLI with LLM-powered reviews", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new topic with comma-separated keywords
    #[command(name = "add")]
    Add {
        /// Comma-separated keywords (spaces allowed within keywords)
        /// Example: "AI, machine learning" or "LSTM"
        keywords: String,
    },

    /// Start a topic review session
    #[command(name = "start")]
    Start,

    /// Show detailed statistics (TUI)
    #[command(name = "stats")]
    Stats,

    /// List all topics
    #[command(name = "topics")]
    Topics {
        /// Show only due topics
        #[arg(long)]
        due: bool,
    },

    /// Delete a topic
    #[command(name = "del")]
    Delete {
        /// Topic ID to delete
        topic_id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { keywords } => {
            zen::commands::add_topic(&keywords)?;
        }
        Commands::Start => {
            zen::commands::start_topic_review()?;
        }
        Commands::Stats => {
            zen::commands::show_stats_tui()?;
        }
        Commands::Topics { due } => {
            zen::commands::list_topics(due)?;
        }
        Commands::Delete { topic_id } => {
            zen::commands::delete_topic(&topic_id)?;
        }
    }

    Ok(())
}

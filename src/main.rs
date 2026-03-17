use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zen")]
#[command(about = "A spaced repetition CLI for active recall", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new flashcard
    #[command(name = "new")]
    New {
        /// The question for the flashcard (no quotes needed)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        question: Vec<String>,
    },

    /// Find and edit cards (interactive fuzzy search)
    #[command(name = "find", alias = "f")]
    Find {
        /// Optional initial search query (can also type in the interactive search)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        query: Option<Vec<String>>,
    },

    /// Start a review session
    #[command(name = "start")]
    Start,

    /// Show statistics
    #[command(name = "stats")]
    Stats,

    /// List all cards
    #[command(name = "list")]
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { question } => {
            let question_text = question.join(" ");
            zen::commands::new_card(&question_text)?;
        }
        Commands::Find { query } => {
            let query_text = query.map(|q| q.join(" ")).unwrap_or_default();
            zen::commands::find_cards(&query_text)?;
        }
        Commands::Start => {
            zen::commands::start_review()?;
        }
        Commands::Stats => {
            zen::commands::show_stats()?;
        }
        Commands::List => {
            println!("List command not yet implemented.");
        }
    }

    Ok(())
}

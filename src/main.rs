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

    /// Find and edit cards (fuzzy search)
    #[command(name = "find", alias = "f")]
    Find {
        /// Search query
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        query: Vec<String>,
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
            let query_text = query.join(" ");
            println!("Find command not yet implemented. Query: {}", query_text);
        }
        Commands::Start => {
            println!("Start command not yet implemented.");
        }
        Commands::Stats => {
            println!("Stats command not yet implemented.");
        }
        Commands::List => {
            println!("List command not yet implemented.");
        }
    }

    Ok(())
}

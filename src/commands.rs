//! Command implementations for the CLI

use anyhow::{Context, Result};
use std::io::{self, Write};

use crate::card::{generate_unique_card_id, Card};
use crate::database;
use crate::storage;

/// Create a new flashcard
pub fn new_card(question: &str) -> Result<()> {
    if question.trim().is_empty() {
        anyhow::bail!("Question cannot be empty");
    }

    // Prompt for answer
    println!("\nQuestion: {}\n", question);
    println!("Enter answer (press Enter twice to finish):");
    print!("> ");
    io::stdout().flush()?;

    let answer = read_multiline_input()?;

    if answer.trim().is_empty() {
        anyhow::bail!("Answer cannot be empty");
    }

    // Create the card
    let card = Card::new(question.to_string(), answer);

    // Generate unique ID
    let card_id = generate_unique_card_id()?;

    // Save to filesystem
    storage::write_card(&card_id, &card.question, &card.answer)
        .context("Failed to write card to file")?;

    // Save to database
    let conn = database::init_database()?;
    database::insert_card(&conn, &card_id, &card.created_at, &card.modified_at)
        .context("Failed to save card to database")?;

    println!("\n✓ Card created successfully!");
    println!("  ID: {}", card_id);
    println!("  File: ~/.zen/cards/{}.md", card_id);

    Ok(())
}

/// Read multi-line input from stdin
/// Stops when user presses Enter twice (empty line after content)
fn read_multiline_input() -> Result<String> {
    let mut lines = Vec::new();
    let stdin = io::stdin();

    loop {
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                // EOF (Ctrl+D)
                break;
            }
            Ok(_) => {
                // Check if it's an empty line
                if line.trim().is_empty() && !lines.is_empty() {
                    // Empty line after content - finish input
                    break;
                }

                let ends_with_newline = line.ends_with('\n');
                lines.push(line);

                // Show prompt for next line
                if !ends_with_newline {
                    break;
                }
                print!("> ");
                io::stdout().flush()?;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    // Join lines and trim trailing newline
    let result = lines.join("");
    Ok(result.trim_end().to_string())
}

/// Find and edit flashcards using fuzzy search
pub fn find_cards(query: &str) -> Result<()> {
    // Create TUI app with initial query
    let mut app = crate::tui::FinderApp::new(query).context("Failed to create finder app")?;

    // Run TUI loop
    match app.run()? {
        Some(card_id) => {
            // User pressed Enter - edit the card
            println!("\nOpening card {} in editor...", card_id);

            if crate::editor::edit_card_in_editor(&card_id)? {
                println!("✓ Card updated and schedule reset!");
            } else {
                println!("No changes made.");
            }
        }
        None => {
            // User pressed ESC - just exit
            println!("Search cancelled.");
        }
    }

    Ok(())
}

/// Start a review session
pub fn start_review() -> Result<()> {
    match crate::review_tui::ReviewApp::new() {
        Ok(mut app) => app.run(),
        Err(e) if e.to_string().contains("No cards due") => {
            println!("\nNo cards are due for review right now!");
            println!("Come back later or create new cards with 'zen new'.");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Show statistics and card information
pub fn show_stats() -> Result<()> {
    let mut app = crate::stats_tui::StatsApp::new()
        .context("Failed to load statistics")?;
    app.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_question() {
        let result = new_card("");
        assert!(result.is_err());
    }

    #[test]
    fn test_whitespace_question() {
        let result = new_card("   ");
        assert!(result.is_err());
    }
}

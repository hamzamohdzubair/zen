//! Command implementations for the CLI

use anyhow::{Context, Result};

use crate::card::{generate_unique_card_id, Card};
use crate::database;
use crate::storage;

/// Create a new flashcard using TUI interface
pub fn new_card() -> Result<()> {
    // Launch the card creation TUI
    let mut app = crate::card_creation_tui::CardCreationApp::new()?;

    match app.run()? {
        Some((question, answer)) => {
            // User saved a card, create it
            let card = Card::new(question, answer);

            // Generate unique ID
            let card_id = generate_unique_card_id()?;

            // Save to filesystem
            storage::write_card(&card_id, &card.question, &card.answer)
                .context("Failed to write card to file")?;

            // Save to database
            let conn = database::init_database()?;
            database::insert_card(&conn, &card_id, &card.created_at, &card.modified_at)
                .context("Failed to save card to database")?;

            println!("\n✓ Card created: {}", card_id);
            Ok(())
        }
        None => {
            // User cancelled
            println!("Card creation cancelled.");
            Ok(())
        }
    }
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
    // Tests for new_card() are integration tests since it requires TUI interaction
}

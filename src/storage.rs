//! File system storage for card content

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const ZEN_DIR: &str = ".zen";
const CARDS_DIR: &str = "cards";
const SEPARATOR: &str = "\n\n---\n\n";

/// Get the base zen directory (~/.zen)
pub fn zen_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(ZEN_DIR))
}

/// Get the cards directory (~/.zen/cards)
pub fn cards_dir() -> Result<PathBuf> {
    Ok(zen_dir()?.join(CARDS_DIR))
}

/// Get the database path (~/.zen/zen.db)
pub fn db_path() -> Result<PathBuf> {
    Ok(zen_dir()?.join("zen.db"))
}

/// Ensure the zen directory structure exists
pub fn ensure_directories() -> Result<()> {
    let cards = cards_dir()?;
    if !cards.exists() {
        fs::create_dir_all(&cards)
            .with_context(|| format!("Failed to create directory: {}", cards.display()))?;
    }
    Ok(())
}

/// Get the file path for a card by ID
pub fn card_path(card_id: &str) -> Result<PathBuf> {
    Ok(cards_dir()?.join(format!("{}.md", card_id)))
}

/// Write a card to a markdown file
pub fn write_card(card_id: &str, question: &str, answer: &str) -> Result<()> {
    ensure_directories()?;

    let content = format!("{}{}{}", question, SEPARATOR, answer);
    let path = card_path(card_id)?;

    fs::write(&path, content)
        .with_context(|| format!("Failed to write card to {}", path.display()))?;

    Ok(())
}

/// Read a card from a markdown file
pub fn read_card(card_id: &str) -> Result<(String, String)> {
    let path = card_path(card_id)?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read card from {}", path.display()))?;

    parse_card_content(&content)
}

/// Parse card content into question and answer
fn parse_card_content(content: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = content.split(SEPARATOR).collect();

    if parts.len() != 2 {
        anyhow::bail!("Invalid card format: expected question and answer separated by '\\n\\n---\\n\\n'");
    }

    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Check if a card file exists
pub fn card_exists(card_id: &str) -> Result<bool> {
    Ok(card_path(card_id)?.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_card_content() {
        let content = "What is Rust?\n\n---\n\nA systems programming language";
        let (question, answer) = parse_card_content(content).unwrap();
        assert_eq!(question, "What is Rust?");
        assert_eq!(answer, "A systems programming language");
    }

    #[test]
    fn test_parse_multiline_card() {
        let content = "What are the main features?\n\n1. Safety\n2. Speed\n\n---\n\nRust provides:\n- Memory safety\n- Thread safety";
        let (question, answer) = parse_card_content(content).unwrap();
        assert_eq!(question, "What are the main features?\n\n1. Safety\n2. Speed");
        assert_eq!(answer, "Rust provides:\n- Memory safety\n- Thread safety");
    }
}

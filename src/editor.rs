//! Editor integration for card editing

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Edit a card in the user's preferred editor
/// Returns true if the card was modified, false otherwise
pub fn edit_card_in_editor(card_id: &str) -> Result<bool> {
    let path = crate::storage::card_path(card_id).context("Failed to get card path")?;

    // Hash the file content before editing
    let hash_before = hash_file_content(&path).context("Failed to hash file before editing")?;

    // Get editor and spawn it
    let editor = get_editor().context("Failed to determine editor")?;

    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("Failed to spawn editor: {}", editor))?;

    if !status.success() {
        bail!("Editor exited with non-zero status: {}", status);
    }

    // Hash the file content after editing
    let hash_after = hash_file_content(&path).context("Failed to hash file after editing")?;

    // If unchanged, return false
    if hash_before == hash_after {
        return Ok(false);
    }

    // Validate the format by attempting to read and parse the card
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read card file after editing: {}", path.display()))?;

    // Validate format (must have \n\n---\n\n separator)
    if !content.contains("\n\n---\n\n") {
        bail!(
            "Invalid card format after editing. Cards must have a question and answer \
             separated by '\\n\\n---\\n\\n'"
        );
    }

    // Validate by actually parsing
    crate::storage::read_card(card_id).context("Failed to parse card content after editing")?;

    // Update modified_at timestamp and reset schedule
    let conn = crate::database::init_database().context("Failed to initialize database")?;

    crate::database::update_modified_at(&conn, card_id)
        .context("Failed to update modified_at timestamp")?;

    crate::database::reset_card_schedule(&conn, card_id)
        .context("Failed to reset card schedule")?;

    Ok(true)
}

/// Compute SHA-256 hash of file content
fn hash_file_content(path: &Path) -> Result<[u8; 32]> {
    let content =
        fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mut hasher = Sha256::new();
    hasher.update(&content);
    let result = hasher.finalize();

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    Ok(hash)
}

/// Get the editor to use
fn get_editor() -> Result<String> {
    // Try $EDITOR environment variable first
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    // Try common editors
    for editor in &["vim", "nano", "vi"] {
        if Command::new("which")
            .arg(editor)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(editor.to_string());
        }
    }

    bail!(
        "No editor found. Please set the EDITOR environment variable \
         (e.g., export EDITOR=vim)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_hash_file_content() {
        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "test content").unwrap();

        let hash1 = hash_file_content(temp_file.path()).unwrap();
        let hash2 = hash_file_content(temp_file.path()).unwrap();

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_different_content() {
        let mut temp_file1 = NamedTempFile::new().unwrap();
        write!(temp_file1, "content 1").unwrap();

        let mut temp_file2 = NamedTempFile::new().unwrap();
        write!(temp_file2, "content 2").unwrap();

        let hash1 = hash_file_content(temp_file1.path()).unwrap();
        let hash2 = hash_file_content(temp_file2.path()).unwrap();

        // Different content should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_get_editor() {
        // This test just ensures get_editor() doesn't panic
        // The actual result depends on the system configuration
        let result = get_editor();
        // Should either succeed or fail with a helpful error message
        if let Err(e) = result {
            assert!(e.to_string().contains("EDITOR"));
        }
    }
}

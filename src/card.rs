//! Card data structures and ID generation

use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// A flashcard with question and answer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub question: String,
    pub answer: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

impl Card {
    /// Create a new card with generated ID
    pub fn new(question: String, answer: String) -> Self {
        let now = Utc::now();
        Self {
            id: generate_card_id(),
            question,
            answer,
            created_at: now,
            modified_at: now,
        }
    }
}

/// Generate a 6-digit alphanumeric case-sensitive ID
/// Characters: a-z, A-Z, 0-9 (62 possible characters)
/// Total combinations: 62^6 = ~56.8 billion
pub fn generate_card_id() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    const ID_LENGTH: usize = 6;

    let mut rng = rand::thread_rng();
    (0..ID_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate a unique card ID by checking against existing cards
pub fn generate_unique_card_id() -> Result<String> {
    loop {
        let id = generate_card_id();
        // Check if card exists
        if !crate::storage::card_exists(&id)? {
            return Ok(id);
        }
        // If collision (extremely rare), try again
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_card_id() {
        let id = generate_card_id();
        assert_eq!(id.len(), 6);

        // Check all characters are alphanumeric
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_card_creation() {
        let card = Card::new("Question?".to_string(), "Answer!".to_string());
        assert_eq!(card.id.len(), 6);
        assert_eq!(card.question, "Question?");
        assert_eq!(card.answer, "Answer!");
        assert_eq!(card.created_at, card.modified_at);
    }

    #[test]
    fn test_unique_ids() {
        // Generate multiple IDs and ensure they're unique
        let ids: std::collections::HashSet<_> = (0..100).map(|_| generate_card_id()).collect();
        // With 62^6 possibilities, 100 IDs should all be unique
        assert_eq!(ids.len(), 100);
    }
}

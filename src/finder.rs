//! Fuzzy search functionality for flashcards

use anyhow::{Context, Result};
use nucleo_matcher::{pattern::Pattern, Matcher, Utf32Str};

/// A card that can be searched
#[derive(Clone)]
pub struct SearchableCard {
    pub id: String,
    pub question: String,
    pub answer: String,
    search_text: String,
}

impl SearchableCard {
    pub fn new(id: String, question: String, answer: String) -> Self {
        // Concatenate question and answer for searching
        let search_text = format!("{}\n\n---\n\n{}", question, answer);
        Self {
            id,
            question,
            answer,
            search_text,
        }
    }
}

/// A search result with score
pub struct SearchResult {
    pub card: SearchableCard,
    pub score: u32,
}

/// Fuzzy finder for cards
pub struct Finder {
    cards: Vec<SearchableCard>,
    matcher: Matcher,
}

impl Finder {
    /// Create a new finder by loading all cards from the database
    pub fn new() -> Result<Self> {
        let conn = crate::database::init_database().context("Failed to initialize database")?;

        let card_ids =
            crate::database::get_all_card_ids(&conn).context("Failed to get card IDs")?;

        let mut cards = Vec::new();
        for id in card_ids {
            match crate::storage::read_card(&id) {
                Ok((question, answer)) => {
                    cards.push(SearchableCard::new(id, question, answer));
                }
                Err(e) => {
                    eprintln!("Warning: Failed to load card {}: {}", id, e);
                }
            }
        }

        Ok(Self {
            cards,
            matcher: Matcher::new(nucleo_matcher::Config::DEFAULT),
        })
    }

    /// Search for cards matching the query
    pub fn search(&mut self, query: &str) -> Vec<SearchResult> {
        // Empty query returns all cards
        if query.trim().is_empty() {
            return self
                .cards
                .iter()
                .map(|card| SearchResult {
                    card: card.clone(),
                    score: 0,
                })
                .collect();
        }

        let pattern = Pattern::parse(
            query,
            nucleo_matcher::pattern::CaseMatching::Ignore,
            nucleo_matcher::pattern::Normalization::Smart,
        );
        let mut results = Vec::new();
        let mut buf = Vec::new();

        for card in &self.cards {
            let haystack = Utf32Str::new(&card.search_text, &mut buf);
            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                results.push(SearchResult {
                    card: card.clone(),
                    score,
                });
            }
        }

        // Sort by score descending (higher is better)
        results.sort_by(|a, b| b.score.cmp(&a.score));

        results
    }

    /// Get a card by ID
    pub fn get_card(&self, id: &str) -> Option<&SearchableCard> {
        self.cards.iter().find(|card| card.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_searchable_card_new() {
        let card = SearchableCard::new(
            "test-id".to_string(),
            "What is Rust?".to_string(),
            "A systems programming language".to_string(),
        );

        assert_eq!(card.id, "test-id");
        assert_eq!(card.question, "What is Rust?");
        assert_eq!(card.answer, "A systems programming language");
        assert_eq!(
            card.search_text,
            "What is Rust?\n\n---\n\nA systems programming language"
        );
    }
}

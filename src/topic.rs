//! Topic data structures and ID generation

use anyhow::Result;
use chrono::{DateTime, Utc};
use rand::Rng;

/// A topic with associated keywords
#[derive(Debug, Clone)]
pub struct Topic {
    pub id: String,
    pub keywords: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

impl Topic {
    pub fn new(keywords: Vec<String>) -> Self {
        let now = Utc::now();
        Self {
            id: generate_topic_id(),
            keywords,
            created_at: now,
            modified_at: now,
        }
    }
}

/// Generate a 6-character alphanumeric ID (same as old cards)
pub fn generate_topic_id() -> String {
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

/// Generate unique topic ID by checking database
pub fn generate_unique_topic_id() -> Result<String> {
    let conn = crate::database::init_database()?;
    loop {
        let id = generate_topic_id();
        if !crate::database::topic_exists_in_db(&conn, &id)? {
            return Ok(id);
        }
    }
}

/// A topic ready for review with schedule information
#[derive(Debug, Clone)]
pub struct ReviewTopic {
    pub topic_id: String,
    pub keywords: Vec<String>,
    pub schedule: crate::database::TopicSchedule,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_topic_id() {
        let id = generate_topic_id();
        assert_eq!(id.len(), 6);
        assert!(id.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_topic_creation() {
        let keywords = vec!["AI".to_string(), "Machine Learning".to_string()];
        let topic = Topic::new(keywords.clone());

        assert_eq!(topic.keywords, keywords);
        assert_eq!(topic.id.len(), 6);
        assert!(topic.created_at <= Utc::now());
        assert_eq!(topic.created_at, topic.modified_at);
    }

    #[test]
    fn test_generate_topic_id_uniqueness() {
        let id1 = generate_topic_id();
        let id2 = generate_topic_id();

        // Very unlikely to be the same (1 in 62^6)
        assert_ne!(id1, id2);
    }
}

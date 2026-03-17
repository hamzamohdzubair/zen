//! Review session logic with FSRS scheduling

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use fsrs::{MemoryState, FSRS};
use rusqlite::Connection;

use crate::database::{self, CardSchedule};
use crate::storage;

/// A card ready for review
#[derive(Debug, Clone)]
pub struct ReviewCard {
    pub card_id: String,
    pub question: String,
    pub answer: String,
    pub schedule: CardSchedule,
}

/// Preview of next intervals for all ratings
#[derive(Debug, Clone)]
pub struct NextStatePreview {
    pub again_days: f64,
    pub hard_days: f64,
    pub good_days: f64,
    pub easy_days: f64,
}

/// A completed review
#[derive(Debug)]
struct CompletedReview {
    _card_id: String,
    rating: u8,
}

/// Summary statistics for a review session
#[derive(Debug, Clone, Default)]
pub struct ReviewSummary {
    pub total_reviewed: usize,
    pub again_count: usize,
    pub hard_count: usize,
    pub good_count: usize,
    pub easy_count: usize,
}

/// Manages a review session
pub struct ReviewSession {
    fsrs: FSRS,
    due_cards: Vec<ReviewCard>,
    current_index: usize,
    completed_reviews: Vec<CompletedReview>,
    desired_retention: f32,
    conn: Connection,
}

impl ReviewSession {
    /// Create a new review session with all due cards
    pub fn new() -> Result<Self> {
        // Initialize FSRS with default parameters (empty slice = use defaults)
        let fsrs = FSRS::new(Some(&[]))?;
        let desired_retention = 0.9; // 90% target retention

        // Initialize database
        let conn = database::init_database()?;

        // Get due card IDs
        let card_ids = database::get_due_cards(&conn)?;

        if card_ids.is_empty() {
            anyhow::bail!("No cards due for review");
        }

        // Load cards with their schedules
        let mut due_cards = Vec::new();
        for card_id in card_ids {
            // Load card content
            let (question, answer) = match storage::read_card(&card_id) {
                Ok((q, a)) => (q, a),
                Err(e) => {
                    eprintln!("Warning: Skipping card {} - {}", card_id, e);
                    continue;
                }
            };

            // Load schedule
            let schedule = database::get_card_schedule(&conn, &card_id)?;

            due_cards.push(ReviewCard {
                card_id,
                question,
                answer,
                schedule,
            });
        }

        if due_cards.is_empty() {
            anyhow::bail!("No cards due for review");
        }

        Ok(Self {
            fsrs,
            due_cards,
            current_index: 0,
            completed_reviews: Vec::new(),
            desired_retention,
            conn,
        })
    }

    /// Get the current card being reviewed
    pub fn current_card(&self) -> Option<&ReviewCard> {
        self.due_cards.get(self.current_index)
    }

    /// Get the current progress (1-indexed position, total)
    pub fn progress(&self) -> (usize, usize) {
        (self.current_index + 1, self.due_cards.len())
    }

    /// Check if the session is complete
    pub fn is_complete(&self) -> bool {
        self.current_index >= self.due_cards.len()
    }

    /// Preview the next intervals for all rating options
    pub fn preview_next_intervals(&self) -> Result<NextStatePreview> {
        let card = self.current_card().context("No current card to preview")?;

        let now = Utc::now();

        // Calculate elapsed days since last review
        let elapsed_days = if let Some(last_review) = card.schedule.last_review {
            (now - last_review).num_days().max(0) as u32
        } else {
            0
        };

        // Build memory state from existing schedule
        let memory_state = if let (Some(stability), Some(difficulty)) =
            (card.schedule.stability, card.schedule.difficulty)
        {
            Some(MemoryState {
                stability: stability as f32,
                difficulty: difficulty as f32,
            })
        } else {
            None
        };

        // Get next states for all ratings
        let next_states =
            self.fsrs
                .next_states(memory_state, self.desired_retention, elapsed_days)?;

        Ok(NextStatePreview {
            again_days: next_states.again.interval as f64,
            hard_days: next_states.hard.interval as f64,
            good_days: next_states.good.interval as f64,
            easy_days: next_states.easy.interval as f64,
        })
    }

    /// Submit a rating for the current card and move to the next
    pub fn submit_rating(&mut self, rating: u8) -> Result<()> {
        if !(1..=4).contains(&rating) {
            anyhow::bail!("Rating must be between 1 and 4");
        }

        let card = self
            .current_card()
            .context("No current card to rate")?
            .clone();

        let now = Utc::now();

        // Calculate elapsed days since last review
        let elapsed_days = if let Some(last_review) = card.schedule.last_review {
            (now - last_review).num_days().max(0) as u32
        } else {
            0
        };

        // Build memory state from existing schedule
        let memory_state = if let (Some(stability), Some(difficulty)) =
            (card.schedule.stability, card.schedule.difficulty)
        {
            Some(MemoryState {
                stability: stability as f32,
                difficulty: difficulty as f32,
            })
        } else {
            None
        };

        // Get next states from FSRS
        let next_states =
            self.fsrs
                .next_states(memory_state, self.desired_retention, elapsed_days)?;

        // Select the appropriate state based on rating
        let selected = match rating {
            1 => next_states.again,
            2 => next_states.hard,
            3 => next_states.good,
            4 => next_states.easy,
            _ => unreachable!(),
        };

        // Calculate new due date
        let interval_days = (selected.interval as f64).round().max(1.0) as i64;
        let new_due_date = now + Duration::days(interval_days);

        // Extract new memory state
        let new_stability = selected.memory.stability as f64;
        let new_difficulty = selected.memory.difficulty as f64;

        // Calculate retrievability (R = e^(-t/S) where t=elapsed_days, S=stability)
        // For a newly reviewed card, retrievability is close to 1.0
        let new_retrievability = if new_stability > 0.0 {
            (-(elapsed_days as f64) / new_stability).exp()
        } else {
            1.0
        };

        // Save to database
        database::insert_review_log(
            &self.conn,
            &card.card_id,
            rating,
            selected.interval as f64,
            elapsed_days as f64,
        )?;

        database::update_card_schedule(
            &self.conn,
            &card.card_id,
            &new_due_date,
            new_stability,
            new_difficulty,
            new_retrievability,
        )?;

        // Track completion
        self.completed_reviews.push(CompletedReview {
            _card_id: card.card_id,
            rating,
        });

        // Move to next card
        self.current_index += 1;

        Ok(())
    }

    /// Get summary statistics for the session
    pub fn summary(&self) -> ReviewSummary {
        let mut summary = ReviewSummary {
            total_reviewed: self.completed_reviews.len(),
            ..Default::default()
        };

        for review in &self.completed_reviews {
            match review.rating {
                1 => summary.again_count += 1,
                2 => summary.hard_count += 1,
                3 => summary.good_count += 1,
                4 => summary.easy_count += 1,
                _ => {}
            }
        }

        summary
    }
}

/// Format an interval in days to a human-readable string
pub fn format_interval(days: f64) -> String {
    if days < 1.0 {
        let minutes = (days * 24.0 * 60.0).round() as u32;
        format!("{}m", minutes)
    } else if days < 30.0 {
        format!("{}d", days.round() as u32)
    } else if days < 365.0 {
        let months = (days / 30.0).round() as u32;
        format!("{}mo", months)
    } else {
        let years = (days / 365.0).round() as u32;
        format!("{}y", years)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_interval() {
        assert_eq!(format_interval(0.0), "0m");
        assert_eq!(format_interval(0.007), "10m"); // ~10 minutes
        assert_eq!(format_interval(0.5), "720m"); // 12 hours
        assert_eq!(format_interval(1.0), "1d");
        assert_eq!(format_interval(3.0), "3d");
        assert_eq!(format_interval(8.0), "8d");
        assert_eq!(format_interval(21.0), "21d");
        assert_eq!(format_interval(30.0), "1mo");
        assert_eq!(format_interval(90.0), "3mo");
        assert_eq!(format_interval(365.0), "1y");
        assert_eq!(format_interval(730.0), "2y");
    }

    #[test]
    fn test_review_summary_default() {
        let summary = ReviewSummary::default();
        assert_eq!(summary.total_reviewed, 0);
        assert_eq!(summary.again_count, 0);
        assert_eq!(summary.hard_count, 0);
        assert_eq!(summary.good_count, 0);
        assert_eq!(summary.easy_count, 0);
    }
}

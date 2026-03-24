//! Topic review session logic with FSRS scheduling

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use fsrs::{MemoryState, FSRS};
use rusqlite::Connection;

use crate::database;
use crate::topic::ReviewTopic;

/// Manages a topic review session
pub struct TopicReviewSession {
    fsrs: FSRS,
    pub due_topics: Vec<ReviewTopic>,
    pub current_index: usize,
    desired_retention: f32,
    conn: Connection,
}

impl TopicReviewSession {
    pub fn new() -> Result<Self> {
        // Initialize FSRS with default parameters
        let fsrs = FSRS::new(Some(&[]))?;
        let desired_retention = 0.9;

        // Get due topics
        let conn = database::init_database()?;
        let topic_ids = database::get_due_topics(&conn)?;

        if topic_ids.is_empty() {
            anyhow::bail!("No topics due for review");
        }

        // Load topics with schedules
        let mut due_topics = Vec::new();
        for topic_id in topic_ids {
            let keywords = database::get_topic_keywords(&conn, &topic_id)?;
            let schedule = database::get_topic_schedule(&conn, &topic_id)?;

            due_topics.push(ReviewTopic {
                topic_id,
                keywords,
                schedule,
            });
        }

        Ok(Self {
            fsrs,
            due_topics,
            current_index: 0,
            desired_retention,
            conn,
        })
    }

    pub fn current_topic(&self) -> Option<&ReviewTopic> {
        self.due_topics.get(self.current_index)
    }

    pub fn progress(&self) -> (usize, usize) {
        (self.current_index + 1, self.due_topics.len())
    }

    pub fn is_complete(&self) -> bool {
        self.current_index >= self.due_topics.len()
    }

    /// Get previous questions for the current topic to ensure breadth coverage
    pub fn get_previous_questions(&self) -> Result<Vec<String>> {
        let topic = self.current_topic().context("No current topic")?;
        // Get last 10 questions to avoid repetition
        database::get_topic_previous_questions(&self.conn, &topic.topic_id, 10)
    }

    /// Submit average score for current topic and move to next
    pub fn submit_review(&mut self, average_score: f64, questions_data: Vec<QuestionData>) -> Result<()> {
        // Convert score to rating
        let rating = score_to_rating(average_score);

        // Calculate new schedule using FSRS
        let topic = self.current_topic().context("No current topic")?.clone();
        let now = Utc::now();

        let elapsed_days = if let Some(last_review) = topic.schedule.last_review {
            (now - last_review).num_days().max(0) as u32
        } else {
            0
        };

        let memory_state = if let (Some(stability), Some(difficulty)) =
            (topic.schedule.stability, topic.schedule.difficulty)
        {
            Some(MemoryState {
                stability: stability as f32,
                difficulty: difficulty as f32,
            })
        } else {
            None
        };

        let next_states = self.fsrs.next_states(memory_state, self.desired_retention, elapsed_days)?;

        let selected = match rating {
            1 => next_states.again,
            2 => next_states.hard,
            3 => next_states.good,
            4 => next_states.easy,
            _ => unreachable!(),
        };

        // Update database
        let interval_days = (selected.interval as f64).round().max(1.0) as i64;
        let new_due_date = now + Duration::days(interval_days);

        let review_log_id = database::insert_topic_review_log(
            &self.conn,
            &topic.topic_id,
            rating,
            selected.interval as f64,
            elapsed_days as f64,
            average_score,
        )?;

        // Store individual question logs
        for (i, q_data) in questions_data.iter().enumerate() {
            database::insert_topic_question_log(
                &self.conn,
                review_log_id,
                (i + 1) as i32,
                &q_data.question,
                &q_data.user_answer,
                q_data.score,
                q_data.feedback.as_deref(),
            )?;
        }

        database::update_topic_schedule(
            &self.conn,
            &topic.topic_id,
            &new_due_date,
            selected.memory.stability as f64,
            selected.memory.difficulty as f64,
            0.9,
        )?;

        self.current_index += 1;
        Ok(())
    }
}

/// Convert LLM score (0-100) to FSRS rating (1-4)
pub fn score_to_rating(score: f64) -> u8 {
    if score >= 90.0 {
        4 // Easy
    } else if score >= 70.0 {
        3 // Good
    } else if score >= 60.0 {
        2 // Hard
    } else {
        1 // Again
    }
}

/// Data for a single question in a review
#[derive(Debug, Clone)]
pub struct QuestionData {
    pub question: String,
    pub user_answer: String,
    pub score: f64,
    pub feedback: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_to_rating() {
        assert_eq!(score_to_rating(95.0), 4); // Easy
        assert_eq!(score_to_rating(90.0), 4); // Easy
        assert_eq!(score_to_rating(85.0), 3); // Good
        assert_eq!(score_to_rating(70.0), 3); // Good
        assert_eq!(score_to_rating(65.0), 2); // Hard
        assert_eq!(score_to_rating(60.0), 2); // Hard
        assert_eq!(score_to_rating(59.0), 1); // Again
        assert_eq!(score_to_rating(30.0), 1); // Again
        assert_eq!(score_to_rating(0.0), 1);  // Again
    }
}

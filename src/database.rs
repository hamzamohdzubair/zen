//! SQLite database operations for topic-based spaced repetition

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

/// Initialize the database and create topic tables
pub fn init_database() -> Result<Connection> {
    crate::config::ensure_directories()?;
    let db_path = crate::config::db_path()?;

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    // Create topics table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS topics (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            modified_at TEXT NOT NULL
        )",
        [],
    )?;

    // Create topic keywords table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS topic_keywords (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic_id TEXT NOT NULL,
            keyword TEXT NOT NULL,
            position INTEGER NOT NULL,
            FOREIGN KEY(topic_id) REFERENCES topics(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_topic_keywords_topic_id
         ON topic_keywords(topic_id)",
        [],
    )?;

    // Create topic schedule table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS topic_schedule (
            topic_id TEXT PRIMARY KEY,
            due_date TEXT NOT NULL,
            stability REAL,
            difficulty REAL,
            retrievability REAL,
            FOREIGN KEY(topic_id) REFERENCES topics(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create topic review logs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS topic_review_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            rating INTEGER NOT NULL,
            scheduled_days REAL NOT NULL,
            elapsed_days REAL NOT NULL,
            average_score REAL NOT NULL,
            FOREIGN KEY(topic_id) REFERENCES topics(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create topic question logs table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS topic_question_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            review_log_id INTEGER NOT NULL,
            question_number INTEGER NOT NULL,
            generated_question TEXT NOT NULL,
            user_answer TEXT NOT NULL,
            llm_score REAL NOT NULL,
            llm_feedback TEXT,
            FOREIGN KEY(review_log_id) REFERENCES topic_review_logs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    Ok(conn)
}

/// Insert a new topic into the database
pub fn insert_topic(
    conn: &Connection,
    id: &str,
    created_at: &str,
    modified_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO topics (id, created_at, modified_at) VALUES (?1, ?2, ?3)",
        params![id, created_at, modified_at],
    )?;
    Ok(())
}

/// Insert keywords for a topic
pub fn insert_topic_keywords(
    conn: &Connection,
    topic_id: &str,
    keywords: &[String],
) -> Result<()> {
    for (position, keyword) in keywords.iter().enumerate() {
        conn.execute(
            "INSERT INTO topic_keywords (topic_id, keyword, position) VALUES (?1, ?2, ?3)",
            params![topic_id, keyword, position as i32],
        )?;
    }
    Ok(())
}

/// Get keywords for a topic
pub fn get_topic_keywords(conn: &Connection, topic_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT keyword FROM topic_keywords
         WHERE topic_id = ?1
         ORDER BY position ASC",
    )?;

    let keywords = stmt
        .query_map(params![topic_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(keywords)
}

/// Check if a topic exists
pub fn topic_exists_in_db(conn: &Connection, id: &str) -> Result<bool> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM topics WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Initialize schedule for a new topic (due immediately)
pub fn insert_initial_topic_schedule(
    conn: &Connection,
    topic_id: &str,
    due_date: &DateTime<Utc>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO topic_schedule (topic_id, due_date, stability, difficulty, retrievability)
         VALUES (?1, ?2, NULL, NULL, NULL)",
        params![topic_id, due_date.to_rfc3339()],
    )?;
    Ok(())
}

/// Get topic IDs that are due for review
pub fn get_due_topics(conn: &Connection) -> Result<Vec<String>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn.prepare(
        "SELECT DISTINCT topic_id FROM topic_schedule
         WHERE due_date <= ?1
         ORDER BY due_date ASC",
    )?;

    let ids = stmt
        .query_map(params![now], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(ids)
}

/// Topic schedule information
#[derive(Debug, Clone)]
pub struct TopicSchedule {
    pub due_date: DateTime<Utc>,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub retrievability: Option<f64>,
    pub last_review: Option<DateTime<Utc>>,
}

/// Get the schedule for a topic
pub fn get_topic_schedule(conn: &Connection, topic_id: &str) -> Result<TopicSchedule> {
    let (due_date_str, stability, difficulty, retrievability): (String, Option<f64>, Option<f64>, Option<f64>) =
        conn.query_row(
            "SELECT due_date, stability, difficulty, retrievability FROM topic_schedule WHERE topic_id = ?1",
            params![topic_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

    let due_date = DateTime::parse_from_rfc3339(&due_date_str)
        .context("Failed to parse due_date")?
        .with_timezone(&Utc);

    // Get last review timestamp from review_logs
    let last_review: Option<String> = conn
        .query_row(
            "SELECT timestamp FROM topic_review_logs WHERE topic_id = ?1 ORDER BY timestamp DESC LIMIT 1",
            params![topic_id],
            |row| row.get(0),
        )
        .ok();

    let last_review = last_review
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(TopicSchedule {
        due_date,
        stability,
        difficulty,
        retrievability,
        last_review,
    })
}

/// Update topic schedule with new FSRS parameters
pub fn update_topic_schedule(
    conn: &Connection,
    topic_id: &str,
    due_date: &DateTime<Utc>,
    stability: f64,
    difficulty: f64,
    retrievability: f64,
) -> Result<()> {
    conn.execute(
        "UPDATE topic_schedule
         SET due_date = ?1, stability = ?2, difficulty = ?3, retrievability = ?4
         WHERE topic_id = ?5",
        params![
            due_date.to_rfc3339(),
            stability,
            difficulty,
            retrievability,
            topic_id
        ],
    )?;
    Ok(())
}

/// Insert a topic review log and return the log ID
pub fn insert_topic_review_log(
    conn: &Connection,
    topic_id: &str,
    rating: u8,
    scheduled_days: f64,
    elapsed_days: f64,
    average_score: f64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO topic_review_logs (topic_id, timestamp, rating, scheduled_days, elapsed_days, average_score)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            topic_id,
            Utc::now().to_rfc3339(),
            rating as i32,
            scheduled_days,
            elapsed_days,
            average_score
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Insert a question log for a specific review
pub fn insert_topic_question_log(
    conn: &Connection,
    review_log_id: i64,
    question_number: i32,
    question: &str,
    answer: &str,
    score: f64,
    feedback: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO topic_question_logs (review_log_id, question_number, generated_question, user_answer, llm_score, llm_feedback)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            review_log_id,
            question_number,
            question,
            answer,
            score,
            feedback
        ],
    )?;
    Ok(())
}

/// Get previous questions asked for a topic (for breadth coverage)
/// Only returns questions from reviews that were rated "Easy" (rating = 4)
/// to avoid repeating questions the user already found easy
/// Get questions from "Easy" (rating=4) sessions in the last 10 days for a topic
pub fn get_topic_previous_questions(conn: &Connection, topic_id: &str, limit: usize) -> Result<Vec<String>> {
    // Calculate timestamp for 10 days ago
    let ten_days_ago = (Utc::now() - chrono::Duration::days(10)).to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT tql.generated_question
         FROM topic_question_logs tql
         JOIN topic_review_logs trl ON tql.review_log_id = trl.id
         WHERE trl.topic_id = ?1
           AND trl.rating = 4
           AND trl.timestamp >= ?2
         ORDER BY trl.timestamp DESC
         LIMIT ?3"
    )?;

    let questions = stmt
        .query_map(params![topic_id, ten_days_ago, limit], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(questions)
}

/// Get count of questions answered today for a topic
pub fn get_today_question_count(conn: &Connection, topic_id: &str) -> Result<usize> {
    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM topic_question_logs tql
         JOIN topic_review_logs trl ON tql.review_log_id = trl.id
         WHERE trl.topic_id = ?1 AND trl.timestamp >= ?2",
        params![topic_id, today_start],
        |row| row.get(0),
    )?;

    Ok(count as usize)
}

/// Get today's question scores for a topic (returns up to 3 scores)
pub fn get_today_question_scores(conn: &Connection, topic_id: &str) -> Result<Vec<f64>> {
    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT tql.llm_score
         FROM topic_question_logs tql
         JOIN topic_review_logs trl ON tql.review_log_id = trl.id
         WHERE trl.topic_id = ?1 AND trl.timestamp >= ?2
         ORDER BY tql.question_number ASC",
    )?;

    let scores = stmt
        .query_map(params![topic_id, today_start], |row| row.get(0))?
        .collect::<Result<Vec<f64>, _>>()?;

    Ok(scores)
}

/// Get ALL questions asked today (for any topic) to avoid repetition within the same day
pub fn get_all_questions_asked_today(conn: &Connection) -> Result<Vec<String>> {
    let today_start = Utc::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT tql.generated_question
         FROM topic_question_logs tql
         JOIN topic_review_logs trl ON tql.review_log_id = trl.id
         WHERE trl.timestamp >= ?1
         ORDER BY trl.timestamp DESC",
    )?;

    let questions = stmt
        .query_map(params![today_start], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(questions)
}

/// Find a topic by its keywords (returns topic_id if found)
pub fn find_topic_by_keywords(conn: &Connection, keywords: &[String]) -> Result<Option<String>> {
    // Get all topics
    let all_topics = get_all_topics(conn)?;

    // Normalize the input keywords for comparison (sort and join)
    let mut normalized_input = keywords.to_vec();
    normalized_input.sort();
    let input_key = normalized_input.join(",").to_lowercase();

    // Check each topic
    for topic in all_topics {
        let mut topic_keywords = topic.keywords.clone();
        topic_keywords.sort();
        let topic_key = topic_keywords.join(",").to_lowercase();

        if topic_key == input_key {
            return Ok(Some(topic.id));
        }
    }

    Ok(None)
}

/// Create a placeholder review log for a topic (to be updated when complete)
pub fn create_topic_review_session(conn: &Connection, topic_id: &str) -> Result<i64> {
    conn.execute(
        "INSERT INTO topic_review_logs (topic_id, timestamp, rating, scheduled_days, elapsed_days, average_score)
         VALUES (?1, ?2, 0, 0, 0, 0.0)",
        params![topic_id, Utc::now().to_rfc3339()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Update an existing review log with final FSRS values
pub fn update_review_log(
    conn: &Connection,
    review_log_id: i64,
    rating: u8,
    scheduled_days: f64,
    elapsed_days: f64,
    average_score: f64,
) -> Result<()> {
    conn.execute(
        "UPDATE topic_review_logs
         SET rating = ?1, scheduled_days = ?2, elapsed_days = ?3, average_score = ?4
         WHERE id = ?5",
        params![rating as i32, scheduled_days, elapsed_days, average_score, review_log_id],
    )?;
    Ok(())
}

/// Delete a topic and all associated data (cascades automatically)
pub fn delete_topic(conn: &Connection, topic_id: &str) -> Result<()> {
    conn.execute("DELETE FROM topics WHERE id = ?1", params![topic_id])?;
    Ok(())
}

/// Topic info for listing
#[derive(Debug, Clone)]
pub struct TopicInfo {
    pub id: String,
    pub keywords: Vec<String>,
    pub due_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Get all topics with their info
pub fn get_all_topics(conn: &Connection) -> Result<Vec<TopicInfo>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.created_at, ts.due_date
         FROM topics t
         LEFT JOIN topic_schedule ts ON t.id = ts.topic_id
         ORDER BY t.created_at DESC",
    )?;

    let topic_rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut topics = Vec::new();
    for row in topic_rows {
        let (id, created_at_str, due_date_str) = row?;

        let keywords = get_topic_keywords(conn, &id)?;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .context("Failed to parse created_at")?
            .with_timezone(&Utc);

        let due_date = if let Some(due_str) = due_date_str {
            DateTime::parse_from_rfc3339(&due_str)
                .context("Failed to parse due_date")?
                .with_timezone(&Utc)
        } else {
            Utc::now()
        };

        topics.push(TopicInfo {
            id,
            keywords,
            due_date,
            created_at,
        });
    }

    Ok(topics)
}

/// Get only due topics with their info
pub fn get_due_topics_with_info(conn: &Connection) -> Result<Vec<TopicInfo>> {
    let due_ids = get_due_topics(conn)?;
    let all_topics = get_all_topics(conn)?;

    Ok(all_topics
        .into_iter()
        .filter(|t| due_ids.contains(&t.id))
        .collect())
}

/// Statistics for the topic system
#[derive(Debug, Clone)]
pub struct TopicStats {
    pub total: usize,
    pub due_today: usize,
    pub due_week: usize,
    pub reviews_completed: usize,
    pub average_score: f64,
}

/// Statistics for keyword view
#[derive(Debug, Clone)]
pub struct KeywordStats {
    pub total_keywords: usize,
    pub total_topics: usize,
    pub due_today: usize,
    pub due_week: usize,
    pub reviews_completed: usize,
    pub average_score: f64,
}

/// Get topic statistics
pub fn get_topic_stats(conn: &Connection) -> Result<TopicStats> {
    let total: i32 = conn.query_row("SELECT COUNT(*) FROM topics", [], |row| row.get(0))?;

    let now = Utc::now();
    let due_today: i32 = conn.query_row(
        "SELECT COUNT(*) FROM topic_schedule WHERE due_date <= ?1",
        params![now.to_rfc3339()],
        |row| row.get(0),
    )?;

    let week_from_now = now + chrono::Duration::days(7);
    let due_week: i32 = conn.query_row(
        "SELECT COUNT(*) FROM topic_schedule WHERE due_date <= ?1",
        params![week_from_now.to_rfc3339()],
        |row| row.get(0),
    )?;

    let reviews_completed: i32 = conn.query_row(
        "SELECT COUNT(*) FROM topic_review_logs",
        [],
        |row| row.get(0),
    )?;

    let average_score: Option<f64> = conn
        .query_row(
            "SELECT AVG(average_score) FROM topic_review_logs",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(TopicStats {
        total: total as usize,
        due_today: due_today as usize,
        due_week: due_week as usize,
        reviews_completed: reviews_completed as usize,
        average_score: average_score.unwrap_or(0.0),
    })
}

/// Question score from a review session
#[derive(Debug, Clone)]
pub struct QuestionScore {
    pub score: f64,
    pub rating: u8,
}

/// Review session with 3 question scores
#[derive(Debug, Clone)]
pub struct ReviewSession {
    pub timestamp: DateTime<Utc>,
    pub questions: Vec<QuestionScore>,  // Should be 3 questions
    pub average_score: f64,
}

/// Data for topic performance statistics
#[derive(Debug, Clone)]
pub struct TopicStatsData {
    pub keywords: Vec<String>,
    pub last_session_score: Option<f64>,
    pub overall_average_score: f64,
    pub recent_sessions: Vec<ReviewSession>,  // Last 10 review sessions
    pub review_count: usize,
}

/// Topic-specific review data for keywords
#[derive(Debug, Clone)]
pub struct KeywordTopicReview {
    pub topic_keywords: Vec<String>,  // All keywords from this topic
    pub recent_sessions: Vec<ReviewSession>,  // Last few sessions from this topic
}

/// Data for keyword performance statistics
#[derive(Debug, Clone)]
pub struct KeywordStatsData {
    pub keyword: String,
    pub topic_count: usize,
    pub review_count: usize,
    pub average_score: f64,
    pub last_review_date: Option<DateTime<Utc>>,
    pub topic_reviews: Vec<KeywordTopicReview>,  // Reviews from each topic containing this keyword
}

/// Get topic performance statistics for stats TUI
pub fn get_topic_performance_stats(conn: &Connection) -> Result<Vec<TopicStatsData>> {
    // Get all topics that have at least one review
    let mut stmt = conn.prepare(
        "SELECT DISTINCT t.id
         FROM topics t
         INNER JOIN topic_review_logs trl ON t.id = trl.topic_id
         ORDER BY t.id"
    )?;

    let topic_ids: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    let mut results = Vec::new();

    for topic_id in topic_ids {
        // Get keywords
        let keywords = get_topic_keywords(conn, &topic_id)?;

        // Get last session score (most recent review)
        let last_session_score: Option<f64> = conn
            .query_row(
                "SELECT average_score FROM topic_review_logs
                 WHERE topic_id = ?1
                 ORDER BY timestamp DESC LIMIT 1",
                params![&topic_id],
                |row| row.get(0),
            )
            .ok();

        // Get overall average score
        let overall_average_score: f64 = conn
            .query_row(
                "SELECT AVG(average_score) FROM topic_review_logs
                 WHERE topic_id = ?1",
                params![&topic_id],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        // Get review count
        let review_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM topic_review_logs
                 WHERE topic_id = ?1",
                params![&topic_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Get last 10 review sessions with their questions
        let mut session_stmt = conn.prepare(
            "SELECT id, timestamp, average_score, rating
             FROM topic_review_logs
             WHERE topic_id = ?1
             ORDER BY timestamp DESC LIMIT 10"
        )?;

        let sessions: Vec<ReviewSession> = session_stmt
            .query_map(params![&topic_id], |row| {
                let review_log_id: i64 = row.get(0)?;
                let timestamp_str: String = row.get(1)?;
                let average_score: f64 = row.get(2)?;
                let rating: i32 = row.get(3)?;

                let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok((review_log_id, timestamp, average_score, rating as u8))
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|(review_log_id, timestamp, average_score, _rating)| {
                // Get the 3 questions for this review session
                let mut q_stmt = conn.prepare(
                    "SELECT llm_score FROM topic_question_logs
                     WHERE review_log_id = ?1
                     ORDER BY question_number ASC"
                ).ok()?;

                let questions: Vec<QuestionScore> = q_stmt
                    .query_map(params![review_log_id], |row| {
                        let score: f64 = row.get(0)?;
                        // Convert score to rating for display
                        let q_rating = if score >= 90.0 {
                            4
                        } else if score >= 70.0 {
                            3
                        } else if score >= 60.0 {
                            2
                        } else {
                            1
                        };
                        Ok(QuestionScore {
                            score,
                            rating: q_rating,
                        })
                    })
                    .ok()?
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;

                Some(ReviewSession {
                    timestamp,
                    questions,
                    average_score,
                })
            })
            .flatten()
            .collect();

        results.push(TopicStatsData {
            keywords,
            last_session_score,
            overall_average_score,
            recent_sessions: sessions,
            review_count: review_count as usize,
        });
    }

    // Sort by overall_average_score ascending (lower scores first)
    results.sort_by(|a, b| {
        a.overall_average_score
            .partial_cmp(&b.overall_average_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

/// Get dates when reviews were completed (for calendar display)
pub fn get_review_dates(conn: &Connection) -> Result<Vec<chrono::NaiveDate>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT DATE(timestamp) as review_date
         FROM topic_review_logs
         WHERE rating > 0
         ORDER BY review_date DESC"
    )?;

    let dates = stmt
        .query_map([], |row| {
            let date_str: String = row.get(0)?;
            Ok(date_str)
        })?
        .filter_map(|result| {
            result.ok().and_then(|date_str| {
                chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()
            })
        })
        .collect();

    Ok(dates)
}

/// Get keyword statistics summary
pub fn get_keyword_stats(conn: &Connection) -> Result<KeywordStats> {
    let total_keywords: i32 = conn.query_row(
        "SELECT COUNT(DISTINCT keyword) FROM topic_keywords",
        [],
        |row| row.get(0),
    )?;

    let total_topics: i32 = conn.query_row("SELECT COUNT(*) FROM topics", [], |row| row.get(0))?;

    let now = Utc::now();

    // Keywords that are due today (any topic containing the keyword is due)
    let due_today: i32 = conn.query_row(
        "SELECT COUNT(DISTINCT tk.keyword)
         FROM topic_keywords tk
         INNER JOIN topic_schedule ts ON tk.topic_id = ts.topic_id
         WHERE ts.due_date <= ?1",
        params![now.to_rfc3339()],
        |row| row.get(0),
    )?;

    let week_from_now = now + chrono::Duration::days(7);
    let due_week: i32 = conn.query_row(
        "SELECT COUNT(DISTINCT tk.keyword)
         FROM topic_keywords tk
         INNER JOIN topic_schedule ts ON tk.topic_id = ts.topic_id
         WHERE ts.due_date <= ?1",
        params![week_from_now.to_rfc3339()],
        |row| row.get(0),
    )?;

    let reviews_completed: i32 = conn.query_row(
        "SELECT COUNT(*) FROM topic_review_logs",
        [],
        |row| row.get(0),
    )?;

    let average_score: Option<f64> = conn
        .query_row(
            "SELECT AVG(average_score) FROM topic_review_logs",
            [],
            |row| row.get(0),
        )
        .ok();

    Ok(KeywordStats {
        total_keywords: total_keywords as usize,
        total_topics: total_topics as usize,
        due_today: due_today as usize,
        due_week: due_week as usize,
        reviews_completed: reviews_completed as usize,
        average_score: average_score.unwrap_or(0.0),
    })
}

/// Get keyword performance statistics for stats TUI
pub fn get_keyword_performance_stats(conn: &Connection) -> Result<Vec<KeywordStatsData>> {
    // First, get all unique keywords with their aggregate stats
    let mut stmt = conn.prepare(
        "SELECT tk.keyword,
                COUNT(DISTINCT tk.topic_id) as topic_count,
                COUNT(trl.id) as review_count,
                AVG(trl.average_score) as avg_score,
                MAX(trl.timestamp) as last_review
         FROM topic_keywords tk
         INNER JOIN topic_review_logs trl ON tk.topic_id = trl.topic_id
         GROUP BY tk.keyword
         ORDER BY avg_score ASC"
    )?;

    let keyword_data: Vec<(String, usize, usize, f64, Option<DateTime<Utc>>)> = stmt
        .query_map([], |row| {
            let keyword: String = row.get(0)?;
            let topic_count: i32 = row.get(1)?;
            let review_count: i32 = row.get(2)?;
            let avg_score: f64 = row.get(3)?;
            let last_review_str: Option<String> = row.get(4)?;

            let last_review_date = last_review_str
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            Ok((
                keyword,
                topic_count as usize,
                review_count as usize,
                avg_score,
                last_review_date,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut results = Vec::new();

    for (keyword, topic_count, review_count, avg_score, last_review_date) in keyword_data {
        // Get all topics containing this keyword
        let mut topic_stmt = conn.prepare(
            "SELECT DISTINCT topic_id FROM topic_keywords WHERE keyword = ?1"
        )?;

        let topic_ids: Vec<String> = topic_stmt
            .query_map(params![&keyword], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        let mut topic_reviews = Vec::new();

        for topic_id in topic_ids {
            // Get all keywords for this topic
            let topic_keywords = get_topic_keywords(conn, &topic_id)?;

            // Get last 3 review sessions for this topic
            let mut session_stmt = conn.prepare(
                "SELECT id, timestamp, average_score
                 FROM topic_review_logs
                 WHERE topic_id = ?1
                 ORDER BY timestamp DESC LIMIT 3"
            )?;

            let sessions: Vec<ReviewSession> = session_stmt
                .query_map(params![&topic_id], |row| {
                    let review_log_id: i64 = row.get(0)?;
                    let timestamp_str: String = row.get(1)?;
                    let average_score: f64 = row.get(2)?;

                    let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok((review_log_id, timestamp, average_score))
                })?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .filter_map(|(review_log_id, timestamp, average_score)| {
                    // Get the 3 questions for this review session
                    let mut q_stmt = conn.prepare(
                        "SELECT llm_score FROM topic_question_logs
                         WHERE review_log_id = ?1
                         ORDER BY question_number ASC"
                    ).ok()?;

                    let questions: Vec<QuestionScore> = q_stmt
                        .query_map(params![review_log_id], |row| {
                            let score: f64 = row.get(0)?;
                            let q_rating = if score >= 90.0 {
                                4
                            } else if score >= 70.0 {
                                3
                            } else if score >= 60.0 {
                                2
                            } else {
                                1
                            };
                            Ok(QuestionScore {
                                score,
                                rating: q_rating,
                            })
                        })
                        .ok()?
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?;

                    Some(ReviewSession {
                        timestamp,
                        questions,
                        average_score,
                    })
                })
                .collect();

            if !sessions.is_empty() {
                topic_reviews.push(KeywordTopicReview {
                    topic_keywords,
                    recent_sessions: sessions,
                });
            }
        }

        results.push(KeywordStatsData {
            keyword,
            topic_count,
            review_count,
            average_score: avg_score,
            last_review_date,
            topic_reviews,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS topics (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS topic_keywords (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic_id TEXT NOT NULL,
                keyword TEXT NOT NULL,
                position INTEGER NOT NULL,
                FOREIGN KEY(topic_id) REFERENCES topics(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS topic_schedule (
                topic_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(topic_id) REFERENCES topics(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS topic_review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                rating INTEGER NOT NULL,
                scheduled_days REAL NOT NULL,
                elapsed_days REAL NOT NULL,
                average_score REAL NOT NULL,
                FOREIGN KEY(topic_id) REFERENCES topics(id)
            )",
            [],
        )
        .unwrap();

        conn
    }

    #[test]
    fn test_insert_and_check_topic_exists() {
        let conn = create_test_db();
        let topic_id = "test-topic-1";
        let now = Utc::now().to_rfc3339();

        assert!(!topic_exists_in_db(&conn, topic_id).unwrap());

        insert_topic(&conn, topic_id, &now, &now).unwrap();

        assert!(topic_exists_in_db(&conn, topic_id).unwrap());
    }

    #[test]
    fn test_insert_and_get_keywords() {
        let conn = create_test_db();
        let topic_id = "test-topic-2";
        let now = Utc::now().to_rfc3339();
        let keywords = vec![
            "AI".to_string(),
            "RMSE".to_string(),
            "metrics".to_string(),
        ];

        insert_topic(&conn, topic_id, &now, &now).unwrap();
        insert_topic_keywords(&conn, topic_id, &keywords).unwrap();

        let retrieved = get_topic_keywords(&conn, topic_id).unwrap();

        assert_eq!(retrieved, keywords);
    }

    #[test]
    fn test_get_due_topics() {
        let conn = create_test_db();
        let now = Utc::now();

        // Create topic due now
        let topic_id_1 = "topic-due";
        insert_topic(&conn, topic_id_1, &now.to_rfc3339(), &now.to_rfc3339()).unwrap();
        insert_initial_topic_schedule(&conn, topic_id_1, &now).unwrap();

        // Create topic due in future
        let topic_id_2 = "topic-future";
        let future = now + chrono::Duration::days(7);
        insert_topic(&conn, topic_id_2, &now.to_rfc3339(), &now.to_rfc3339()).unwrap();
        insert_initial_topic_schedule(&conn, topic_id_2, &future).unwrap();

        let due = get_due_topics(&conn).unwrap();

        assert_eq!(due.len(), 1);
        assert_eq!(due[0], topic_id_1);
    }
}

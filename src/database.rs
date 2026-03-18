//! SQLite database operations for scheduling metadata

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

/// Initialize the database and create tables if they don't exist
pub fn init_database() -> Result<Connection> {
    crate::storage::ensure_directories()?;
    let db_path = crate::storage::db_path()?;

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            modified_at TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS card_schedule (
            card_id TEXT PRIMARY KEY,
            due_date TEXT NOT NULL,
            stability REAL,
            difficulty REAL,
            retrievability REAL,
            FOREIGN KEY(card_id) REFERENCES cards(id)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS review_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            rating INTEGER NOT NULL,
            scheduled_days REAL NOT NULL,
            elapsed_days REAL NOT NULL,
            FOREIGN KEY(card_id) REFERENCES cards(id)
        )",
        [],
    )?;

    Ok(conn)
}

/// Insert a new card into the database
pub fn insert_card(
    conn: &Connection,
    id: &str,
    created_at: &DateTime<Utc>,
    modified_at: &DateTime<Utc>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO cards (id, created_at, modified_at) VALUES (?1, ?2, ?3)",
        params![id, created_at.to_rfc3339(), modified_at.to_rfc3339()],
    )?;

    // Initialize schedule for new card (due after 24-hour learning delay)
    let initial_due_date = Utc::now() + chrono::Duration::hours(24);
    conn.execute(
        "INSERT INTO card_schedule (card_id, due_date, stability, difficulty, retrievability)
         VALUES (?1, ?2, NULL, NULL, NULL)",
        params![id, initial_due_date.to_rfc3339()],
    )?;

    Ok(())
}

/// Check if a card exists in the database
pub fn card_exists_in_db(conn: &Connection, id: &str) -> Result<bool> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM cards WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Get all card IDs
pub fn get_all_card_ids(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM cards ORDER BY created_at DESC")?;
    let ids = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

/// Get cards that are due for review
pub fn get_due_cards(conn: &Connection) -> Result<Vec<String>> {
    let now = Utc::now().to_rfc3339();
    let mut stmt = conn
        .prepare("SELECT card_id FROM card_schedule WHERE due_date <= ?1 ORDER BY due_date ASC")?;
    let ids = stmt
        .query_map(params![now], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

/// Reset a card's schedule (treat as new card)
pub fn reset_card_schedule(conn: &Connection, card_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE card_schedule
         SET due_date = ?1, stability = NULL, difficulty = NULL, retrievability = NULL
         WHERE card_id = ?2",
        params![Utc::now().to_rfc3339(), card_id],
    )?;

    // Optionally clear review logs
    conn.execute(
        "DELETE FROM review_logs WHERE card_id = ?1",
        params![card_id],
    )?;

    Ok(())
}

/// Get total card count
pub fn get_card_count(conn: &Connection) -> Result<usize> {
    let count: i32 = conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    Ok(count as usize)
}

/// Get count of cards due today
pub fn get_due_count(conn: &Connection) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM card_schedule WHERE due_date <= ?1",
        params![now],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Update the modified_at timestamp for a card
pub fn update_modified_at(conn: &Connection, card_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE cards SET modified_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), card_id],
    )?;
    Ok(())
}

/// Card schedule information for FSRS
#[derive(Debug, Clone)]
pub struct CardSchedule {
    pub due_date: DateTime<Utc>,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub retrievability: Option<f64>,
    pub last_review: Option<DateTime<Utc>>,
}

/// Get the schedule for a card
pub fn get_card_schedule(conn: &Connection, card_id: &str) -> Result<CardSchedule> {
    let (due_date_str, stability, difficulty, retrievability): (String, Option<f64>, Option<f64>, Option<f64>) =
        conn.query_row(
            "SELECT due_date, stability, difficulty, retrievability FROM card_schedule WHERE card_id = ?1",
            params![card_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

    let due_date = DateTime::parse_from_rfc3339(&due_date_str)
        .context("Failed to parse due_date")?
        .with_timezone(&Utc);

    // Get last review timestamp from review_logs
    let last_review: Option<String> = conn
        .query_row(
            "SELECT timestamp FROM review_logs WHERE card_id = ?1 ORDER BY timestamp DESC LIMIT 1",
            params![card_id],
            |row| row.get(0),
        )
        .ok();

    let last_review = last_review
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(CardSchedule {
        due_date,
        stability,
        difficulty,
        retrievability,
        last_review,
    })
}

/// Insert a review log entry
pub fn insert_review_log(
    conn: &Connection,
    card_id: &str,
    rating: u8,
    scheduled_days: f64,
    elapsed_days: f64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO review_logs (card_id, timestamp, rating, scheduled_days, elapsed_days)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            card_id,
            Utc::now().to_rfc3339(),
            rating as i32,
            scheduled_days,
            elapsed_days
        ],
    )?;
    Ok(())
}

/// Update the card schedule with new FSRS parameters
pub fn update_card_schedule(
    conn: &Connection,
    card_id: &str,
    due_date: &DateTime<Utc>,
    stability: f64,
    difficulty: f64,
    retrievability: f64,
) -> Result<()> {
    conn.execute(
        "UPDATE card_schedule
         SET due_date = ?1, stability = ?2, difficulty = ?3, retrievability = ?4
         WHERE card_id = ?5",
        params![
            due_date.to_rfc3339(),
            stability,
            difficulty,
            retrievability,
            card_id
        ],
    )?;
    Ok(())
}

/// Review log entry
#[derive(Debug, Clone)]
pub struct ReviewLog {
    pub timestamp: DateTime<Utc>,
    pub rating: u8,
    pub scheduled_days: f64,
    pub elapsed_days: f64,
}

/// Get review logs for a specific card
pub fn get_review_logs(conn: &Connection, card_id: &str) -> Result<Vec<ReviewLog>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, rating, scheduled_days, elapsed_days
         FROM review_logs
         WHERE card_id = ?1
         ORDER BY timestamp ASC"
    )?;

    let logs = stmt
        .query_map(params![card_id], |row| {
            let timestamp_str: String = row.get(0)?;
            let timestamp = DateTime::parse_from_rfc3339(&timestamp_str)
                .map_err(|_| rusqlite::Error::InvalidQuery)?
                .with_timezone(&Utc);

            Ok(ReviewLog {
                timestamp,
                rating: row.get::<_, i32>(1)? as u8,
                scheduled_days: row.get(2)?,
                elapsed_days: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(logs)
}

/// Get the total number of reviews for a card
pub fn get_review_count(conn: &Connection, card_id: &str) -> Result<usize> {
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM review_logs WHERE card_id = ?1",
        params![card_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Detailed card statistics
#[derive(Debug, Clone)]
pub struct CardStats {
    pub card_id: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub due_date: DateTime<Utc>,
    pub stability: Option<f64>,
    pub difficulty: Option<f64>,
    pub retrievability: Option<f64>,
    pub review_count: usize,
    pub last_review: Option<DateTime<Utc>>,
    pub rating_counts: [usize; 4], // Count of [Again, Hard, Good, Easy]
}

/// Get detailed statistics for a specific card
pub fn get_card_stats(conn: &Connection, card_id: &str) -> Result<CardStats> {
    // Get basic card info
    let (created_at_str, modified_at_str): (String, String) = conn.query_row(
        "SELECT created_at, modified_at FROM cards WHERE id = ?1",
        params![card_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)?
        .with_timezone(&Utc);
    let modified_at = DateTime::parse_from_rfc3339(&modified_at_str)?
        .with_timezone(&Utc);

    // Get schedule info
    let schedule = get_card_schedule(conn, card_id)?;

    // Get review count
    let review_count = get_review_count(conn, card_id)?;

    // Get rating counts
    let mut rating_counts = [0usize; 4];
    let mut stmt = conn.prepare(
        "SELECT rating, COUNT(*) FROM review_logs WHERE card_id = ?1 GROUP BY rating"
    )?;

    let rating_rows = stmt.query_map(params![card_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i32>(1)?))
    })?;

    for row in rating_rows {
        let (rating, count) = row?;
        if rating >= 1 && rating <= 4 {
            rating_counts[rating as usize - 1] = count as usize;
        }
    }

    Ok(CardStats {
        card_id: card_id.to_string(),
        created_at,
        modified_at,
        due_date: schedule.due_date,
        stability: schedule.stability,
        difficulty: schedule.difficulty,
        retrievability: schedule.retrievability,
        review_count,
        last_review: schedule.last_review,
        rating_counts,
    })
}

/// Get statistics for all cards
pub fn get_all_card_stats(conn: &Connection) -> Result<Vec<CardStats>> {
    let card_ids = get_all_card_ids(conn)?;
    let mut stats = Vec::new();

    for card_id in card_ids {
        stats.push(get_card_stats(conn, &card_id)?);
    }

    Ok(stats)
}

/// Overall system statistics
#[derive(Debug, Clone)]
pub struct SystemStats {
    pub total_cards: usize,
    pub due_today: usize,
    pub total_reviews: usize,
    pub new_cards: usize,      // Cards never reviewed
    pub learning_cards: usize,  // Cards with < 5 reviews
    pub mature_cards: usize,    // Cards with >= 5 reviews
}

/// Get overall system statistics
pub fn get_system_stats(conn: &Connection) -> Result<SystemStats> {
    let total_cards = get_card_count(conn)?;
    let due_today = get_due_count(conn)?;

    let total_reviews: i32 = conn.query_row(
        "SELECT COUNT(*) FROM review_logs",
        [],
        |row| row.get(0),
    )?;

    // Count cards by review count
    let mut stmt = conn.prepare(
        "SELECT
            COUNT(CASE WHEN review_count = 0 THEN 1 END) as new_cards,
            COUNT(CASE WHEN review_count > 0 AND review_count < 5 THEN 1 END) as learning_cards,
            COUNT(CASE WHEN review_count >= 5 THEN 1 END) as mature_cards
         FROM (
             SELECT card_id, COUNT(*) as review_count
             FROM review_logs
             GROUP BY card_id
         )"
    )?;

    let (learning_cards, mature_cards): (i32, i32) = stmt.query_row([], |row| {
        Ok((row.get(1)?, row.get(2)?))
    })?;

    // New cards are those with no reviews
    let reviewed_cards = learning_cards + mature_cards;
    let new_cards = (total_cards as i32 - reviewed_cards).max(0);

    Ok(SystemStats {
        total_cards,
        due_today,
        total_reviews: total_reviews as usize,
        new_cards: new_cards as usize,
        learning_cards: learning_cards as usize,
        mature_cards: mature_cards as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_check_card_exists() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card-1";
        let now = Utc::now();

        // Card should not exist initially
        assert!(!card_exists_in_db(&conn, card_id).unwrap());

        // Insert card
        insert_card(&conn, card_id, &now, &now).unwrap();

        // Card should now exist
        assert!(card_exists_in_db(&conn, card_id).unwrap());
    }

    #[test]
    fn test_update_modified_at() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card-2";
        let created_at = Utc::now();
        let initial_modified = created_at;

        // Insert card
        insert_card(&conn, card_id, &created_at, &initial_modified).unwrap();

        // Get initial modified_at
        let initial_modified_str: String = conn
            .query_row(
                "SELECT modified_at FROM cards WHERE id = ?1",
                params![card_id],
                |row| row.get(0),
            )
            .unwrap();

        // Wait a tiny bit to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Update modified_at
        update_modified_at(&conn, card_id).unwrap();

        // Get new modified_at
        let new_modified_str: String = conn
            .query_row(
                "SELECT modified_at FROM cards WHERE id = ?1",
                params![card_id],
                |row| row.get(0),
            )
            .unwrap();

        // Timestamps should be different
        assert_ne!(initial_modified_str, new_modified_str);
    }

    #[test]
    fn test_reset_card_schedule() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                rating INTEGER NOT NULL,
                scheduled_days REAL NOT NULL,
                elapsed_days REAL NOT NULL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card-3";
        let now = Utc::now();

        // Insert card with some schedule data
        insert_card(&conn, card_id, &now, &now).unwrap();

        // Update schedule with some values
        conn.execute(
            "UPDATE card_schedule SET stability = 5.0, difficulty = 3.0, retrievability = 0.9 WHERE card_id = ?1",
            params![card_id],
        ).unwrap();

        // Add a review log
        conn.execute(
            "INSERT INTO review_logs (card_id, timestamp, rating, scheduled_days, elapsed_days) VALUES (?1, ?2, 3, 1.0, 0.5)",
            params![card_id, now.to_rfc3339()],
        ).unwrap();

        // Reset schedule
        reset_card_schedule(&conn, card_id).unwrap();

        // Check that FSRS params are cleared
        let (stability, difficulty, retrievability): (Option<f64>, Option<f64>, Option<f64>) = conn
            .query_row(
                "SELECT stability, difficulty, retrievability FROM card_schedule WHERE card_id = ?1",
                params![card_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert!(stability.is_none());
        assert!(difficulty.is_none());
        assert!(retrievability.is_none());

        // Check that review logs are cleared
        let log_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM review_logs WHERE card_id = ?1",
                params![card_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(log_count, 0);
    }

    #[test]
    fn test_get_all_card_ids() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let now = Utc::now();

        // Insert multiple cards
        insert_card(&conn, "card-1", &now, &now).unwrap();
        insert_card(&conn, "card-2", &now, &now).unwrap();
        insert_card(&conn, "card-3", &now, &now).unwrap();

        // Get all IDs
        let ids = get_all_card_ids(&conn).unwrap();

        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"card-1".to_string()));
        assert!(ids.contains(&"card-2".to_string()));
        assert!(ids.contains(&"card-3".to_string()));
    }

    #[test]
    fn test_get_card_schedule_new_card() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                rating INTEGER NOT NULL,
                scheduled_days REAL NOT NULL,
                elapsed_days REAL NOT NULL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card";
        let now = Utc::now();

        // Insert new card (NULL FSRS params)
        insert_card(&conn, card_id, &now, &now).unwrap();

        // Get schedule
        let schedule = get_card_schedule(&conn, card_id).unwrap();

        assert!(schedule.stability.is_none());
        assert!(schedule.difficulty.is_none());
        assert!(schedule.retrievability.is_none());
        assert!(schedule.last_review.is_none());
    }

    #[test]
    fn test_get_card_schedule_with_history() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                rating INTEGER NOT NULL,
                scheduled_days REAL NOT NULL,
                elapsed_days REAL NOT NULL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card-with-history";
        let now = Utc::now();

        // Insert card and update with FSRS params
        insert_card(&conn, card_id, &now, &now).unwrap();

        conn.execute(
            "UPDATE card_schedule SET stability = 5.0, difficulty = 3.0, retrievability = 0.9 WHERE card_id = ?1",
            params![card_id],
        )
        .unwrap();

        // Add a review log
        insert_review_log(&conn, card_id, 3, 8.0, 0.0).unwrap();

        // Get schedule
        let schedule = get_card_schedule(&conn, card_id).unwrap();

        assert_eq!(schedule.stability, Some(5.0));
        assert_eq!(schedule.difficulty, Some(3.0));
        assert_eq!(schedule.retrievability, Some(0.9));
        assert!(schedule.last_review.is_some());
    }

    #[test]
    fn test_insert_review_log() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                rating INTEGER NOT NULL,
                scheduled_days REAL NOT NULL,
                elapsed_days REAL NOT NULL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card";
        let now = Utc::now();

        insert_card(&conn, card_id, &now, &now).unwrap();

        // Insert review log
        insert_review_log(&conn, card_id, 3, 8.0, 0.5).unwrap();

        // Verify it was inserted
        let (rating, scheduled_days, elapsed_days): (i32, f64, f64) = conn
            .query_row(
                "SELECT rating, scheduled_days, elapsed_days FROM review_logs WHERE card_id = ?1",
                params![card_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(rating, 3);
        assert_eq!(scheduled_days, 8.0);
        assert_eq!(elapsed_days, 0.5);
    }

    #[test]
    fn test_update_card_schedule() {
        let conn = Connection::open_in_memory().unwrap();

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS cards (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS card_schedule (
                card_id TEXT PRIMARY KEY,
                due_date TEXT NOT NULL,
                stability REAL,
                difficulty REAL,
                retrievability REAL,
                FOREIGN KEY(card_id) REFERENCES cards(id)
            )",
            [],
        )
        .unwrap();

        let card_id = "test-card";
        let now = Utc::now();

        insert_card(&conn, card_id, &now, &now).unwrap();

        // Update schedule with FSRS params
        let new_due_date = now + chrono::Duration::days(8);
        update_card_schedule(&conn, card_id, &new_due_date, 7.5, 4.2, 0.85).unwrap();

        // Verify update
        let (stability, difficulty, retrievability): (f64, f64, f64) = conn
            .query_row(
                "SELECT stability, difficulty, retrievability FROM card_schedule WHERE card_id = ?1",
                params![card_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(stability, 7.5);
        assert_eq!(difficulty, 4.2);
        assert_eq!(retrievability, 0.85);
    }
}

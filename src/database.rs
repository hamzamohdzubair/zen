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

    // Initialize schedule for new card (due immediately)
    conn.execute(
        "INSERT INTO card_schedule (card_id, due_date, stability, difficulty, retrievability)
         VALUES (?1, ?2, NULL, NULL, NULL)",
        params![id, Utc::now().to_rfc3339()],
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
    let mut stmt = conn.prepare(
        "SELECT card_id FROM card_schedule WHERE due_date <= ?1 ORDER BY due_date ASC",
    )?;
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

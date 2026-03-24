//! Integration tests for review session

use chrono::Utc;
use rusqlite::Connection;
use zen::database;

fn setup_test_db() -> Connection {
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

    conn
}

#[test]
fn test_get_card_schedule_new_card() {
    let conn = setup_test_db();
    let card_id = "test-card";
    let now = Utc::now();

    // Insert new card
    database::insert_card(&conn, card_id, &now, &now).unwrap();

    // Get schedule
    let schedule = database::get_card_schedule(&conn, card_id).unwrap();

    assert!(schedule.stability.is_none());
    assert!(schedule.difficulty.is_none());
    assert!(schedule.retrievability.is_none());
    assert!(schedule.last_review.is_none());
}

#[test]
fn test_get_card_schedule_with_history() {
    let conn = setup_test_db();
    let card_id = "test-card";
    let now = Utc::now();

    // Insert card
    database::insert_card(&conn, card_id, &now, &now).unwrap();

    // Update with FSRS params
    let due_date = now + chrono::Duration::days(8);
    database::update_card_schedule(&conn, card_id, &due_date, 7.5, 4.2, 0.85).unwrap();

    // Add review log
    database::insert_review_log(&conn, card_id, 3, 8.0, 0.0).unwrap();

    // Get schedule
    let schedule = database::get_card_schedule(&conn, card_id).unwrap();

    assert_eq!(schedule.stability, Some(7.5));
    assert_eq!(schedule.difficulty, Some(4.2));
    assert_eq!(schedule.retrievability, Some(0.85));
    assert!(schedule.last_review.is_some());
}

#[test]
fn test_insert_review_log() {
    let conn = setup_test_db();
    let card_id = "test-card";
    let now = Utc::now();

    database::insert_card(&conn, card_id, &now, &now).unwrap();

    // Insert review log
    database::insert_review_log(&conn, card_id, 3, 8.0, 0.5).unwrap();

    // Verify
    let (rating, scheduled_days, elapsed_days): (i32, f64, f64) = conn
        .query_row(
            "SELECT rating, scheduled_days, elapsed_days FROM review_logs WHERE card_id = ?1",
            rusqlite::params![card_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(rating, 3);
    assert_eq!(scheduled_days, 8.0);
    assert_eq!(elapsed_days, 0.5);
}

#[test]
fn test_update_card_schedule() {
    let conn = setup_test_db();
    let card_id = "test-card";
    let now = Utc::now();

    database::insert_card(&conn, card_id, &now, &now).unwrap();

    // Update schedule
    let new_due_date = now + chrono::Duration::days(8);
    database::update_card_schedule(&conn, card_id, &new_due_date, 7.5, 4.2, 0.85).unwrap();

    // Verify
    let (stability, difficulty, retrievability): (f64, f64, f64) = conn
        .query_row(
            "SELECT stability, difficulty, retrievability FROM card_schedule WHERE card_id = ?1",
            rusqlite::params![card_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();

    assert_eq!(stability, 7.5);
    assert_eq!(difficulty, 4.2);
    assert_eq!(retrievability, 0.85);
}

#[test]
fn test_multiple_reviews() {
    let conn = setup_test_db();
    let card_id = "test-card";
    let now = Utc::now();

    database::insert_card(&conn, card_id, &now, &now).unwrap();

    // First review
    database::insert_review_log(&conn, card_id, 3, 8.0, 0.0).unwrap();
    let due_date1 = now + chrono::Duration::days(8);
    database::update_card_schedule(&conn, card_id, &due_date1, 7.5, 4.2, 0.85).unwrap();

    // Second review
    database::insert_review_log(&conn, card_id, 3, 21.0, 8.0).unwrap();
    let due_date2 = now + chrono::Duration::days(29);
    database::update_card_schedule(&conn, card_id, &due_date2, 18.3, 4.1, 0.88).unwrap();

    // Verify we have 2 review logs
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM review_logs WHERE card_id = ?1",
            rusqlite::params![card_id],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(count, 2);

    // Verify final schedule
    let schedule = database::get_card_schedule(&conn, card_id).unwrap();
    assert_eq!(schedule.stability, Some(18.3));
    assert_eq!(schedule.difficulty, Some(4.1));
    assert_eq!(schedule.retrievability, Some(0.88));
}

#[test]
fn test_get_due_cards() {
    let conn = setup_test_db();
    let now = Utc::now();

    // Insert 3 cards (due immediately by default)
    database::insert_card(&conn, "card-1", &now, &now).unwrap();
    database::insert_card(&conn, "card-2", &now, &now).unwrap();
    database::insert_card(&conn, "card-3", &now, &now).unwrap();

    // card-1 and card-2 are already due now
    conn.execute(
        "UPDATE card_schedule SET due_date = ?1 WHERE card_id = ?2",
        rusqlite::params![now.to_rfc3339(), "card-1"],
    )
    .unwrap();
    conn.execute(
        "UPDATE card_schedule SET due_date = ?1 WHERE card_id = ?2",
        rusqlite::params![now.to_rfc3339(), "card-2"],
    )
    .unwrap();

    // Make card-3 due in the future
    let future = now + chrono::Duration::days(10);
    conn.execute(
        "UPDATE card_schedule SET due_date = ?1 WHERE card_id = ?2",
        rusqlite::params![future.to_rfc3339(), "card-3"],
    )
    .unwrap();

    // Get due cards
    let due_cards = database::get_due_cards(&conn).unwrap();

    assert_eq!(due_cards.len(), 2);
    assert!(due_cards.contains(&"card-1".to_string()));
    assert!(due_cards.contains(&"card-2".to_string()));
    assert!(!due_cards.contains(&"card-3".to_string()));
}

#[test]
fn test_new_card_immediate_due() {
    let conn = setup_test_db();
    let now = Utc::now();

    // Insert a new card
    database::insert_card(&conn, "new-card", &now, &now).unwrap();

    // Get the schedule for the new card
    let schedule = database::get_card_schedule(&conn, "new-card").unwrap();

    // Verify that the due date is approximately now (immediate)
    let time_diff = (schedule.due_date - now).num_seconds().abs();

    // Allow a small margin of error (within 2 seconds) for test execution time
    assert!(
        time_diff < 2,
        "New card should be due immediately. Expected: {}, Got: {}",
        now,
        schedule.due_date
    );

    // Verify the card is due immediately
    let due_cards = database::get_due_cards(&conn).unwrap();
    assert!(
        due_cards.contains(&"new-card".to_string()),
        "New card should be due immediately"
    );
}

#[test]
fn test_format_interval() {
    use zen::review::format_interval;

    assert_eq!(format_interval(0.0), "0m");
    assert_eq!(format_interval(0.007), "10m");
    assert_eq!(format_interval(0.5), "720m");
    assert_eq!(format_interval(1.0), "1d");
    assert_eq!(format_interval(3.0), "3d");
    assert_eq!(format_interval(8.0), "8d");
    assert_eq!(format_interval(21.0), "21d");
    assert_eq!(format_interval(30.0), "1mo");
    assert_eq!(format_interval(90.0), "3mo");
    assert_eq!(format_interval(365.0), "1y");
    assert_eq!(format_interval(730.0), "2y");
}

//! Command implementations for the zen CLI

use anyhow::Result;
use chrono::Utc;
use tabled::{Table, Tabled, settings::Style};

/// Add a new topic with comma-separated keywords
pub fn add_topic(keywords_str: &str) -> Result<()> {
    // Parse comma-separated keywords
    let keywords: Vec<String> = keywords_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if keywords.is_empty() {
        anyhow::bail!("No keywords provided");
    }

    // Validate keywords
    if keywords.iter().any(|k| k.len() > 100) {
        anyhow::bail!("Keywords must be under 100 characters");
    }

    if keywords.len() > 20 {
        anyhow::bail!("Maximum 20 keywords per topic");
    }

    // Check for existing topic with same keywords
    let conn = crate::database::init_database()?;
    if let Some(existing_id) = crate::database::find_topic_by_keywords(&conn, &keywords)? {
        println!("✗ Topic already exists with ID: {}", existing_id);
        println!("  Keywords: {}", keywords.join(", "));
        println!("\nTip: Use 'zen del {}' to remove it if needed", existing_id);
        anyhow::bail!("Topic with these keywords already exists");
    }

    // Create topic
    let topic_id = crate::topic::generate_unique_topic_id()?;
    let now = Utc::now();

    // Save to database
    crate::database::insert_topic(&conn, &topic_id, &now.to_rfc3339(), &now.to_rfc3339())?;
    crate::database::insert_topic_keywords(&conn, &topic_id, &keywords)?;

    // Initialize schedule (due immediately)
    let due_date = now;
    crate::database::insert_initial_topic_schedule(&conn, &topic_id, &due_date)?;

    println!("✓ Topic created: {}", topic_id);
    println!("  Keywords: {}", keywords.join(", "));
    println!("  Due for review now");

    Ok(())
}

/// Start a topic review session
pub fn start_topic_review() -> Result<()> {
    // Check if there are any due topics
    let conn = crate::database::init_database()?;
    let due_topics = crate::database::get_due_topics(&conn)?;

    if due_topics.is_empty() {
        println!("No topics due for review!");
        println!("\nTip: Use 'zen topics --due' to see when topics are due");
        return Ok(());
    }

    // Launch TUI app
    crate::topic_review_tui::TopicReviewApp::new()?;

    Ok(())
}

/// Show topic statistics
pub fn show_topic_stats() -> Result<()> {
    let conn = crate::database::init_database()?;
    let stats = crate::database::get_topic_stats(&conn)?;

    println!("\n╔════════════════════════════╗");
    println!("║   Topic Statistics         ║");
    println!("╚════════════════════════════╝");
    println!();
    println!("  Total topics:       {}", stats.total);
    println!("  Due today:          {}", stats.due_today);
    println!("  Due this week:      {}", stats.due_week);
    println!("  Reviews completed:  {}", stats.reviews_completed);

    if stats.reviews_completed > 0 {
        println!("  Average score:      {:.1}%", stats.average_score);
    }
    println!();

    Ok(())
}

/// Table row for displaying topic information
#[derive(Tabled)]
struct TopicTableRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Keywords")]
    keywords: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Created")]
    created: String,
}

/// List all topics or only due topics
pub fn list_topics(due_only: bool) -> Result<()> {
    let conn = crate::database::init_database()?;
    let topics = if due_only {
        crate::database::get_due_topics_with_info(&conn)?
    } else {
        crate::database::get_all_topics(&conn)?
    };

    if topics.is_empty() {
        if due_only {
            println!("No topics are due for review");
        } else {
            println!("No topics yet. Create one with: zen add \"keyword1, keyword2\"");
        }
        return Ok(());
    }

    let title = if due_only { "Due Topics" } else { "All Topics" };

    // Build table rows
    let now = Utc::now();
    let mut rows = Vec::new();

    for topic in topics {
        let keywords_display = topic.keywords.join(", ");
        let keywords_display = if keywords_display.len() > 80 {
            format!("{}...", &keywords_display[..77])
        } else {
            keywords_display
        };

        // Format due date with color indicators
        let status = if topic.due_date <= now {
            "🔴 Due now".to_string()
        } else {
            let days = (topic.due_date - now).num_days();
            if days == 0 {
                "🔴 Due today".to_string()
            } else if days == 1 {
                "🟡 Due tomorrow".to_string()
            } else if days <= 3 {
                format!("🟡 Due in {} days", days)
            } else if days <= 7 {
                format!("🟢 Due in {} days", days)
            } else {
                format!("⚪ Due in {} days", days)
            }
        };

        // Format created date
        let created = topic.created_at.format("%Y-%m-%d").to_string();

        rows.push(TopicTableRow {
            id: topic.id,
            keywords: keywords_display,
            status,
            created,
        });
    }

    // Create and display table
    let total_count = rows.len();

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  {}{}║", title, " ".repeat(56 - title.len()));
    println!("╚══════════════════════════════════════════════════════════╝\n");

    let mut table = Table::new(rows);
    table.with(Style::modern());

    println!("{}", table);
    println!("\n📊 Total: {} topic(s)\n", total_count);

    Ok(())
}

/// Delete a topic
pub fn delete_topic(topic_id: &str) -> Result<()> {
    let conn = crate::database::init_database()?;

    // Check if topic exists
    if !crate::database::topic_exists_in_db(&conn, topic_id)? {
        anyhow::bail!("Topic '{}' not found", topic_id);
    }

    // Get keywords for confirmation
    let keywords = crate::database::get_topic_keywords(&conn, topic_id)?;

    // Confirm deletion
    println!("Delete topic '{}'?", topic_id);
    println!("Keywords: {}", keywords.join(", "));
    print!("\nType 'yes' to confirm: ");

    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().eq_ignore_ascii_case("yes") {
        crate::database::delete_topic(&conn, topic_id)?;
        println!("✓ Topic deleted");
    } else {
        println!("Deletion cancelled");
    }

    Ok(())
}

/// Show stats TUI with topic and keyword performance
pub fn show_stats_tui() -> Result<()> {
    crate::stats_tui::StatsApp::new()
}

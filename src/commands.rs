use anyhow::{Context, Result};
use std::io::{self, Write};
use crate::{api_client::ApiClient, auth};

/// Login to forgetmeifyoucan account
pub fn login(email: Option<String>) -> Result<()> {
    // Check if already logged in
    if auth::is_logged_in() {
        println!("✓ Already logged in!");
        println!("Run 'zen logout' first if you want to switch accounts.");
        return Ok(());
    }

    // Get email
    let email = if let Some(email) = email {
        email
    } else {
        print!("Email: ");
        io::stdout().flush()?;
        let mut email = String::new();
        io::stdin().read_line(&mut email)?;
        email.trim().to_string()
    };

    // Get password (hidden)
    let password = rpassword::prompt_password("Password: ")
        .context("Failed to read password")?;

    // Create API client and login
    let client = ApiClient::new(None)?;
    println!("Logging in...");

    let response = client.login(&email, &password)
        .context("Login failed. Please check your credentials.")?;

    // Store token
    auth::store_token(&response.token)?;

    println!("✓ Login successful!");
    println!("Welcome back, {}!", response.user.email);
    println!("Subscription: {} ({})", response.user.subscription_tier, response.user.subscription_status);

    Ok(())
}

/// Logout from account
pub fn logout() -> Result<()> {
    if !auth::is_logged_in() {
        println!("Not logged in.");
        return Ok(());
    }

    auth::delete_token()?;
    println!("✓ Logged out successfully.");

    Ok(())
}

/// Show current user info
pub fn show_me() -> Result<()> {
    let token = auth::get_token()?;
    let client = ApiClient::new(Some(token))?;

    let user = client.get_me()?;

    println!("\n📊 Account Information\n");
    println!("Email: {}", user.email);
    println!("User ID: {}", user.id);
    println!("Subscription: {} ({})", user.subscription_tier, user.subscription_status);
    println!();

    Ok(())
}

/// List all topics
pub fn list_topics(due_only: bool) -> Result<()> {
    let token = auth::get_token()?;
    let client = ApiClient::new(Some(token))?;

    let topics = client.list_topics(due_only)?;

    if topics.is_empty() {
        if due_only {
            println!("✓ No topics due for review!");
        } else {
            println!("No topics found. Add one with: zen add \"Rust, Programming, Web\"");
        }
        return Ok(());
    }

    println!("\n📚 {} Topic(s):\n", topics.len());
    for topic in topics {
        let due = if let Some(due_date) = topic.due_date {
            if due_date < chrono::Utc::now() {
                "⚠️  DUE NOW".to_string()
            } else {
                format!("Due: {}", due_date.format("%Y-%m-%d %H:%M"))
            }
        } else {
            "New topic".to_string()
        };

        println!("  {} → {} [{}]", topic.id, topic.keywords.join(", "), due);
    }
    println!();

    Ok(())
}

/// Add a new topic
pub fn add_topic(keywords: Vec<String>) -> Result<()> {
    let token = auth::get_token()?;
    let client = ApiClient::new(Some(token))?;

    // Join all args into comma-separated keywords
    let keywords_str = keywords.join(" ");
    let keywords: Vec<String> = keywords_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if keywords.is_empty() {
        anyhow::bail!("Please provide at least one keyword. Example: zen add \"Rust, Programming, Web\"");
    }

    println!("Creating topic: {}...", keywords.join(", "));
    let topic = client.create_topic(keywords)?;

    println!("✓ Topic created!");
    println!("ID: {}", topic.id);
    println!("Start reviewing with: zen start");

    Ok(())
}

/// Delete a topic
pub fn delete_topic(topic_id: &str) -> Result<()> {
    let token = auth::get_token()?;
    let client = ApiClient::new(Some(token))?;

    print!("Are you sure you want to delete topic '{}'? [y/N] ", topic_id);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        println!("Cancelled.");
        return Ok(());
    }

    client.delete_topic(topic_id)?;
    println!("✓ Topic deleted.");

    Ok(())
}

/// Show statistics
pub fn show_stats() -> Result<()> {
    let token = auth::get_token()?;
    let client = ApiClient::new(Some(token))?;

    let stats = client.get_stats_summary()?;

    println!("\n📊 Your Statistics\n");
    println!("Total Topics: {}", stats.total_topics);
    println!("Due Today: {}", stats.due_today);
    println!("Total Reviews: {}", stats.total_reviews);
    println!("Average Score: {:.1}%", stats.average_score);
    println!("Current Streak: {} days", stats.current_streak);
    println!();

    Ok(())
}

/// Start a review session (TUI)
pub fn start_review() -> Result<()> {
    let token = auth::get_token()?;
    let client = ApiClient::new(Some(token))?;

    // Get due topics
    let topics = client.get_due_topics()?;

    if topics.is_empty() {
        println!("✓ No topics due for review!");
        println!("Check back later or add more topics with: zen add \"Topic, Keywords\"");
        return Ok(());
    }

    println!("\n📚 {} topic(s) due for review\n", topics.len());
    println!("Starting review session...\n");

    // For now, simple CLI-based review (we'll add TUI later)
    for topic in topics {
        println!("\n{'=':<60}\n", "");
        println!("Topic: {}\n", topic.keywords.join(", "));

        let mut total_score = 0;

        // Ask 3 questions
        for q_num in 1..=3 {
            println!("\nQuestion {}/3:", q_num);

            // Generate question
            let question_response = client.generate_question(&topic.id, q_num)?;
            println!("{}\n", question_response.question);

            // Get user answer
            print!("Your answer: ");
            io::stdout().flush()?;
            let mut answer = String::new();
            io::stdin().read_line(&mut answer)?;
            let answer = answer.trim();

            if answer.is_empty() {
                println!("Skipped.");
                continue;
            }

            // Evaluate answer
            println!("Evaluating...");
            let eval = client.evaluate_answer(&topic.id, &question_response.question, answer)?;

            println!("\nScore: {}/100", eval.score);
            println!("Feedback: {}", eval.feedback);
            println!("Ideal answer: {}", eval.ideal_answer);

            total_score += eval.score;
        }

        // Complete review
        let avg_score = total_score / 3;
        client.complete_review(&topic.id, avg_score)?;

        println!("\n✓ Review completed! Average score: {}/100", avg_score);
    }

    println!("\n{'=':<60}\n", "");
    println!("✓ All reviews completed! Great job! 🎉\n");

    Ok(())
}

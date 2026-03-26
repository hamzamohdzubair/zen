use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use reqwest::blocking::Client;
use chrono::{DateTime, Utc};

// TODO: Change this to your production URL after deployment
const API_BASE_URL: &str = "https://forgetmeifyoucan.fly.dev/v1";
// For local development: const API_BASE_URL: &str = "http://localhost:8080/v1";

pub struct ApiClient {
    client: Client,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(token: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client, token })
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());

        if let Some(token) = &self.token {
            headers.insert(
                "Authorization",
                format!("Bearer {}", token).parse().unwrap(),
            );
        }

        headers
    }
}

// ============================================================================
// Data Types (matching backend API)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub subscription_tier: String,
    pub subscription_status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub keywords: Vec<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTopicRequest {
    pub keywords: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewQuestion {
    pub question: String,
    pub question_number: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluateAnswerRequest {
    pub topic_id: String,
    pub question: String,
    pub user_answer: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvaluationResponse {
    pub score: i32,
    pub feedback: String,
    pub ideal_answer: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteReviewRequest {
    pub topic_id: String,
    pub average_score: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsSummary {
    pub total_topics: i32,
    pub due_today: i32,
    pub total_reviews: i32,
    pub average_score: f64,
    pub current_streak: i32,
}

// ============================================================================
// API Methods
// ============================================================================

impl ApiClient {
    /// Login with email and password
    pub fn login(&self, email: &str, password: &str) -> Result<LoginResponse> {
        let req = LoginRequest {
            email: email.to_string(),
            password: password.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/auth/login", API_BASE_URL))
            .headers(self.headers())
            .json(&req)
            .send()
            .context("Failed to send login request")?;

        if !response.status().is_success() {
            anyhow::bail!("Login failed: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse login response")
    }

    /// Get current user info
    pub fn get_me(&self) -> Result<User> {
        let response = self
            .client
            .get(format!("{}/auth/me", API_BASE_URL))
            .headers(self.headers())
            .send()
            .context("Failed to get user info")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get user info: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse user info")
    }

    /// List all topics
    pub fn list_topics(&self, due_only: bool) -> Result<Vec<Topic>> {
        let url = if due_only {
            format!("{}/topics?due=true", API_BASE_URL)
        } else {
            format!("{}/topics", API_BASE_URL)
        };

        let response = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .context("Failed to list topics")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to list topics: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse topics")
    }

    /// Create a new topic
    pub fn create_topic(&self, keywords: Vec<String>) -> Result<Topic> {
        let req = CreateTopicRequest { keywords };

        let response = self
            .client
            .post(format!("{}/topics", API_BASE_URL))
            .headers(self.headers())
            .json(&req)
            .send()
            .context("Failed to create topic")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to create topic: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse created topic")
    }

    /// Delete a topic
    pub fn delete_topic(&self, topic_id: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/topics/{}", API_BASE_URL, topic_id))
            .headers(self.headers())
            .send()
            .context("Failed to delete topic")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to delete topic: {}", response.text().unwrap_or_default());
        }

        Ok(())
    }

    /// Get statistics summary
    pub fn get_stats_summary(&self) -> Result<StatsSummary> {
        let response = self
            .client
            .get(format!("{}/stats/summary", API_BASE_URL))
            .headers(self.headers())
            .send()
            .context("Failed to get stats")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get stats: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse stats")
    }

    /// Get due topics for review
    pub fn get_due_topics(&self) -> Result<Vec<Topic>> {
        self.list_topics(true)
    }

    /// Generate a review question for a topic
    pub fn generate_question(&self, topic_id: &str, question_number: i32) -> Result<ReviewQuestion> {
        #[derive(Serialize)]
        struct Req {
            topic_id: String,
            question_number: i32,
        }

        let req = Req {
            topic_id: topic_id.to_string(),
            question_number,
        };

        let response = self
            .client
            .post(format!("{}/reviews/question", API_BASE_URL))
            .headers(self.headers())
            .json(&req)
            .send()
            .context("Failed to generate question")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to generate question: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse question")
    }

    /// Evaluate user's answer
    pub fn evaluate_answer(
        &self,
        topic_id: &str,
        question: &str,
        user_answer: &str,
    ) -> Result<EvaluationResponse> {
        let req = EvaluateAnswerRequest {
            topic_id: topic_id.to_string(),
            question: question.to_string(),
            user_answer: user_answer.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/reviews/evaluate", API_BASE_URL))
            .headers(self.headers())
            .json(&req)
            .send()
            .context("Failed to evaluate answer")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to evaluate answer: {}", response.text().unwrap_or_default());
        }

        response
            .json()
            .context("Failed to parse evaluation")
    }

    /// Complete a review session
    pub fn complete_review(&self, topic_id: &str, average_score: i32) -> Result<()> {
        let req = CompleteReviewRequest {
            topic_id: topic_id.to_string(),
            average_score,
        };

        let response = self
            .client
            .post(format!("{}/reviews/complete", API_BASE_URL))
            .headers(self.headers())
            .json(&req)
            .send()
            .context("Failed to complete review")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to complete review: {}", response.text().unwrap_or_default());
        }

        Ok(())
    }
}

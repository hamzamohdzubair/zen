use anyhow::{Context, Result};
use crate::config::LLMConfig;

/// Result of answer evaluation
#[derive(Debug, Clone)]
pub struct AnswerEvaluation {
    pub ideal_answer: String,
    pub score: Option<f64>,
    pub feedback: Option<String>,
    pub tokens_used: Option<u32>,
    pub response_time_ms: Option<u64>,
}

/// Trait for LLM providers that can evaluate flashcard answers
pub trait AnswerEvaluator {
    /// Evaluate a user's answer and return an ideal answer
    fn evaluate_answer(&self, question: &str, user_answer: &str) -> Result<AnswerEvaluation>;

    /// Get the name of this evaluator
    fn name(&self) -> &str;
}

/// Factory function to create evaluator from config
pub fn create_evaluator(config: &LLMConfig) -> Result<Box<dyn AnswerEvaluator>> {
    match config.provider.as_str() {
        "groq" => Ok(Box::new(GroqEvaluator::new(&config.api_key, &config.model)?)),
        _ => anyhow::bail!("Unknown provider: {}", config.provider),
    }
}

// ============================================================================
// Groq Implementation
// ============================================================================

pub struct GroqEvaluator {
    api_key: String,
    model: String,
    client: ureq::Agent,
}

impl GroqEvaluator {
    pub fn new(api_key: &str, model: &str) -> Result<Self> {
        Ok(Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            client: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .build(),
        })
    }

    fn build_prompt(&self, question: &str, user_answer: &str) -> String {
        format!(
            "You are a strict teacher. Evaluate this answer with high standards.

Question: {}
Student's Answer: {}

Provide ONLY:
SCORE: [0-100, be very strict]
ANSWER: [comprehensive, complete answer that would score 100%]",
            question, user_answer
        )
    }

    fn call_groq_api(&self, question: &str, user_answer: &str) -> Result<AnswerEvaluation> {
        let start_time = std::time::Instant::now();
        let prompt = self.build_prompt(question, user_answer);

        let request_body = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": prompt
            }],
            "temperature": 0.5,
            "max_tokens": 1024,
        });

        let json_body = serde_json::to_string(&request_body)
            .context("Failed to serialize request")?;

        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_string(&json_body)
            .context("Failed to call Groq API")?;

        let elapsed = start_time.elapsed();

        if response.status() != 200 {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            anyhow::bail!("API returned status {}: {}", status, body);
        }

        let response_body = response.into_string()
            .context("Failed to read response body")?;

        let response_json: serde_json::Value = serde_json::from_str(&response_body)
            .context("Failed to parse JSON response")?;

        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .context("Missing content in response")?;

        // Extract tokens used
        let tokens_used = response_json["usage"]["total_tokens"]
            .as_u64()
            .map(|t| t as u32);

        // Parse SCORE and ANSWER from response
        let (score, ideal_answer) = self.parse_response(content);

        Ok(AnswerEvaluation {
            ideal_answer,
            score: Some(score),
            feedback: None,
            tokens_used,
            response_time_ms: Some(elapsed.as_millis() as u64),
        })
    }

    fn parse_response(&self, content: &str) -> (f64, String) {
        let mut score = None;
        let mut answer = String::new();
        let mut in_answer = false;

        for line in content.lines() {
            if line.starts_with("SCORE:") {
                if let Some(score_str) = line.strip_prefix("SCORE:") {
                    score = score_str.trim().parse::<f64>().ok();
                }
            } else if line.starts_with("ANSWER:") {
                in_answer = true;
                if let Some(answer_start) = line.strip_prefix("ANSWER:") {
                    let trimmed = answer_start.trim();
                    if !trimmed.is_empty() {
                        answer.push_str(trimmed);
                        answer.push('\n');
                    }
                }
            } else if in_answer {
                answer.push_str(line);
                answer.push('\n');
            }
        }

        let score = score.unwrap_or(0.0);
        let answer = answer.trim().to_string();

        // Fallback: if parsing failed, use entire content as answer
        if answer.is_empty() {
            return (score, content.to_string());
        }

        (score, answer)
    }
}

impl AnswerEvaluator for GroqEvaluator {
    fn evaluate_answer(&self, question: &str, user_answer: &str) -> Result<AnswerEvaluation> {
        self.call_groq_api(question, user_answer)
    }

    fn name(&self) -> &str {
        "Groq (Cloud)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_prompt_building() {
        let evaluator = GroqEvaluator::new("test_key", "test_model").unwrap();
        let prompt = evaluator.build_prompt("What is Rust?", "A programming language");

        assert!(prompt.contains("What is Rust?"));
        assert!(prompt.contains("A programming language"));
        assert!(prompt.contains("ideal flashcard answer"));
    }

    #[test]
    fn test_evaluator_name() {
        let evaluator = GroqEvaluator::new("test_key", "test_model").unwrap();
        assert_eq!(evaluator.name(), "Groq");
    }

    #[test]
    fn test_unknown_provider() {
        let config = LLMConfig {
            provider: "unknown".to_string(),
            api_key: "test".to_string(),
            model: "test".to_string(),
        };

        let result = create_evaluator(&config);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Unknown provider"));
        }
    }
}

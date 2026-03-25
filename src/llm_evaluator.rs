use anyhow::{Context, Result};
use crate::config::{LLMConfig, WebSearchConfig};

/// Result of answer evaluation
#[derive(Debug, Clone)]
pub struct AnswerEvaluation {
    pub ideal_answer: String,
    pub score: Option<f64>,
    pub feedback: Option<String>,
    pub tokens_used: Option<u32>,
    pub response_time_ms: Option<u64>,
}

/// Trait for LLM providers that can evaluate answers
pub trait AnswerEvaluator {
    /// Evaluate a user's answer and return score + feedback
    fn evaluate_answer(&self, question: &str, user_answer: &str) -> Result<AnswerEvaluation>;

    /// Get the name of this evaluator
    fn name(&self) -> &str;
}

/// Trait for LLM providers that can generate questions
pub trait QuestionGenerator {
    /// Generate a question covering the given keywords, avoiding previous questions
    fn generate_question(&self, keywords: &[String], previous_questions: &[String]) -> Result<String>;

    /// Get the name of this generator
    fn name(&self) -> &str;
}

/// Factory function to create evaluator from config
pub fn create_evaluator(config: &LLMConfig, web_search: Option<WebSearchConfig>) -> Result<Box<dyn AnswerEvaluator>> {
    match config.provider.as_str() {
        "groq" => {
            if let Some(web_config) = web_search {
                Ok(Box::new(GroqEvaluator::new_with_web_search(&config.api_key, &config.model, web_config)?))
            } else {
                Ok(Box::new(GroqEvaluator::new(&config.api_key, &config.model)?))
            }
        }
        _ => anyhow::bail!("Unknown provider: {}", config.provider),
    }
}

/// Factory function to create question generator from config
pub fn create_question_generator(config: &LLMConfig, web_search: Option<WebSearchConfig>) -> Result<Box<dyn QuestionGenerator>> {
    match config.provider.as_str() {
        "groq" => {
            if let Some(web_config) = web_search {
                Ok(Box::new(GroqEvaluator::new_with_web_search(&config.api_key, &config.model, web_config)?))
            } else {
                Ok(Box::new(GroqEvaluator::new(&config.api_key, &config.model)?))
            }
        }
        _ => anyhow::bail!("Unknown provider: {}", config.provider),
    }
}

// ============================================================================
// Groq Implementation
// ============================================================================

/// Groq API evaluator (implements both AnswerEvaluator and QuestionGenerator)
pub struct GroqEvaluator {
    api_key: String,
    model: String,
    client: ureq::Agent,
    web_search: Option<WebSearchConfig>,
}

impl GroqEvaluator {
    pub fn new(api_key: &str, model: &str) -> Result<Self> {
        Ok(Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            client: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .build(),
            web_search: None,
        })
    }

    pub fn new_with_web_search(api_key: &str, model: &str, web_search: WebSearchConfig) -> Result<Self> {
        Ok(Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            client: ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .build(),
            web_search: Some(web_search),
        })
    }

    fn build_evaluation_prompt(&self, question: &str, user_answer: &str) -> String {
        format!(
            "You are a teacher grading an exam answer.

Question: {}
Student's Answer: {}

Evaluate the answer and provide:
IDEAL_ANSWER: [A complete, accurate answer that would score 100%]
SCORE: [0-100 based on accuracy and completeness]
FEEDBACK: [One sentence explaining what was good or what was missing]",
            question, user_answer
        )
    }

    fn build_question_prompt(&self, keywords: &[String], previous_questions: &[String]) -> String {
        let keywords_str = keywords.join(", ");

        let mut prompt = format!(
            "You are a teacher creating exam questions. Generate ONE clear, specific question that covers these topics: {}

The question should:
- Test understanding of these concepts
- Be answerable in 2-3 sentences
- Be specific and focused
- Cover a DIFFERENT aspect than previous questions",
            keywords_str
        );

        if !previous_questions.is_empty() {
            prompt.push_str("\n\nPrevious questions asked (DO NOT repeat these aspects):\n");
            for (i, prev_q) in previous_questions.iter().enumerate() {
                prompt.push_str(&format!("{}. {}\n", i + 1, prev_q));
            }
            prompt.push_str("\nGenerate a question that explores a different aspect of the topics.");
        }

        prompt.push_str("\n\nProvide ONLY the question text, nothing else.");
        prompt
    }

    fn call_groq_api_evaluation(&self, question: &str, user_answer: &str) -> Result<AnswerEvaluation> {
        let start_time = std::time::Instant::now();
        let prompt = self.build_evaluation_prompt(question, user_answer);

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

        // Parse IDEAL_ANSWER, SCORE and FEEDBACK from response
        let (ideal_answer, score, feedback) = self.parse_evaluation_response(content);

        Ok(AnswerEvaluation {
            ideal_answer,
            score: Some(score),
            feedback: Some(feedback),
            tokens_used,
            response_time_ms: Some(elapsed.as_millis() as u64),
        })
    }

    fn call_groq_api_question(&self, keywords: &[String], previous_questions: &[String]) -> Result<String> {
        let prompt = self.build_question_prompt(keywords, previous_questions);

        let mut messages = vec![serde_json::json!({
            "role": "user",
            "content": prompt
        })];

        let mut request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.8,  // Higher for variety
            "max_tokens": 512,
        });

        // Add web search tool if configured
        if self.web_search.is_some() {
            request_body["tools"] = serde_json::json!([{
                "type": "function",
                "function": {
                    "name": "web_search",
                    "description": "Search the web for current, up-to-date information about a topic. Use this when you need recent information, latest developments, or when the topic might have changed since your training data.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query to find relevant information"
                            }
                        },
                        "required": ["query"]
                    }
                }
            }]);
            request_body["tool_choice"] = serde_json::json!("auto");
        }

        // Make initial request
        let json_body = serde_json::to_string(&request_body)
            .context("Failed to serialize request")?;

        let response = self.client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .set("Authorization", &format!("Bearer {}", self.api_key))
            .set("Content-Type", "application/json")
            .send_string(&json_body)
            .context("Failed to call Groq API")?;

        if response.status() != 200 {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            anyhow::bail!("API returned status {}: {}", status, body);
        }

        let response_body = response.into_string()
            .context("Failed to read response body")?;

        let mut response_json: serde_json::Value = serde_json::from_str(&response_body)
            .context("Failed to parse JSON response")?;

        // Check if the model wants to call a tool
        let message = &response_json["choices"][0]["message"];

        if let Some(tool_calls) = message.get("tool_calls") {
            if let Some(tool_calls_array) = tool_calls.as_array() {
                if !tool_calls_array.is_empty() {
                    // Handle tool calls
                    messages.push(message.clone());

                    for tool_call in tool_calls_array {
                        let function_name = tool_call["function"]["name"].as_str()
                            .context("Missing function name")?;
                        let arguments_str = tool_call["function"]["arguments"].as_str()
                            .context("Missing function arguments")?;
                        let tool_call_id = tool_call["id"].as_str()
                            .context("Missing tool call id")?;

                        if function_name == "web_search" {
                            let args: serde_json::Value = serde_json::from_str(arguments_str)
                                .context("Failed to parse function arguments")?;
                            let query = args["query"].as_str()
                                .context("Missing query parameter")?;

                            // Perform web search
                            let search_results = self.web_search(query)?;

                            // Add tool response to messages
                            messages.push(serde_json::json!({
                                "role": "tool",
                                "tool_call_id": tool_call_id,
                                "name": function_name,
                                "content": search_results
                            }));
                        }
                    }

                    // Make follow-up request with tool results
                    let follow_up_body = serde_json::json!({
                        "model": self.model,
                        "messages": messages,
                        "temperature": 0.8,
                        "max_tokens": 512,
                    });

                    let json_body = serde_json::to_string(&follow_up_body)
                        .context("Failed to serialize follow-up request")?;

                    let response = self.client
                        .post("https://api.groq.com/openai/v1/chat/completions")
                        .set("Authorization", &format!("Bearer {}", self.api_key))
                        .set("Content-Type", "application/json")
                        .send_string(&json_body)
                        .context("Failed to call Groq API for follow-up")?;

                    if response.status() != 200 {
                        let status = response.status();
                        let body = response.into_string().unwrap_or_default();
                        anyhow::bail!("Follow-up API returned status {}: {}", status, body);
                    }

                    let response_body = response.into_string()
                        .context("Failed to read follow-up response body")?;

                    response_json = serde_json::from_str(&response_body)
                        .context("Failed to parse follow-up JSON response")?;
                }
            }
        }

        // Extract final content
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .context("Missing content in response")?;

        Ok(content.trim().to_string())
    }

    /// Perform web search using configured provider
    fn web_search(&self, query: &str) -> Result<String> {
        let web_config = self.web_search.as_ref()
            .context("Web search not configured")?;

        match web_config.provider.as_str() {
            "tavily" => self.tavily_search(query, &web_config.api_key),
            "brave" => self.brave_search(query, &web_config.api_key),
            "serper" => self.serper_search(query, &web_config.api_key),
            "serpapi" => self.serpapi_search(query, &web_config.api_key),
            _ => anyhow::bail!("Unsupported web search provider: {}", web_config.provider),
        }
    }

    /// Tavily Search API implementation
    fn tavily_search(&self, query: &str, api_key: &str) -> Result<String> {
        let request_body = serde_json::json!({
            "api_key": api_key,
            "query": query,
            "search_depth": "basic",
            "include_answer": true,
            "max_results": 3
        });

        let response = self.client
            .post("https://api.tavily.com/search")
            .set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(&request_body)?)
            .context("Failed to call Tavily API")?;

        if response.status() != 200 {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            anyhow::bail!("Tavily API returned status {}: {}", status, body);
        }

        let response_body = response.into_string()?;
        let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

        // Format results
        let mut result = String::new();

        if let Some(answer) = response_json["answer"].as_str() {
            result.push_str(&format!("Summary: {}\n\n", answer));
        }

        if let Some(results) = response_json["results"].as_array() {
            result.push_str("Sources:\n");
            for (i, item) in results.iter().take(3).enumerate() {
                if let (Some(title), Some(content)) = (item["title"].as_str(), item["content"].as_str()) {
                    result.push_str(&format!("{}. {}: {}\n", i + 1, title, content));
                }
            }
        }

        Ok(result)
    }

    /// Brave Search API implementation
    fn brave_search(&self, query: &str, api_key: &str) -> Result<String> {
        let encoded_query = Self::url_encode(query);
        let url = format!("https://api.search.brave.com/res/v1/web/search?q={}", encoded_query);

        let response = self.client
            .get(&url)
            .set("Accept", "application/json")
            .set("X-Subscription-Token", api_key)
            .call()
            .context("Failed to call Brave Search API")?;

        if response.status() != 200 {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            anyhow::bail!("Brave Search API returned status {}: {}", status, body);
        }

        let response_body = response.into_string()?;
        let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

        let mut result = String::new();

        if let Some(results) = response_json["web"]["results"].as_array() {
            for (i, item) in results.iter().take(3).enumerate() {
                if let (Some(title), Some(description)) = (item["title"].as_str(), item["description"].as_str()) {
                    result.push_str(&format!("{}. {}: {}\n", i + 1, title, description));
                }
            }
        }

        Ok(result)
    }

    /// Serper API implementation
    fn serper_search(&self, query: &str, api_key: &str) -> Result<String> {
        let request_body = serde_json::json!({
            "q": query,
            "num": 3
        });

        let response = self.client
            .post("https://google.serper.dev/search")
            .set("X-API-KEY", api_key)
            .set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(&request_body)?)
            .context("Failed to call Serper API")?;

        if response.status() != 200 {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            anyhow::bail!("Serper API returned status {}: {}", status, body);
        }

        let response_body = response.into_string()?;
        let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

        let mut result = String::new();

        if let Some(results) = response_json["organic"].as_array() {
            for (i, item) in results.iter().take(3).enumerate() {
                if let (Some(title), Some(snippet)) = (item["title"].as_str(), item["snippet"].as_str()) {
                    result.push_str(&format!("{}. {}: {}\n", i + 1, title, snippet));
                }
            }
        }

        Ok(result)
    }

    /// SerpAPI implementation
    fn serpapi_search(&self, query: &str, api_key: &str) -> Result<String> {
        let encoded_query = Self::url_encode(query);
        let url = format!("https://serpapi.com/search?q={}&api_key={}", encoded_query, api_key);

        let response = self.client
            .get(&url)
            .call()
            .context("Failed to call SerpAPI")?;

        if response.status() != 200 {
            let status = response.status();
            let body = response.into_string().unwrap_or_default();
            anyhow::bail!("SerpAPI returned status {}: {}", status, body);
        }

        let response_body = response.into_string()?;
        let response_json: serde_json::Value = serde_json::from_str(&response_body)?;

        let mut result = String::new();

        if let Some(results) = response_json["organic_results"].as_array() {
            for (i, item) in results.iter().take(3).enumerate() {
                if let (Some(title), Some(snippet)) = (item["title"].as_str(), item["snippet"].as_str()) {
                    result.push_str(&format!("{}. {}: {}\n", i + 1, title, snippet));
                }
            }
        }

        Ok(result)
    }

    /// Simple URL encoding for query parameters
    fn url_encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }

    fn parse_evaluation_response(&self, content: &str) -> (String, f64, String) {
        let mut ideal_answer = String::new();
        let mut score = None;
        let mut feedback = String::new();
        let mut in_ideal_answer = false;
        let mut in_feedback = false;

        for line in content.lines() {
            if line.starts_with("IDEAL_ANSWER:") {
                in_ideal_answer = true;
                in_feedback = false;
                if let Some(answer_start) = line.strip_prefix("IDEAL_ANSWER:") {
                    let trimmed = answer_start.trim();
                    if !trimmed.is_empty() {
                        ideal_answer.push_str(trimmed);
                        ideal_answer.push(' ');
                    }
                }
            } else if line.starts_with("SCORE:") {
                in_ideal_answer = false;
                in_feedback = false;
                if let Some(score_str) = line.strip_prefix("SCORE:") {
                    score = score_str.trim().parse::<f64>().ok();
                }
            } else if line.starts_with("FEEDBACK:") {
                in_ideal_answer = false;
                in_feedback = true;
                if let Some(feedback_start) = line.strip_prefix("FEEDBACK:") {
                    let trimmed = feedback_start.trim();
                    if !trimmed.is_empty() {
                        feedback.push_str(trimmed);
                        feedback.push(' ');
                    }
                }
            } else if in_ideal_answer {
                ideal_answer.push_str(line.trim());
                ideal_answer.push(' ');
            } else if in_feedback {
                feedback.push_str(line.trim());
                feedback.push(' ');
            }
        }

        let score = score.unwrap_or(0.0);
        let ideal_answer = ideal_answer.trim().to_string();
        let feedback = feedback.trim().to_string();

        // Fallback: if parsing failed, use entire content as feedback
        if ideal_answer.is_empty() && feedback.is_empty() {
            return (String::new(), score, content.to_string());
        }

        (ideal_answer, score, feedback)
    }
}

impl AnswerEvaluator for GroqEvaluator {
    fn evaluate_answer(&self, question: &str, user_answer: &str) -> Result<AnswerEvaluation> {
        self.call_groq_api_evaluation(question, user_answer)
    }

    fn name(&self) -> &str {
        "Groq (Cloud)"
    }
}

impl QuestionGenerator for GroqEvaluator {
    fn generate_question(&self, keywords: &[String], previous_questions: &[String]) -> Result<String> {
        self.call_groq_api_question(keywords, previous_questions)
    }

    fn name(&self) -> &str {
        "Groq (Cloud)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_groq_evaluation_prompt_building() {
        let evaluator = GroqEvaluator::new("test_key", "test_model").unwrap();
        let prompt = evaluator.build_evaluation_prompt("What is Rust?", "A programming language");

        assert!(prompt.contains("What is Rust?"));
        assert!(prompt.contains("A programming language"));
        assert!(prompt.contains("SCORE"));
        assert!(prompt.contains("FEEDBACK"));
    }

    #[test]
    fn test_groq_question_prompt_building() {
        let evaluator = GroqEvaluator::new("test_key", "test_model").unwrap();
        let keywords = vec!["Rust".to_string(), "ownership".to_string()];
        let previous = vec!["What is ownership in Rust?".to_string()];
        let prompt = evaluator.build_question_prompt(&keywords, &previous);

        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("ownership"));
        assert!(prompt.contains("question"));
        assert!(prompt.contains("Previous questions"));
    }

    #[test]
    fn test_evaluator_name() {
        let evaluator = GroqEvaluator::new("test_key", "test_model").unwrap();
        assert_eq!(AnswerEvaluator::name(&evaluator), "Groq (Cloud)");
        assert_eq!(QuestionGenerator::name(&evaluator), "Groq (Cloud)");
    }

    #[test]
    fn test_unknown_provider() {
        let config = LLMConfig {
            provider: "unknown".to_string(),
            api_key: "test".to_string(),
            model: "test".to_string(),
        };

        let result = create_evaluator(&config, None);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Unknown provider"));
        }
    }
}

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMConfig {
    pub provider: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebSearchConfig {
    /// Provider: "tavily", "brave", "serper", or "serpapi"
    pub provider: String,
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub llm: Option<LLMConfig>,
    pub web_search: Option<WebSearchConfig>,
}

impl Config {
    /// Load configuration from ~/.zen/config.toml
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .context("Failed to read config file")?;

        toml::from_str(&content)
            .context("Failed to parse config file")
    }

    /// Save configuration to ~/.zen/config.toml
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        std::fs::write(&path, content)
            .context("Failed to write config file")
    }

    /// Get path to config file: ~/.zen/config.toml
    fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not find home directory")?;
        Ok(home.join(".zen").join("config.toml"))
    }
}

/// Get path to zen directory: ~/.zen
pub fn zen_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .context("Could not find home directory")?;
    Ok(home.join(".zen"))
}

/// Get path to database file: ~/.zen/zen.db
pub fn db_path() -> Result<PathBuf> {
    Ok(zen_dir()?.join("zen.db"))
}

/// Ensure zen directory exists
pub fn ensure_directories() -> Result<()> {
    let zen_path = zen_dir()?;
    std::fs::create_dir_all(&zen_path)
        .with_context(|| format!("Failed to create directory: {}", zen_path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = Config {
            llm: Some(LLMConfig {
                provider: "groq".to_string(),
                api_key: "test_key".to_string(),
                model: "llama-3.3-70b-versatile".to_string(),
            }),
            web_search: None,
        };

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("provider = \"groq\""));
        assert!(toml_str.contains("api_key = \"test_key\""));
        assert!(toml_str.contains("model = \"llama-3.3-70b-versatile\""));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
[llm]
provider = "groq"
api_key = "test_key"
model = "llama-3.3-70b-versatile"
"#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.llm.is_some());

        let llm = config.llm.unwrap();
        assert_eq!(llm.provider, "groq");
        assert_eq!(llm.api_key, "test_key");
        assert_eq!(llm.model, "llama-3.3-70b-versatile");
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.llm.is_none());
    }
}

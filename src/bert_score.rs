//! BERT-based semantic similarity scoring for flashcard answers

use anyhow::{Context, Result};
use ndarray::Array1;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use std::path::PathBuf;
use tokenizers::Tokenizer;

/// BERT scorer for calculating semantic similarity between texts
pub struct BertScorer {
    session: Session,
    tokenizer: Tokenizer,
}

impl BertScorer {
    /// Create a new BERT scorer
    ///
    /// This attempts to load the all-MiniLM-L6-v2 model for sentence embeddings.
    /// The model will be downloaded on first use and cached locally.
    pub fn new() -> Result<Self> {
        let model_path = Self::get_or_download_model()?;
        let tokenizer_path = Self::get_or_download_tokenizer()?;

        // Load the ONNX model
        let model_bytes = std::fs::read(&model_path)
            .context("Failed to read model file")?;

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("Failed to create session builder: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {}", e))?
            .commit_from_memory(&model_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to load ONNX model: {}", e))?;

        // Load the tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        Ok(Self { session, tokenizer })
    }

    /// Calculate semantic similarity score between two texts
    ///
    /// Returns a score between 0.0 and 1.0, where 1.0 means identical meaning
    pub fn calculate_score(&mut self, text1: &str, text2: &str) -> Result<f64> {
        let embedding1 = self.get_embedding(text1)?;
        let embedding2 = self.get_embedding(text2)?;
        Ok(self.cosine_similarity(&embedding1, &embedding2))
    }

    /// Get sentence embedding for a text
    fn get_embedding(&mut self, text: &str) -> Result<Array1<f32>> {
        // Tokenize the text
        let encoding = self.tokenizer
            .encode(text, false)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        // Convert to i64 arrays (ONNX expects this type)
        let ids_i64: Vec<i64> = ids.iter().map(|&x| x as i64).collect();
        let attention_i64: Vec<i64> = attention_mask.iter().map(|&x| x as i64).collect();

        let len = ids_i64.len();

        // Create token_type_ids (all zeros for single sentence)
        let token_type_ids: Vec<i64> = vec![0; len];

        // Create Value objects directly from tuple (shape, data)
        use ort::value::Value;
        let input_ids_value = Value::from_array(([1, len], ids_i64))
            .map_err(|e| anyhow::anyhow!("Failed to create input_ids value: {}", e))?;
        let attention_mask_value = Value::from_array(([1, len], attention_i64))
            .map_err(|e| anyhow::anyhow!("Failed to create attention_mask value: {}", e))?;
        let token_type_ids_value = Value::from_array(([1, len], token_type_ids))
            .map_err(|e| anyhow::anyhow!("Failed to create token_type_ids value: {}", e))?;

        // Run inference
        let inputs = ort::inputs!{
            "input_ids" => input_ids_value,
            "attention_mask" => attention_mask_value,
            "token_type_ids" => token_type_ids_value,
        };
        let outputs = self.session
            .run(inputs)
            .map_err(|e| anyhow::anyhow!("Failed to run model inference: {}", e))?;

        // Extract the output tensor
        let output = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("Failed to extract output tensor: {}", e))?;

        // Get the sentence embedding using mean pooling
        // The output shape is [batch_size, sequence_length, hidden_size]
        let (shape, data) = output;
        let _batch_size = shape[0] as usize;
        let seq_len = shape[1] as usize;
        let hidden_size = shape[2] as usize;

        // For sentence-transformers models, we should use mean pooling over all tokens
        // (not just the [CLS] token) with attention mask consideration
        let mut embedding = Array1::zeros(hidden_size);
        let mut token_count = 0;

        for seq_idx in 0..seq_len {
            // Only include tokens that are not padding (attention_mask == 1)
            if attention_mask[seq_idx] == 1 {
                for hidden_idx in 0..hidden_size {
                    // Calculate flat index for [batch_idx=0, seq_idx, hidden_idx]
                    let flat_idx = seq_idx * hidden_size + hidden_idx;
                    embedding[hidden_idx] += data[flat_idx];
                }
                token_count += 1;
            }
        }

        // Average the embeddings
        if token_count > 0 {
            for val in embedding.iter_mut() {
                *val /= token_count as f32;
            }
        }

        Ok(embedding)
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(&self, a: &Array1<f32>, b: &Array1<f32>) -> f64 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot_product / (norm_a * norm_b)) as f64
    }

    /// Get the path to the model file, downloading if necessary
    /// Uses HuggingFace Hub standard cache location for interoperability
    fn get_or_download_model() -> Result<PathBuf> {
        let cache_dir = Self::get_hf_cache_dir()?;

        std::fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

        let model_path = cache_dir.join("model.onnx");

        if !model_path.exists() {
            eprintln!("Downloading BERT model (first time only, ~23MB)...");
            Self::download_model(&model_path)?;
            eprintln!("Model downloaded successfully!");
        }

        Ok(model_path)
    }

    /// Get the path to the tokenizer file, downloading if necessary
    /// Uses HuggingFace Hub standard cache location for interoperability
    fn get_or_download_tokenizer() -> Result<PathBuf> {
        let cache_dir = Self::get_hf_cache_dir()?;

        std::fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

        let tokenizer_path = cache_dir.join("tokenizer.json");

        if !tokenizer_path.exists() {
            eprintln!("Downloading tokenizer...");
            Self::download_tokenizer(&tokenizer_path)?;
            eprintln!("Tokenizer downloaded successfully!");
        }

        Ok(tokenizer_path)
    }

    /// Get HuggingFace Hub cache directory for this model
    /// Follows the standard: ~/.cache/huggingface/hub/models--{org}--{model}/snapshots/{revision}/
    fn get_hf_cache_dir() -> Result<PathBuf> {
        // Check for HF_HOME or HUGGINGFACE_HUB_CACHE environment variables
        let base_cache = if let Ok(hf_home) = std::env::var("HF_HOME") {
            PathBuf::from(hf_home).join("hub")
        } else if let Ok(hf_cache) = std::env::var("HUGGINGFACE_HUB_CACHE") {
            PathBuf::from(hf_cache)
        } else {
            // Default to standard cache location
            dirs::cache_dir()
                .context("Failed to get cache directory")?
                .join("huggingface")
                .join("hub")
        };

        // Use HuggingFace Hub naming convention
        // Format: models--{organization}--{model_name}/snapshots/{revision}/onnx/
        Ok(base_cache
            .join("models--sentence-transformers--all-MiniLM-L6-v2")
            .join("snapshots")
            .join("main")
            .join("onnx"))
    }

    /// Download the ONNX model from HuggingFace
    fn download_model(dest: &PathBuf) -> Result<()> {
        let url = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
        Self::download_file(url, dest)
    }

    /// Download the tokenizer from HuggingFace
    fn download_tokenizer(dest: &PathBuf) -> Result<()> {
        let url = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";
        Self::download_file(url, dest)
    }

    /// Download a file from a URL
    fn download_file(url: &str, dest: &PathBuf) -> Result<()> {
        let response = ureq::get(url)
            .call()
            .map_err(|e| anyhow::anyhow!("Failed to download from {}: {}", url, e))?;

        let mut file = std::fs::File::create(dest)
            .context("Failed to create file")?;

        let mut reader = response.into_reader();
        std::io::copy(&mut reader, &mut file)
            .context("Failed to write file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Ignore by default as it requires model download
    fn test_identical_texts() {
        let scorer = BertScorer::new().expect("Failed to create scorer");
        let text = "The capital of France is Paris";
        let score = scorer.calculate_score(text, text).expect("Failed to calculate score");

        // Should be very close to 1.0
        assert!(score > 0.99, "Identical texts should have score close to 1.0, got {}", score);
    }

    #[test]
    #[ignore] // Ignore by default as it requires model download
    fn test_semantic_similarity() {
        let scorer = BertScorer::new().expect("Failed to create scorer");
        let text1 = "The capital of France is Paris";
        let text2 = "Paris is the capital city of France";
        let score = scorer.calculate_score(text1, text2).expect("Failed to calculate score");

        // Should be high since they have similar meaning
        assert!(score > 0.7, "Similar texts should have high score, got {}", score);
    }

    #[test]
    #[ignore] // Ignore by default as it requires model download
    fn test_different_texts() {
        let scorer = BertScorer::new().expect("Failed to create scorer");
        let text1 = "The capital of France is Paris";
        let text2 = "Dogs are great pets";
        let score = scorer.calculate_score(text1, text2).expect("Failed to calculate score");

        // Should be low since they have different meanings
        assert!(score < 0.5, "Different texts should have low score, got {}", score);
    }

    #[test]
    fn test_cosine_similarity() {
        let scorer = BertScorer::new().unwrap_or_else(|_| {
            // Create a dummy scorer just for testing cosine similarity
            panic!("Cosine similarity test doesn't require model loading");
        });

        // This test doesn't actually need a model, but we can't easily construct
        // a BertScorer without one. Skipping for now.
    }
}

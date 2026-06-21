//! A generic OpenAI-compatible `Embedder` (D-M3-1): produces embedding vectors via the
//! `/v1/embeddings` protocol, with `nomic-embed-text` (Ollama) as the local default preset.
//!
//! Embeddings feed `being-core-memory`'s `SemanticIndex`. Backend specifics (base URL, model) live
//! in [`EmbedConfig`]; the impl is generic over any OpenAI-compatible embeddings endpoint.
//!
//! **Memory discipline (16 GB).** Compiling loads nothing. Only `embed()` reaches the backend (the
//! `nomic-embed-text` model is ~0.3 GB but is still only ever called foreground/runtime, never in
//! `cargo test`/hooks). The unit tests below run against fixed JSON; the live call is behind the
//! off-by-default `live-model` feature.

use std::time::Duration;

use being_core_memory::Embedder;
use serde_json::{json, Value};

/// Where and how to call an OpenAI-compatible embeddings backend.
#[derive(Clone, Debug)]
pub struct EmbedConfig {
    pub base_url: String,
    pub model: String,
}

impl EmbedConfig {
    /// The local default: Ollama serving `nomic-embed-text` (the shared embedding, CLAUDE.md).
    pub fn nomic() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "nomic-embed-text".to_string(),
        }
    }
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self::nomic()
    }
}

/// An `Embedder` over any OpenAI-compatible embeddings backend.
pub struct OpenAiEmbedder {
    config: EmbedConfig,
    agent: ureq::Agent,
}

impl OpenAiEmbedder {
    pub fn new(config: EmbedConfig) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(60))
            .build();
        Self { config, agent }
    }

    /// Convenience: the local `nomic-embed-text` preset.
    pub fn nomic() -> Self {
        Self::new(EmbedConfig::nomic())
    }

    /// Build the OpenAI-compatible embeddings request body (pure; unit-tested without a network).
    pub fn build_embed_request(&self, text: &str) -> String {
        json!({ "model": self.config.model, "input": text }).to_string()
    }
}

impl Embedder for OpenAiEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let url = format!("{}/v1/embeddings", self.config.base_url);
        let resp = self
            .agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&self.build_embed_request(text))
            .map_err(|e| format!("embeddings request failed: {e}"))?;
        let body = resp
            .into_string()
            .map_err(|e| format!("reading embeddings response failed: {e}"))?;
        parse_embedding_response(&body)
    }
}

/// Extract `data[0].embedding` from an OpenAI-compatible embeddings response. Pure; unit-tested.
pub fn parse_embedding_response(body: &str) -> Result<Vec<f32>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    let arr = v["data"][0]["embedding"]
        .as_array()
        .ok_or_else(|| "no data[0].embedding in response".to_string())?;
    let out: Vec<f32> = arr
        .iter()
        .map(|x| x.as_f64().map(|f| f as f32))
        .collect::<Option<Vec<f32>>>()
        .ok_or_else(|| "non-numeric value in embedding".to_string())?;
    if out.is_empty() {
        return Err("empty embedding".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_includes_model_and_input() {
        let e = OpenAiEmbedder::nomic();
        let body = e.build_embed_request("hello world");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], "nomic-embed-text");
        assert_eq!(v["input"], "hello world");
    }

    #[test]
    fn parse_extracts_embedding_vector() {
        let body = r#"{"object":"list","data":[{"embedding":[0.1,0.2,-0.3],"index":0}]}"#;
        let v = parse_embedding_response(body).unwrap();
        assert_eq!(v.len(), 3);
        assert!((v[0] - 0.1).abs() < 1e-6);
        assert!((v[2] + 0.3).abs() < 1e-6);
    }

    #[test]
    fn parse_errors_on_malformed_or_empty() {
        assert!(parse_embedding_response("not json").is_err());
        assert!(parse_embedding_response(r#"{"data":[]}"#).is_err());
        assert!(parse_embedding_response(r#"{"data":[{"embedding":[]}]}"#).is_err());
        // missing data key entirely
        assert!(parse_embedding_response(r#"{"object":"list"}"#).is_err());
        // a non-numeric element makes the whole vector invalid (the Option-collect failure branch)
        assert!(parse_embedding_response(r#"{"data":[{"embedding":["x",0.2]}]}"#).is_err());
    }

    #[test]
    fn parse_accepts_json_integer_components() {
        // Some backends emit whole numbers without a decimal point; as_f64 must still accept them.
        let v = parse_embedding_response(r#"{"data":[{"embedding":[1,2,-3]}]}"#).unwrap();
        assert_eq!(v, vec![1.0, 2.0, -3.0]);
    }

    /// Live call — loads `nomic-embed-text`. Foreground/manual only:
    ///   cargo test -p being-embed-openai --features live-model
    #[cfg(feature = "live-model")]
    #[test]
    fn live_nomic_embeds() {
        let e = OpenAiEmbedder::nomic();
        let v = e
            .embed("the capital of France is Paris")
            .expect("live embedding failed");
        assert!(
            v.len() >= 256,
            "expected a real embedding vector, got len {}",
            v.len()
        );
    }
}

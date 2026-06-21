//! A generic OpenAI-compatible chat `Proposer` (build-spec §3.4; D2).
//!
//! The model is the swappable tier — the being depends on the `Proposer` trait, never on a backend.
//! This impl speaks the OpenAI-compatible `/v1/chat/completions` protocol, which Ollama, vLLM,
//! llama.cpp's server, LM Studio, etc. all implement. The only backend-specific things
//! (base URL, model tag, qwen3's `/no_think` quirk) live in [`OpenAiChatConfig`], not in the type —
//! so switching backends is a config change, and [`OpenAiChatConfig::ollama_qwen3`] is just the
//! local default preset.
//!
//! **Memory discipline (16 GB budget).** Compiling this crate loads nothing. Only a `propose()` call
//! reaches the backend and loads the model. The automated loop never does that: the unit tests below
//! run against fixed strings with no network, and the single live call is behind the off-by-default
//! `live-model` feature, foreground/operator-run only.

use std::time::Duration;

use being_runtime::{ContextPack, PlanStep, Proposal, Proposer};
use serde_json::{json, Value};

/// Where and how to call an OpenAI-compatible chat backend.
#[derive(Clone, Debug)]
pub struct OpenAiChatConfig {
    pub base_url: String,
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: u32,
    /// Sampling temperature. 0.0 = greedy/deterministic — the default, so the falsification bench
    /// measures capability rather than sampling noise. (qwen3 thinking mode wants ~0.6, not greedy.)
    pub temperature: f32,
    /// Nucleus sampling. qwen3 thinking-mode guidance: 0.95.
    pub top_p: f32,
    /// Top-k sampling (Ollama-native; ignored by strict OpenAI endpoints). qwen3 thinking: 20.
    pub top_k: u32,
    /// Optional prefix prepended to the user message (e.g. qwen3's `"/no_think\n"`). Empty = none.
    pub user_prefix: String,
}

impl OpenAiChatConfig {
    /// The local default backend: Ollama serving `qwen3:8b` (CLAUDE.md). Backend-specific only here.
    pub fn ollama_qwen3() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            model: "qwen3:8b".to_string(),
            system_prompt: "You are Yogi, a concise, helpful assistant. Answer directly."
                .to_string(),
            max_tokens: 256,
            temperature: 0.0,
            top_p: 0.95,
            top_k: 20,
            user_prefix: "/no_think\n".to_string(),
        }
    }
}

impl Default for OpenAiChatConfig {
    fn default() -> Self {
        Self::ollama_qwen3()
    }
}

/// A `Proposer` over any OpenAI-compatible chat backend. Holds a `ureq` agent with bounded timeouts.
pub struct OpenAiChatProposer {
    config: OpenAiChatConfig,
    agent: ureq::Agent,
}

impl OpenAiChatProposer {
    pub fn new(config: OpenAiChatConfig) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(180))
            .build();
        Self { config, agent }
    }

    /// Convenience: the local Ollama/qwen3 preset.
    pub fn ollama_qwen3() -> Self {
        Self::new(OpenAiChatConfig::ollama_qwen3())
    }

    /// Build the OpenAI-compatible chat request body (pure; unit-tested without a network).
    pub fn build_chat_request(&self, ctx: &ContextPack) -> String {
        let mut user = String::new();
        user.push_str(&self.config.user_prefix);
        for r in &ctx.retrieved {
            user.push_str("[memory] ");
            user.push_str(r);
            user.push('\n');
        }
        user.push_str(&ctx.input);

        json!({
            "model": self.config.model,
            "stream": false,
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "top_p": self.config.top_p,
            "top_k": self.config.top_k,
            "messages": [
                {"role": "system", "content": self.config.system_prompt},
                {"role": "user", "content": user},
            ],
        })
        .to_string()
    }

    /// Perform the live call. Reaches the backend and loads the model — foreground/manual only.
    pub fn try_propose(&self, ctx: &ContextPack) -> Result<String, String> {
        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let resp = self
            .agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_string(&self.build_chat_request(ctx))
            .map_err(|e| format!("backend request failed: {e}"))?;
        let body = resp
            .into_string()
            .map_err(|e| format!("reading backend response failed: {e}"))?;
        parse_chat_response(&body)
    }
}

impl Proposer for OpenAiChatProposer {
    /// Infallible per the trait: a failed call yields an `error` step rather than panicking, so the
    /// turn (and the committer's gate) still runs.
    fn propose(&mut self, ctx: &ContextPack) -> Proposal {
        match self.try_propose(ctx) {
            Ok(text) => Proposal {
                intent: "respond".to_string(),
                est_cost: text.len() as i64,
                candidate_steps: vec![PlanStep {
                    action: "respond".to_string(),
                    arg: text,
                }],
                preferred: 0,
            },
            Err(e) => Proposal {
                intent: "proposer-error".to_string(),
                candidate_steps: vec![PlanStep {
                    action: "error".to_string(),
                    arg: e,
                }],
                preferred: 0,
                est_cost: 1,
            },
        }
    }
}

/// Extract `choices[0].message.content` from an OpenAI-compatible response, stripping a
/// `<think>…</think>` reasoning block if present. Pure; unit-tested.
pub fn parse_chat_response(body: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "no choices[0].message.content in response".to_string())?;
    Ok(strip_think(content).trim().to_string())
}

/// Remove a `<think>…</think>` block (harmless no-op for backends/models that don't emit one).
pub fn strip_think(s: &str) -> String {
    match (s.find("<think>"), s.find("</think>")) {
        (Some(a), Some(b)) if b > a => {
            let mut out = String::with_capacity(s.len());
            out.push_str(&s[..a]);
            out.push_str(&s[b + "</think>".len()..]);
            out
        }
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(input: &str) -> ContextPack {
        ContextPack {
            input: input.to_string(),
            retrieved: vec!["yogi prefers brevity".to_string()],
        }
    }

    #[test]
    fn request_includes_model_input_context_and_no_stream() {
        let p = OpenAiChatProposer::ollama_qwen3();
        let body = p.build_chat_request(&ctx("what is 2+2?"));
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], "qwen3:8b");
        assert_eq!(v["stream"], false);
        let user = v["messages"][1]["content"].as_str().unwrap();
        assert!(user.contains("what is 2+2?"));
        assert!(user.contains("/no_think")); // qwen3 preset prefix
        assert!(user.contains("[memory] yogi prefers brevity"));
    }

    #[test]
    fn generic_config_without_prefix_omits_it() {
        let cfg = OpenAiChatConfig {
            base_url: "http://example".into(),
            model: "some-other-model".into(),
            system_prompt: "sys".into(),
            max_tokens: 64,
            temperature: 0.0,
            top_p: 0.95,
            top_k: 20,
            user_prefix: String::new(),
        };
        let p = OpenAiChatProposer::new(cfg);
        let body = p.build_chat_request(&ctx("hello"));
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["model"], "some-other-model");
        let user = v["messages"][1]["content"].as_str().unwrap();
        assert!(!user.contains("/no_think")); // generic backend: no qwen3 quirk
        assert!(user.contains("hello"));
    }

    #[test]
    fn parse_extracts_content() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"4"}}]}"#;
        assert_eq!(parse_chat_response(body).unwrap(), "4");
    }

    #[test]
    fn parse_strips_think_block() {
        let body = r#"{"choices":[{"message":{"content":"<think>reasoning</think> 4"}}]}"#;
        assert_eq!(parse_chat_response(body).unwrap(), "4");
    }

    #[test]
    fn strip_think_handles_no_block() {
        assert_eq!(strip_think("just an answer"), "just an answer");
    }

    #[test]
    fn parse_errors_on_malformed_or_empty() {
        assert!(parse_chat_response("not json").is_err());
        assert!(parse_chat_response(r#"{"choices":[]}"#).is_err());
    }

    /// Live call — loads the model. Foreground/manual only:
    ///   cargo test -p being-proposer-openai --features live-model
    #[cfg(feature = "live-model")]
    #[test]
    fn live_backend_responds() {
        let mut p = OpenAiChatProposer::ollama_qwen3();
        let proposal = p.propose(&ContextPack {
            input: "Reply with exactly the word: hi".to_string(),
            retrieved: vec![],
        });
        assert_eq!(
            proposal.intent, "respond",
            "expected a live response, got: {}",
            proposal.candidate_steps[0].arg
        );
        assert!(!proposal.candidate_steps[0].arg.is_empty());
        eprintln!("model said: {}", proposal.candidate_steps[0].arg);
    }
}

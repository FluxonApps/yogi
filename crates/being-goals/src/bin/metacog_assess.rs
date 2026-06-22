//! AWARENESS pass (foreground): probe the REAL agent (qwen3:8b via Ollama) on the ⊕ goal, COLD (no
//! rule) vs TAUGHT (rule in-context), grade with the FREE verifier, and build a verifier-grounded
//! capability map. Output = the agent's self-knowledge (mastered / frontier / beyond) and what it
//! should practice next. This is the "awareness" in awareness+practice+loop — the driver that makes
//! the ratchet self-directed. Model call = foreground only.
use being_goals::op;
use being_metacog::{CapabilityMap, NextAction};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::ContextPack;

/// Strip qwen3 `<think>…</think>`; keep what follows the last close tag.
fn strip_think(s: &str) -> String {
    s.rsplit("</think>").next().unwrap_or(s).trim().to_string()
}

fn ask(p: &mut OpenAiChatProposer, prompt: &str) -> String {
    for _ in 0..3 {
        let raw = p
            .try_propose(&ContextPack {
                input: prompt.to_string(),
                retrieved: Vec::new(),
            })
            .unwrap_or_default();
        let out = strip_think(&raw);
        if !out.trim().is_empty() {
            return out;
        }
    }
    String::new()
}

fn main() {
    let mut p = OpenAiChatProposer::new(OpenAiChatConfig::ollama_qwen3_thinking());
    let mut map: CapabilityMap<String> = CapabilityMap::new();
    eprintln!("AWARENESS: probing qwen3:8b on the operator goal (cold vs taught) — verifier-grounded...");
    for (a, b) in op::test_pairs() {
        let cold = op::verify(a, b, &ask(&mut p, &op::cold_prompt(a, b)));
        let taught = op::verify(a, b, &ask(&mut p, &op::taught_prompt(a, b)));
        map.record(format!("{a} op {b}"), cold, taught);
        println!("  {a} op {b}: cold={cold} taught={taught}");
    }
    let (m, f, b) = map.summary();
    println!(
        "\ncapability map: mastered={m} frontier={f} beyond={b}  floor={:.0}%",
        map.floor() * 100.0
    );
    match map.next_action() {
        NextAction::PracticeFrontier(n) => println!(
            "AWARE → {n} items are learnable NOW (fail cold, pass taught = the ZPD): practice them, then distill."
        ),
        NextAction::NeedScaffolding(n) => {
            println!("AWARE → {n} items are beyond reach: need scaffolding/teacher before practicing.")
        }
        NextAction::AllMastered => println!("AWARE → everything probed is already mastered."),
    }
}

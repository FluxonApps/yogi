//! Foreground multi-skill + compositional transfer cert (loads qwen3:8b + nomic). Run manually:
//!   cargo run -p being-bench --bin multi_skill --release
//!
//! Extends the single-skill cert (D-M3-3): a thinking-mode being learns THREE distinct made-up
//! operations (⊕,⊗,⊙) via `learn_skill`, then we certify:
//!   (a) single-op transfer — apply each learned rule to new operands;
//!   (b) the COMPOSITIONAL held-out split — `(a⊕b)⊗c`, two rules never seen together (SCAN/MCD);
//!   (c) LiMem — re-score with perturbed operands; low LiMem ⇒ it's applying rules, not memorizing.
//! Cold (no skills) is the paired baseline. Foreground.

use std::sync::Arc;

use being_bench::{limem, mean, multi_skill_corpus, paired_bootstrap_ci, TransferTask};
use being_core_economy::Account;
use being_embed_openai::OpenAiEmbedder;
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Score tasks with or without the learned skills (fresh thinking-mode being per task).
fn score(tasks: &[TransferTask], skills: &[String], with_skills: bool) -> Vec<f64> {
    tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
            let mut cfg = OpenAiChatConfig::ollama_qwen3_thinking();
            cfg.system_prompt =
                "You are a careful calculator. Reason step by step, then end with a line exactly \
                 like 'ANSWER: <number>'."
                    .to_string();
            let mut being = Being::from_seed(
                [1u8; 32],
                Supervisor::as_port(&sup),
                OpenAiChatProposer::new(cfg),
                PassThroughCommitter,
                EchoExecutor,
            )
            .with_embedder(Arc::new(OpenAiEmbedder::nomic()));
            if with_skills {
                for s in skills {
                    being.learn_skill(s, true, 0);
                }
            }
            let turn = being.turn(&t.prompt, (i + 1) as i64);
            if turn.observations.join(" ").contains(&t.expected) {
                1.0
            } else {
                0.0
            }
        })
        .collect()
}

fn report(label: &str, cold: &[f64], skilled: &[f64]) {
    let ci = paired_bootstrap_ci(cold, skilled, 4000, 12345, 0.05);
    println!(
        "{label}: cold {:.3} -> skilled {:.3} | delta {:.3} CI=[{:.3},{:.3}] compounds={}",
        mean(cold),
        mean(skilled),
        ci.mean_delta,
        ci.lower,
        ci.upper,
        ci.improves_monotonically()
    );
}

fn main() {
    let n = 3;
    let corpus = multi_skill_corpus(n, 7);
    let pert = multi_skill_corpus(n, 99); // same structure, perturbed operands (for LiMem)
    eprintln!(
        "Multi-skill cert: {} single-op + {} compositional tasks, 3 learned skills, foreground ...",
        corpus.single.len(),
        corpus.compositional.len()
    );

    let cold_s = score(&corpus.single, &corpus.skills, false);
    let skilled_s = score(&corpus.single, &corpus.skills, true);
    report("single-op transfer", &cold_s, &skilled_s);

    let cold_c = score(&corpus.compositional, &corpus.skills, false);
    let skilled_c = score(&corpus.compositional, &corpus.skills, true);
    report("compositional split", &cold_c, &skilled_c);

    // LiMem: re-score the skilled being on perturbed operands; low LiMem = applying rules, not memorizing.
    let skilled_pert = score(&pert.single, &pert.skills, true);
    let orig: Vec<bool> = skilled_s.iter().map(|&x| x > 0.5).collect();
    let perturbed: Vec<bool> = skilled_pert.iter().map(|&x| x > 0.5).collect();
    println!(
        "LiMem (single, skilled): {:.3}  (0 = pure rule-application, high = memorization)",
        limem(&orig, &perturbed)
    );
}

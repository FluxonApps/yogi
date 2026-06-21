//! Foreground transfer-compounding certification (loads qwen3:8b + nomic-embed-text). Run manually:
//!   cargo run -p being-bench --bin transfer --release
//!
//! Measures TRANSFER, not answer-lookup (D-M3-3): a made-up operation `a ⊕ b = a·b+a+b` the model
//! can't know cold, on fresh seeded operands. Cold (no skill) → fails; with the learned RULE skill
//! retrieved into context → applies it to the new operands → passes. The answer is never stored, so
//! a pass is genuine transfer. Each task uses a fresh being (the skill is the only difference).

use std::sync::Arc;

use being_bench::{mean, paired_bootstrap_ci, transfer_corpus, TransferTask, TRANSFER_RULE};
use being_core_economy::Account;
use being_embed_openai::OpenAiEmbedder;
use being_proposer_openai::OpenAiChatProposer;
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Score the corpus with or without the learned rule-skill. Fresh being per task so the only
/// difference is whether the skill is present (isolates the transfer effect).
fn score_transfer(corpus: &[TransferTask], with_skill: bool) -> Vec<f64> {
    corpus
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
            let mut being = Being::from_seed(
                [1u8; 32],
                Supervisor::as_port(&sup),
                OpenAiChatProposer::ollama_qwen3(),
                PassThroughCommitter,
                EchoExecutor,
            )
            .with_embedder(Arc::new(OpenAiEmbedder::nomic()));
            if with_skill {
                being.learn_skill(TRANSFER_RULE, true, 0);
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

fn main() {
    let n = 20;
    let corpus = transfer_corpus(n, 7);
    eprintln!("Transfer-compounding certification: {n} cold-failing ⊕-tasks, foreground (qwen3:8b + nomic) ...");

    let cold = score_transfer(&corpus, false);
    let skilled = score_transfer(&corpus, true);

    println!("cold (no skill)     mean: {:.3}", mean(&cold));
    println!("with learned skill  mean: {:.3}", mean(&skilled));
    let ci = paired_bootstrap_ci(&cold, &skilled, 4000, 12345, 0.05);
    println!(
        "paired delta: mean={:.3}  CI=[{:.3}, {:.3}]  compounds={}",
        ci.mean_delta,
        ci.lower,
        ci.upper,
        ci.improves_monotonically()
    );
    if ci.improves_monotonically() {
        println!("=> CERTIFIED: the skill transfers to new operands (CI excludes zero). Token-space compounding.");
    } else {
        println!("=> not certified at this N — widen the corpus (D-M3-3 sizing).");
    }
}

//! End-to-end transfer-compounding cert (loads qwen3:8b + nomic-embed-text). Run manually:
//!   cargo run -p being-bench --bin transfer_e2e --release
//!
//! Unlike `transfer` (which injects the rule deterministically to isolate the APPLY mechanism), this
//! exercises the FULL being: `learn_skill` stores the rule, the turn retrieves it via the **hybrid**
//! index (so the rare `⊕` is found lexically), and the **thinking** proposer applies it. It certifies
//! the whole retrieve→apply loop self-certifies on cold-failing transfer tasks. Foreground.

use std::sync::Arc;

use being_bench::{mean, paired_bootstrap_ci, transfer_corpus, TransferTask, TRANSFER_SKILL_NOTE};
use being_core_economy::Account;
use being_embed_openai::OpenAiEmbedder;
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Fresh being per task (the skill is the only difference). Thinking proposer + hybrid-retrieval index.
fn score_e2e(corpus: &[TransferTask], with_skill: bool) -> Vec<f64> {
    corpus
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
            if with_skill {
                being.learn_skill(TRANSFER_SKILL_NOTE, true, 0); // stored; retrieved via hybrid next turn
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
    let n = 10;
    let corpus = transfer_corpus(n, 7);
    eprintln!(
        "Transfer cert E2E (full being: hybrid retrieval + thinking + learn_skill): {n} tasks, foreground ..."
    );

    let cold = score_e2e(&corpus, false);
    let skilled = score_e2e(&corpus, true);

    println!("cold (no skill)    mean: {:.3}", mean(&cold));
    println!("with learned skill mean: {:.3}", mean(&skilled));
    let ci = paired_bootstrap_ci(&cold, &skilled, 4000, 12345, 0.05);
    println!(
        "paired delta: mean={:.3}  CI=[{:.3}, {:.3}]  compounds={}",
        ci.mean_delta,
        ci.lower,
        ci.upper,
        ci.improves_monotonically()
    );
    if ci.improves_monotonically() {
        println!("=> CERTIFIED end-to-end: the full retrieve→apply loop self-certifies. Token-space compounding.");
    } else {
        println!("=> not certified end-to-end — retrieval is likely the gap; inspect the hybrid hit on ⊕.");
    }
}

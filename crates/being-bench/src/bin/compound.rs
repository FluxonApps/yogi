//! Foreground Day-0 vs Day-N compounding demo (loads qwen3:8b + nomic-embed-text). Run manually:
//!   cargo run -p being-bench --bin compound --release
//!
//! This is the first place the bench's compounding signal can actually appear: does the *same model*
//! score higher once memory has accumulated (Day-N) than cold (Day-0)? The automated loop never runs
//! binaries, so this never fires from a hook or `cargo test`.

use std::sync::Arc;

use being_bench::{default_frozen_suite, mean, paired_bootstrap_ci, score_response};
use being_core_economy::Account;
use being_embed_openai::OpenAiEmbedder;
use being_proposer_openai::OpenAiChatProposer;
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Score the frozen suite with a live being. With `with_memory`, attach the embedder and run a
/// "study" pass first so memory accumulates (Day-N); without it, score cold (Day-0).
fn score_run(seed: [u8; 32], with_memory: bool) -> Vec<f64> {
    let suite = default_frozen_suite();
    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
    let mut being = Being::from_seed(
        seed,
        Supervisor::as_port(&sup),
        OpenAiChatProposer::ollama_qwen3(),
        PassThroughCommitter,
        EchoExecutor,
    );
    if with_memory {
        being = being.with_embedder(Arc::new(OpenAiEmbedder::nomic()));
        for (i, t) in suite.iter().enumerate() {
            // study pass: see each task alongside its answer once, so memory accumulates
            being.turn(&format!("{} (answer: {})", t.prompt, t.expected), i as i64);
        }
    }
    suite
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let turn = being.turn(&t.prompt, 1_000 + i as i64);
            score_response(t, &turn.observations.join(" "))
        })
        .collect()
}

fn main() {
    eprintln!(
        "Day-0 vs Day-N compounding demo (foreground — loads qwen3:8b + nomic-embed-text) ..."
    );
    let day0 = score_run([1u8; 32], false);
    let day_n = score_run([1u8; 32], true);
    println!("Day-0 mean score: {:.3}", mean(&day0));
    println!("Day-N mean score: {:.3}", mean(&day_n));
    let ci = paired_bootstrap_ci(&day0, &day_n, 2_000, 12_345, 0.05);
    println!(
        "paired delta: mean={:.3}  CI=[{:.3}, {:.3}]  compounds={}",
        ci.mean_delta,
        ci.lower,
        ci.upper,
        ci.improves_monotonically()
    );
    println!(
        "(One run with N=5 tasks is illustrative, not the falsification. The real gate is the \
         derived replication count on a frozen, provenance-isolated suite — build-spec §7.)"
    );
}

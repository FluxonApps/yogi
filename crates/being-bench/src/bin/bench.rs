//! Foreground bench runner. Scores a live being against the frozen suite using qwen3:8b.
//!
//! THIS LOADS THE MODEL (~5–6 GB). Run it yourself, foreground only:
//!   cargo run -p being-bench --bin bench --release
//! The automated loop never runs binaries, so this never fires from a hook or `cargo test`.

use being_bench::{default_frozen_suite, mean, score_response};
use being_core_economy::Account;
use being_proposer_openai::OpenAiChatProposer;
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

fn main() {
    let suite = default_frozen_suite();

    // Well-funded supervisor (no watchdog timeout) so a bench run isn't reaped mid-suite.
    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
    let mut being = Being::from_seed(
        [1u8; 32],
        Supervisor::as_port(&sup),
        OpenAiChatProposer::ollama_qwen3(),
        PassThroughCommitter,
        EchoExecutor,
    );

    eprintln!(
        "being-bench: scoring {} frozen tasks against qwen3:8b (foreground — loads ~5-6 GB) ...",
        suite.len()
    );

    let mut scores = Vec::with_capacity(suite.len());
    for (i, task) in suite.iter().enumerate() {
        let turn = being.turn(&task.prompt, i as i64);
        let response = turn.observations.join(" ");
        let s = score_response(task, &response);
        scores.push(s);
        let preview: String = response.chars().take(80).collect();
        println!("[{}] score {s}  ::  {preview}", task.id);
    }

    println!("\nDay-0 mean score: {:.3}", mean(&scores));
    println!(
        "(Baseline only. The Day-N + anti-theater comparison via paired_bootstrap_ci needs learning \
         to accrue — exercised at M3. Nothing compounds yet, by design.)"
    );
}

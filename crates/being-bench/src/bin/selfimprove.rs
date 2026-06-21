//! Foreground bounded self-modification demo (loads qwen3:8b repeatedly). Run manually:
//!   cargo run -p being-bench --bin selfimprove --release
//!
//! Each round: the Improver picks a candidate system-prompt mutation; the being is benched under the
//! incumbent vs. candidate genome; the Two-Gate accepts only a real, capacity-bounded improvement,
//! else rolls back. The bench is the only verifier (no LLM judge). The acceptance machinery is pure
//! (being-loop, tested); only the scorer here loads the model, foreground.

use being_bench::{default_frozen_suite, score_response};
use being_core_economy::Account;
use being_core_mutation::{Genome, MutationKind};
use being_loop::{
    self_improve_round, AuditLog, CapacityCaps, EpsilonGreedyImprover, TwoGate, ValidationGate,
};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Grade a genome: build a being whose system prompt is the genome's prompt, run the frozen suite,
/// return per-case pass/fail. (Loads the model — foreground.)
fn score_genome(genome: &Genome) -> Vec<bool> {
    let suite = default_frozen_suite();
    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
    let mut cfg = OpenAiChatConfig::ollama_qwen3();
    if !genome.prompt.is_empty() {
        cfg.system_prompt = genome.prompt.clone();
    }
    let mut being = Being::from_seed(
        [1u8; 32],
        Supervisor::as_port(&sup),
        OpenAiChatProposer::new(cfg),
        PassThroughCommitter,
        EchoExecutor,
    );
    suite
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let turn = being.turn(&t.prompt, i as i64);
            score_response(t, &turn.observations.join(" ")) > 0.5
        })
        .collect()
}

fn main() {
    eprintln!("Self-improve demo (foreground — loads qwen3:8b repeatedly) ...");
    let gate = TwoGate {
        validation: ValidationGate::conservative(),
        caps: CapacityCaps::conservative(),
    };
    let mut improver = EpsilonGreedyImprover::new(0.1, 7);
    let mut audit = AuditLog::new();
    let arms = [
        MutationKind::Prompt("You are Yogi. Reply with ONLY the answer, nothing else.".into()),
        MutationKind::Prompt(
            "You are a careful assistant. Give the single best short answer.".into(),
        ),
    ];

    let mut genome = Genome::default();
    let mut scorer = score_genome;
    for round in 1..=3 {
        let before = genome.prompt.clone();
        genome = self_improve_round(
            &genome,
            &arms,
            &mut scorer,
            &gate,
            &mut improver,
            &mut audit,
        );
        println!("round {round}: prompt {before:?} -> {:?}", genome.prompt);
    }

    println!("\naudit trail:");
    for e in audit.entries() {
        println!(
            "  accepted={} delta={:.3}  {}",
            e.accepted, e.delta, e.summary
        );
    }
    println!(
        "\n(Illustrative with N=5 tasks; the Validation Gate will reject noise-level gains by design \
         — a real run needs the derived replication count, build-spec §7.)"
    );
}

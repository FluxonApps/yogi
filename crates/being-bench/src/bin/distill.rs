//! Foreground M3 gap detection (loads qwen3:8b twice). Run manually:
//!   cargo run -p being-bench --bin distill --release
//!
//! Computes the live `(teacher-success ∩ student-weak)` set — the tasks distillation would target.
//! Here the "student" is qwen3 in `/no_think` (fast, weak on reasoning) and the "teacher" is the same
//! model in thinking mode (the certified-stronger config, FINDINGS 2026-06-21). The gap is exactly the
//! reasoning tasks the no-think student drops — empirically re-confirming "never /no_think a reasoning
//! task" and producing the input set a distilled student should be trained to close. The promotion
//! gate ([`being_distill::PromotionGate`]) then decides whether a retrained student may be promoted.

use being_bench::{default_frozen_suite, score_response};
use being_core_economy::Account;
use being_distill::gap_set;
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Run the frozen suite under `cfg`, returning per-task pass/fail. Loads the model — foreground only.
fn run_suite(cfg: OpenAiChatConfig) -> Vec<bool> {
    let suite = default_frozen_suite();
    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
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
            score_response(t, &being.turn(&t.prompt, i as i64).observations.join(" ")) > 0.5
        })
        .collect()
}

fn main() {
    eprintln!("M3 gap detection (foreground — loads qwen3:8b twice: no-think student, thinking teacher) ...");
    let suite = default_frozen_suite();

    let student = run_suite(OpenAiChatConfig::ollama_qwen3()); // /no_think — the weak student
    let teacher = run_suite(OpenAiChatConfig::ollama_qwen3_thinking()); // thinking — the teacher

    let s_rate = student.iter().filter(|p| **p).count() as f64 / student.len().max(1) as f64;
    let t_rate = teacher.iter().filter(|p| **p).count() as f64 / teacher.len().max(1) as f64;
    let gap = gap_set(&teacher, &student);

    println!("\nstudent (/no_think) pass-rate = {s_rate:.2}   teacher (thinking) pass-rate = {t_rate:.2}");
    println!(
        "gap (teacher-success ∩ student-weak) = {} tasks — the distillation target set:",
        gap.len()
    );
    for &i in &gap {
        println!("  [{}] {}", suite[i].id, suite[i].prompt);
    }
    if gap.is_empty() {
        println!("  (none — the no-think student already matches the teacher on this suite)");
    }
    println!(
        "\n(A distilled student should close this gap WITHOUT regressing the broad set — \
         being_distill::PromotionGate enforces both clauses.)"
    );
}

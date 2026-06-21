//! Foreground M3 distillation flywheel — close a real gap end-to-end (loads qwen3:8b, thinking mode):
//!   cargo run -p being-bench --bin distill_close --release
//!
//! Ties together everything: gap-detect → distill the teacher's rule as a skill → re-evaluate on
//! FRESH operands → judge with `being_distill::PromotionGate`. The domain is the made-up op ⊕; the
//! mixed set is ⊗/⊙. Both arms run in THINKING mode so the only difference is the distilled ⊕ rule —
//! isolating distillation's effect. Fresh operands mean a pass is capability (rule application), not
//! memorization. Promotion requires closing the ⊕ gap WITHOUT regressing ⊗/⊙ (non-inferiority).

use being_bench::multi_skill_corpus;
use being_core_economy::Account;
use being_distill::{gap_set, PromotionGate};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Run the given single-op tasks under a thinking-mode being with `rules` prepended to the system
/// prompt; return per-task pass/fail. Loads the model — foreground only.
fn run(tasks: &[(String, String)], rules: &str) -> Vec<bool> {
    let mut cfg = OpenAiChatConfig::ollama_qwen3_thinking();
    cfg.system_prompt = format!(
        "You are a careful calculator. Apply ONLY the rules given. Reply with just the final number.\n{rules}"
    );
    cfg.max_tokens = 1024;
    let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
    let mut being = Being::from_seed(
        [1u8; 32],
        Supervisor::as_port(&sup),
        OpenAiChatProposer::new(cfg),
        PassThroughCommitter,
        EchoExecutor,
    );
    tasks
        .iter()
        .enumerate()
        .map(|(i, (p, exp))| being.turn(p, i as i64).observations.join(" ").contains(exp))
        .collect()
}

fn main() {
    eprintln!("M3 distillation flywheel (foreground — qwen3:8b thinking, cold vs distilled) ...");
    let seed = std::env::var("EVOLVE_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let corpus = multi_skill_corpus(2, seed); // 2 fresh tasks per op
    let oplus_rule = corpus.skills[0].clone(); // "Rule for ⊕: …"

    // Split the single-op tasks into the ⊕ DOMAIN and the ⊗/⊙ MIXED set (operations alternate ⊕,⊗,⊙).
    let mut domain = Vec::new(); // ⊕ tasks
    let mut mixed = Vec::new(); // ⊗,⊙ tasks
    for t in &corpus.single {
        let pair = (t.prompt.clone(), t.expected.clone());
        if t.prompt.contains('⊕') {
            domain.push(pair);
        } else {
            mixed.push(pair);
        }
    }

    // Teacher = has the rule → passes the domain (this is the success side of the gap).
    let teacher_domain = run(&domain, &oplus_rule);
    // Cold student = no rules → the weak side.
    let student_old_domain = run(&domain, "");
    let mixed_old = run(&mixed, "");
    // Distilled student = the ⊕ rule injected (the "teacher trace" distilled into a skill).
    let student_new_domain = run(&domain, &oplus_rule);
    let mixed_new = run(&mixed, &oplus_rule); // mixed has no ⊕; the rule shouldn't help OR hurt

    let gate = PromotionGate {
        gap_margin: 0.5,
        ni_epsilon: 0.1,
    };
    let v = gate.evaluate(
        &teacher_domain,
        &student_old_domain,
        &student_new_domain,
        &mixed_old,
        &mixed_new,
    );
    let gap = gap_set(&teacher_domain, &student_old_domain);

    let rate = |b: &[bool]| b.iter().filter(|x| **x).count() as f64 / b.len().max(1) as f64;
    println!(
        "\n⊕ domain: teacher={:.2}  cold-student={:.2}  distilled-student={:.2}  (gap size {})",
        rate(&teacher_domain),
        rate(&student_old_domain),
        rate(&student_new_domain),
        gap.len()
    );
    println!(
        "⊗/⊙ mixed: cold={:.2}  distilled={:.2}  (non-inferiority check)",
        rate(&mixed_old),
        rate(&mixed_new)
    );
    println!(
        "\nPromotionGate: gap_closure={:.2} (need ≥{:.2})  mixed_delta={:+.2} (need ≥ -{:.2})  → PROMOTED={}",
        v.gap_closure, gate.gap_margin, v.mixed_delta, gate.ni_epsilon, v.promoted
    );
    println!(
        "{}",
        if v.promoted {
            "→ distillation closed the ⊕ gap on FRESH operands (capability, not memorization) without \
             regressing ⊗/⊙: the M3 flywheel works end-to-end, live."
        } else {
            "→ not promoted: the gate held (gap not closed and/or the mixed set regressed). Honest."
        }
    );
}

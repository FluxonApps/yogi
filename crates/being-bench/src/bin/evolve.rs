//! Foreground M6 MAP-Elites illumination over genomes (loads qwen3:8b repeatedly). Run manually:
//!   cargo run -p being-bench --bin evolve --release
//!
//! This is the real (model-scored) counterpart to `tests/m6_acceptance.rs`. The illumination engine
//! ([`being_lineage::illuminate`]) searches prompt-space: each candidate genome is benched on the
//! frozen suite, its **fitness** is the pass-rate and its **behavior** is mean response length (a
//! verbosity niche axis). The MAP-Elites archive keeps the best genome per niche, so the run
//! illuminates a *diversity* of working styles, not one hill-climb. The acceptance machinery is pure
//! and tested; only the [`Evaluator`] here loads the model, foreground (CLAUDE.md HARD RULE).

use being_bench::{default_frozen_suite, neutral_drift_gate, score_response};
use being_core_economy::Account;
use being_core_id::Ed25519Signer;
use being_core_mutation::{Genome, MutationKind};
use being_lineage::{
    illuminate, Archive, BehaviorDescriptor, Colony, Evaluation, Evaluator, IlluminationConfig,
    Retention, Rng, Variator,
};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Bench a genome: fitness = frozen-suite pass-rate; behavior = (passes in the first half, passes in
/// the second half of the suite). Two genomes that solve *different* tasks land in different niches —
/// the building-block axis MAP-Elites can actually illuminate (a length axis collapses on this terse
/// suite; see FINDINGS 2026-06-21). Loads the model — foreground only.
struct BenchEvaluator;
impl Evaluator for BenchEvaluator {
    fn evaluate(&mut self, genome: &Genome) -> Evaluation {
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
        let half = suite.len().div_ceil(2);
        let (mut p_first, mut p_second) = (0usize, 0usize);
        for (i, t) in suite.iter().enumerate() {
            let resp = being.turn(&t.prompt, i as i64).observations.join(" ");
            if score_response(t, &resp) > 0.5 {
                if i < half {
                    p_first += 1;
                } else {
                    p_second += 1;
                }
            }
        }
        let n = suite.len().max(1);
        Evaluation {
            fitness: (p_first + p_second) as f64 / n as f64,
            behavior: vec![p_first as f64, p_second as f64],
        }
    }
}

/// Vary a genome by appending one of a few style directives to its system prompt (closed surface).
struct PromptVariator;
impl Variator for PromptVariator {
    fn vary(&mut self, rng: &mut Rng, parent: &Genome) -> Vec<MutationKind> {
        const STYLES: [&str; 5] = [
            "Answer with ONLY the answer, nothing else.",
            "Be concise.",
            "Think step by step, then give the final answer.",
            "Reply in a single word when possible.",
            "Give a complete, well-formed sentence.",
        ];
        let base = if parent.prompt.is_empty() {
            "You are Yogi."
        } else {
            parent.prompt.as_str()
        };
        let style = STYLES[rng.below(STYLES.len())];
        vec![MutationKind::Prompt(format!("{base} {style}"))]
    }
}

fn main() {
    eprintln!("M6 illumination (foreground — loads qwen3:8b repeatedly) ...");
    // Building-block niches: (first-half passes, second-half passes). 7 tasks → 4 + 3 → 5×4 = 20 cells.
    let descriptor =
        BehaviorDescriptor::bounded([(0.0, 1.0, 5), (0.0, 1.0, 4)]).unwrap();
    let iterations = std::env::var("EVOLVE_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    // Optional sexual reproduction: EVOLVE_RECOMB=0.3 breeds ~30% of children from two parents.
    let recomb = std::env::var("EVOLVE_RECOMB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    // Run inside a Colony: every fork is colony-signed into the ledger and the full genealogy recorded.
    let cfg = IlluminationConfig::new(iterations, 42).with_recombination(recomb);
    let mut colony = Colony::new(descriptor.clone(), Ed25519Signer::from_seed([42; 32]), 1);
    let stats = colony.run(
        Genome::default(),
        &mut BenchEvaluator,
        &mut PromptVariator,
        &cfg,
    );
    let archive = &colony.archive;

    println!(
        "\nillumination: {} evaluations, {} archive improvements, {} recombinations, {} niches filled",
        stats.evaluations,
        stats.improvements,
        stats.recombinations,
        archive.len()
    );
    println!(
        "QD-score={:.3}  mean-fitness={:.3}  coverage={:.1}%",
        archive.qd_score(),
        archive.mean_fitness().unwrap_or(0.0),
        descriptor.coverage(archive).unwrap_or(0.0) * 100.0
    );
    println!(
        "signed fork ledger: {} committed forks · genealogy {} lineages (depth {}) · colony {}",
        colony.ledger.len(),
        colony.phylogeny.len(),
        colony.phylogeny.max_generation(),
        colony.did().0
    );
    println!("\nelites per verbosity niche (len-band -> fitness, gen, prompt):");
    for e in archive.elites() {
        let band = e.lineage.generation;
        println!(
            "  cell fitness={:.2} gen={band} :: {:?}",
            e.fitness,
            e.genome.prompt.chars().take(80).collect::<String>()
        );
    }
    if let Some(best) = archive.best() {
        println!(
            "\nglobal best: fitness={:.2} gen={} prompt={:?}",
            best.fitness, best.lineage.generation, best.genome.prompt
        );
    }
    // EVOLVE_DRIFT=1 runs the real M6 acceptance: replicate elitist (selection) vs neutral-drift
    // arms, model-scored, judged by neutral_drift_gate. Expensive (many model calls) — opt-in.
    if std::env::var("EVOLVE_DRIFT").as_deref() == Ok("1") {
        run_drift_acceptance(&descriptor, iterations);
    } else {
        println!(
            "\n(Set EVOLVE_DRIFT=1 for the replicate selection-vs-neutral-drift gate — the \
             build-spec §6 acceptance on real model scores. The gate already fires on synthetic data.)"
        );
    }
}

/// The M6 acceptance on real model scores: a few replicates of elitist selection vs the matched
/// neutral-drift control, each measured by archive mean-fitness, judged by the real drift gate.
fn run_drift_acceptance(descriptor: &BehaviorDescriptor, iterations: usize) {
    let replicates = std::env::var("EVOLVE_REPLICATES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    eprintln!("M6 drift acceptance: {replicates} replicates × 2 arms (model-scored) ...");

    let mut selection = Vec::new();
    let mut drift = Vec::new();
    for i in 0..replicates {
        let seed = 100 + i as u64 * 17;
        for (arm, out) in [
            (Retention::Elitist, &mut selection),
            (Retention::NeutralDrift, &mut drift),
        ] {
            let mut archive = Archive::new();
            let cfg = IlluminationConfig::new(iterations, seed).with_retention(arm);
            illuminate(
                &mut archive,
                descriptor,
                Genome::default(),
                1,
                &mut BenchEvaluator,
                &mut PromptVariator,
                &cfg,
                None,
            );
            out.push(archive.mean_fitness().unwrap_or(0.0));
        }
    }

    let report = neutral_drift_gate(&drift, &selection, 0.0, 2000, 7, 0.05);
    println!(
        "\nM6 ACCEPTANCE: selection mean={:.3} vs drift mean={:.3}  advantage CI=[{:.3},{:.3}]  fires={}",
        report.selection_mean, report.drift_mean, report.ci.lower, report.ci.upper, report.fires
    );
    println!(
        "{}",
        if report.fires {
            "→ selection beats neutral drift at power: real open-ended signal."
        } else {
            "→ no significant edge over drift: the honest breeding-program-not-evolution null."
        }
    );
}

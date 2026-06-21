//! Foreground M6 MAP-Elites illumination over genomes (loads qwen3:8b repeatedly). Run manually:
//!   cargo run -p being-bench --bin evolve --release
//!
//! This is the real (model-scored) counterpart to `tests/m6_acceptance.rs`. The illumination engine
//! ([`being_lineage::illuminate`]) searches prompt-space: each candidate genome is benched on the
//! frozen suite, its **fitness** is the pass-rate and its **behavior** is mean response length (a
//! verbosity niche axis). The MAP-Elites archive keeps the best genome per niche, so the run
//! illuminates a *diversity* of working styles, not one hill-climb. The acceptance machinery is pure
//! and tested; only the [`Evaluator`] here loads the model, foreground (CLAUDE.md HARD RULE).

use being_bench::{default_frozen_suite, score_response};
use being_core_economy::Account;
use being_core_mutation::{Genome, MutationKind};
use being_lineage::{
    illuminate, Archive, BehaviorDescriptor, Evaluation, Evaluator, IlluminationConfig, Rng,
    Variator,
};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

/// Bench a genome: fitness = frozen-suite pass-rate; behavior = mean response length (chars). Loads
/// the model — foreground only.
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
        let (mut passes, mut total_len) = (0usize, 0usize);
        for (i, t) in suite.iter().enumerate() {
            let resp = being.turn(&t.prompt, i as i64).observations.join(" ");
            total_len += resp.len();
            if score_response(t, &resp) > 0.5 {
                passes += 1;
            }
        }
        let n = suite.len().max(1);
        Evaluation {
            fitness: passes as f64 / n as f64,
            behavior: vec![(total_len / n) as f64],
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
    // Verbosity niches: 30 bands of 20 chars (0..600), bounded so coverage is a fraction.
    let descriptor = BehaviorDescriptor::bounded([(0.0, 20.0, 30)]).unwrap();
    let iterations = std::env::var("EVOLVE_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    // Optional sexual reproduction: EVOLVE_RECOMB=0.3 breeds ~30% of children from two parents.
    let recomb = std::env::var("EVOLVE_RECOMB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    let mut archive = Archive::new();
    let cfg = IlluminationConfig::new(iterations, 42).with_recombination(recomb);
    let stats = illuminate(
        &mut archive,
        &descriptor,
        Genome::default(),
        1,
        &mut BenchEvaluator,
        &mut PromptVariator,
        &cfg,
        None,
    );

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
        descriptor.coverage(&archive).unwrap_or(0.0) * 100.0
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
    println!(
        "\n(Illustrative; a publishable M6 result needs replicate runs + the neutral_drift_gate over \
         QD-scores — build-spec §6 acceptance. The gate already fires on synthetic data.)"
    );
}

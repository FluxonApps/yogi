//! Foreground M6 open-ended search on the TRANSFER corpus (loads qwen3:8b, thinking mode). Run:
//!   cargo run -p being-bench --bin evolve_transfer --release
//!
//! Unlike the frozen-suite `evolve` bin (saturated → one niche, FINDINGS 2026-06-21), here the genome
//! genuinely changes behavior: a genome carries a SUBSET of three made-up operation rules (⊕,⊗,⊙) in
//! its `installed_skills`; the evaluator injects those rules into the system prompt. Cold (no rules) the
//! model fails all of them; with a rule present it can apply it. So different rule-subsets solve
//! different operations → genuine MAP-Elites niches, and recombination (per-element skill-set crossover)
//! combines a ⊕-genome with a ⊗-genome into a child that solves both. This is the live setting where
//! open-ended search has something to illuminate.

use being_bench::multi_skill_corpus;
use being_core_economy::Account;
use being_core_id::Ed25519Signer;
use being_core_mutation::{Genome, MutationKind};
use being_lineage::{
    BehaviorDescriptor, Colony, Evaluation, Evaluator, IlluminationConfig, Rng, Variator,
};
use being_proposer_openai::{OpenAiChatConfig, OpenAiChatProposer};
use being_runtime::{Being, EchoExecutor, PassThroughCommitter};
use being_supervisor::Supervisor;

const SKILL_IDS: [&str; 3] = ["s0", "s1", "s2"]; // s0=⊕, s1=⊗, s2=⊙

/// Scores a rule-carrying genome on the single-op transfer tasks; behavior = which of the 3 ops it
/// solves. Loads the model (thinking mode — rule application needs a scratchpad). Foreground only.
struct TransferEvaluator {
    /// `tasks[i]` is the single-op task for operation `i` (⊕,⊗,⊙); `rules[i]` its rule text.
    tasks: Vec<(String, String)>, // (prompt, expected)
    rules: Vec<String>,
}

impl Evaluator for TransferEvaluator {
    fn evaluate(&mut self, g: &Genome) -> Evaluation {
        // Build the system prompt: base instruction + the rules this genome carries.
        let mut sys =
            String::from("You are a careful calculator. Apply ONLY the rules given below. Reply with just the final number.\n");
        for (i, id) in SKILL_IDS.iter().enumerate() {
            if g.installed_skills.contains(*id) {
                sys.push_str(&self.rules[i]);
                sys.push('\n');
            }
        }
        let mut cfg = OpenAiChatConfig::ollama_qwen3_thinking();
        cfg.system_prompt = sys;
        cfg.max_tokens = 1024; // bound thinking length to keep runs tractable
        let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
        let mut being = Being::from_seed(
            [1u8; 32],
            Supervisor::as_port(&sup),
            OpenAiChatProposer::new(cfg),
            PassThroughCommitter,
            EchoExecutor,
        );

        let mut behavior = vec![0.0; self.tasks.len()];
        let mut passes = 0usize;
        for (i, (prompt, expected)) in self.tasks.iter().enumerate() {
            let resp = being.turn(prompt, i as i64).observations.join(" ");
            if resp.contains(expected) {
                behavior[i] = 1.0;
                passes += 1;
            }
        }
        Evaluation {
            fitness: passes as f64 / self.tasks.len().max(1) as f64,
            behavior,
        }
    }
}

/// Adds or removes one operation rule from the genome's skill-set (closed surface: SkillInstall /
/// SkillRevoke). This genuinely moves the genome in behavior space, unlike a style-directive tweak.
struct SkillSetVariator;
impl Variator for SkillSetVariator {
    fn vary(&mut self, rng: &mut Rng, parent: &Genome) -> Vec<MutationKind> {
        let absent: Vec<&str> = SKILL_IDS
            .iter()
            .copied()
            .filter(|id| !parent.installed_skills.contains(*id))
            .collect();
        let present: Vec<&str> = SKILL_IDS
            .iter()
            .copied()
            .filter(|id| parent.installed_skills.contains(*id))
            .collect();
        // Prefer adding a missing rule; otherwise revoke one (keeps the population exploring subsets).
        if !absent.is_empty() && (present.is_empty() || rng.below(3) > 0) {
            vec![MutationKind::SkillInstall(
                absent[rng.below(absent.len())].into(),
            )]
        } else if !present.is_empty() {
            vec![MutationKind::SkillRevoke(
                present[rng.below(present.len())].into(),
            )]
        } else {
            vec![]
        }
    }
}

fn main() {
    eprintln!("M6 transfer illumination (foreground — loads qwen3:8b thinking mode) ...");
    let iters = std::env::var("EVOLVE_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    let corpus = multi_skill_corpus(1, 7); // 1 task per op → 3 single-op tasks (⊕,⊗,⊙)
    let tasks: Vec<(String, String)> = corpus
        .single
        .iter()
        .map(|t| (t.prompt.clone(), t.expected.clone()))
        .collect();
    let mut evaluator = TransferEvaluator {
        tasks,
        rules: corpus.skills.clone(),
    };

    // 3 binary niche axes (one per op solved) → 8 reachable cells.
    let descriptor =
        BehaviorDescriptor::bounded([(0.0, 1.0, 2), (0.0, 1.0, 2), (0.0, 1.0, 2)]).unwrap();
    let cfg = IlluminationConfig::new(iters, 42).with_recombination(0.5);
    let mut colony = Colony::new(descriptor.clone(), Ed25519Signer::from_seed([42; 32]), 1);

    let stats = colony.run(
        Genome::default(),
        &mut evaluator,
        &mut SkillSetVariator,
        &cfg,
    );
    let archive = &colony.archive;

    println!(
        "\nillumination: {} evals, {} improvements, {} recombinations, {} niches (coverage {:.0}%)",
        stats.evaluations,
        stats.improvements,
        stats.recombinations,
        archive.len(),
        descriptor.coverage(archive).unwrap_or(0.0) * 100.0
    );
    println!(
        "QD-score={:.3}  mean-fitness={:.3}",
        archive.qd_score(),
        archive.mean_fitness().unwrap_or(0.0)
    );
    println!(
        "signed fork ledger: {} forks · genealogy {} lineages (depth {})",
        colony.ledger.len(),
        colony.phylogeny.len(),
        colony.phylogeny.max_generation()
    );
    println!("\nniches (fitness, gen, #parents, skills):");
    for e in archive.elites() {
        let mut skills: Vec<&str> = e
            .genome
            .installed_skills
            .iter()
            .map(|s| s.as_str())
            .collect();
        skills.sort();
        println!(
            "  fitness={:.2} gen={} parents={} skills={:?}",
            e.fitness,
            e.lineage.generation,
            e.lineage.parents.len(),
            skills
        );
    }
    if let Some(best) = archive.best() {
        let mut s: Vec<&str> = best
            .genome
            .installed_skills
            .iter()
            .map(|x| x.as_str())
            .collect();
        s.sort();
        println!("\nglobal best: fitness={:.2} skills={s:?}", best.fitness);
    }
}

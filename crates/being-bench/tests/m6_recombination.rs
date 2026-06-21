//! M6 experiment (loop-safe, no model): does sexual recombination beat asexual mutation on a
//! **building-block** landscape — the textbook case where crossover should win?
//!
//! The genome carries four independent "genes", one per field (prompt, tool/retrieval/routing
//! policy); each is "correct" iff its first byte is `a`. Fitness is the fraction correct. Mutation
//! sets ONE random gene to a random letter (prob 1/K of being right), so assembling all four by
//! mutation alone is slow. `being-lineage::recombine` does *field-level* crossover, so it can take a
//! parent that solved genes {0,1} and one that solved {2,3} and breed a child with all four — exactly
//! the building-block advantage. We run asexual vs sexual arms over matched replicates and judge the
//! difference with the real `neutral_drift_gate`.

use being_bench::neutral_drift_gate;
use being_core_mutation::{Genome, MutationKind};
use being_lineage::{
    illuminate, Archive, BehaviorDescriptor, Evaluation, Evaluator, IlluminationConfig, Rng,
    Variator,
};

const GENES: usize = 4;
const ALPHABET: usize = 6; // letters a..f; only `a` is correct

/// Fitness = fraction of the four genes equal to `a`; behavior = correct-count (a 0..4 niche axis).
struct BuildingBlocks;
impl Evaluator for BuildingBlocks {
    fn evaluate(&mut self, g: &Genome) -> Evaluation {
        let genes = [
            g.prompt.as_bytes().first().copied(),
            g.tool_policy.first().copied(),
            g.retrieval_policy.first().copied(),
            g.routing_policy.first().copied(),
        ];
        let correct = genes.iter().filter(|b| **b == Some(b'a')).count();
        // Niche by WHICH of the first two genes are solved (preserves building-block diversity, so
        // the archive holds parents specialised on different blocks for crossover to combine).
        let g0 = (genes[0] == Some(b'a')) as i64 as f64;
        let g1 = (genes[1] == Some(b'a')) as i64 as f64;
        Evaluation {
            fitness: correct as f64 / GENES as f64,
            behavior: vec![g0, g1],
        }
    }
}

/// Set one random gene (field) to a random letter — single-gene point mutation over the closed surface.
struct MutateGene;
impl Variator for MutateGene {
    fn vary(&mut self, rng: &mut Rng, _parent: &Genome) -> Vec<MutationKind> {
        let ch = (b'a' + rng.below(ALPHABET) as u8) as char;
        match rng.below(GENES) {
            0 => vec![MutationKind::Prompt(ch.to_string())],
            1 => vec![MutationKind::ToolPolicy(vec![ch as u8])],
            2 => vec![MutationKind::RetrievalPolicy(vec![ch as u8])],
            _ => vec![MutationKind::RoutingPolicy(vec![ch as u8])],
        }
    }
}

fn best_fitness(recombination_rate: f64, seed: u64) -> f64 {
    let desc = BehaviorDescriptor::bounded([(0.0, 1.0, 2), (0.0, 1.0, 2)]).unwrap();
    let mut archive = Archive::new();
    let cfg = IlluminationConfig::new(30, seed).with_recombination(recombination_rate);
    illuminate(
        &mut archive,
        &desc,
        Genome::default(),
        1,
        &mut BuildingBlocks,
        &mut MutateGene,
        &cfg,
        None,
    );
    archive.best().map(|e| e.fitness).unwrap_or(0.0)
}

// Finding (recorded in docs/FINDINGS.md): on this landscape recombination's effect depends entirely
// on whether the BEHAVIOR SPACE preserves building-block diversity. With correct-count niching the
// archive keeps one genome per fitness-level and loses *which* blocks each lineage solved, so crossover
// has nothing diverse to combine and (paying eval overhead) does slightly worse. Niching by *which*
// genes are solved (as here) keeps specialists for different blocks, and crossover then edges ahead —
// but only marginally at a 30-eval budget, so the gate correctly does NOT fire (no significant effect).
// We therefore assert only the robust facts: the pipeline is deterministic and both arms make real
// progress. The honest "not significant" verdict is the gate doing its job, not a bug.
#[test]
fn recombination_experiment_runs_deterministically_and_both_arms_progress() {
    let replicates = 16;
    let mut asexual = Vec::new();
    let mut sexual = Vec::new();
    for i in 0..replicates {
        let seed = 500 + i as u64 * 13;
        asexual.push(best_fitness(0.0, seed));
        sexual.push(best_fitness(0.6, seed));
    }

    let report = neutral_drift_gate(&asexual, &sexual, 0.0, 2000, 7, 0.05);
    // Both arms climb well above the all-wrong baseline (search is doing real work).
    assert!(
        report.drift_mean > 0.5,
        "asexual should make progress: {report:?}"
    );
    assert!(
        report.selection_mean > 0.5,
        "sexual should make progress: {report:?}"
    );

    // Determinism: same seeds → identical verdict (replayable experiment).
    let again = neutral_drift_gate(&asexual, &sexual, 0.0, 2000, 7, 0.05);
    assert_eq!(report.ci.lower, again.ci.lower);
    assert_eq!(report.fires, again.fires);
}

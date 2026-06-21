//! M6 acceptance, demonstrated loop-safe (no model): the real MAP-Elites engine
//! ([`being_lineage::illuminate`]) driven through the real entry gate
//! ([`being_bench::neutral_drift_gate`]).
//!
//! The honest question M6 must answer is *"is the search doing real work, or is it neutral drift
//! dressed up by a best-so-far operator?"* Here both arms run the SAME evaluation budget and the SAME
//! random variation; the ONLY difference is retention — elitist (fitness-based, real selection) vs
//! latest-wins (fitness ignored, the matched drift control). On a noisy fitness landscape, elitist
//! retention captures the max within each behavior niche while drift random-walks around the mean, so
//! selection reliably beats drift and the gate fires. (If they tied, the gate would *not* fire and the
//! breeding-program-not-evolution null would be the honest report — that is the point of the control.)

use being_bench::neutral_drift_gate;
use being_core_mutation::{Genome, MutationKind};
use being_lineage::{
    illuminate, Archive, BehaviorDescriptor, Evaluation, Evaluator, IlluminationConfig, Retention,
    Rng, Variator,
};

/// FNV-1a over bytes from a chosen 64-bit initial state (different inits → decorrelated streams).
fn fnv(bytes: &[u8], init: u64) -> u64 {
    let mut h = init;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A noisy landscape: behavior spreads a genome across 8 niches; fitness is a per-genome pseudo-random
/// value in [0,1) decorrelated from behavior. Within a niche, repeated visits draw fresh fitnesses —
/// so elitist retention (keep the max) climbs while drift (keep the latest) does not.
struct NoisyLandscape;
impl Evaluator for NoisyLandscape {
    fn evaluate(&mut self, g: &Genome) -> Evaluation {
        let niche = (fnv(g.prompt.as_bytes(), 0xcbf2_9ce4_8422_2325) % 8) as f64;
        let fitness = (fnv(g.prompt.as_bytes(), 0x1234_5678_9abc_def0) % 1_000_000) as f64 / 1e6;
        Evaluation {
            fitness,
            behavior: vec![niche],
        }
    }
}

/// Random search over the closed surface: replace the prompt with a fresh 8-letter string.
struct RandomPrompt;
impl Variator for RandomPrompt {
    fn vary(&mut self, rng: &mut Rng, _parent: &Genome) -> Vec<MutationKind> {
        let s: String = (0..8)
            .map(|_| (b'a' + rng.below(26) as u8) as char)
            .collect();
        vec![MutationKind::Prompt(s)]
    }
}

#[test]
fn selection_beats_neutral_drift_and_the_gate_fires() {
    // One behavior axis, width 1 → niches 0..7.
    let desc = BehaviorDescriptor::new([(0.0, 1.0)]).unwrap();
    let replicates = 12;
    let iterations = 200;

    let mut drift_finals = Vec::new();
    let mut selection_finals = Vec::new();
    for i in 0..replicates {
        let seed = 1000 + i as u64 * 7; // matched seed per replicate (paired control)

        let mut a_sel = Archive::new();
        illuminate(
            &mut a_sel,
            &desc,
            Genome::default(),
            1,
            &mut NoisyLandscape,
            &mut RandomPrompt,
            &IlluminationConfig::new(iterations, seed).with_retention(Retention::Elitist),
            None,
        );
        selection_finals.push(a_sel.mean_fitness().unwrap());

        let mut a_drift = Archive::new();
        illuminate(
            &mut a_drift,
            &desc,
            Genome::default(),
            1,
            &mut NoisyLandscape,
            &mut RandomPrompt,
            &IlluminationConfig::new(iterations, seed).with_retention(Retention::NeutralDrift),
            None,
        );
        drift_finals.push(a_drift.mean_fitness().unwrap());
    }

    // The M6 entry gate: selection must reliably beat the drift control by the margin.
    let report = neutral_drift_gate(&drift_finals, &selection_finals, 0.1, 2000, 99, 0.05);
    assert!(
        report.fires,
        "selection should beat neutral drift: {report:?}"
    );
    // Selection captures the per-niche max (~1.0); drift random-walks around the mean (~0.5).
    assert!(report.selection_mean > 0.85, "{report:?}");
    assert!(report.drift_mean < 0.7, "{report:?}");
    // The advantage CI excludes zero and clears the margin (this is what `fires` encodes).
    assert!(report.ci.lower > 0.1, "{report:?}");

    // Determinism: re-running the whole pipeline reproduces the verdict.
    let again = neutral_drift_gate(&drift_finals, &selection_finals, 0.1, 2000, 99, 0.05);
    assert_eq!(report.fires, again.fires);
    assert_eq!(report.ci.lower, again.ci.lower);
}

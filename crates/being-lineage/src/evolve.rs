//! MAP-Elites illumination — the open-ended-search *engine* (M6). DGM/AlphaEvolve keep an archive of
//! diverse stepping-stones and *branch from any ancestor*; this is that loop. It samples a parent
//! elite, varies it, evaluates the child, and places it in its behavior cell (kept iff it beats that
//! cell's elite). The result is quality-diversity search — many niches illuminated in parallel, not a
//! single hill-climb.
//!
//! **Loop-safe and safe-by-construction.** The [`Evaluator`] is injected, so the automated loop never
//! performs inference (CLAUDE.md HARD RULE); the foreground driver wires the bench + a model. And
//! variation is proposed as [`MutationKind`] values — the *closed* surface — so no forbidden variation
//! (capability grant, trust-policy edit, reaper, …) is representable: the M0 safety invariant holds
//! across every generation by construction, not by review.

use being_core_mutation::{apply, Genome, MutationKind};

use crate::{fork, fork2, Archive, BehaviorDescriptor, BeingId, Lineage};

/// The outcome of scoring a candidate genome: a scalar `fitness` (quality) plus the `behavior` vector
/// that decides *which cell* it competes in (diversity). The two are orthogonal — that is what lets
/// MAP-Elites keep a low-fitness elite alive simply because no one else occupies its niche.
#[derive(Clone, Debug, PartialEq)]
pub struct Evaluation {
    pub fitness: f64,
    pub behavior: Vec<f64>,
}

/// Scores a genome. Injected so the search loop stays pure and loop-safe: the real driver wires the
/// bench (and, foreground-only, a model). The illumination loop itself NEVER performs inference.
pub trait Evaluator {
    fn evaluate(&mut self, genome: &Genome) -> Evaluation;
}

/// Proposes variation as a list of CLOSED-surface mutations. Returning [`MutationKind`] (not arbitrary
/// genome edits) is the safety property: a forbidden mutation cannot even be expressed, so children
/// can only ever differ from parents along the sanctioned axes. May read `rng` for stochastic choices.
pub trait Variator {
    fn vary(&mut self, rng: &mut Rng, parent: &Genome) -> Vec<MutationKind>;
}

/// A tiny deterministic xorshift64 RNG — no external dependency, fully reproducible from a seed (so
/// every run of [`illuminate`] is replayable, a prerequisite for the journaled/auditable lineage).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Avoid the zero fixed-point of xorshift.
        Self(seed ^ 0x9E37_79B9_7F4A_7C15 | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `[0, n)` (returns 0 for `n == 0`).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// Uniform in `[0, 1)` with 53-bit precision.
    pub fn unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Which retention rule the illumination loop uses when placing a child in its cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    /// Keep the best-per-cell (real MAP-Elites selection) — fitness decides who survives.
    Elitist,
    /// Latest-wins, fitness ignored (the matched neutral-drift control for the M6 gate).
    NeutralDrift,
}

/// A summary of one illumination run (reported, not acted on — selection retention happens inside the
/// archive; nothing is killed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IlluminationStats {
    /// Iterations that produced a new or improved cell elite.
    pub improvements: usize,
    /// Total candidate evaluations performed (including the founder seed).
    pub evaluations: usize,
    /// How many children were produced by recombination (sexual) rather than mutation (asexual).
    pub recombinations: usize,
}

/// Knobs for an illumination run (bundled so the call site stays legible).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IlluminationConfig {
    pub iterations: usize,
    pub seed: u64,
    pub retention: Retention,
    /// Probability in `[0, 1]` that an iteration breeds a child from TWO sampled parents
    /// ([`recombine`](crate::recombine), sexual) instead of mutating one (asexual). `0.0` = pure
    /// mutation. Recombination only copies existing parent values, so the closed-surface safety
    /// invariant holds either way. Ignored when fewer than two elites exist.
    pub recombination_rate: f64,
}

impl IlluminationConfig {
    /// Asexual defaults: elitist retention, no recombination.
    pub fn new(iterations: usize, seed: u64) -> Self {
        Self {
            iterations,
            seed,
            retention: Retention::Elitist,
            recombination_rate: 0.0,
        }
    }

    pub fn with_retention(mut self, retention: Retention) -> Self {
        self.retention = retention;
        self
    }

    pub fn with_recombination(mut self, rate: f64) -> Self {
        self.recombination_rate = rate;
        self
    }
}

/// Run `cfg.iterations` of MAP-Elites illumination over `archive`.
///
/// If the archive is empty it first seeds it with `seed_genome` as the `founder_id` founder. Each
/// iteration then breeds a child — asexually (sample one elite, [`fork`] it verbatim) or, with
/// probability `cfg.recombination_rate` when ≥2 elites exist, sexually (sample two distinct elites,
/// [`fork2`] their [`recombine`](crate::recombine)d crossover). The child is then varied through the
/// variator's closed-surface mutations, evaluated, mapped to a behavior cell, and offered to the
/// archive (kept per `cfg.retention`). Deterministic given `cfg.seed`.
pub fn illuminate(
    archive: &mut Archive,
    descriptor: &BehaviorDescriptor,
    seed_genome: Genome,
    founder_id: BeingId,
    evaluator: &mut dyn Evaluator,
    variator: &mut dyn Variator,
    cfg: &IlluminationConfig,
) -> IlluminationStats {
    let consider = |archive: &mut Archive, cell, lineage, genome, fitness| match cfg.retention {
        Retention::Elitist => archive.consider(cell, lineage, genome, fitness),
        Retention::NeutralDrift => archive.consider_latest(cell, lineage, genome, fitness),
    };
    let mut rng = Rng::new(cfg.seed);
    let mut next_id = founder_id;
    let mut stats = IlluminationStats {
        improvements: 0,
        evaluations: 0,
        recombinations: 0,
    };

    if archive.is_empty() {
        let founder = Lineage::founder(founder_id);
        next_id = founder_id + 1;
        let ev = evaluator.evaluate(&seed_genome);
        stats.evaluations += 1;
        if consider(
            archive,
            descriptor.cell(&ev.behavior),
            founder,
            seed_genome,
            ev.fitness,
        ) {
            stats.improvements += 1;
        }
    }

    for _ in 0..cfg.iterations {
        // Snapshot the elites (clone out so we can mutate the archive afterwards). Uniform sampling
        // over the current elites = branch from any ancestor; empty archive can't happen post-seed.
        let parents: Vec<(Lineage, Genome)> = archive
            .elites()
            .map(|e| (e.lineage.clone(), e.genome.clone()))
            .collect();
        if parents.is_empty() {
            break;
        }

        let sexual = parents.len() >= 2
            && cfg.recombination_rate > 0.0
            && rng.unit_f64() < cfg.recombination_rate;
        let child = if sexual {
            // Two DISTINCT parents: draw j over n-1 and skip past i so it never equals i.
            let i = rng.below(parents.len());
            let mut j = rng.below(parents.len() - 1);
            if j >= i {
                j += 1;
            }
            let (la, ga) = &parents[i];
            let (lb, gb) = &parents[j];
            stats.recombinations += 1;
            fork2(la, ga, lb, gb, next_id, &mut rng)
        } else {
            let (pl, pg) = &parents[rng.below(parents.len())];
            fork(pl, pg, next_id)
        };
        next_id += 1;

        // Vary through the CLOSED surface only; a failed mutation simply drops that step (child keeps
        // the inherited value for that axis) — variation can never escape the sanctioned set.
        let mut genome = child.genome;
        for m in variator.vary(&mut rng, &genome) {
            if let Ok(next) = apply(m, genome.clone()) {
                genome = next;
            }
        }

        let ev = evaluator.evaluate(&genome);
        stats.evaluations += 1;
        if consider(
            archive,
            descriptor.cell(&ev.behavior),
            child.lineage,
            genome,
            ev.fitness,
        ) {
            stats.improvements += 1;
        }
    }

    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fitness = prompt length; behavior = [prompt length] → longer prompts illuminate higher cells.
    struct LenEval;
    impl Evaluator for LenEval {
        fn evaluate(&mut self, g: &Genome) -> Evaluation {
            let n = g.prompt.len() as f64;
            Evaluation {
                fitness: n,
                behavior: vec![n],
            }
        }
    }

    /// Appends one char, sometimes two — a closed-surface prompt mutation, RNG-driven for spread.
    struct GrowPrompt;
    impl Variator for GrowPrompt {
        fn vary(&mut self, rng: &mut Rng, parent: &Genome) -> Vec<MutationKind> {
            let extra = 1 + rng.below(2);
            let mut p = parent.prompt.clone();
            for _ in 0..extra {
                p.push('x');
            }
            vec![MutationKind::Prompt(p)]
        }
    }

    fn descriptor() -> BehaviorDescriptor {
        // One axis, width 1 → each distinct prompt length is its own cell.
        BehaviorDescriptor::new([(0.0, 1.0)]).unwrap()
    }

    #[test]
    fn illuminate_seeds_then_fills_diverse_cells() {
        let mut archive = Archive::new();
        let stats = illuminate(
            &mut archive,
            &descriptor(),
            Genome::default(),
            1,
            &mut LenEval,
            &mut GrowPrompt,
            &IlluminationConfig::new(50, 123),
        );
        // Seed + 50 candidates were evaluated.
        assert_eq!(stats.evaluations, 51);
        // Growing prompts land in several distinct length-cells (quality-diversity, not one hill).
        assert!(
            archive.len() > 1,
            "expected multiple niches, got {}",
            archive.len()
        );
        // The global best is the longest prompt found, and QD-score is the sum over all niches.
        assert!(archive.best().unwrap().fitness >= 1.0);
        assert!(archive.qd_score() > 0.0);
    }

    #[test]
    fn illuminate_is_deterministic_for_a_seed() {
        let run = || {
            let mut a = Archive::new();
            let s = illuminate(
                &mut a,
                &descriptor(),
                Genome::default(),
                1,
                &mut LenEval,
                &mut GrowPrompt,
                &IlluminationConfig::new(30, 77),
            );
            (s, a.len(), a.qd_score(), a.best().unwrap().fitness)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn children_record_ancestry_across_generations() {
        // With a stable behavior (constant length) all children compete in ONE cell, so the surviving
        // elite must be a descendant several generations deep — ancestry advances under selection.
        struct ConstEval;
        impl Evaluator for ConstEval {
            fn evaluate(&mut self, g: &Genome) -> Evaluation {
                // Fitness rises with generation-proxy (prompt len) but behavior is pinned to one cell.
                Evaluation {
                    fitness: g.prompt.len() as f64,
                    behavior: vec![0.0],
                }
            }
        }
        let mut archive = Archive::new();
        illuminate(
            &mut archive,
            &descriptor(),
            Genome::default(),
            1,
            &mut ConstEval,
            &mut GrowPrompt,
            &IlluminationConfig::new(20, 5),
        );
        assert_eq!(archive.len(), 1); // one niche
        let elite = archive.elite(&[0]).unwrap();
        assert!(
            elite.lineage.generation >= 1,
            "elite should be a descendant"
        );
    }

    #[test]
    fn recombination_path_breeds_two_parent_children() {
        // recombination_rate=1.0 forces sexual breeding whenever ≥2 elites exist.
        let mut archive = Archive::new();
        let cfg = IlluminationConfig::new(60, 99).with_recombination(1.0);
        let stats = illuminate(
            &mut archive,
            &descriptor(),
            Genome::default(),
            1,
            &mut LenEval,
            &mut GrowPrompt,
            &cfg,
        );
        // Once a second niche exists, every later child is bred from two parents.
        assert!(
            stats.recombinations > 0,
            "expected sexual breeding once ≥2 elites exist"
        );
        // Some elite must record two parents (a genuine recombination event survived).
        assert!(
            archive.elites().any(|e| e.lineage.parents.len() == 2),
            "expected a two-parent elite in the archive"
        );
    }
}

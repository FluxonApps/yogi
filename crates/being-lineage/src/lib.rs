//! M6 lineage substrate — **GATED**. Heredity mechanics ONLY.
//!
//! This crate is the *phylogeny* substrate (build-spec §6, M6): a child inherits the parent genome
//! and records its ancestry. **SELECTION IS OFF.** There is deliberately no fitness function, no
//! reproduction scheduler, no population-level death here — those turn on only after the bench's
//! compounding AND anti-theater gates fire (CLAUDE.md: "Selection (M6) stays OFF until the bench
//! shows compounding AND the anti-theater gate fires"). Building the heredity data + the fork
//! operation now keeps the substrate ready without enabling the dangerous part.
//!
//! Pure and loop-safe: no model, no clock, no I/O. Variation is NOT applied here — a forked child
//! inherits the genome verbatim; it varies later only through the closed [`being_core_mutation`]
//! surface, so the safety invariant (no forbidden mutation is representable) holds across generations.

use std::collections::BTreeMap;

use being_core_mutation::Genome;

mod evolve;
pub use evolve::{illuminate, Evaluation, Evaluator, IlluminationStats, Retention, Rng, Variator};

/// Identifies a being within a lineage (the operator/registry assigns ids; this crate only records).
pub type BeingId = u64;

/// A being's ancestry record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lineage {
    pub id: BeingId,
    /// Parent ids (one for asexual fork; two for a future recombining variant — deferred with selection).
    pub parents: Vec<BeingId>,
    pub generation: u64,
}

impl Lineage {
    /// A founder: generation 0, no parents.
    pub fn founder(id: BeingId) -> Self {
        Self {
            id,
            parents: Vec::new(),
            generation: 0,
        }
    }
}

/// A forked child: its inherited genome + its lineage. NOT installed anywhere and NOT subject to
/// selection — heredity mechanics only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offspring {
    pub lineage: Lineage,
    pub genome: Genome,
}

/// Fork a child from one parent. The child **inherits the parent genome verbatim** (variation happens
/// later, only via the closed mutation surface), generation + 1, ancestry recorded. Asexual; a
/// sexual/recombining variant would take two parents and is deferred along with selection.
///
/// This does NOT select, score, or kill anything — it is the heredity primitive, nothing more.
pub fn fork(parent: &Lineage, parent_genome: &Genome, child_id: BeingId) -> Offspring {
    Offspring {
        lineage: Lineage {
            id: child_id,
            parents: vec![parent.id],
            generation: parent.generation + 1,
        },
        genome: parent_genome.clone(),
    }
}

// ---------------------------------------------------------------------------------------------
// MAP-Elites archive (M6 substrate; GATED). DGM/AlphaEvolve keep an archive and branch off any
// ancestor — the engine of *open-ended* search (keep diverse stepping-stones, not one hill-climb).
// This stores the best member per behavior-descriptor cell. **SELECTION IS OFF**: it only stores and
// queries; it never reproduces, scores, or kills. Pure. Driving reproduction/death from this archive
// is the gated step that needs human review before it is enabled.
// ---------------------------------------------------------------------------------------------

/// A behavior-descriptor cell key (discretized behavior coordinates). Two members in the same cell
/// compete; different cells coexist (that is what preserves diversity).
pub type Cell = Vec<i64>;

/// The best member found so far for one cell.
#[derive(Clone, Debug, PartialEq)]
pub struct Elite {
    pub lineage: Lineage,
    pub genome: Genome,
    pub fitness: f64,
}

/// A MAP-Elites archive: best-per-cell store. No selection loop — storage + queries only.
#[derive(Default)]
pub struct Archive {
    cells: BTreeMap<Cell, Elite>,
}

impl Archive {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Consider a member for its cell; keep it iff the cell is empty or it strictly beats the current
    /// elite. Returns `true` if it became (or replaced) the cell's elite. (Pure storage — no death.)
    pub fn consider(&mut self, cell: Cell, lineage: Lineage, genome: Genome, fitness: f64) -> bool {
        match self.cells.get(&cell) {
            Some(e) if e.fitness >= fitness => false,
            _ => {
                self.cells.insert(
                    cell,
                    Elite {
                        lineage,
                        genome,
                        fitness,
                    },
                );
                true
            }
        }
    }

    /// **Neutral-drift retention** — always install the candidate as the cell's occupant, *ignoring*
    /// fitness (latest-wins random walk within each cell). This is the matched control arm for the M6
    /// gate: identical eval budget and variation as elitist [`consider`](Archive::consider), the ONLY
    /// difference being that retention is not fitness-based. Returns `true` iff it opened a new cell.
    pub fn consider_latest(
        &mut self,
        cell: Cell,
        lineage: Lineage,
        genome: Genome,
        fitness: f64,
    ) -> bool {
        let is_new = !self.cells.contains_key(&cell);
        self.cells.insert(
            cell,
            Elite {
                lineage,
                genome,
                fitness,
            },
        );
        is_new
    }

    pub fn elite(&self, cell: &[i64]) -> Option<&Elite> {
        self.cells.get(cell)
    }

    pub fn elites(&self) -> impl Iterator<Item = &Elite> {
        self.cells.values()
    }

    /// The highest-fitness elite across all cells (the current global best — *reported*, not selected).
    pub fn best(&self) -> Option<&Elite> {
        self.cells
            .values()
            .max_by(|a, b| a.fitness.total_cmp(&b.fitness))
    }

    /// **QD-score** — the canonical quality-diversity metric: sum of elite fitnesses over filled cells.
    /// It rewards *both* covering more cells (diversity) and holding better elites (quality), so it is
    /// the single number that tracks open-ended progress. Pure observability — *reported*, never used
    /// to select, reproduce, or kill (that is the gated step).
    pub fn qd_score(&self) -> f64 {
        self.cells.values().map(|e| e.fitness).sum()
    }

    /// Mean elite fitness across filled cells (`None` when empty). Companion to [`Archive::qd_score`]:
    /// QD-score grows with coverage, this isolates per-cell quality. Reportive only.
    pub fn mean_fitness(&self) -> Option<f64> {
        if self.cells.is_empty() {
            None
        } else {
            Some(self.qd_score() / self.cells.len() as f64)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Behavior descriptor (M6 substrate; GATED). The conceptual heart of MAP-Elites: *where* in the
// archive a member lands. A being's measured behavior is a point in a continuous space (e.g.
// [verbosity, tool-calls/turn, latency]); the descriptor discretizes that point into the integer
// [`Cell`] key the [`Archive`] competes within. Pure and deterministic — no model, no clock, no I/O,
// no selection. It only *names a cell*; it never decides who lives or reproduces.
// ---------------------------------------------------------------------------------------------

/// Maps a continuous behavior vector to its discrete [`Cell`] by binning each dimension at a fixed
/// origin + width. Deterministic: the same behavior always lands in the same cell, so two members are
/// compared (in the archive) iff they behave alike along every characterized axis.
#[derive(Clone, Debug, PartialEq)]
pub struct BehaviorDescriptor {
    /// Lower edge of bin 0 for each dimension.
    origin: Vec<f64>,
    /// Bin width for each dimension (must be > 0).
    width: Vec<f64>,
}

impl BehaviorDescriptor {
    /// Build a descriptor from per-dimension `(origin, width)` pairs. `width` must be strictly
    /// positive on every axis (a zero/negative bin is meaningless); returns `None` otherwise.
    pub fn new(bins: impl IntoIterator<Item = (f64, f64)>) -> Option<Self> {
        let (origin, width): (Vec<f64>, Vec<f64>) = bins.into_iter().unzip();
        // Reject zero, negative, and NaN widths (a non-positive bin is meaningless).
        if width.iter().any(|w| *w <= 0.0 || w.is_nan()) {
            return None;
        }
        Some(Self { origin, width })
    }

    /// Number of characterized behavior dimensions.
    pub fn dims(&self) -> usize {
        self.width.len()
    }

    /// Discretize a behavior vector into its cell. Each axis `i` bins to `floor((x_i − origin_i)/width_i)`.
    /// A missing coordinate (shorter `behavior`) falls into bin 0's axis (treated as `origin_i`), so the
    /// result is always `dims()` long and never panics — deterministic regardless of input length.
    pub fn cell(&self, behavior: &[f64]) -> Cell {
        (0..self.dims())
            .map(|i| {
                let x = behavior.get(i).copied().unwrap_or(self.origin[i]);
                ((x - self.origin[i]) / self.width[i]).floor() as i64
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use being_core_mutation::{apply, MutationKind};

    use super::*;

    #[test]
    fn founder_has_no_parents_and_is_generation_zero() {
        let f = Lineage::founder(1);
        assert!(f.parents.is_empty());
        assert_eq!(f.generation, 0);
    }

    #[test]
    fn fork_inherits_genome_and_advances_generation() {
        let parent = Lineage::founder(1);
        let pg = Genome {
            prompt: "parent prompt".into(),
            ..Genome::default()
        };

        let child = fork(&parent, &pg, 2);
        assert_eq!(child.lineage.generation, 1);
        assert_eq!(child.lineage.parents, vec![1]);
        assert_eq!(child.genome, pg); // inherited verbatim
    }

    #[test]
    fn archive_keeps_best_per_cell_and_reports_global_best() {
        let mut a = Archive::new();
        let g = Genome::default;
        // empty cell → inserted
        assert!(a.consider(vec![0], Lineage::founder(1), g(), 0.5));
        // same cell, worse → rejected; better → replaces
        assert!(!a.consider(vec![0], Lineage::founder(2), g(), 0.4));
        assert!(a.consider(vec![0], Lineage::founder(3), g(), 0.9));
        assert_eq!(a.elite(&[0]).unwrap().fitness, 0.9);
        // a different cell coexists (diversity preserved)
        assert!(a.consider(vec![1], Lineage::founder(4), g(), 0.6));
        assert_eq!(a.len(), 2);
        // global best across cells
        assert_eq!(a.best().unwrap().fitness, 0.9);
    }

    #[test]
    fn qd_score_and_mean_track_quality_and_diversity() {
        let mut a = Archive::new();
        let g = Genome::default;
        assert_eq!(a.qd_score(), 0.0);
        assert_eq!(a.mean_fitness(), None);

        a.consider(vec![0], Lineage::founder(1), g(), 0.5);
        a.consider(vec![1], Lineage::founder(2), g(), 0.9);
        // QD-score = sum over filled cells; rewards both coverage and quality.
        assert_eq!(a.qd_score(), 1.4);
        assert_eq!(a.mean_fitness(), Some(0.7));

        // Beating a cell's elite raises QD-score; it never double-counts (best-per-cell).
        a.consider(vec![0], Lineage::founder(3), g(), 0.8);
        assert!((a.qd_score() - 1.7).abs() < 1e-9);
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn behavior_descriptor_discretizes_deterministically() {
        // Two axes: origin 0.0, width 1.0 on the first; origin 0.0, width 10.0 on the second.
        let bd = BehaviorDescriptor::new([(0.0, 1.0), (0.0, 10.0)]).unwrap();
        assert_eq!(bd.dims(), 2);
        // floor((2.3-0)/1)=2 ; floor((25-0)/10)=2
        assert_eq!(bd.cell(&[2.3, 25.0]), vec![2, 2]);
        // determinism: nearby points in the same bin collide (that is the competition rule)
        assert_eq!(bd.cell(&[2.9, 29.9]), vec![2, 2]);
        // a crossed bin edge lands elsewhere (diversity preserved)
        assert_eq!(bd.cell(&[3.0, 30.0]), vec![3, 3]);
        // missing coordinate → bin 0 on that axis, never panics
        assert_eq!(bd.cell(&[3.0]), vec![3, 0]);
        // a zero/negative width is rejected
        assert!(BehaviorDescriptor::new([(0.0, 0.0)]).is_none());
    }

    #[test]
    fn descriptor_feeds_the_archive_end_to_end() {
        // The intended wiring: measured behavior → descriptor → cell → archive. Selection still OFF.
        let bd = BehaviorDescriptor::new([(0.0, 0.5)]).unwrap();
        let mut a = Archive::new();
        let g = Genome::default;
        // behaviors 0.1 and 0.4 share bin 0; 0.7 lands in bin 1 — two diverse niches.
        assert!(a.consider(bd.cell(&[0.1]), Lineage::founder(1), g(), 0.3));
        assert!(!a.consider(bd.cell(&[0.4]), Lineage::founder(2), g(), 0.2)); // same cell, worse
        assert!(a.consider(bd.cell(&[0.7]), Lineage::founder(3), g(), 0.6));
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn child_variation_does_not_mutate_the_parent() {
        let parent = Lineage::founder(1);
        let pg = Genome {
            prompt: "parent".into(),
            ..Genome::default()
        };
        let child = fork(&parent, &pg, 2);

        // The child varies only through the closed mutation surface; the parent genome is untouched.
        let varied = apply(MutationKind::Prompt("child".into()), child.genome).unwrap();
        assert_eq!(varied.prompt, "child");
        assert_eq!(pg.prompt, "parent"); // parent unchanged — selection-free heredity
    }
}

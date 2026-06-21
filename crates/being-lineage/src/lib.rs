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

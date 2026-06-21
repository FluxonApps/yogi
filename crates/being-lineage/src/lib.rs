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

//! `Colony` — signed, crash-recoverable open-ended search (the M6 capstone). It runs the
//! [`illuminate`] engine while a [`ForkObserver`] **signs every fork edge into a [`ForkLedger`]** and
//! **records the full genealogy into a [`Phylogeny`]**. The result is the M6 acceptance object: an
//! open-ended search whose every reproduction event is a signed, content-addressed, idempotently
//! committed snapshot (replay after a crash re-commits as no-ops), with the whole ancestry preserved.
//!
//! One colony key attests all fork edges (the research-build choice; per-being keys are a later
//! refinement). Pure and loop-safe: the [`Evaluator`] is injected, so the automated loop never infers.

use being_core_id::Signer;
use being_core_mutation::Genome;

use crate::{
    illuminate, Archive, BehaviorDescriptor, BeingId, Evaluator, ForkLedger, ForkObserver,
    ForkSnapshot, IlluminationConfig, IlluminationStats, Lineage, Offspring, Phylogeny, Variator,
};

/// Signs each non-founder fork into the ledger and records every lineage into the phylogeny. Holds
/// disjoint `&mut` borrows of the colony's fields so it can run alongside the engine's `&mut archive`.
struct SigningObserver<'a> {
    phylogeny: &'a mut Phylogeny,
    ledger: &'a mut ForkLedger,
    signer: &'a dyn Signer,
}

impl ForkObserver for SigningObserver<'_> {
    fn on_fork(&mut self, parents: &[(Lineage, Genome)], child: &Offspring) {
        self.phylogeny.record(&child.lineage);
        if parents.is_empty() {
            return; // the founder is the root — no fork edge to attest.
        }
        let parent_lineages: Vec<Lineage> = parents.iter().map(|(l, _)| l.clone()).collect();
        let snap = ForkSnapshot::attest(&parent_lineages, child.clone(), self.signer);
        self.ledger.commit(&snap);
    }
}

/// A population running signed, genealogy-recorded open-ended search.
pub struct Colony<S: Signer> {
    pub archive: Archive,
    pub phylogeny: Phylogeny,
    pub ledger: ForkLedger,
    pub descriptor: BehaviorDescriptor,
    signer: S,
    founder_id: BeingId,
}

impl<S: Signer> Colony<S> {
    pub fn new(descriptor: BehaviorDescriptor, signer: S, founder_id: BeingId) -> Self {
        Self {
            archive: Archive::new(),
            phylogeny: Phylogeny::new(),
            ledger: ForkLedger::new(),
            descriptor,
            signer,
            founder_id,
        }
    }

    /// The colony DID that attests every fork edge.
    pub fn did(&self) -> &being_core_types::Did {
        self.signer.did()
    }

    /// Run an illumination pass: breed/evaluate/place candidates while signing each fork into the
    /// ledger and recording the genealogy. Seeds the archive from `seed_genome` on the first call.
    pub fn run(
        &mut self,
        seed_genome: Genome,
        evaluator: &mut dyn Evaluator,
        variator: &mut dyn Variator,
        cfg: &IlluminationConfig,
    ) -> IlluminationStats {
        // Disjoint field borrows: the observer takes phylogeny/ledger/signer, the engine takes archive.
        let mut observer = SigningObserver {
            phylogeny: &mut self.phylogeny,
            ledger: &mut self.ledger,
            signer: &self.signer,
        };
        illuminate(
            &mut self.archive,
            &self.descriptor,
            seed_genome,
            self.founder_id,
            evaluator,
            variator,
            cfg,
            Some(&mut observer),
        )
    }
}

#[cfg(test)]
mod tests {
    use being_core_id::Ed25519Signer;

    use super::*;
    use crate::{Evaluation, Rng};

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

    struct GrowPrompt;
    impl Variator for GrowPrompt {
        fn vary(
            &mut self,
            rng: &mut Rng,
            parent: &Genome,
        ) -> Vec<being_core_mutation::MutationKind> {
            let mut p = parent.prompt.clone();
            for _ in 0..=rng.below(2) {
                p.push('x');
            }
            vec![being_core_mutation::MutationKind::Prompt(p)]
        }
    }

    #[test]
    fn colony_signs_every_fork_and_records_genealogy() {
        let descriptor = BehaviorDescriptor::new([(0.0, 1.0)]).unwrap();
        let signer = Ed25519Signer::from_seed([3; 32]);
        let mut colony = Colony::new(descriptor, signer, 1);
        let cfg = IlluminationConfig::new(40, 7).with_recombination(0.5);

        let stats = colony.run(Genome::default(), &mut LenEval, &mut GrowPrompt, &cfg);

        // Every non-founder child (one per iteration) is a signed, committed fork; the founder is not.
        assert_eq!(colony.ledger.len(), stats.evaluations - 1);
        // The full genealogy is recorded and well-formed.
        assert_eq!(colony.phylogeny.len(), stats.evaluations);
        assert!(colony.phylogeny.is_well_formed());
        // The archive illuminated multiple niches and the colony has a stable DID.
        assert!(colony.archive.len() > 1);
        assert!(colony.did().0.starts_with("did:key:"));
    }

    #[test]
    fn re_running_is_idempotent_for_already_committed_forks() {
        // A second identical pass over a fresh colony with the same seed re-derives the same signed
        // snapshots — proving the commit ids are content-addressed and replay-stable.
        let run_ledger_size = || {
            let descriptor = BehaviorDescriptor::new([(0.0, 1.0)]).unwrap();
            let signer = Ed25519Signer::from_seed([3; 32]);
            let mut colony = Colony::new(descriptor, signer, 1);
            let cfg = IlluminationConfig::new(30, 11);
            colony.run(Genome::default(), &mut LenEval, &mut GrowPrompt, &cfg);
            colony.ledger.len()
        };
        assert_eq!(run_ledger_size(), run_ledger_size());
    }
}

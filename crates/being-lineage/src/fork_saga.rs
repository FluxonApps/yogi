//! Signed, crash-recoverable fork snapshot (M6 acceptance: *"a fork is a signed, crash-recoverable
//! distributed snapshot"*).
//!
//! A plain [`fork`](crate::fork) is the in-memory heredity primitive. To *commit* a fork durably —
//! survivably across a crash and attributably to its parent — we wrap it in a [`ForkSnapshot`]: the
//! **parent signs the exact heritable state the child inherits** (its lineage edge + the canonical
//! genome bytes), so the child's provenance is cryptographically traceable to the parent DID. The
//! snapshot's content-addressed [`ForkSnapshot::snapshot_id`] makes commit **idempotent**: replaying
//! the fork log after a crash re-commits the same id as a no-op, giving at-most-once application
//! (the same discipline as the M1 `DedupLedger`).
//!
//! Pure and loop-safe: no model, no clock, no I/O. Variation still only enters via the closed
//! [`being_core_mutation`] surface, so a signed child can never carry a forbidden mutation.

use std::collections::BTreeSet;

use being_core_id::{verify, Signer};
use being_core_mutation::Genome;
use being_core_types::{Did, Hash, Sig};

use crate::{fork, BeingId, Lineage, Offspring};

/// Domain-separation tag so a fork digest can never be confused with any other signed payload.
const FORK_DOMAIN: &[u8] = b"yogi.fork.snapshot.v1";

/// A signed fork record: the parent's signature over `(parent_did, parent edge, child edge, child
/// genome)`, content-addressed by [`snapshot_id`](ForkSnapshot::snapshot_id).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkSnapshot {
    /// The signer (parent) DID — the key the signature verifies against.
    pub parent_did: Did,
    /// The parent's lineage edge at fork time.
    pub parent: Lineage,
    /// The forked child (lineage edge + inherited genome).
    pub child: Offspring,
    /// The parent's Ed25519 signature over the snapshot digest.
    pub sig: Sig,
}

fn put(h: &mut blake3::Hasher, bytes: &[u8]) {
    h.update(&(bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

fn put_lineage(h: &mut blake3::Hasher, l: &Lineage) {
    h.update(&l.id.to_le_bytes());
    h.update(&l.generation.to_le_bytes());
    h.update(&(l.parents.len() as u64).to_le_bytes());
    for p in &l.parents {
        h.update(&p.to_le_bytes());
    }
}

/// The content digest the parent signs and that addresses the snapshot. Deterministic over every
/// field, domain-separated, length-prefixed — distinct forks never share a digest.
fn digest(parent_did: &Did, parent: &Lineage, child: &Offspring) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(FORK_DOMAIN);
    put(&mut h, parent_did.0.as_bytes());
    put_lineage(&mut h, parent);
    put_lineage(&mut h, &child.lineage);
    put(&mut h, &child.genome.canon_bytes());
    Hash(*h.finalize().as_bytes())
}

/// Fork `parent` and have `signer` (the parent) sign the resulting snapshot. The child inherits the
/// genome verbatim (variation happens later only via the closed surface); the parent's signature
/// binds that inherited state to the parent DID.
pub fn fork_signed(
    parent: &Lineage,
    parent_genome: &Genome,
    child_id: BeingId,
    signer: &dyn Signer,
) -> ForkSnapshot {
    let child = fork(parent, parent_genome, child_id);
    let d = digest(signer.did(), parent, &child);
    let sig = signer.sign(&d.0);
    ForkSnapshot {
        parent_did: signer.did().clone(),
        parent: parent.clone(),
        child,
        sig,
    }
}

impl ForkSnapshot {
    /// Content address: blake3 over the canonical snapshot fields. Stable across processes/crashes,
    /// so it is the dedup key for at-most-once commit.
    pub fn snapshot_id(&self) -> Hash {
        digest(&self.parent_did, &self.parent, &self.child)
    }

    /// Fully validate the snapshot:
    /// 1. **heredity invariants** — the child is one generation past the parent and records exactly
    ///    the parent as its sole ancestor (a forged edge is rejected without even checking crypto);
    /// 2. **signature** — the parent DID actually signed this exact content.
    ///
    /// A tampered genome, lineage edge, or DID flips the digest and fails verification.
    pub fn verify(&self) -> bool {
        let edge_ok = self.child.lineage.generation == self.parent.generation + 1
            && self.child.lineage.parents == vec![self.parent.id];
        edge_ok && verify(&self.parent_did, &self.snapshot_id().0, &self.sig)
    }
}

/// Outcome of offering a snapshot to the [`ForkLedger`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    /// Newly committed (first time this snapshot id was seen).
    Committed,
    /// Already present — replay/duplicate; committing again is a safe no-op (crash recovery).
    AlreadyCommitted,
    /// Signature or heredity invariant failed — never committed.
    Rejected,
}

/// At-most-once fork commit ledger. Rebuildable by replaying committed snapshot ids after a crash;
/// re-committing any already-seen id is idempotent, so fork application is exactly-once-effective.
#[derive(Default)]
pub struct ForkLedger {
    committed: BTreeSet<[u8; 32]>,
}

impl ForkLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.committed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.committed.is_empty()
    }

    /// Verify then record the snapshot. Returns [`CommitOutcome::Rejected`] for an invalid snapshot,
    /// [`CommitOutcome::AlreadyCommitted`] if its id was already recorded, else
    /// [`CommitOutcome::Committed`].
    pub fn commit(&mut self, snap: &ForkSnapshot) -> CommitOutcome {
        if !snap.verify() {
            return CommitOutcome::Rejected;
        }
        if self.committed.insert(snap.snapshot_id().0) {
            CommitOutcome::Committed
        } else {
            CommitOutcome::AlreadyCommitted
        }
    }

    pub fn is_committed(&self, id: &Hash) -> bool {
        self.committed.contains(&id.0)
    }
}

#[cfg(test)]
mod tests {
    use being_core_id::Ed25519Signer;
    use being_core_mutation::{apply, MutationKind};

    use super::*;

    fn parent_setup() -> (Ed25519Signer, Lineage, Genome) {
        let signer = Ed25519Signer::from_seed([7; 32]);
        let lineage = Lineage::founder(1);
        let genome = apply(MutationKind::Prompt("parent".into()), Genome::default()).unwrap();
        (signer, lineage, genome)
    }

    #[test]
    fn signed_fork_verifies_and_records_ancestry() {
        let (signer, parent, genome) = parent_setup();
        let snap = fork_signed(&parent, &genome, 2, &signer);
        assert!(snap.verify());
        assert_eq!(snap.child.lineage.generation, 1);
        assert_eq!(snap.child.lineage.parents, vec![1]);
        assert_eq!(snap.child.genome, genome); // inherited verbatim
        assert_eq!(snap.parent_did, *signer.did());
    }

    #[test]
    fn tampering_breaks_the_signature() {
        let (signer, parent, genome) = parent_setup();
        let mut snap = fork_signed(&parent, &genome, 2, &signer);
        // Tamper the inherited genome → digest changes → signature no longer matches.
        snap.child.genome = apply(
            MutationKind::Prompt("evil".into()),
            snap.child.genome.clone(),
        )
        .unwrap();
        assert!(!snap.verify());
    }

    #[test]
    fn forged_lineage_edge_is_rejected() {
        let (signer, parent, genome) = parent_setup();
        let mut snap = fork_signed(&parent, &genome, 2, &signer);
        // Claim a different generation than parent+1 → heredity invariant fails.
        snap.child.lineage.generation = 5;
        assert!(!snap.verify());
    }

    #[test]
    fn another_identity_cannot_pass_as_parent() {
        let (signer, parent, genome) = parent_setup();
        let mut snap = fork_signed(&parent, &genome, 2, &signer);
        // Swap in an impostor DID; the signature was made by the real parent over the real digest.
        snap.parent_did = Ed25519Signer::from_seed([9; 32]).did().clone();
        assert!(!snap.verify());
    }

    #[test]
    fn ledger_is_idempotent_across_replay() {
        let (signer, parent, genome) = parent_setup();
        let snap = fork_signed(&parent, &genome, 2, &signer);
        let mut ledger = ForkLedger::new();

        assert_eq!(ledger.commit(&snap), CommitOutcome::Committed);
        // Crash → replay the same snapshot: re-commit is a safe no-op (at-most-once).
        assert_eq!(ledger.commit(&snap), CommitOutcome::AlreadyCommitted);
        assert_eq!(ledger.len(), 1);
        assert!(ledger.is_committed(&snap.snapshot_id()));

        // A distinct child commits separately.
        let snap3 = fork_signed(&parent, &genome, 3, &signer);
        assert_ne!(snap.snapshot_id(), snap3.snapshot_id());
        assert_eq!(ledger.commit(&snap3), CommitOutcome::Committed);
        assert_eq!(ledger.len(), 2);
    }

    #[test]
    fn ledger_rejects_an_invalid_snapshot() {
        let (signer, parent, genome) = parent_setup();
        let mut snap = fork_signed(&parent, &genome, 2, &signer);
        snap.child.genome = apply(MutationKind::Prompt("x".into()), Genome::default()).unwrap();
        let mut ledger = ForkLedger::new();
        assert_eq!(ledger.commit(&snap), CommitOutcome::Rejected);
        assert!(ledger.is_empty());
    }
}

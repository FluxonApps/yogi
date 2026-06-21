//! `being-colony` — M6 persistence integration. Composes the pure heredity layer ([`being_lineage`])
//! with durable storage ([`being_persist`]) so fork commits survive a crash/restart, while keeping
//! `being-lineage` itself pure (no I/O). No model; loop-safe.

use std::io;
use std::path::Path;

use being_lineage::{CommitOutcome, ForkSnapshot};
use being_persist::DurableIdSet;

/// A [`being_lineage::ForkLedger`] whose committed set is **durable**: each accepted fork's
/// content-addressed `snapshot_id` is persisted via a [`DurableIdSet`], so at-most-once fork commit
/// holds across a process restart (not only as idempotent replay within one run). Reopening replays
/// the durable log to rebuild the committed set.
pub struct DurableForkLedger {
    ids: DurableIdSet,
}

impl DurableForkLedger {
    /// Open (creating if absent) a durable fork ledger at `path`, rebuilding the committed set.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self {
            ids: DurableIdSet::open(path)?,
        })
    }

    /// Verify then durably record the snapshot. `Rejected` for an invalid snapshot (bad signature or
    /// heredity edge), `AlreadyCommitted` if its id is already durable, else `Committed` (persisted +
    /// fsynced before returning).
    pub fn commit(&mut self, snap: &ForkSnapshot) -> io::Result<CommitOutcome> {
        if !snap.verify() {
            return Ok(CommitOutcome::Rejected);
        }
        Ok(if self.ids.insert(snap.snapshot_id().0)? {
            CommitOutcome::Committed
        } else {
            CommitOutcome::AlreadyCommitted
        })
    }

    pub fn is_committed(&self, snap: &ForkSnapshot) -> bool {
        self.ids.contains(&snap.snapshot_id().0)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use being_core_id::Ed25519Signer;
    use being_core_mutation::{apply, Genome, MutationKind};
    use being_lineage::{fork_signed, Lineage};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_path() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("being_colony_{}_{n}.log", std::process::id()))
    }

    #[test]
    fn fork_commits_are_durable_across_restart() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let signer = Ed25519Signer::from_seed([7; 32]);
        let parent = Lineage::founder(1);
        let genome = apply(MutationKind::Prompt("parent".into()), Genome::default()).unwrap();
        let snap = fork_signed(&parent, &genome, 2, &signer);

        {
            let mut ledger = DurableForkLedger::open(&path).unwrap();
            assert_eq!(ledger.commit(&snap).unwrap(), CommitOutcome::Committed);
            assert_eq!(
                ledger.commit(&snap).unwrap(),
                CommitOutcome::AlreadyCommitted
            );
        }
        // "Restart": a fresh ledger rebuilt from disk still knows the fork was committed.
        let mut ledger = DurableForkLedger::open(&path).unwrap();
        assert!(ledger.is_committed(&snap));
        assert_eq!(
            ledger.commit(&snap).unwrap(),
            CommitOutcome::AlreadyCommitted
        );

        // A tampered snapshot is still rejected after restart (verify runs every commit).
        let mut bad = snap.clone();
        bad.child.genome = apply(
            MutationKind::Prompt("evil".into()),
            bad.child.genome.clone(),
        )
        .unwrap();
        assert_eq!(ledger.commit(&bad).unwrap(), CommitOutcome::Rejected);

        std::fs::remove_file(&path).ok();
    }
}

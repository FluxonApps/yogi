//! `being-colony` — M6 persistence integration. Composes the pure heredity layer ([`being_lineage`])
//! with durable storage ([`being_persist`]) so fork commits survive a crash/restart, while keeping
//! `being-lineage` itself pure (no I/O). No model; loop-safe.

use std::io;
use std::path::Path;

use being_core_id::Signer;
use being_core_journal::{MemoryJournal, Seq};
use being_core_types::{Did, Hash};
use being_lineage::{CommitOutcome, ForkSnapshot};
use being_persist::{DurableIdSet, DurableLog};

// ---------------------------------------------------------------------------------------------
// Durable signed journal: the in-memory hash-chained journal made restart-survivable.
// ---------------------------------------------------------------------------------------------

/// A signed, hash-chained journal whose entries are **durable** (build-spec §5). It persists each
/// appended `(kind, payload)` to a [`DurableLog`] and, on [`open`](DurableJournal::open), rebuilds the
/// full chain by replaying those records through a fresh [`MemoryJournal`]. Because Ed25519 signing and
/// blake3 hashing are deterministic, the replayed chain is **byte-identical** (same seqs, prev-hashes,
/// entry-hashes, signatures) — so a restart recovers a journal that still passes `verify_chain`.
pub struct DurableJournal<S: Signer> {
    journal: MemoryJournal<S>,
    log: DurableLog,
}

fn encode_kv(kind: &str, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(4 + kind.len() + payload.len());
    b.extend_from_slice(&(kind.len() as u32).to_le_bytes());
    b.extend_from_slice(kind.as_bytes());
    b.extend_from_slice(payload);
    b
}

fn decode_kv(rec: &[u8]) -> Option<(String, Vec<u8>)> {
    if rec.len() < 4 {
        return None;
    }
    let klen = u32::from_le_bytes(rec[0..4].try_into().ok()?) as usize;
    let kind = String::from_utf8(rec.get(4..4 + klen)?.to_vec()).ok()?;
    let payload = rec.get(4 + klen..)?.to_vec();
    Some((kind, payload))
}

impl<S: Signer> DurableJournal<S> {
    /// Open (creating if absent) a durable journal at `path` for `signer`, rebuilding the chain from
    /// the durable log. The signer must be the same identity that wrote the log (else the rebuilt
    /// signatures won't match the original — caught by `verify_chain`).
    pub fn open(path: impl AsRef<Path>, signer: S) -> io::Result<Self> {
        let log = DurableLog::open(path)?;
        let mut journal = MemoryJournal::new(signer);
        for rec in log.replay()? {
            if let Some((kind, payload)) = decode_kv(&rec) {
                journal.append(&kind, payload);
            }
        }
        Ok(Self { journal, log })
    }

    /// Append a signed entry, **durably** (fsynced to the log) before it joins the in-memory chain, so
    /// it survives a crash. Returns the new sequence number.
    pub fn append(&mut self, kind: &str, payload: Vec<u8>) -> io::Result<Seq> {
        self.log.append(&encode_kv(kind, &payload))?; // durability point first
        Ok(self.journal.append(kind, payload))
    }

    pub fn verify_chain(&self) -> bool {
        self.journal.verify_chain()
    }

    pub fn head(&self) -> (Seq, Hash) {
        self.journal.head()
    }

    pub fn len(&self) -> usize {
        self.journal.len()
    }

    pub fn is_empty(&self) -> bool {
        self.journal.is_empty()
    }

    pub fn did(&self) -> &Did {
        self.journal.did()
    }
}

/// Construct a **durable being**: a [`being_runtime::Being`] whose signed journal is a [`DurableJournal`]
/// at `path`, so its commitment/attestation chain survives a process restart (§5). Reopening the same
/// path + signer rebuilds the being's journal from disk. This is the live-being capstone of the
/// persistence work — the runtime's `Being` plugged onto durable storage via the `Journal` seam.
#[allow(clippy::type_complexity)]
pub fn durable_being<P, C, E, S>(
    path: impl AsRef<Path>,
    signer: S,
    supervisor: std::sync::Arc<dyn being_supervisor::SupervisorPort>,
    proposer: P,
    committer: C,
    executor: E,
) -> io::Result<being_runtime::Being<P, C, E, DurableJournal<S>>>
where
    P: being_runtime::Proposer,
    C: being_runtime::Committer,
    E: being_runtime::Executor,
    S: Signer,
{
    let journal = DurableJournal::open(path, signer)?;
    Ok(being_runtime::Being::from_parts(
        journal, supervisor, proposer, committer, executor,
    ))
}

/// `DurableJournal` plugs into the runtime's [`being_core_journal::Journal`] seam, so a `Being` can be
/// constructed durable behind the same interface as the in-memory one (the §5 persistence plug-point).
impl<S: Signer> being_core_journal::Journal for DurableJournal<S> {
    fn append(&mut self, kind: &str, payload: Vec<u8>) -> io::Result<Seq> {
        DurableJournal::append(self, kind, payload)
    }
    fn verify_chain(&self) -> bool {
        DurableJournal::verify_chain(self)
    }
    fn head(&self) -> (Seq, Hash) {
        DurableJournal::head(self)
    }
    fn len(&self) -> usize {
        DurableJournal::len(self)
    }
    fn is_empty(&self) -> bool {
        DurableJournal::is_empty(self)
    }
    fn did(&self) -> &Did {
        DurableJournal::did(self)
    }
}

// ---------------------------------------------------------------------------------------------
// Durable dedup ledger: the M1 at-most-once egress guard made restart-survivable.
// ---------------------------------------------------------------------------------------------

/// A durable version of the M1 `DedupLedger` (build-spec §5): it persists each side-effecting step's
/// 36-byte [`IdemKey`](being_runtime::step_machine::IdemKey) canon, so **at-most-once** for egress /
/// payment / memory-write / sign holds across a crash/restart — after a restart the executor will not
/// re-emit an effect that was already emitted before the crash. Reopen replays the durable log.
pub struct DurableDedupLedger {
    log: DurableLog,
    seen: std::collections::BTreeSet<[u8; 36]>,
}

impl DurableDedupLedger {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let log = DurableLog::open(path)?;
        let mut seen = std::collections::BTreeSet::new();
        for rec in log.replay()? {
            if let Ok(k) = <[u8; 36]>::try_from(rec.as_slice()) {
                seen.insert(k);
            }
        }
        Ok(Self { log, seen })
    }

    /// Mark an effect's `IdemKey` as emitted, durably. Returns `true` if newly marked, `false` if it
    /// was already present (the effect already happened — do NOT re-emit). Idempotent + crash-safe.
    pub fn mark(&mut self, key: &being_runtime::step_machine::IdemKey) -> io::Result<bool> {
        let k = key.canon();
        if self.seen.contains(&k) {
            return Ok(false);
        }
        self.log.append(&k)?;
        self.seen.insert(k);
        Ok(true)
    }

    pub fn contains(&self, key: &being_runtime::step_machine::IdemKey) -> bool {
        self.seen.contains(&key.canon())
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

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

    #[test]
    fn durable_journal_survives_restart_with_identical_chain() {
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let head_before;
        {
            let mut j = DurableJournal::open(&path, Ed25519Signer::from_seed([5; 32])).unwrap();
            j.append("commitment", b"a".to_vec()).unwrap();
            j.append("attestation", b"b".to_vec()).unwrap();
            assert!(j.verify_chain());
            assert_eq!(j.len(), 2);
            head_before = j.head();
        }
        // "Restart" with the SAME signer: the chain is rebuilt from the durable log and is
        // byte-identical (deterministic Ed25519 + blake3) — same head, still verifies.
        let j = DurableJournal::open(&path, Ed25519Signer::from_seed([5; 32])).unwrap();
        assert_eq!(j.len(), 2);
        assert!(j.verify_chain());
        assert_eq!(j.head(), head_before);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn durable_being_journal_survives_restart() {
        use being_core_economy::Account;
        use being_runtime::{EchoExecutor, EchoProposer, PassThroughCommitter};
        use being_supervisor::Supervisor;
        let path = temp_path();
        let _ = std::fs::remove_file(&path);

        // A live durable being takes a couple of turns; each commitment+attestation is fsynced.
        {
            let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
            let mut being = durable_being(
                &path,
                Ed25519Signer::from_seed([8; 32]),
                Supervisor::as_port(&sup),
                EchoProposer,
                PassThroughCommitter,
                EchoExecutor,
            )
            .unwrap();
            being.turn("hello", 1);
            being.turn("again", 2);
            assert!(being.journal_len() >= 2); // commitments (+ attestations) persisted
            assert!(being.journal_verifies());
        }
        // "Restart": a fresh durable being on the same path + signer rebuilds the journal from disk —
        // the being's signed history survived the process restart and still verifies.
        let sup = Supervisor::new(Account::new(1_000_000_000, 0, 1_000_000_000), i64::MAX, 0);
        let recovered = durable_being(
            &path,
            Ed25519Signer::from_seed([8; 32]),
            Supervisor::as_port(&sup),
            EchoProposer,
            PassThroughCommitter,
            EchoExecutor,
        )
        .unwrap();
        assert!(recovered.journal_len() >= 2);
        assert!(recovered.journal_verifies());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn durable_dedup_survives_restart() {
        use being_core_types::Hash;
        use being_runtime::step_machine::IdemKey;
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let key = IdemKey::new(Hash([3u8; 32]), 1);
        {
            let mut d = DurableDedupLedger::open(&path).unwrap();
            assert!(d.mark(&key).unwrap()); // newly emitted
            assert!(!d.mark(&key).unwrap()); // already emitted → don't re-emit
        }
        // "Restart": the at-most-once guard remembers the effect already happened.
        let d = DurableDedupLedger::open(&path).unwrap();
        assert!(d.contains(&key));
        assert_eq!(d.len(), 1);
        std::fs::remove_file(&path).ok();
    }
}

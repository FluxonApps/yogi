//! `being-colony` — M6 persistence integration. Composes the pure heredity layer ([`being_lineage`])
//! with durable storage ([`being_persist`]) so fork commits survive a crash/restart, while keeping
//! `being-lineage` itself pure (no I/O). No model; loop-safe.

use std::io;
use std::path::Path;

use being_core_economy::{Account, SpendCategory};
use being_core_id::Signer;
use being_core_journal::{MemoryJournal, Seq};
use being_core_mutation::Genome;
use being_core_types::{Did, Hash, Microdollars};
use being_lineage::{fork_signed, BeingId, CommitOutcome, ForkSnapshot, Lineage};
use being_persist::{DurableIdSet, DurableLog};
use being_supervisor::{Supervisor, SupervisorPort};
use std::sync::Arc;

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

// ---------------------------------------------------------------------------------------------
// Live model-backed population: reproduction + death (the M6 next step named in CLAUDE.md).
// ---------------------------------------------------------------------------------------------

/// A live member of the population: its heredity (founding ancestor, lineage, genome) and its metabolic
/// life (a real [`Supervisor`]/Account with reaper authority).
pub struct Member {
    pub founder: BeingId,
    pub lineage: Lineage,
    pub genome: Genome,
    pub supervisor: Arc<Supervisor>,
}

/// Tunables for the population dynamics.
#[derive(Clone, Copy, Debug)]
pub struct PopulationConfig {
    /// Metabolic cost charged to each member per generation.
    pub turn_cost: Microdollars,
    /// Starting balance granted to a newborn offspring.
    pub birth_endowment: Microdollars,
    /// A member reproduces only if its balance is at least this (must be able to afford raising young).
    pub reproduce_threshold: Microdollars,
    /// Hard cap on population size (bounds compute).
    pub max_size: usize,
}

/// A live population with **reproduction and death** — the M6 step CLAUDE.md names as deliberate-next:
/// "wiring reproduction/death to a live model-backed population". Each generation: the caller supplies
/// every member's verified revenue (running its being — the live model plugs in *there*), the
/// supervisor charges metabolism and credits the revenue, **insolvent members are reaped** (real death
/// via the reaper), and **solvent members reproduce** via a signed fork committed to the durable ledger.
/// Selection is purely economic: lineages that earn more than they burn persist and spread; the rest
/// die out. The engine is loop-safe (no model) — revenue is injected, so it runs under `cargo test`.
pub struct Population<S: Signer> {
    members: Vec<Member>,
    signer: S,
    ledger: DurableForkLedger,
    cfg: PopulationConfig,
    next_id: BeingId,
    generation: u64,
}

impl<S: Signer> Population<S> {
    /// Found a population from `(founder_id, genome, starting_balance)` seeds.
    pub fn new(
        founders: impl IntoIterator<Item = (BeingId, Genome, Microdollars)>,
        signer: S,
        ledger: DurableForkLedger,
        cfg: PopulationConfig,
    ) -> Self {
        let mut members = Vec::new();
        let mut next_id: BeingId = 0;
        for (id, genome, balance) in founders {
            next_id = next_id.max(id + 1);
            members.push(Member {
                founder: id,
                lineage: Lineage::founder(id),
                genome,
                supervisor: Supervisor::new(
                    Account::new(balance, 0, Microdollars::MAX),
                    i64::MAX,
                    0,
                ),
            });
        }
        Self {
            members,
            signer,
            ledger,
            cfg,
            next_id,
            generation: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// Advance one generation. `revenue(member) -> Microdollars` is each member's verified earnings this
    /// round (caller runs the being — model or canned — and grades the output). Then charge metabolism,
    /// credit revenue, reap the insolvent, and let solvent members reproduce (signed fork) up to
    /// `max_size`.
    pub fn advance(&mut self, now_ms: i64, mut revenue: impl FnMut(&Member) -> Microdollars) {
        for m in &self.members {
            m.supervisor
                .reserve(SpendCategory::Operating, self.cfg.turn_cost, now_ms);
            let rev = revenue(m);
            if rev > 0 {
                m.supervisor.credit(rev);
            }
            m.supervisor.tick(now_ms);
        }
        // Death: reaped (insolvent) members leave the population.
        self.members.retain(|m| m.supervisor.death().is_none());

        // Reproduction: solvent members above the threshold each fork one offspring, capped at max_size.
        let parents: Vec<usize> = self
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| m.supervisor.balance() >= self.cfg.reproduce_threshold)
            .map(|(i, _)| i)
            .collect();
        for pi in parents {
            if self.members.len() >= self.cfg.max_size {
                break;
            }
            let child_id = self.next_id;
            self.next_id += 1;
            let (founder, snap) = {
                let parent = &self.members[pi];
                (
                    parent.founder,
                    fork_signed(&parent.lineage, &parent.genome, child_id, &self.signer),
                )
            };
            let _ = self.ledger.commit(&snap); // signed, durable, at-most-once heredity record
            self.members.push(Member {
                founder,
                lineage: snap.child.lineage.clone(),
                genome: snap.child.genome.clone(),
                supervisor: Supervisor::new(
                    Account::new(self.cfg.birth_endowment, 0, Microdollars::MAX),
                    i64::MAX,
                    0,
                ),
            });
        }
        self.generation += 1;
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
    fn durable_journal_recovers_from_a_crash_mid_append() {
        use std::io::Write;
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        {
            let mut j = DurableJournal::open(&path, Ed25519Signer::from_seed([6; 32])).unwrap();
            j.append("commitment", b"x".to_vec()).unwrap();
            j.append("attestation", b"y".to_vec()).unwrap();
        }
        // Crash mid-append: a torn frame (length header claiming more bytes than follow) appended to
        // the journal's underlying durable log.
        {
            let mut raw = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            raw.write_all(&(500u32).to_le_bytes()).unwrap();
            raw.write_all(&(0u32).to_le_bytes()).unwrap();
            raw.write_all(b"partial").unwrap();
            raw.sync_data().unwrap();
        }
        // Reopen: the signed chain recovers from the valid prefix, still verifies, and the torn entry
        // is gone — and the being can keep journaling afterward (contiguous, verifiable).
        let mut j = DurableJournal::open(&path, Ed25519Signer::from_seed([6; 32])).unwrap();
        assert_eq!(j.len(), 2);
        assert!(j.verify_chain());
        j.append("commitment", b"z".to_vec()).unwrap();
        let j2 = DurableJournal::open(&path, Ed25519Signer::from_seed([6; 32])).unwrap();
        assert_eq!(j2.len(), 3);
        assert!(j2.verify_chain());
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

    #[test]
    fn economic_natural_selection_earner_survives_and_reproduces_loafer_is_reaped() {
        use being_core_economy::{Account, SpendCategory};
        use being_core_id::Ed25519Signer;
        use being_core_mutation::{apply, Genome, MutationKind};
        use being_lineage::{fork_signed, Lineage};
        use being_supervisor::{Supervisor, SupervisorPort};
        use being_value::{ExternalPayer, OperatorPayer, SubstringGrader, Tariff, Treasury};

        // A being-exogenous payer: pays 100 per verified-correct answer from a bounded treasury.
        let mut payer =
            OperatorPayer::new(Tariff::new(100), SubstringGrader, Treasury::new(10_000));
        // Two beings, same small budget; each turn costs 50 operating (metabolism).
        let earner = Supervisor::new(Account::new(100, 0, 1_000_000), i64::MAX, 0);
        let loafer = Supervisor::new(Account::new(100, 0, 1_000_000), i64::MAX, 0);

        for t in 0..4 {
            earner.reserve(SpendCategory::Operating, 50, t);
            loafer.reserve(SpendCategory::Operating, 50, t);
            // Operator-side settlement of each being's output (the being can't credit itself — credit
            // is not on SupervisorPort). The earner answers correctly (+100); the loafer does not (+0).
            earner.credit(payer.settle("q", "the capital is paris", "paris"));
            loafer.credit(payer.settle("q", "i don't know", "paris"));
            earner.tick(t);
            loafer.tick(t);
        }
        // Selection by solvency: earning > metabolism keeps the earner alive; the loafer starves.
        assert!(
            earner.death().is_none(),
            "earner funded its metabolism from verified success and survived"
        );
        assert!(
            loafer.death().is_some(),
            "loafer earned nothing and was reaped for insolvency"
        );

        // The survivor reproduces: a signed fork committed to the durable ledger. The dead one does not.
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let signer = Ed25519Signer::from_seed([12; 32]);
        let parent = Lineage::founder(1);
        let child = apply(
            MutationKind::Prompt("earner-lineage".into()),
            Genome::default(),
        )
        .unwrap();
        let mut ledger = DurableForkLedger::open(&path).unwrap();
        let snap = fork_signed(&parent, &child, 2, &signer);
        assert_eq!(ledger.commit(&snap).unwrap(), CommitOutcome::Committed);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn population_selects_for_earners_over_generations() {
        use being_core_id::Ed25519Signer;
        use being_core_mutation::{apply, MutationKind};
        let path = temp_path();
        let _ = std::fs::remove_file(&path);
        let ledger = DurableForkLedger::open(&path).unwrap();
        let earner = apply(MutationKind::Prompt("earner".into()), Genome::default()).unwrap();
        let loafer = apply(MutationKind::Prompt("loafer".into()), Genome::default()).unwrap();
        let cfg = PopulationConfig {
            turn_cost: 50,
            birth_endowment: 100,
            reproduce_threshold: 200,
            max_size: 16,
        };
        // Founder 1 = earner (earns 200/gen), founder 2 = loafer (earns 0).
        let mut pop = Population::new(
            vec![(1, earner, 100), (2, loafer, 100)],
            Ed25519Signer::from_seed([13; 32]),
            ledger,
            cfg,
        );
        for t in 0..6 {
            pop.advance(t, |m| if m.founder == 1 { 200 } else { 0 });
        }
        // The loafer lineage starved to extinction; the earner lineage reproduced and fills the niche.
        assert!(!pop.is_empty());
        assert!(
            pop.members().iter().all(|m| m.founder == 1),
            "only earner-descended members survive selection"
        );
        assert!(pop.len() > 1, "the earner lineage reproduced");
        assert_eq!(pop.generation(), 6);
        std::fs::remove_file(&path).ok();
    }
}

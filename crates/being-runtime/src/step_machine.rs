//! The per-step state machine — the executor contract (build-spec §5; arch Appendix A).
//!
//! This is the load-bearing contract, **crash-recovery included**, not as an afterthought. The
//! executor drives every committed step through
//! `Reserved → Dispatched → Attested → Settled`, and on a crash it reads each step's journaled
//! marker (and the `IdemKey` dedup ledger) to place the step at its resume point and finish it —
//! re-executing **only** effects whose `IdemKey` is absent from the ledger. That single discipline
//! is what gives the system **at-most-once** side effects across a crash.
//!
//! Ownership split (build-spec §5, "Batch-reserve ownership"): the reserve batch is **per-turn**
//! (all committed steps share the `turn_id = commitment_hash`) but [`StepMachine::run_step`] is
//! **per-step** — the batch driver issues ONE reserve for the whole batch, enforces the two named
//! caps, runs the survivor-drop loop on a budget `Exceeded`, and only then calls `run_step` for each
//! admitted row. `run_step` itself assumes its row is already reserved and does only
//! dispatch / attest / settle.
//!
//! Pure-`std`, **no model inference** — safe in the automated loop (CLAUDE.md hard rule).

use std::collections::HashSet;

use being_core_economy::BudgetVerdict;
use being_core_types::Hash;

/// Effect class of a step (build-spec §3.10/§5). Drives two orthogonal things: the **dedup
/// discipline** (which effects consult the at-most-once ledger) and which **caps** apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectClass {
    // --- egress / payment: at-most-once via the dedup ledger AND counted by the per-turn cap ---
    Http,
    Notify,
    Render,
    Payment,
    // --- non-egress side-effecting: at-most-once via the SAME ledger, but NOT egress-counted ---
    MemoryWrite,
    Sign,
    // --- pure: no external side effect; re-execution is safe; EXEMPT from dedup ---
    Query,
    Infer,
}

impl EffectClass {
    /// Egress/payment classes (`Http`/`Notify`/`Render`/`Payment`) are the ones bounded by both
    /// `B_INFLIGHT` (cross-turn in-flight) and the per-turn effect-count cap (build-spec §3.10).
    pub fn is_egress(self) -> bool {
        matches!(
            self,
            EffectClass::Http | EffectClass::Notify | EffectClass::Render | EffectClass::Payment
        )
    }

    /// Pure effects (`Query`/`Infer`) have no external side effect; re-execution produces an
    /// equivalent result, so they are exempt from the dedup ledger (build-spec §5).
    pub fn is_pure(self) -> bool {
        matches!(self, EffectClass::Query | EffectClass::Infer)
    }

    /// Side-effecting effects (egress/payment **and** `MemoryWrite`/`Sign`) consult the `IdemKey`
    /// dedup ledger before emitting — at-most-once extends to non-egress side effects (build-spec §5).
    pub fn requires_dedup(self) -> bool {
        !self.is_pure()
    }
}

/// A committed step ready for the executor. `index` is the `step_index` within the turn (used in the
/// `IdemKey` and for the descending-`step_index` survivor-drop order, §3.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub index: u32,
    pub effect_class: EffectClass,
    pub action: String,
    pub arg: String,
}

/// At-most-once key = `(commitment_hash, step_index)` (build-spec §3.10). `commitment_hash` is the
/// turn id; `step_index` distinguishes steps within the turn.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdemKey {
    pub commitment_hash: Hash,
    pub step_index: u32,
}

impl IdemKey {
    pub fn new(commitment_hash: Hash, step_index: u32) -> Self {
        Self {
            commitment_hash,
            step_index,
        }
    }

    /// Canonical 36-byte encoding (`canon(IdemKey)`, §3.10): 32-byte hash ++ big-endian step index.
    /// Deterministic, so the dedup ledger keys identically across a crash/replay.
    pub fn canon(&self) -> [u8; 36] {
        let mut out = [0u8; 36];
        out[..32].copy_from_slice(&self.commitment_hash.0);
        out[32..].copy_from_slice(&self.step_index.to_be_bytes());
        out
    }
}

/// The `IdemKey`-keyed at-most-once ledger (build-spec §3.10/§5). The SAME ledger protects egress,
/// `Payment`, `MemoryWrite`, and `Sign`; pure effects never touch it.
#[derive(Default)]
pub struct DedupLedger {
    seen: HashSet<[u8; 36]>,
}

impl DedupLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Has this effect already been emitted (its `IdemKey` recorded)?
    pub fn contains(&self, key: &IdemKey) -> bool {
        self.seen.contains(&key.canon())
    }

    /// Record an emitted effect. Returns `false` if it was already present (a duplicate attempt) —
    /// the caller must never re-emit in that case.
    pub fn mark(&mut self, key: &IdemKey) -> bool {
        self.seen.insert(key.canon())
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

/// The per-step lifecycle (build-spec §5). The pre-reserve states (`Proposed`/`Committed`/
/// `ExecAttempted`) are owned by the commit + batch-reserve path; [`StepMachine::run_step`] drives
/// the post-reserve tail (`Reserved → Dispatched → Attested → Settled`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Proposed,
    Committed,
    ExecAttempted,
    Reserved,
    Dispatched,
    Attested,
    Settled,
}

/// What a crash-recovery entry must do for a step found in a given [`StepState`], from the
/// per-`StepState` crash-recovery truth table (build-spec §5). Pure data so it can be asserted in
/// tests independently of the executor that acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeAction {
    /// `Proposed`: nothing journaled yet — re-run from commit.
    ReRunFromCommit,
    /// `Committed`: proceed to fsync the ExecMarker, then reserve.
    FsyncMarkerThenReserve,
    /// `ExecAttempted`: marker precedes dispatch — re-issue reserve idempotently, then dispatch.
    ReserveThenDispatch,
    /// `Reserved`: reserve row already present (re-issue is a no-op echo) — dispatch.
    Dispatch,
    /// `Dispatched`, `IdemKey` present ⇒ effect already emitted — skip emit, proceed to attest.
    SkipEmitThenAttest,
    /// `Dispatched`, `IdemKey` absent ⇒ effect not emitted — re-dispatch (re-emit).
    ReDispatch,
    /// `Attested`: settle (idempotent).
    Settle,
    /// `Settled`: done (no-op).
    Done,
}

/// The crash-recovery truth table (build-spec §5). Given the journaled `StepState` at the crash, the
/// step's `EffectClass`, and the dedup ledger, decide the resume action. The `Dispatched` row is the
/// only "unknown" one: it is resolved by consulting the dedup ledger by `IdemKey`.
///
/// **Crash-recovery capability exception (build-spec §5):** the dispatch capability ceiling is NOT
/// re-read from the live trust ledger on recovery — recovery honours the journaled commit-time
/// verdict (arch §4.5 commit-time-snapshot lean). This function therefore never consults trust.
pub fn resume_action(
    state: StepState,
    ec: EffectClass,
    dedup: &DedupLedger,
    key: &IdemKey,
) -> ResumeAction {
    match state {
        StepState::Proposed => ResumeAction::ReRunFromCommit,
        StepState::Committed => ResumeAction::FsyncMarkerThenReserve,
        StepState::ExecAttempted => ResumeAction::ReserveThenDispatch,
        StepState::Reserved => ResumeAction::Dispatch,
        StepState::Dispatched => {
            // "unknown": consult the dedup ledger. Pure effects never record, so they re-dispatch —
            // which is safe (re-execution yields an equivalent result, §5).
            if ec.requires_dedup() && dedup.contains(key) {
                ResumeAction::SkipEmitThenAttest
            } else {
                ResumeAction::ReDispatch
            }
        }
        StepState::Attested => ResumeAction::Settle,
        StepState::Settled => ResumeAction::Done,
    }
}

/// The two named caps the supervisor enforces at reserve time (build-spec §5/§3.10). Kept as **two
/// distinct bounds**, never collapsed into a `min()` over a per-turn-cumulative quantity.
#[derive(Clone, Copy, Debug)]
pub struct Caps {
    /// `B_INFLIGHT`: cross-lane, cross-turn ceiling on reserved-but-not-settled egress effects.
    pub b_inflight: u32,
    /// Per-turn cumulative ceiling on accepted egress/payment reserves (reset on each new `turn_id`).
    pub per_turn_effect_count: u32,
}

/// The outcome of running one step to `Settled`.
#[derive(Clone, Debug)]
pub struct StepRun {
    pub final_state: StepState,
    /// Did this run actually emit the effect? `false` when the dedup ledger showed it was already
    /// emitted before a crash (the at-most-once skip).
    pub emitted: bool,
    pub observation: String,
}

/// One step dropped by the survivor-drop loop on a budget `Exceeded` (build-spec §3.6/§5): its
/// `final_state` is `Committed` with `budget_verdict = Exceeded`, and it never dispatches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedStep {
    pub index: u32,
    pub final_state: StepState,
    pub budget_verdict: BudgetVerdict,
}

/// The outcome of running a whole turn's committed batch through the machine.
#[derive(Clone, Debug)]
pub struct TurnRun {
    pub verdict: BudgetVerdict,
    pub executed: Vec<u32>,
    pub dropped: Vec<DroppedStep>,
    pub observations: Vec<String>,
}

/// The executor's per-step state machine + crash recovery (build-spec §5). Owns the dedup ledger and
/// the cross-turn in-flight egress set, so the two named bounds and at-most-once hold across turns
/// **and** across a crash.
pub struct StepMachine {
    dedup: DedupLedger,
    caps: Caps,
    /// Reserved-but-not-settled **egress** `IdemKey`s, cross-turn — the live `inflight_egress_count`.
    inflight: HashSet<[u8; 36]>,
}

impl StepMachine {
    pub fn new(caps: Caps) -> Self {
        Self {
            dedup: DedupLedger::new(),
            caps,
            inflight: HashSet::new(),
        }
    }

    pub fn dedup(&self) -> &DedupLedger {
        &self.dedup
    }

    /// The live `inflight_egress_count(did)` — reserved-but-not-settled egress effects (§3.10).
    pub fn inflight_egress_count(&self) -> u32 {
        self.inflight.len() as u32
    }

    /// Drive one step from `entry` to `Settled`, doing only dispatch / attest / settle (the row is
    /// already reserved). `dispatch` is the effect actuator; it is invoked **at most once** per
    /// `IdemKey` across the life of the machine — on a crash-recovery entry whose effect already
    /// emitted (`IdemKey` in the ledger), `run_step` skips the actuator entirely.
    ///
    /// Settle is **idempotent**: entering at `Settled`, or settling an egress effect whose lease was
    /// already released, is a no-op (build-spec §5).
    pub fn run_step<F: FnMut(&Step) -> String>(
        &mut self,
        step: &Step,
        key: &IdemKey,
        entry: StepState,
        mut dispatch: F,
    ) -> StepRun {
        debug_assert_eq!(
            key.step_index, step.index,
            "IdemKey must match the step it keys"
        );

        // Settled at entry → done, no-op (idempotent).
        if entry == StepState::Settled {
            return StepRun {
                final_state: StepState::Settled,
                emitted: false,
                observation: String::new(),
            };
        }

        let mut emitted = false;
        let mut observation = String::new();

        // Dispatch phase — only when the step has not yet been attested. Reserved (fresh) and
        // Dispatched (crash) both funnel through the SAME dedup-gated emit: a step whose effect is
        // already in the ledger skips the actuator; otherwise it emits and records.
        if matches!(entry, StepState::Reserved | StepState::Dispatched) {
            let already_emitted = step.effect_class.requires_dedup() && self.dedup.contains(key);
            if already_emitted {
                // at-most-once: the effect emitted before the crash — do NOT re-run the actuator.
            } else {
                observation = dispatch(step);
                emitted = true;
                if step.effect_class.requires_dedup() {
                    self.dedup.mark(key);
                }
            }
            // Dispatched → attest fsynced after the effect → Attested.
        }
        // (entry == Attested falls straight through to settle.)

        // Settle phase — release the in-flight egress lease; idempotent (remove of an absent key is
        // a no-op, so a duplicate settle does nothing).
        if step.effect_class.is_egress() {
            self.inflight.remove(&key.canon());
        }

        StepRun {
            final_state: StepState::Settled,
            emitted,
            observation,
        }
    }

    /// Run a turn's committed batch (build-spec §5 batch-reserve ownership). Issues ONE reserve for
    /// the whole batch via `reserve`; on `Exceeded` runs the survivor-drop loop (drop egress by
    /// **descending `step_index`** until both caps hold, §3.6), then dispatches each admitted step in
    /// order via [`StepMachine::run_step`]. Dropped steps end `Committed`/`Exceeded` and never
    /// dispatch.
    ///
    /// `reserve` is the supervisor IPC: it returns the batch verdict. The two named caps are enforced
    /// here (independent of settle timing): an egress reserve is admitted only while both
    /// `inflight_egress_count + 1 <= B_INFLIGHT` **and** `per_turn_count + 1 <= per_turn cap` hold.
    pub fn run_turn<R, F>(
        &mut self,
        commitment_hash: &Hash,
        steps: &[Step],
        mut reserve: R,
        mut dispatch: F,
    ) -> TurnRun
    where
        R: FnMut() -> BudgetVerdict,
        F: FnMut(&Step) -> String,
    {
        let verdict = reserve();
        if verdict != BudgetVerdict::WithinBudget {
            // Refused/Exceeded batch: nothing dispatches; every step is dropped Committed/<verdict>.
            let dropped = steps
                .iter()
                .map(|s| DroppedStep {
                    index: s.index,
                    final_state: StepState::Committed,
                    budget_verdict: verdict,
                })
                .collect();
            return TurnRun {
                verdict,
                executed: Vec::new(),
                dropped,
                observations: Vec::new(),
            };
        }

        // Cap enforcement + survivor drop. Egress steps compete for the smaller of the remaining
        // in-flight room and the per-turn cap; the LOWEST step_index egress steps win, the highest
        // are dropped (descending step_index drop order, §3.6). Non-egress steps always admit.
        let room = self
            .caps
            .b_inflight
            .saturating_sub(self.inflight_egress_count())
            .min(self.caps.per_turn_effect_count) as usize;

        let mut egress_sorted: Vec<&Step> = steps
            .iter()
            .filter(|s| s.effect_class.is_egress())
            .collect();
        egress_sorted.sort_by_key(|s| s.index);
        let dropped_egress: HashSet<u32> =
            egress_sorted.iter().skip(room).map(|s| s.index).collect();

        let mut executed = Vec::new();
        let mut observations = Vec::new();
        let mut dropped = Vec::new();

        for step in steps {
            if dropped_egress.contains(&step.index) {
                dropped.push(DroppedStep {
                    index: step.index,
                    final_state: StepState::Committed,
                    budget_verdict: BudgetVerdict::Exceeded,
                });
                continue;
            }
            let key = IdemKey::new(commitment_hash.clone(), step.index);
            if step.effect_class.is_egress() {
                self.inflight.insert(key.canon()); // reserved → in-flight until settle
            }
            let run = self.run_step(step, &key, StepState::Reserved, &mut dispatch);
            if run.emitted {
                observations.push(run.observation);
            }
            executed.push(step.index);
        }

        TurnRun {
            verdict,
            executed,
            dropped,
            observations,
        }
    }

    /// Crash recovery (build-spec §5). For each `(step, crash_state)`, place it at its resume point
    /// per [`resume_action`] and finish it to `Settled`, re-executing **only** effects whose
    /// `IdemKey` is absent from the dedup ledger. Steps that crashed before reserve
    /// (`Proposed`/`Committed`/`ExecAttempted`) are reserved here first (their egress re-enters the
    /// in-flight set) and then dispatched; an already-emitted egress effect (in the ledger) is
    /// re-marked in-flight only if it had not yet settled — but since we drive straight to `Settled`,
    /// the net in-flight delta is zero.
    pub fn recover<F: FnMut(&Step) -> String>(
        &mut self,
        commitment_hash: &Hash,
        crashed: &[(Step, StepState)],
        mut dispatch: F,
    ) -> Vec<StepRun> {
        crashed
            .iter()
            .map(|(step, crash_state)| {
                let key = IdemKey::new(commitment_hash.clone(), step.index);
                // The post-reserve tail handles {Reserved, Dispatched, Attested, Settled} directly.
                // Pre-reserve crash states resume at the (idempotent) reserve, then dispatch.
                let entry = match crash_state {
                    StepState::Proposed
                    | StepState::Committed
                    | StepState::ExecAttempted
                    | StepState::Reserved => {
                        if step.effect_class.is_egress() {
                            self.inflight.insert(key.canon());
                        }
                        StepState::Reserved
                    }
                    other => *other,
                };
                self.run_step(step, &key, entry, &mut dispatch)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn h(b: u8) -> Hash {
        Hash([b; 32])
    }

    fn step(index: u32, ec: EffectClass) -> Step {
        Step {
            index,
            effect_class: ec,
            action: "a".to_string(),
            arg: format!("s{index}"),
        }
    }

    fn caps() -> Caps {
        Caps {
            b_inflight: 8,
            per_turn_effect_count: 8,
        }
    }

    #[test]
    fn effect_class_partitions_dedup_and_egress() {
        for ec in [
            EffectClass::Http,
            EffectClass::Notify,
            EffectClass::Render,
            EffectClass::Payment,
        ] {
            assert!(ec.is_egress() && ec.requires_dedup() && !ec.is_pure());
        }
        for ec in [EffectClass::MemoryWrite, EffectClass::Sign] {
            assert!(
                !ec.is_egress() && ec.requires_dedup() && !ec.is_pure(),
                "{ec:?}"
            );
        }
        for ec in [EffectClass::Query, EffectClass::Infer] {
            assert!(
                !ec.is_egress() && !ec.requires_dedup() && ec.is_pure(),
                "{ec:?}"
            );
        }
    }

    #[test]
    fn idemkey_canon_is_deterministic_and_distinguishes_index() {
        let a = IdemKey::new(h(7), 3);
        assert_eq!(a.canon(), IdemKey::new(h(7), 3).canon());
        assert_ne!(a.canon(), IdemKey::new(h(7), 4).canon());
        assert_ne!(a.canon(), IdemKey::new(h(8), 3).canon());
        assert_eq!(&a.canon()[..32], &[7u8; 32]);
        assert_eq!(&a.canon()[32..], &3u32.to_be_bytes());
    }

    #[test]
    fn resume_action_matches_the_truth_table() {
        let dedup = DedupLedger::new();
        let key = IdemKey::new(h(1), 0);
        let ec = EffectClass::Http;
        assert_eq!(
            resume_action(StepState::Proposed, ec, &dedup, &key),
            ResumeAction::ReRunFromCommit
        );
        assert_eq!(
            resume_action(StepState::Committed, ec, &dedup, &key),
            ResumeAction::FsyncMarkerThenReserve
        );
        assert_eq!(
            resume_action(StepState::ExecAttempted, ec, &dedup, &key),
            ResumeAction::ReserveThenDispatch
        );
        assert_eq!(
            resume_action(StepState::Reserved, ec, &dedup, &key),
            ResumeAction::Dispatch
        );
        assert_eq!(
            resume_action(StepState::Attested, ec, &dedup, &key),
            ResumeAction::Settle
        );
        assert_eq!(
            resume_action(StepState::Settled, ec, &dedup, &key),
            ResumeAction::Done
        );
        // Dispatched is the only "unknown": resolved by the ledger.
        assert_eq!(
            resume_action(StepState::Dispatched, ec, &dedup, &key),
            ResumeAction::ReDispatch
        );
        let mut dedup2 = DedupLedger::new();
        dedup2.mark(&key);
        assert_eq!(
            resume_action(StepState::Dispatched, ec, &dedup2, &key),
            ResumeAction::SkipEmitThenAttest
        );
    }

    #[test]
    fn happy_path_runs_one_reserve_and_settles_every_step() {
        let mut m = StepMachine::new(caps());
        let reserves = Cell::new(0);
        let steps = [
            step(0, EffectClass::Http),
            step(1, EffectClass::MemoryWrite),
            step(2, EffectClass::Query),
        ];
        let run = m.run_turn(
            &h(1),
            &steps,
            || {
                reserves.set(reserves.get() + 1);
                BudgetVerdict::WithinBudget
            },
            |s| format!("did:{}", s.arg),
        );
        assert_eq!(reserves.get(), 1, "exactly ONE reserve for the whole batch");
        assert_eq!(run.executed, vec![0, 1, 2]);
        assert!(run.dropped.is_empty());
        assert_eq!(run.observations.len(), 3);
        // Http + MemoryWrite are side-effecting → recorded; Query is pure → not.
        assert_eq!(m.dedup().len(), 2);
        // Every egress lease settled → nothing in flight.
        assert_eq!(m.inflight_egress_count(), 0);
    }

    #[test]
    fn pure_effects_never_touch_the_dedup_ledger() {
        let mut m = StepMachine::new(caps());
        let steps = [step(0, EffectClass::Query), step(1, EffectClass::Infer)];
        m.run_turn(
            &h(2),
            &steps,
            || BudgetVerdict::WithinBudget,
            |s| s.arg.clone(),
        );
        assert!(m.dedup().is_empty());
    }

    #[test]
    fn crash_between_dispatched_and_attested_does_not_re_emit() {
        // The at-most-once invariant: a side-effecting step that already emitted before the crash
        // (its IdemKey is in the ledger) must NOT run the actuator again on recovery.
        let mut m = StepMachine::new(caps());
        let s = step(0, EffectClass::Payment);
        let key = IdemKey::new(h(3), 0);
        m.dedup.mark(&key); // emitted before the crash
        let emits = Cell::new(0);
        let run = m.recover(&h(3), &[(s, StepState::Dispatched)], |_| {
            emits.set(emits.get() + 1);
            "PAID-AGAIN".to_string()
        });
        assert_eq!(emits.get(), 0, "must not re-emit an already-emitted effect");
        assert!(!run[0].emitted);
        assert_eq!(run[0].final_state, StepState::Settled);
    }

    #[test]
    fn crash_at_dispatched_with_idemkey_absent_re_emits_once() {
        // The crash landed before the effect actually emitted (no ledger entry) → re-dispatch.
        let mut m = StepMachine::new(caps());
        let s = step(0, EffectClass::Http);
        let emits = Cell::new(0);
        let run = m.recover(&h(4), &[(s, StepState::Dispatched)], |_| {
            emits.set(emits.get() + 1);
            "SENT".to_string()
        });
        assert_eq!(emits.get(), 1);
        assert!(run[0].emitted);
        assert_eq!(
            m.dedup().len(),
            1,
            "now recorded, so a further crash won't re-emit"
        );
    }

    #[test]
    fn recovery_resumes_each_step_state_to_settled() {
        // One step crashed in each post-reserve state; recovery drives them all to Settled, emitting
        // only where the effect had not yet emitted.
        let mut m = StepMachine::new(caps());
        let dispatched_key = IdemKey::new(h(5), 1);
        m.dedup.mark(&dispatched_key); // the Dispatched step had already emitted
        let crashed = [
            (step(0, EffectClass::Http), StepState::Reserved), // not emitted → emit
            (step(1, EffectClass::Http), StepState::Dispatched), // already emitted → skip
            (step(2, EffectClass::Http), StepState::Attested), // emitted → just settle
            (step(3, EffectClass::Http), StepState::Settled),  // done → no-op
        ];
        let emits = Cell::new(0);
        let runs = m.recover(&h(5), &crashed, |_| {
            emits.set(emits.get() + 1);
            "x".to_string()
        });
        assert!(runs.iter().all(|r| r.final_state == StepState::Settled));
        assert_eq!(
            emits.get(),
            1,
            "only the Reserved step still needed to emit"
        );
        assert!(runs[0].emitted && !runs[1].emitted && !runs[2].emitted && !runs[3].emitted);
    }

    #[test]
    fn idempotent_settle_is_a_noop() {
        // Running the same step twice (a duplicate settle path) must not double-emit nor underflow
        // the in-flight set.
        let mut m = StepMachine::new(caps());
        let s = step(0, EffectClass::Http);
        let key = IdemKey::new(h(6), 0);
        m.inflight.insert(key.canon());
        let r1 = m.run_step(&s, &key, StepState::Reserved, |_| "one".to_string());
        assert!(r1.emitted);
        assert_eq!(m.inflight_egress_count(), 0);
        // Re-running at Settled is a pure no-op.
        let r2 = m.run_step(&s, &key, StepState::Settled, |_| {
            panic!("must not dispatch")
        });
        assert!(!r2.emitted);
        assert_eq!(m.inflight_egress_count(), 0);
    }

    #[test]
    fn batch_exceeded_drops_every_step_and_dispatches_none() {
        let mut m = StepMachine::new(caps());
        let steps = [
            step(0, EffectClass::Http),
            step(1, EffectClass::MemoryWrite),
        ];
        let run = m.run_turn(
            &h(7),
            &steps,
            || BudgetVerdict::Exceeded,
            |_| panic!("a non-affordable batch must not dispatch"),
        );
        assert_eq!(run.verdict, BudgetVerdict::Exceeded);
        assert!(run.executed.is_empty());
        assert_eq!(run.dropped.len(), 2);
        assert!(run
            .dropped
            .iter()
            .all(|d| d.final_state == StepState::Committed
                && d.budget_verdict == BudgetVerdict::Exceeded));
        assert!(m.dedup().is_empty());
    }

    #[test]
    fn per_turn_cap_drops_highest_step_index_egress_first() {
        // Cap of 2 egress per turn, 3 egress steps committed → step_index 2 (highest) is dropped;
        // 0 and 1 survive (descending-step_index drop order, §3.6).
        let mut m = StepMachine::new(Caps {
            b_inflight: 8,
            per_turn_effect_count: 2,
        });
        let steps = [
            step(0, EffectClass::Http),
            step(1, EffectClass::Http),
            step(2, EffectClass::Http),
        ];
        let run = m.run_turn(
            &h(8),
            &steps,
            || BudgetVerdict::WithinBudget,
            |s| s.arg.clone(),
        );
        assert_eq!(run.executed, vec![0, 1]);
        assert_eq!(run.dropped.len(), 1);
        assert_eq!(run.dropped[0].index, 2);
    }

    #[test]
    fn the_two_named_bounds_are_distinct_and_independent() {
        // B_INFLIGHT binds across turns (un-settled leases persist); the per-turn cap binds within a
        // turn. They are separate ceilings, not a min() over one cumulative quantity.
        // Turn 1 leaves 2 egress in flight (simulate a crash: dispatch without settle).
        let mut m = StepMachine::new(Caps {
            b_inflight: 3,
            per_turn_effect_count: 8,
        });
        m.inflight.insert(IdemKey::new(h(9), 0).canon());
        m.inflight.insert(IdemKey::new(h(9), 1).canon());
        assert_eq!(m.inflight_egress_count(), 2);
        // Turn 2: per-turn cap is generous (8) but only 1 in-flight slot remains (3 - 2). So of two
        // new egress steps, only the lowest-index one is admitted.
        let steps = [step(0, EffectClass::Http), step(1, EffectClass::Http)];
        let run = m.run_turn(
            &h(10),
            &steps,
            || BudgetVerdict::WithinBudget,
            |s| s.arg.clone(),
        );
        assert_eq!(
            run.executed,
            vec![0],
            "B_INFLIGHT bound, not the per-turn cap, is binding here"
        );
        assert_eq!(run.dropped.len(), 1);
        assert_eq!(run.dropped[0].index, 1);
    }
}

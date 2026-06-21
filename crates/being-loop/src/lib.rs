//! Bounded self-modification — the Two-Gate loop (build-spec §8; D-M4-1).
//!
//! An [`Improver`] **proposes** a closed-surface [`MutationKind`]; acceptance is **pure deterministic
//! machinery over recorded bench results — no model inference in the loop** (the proposing model is
//! foreground). A proposed edit commits only if it clears BOTH gates, and is fully reversible (a
//! rejected edit simply leaves the genome unchanged; the caller keeps the frozen last-known-good):
//!
//! - [`ValidationGate`] — the arXiv 2510.04399 rule: accept iff the candidate beats the incumbent by
//!   more than `2·eps_V + tau` over the *same* cases (`eps_V = sqrt((K + ln(1/delta))/n)`). This
//!   rejects noise-level gains; small benches cannot certify improvement, by design.
//! - [`CapacityCaps`] — the declarative capacity proxy the paper leaves open: bound prompt size,
//!   skill count, domain-model count, and policy-blob sizes. The closed, non-`#[non_exhaustive]`
//!   `MutationKind` is the hard outer bound; this keeps the reachable family PAC-bounded.
//!
//! Bias is avoided by construction: the verifier is a pass/fail bench, never an LLM judge in the
//! acceptance path. `tau`, the caps, and the (currently flat) capacity schedule are human-reviewed.

use being_core_mutation::{apply, Genome, MutationError, MutationKind};

fn mean_bool(xs: &[bool]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().filter(|&&b| b).count() as f64 / xs.len() as f64
    }
}

// --- Validation Gate -------------------------------------------------------------------------

/// Statistical acceptance: an edit must beat the incumbent by more than the noise floor + margin.
#[derive(Clone, Debug)]
pub struct ValidationGate {
    /// Stability margin beyond the noise floor (human-reviewed).
    pub tau: f64,
    /// Confidence parameter for the uniform-convergence bound.
    pub delta: f64,
    /// Capacity term `K` in `eps = sqrt((K + ln(1/delta))/n)` (the noise-floor capacity, distinct
    /// from the structural [`CapacityCaps`]).
    pub k: f64,
}

impl ValidationGate {
    /// Conservative defaults (D-M4-1, human-approved): tau 0.05, delta 0.05, K 2.0.
    pub fn conservative() -> Self {
        Self {
            tau: 0.05,
            delta: 0.05,
            k: 2.0,
        }
    }

    pub fn eps(&self, n: usize) -> f64 {
        if n == 0 {
            return f64::INFINITY;
        }
        ((self.k + (1.0 / self.delta).ln()) / n as f64).sqrt()
    }

    /// Accept iff `mean(candidate) - mean(incumbent) > 2*eps(n) + tau` over the same `n` cases.
    pub fn accept(&self, incumbent: &[bool], candidate: &[bool]) -> bool {
        if incumbent.len() != candidate.len() || incumbent.is_empty() {
            return false;
        }
        mean_bool(candidate) - mean_bool(incumbent) > 2.0 * self.eps(incumbent.len()) + self.tau
    }
}

// --- Capacity Gate ---------------------------------------------------------------------------

/// Flat, conservative structural caps on the declarative genome (D-M4-1; no growth schedule yet).
#[derive(Clone, Debug)]
pub struct CapacityCaps {
    pub max_prompt_bytes: usize,
    pub max_skills: usize,
    pub max_domain_models: usize,
    pub max_policy_bytes: usize,
}

impl CapacityCaps {
    pub fn conservative() -> Self {
        Self {
            max_prompt_bytes: 4096,
            max_skills: 32,
            max_domain_models: 64,
            max_policy_bytes: 4096,
        }
    }

    /// Whether `g` is within all caps (the computable capacity proxy `B[genome] <= K`).
    pub fn within(&self, g: &Genome) -> bool {
        g.prompt.len() <= self.max_prompt_bytes
            && g.installed_skills.len() <= self.max_skills
            && g.domain_models.len() <= self.max_domain_models
            && g.tool_policy.len() <= self.max_policy_bytes
            && g.retrieval_policy.len() <= self.max_policy_bytes
            && g.decomposition_policy.len() <= self.max_policy_bytes
            && g.routing_policy.len() <= self.max_policy_bytes
    }
}

// --- The Two-Gate ----------------------------------------------------------------------------

/// Outcome of evaluating a proposed mutation through both gates. Anything but `Accepted` leaves the
/// genome unchanged (rollback is implicit; the caller keeps the prior genome).
#[derive(Clone, Debug, PartialEq)]
pub enum GateOutcome {
    Accepted { genome: Genome, delta: f64 },
    RejectedCapacity,
    RejectedValidation,
    ApplyError(MutationError),
}

/// The Validation + Capacity gate pair.
pub struct TwoGate {
    pub validation: ValidationGate,
    pub caps: CapacityCaps,
}

impl TwoGate {
    /// Evaluate `kind` against `current`, given the incumbent's and candidate's per-case bench
    /// results over the SAME cases. Order: apply → Capacity → Validation (capacity is the
    /// learnability-preserving bound and is checked even when validation would pass).
    pub fn evaluate(
        &self,
        current: &Genome,
        kind: MutationKind,
        incumbent: &[bool],
        candidate: &[bool],
    ) -> GateOutcome {
        let new_genome = match apply(kind, current.clone()) {
            Ok(g) => g,
            Err(e) => return GateOutcome::ApplyError(e),
        };
        if !self.caps.within(&new_genome) {
            return GateOutcome::RejectedCapacity;
        }
        if !self.validation.accept(incumbent, candidate) {
            return GateOutcome::RejectedValidation;
        }
        let delta = mean_bool(candidate) - mean_bool(incumbent);
        GateOutcome::Accepted {
            genome: new_genome,
            delta,
        }
    }
}

// --- Audit log -------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub summary: String,
    pub accepted: bool,
    pub delta: f64,
}

/// Append-only record of every proposed/applied self-edit (D-M4-1 audit + rollback support).
#[derive(Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, summary: impl Into<String>, accepted: bool, delta: f64) {
        self.entries.push(AuditEntry {
            summary: summary.into(),
            accepted,
            delta,
        });
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// --- Improver --------------------------------------------------------------------------------

/// Chooses which candidate mutation to try, and learns from outcomes. The candidate payloads come
/// from the caller (a foreground model proposes them); the Improver only *selects* among them.
pub trait Improver {
    fn choose(&mut self, arms: &[MutationKind]) -> usize;
    fn record(&mut self, arm: usize, accepted: bool, delta: f64);
}

struct Xorshift64(u64);
impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[derive(Clone, Copy, Default)]
struct ArmStat {
    count: u64,
    mean: f64,
}

/// Epsilon-greedy arm selection (deterministic given the seed). Explores with probability `epsilon`,
/// otherwise exploits the highest mean reward (ties → lowest index).
pub struct EpsilonGreedyImprover {
    epsilon: f64,
    rng: Xorshift64,
    stats: Vec<ArmStat>,
}

impl EpsilonGreedyImprover {
    pub fn new(epsilon: f64, seed: u64) -> Self {
        Self {
            epsilon,
            rng: Xorshift64::new(seed),
            stats: Vec::new(),
        }
    }

    fn ensure(&mut self, n: usize) {
        while self.stats.len() < n {
            self.stats.push(ArmStat::default());
        }
    }
}

impl Improver for EpsilonGreedyImprover {
    fn choose(&mut self, arms: &[MutationKind]) -> usize {
        if arms.is_empty() {
            return 0;
        }
        self.ensure(arms.len());
        let r = (self.rng.next() % 1_000_000) as f64 / 1_000_000.0;
        if r < self.epsilon {
            (self.rng.next() as usize) % arms.len()
        } else {
            self.stats[..arms.len()]
                .iter()
                .enumerate()
                .fold((0usize, f64::NEG_INFINITY), |(bi, bm), (i, s)| {
                    if s.mean > bm {
                        (i, s.mean)
                    } else {
                        (bi, bm)
                    }
                })
                .0
        }
    }

    fn record(&mut self, arm: usize, accepted: bool, delta: f64) {
        self.ensure(arm + 1);
        let reward = if accepted { delta.max(0.0) } else { 0.0 };
        let s = &mut self.stats[arm];
        s.count += 1;
        s.mean += (reward - s.mean) / s.count as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_accepts_clear_improvement() {
        let g = ValidationGate::conservative();
        assert!(g.accept(&[false; 100], &[true; 100]));
    }

    #[test]
    fn validation_rejects_noise_and_small_gains() {
        let g = ValidationGate::conservative();
        let inc: Vec<bool> = (0..100).map(|i| i < 50).collect(); // mean 0.50
        assert!(!g.accept(&inc, &inc)); // identical → no gain
        let cand: Vec<bool> = (0..100).map(|i| i < 55).collect(); // mean 0.55 (+0.05, within noise)
        assert!(!g.accept(&inc, &cand));
    }

    #[test]
    fn capacity_rejects_oversize_prompt() {
        let caps = CapacityCaps::conservative();
        let mut g = Genome::default();
        assert!(caps.within(&g));
        g.prompt = "x".repeat(5000);
        assert!(!caps.within(&g));
    }

    fn two_gate() -> TwoGate {
        TwoGate {
            validation: ValidationGate::conservative(),
            caps: CapacityCaps::conservative(),
        }
    }

    #[test]
    fn two_gate_accepts_good_edit_within_caps() {
        let out = two_gate().evaluate(
            &Genome::default(),
            MutationKind::Prompt("hello".into()),
            &[false; 100],
            &[true; 100],
        );
        match out {
            GateOutcome::Accepted { genome, delta } => {
                assert_eq!(genome.prompt, "hello");
                assert!(delta > 0.9);
            }
            other => panic!("expected Accepted, got {other:?}"),
        }
    }

    #[test]
    fn two_gate_rejects_capacity_breach() {
        let out = two_gate().evaluate(
            &Genome::default(),
            MutationKind::Prompt("x".repeat(5000)),
            &[false; 100],
            &[true; 100],
        );
        assert_eq!(out, GateOutcome::RejectedCapacity);
    }

    #[test]
    fn two_gate_rejects_unimproved_edit() {
        let inc: Vec<bool> = (0..100).map(|i| i < 50).collect();
        let out = two_gate().evaluate(
            &Genome::default(),
            MutationKind::Prompt("hi".into()),
            &inc,
            &inc,
        );
        assert_eq!(out, GateOutcome::RejectedValidation);
    }

    #[test]
    fn two_gate_surfaces_apply_errors() {
        let out = two_gate().evaluate(
            &Genome::default(),
            MutationKind::Prompt("   ".into()), // empty after trim
            &[false; 100],
            &[true; 100],
        );
        assert!(matches!(out, GateOutcome::ApplyError(_)));
    }

    #[test]
    fn epsilon_greedy_exploits_the_rewarded_arm() {
        let arms = [
            MutationKind::Prompt("a".into()),
            MutationKind::Prompt("b".into()),
        ];
        let mut imp = EpsilonGreedyImprover::new(0.0, 42); // pure exploit
        imp.record(1, true, 1.0);
        assert_eq!(imp.choose(&arms), 1);
    }

    #[test]
    fn audit_log_records_edits() {
        let mut log = AuditLog::new();
        log.record("Prompt edit", true, 0.3);
        log.record("rejected edit", false, 0.0);
        assert_eq!(log.len(), 2);
        assert!(log.entries()[0].accepted);
        assert!(!log.entries()[1].accepted);
    }
}

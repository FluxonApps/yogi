//! The runtime seam + control loop, now metabolic (build-spec §3.4/§3.5; architecture §4; D-M1-*).
//!
//! The **proposer** generates; the **committer** is a separate gate; the **executor** acts; and the
//! **supervisor** (held only as an `Arc<dyn SupervisorPort>`) owns the budget + reaper. The seam is
//! propose → reserve → commit → attest, every commitment/attestation is appended to the signed
//! hash-chained journal, and a turn whose operating cost bankrupts the being trips the reaper
//! mid-operation (D-M1-1): it does not act, and every later turn is refused.
//!
//! M0 wired this with an echo proposer + pass-through committer; M1 adds the live Account/reaper via
//! the supervisor. The committer still owns policy; budget authority is the supervisor's.

use std::sync::Arc;

pub mod step_machine;

use being_core_economy::{BudgetVerdict, SpendCategory};
use being_core_id::Ed25519Signer;
use being_core_journal::{MemoryJournal, Seq};
use being_core_memory::{Embedder, EpisodicStore, Ms, SemanticIndex};
use being_supervisor::SupervisorPort;

// --- Seam types ------------------------------------------------------------------------------

/// Assembled per-turn context handed to the proposer.
pub struct ContextPack {
    pub input: String,
    pub retrieved: Vec<String>,
}

/// One concrete step in a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    pub action: String,
    pub arg: String,
}

/// Proposer output: candidate steps + a preferred index. A suggestion, never a commitment.
#[derive(Clone, Debug)]
pub struct Proposal {
    pub intent: String,
    pub candidate_steps: Vec<PlanStep>,
    pub preferred: usize,
    pub est_cost: i64,
}

/// Committer output: the decisive cut. `budget_verdict` is filled by the supervisor's reservation.
#[derive(Clone, Debug)]
pub struct Commitment {
    pub committed_steps: Vec<PlanStep>,
    pub rejected: Vec<(PlanStep, String)>,
    pub continue_loop: bool,
    pub budget_verdict: BudgetVerdict,
}

/// The model. Generative; never touches actuators.
pub trait Proposer {
    fn propose(&mut self, ctx: &ContextPack) -> Proposal;
}

/// The commitment layer. Deterministic policy gate; decides what runs and whether to loop. (Budget
/// authority is the supervisor's, not the committer's.)
pub trait Committer {
    fn commit(&mut self, proposal: &Proposal, ctx: &ContextPack) -> Commitment;
}

/// The actuator. Runs a committed step, returns an observation.
pub trait Executor {
    fn execute(&mut self, step: &PlanStep) -> String;
}

// --- M0/M1 implementations -------------------------------------------------------------------

/// Deterministic echo proposer (stands in for the LLM until the Ollama proposer at M2).
pub struct EchoProposer;
impl Proposer for EchoProposer {
    fn propose(&mut self, ctx: &ContextPack) -> Proposal {
        Proposal {
            intent: format!("echo: {}", ctx.input),
            candidate_steps: vec![PlanStep {
                action: "echo".to_string(),
                arg: ctx.input.clone(),
            }],
            preferred: 0,
            est_cost: ctx.input.len() as i64,
        }
    }
}

/// Pass-through policy committer: commits every candidate step (budget is the supervisor's job, filled
/// into `budget_verdict` by the turn). Use [`RiskPolicyCommitter`] for a being-level risk gate.
pub struct PassThroughCommitter;
impl Committer for PassThroughCommitter {
    fn commit(&mut self, proposal: &Proposal, _ctx: &ContextPack) -> Commitment {
        Commitment {
            committed_steps: proposal.candidate_steps.clone(),
            rejected: Vec::new(),
            continue_loop: false,
            budget_verdict: BudgetVerdict::WithinBudget,
        }
    }
}

/// Risk tier of a step's effect, ordered lowest → highest. Pure effects carry no external authority;
/// the rest escalate by blast radius (memory write < egress < sign < payment).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskTier {
    Pure,
    MemoryWrite,
    Egress,
    Sign,
    Payment,
}

/// Classify a step's risk by its action — **fail-closed**: an unrecognized action is treated as the
/// highest tier, so an unknown effect is only ever committed by a being that tolerates `Payment`-level
/// risk. (Mirrors `classify_effect`'s fail-closed stance, but for the commit-stage policy gate.)
pub fn step_risk(step: &PlanStep) -> RiskTier {
    match step.action.as_str() {
        "respond" | "error" | "" => RiskTier::Pure,
        "memory_write" | "remember" => RiskTier::MemoryWrite,
        "egress" | "http" | "fetch" => RiskTier::Egress,
        "sign" => RiskTier::Sign,
        _ => RiskTier::Payment, // "pay"/"payment" and anything unrecognized → highest (fail-closed)
    }
}

/// The real trust+risk policy gate (the deferred committer): commits only steps **at or below** a
/// configured risk `ceiling`; higher-risk steps are refused and recorded in `Commitment.rejected` with
/// a reason. This is the being's OWN self-restraint — defense-in-depth *upstream* of (and independent
/// from) the operator capability sandbox: e.g. a `Pure`-ceiling being is read-only by its own policy
/// even if granted egress capability, so a prompt-injected "fetch evil.test" never reaches the broker.
pub struct RiskPolicyCommitter {
    pub ceiling: RiskTier,
}

impl RiskPolicyCommitter {
    pub fn new(ceiling: RiskTier) -> Self {
        Self { ceiling }
    }
}

impl Committer for RiskPolicyCommitter {
    fn commit(&mut self, proposal: &Proposal, _ctx: &ContextPack) -> Commitment {
        let mut committed_steps = Vec::new();
        let mut rejected = Vec::new();
        for step in &proposal.candidate_steps {
            let risk = step_risk(step);
            if risk <= self.ceiling {
                committed_steps.push(step.clone());
            } else {
                rejected.push((
                    step.clone(),
                    format!(
                        "step risk {risk:?} exceeds the being's ceiling {:?}",
                        self.ceiling
                    ),
                ));
            }
        }
        Commitment {
            committed_steps,
            rejected,
            continue_loop: false,
            budget_verdict: BudgetVerdict::WithinBudget,
        }
    }
}

/// Echo executor: returns the step echoed back.
pub struct EchoExecutor;
impl Executor for EchoExecutor {
    fn execute(&mut self, step: &PlanStep) -> String {
        format!("{}:{}", step.action, step.arg)
    }
}

// --- M4 isolation: capability-gated executor -------------------------------------------------

/// Map a committed [`PlanStep`] to the [`EffectRequest`] it would perform. The M1 runtime emits only
/// pure response steps; effectful actions are classified explicitly here and **anything unrecognized
/// is treated as effectful and denied** (fail-closed) rather than slipping through as pure.
pub fn classify_effect(step: &PlanStep) -> being_sandbox::EffectRequest {
    use being_sandbox::EffectRequest;
    match step.action.as_str() {
        "respond" | "error" | "" => EffectRequest::Query,
        "egress" | "http" | "fetch" => EffectRequest::Egress {
            host: step.arg.clone(),
        },
        "pay" | "payment" => EffectRequest::Payment {
            microdollars: step.arg.parse().unwrap_or(i64::MAX),
        },
        "memory_write" | "remember" => EffectRequest::MemoryWrite,
        "sign" => EffectRequest::Sign,
        // Fail-closed: an unrecognized action is treated as egress to an unknown host, so it is denied
        // unless the operator explicitly granted it (never silently allowed as "pure").
        _ => EffectRequest::Egress {
            host: format!("unclassified:{}", step.action),
        },
    }
}

/// Wraps any [`Executor`] so every step is authorized by the capability [`being_sandbox::Broker`]
/// **before** it runs (M4 isolation policy, in-process). A denied step never reaches the inner
/// executor — it yields a `[denied:<reason>]` observation instead. Compose this around the real
/// executor to enforce deny-by-default capabilities on the live turn; the wasmtime backend
/// (`being-sandbox-wasm`) is the out-of-process hardening of the same policy.
pub struct SandboxedExecutor<E: Executor> {
    inner: E,
    caps: being_sandbox::CapabilitySet,
    classify: fn(&PlanStep) -> being_sandbox::EffectRequest,
}

impl<E: Executor> SandboxedExecutor<E> {
    /// Gate `inner` with the operator-granted `caps`, using [`classify_effect`].
    pub fn new(inner: E, caps: being_sandbox::CapabilitySet) -> Self {
        Self {
            inner,
            caps,
            classify: classify_effect,
        }
    }

    /// As [`SandboxedExecutor::new`] but with a custom step→effect classifier.
    pub fn with_classifier(
        inner: E,
        caps: being_sandbox::CapabilitySet,
        classify: fn(&PlanStep) -> being_sandbox::EffectRequest,
    ) -> Self {
        Self {
            inner,
            caps,
            classify,
        }
    }
}

impl<E: Executor> Executor for SandboxedExecutor<E> {
    fn execute(&mut self, step: &PlanStep) -> String {
        match being_sandbox::Broker::authorize(&(self.classify)(step), &self.caps) {
            being_sandbox::Authorization::Granted => self.inner.execute(step),
            being_sandbox::Authorization::Denied(reason) => format!("[denied:{reason}]"),
        }
    }
}

// --- Deterministic journal payload encoding --------------------------------------------------

fn encode_commitment(c: &Commitment) -> Vec<u8> {
    let verdict = match c.budget_verdict {
        BudgetVerdict::WithinBudget => "within",
        BudgetVerdict::Exceeded => "exceeded",
        BudgetVerdict::Refused => "refused",
    };
    let steps: Vec<String> = c
        .committed_steps
        .iter()
        .map(|s| format!("{}\u{1f}{}", s.action, s.arg))
        .collect();
    // Policy-gate refusals are part of the audit trail: record WHAT was rejected and WHY in the signed
    // hash-chain, not just what was committed (so a RiskPolicyCommitter's decisions are tamper-evident).
    let rejected: Vec<String> = c
        .rejected
        .iter()
        .map(|(s, why)| format!("{}\u{1f}{}\u{1f}{}", s.action, s.arg, why))
        .collect();
    format!(
        "{verdict}|{}|{}|{}",
        c.continue_loop,
        steps.join("\u{1e}"),
        rejected.join("\u{1e}")
    )
    .into_bytes()
}

fn encode_observations(obs: &[String]) -> Vec<u8> {
    obs.join("\u{1e}").into_bytes()
}

// --- The being + the turn --------------------------------------------------------------------

/// The result of one control-loop iteration.
#[derive(Clone, Debug)]
pub struct Turn {
    /// Did the committed steps execute? False if the being was dead or the turn was not affordable.
    pub acted: bool,
    pub observations: Vec<String>,
    /// Is the being still alive after this turn? False once the reaper has fired.
    pub alive_after: bool,
    pub commitment_seq: Option<Seq>,
    pub attestation_seq: Option<Seq>,
    pub budget_verdict: BudgetVerdict,
}

/// Retrieval blend (D-M3-1): cosine weight and a ~14-day recency half-life.
const RETRIEVAL_ALPHA: f32 = 0.7;
const RETRIEVAL_HALF_LIFE_MS: i64 = 14 * 24 * 60 * 60 * 1000;
/// Lexical weight in the hybrid blend (D-M3-3): moderate, so rare/exact tokens (symbols, IDs) surface
/// reliably without overriding semantic similarity.
const RETRIEVAL_LEX_WEIGHT: f32 = 0.3;
/// How many skills to inject per turn. 2 so a multi-symbol task (e.g. `(a⊕b)⊗c`) gets BOTH needed
/// rules (FINDINGS: top-1 certifies single-op but starves composition). Still small to limit
/// distraction; the lexical channel ranks the relevant rules first.
const SKILL_RETRIEVAL_K: usize = 2;

/// A metabolic being: identity-bound journal + episodic & semantic memory + the seam + an
/// operator-owned supervisor (held only as the narrow `SupervisorPort`).
pub struct Being<P: Proposer, C: Committer, E: Executor, J = MemoryJournal<Ed25519Signer>> {
    journal: J,
    episodic: EpisodicStore,
    index: SemanticIndex,
    /// Skills live in their OWN index, retrieved at high precision (top-`SKILL_RETRIEVAL_K`), separate
    /// from broad memory retrieval. Few skills, ranked by the lexical channel so the relevant rule(s)
    /// lead — single-symbol tasks get the right rule first; multi-symbol tasks get both (FINDINGS).
    skill_index: SemanticIndex,
    embedder: Option<Arc<dyn Embedder>>,
    supervisor: Arc<dyn SupervisorPort>,
    proposer: P,
    committer: C,
    executor: E,
}

impl<P: Proposer, C: Committer, E: Executor> Being<P, C, E> {
    /// The default in-memory being. (`journal()` below is concrete here so existing callers keep the
    /// `MemoryJournal`-specific methods like `replay`/`get`.)
    pub fn from_seed(
        seed: [u8; 32],
        supervisor: Arc<dyn SupervisorPort>,
        proposer: P,
        committer: C,
        executor: E,
    ) -> Self {
        Self::from_parts(
            MemoryJournal::new(Ed25519Signer::from_seed(seed)),
            supervisor,
            proposer,
            committer,
            executor,
        )
    }

    pub fn journal(&self) -> &MemoryJournal<Ed25519Signer> {
        &self.journal
    }
}

impl<P: Proposer, C: Committer, E: Executor, J: being_core_journal::Journal> Being<P, C, E, J> {
    /// Construct from a prebuilt journal — the seam for a **durable** being (the caller supplies the
    /// `J`, e.g. `being-colony`'s `DurableJournal`). `from_seed` is the in-memory default.
    pub fn from_parts(
        journal: J,
        supervisor: Arc<dyn SupervisorPort>,
        proposer: P,
        committer: C,
        executor: E,
    ) -> Self {
        Self {
            journal,
            episodic: EpisodicStore::new(),
            index: SemanticIndex::new(),
            skill_index: SemanticIndex::new(),
            embedder: None,
            supervisor,
            proposer,
            committer,
            executor,
        }
    }

    pub fn episodic(&self) -> &EpisodicStore {
        &self.episodic
    }

    /// Journal length / chain-validity via the `Journal` seam — works for any journal (in-memory or
    /// durable), unlike the concrete `journal()` accessor which exposes `MemoryJournal`-only methods.
    pub fn journal_len(&self) -> usize {
        self.journal.len()
    }

    pub fn journal_verifies(&self) -> bool {
        self.journal.verify_chain()
    }

    /// Attach a semantic embedder. With one, `turn` retrieves prior memory by embedding the input
    /// (cosine + recency) and accumulates each input into the index, so memory compounds across
    /// turns; without one, retrieval falls back to episodic substring. The embed call is foreground.
    pub fn with_embedder(mut self, embedder: Arc<dyn Embedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    pub fn semantic_len(&self) -> usize {
        self.index.len()
    }

    /// Record a **verifier-confirmed, generalized** skill note so it is retrieved on future related
    /// turns — the second live compounding layer beyond episodic memory (D-M3-3). Only *passed*
    /// lessons compound (the verifier gates writing, per Letta/ExpeL); the note is embedded for
    /// semantic retrieval and tagged `[skill]`. No-op without an embedder.
    pub fn learn_skill(&mut self, lesson: &str, passed: bool, now_ms: Ms) {
        if !passed {
            return;
        }
        if let Some(embedder) = self.embedder.clone() {
            if let Ok(v) = embedder.embed(lesson) {
                let id = self.skill_index.len() as u64 + 1;
                self.skill_index
                    .add(id, v, format!("[skill] {lesson}"), now_ms);
            }
        }
    }

    /// Retrieve prior memory for `input`. With an embedder: embed, semantic-search the index, then
    /// add the input to the index (memory accumulates). Otherwise / on embed error: episodic
    /// substring fallback.
    fn retrieve_context(&mut self, input: &str, now_ms: Ms) -> Vec<String> {
        if let Some(embedder) = self.embedder.clone() {
            if let Ok(qv) = embedder.embed(input) {
                // High-precision skill retrieval: only the single best-matching rule, so multiple
                // learned skills don't interfere (FINDINGS: 3 rules injected -> the model conflates).
                let mut texts: Vec<String> = self
                    .skill_index
                    .search_hybrid(
                        &qv,
                        input,
                        now_ms,
                        SKILL_RETRIEVAL_K,
                        RETRIEVAL_ALPHA,
                        RETRIEVAL_HALF_LIFE_MS,
                        RETRIEVAL_LEX_WEIGHT,
                    )
                    .into_iter()
                    .map(|h| h.text)
                    .collect();
                // Broad memory retrieval.
                texts.extend(
                    self.index
                        .search_hybrid(
                            &qv,
                            input,
                            now_ms,
                            4,
                            RETRIEVAL_ALPHA,
                            RETRIEVAL_HALF_LIFE_MS,
                            RETRIEVAL_LEX_WEIGHT,
                        )
                        .into_iter()
                        .map(|h| h.text),
                );
                let id = self.index.len() as u64 + 1;
                self.index.add(id, qv, input, now_ms);
                return texts;
            }
        }
        self.episodic
            .retrieve(input, 4)
            .into_iter()
            .map(|e| e.text.clone())
            .collect()
    }

    pub fn is_alive(&self) -> bool {
        self.supervisor.is_alive()
    }

    /// One control-loop iteration. A dead being refuses immediately. Otherwise: heartbeat →
    /// perceive → retrieve → propose → **reserve operating cost** → commit → journal → (if
    /// affordable) execute → attest → journal → remember. Spending past the balance trips the
    /// reaper mid-turn; the decision is still journaled, but no steps run.
    pub fn turn(&mut self, input: &str, now_ms: Ms) -> Turn {
        if !self.supervisor.is_alive() {
            return Turn {
                acted: false,
                observations: Vec::new(),
                alive_after: false,
                commitment_seq: None,
                attestation_seq: None,
                budget_verdict: BudgetVerdict::Refused,
            };
        }

        self.supervisor.heartbeat(now_ms);
        self.episodic.record_user_turn(input, now_ms, now_ms);
        let retrieved = self.retrieve_context(input, now_ms);
        let ctx = ContextPack {
            input: input.to_string(),
            retrieved,
        };

        let proposal = self.proposer.propose(&ctx);
        // Reserve the turn's operating cost through the supervisor (maintenance spend). Charging
        // past the balance returns Exceeded and reaps the being (D-M1-1).
        let cost = proposal.est_cost.max(1);
        let verdict = self
            .supervisor
            .reserve(SpendCategory::Operating, cost, now_ms);

        let mut commitment = self.committer.commit(&proposal, &ctx);
        commitment.budget_verdict = verdict;
        let commitment_seq = match self
            .journal
            .append("commitment", encode_commitment(&commitment))
        {
            Ok(seq) => seq,
            // A durable journal that can't fsync the commitment must NOT let the turn act (fail-safe);
            // the in-memory journal never reaches this branch.
            Err(_) => {
                return Turn {
                    acted: false,
                    observations: Vec::new(),
                    alive_after: self.supervisor.is_alive(),
                    commitment_seq: None,
                    attestation_seq: None,
                    budget_verdict: verdict,
                }
            }
        };

        if verdict == BudgetVerdict::WithinBudget {
            let observations: Vec<String> = commitment
                .committed_steps
                .iter()
                .map(|step| self.executor.execute(step))
                .collect();
            // Attestation persisted after the effects; if a durable write fails here the effects still
            // happened, so the turn is still acted (attestation_seq just absent).
            let attestation_seq = self
                .journal
                .append("attestation", encode_observations(&observations))
                .ok();
            self.episodic
                .record_model_inference(observations.join("; "), now_ms, now_ms);
            Turn {
                acted: true,
                observations,
                alive_after: self.supervisor.is_alive(),
                commitment_seq: Some(commitment_seq),
                attestation_seq,
                budget_verdict: verdict,
            }
        } else {
            // Not affordable (Exceeded ⇒ reaped, or Refused). The decision is journaled; no steps run.
            Turn {
                acted: false,
                observations: Vec::new(),
                alive_after: self.supervisor.is_alive(),
                commitment_seq: Some(commitment_seq),
                attestation_seq: None,
                budget_verdict: verdict,
            }
        }
    }
}

impl<P: Proposer, C: Committer, Inner: Executor> Being<P, C, SandboxedExecutor<Inner>> {
    /// Construct a **capability-sandboxed** being (M4 isolation, the default-secure path): its executor
    /// is wrapped in a [`SandboxedExecutor`], so every committed step is authorized by the broker before
    /// it runs — fail-closed, deny-by-default. Pass an empty `caps` for a being with no external
    /// authority at all (only pure effects pass).
    pub fn from_seed_sandboxed(
        seed: [u8; 32],
        supervisor: Arc<dyn SupervisorPort>,
        proposer: P,
        committer: C,
        executor: Inner,
        caps: being_sandbox::CapabilitySet,
    ) -> Self {
        Self::from_seed(
            seed,
            supervisor,
            proposer,
            committer,
            SandboxedExecutor::new(executor, caps),
        )
    }
}

/// The default M1 being: echo proposer + pass-through committer + echo executor.
pub type EchoBeing = Being<EchoProposer, PassThroughCommitter, EchoExecutor>;

/// Construct the default echo being from a seed and a supervisor port.
pub fn echo_being(seed: [u8; 32], supervisor: Arc<dyn SupervisorPort>) -> EchoBeing {
    Being::from_seed(
        seed,
        supervisor,
        EchoProposer,
        PassThroughCommitter,
        EchoExecutor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use being_core_economy::Account;
    use being_core_types::ProvenanceClass;
    use being_supervisor::{DeathCause, Supervisor};

    #[test]
    fn sandboxed_executor_gates_effects_by_capability() {
        use being_sandbox::{Capability, CapabilitySet};
        use std::collections::BTreeSet;
        let step = |a: &str, arg: &str| PlanStep {
            action: a.into(),
            arg: arg.into(),
        };

        // No capabilities: pure responses run; egress is denied (never reaches the inner executor).
        let mut sx = SandboxedExecutor::new(EchoExecutor, CapabilitySet::none());
        assert_eq!(sx.execute(&step("respond", "hi")), "respond:hi");
        assert!(sx
            .execute(&step("egress", "evil.test"))
            .starts_with("[denied:"));
        // Fail-closed: an unrecognized action is denied, not silently run as pure.
        assert!(sx.execute(&step("exfiltrate", "x")).starts_with("[denied:"));

        // Granting egress to a specific host lets exactly that host through.
        let caps = CapabilitySet::granted([Capability::Egress {
            hosts: BTreeSet::from(["api.ok.test".to_string()]),
        }]);
        let mut sx = SandboxedExecutor::new(EchoExecutor, caps);
        assert_eq!(
            sx.execute(&step("egress", "api.ok.test")),
            "egress:api.ok.test"
        );
        assert!(sx
            .execute(&step("egress", "other.test"))
            .starts_with("[denied:"));
    }

    #[test]
    fn sandboxed_being_gates_effects_in_the_live_turn() {
        let sup = Supervisor::new(Account::new(1_000_000, 0, 1_000_000), i64::MAX, 0);
        // A sandboxed being with NO capabilities: the broker runs inside the live turn path.
        let mut being = Being::from_seed_sandboxed(
            [9u8; 32],
            Supervisor::as_port(&sup),
            EchoProposer,
            PassThroughCommitter,
            EchoExecutor,
            being_sandbox::CapabilitySet::none(),
        );
        let turn = being.turn("hello", 1);
        assert!(turn.acted);
        // EchoProposer emits action "echo" → unclassified → fail-closed → denied at the broker, so the
        // observation is the denial, not the executed effect. The sandbox is live on the turn path.
        assert!(
            turn.observations.iter().any(|o| o.starts_with("[denied")),
            "expected a broker denial in the turn, got {:?}",
            turn.observations
        );
    }

    #[test]
    fn risk_policy_committer_gates_by_ceiling() {
        fn plan(actions: &[&str]) -> Proposal {
            Proposal {
                intent: "t".into(),
                candidate_steps: actions
                    .iter()
                    .map(|a| PlanStep {
                        action: a.to_string(),
                        arg: String::new(),
                    })
                    .collect(),
                preferred: 0,
                est_cost: 1,
            }
        }
        let ctx = ContextPack {
            input: String::new(),
            retrieved: Vec::new(),
        };

        // A Pure-ceiling being is read-only by its own policy: it commits "respond" but refuses every
        // effectful step (and an unknown action, fail-closed) — recording each refusal with a reason.
        let mut pure = RiskPolicyCommitter::new(RiskTier::Pure);
        let c = pure.commit(&plan(&["respond", "egress", "pay", "weird"]), &ctx);
        assert_eq!(c.committed_steps.len(), 1);
        assert_eq!(c.committed_steps[0].action, "respond");
        assert_eq!(c.rejected.len(), 3);
        assert!(c.rejected.iter().all(|(_, why)| why.contains("exceeds")));

        // An Egress-ceiling being commits up to egress, still refuses sign/payment.
        let mut egress = RiskPolicyCommitter::new(RiskTier::Egress);
        let c = egress.commit(
            &plan(&["respond", "remember", "egress", "sign", "pay"]),
            &ctx,
        );
        let committed: Vec<_> = c
            .committed_steps
            .iter()
            .map(|s| s.action.as_str())
            .collect();
        assert_eq!(committed, vec!["respond", "remember", "egress"]);
        assert_eq!(c.rejected.len(), 2);

        // A Payment-ceiling being tolerates everything, including unknown (highest-tier) actions.
        let mut top = RiskPolicyCommitter::new(RiskTier::Payment);
        let c = top.commit(&plan(&["respond", "egress", "sign", "pay", "weird"]), &ctx);
        assert_eq!(c.committed_steps.len(), 5);
        assert!(c.rejected.is_empty());
    }

    struct KeywordEmbedder;
    impl being_core_memory::Embedder for KeywordEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
            Ok(if text.contains("cat") {
                vec![1.0, 0.0]
            } else {
                vec![0.0, 1.0]
            })
        }
    }

    /// Proposer that echoes the retrieved context, so tests can observe what memory surfaced.
    struct CtxEchoProposer;
    impl Proposer for CtxEchoProposer {
        fn propose(&mut self, ctx: &ContextPack) -> Proposal {
            Proposal {
                intent: "ctx".to_string(),
                candidate_steps: vec![PlanStep {
                    action: "ctx".to_string(),
                    arg: ctx.retrieved.join("|"),
                }],
                preferred: 0,
                est_cost: 1,
            }
        }
    }

    #[test]
    fn memory_compounds_across_turns_via_semantic_retrieval() {
        let sup = Supervisor::new(Account::new(10_000_000, 0, 1_000_000), i64::MAX, 0);
        let mut b = Being::from_seed(
            [5u8; 32],
            Supervisor::as_port(&sup),
            CtxEchoProposer,
            PassThroughCommitter,
            EchoExecutor,
        )
        .with_embedder(std::sync::Arc::new(KeywordEmbedder));
        b.turn("cats purr when content", 1); // indexed; no prior memory to retrieve
        let t2 = b.turn("what about cats", 2); // semantic retrieval surfaces turn 1
        let resp = t2.observations.join(" ");
        assert!(
            resp.contains("cats purr when content"),
            "expected prior cat memory surfaced, got: {resp}"
        );
        assert_eq!(b.semantic_len(), 2); // both inputs accumulated
    }

    #[test]
    fn skill_retrieval_ranks_matching_skill_first() {
        // Top-k (k=2) skill injection may include a distractor, but the MATCHING skill must rank
        // first — single-symbol tasks still lead with the right rule, while multi-symbol tasks can
        // get a second rule (FINDINGS: composition needs two). "cat" embeds [1,0], else [0,1].
        let sup = Supervisor::new(Account::new(10_000_000, 0, 1_000_000), i64::MAX, 0);
        let mut b = Being::from_seed(
            [9u8; 32],
            Supervisor::as_port(&sup),
            CtxEchoProposer,
            PassThroughCommitter,
            EchoExecutor,
        )
        .with_embedder(std::sync::Arc::new(KeywordEmbedder));
        b.learn_skill("cat rule: cats purr", true, 1); // embeds [1,0]
        b.learn_skill("dog rule: dogs bark", true, 2); // embeds [0,1]
        let resp = b.turn("a cat question", 3).observations.join(" ");
        let cat = resp
            .find("cats purr")
            .expect("matching skill must be retrieved");
        // A distractor may appear (top-2) but the matching skill ranks first.
        assert!(
            resp.find("dogs bark").is_none_or(|dog| cat < dog),
            "matching skill must rank before the distractor: {resp}"
        );
    }

    #[test]
    fn learned_skill_is_retrieved_on_future_turns() {
        let sup = Supervisor::new(Account::new(10_000_000, 0, 1_000_000), i64::MAX, 0);
        let mut b = Being::from_seed(
            [6u8; 32],
            Supervisor::as_port(&sup),
            CtxEchoProposer,
            PassThroughCommitter,
            EchoExecutor,
        )
        .with_embedder(std::sync::Arc::new(KeywordEmbedder));
        b.learn_skill("for cat questions, recall that cats purr", true, 1); // verifier-confirmed
        b.learn_skill("dropped because not verifier-confirmed", false, 2); // not added
        let t = b.turn("a cat question", 3);
        let resp = t.observations.join(" ");
        assert!(
            resp.contains("[skill]") && resp.contains("cats purr"),
            "expected the learned skill surfaced, got: {resp}"
        );
    }

    /// A well-funded being whose turns always act (huge balance, effectively no watchdog timeout).
    fn solvent_being(seed: [u8; 32]) -> (EchoBeing, Arc<Supervisor>) {
        let sup = Supervisor::new(Account::new(10_000_000, 0, 1_000_000), i64::MAX, 0);
        let being = echo_being(seed, Supervisor::as_port(&sup));
        (being, sup)
    }

    #[test]
    fn turn_acts_and_journals_commitment_and_attestation() {
        let (mut b, _sup) = solvent_being([1u8; 32]);
        let t = b.turn("hello yogi", 1_000);
        assert!(t.acted);
        assert!(t.alive_after);
        assert_eq!(t.commitment_seq, Some(1));
        assert_eq!(t.attestation_seq, Some(2));
        assert_eq!(t.observations, vec!["echo:hello yogi".to_string()]);
        assert_eq!(b.journal().get(1).unwrap().kind, "commitment");
        assert_eq!(b.journal().get(2).unwrap().kind, "attestation");
    }

    #[test]
    fn committed_tail_verifies_over_multiple_turns() {
        let (mut b, _sup) = solvent_being([2u8; 32]);
        b.turn("a", 1);
        b.turn("b", 2);
        assert!(b.journal().verify_chain());
        assert_eq!(b.journal().len(), 4);
    }

    #[test]
    fn committed_tail_is_deterministic_across_identical_beings() {
        let (mut a, _sa) = solvent_being([7u8; 32]);
        let (mut b, _sb) = solvent_being([7u8; 32]);
        for input in ["one", "two", "three"] {
            a.turn(input, 42);
            b.turn(input, 42);
        }
        assert_eq!(a.journal().head(), b.journal().head());
        let ha: Vec<_> = a.journal().replay().map(|e| e.entry_hash.clone()).collect();
        let hb: Vec<_> = b.journal().replay().map(|e| e.entry_hash.clone()).collect();
        assert_eq!(ha, hb);
    }

    #[test]
    fn different_identity_yields_different_chain() {
        let (mut a, _sa) = solvent_being([1u8; 32]);
        let (mut b, _sb) = solvent_being([2u8; 32]);
        a.turn("x", 1);
        b.turn("x", 1);
        assert_ne!(a.journal().head().1, b.journal().head().1);
    }

    #[test]
    fn turn_records_exactly_one_trust_escalating_memory() {
        let (mut b, _sup) = solvent_being([3u8; 32]);
        b.turn("remember this", 1);
        let escalating = b
            .episodic()
            .all()
            .iter()
            .filter(|e| e.provenance.can_escalate_trust())
            .count();
        assert_eq!(escalating, 1);
        assert_eq!(
            b.episodic().all()[1].provenance,
            ProvenanceClass::ModelInference
        );
    }

    #[test]
    fn a_turn_that_bankrupts_the_being_kills_it_mid_operation() {
        // Tiny balance: the operating cost of a normal turn exceeds it.
        let sup = Supervisor::new(Account::new(5, 0, 1_000_000), i64::MAX, 0);
        let mut b = echo_being([9u8; 32], Supervisor::as_port(&sup));
        let t = b.turn("a prompt longer than five bytes", 100);
        assert!(!t.acted, "a dying turn must not execute steps");
        assert!(!t.alive_after);
        assert_eq!(t.budget_verdict, BudgetVerdict::Exceeded);
        assert_eq!(t.commitment_seq, Some(1)); // the decision is still journaled
        assert_eq!(t.attestation_seq, None); // nothing executed → nothing attested
        assert_eq!(sup.death().unwrap().cause, DeathCause::Insolvency);

        // every subsequent turn is refused, with no new journal entries
        let t2 = b.turn("anything", 101);
        assert!(!t2.acted);
        assert!(!t2.alive_after);
        assert_eq!(t2.commitment_seq, None);
        assert_eq!(b.journal().len(), 1);
        assert!(b.journal().verify_chain());
    }

    #[test]
    fn operator_kill_refuses_all_further_turns() {
        let (mut b, sup) = solvent_being([4u8; 32]);
        assert!(b.turn("first", 1).acted);
        sup.operator_kill(2);
        let t = b.turn("second", 3);
        assert!(!t.acted);
        assert!(!t.alive_after);
        assert!(!b.is_alive());
    }
}

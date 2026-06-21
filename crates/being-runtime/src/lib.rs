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

/// Pass-through policy committer. The real policy gate (trust + risk) lands later; budget is the
/// supervisor's job and is filled into `budget_verdict` by the turn.
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

/// Echo executor: returns the step echoed back.
pub struct EchoExecutor;
impl Executor for EchoExecutor {
    fn execute(&mut self, step: &PlanStep) -> String {
        format!("{}:{}", step.action, step.arg)
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
    format!("{verdict}|{}|{}", c.continue_loop, steps.join("\u{1e}")).into_bytes()
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

/// A metabolic being: identity-bound journal + episodic & semantic memory + the seam + an
/// operator-owned supervisor (held only as the narrow `SupervisorPort`).
pub struct Being<P: Proposer, C: Committer, E: Executor> {
    journal: MemoryJournal<Ed25519Signer>,
    episodic: EpisodicStore,
    index: SemanticIndex,
    /// Skills live in their OWN index, retrieved at high precision (top-1), separate from broad
    /// memory retrieval — multi-skill transfer collapses if several rules are injected at once
    /// (FINDINGS: skill interference). The lexical channel already ranks the right rule first.
    skill_index: SemanticIndex,
    embedder: Option<Arc<dyn Embedder>>,
    supervisor: Arc<dyn SupervisorPort>,
    proposer: P,
    committer: C,
    executor: E,
}

impl<P: Proposer, C: Committer, E: Executor> Being<P, C, E> {
    pub fn from_seed(
        seed: [u8; 32],
        supervisor: Arc<dyn SupervisorPort>,
        proposer: P,
        committer: C,
        executor: E,
    ) -> Self {
        Self {
            journal: MemoryJournal::new(Ed25519Signer::from_seed(seed)),
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

    pub fn journal(&self) -> &MemoryJournal<Ed25519Signer> {
        &self.journal
    }

    pub fn episodic(&self) -> &EpisodicStore {
        &self.episodic
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
                        1,
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
        let commitment_seq = self
            .journal
            .append("commitment", encode_commitment(&commitment));

        if verdict == BudgetVerdict::WithinBudget {
            let observations: Vec<String> = commitment
                .committed_steps
                .iter()
                .map(|step| self.executor.execute(step))
                .collect();
            let attestation_seq = self
                .journal
                .append("attestation", encode_observations(&observations));
            self.episodic
                .record_model_inference(observations.join("; "), now_ms, now_ms);
            Turn {
                acted: true,
                observations,
                alive_after: self.supervisor.is_alive(),
                commitment_seq: Some(commitment_seq),
                attestation_seq: Some(attestation_seq),
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
    fn only_the_matching_skill_is_injected_no_interference() {
        // With several learned skills, retrieval must inject ONLY the matching one (top-1) — the fix
        // for multi-skill interference (FINDINGS). Here "cat" embeds [1,0], everything else [0,1].
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
        assert!(resp.contains("cats purr"), "matching skill missing: {resp}");
        assert!(
            !resp.contains("dogs bark"),
            "interfering skill injected: {resp}"
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

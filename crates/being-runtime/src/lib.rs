//! The runtime seam + control loop (build-spec §3.4, §3.5; architecture §4).
//!
//! The **proposer** (an LLM in the full build) generates; the **committer** is a separate,
//! mostly-deterministic gate that decides what runs; the **executor** acts. The seam is
//! propose → commit → attest, and every commitment and attestation is appended to the signed,
//! hash-chained journal, so the committed tail of a turn is replayable and attributable.
//!
//! M0 wires this end-to-end with an echo proposer (no model) and a pass-through committer (no
//! budget/trust gate — those arrive at M1 with the Account and the reaper). The seam *shape* and
//! the journaling/determinism guarantees are the deliverable.

use being_core_id::Ed25519Signer;
use being_core_journal::{MemoryJournal, Seq};
use being_core_memory::{EpisodicStore, Ms};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetVerdict {
    WithinBudget,
    Exceeded,
    Refused,
}

/// Committer output: the decisive cut — what actually runs this turn.
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

/// The commitment layer. Deterministic gate; decides what runs and whether to loop.
pub trait Committer {
    fn commit(&mut self, proposal: &Proposal, ctx: &ContextPack) -> Commitment;
}

/// The actuator. Runs a committed step, returns an observation.
pub trait Executor {
    fn execute(&mut self, step: &PlanStep) -> String;
}

// --- M0 implementations ----------------------------------------------------------------------

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

/// Pass-through committer: commits the proposed steps unchanged. The real gate (policy + trust +
/// budget reservation) lands at M1; the seam is already separate so that gate drops in here.
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
// Manual, stable byte encoding (no serde) so committed-tail replay is byte-identical.

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
    pub commitment_seq: Seq,
    pub attestation_seq: Seq,
    pub observations: Vec<String>,
}

/// A minimal being: identity-bound journal + episodic memory + the seam components. This is the
/// M0 skeleton; the Account/reaper (M1), real proposer (M2), and the rest layer onto it.
pub struct Being<P: Proposer, C: Committer, E: Executor> {
    journal: MemoryJournal<Ed25519Signer>,
    episodic: EpisodicStore,
    proposer: P,
    committer: C,
    executor: E,
}

impl<P: Proposer, C: Committer, E: Executor> Being<P, C, E> {
    pub fn from_seed(seed: [u8; 32], proposer: P, committer: C, executor: E) -> Self {
        Self {
            journal: MemoryJournal::new(Ed25519Signer::from_seed(seed)),
            episodic: EpisodicStore::new(),
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

    /// One control-loop iteration: perceive → retrieve → propose → commit → journal → execute →
    /// attest → journal → remember. The commitment and attestation are signed journal entries, so
    /// the committed tail is replayable and tamper-evident.
    pub fn turn(&mut self, input: &str, now_ms: Ms) -> Turn {
        // perceive
        self.episodic.record_user_turn(input, now_ms, now_ms);
        // retrieve
        let retrieved: Vec<String> = self
            .episodic
            .retrieve(input, 4)
            .into_iter()
            .map(|e| e.text.clone())
            .collect();
        let ctx = ContextPack {
            input: input.to_string(),
            retrieved,
        };
        // propose (model) → commit (deterministic gate)
        let proposal = self.proposer.propose(&ctx);
        let commitment = self.committer.commit(&proposal, &ctx);
        let commitment_seq = self
            .journal
            .append("commitment", encode_commitment(&commitment));
        // execute committed steps
        let observations: Vec<String> = commitment
            .committed_steps
            .iter()
            .map(|step| self.executor.execute(step))
            .collect();
        // attest the observations to the journal
        let attestation_seq = self
            .journal
            .append("attestation", encode_observations(&observations));
        // remember the model-derived result (ModelInference provenance — cannot escalate trust)
        self.episodic
            .record_model_inference(observations.join("; "), now_ms, now_ms);
        Turn {
            commitment_seq,
            attestation_seq,
            observations,
        }
    }
}

/// The default M0 being: echo proposer + pass-through committer + echo executor.
pub type EchoBeing = Being<EchoProposer, PassThroughCommitter, EchoExecutor>;

/// Construct the default M0 being from a seed.
pub fn echo_being(seed: [u8; 32]) -> EchoBeing {
    Being::from_seed(seed, EchoProposer, PassThroughCommitter, EchoExecutor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use being_core_types::ProvenanceClass;

    #[test]
    fn turn_runs_end_to_end_and_journals_commitment_and_attestation() {
        let mut b = echo_being([1u8; 32]);
        let t = b.turn("hello yogi", 1_000);
        assert_eq!(t.commitment_seq, 1);
        assert_eq!(t.attestation_seq, 2);
        assert_eq!(t.observations, vec!["echo:hello yogi".to_string()]);
        // both steps landed in the signed journal
        assert_eq!(b.journal().len(), 2);
        assert_eq!(b.journal().get(1).unwrap().kind, "commitment");
        assert_eq!(b.journal().get(2).unwrap().kind, "attestation");
    }

    #[test]
    fn committed_tail_verifies() {
        let mut b = echo_being([2u8; 32]);
        b.turn("a", 1);
        b.turn("b", 2);
        assert!(b.journal().verify_chain());
        assert_eq!(b.journal().len(), 4); // 2 turns × (commitment + attestation)
    }

    #[test]
    fn committed_tail_is_deterministic_across_identical_beings() {
        let mut a = echo_being([7u8; 32]);
        let mut b = echo_being([7u8; 32]);
        for input in ["one", "two", "three"] {
            a.turn(input, 42);
            b.turn(input, 42);
        }
        // same seed + same inputs + same clock ⇒ byte-identical signed chains
        assert_eq!(a.journal().head(), b.journal().head());
        let ha: Vec<_> = a.journal().replay().map(|e| e.entry_hash.clone()).collect();
        let hb: Vec<_> = b.journal().replay().map(|e| e.entry_hash.clone()).collect();
        assert_eq!(ha, hb);
    }

    #[test]
    fn different_identity_yields_different_chain() {
        let mut a = echo_being([1u8; 32]);
        let mut b = echo_being([2u8; 32]);
        a.turn("x", 1);
        b.turn("x", 1);
        assert_ne!(a.journal().head().1, b.journal().head().1);
    }

    #[test]
    fn turn_records_exactly_one_trust_escalating_memory() {
        let mut b = echo_being([3u8; 32]);
        b.turn("remember this", 1);
        let escalating = b
            .episodic()
            .all()
            .iter()
            .filter(|e| e.provenance.can_escalate_trust())
            .count();
        assert_eq!(escalating, 1); // only the user turn; the model result is ModelInference
        assert_eq!(
            b.episodic().all()[1].provenance,
            ProvenanceClass::ModelInference
        );
    }
}

//! `being-goals` — FREE-verifier goals for the democratization ratchet (docs/plan P1).
//!
//! Each goal supplies a **deterministic, no-salary verifier**, so the self-distillation ratchet (does
//! distilling the being's own verified successes raise its held-out floor?) can be measured at ZERO
//! cloud cost — the purest democratization setup. All modules are pure + model-free (green-gate safe);
//! the model runs only in foreground bins.
//!
//! - [`op`] — the active P1 goal: a **novel rule** with guaranteed headroom (cold ≈ 0). With the rule
//!   in-context the model solves it for free; distilling those verified traces (prompt *without* the
//!   rule → answer) should raise the COLD floor = the skill internalized into weights.
//! - [`tautogram`] — a constraint goal that turned out **near-saturated** for qwen3:8b even at K=9
//!   (mean ≈ 0.95) — a recorded finding: a strong small model has little headroom on simple constraint
//!   goals, so headroom lives on *novel* skills.

pub mod cipher;
pub mod op;
pub mod tautogram;

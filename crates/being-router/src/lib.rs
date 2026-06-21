//! Routing / navigator — the compounding layer that picks **reasoning depth per task** (architecture
//! §: navigator). Operationalizes the certified lesson (FINDINGS 2026-06-21): tasks that need a
//! scratchpad (arithmetic, multi-step, "why/how/compute") must run with **thinking on**; trivial
//! recall should not, to save latency/tokens. Pure + deterministic — no model, no I/O.
//!
//! This is the cheap heuristic router. An **outcome-learned** upgrade (route on past pass/fail per
//! task class — the verifier signal) is a later step; the trait below is the seam for it.

/// How much reasoning a task gets. `Think` ⇒ the proposer's thinking-mode preset; `NoThink` ⇒ the
/// fast/no-think preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReasoningMode {
    NoThink,
    Think,
}

/// Picks a reasoning depth for a task. The seam an outcome-learned router will also implement.
pub trait Router {
    fn route(&self, task: &str) -> ReasoningMode;
}

/// Lexical cues that a task needs step-by-step reasoning.
const COMPUTE_CUES: &[&str] = &[
    "compute",
    "calculate",
    "how many",
    "how much",
    "why",
    "prove",
    "step",
    "solve",
    "derive",
    "sum of",
    "product of",
    "multiply",
    "divide",
];

/// Math operators (incl. the novel `⊕`) that, alongside a digit, signal computation.
const MATH_SYMBOLS: &str = "+-*/×÷⊕=^%";

/// Heuristic reasoning-depth router. Errs toward `Think` when in doubt — extra reasoning costs
/// latency, never correctness, so a false "Think" is cheap while a false "NoThink" can break a
/// computation (the certified failure mode).
#[derive(Clone, Debug, Default)]
pub struct HeuristicRouter;

impl Router for HeuristicRouter {
    fn route(&self, task: &str) -> ReasoningMode {
        let t = task.to_lowercase();
        let has_cue = COMPUTE_CUES.iter().any(|c| t.contains(c));
        let has_math =
            t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| MATH_SYMBOLS.contains(c));
        if has_cue || has_math {
            ReasoningMode::Think
        } else {
            ReasoningMode::NoThink
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computation_routes_to_thinking() {
        let r = HeuristicRouter;
        assert_eq!(r.route("What is 5 ⊕ 6?"), ReasoningMode::Think); // digit + ⊕
        assert_eq!(r.route("Compute 17 * 23"), ReasoningMode::Think); // cue + math
        assert_eq!(r.route("How many primes below 20?"), ReasoningMode::Think); // cue
    }

    #[test]
    fn trivial_recall_routes_to_no_think() {
        let r = HeuristicRouter;
        assert_eq!(
            r.route("What is the capital of France?"),
            ReasoningMode::NoThink
        );
        assert_eq!(r.route("Name a color of the sky."), ReasoningMode::NoThink);
    }
}

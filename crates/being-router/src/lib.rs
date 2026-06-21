//! Routing / navigator — the compounding layer that picks **reasoning depth per task** (architecture
//! §: navigator). Operationalizes the certified lesson (FINDINGS 2026-06-21): tasks that need a
//! scratchpad (arithmetic, multi-step, "why/how/compute") must run with **thinking on**; trivial
//! recall should not, to save latency/tokens. Pure + deterministic — no model, no I/O.
//!
//! This is the cheap heuristic router. An **outcome-learned** upgrade (route on past pass/fail per
//! task class — the verifier signal) is a later step; the trait below is the seam for it.

use std::collections::BTreeMap;

/// How much reasoning a task gets. `Think` ⇒ the proposer's thinking-mode preset; `NoThink` ⇒ the
/// fast/no-think preset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

/// Coarse task class for outcome-learned routing — the bucket pass/fail is aggregated over.
fn class_of(task: &str) -> &'static str {
    let t = task.to_lowercase();
    let has_math =
        t.chars().any(|c| c.is_ascii_digit()) && t.chars().any(|c| MATH_SYMBOLS.contains(c));
    if has_math {
        "math"
    } else if COMPUTE_CUES.iter().any(|c| t.contains(c)) {
        "cue"
    } else {
        "plain"
    }
}

/// Outcome-learned router (D-M3-4): learns which [`ReasoningMode`] wins per coarse task-class from
/// recorded **verifier pass/fail**, falling back to the heuristic until a class has enough samples.
/// Value = `pass_rate − λ·cost(mode)` (Think costs more), so it keeps accuracy while preferring the
/// cheaper mode on ties — wiring routing into the metabolism budget. Does **no model inference**, so
/// it is legal inside the automated loop. A LinUCB/Thompson upgrade (richer context + exploration)
/// slots behind the same [`Router`] trait later.
pub struct OutcomeLearnedRouter {
    min_samples: u32,
    lambda: f64,
    stats: BTreeMap<(&'static str, ReasoningMode), (u32, u32)>, // (class, mode) -> (n, passes)
    fallback: HeuristicRouter,
}

impl OutcomeLearnedRouter {
    pub fn new(min_samples: u32, lambda: f64) -> Self {
        Self {
            min_samples,
            lambda,
            stats: BTreeMap::new(),
            fallback: HeuristicRouter,
        }
    }

    /// Record a verifier outcome for the mode actually used on `task`.
    pub fn record(&mut self, task: &str, mode: ReasoningMode, passed: bool) {
        let e = self.stats.entry((class_of(task), mode)).or_insert((0, 0));
        e.0 += 1;
        if passed {
            e.1 += 1;
        }
    }

    /// Classes that have graduated from cold-start (both modes sampled) and the mode now chosen for
    /// each — observability for the learned policy. Pure.
    pub fn learned_decisions(&self) -> BTreeMap<&'static str, ReasoningMode> {
        let classes: std::collections::BTreeSet<&'static str> =
            self.stats.keys().map(|(c, _)| *c).collect();
        classes
            .into_iter()
            .filter_map(|c| {
                match (
                    self.value(c, ReasoningMode::Think),
                    self.value(c, ReasoningMode::NoThink),
                ) {
                    (Some(vt), Some(vn)) => Some((
                        c,
                        if vn >= vt {
                            ReasoningMode::NoThink
                        } else {
                            ReasoningMode::Think
                        },
                    )),
                    _ => None,
                }
            })
            .collect()
    }

    /// Estimated value of a mode for a class, or `None` until it has `min_samples` outcomes.
    fn value(&self, class: &'static str, mode: ReasoningMode) -> Option<f64> {
        let (n, passes) = self.stats.get(&(class, mode)).copied().unwrap_or((0, 0));
        if n < self.min_samples {
            return None;
        }
        let cost = match mode {
            ReasoningMode::Think => 1.0,
            ReasoningMode::NoThink => 0.0,
        };
        Some(passes as f64 / n as f64 - self.lambda * cost)
    }
}

impl Router for OutcomeLearnedRouter {
    fn route(&self, task: &str) -> ReasoningMode {
        let class = class_of(task);
        match (
            self.value(class, ReasoningMode::Think),
            self.value(class, ReasoningMode::NoThink),
        ) {
            // Both modes have enough samples: pick the higher value; ties go to the cheaper NoThink.
            (Some(vt), Some(vn)) => {
                if vn >= vt {
                    ReasoningMode::NoThink
                } else {
                    ReasoningMode::Think
                }
            }
            // Cold-start: fall back to the heuristic until both modes are sampled.
            _ => self.fallback.route(task),
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

    #[test]
    fn outcome_router_cold_starts_with_heuristic() {
        let r = OutcomeLearnedRouter::new(3, 0.05);
        assert_eq!(r.route("What is 5 ⊕ 6?"), ReasoningMode::Think);
        assert_eq!(
            r.route("What is the capital of France?"),
            ReasoningMode::NoThink
        );
    }

    #[test]
    fn outcome_router_learns_to_override_heuristic() {
        let mut r = OutcomeLearnedRouter::new(3, 0.05);
        let task = "describe the vibe"; // class "plain" → heuristic says NoThink
        assert_eq!(HeuristicRouter.route(task), ReasoningMode::NoThink);
        for _ in 0..3 {
            r.record(task, ReasoningMode::NoThink, false); // NoThink keeps failing
            r.record(task, ReasoningMode::Think, true); // Think keeps passing
        }
        assert_eq!(r.route(task), ReasoningMode::Think); // learned to override the heuristic
    }

    #[test]
    fn outcome_router_reports_learned_decisions() {
        let mut r = OutcomeLearnedRouter::new(3, 0.05);
        let task = "describe the vibe"; // class "plain"
        for _ in 0..3 {
            r.record(task, ReasoningMode::NoThink, false);
            r.record(task, ReasoningMode::Think, true);
        }
        let d = r.learned_decisions();
        assert_eq!(d.get("plain"), Some(&ReasoningMode::Think));
        assert!(!d.contains_key("math")); // never sampled → not graduated
    }

    #[test]
    fn outcome_router_prefers_cheaper_mode_on_accuracy_ties() {
        let mut r = OutcomeLearnedRouter::new(3, 0.05);
        let task = "Compute something"; // class "cue" → heuristic says Think
        for _ in 0..3 {
            r.record(task, ReasoningMode::NoThink, true);
            r.record(task, ReasoningMode::Think, true);
        }
        assert_eq!(r.route(task), ReasoningMode::NoThink); // equal accuracy → cheaper wins
    }
}

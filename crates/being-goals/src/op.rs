//! A **novel made-up operator** — the P1 ratchet's free-verifier goal with *guaranteed headroom*.
//!
//! `a ⊕ b = 3·a + 2·b`. A base model can't know ⊕ (cold ≈ 0). With the rule in-context it computes it
//! easily → it generates its OWN verified-correct traces for free. Distilling those traces with a COLD
//! prompt (no rule) → answer should lift the cold floor from ≈0 = the rule internalized into the weights.
//!
//! The arithmetic is deliberately EASY (`3a+2b`, no multi-digit multiply): the thesis is
//! *rule-internalization*, not arithmetic ability. A first run with `a·b+a+b` confounded the two — the
//! 1.5B self-generated only 5/64 traces because it couldn't do the multiply one-shot, starving the
//! distill. Easy arithmetic isolates the variable so self-gen yields plenty of verified traces.
//! Free, exact verifier (compute the truth). Train/test pairs are disjoint (test pairs all contain a
//! `9`, unseen in training) so a gain is generalization, not memorization.

/// The made-up binary operator: `a ⊕ b = 3*a + 2*b` (novel mapping, easy arithmetic).
pub fn op(a: i64, b: i64) -> i64 {
    3 * a + 2 * b
}

/// COLD prompt — the rule is NOT given (what the floor is measured on).
pub fn cold_prompt(a: i64, b: i64) -> String {
    format!("What is {a} \u{2295} {b}? Reply with only the integer.")
}

/// TAUGHT prompt — the rule IS in context (used to cheaply GENERATE verified traces).
pub fn taught_prompt(a: i64, b: i64) -> String {
    format!(
        "The operator \u{2295} is defined by a \u{2295} b = 3*a + 2*b. \
         What is {a} \u{2295} {b}? Reply with only the integer."
    )
}

/// Extract the model's integer answer = the last integer token in the text.
pub fn parse_answer(text: &str) -> Option<i64> {
    text.chars()
        .map(|c| if c.is_ascii_digit() || c == '-' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .filter_map(|t| t.parse::<i64>().ok())
        .next_back()
}

/// Free verifier: the model's answer equals the computed truth.
pub fn verify(a: i64, b: i64, answer: &str) -> bool {
    parse_answer(answer) == Some(op(a, b))
}

/// Training pairs (1..=8 squared) — the rule is generated/verified on these.
pub fn train_pairs() -> Vec<(i64, i64)> {
    (1..=8).flat_map(|a| (1..=8).map(move |b| (a, b))).collect()
}

/// Held-out pairs (all contain a `9`, unseen in training) — a gain here is generalization.
pub fn test_pairs() -> Vec<(i64, i64)> {
    vec![(9, 3), (7, 9), (9, 9), (2, 9), (9, 6), (4, 9), (9, 1), (8, 9)]
}

/// The operator goal as a [`Goal`] instance — proves the engine is goal-agnostic (adding a goal is just
/// implementing this trait; no change to the awareness layer or the ratchet).
pub struct OpGoal;

impl being_metacog::Goal for OpGoal {
    type Instance = (i64, i64);
    fn name(&self) -> &str {
        "operator (3a+2b)"
    }
    fn train(&self) -> Vec<(i64, i64)> {
        train_pairs()
    }
    fn test(&self) -> Vec<(i64, i64)> {
        test_pairs()
    }
    fn cold_prompt(&self, i: &(i64, i64)) -> String {
        cold_prompt(i.0, i.1)
    }
    fn taught_prompt(&self, i: &(i64, i64)) -> String {
        taught_prompt(i.0, i.1)
    }
    fn verify(&self, i: &(i64, i64), output: &str) -> bool {
        verify(i.0, i.1, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use being_metacog::Goal;

    #[test]
    fn operator_rule() {
        assert_eq!(op(3, 4), 17); // 3*3 + 2*4 = 9+8
        assert_eq!(op(1, 1), 5);
        assert_eq!(op(9, 9), 45);
    }

    #[test]
    fn parses_last_integer() {
        assert_eq!(parse_answer("The answer is 17."), Some(17));
        assert_eq!(parse_answer("3 \u{2295} 4 ... = 17"), Some(17));
        assert_eq!(parse_answer("no number here"), None);
        assert_eq!(parse_answer("-5"), Some(-5));
    }

    #[test]
    fn verifier_checks_truth() {
        assert!(verify(3, 4, "17"));
        assert!(verify(3, 4, "the result is 17"));
        assert!(!verify(3, 4, "7")); // a+b — the cold-model guess
    }

    #[test]
    fn op_goal_implements_the_generic_goal_trait() {
        let g = OpGoal;
        assert!(!g.train().is_empty() && !g.test().is_empty());
        let i = g.test()[0];
        assert!(g.cold_prompt(&i).contains('\u{2295}'));
        assert!(g.taught_prompt(&i).contains("3*a"));
        assert!(g.verify(&i, &format!("{}", op(i.0, i.1))));
        assert!(!g.verify(&i, "0"));
    }

    #[test]
    fn train_and_test_pairs_are_disjoint() {
        let tr = train_pairs();
        assert_eq!(tr.len(), 64);
        for p in test_pairs() {
            assert!(!tr.contains(&p), "{p:?} leaks across the split");
        }
    }
}

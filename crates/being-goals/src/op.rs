//! A **novel made-up operator** — the P1 ratchet's free-verifier goal with *guaranteed headroom*.
//!
//! `a ⊕ b = a·b + a + b`. A base model can't know ⊕ (cold ≈ 0). With the rule in-context it computes
//! it trivially → it generates its OWN verified-correct traces for free. Distilling those traces with a
//! COLD prompt (no rule) → answer should lift the cold floor from ≈0 = the rule internalized into the
//! weights. Free, exact verifier (compute the truth). Train/test pairs are disjoint (test pairs all
//! contain a `9`, unseen in training) so a gain is generalization, not memorization.

/// The made-up binary operator: `a ⊕ b = a*b + a + b`.
pub fn op(a: i64, b: i64) -> i64 {
    a * b + a + b
}

/// COLD prompt — the rule is NOT given (what the floor is measured on).
pub fn cold_prompt(a: i64, b: i64) -> String {
    format!("What is {a} \u{2295} {b}? Reply with only the integer.")
}

/// TAUGHT prompt — the rule IS in context (used to cheaply GENERATE verified traces).
pub fn taught_prompt(a: i64, b: i64) -> String {
    format!(
        "The operator \u{2295} is defined by a \u{2295} b = a*b + a + b. \
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_rule() {
        assert_eq!(op(3, 4), 19); // 12+3+4
        assert_eq!(op(1, 1), 3);
        assert_eq!(op(9, 9), 99);
    }

    #[test]
    fn parses_last_integer() {
        assert_eq!(parse_answer("The answer is 19."), Some(19));
        assert_eq!(parse_answer("19 \u{2295} ... = 99"), Some(99));
        assert_eq!(parse_answer("no number here"), None);
        assert_eq!(parse_answer("-5"), Some(-5));
    }

    #[test]
    fn verifier_checks_truth() {
        assert!(verify(3, 4, "19"));
        assert!(verify(3, 4, "the result is 19"));
        assert!(!verify(3, 4, "12")); // a*b only — the cold-model mistake
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

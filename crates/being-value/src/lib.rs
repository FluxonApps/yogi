//! The value source (build-spec §M5; D-M5-1): operator-as-customer payer, inflow-bounded.
//!
//! A [`Tariff`] prices each task class; a [`Grader`] decides acceptance against operator-held ground
//! truth; a [`Treasury`] bounds total payout by the committed external inflow (budget-conservation).
//! On a graded-accepted delivery the operator credits the survival Account (the crediting itself is
//! the supervisor's job; this crate only computes the revenue, purely).
//!
//! **Efficiency-only until a genuinely exogenous payer exists.** Under operator-as-customer the value
//! gradient is operator-internal, so value-capture claims stay labeled *efficiency-only*; the
//! [`ExternalPayer`] trait is the hook for a payer the operator cannot reprice (D-M5-1, D-M1-2).
//! Pure and loop-safe — no model, no network.

use std::collections::BTreeMap;

use being_core_types::Microdollars;

/// Per-task-class pricing with a default fallback.
#[derive(Clone, Debug)]
pub struct Tariff {
    default_price: Microdollars,
    prices: BTreeMap<String, Microdollars>,
}

impl Tariff {
    pub fn new(default_price: Microdollars) -> Self {
        Self {
            default_price,
            prices: BTreeMap::new(),
        }
    }

    pub fn with(mut self, task_class: impl Into<String>, price: Microdollars) -> Self {
        self.prices.insert(task_class.into(), price);
        self
    }

    pub fn price(&self, task_class: &str) -> Microdollars {
        self.prices
            .get(task_class)
            .copied()
            .unwrap_or(self.default_price)
    }
}

/// Decides whether delivered work is acceptable. Real graders use **held-out, non-stationary** ground
/// truth the being cannot observe at decision time (the load-bearing anti-Goodhart surface, D-M1-2).
pub trait Grader {
    fn accept(&self, task_class: &str, response: &str, ground_truth: &str) -> bool;
}

/// Deterministic v0 grader: accept iff the response contains the ground-truth substring (case-insensitive).
pub struct SubstringGrader;
impl Grader for SubstringGrader {
    fn accept(&self, _task_class: &str, response: &str, ground_truth: &str) -> bool {
        response
            .to_lowercase()
            .contains(&ground_truth.to_lowercase())
    }
}

/// Inflow-bounded treasury: total credits paid can never exceed the committed external inflow
/// (budget-conservation; D-M5-1). Operator-owned.
#[derive(Clone, Debug)]
pub struct Treasury {
    remaining: Microdollars,
    paid: Microdollars,
}

impl Treasury {
    pub fn new(inflow: Microdollars) -> Self {
        Self {
            remaining: inflow.max(0),
            paid: 0,
        }
    }

    pub fn remaining(&self) -> Microdollars {
        self.remaining
    }

    pub fn paid(&self) -> Microdollars {
        self.paid
    }

    /// Pay up to `amount`, bounded by remaining inflow. Returns the amount actually paid.
    pub fn draw(&mut self, amount: Microdollars) -> Microdollars {
        let pay = amount.clamp(0, self.remaining);
        self.remaining -= pay;
        self.paid += pay;
        pay
    }
}

/// The inflow source. The hook for a genuinely exogenous payer to replace operator-as-customer.
pub trait ExternalPayer {
    /// Settle a delivered task: returns the microdollars to credit (0 if rejected or inflow exhausted).
    fn settle(&mut self, task_class: &str, response: &str, ground_truth: &str) -> Microdollars;
}

/// Operator-as-customer payer (v0). Revenue is **efficiency-only** until a genuinely exogenous payer
/// replaces it (D-M5-1, D-M1-2).
pub struct OperatorPayer<G: Grader> {
    pub tariff: Tariff,
    pub grader: G,
    pub treasury: Treasury,
}

impl<G: Grader> OperatorPayer<G> {
    pub fn new(tariff: Tariff, grader: G, treasury: Treasury) -> Self {
        Self {
            tariff,
            grader,
            treasury,
        }
    }
}

impl<G: Grader> ExternalPayer for OperatorPayer<G> {
    fn settle(&mut self, task_class: &str, response: &str, ground_truth: &str) -> Microdollars {
        if !self.grader.accept(task_class, response, ground_truth) {
            return 0;
        }
        let price = self.tariff.price(task_class);
        self.treasury.draw(price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tariff_uses_class_price_then_default() {
        let t = Tariff::new(1000).with("hard", 5000);
        assert_eq!(t.price("hard"), 5000);
        assert_eq!(t.price("unknown"), 1000);
    }

    #[test]
    fn substring_grader_is_case_insensitive() {
        let g = SubstringGrader;
        assert!(g.accept("q", "The answer is Paris.", "paris"));
        assert!(!g.accept("q", "London", "Paris"));
    }

    #[test]
    fn treasury_bounds_payout_by_inflow() {
        let mut t = Treasury::new(1000);
        assert_eq!(t.draw(600), 600);
        assert_eq!(t.remaining(), 400);
        assert_eq!(t.draw(900), 400); // capped at remaining
        assert_eq!(t.remaining(), 0);
        assert_eq!(t.paid(), 1000);
        assert_eq!(t.draw(100), 0); // exhausted
    }

    #[test]
    fn operator_payer_credits_only_accepted_work_within_inflow() {
        let mut p = OperatorPayer::new(Tariff::new(1000), SubstringGrader, Treasury::new(1500));
        assert_eq!(p.settle("q", "it is paris", "Paris"), 1000); // accepted → tariff
        assert_eq!(p.settle("q", "wrong", "Paris"), 0); // rejected → 0
        assert_eq!(p.settle("q", "paris again", "Paris"), 500); // accepted but inflow-bounded
        assert_eq!(p.treasury.paid(), 1500);
    }

    #[test]
    fn treasury_clamps_negative_inflow_and_negative_draws() {
        // Negative committed inflow is treated as zero (no payout possible).
        let mut empty = Treasury::new(-100);
        assert_eq!(empty.remaining(), 0);
        assert_eq!(empty.draw(-50), 0);
        assert_eq!(empty.draw(10), 0);
        // A negative draw on a funded treasury is a no-op (never refunds/inflates the budget).
        let mut funded = Treasury::new(1000);
        assert_eq!(funded.draw(-50), 0);
        assert_eq!(funded.remaining(), 1000);
        assert_eq!(funded.paid(), 0);
    }

    #[test]
    fn zero_priced_accepted_work_pays_nothing() {
        // Acceptance alone never moves money — the tariff does. A 0-priced class pays 0 even when graded.
        let mut p = OperatorPayer::new(Tariff::new(0), SubstringGrader, Treasury::new(1000));
        assert_eq!(p.settle("q", "contains paris", "paris"), 0);
        assert_eq!(p.treasury.remaining(), 1000); // untouched
        assert_eq!(p.treasury.paid(), 0);
    }

    #[test]
    fn accepted_work_pays_zero_once_inflow_is_exhausted() {
        let mut p = OperatorPayer::new(Tariff::new(1000), SubstringGrader, Treasury::new(1000));
        assert_eq!(p.settle("q", "paris", "paris"), 1000); // exhausts inflow
        assert_eq!(p.settle("q", "paris", "paris"), 0); // accepted, but nothing left to pay
        assert_eq!(p.treasury.paid(), 1000); // conserved: never pays beyond committed inflow
    }
}

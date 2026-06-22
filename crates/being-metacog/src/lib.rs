//! `being-metacog` — the **awareness layer** that drives self-directed compounding.
//!
//! Compounding needs *awareness + practice + loop*. Practice and loop are the ratchet
//! (generate→verify→distill→re-eval); **awareness** is the missing metacognitive driver: the agent
//! knowing what it can do, what it can *learn now*, and what is *beyond* it — so it can target its own
//! practice instead of an operator doing it.
//!
//! Frontier grounding:
//! - Awareness is **verifier-grounded, not introspective**: LLMs are overconfident and "know what they
//!   know but don't act on it" (arXiv 2509.21545; 2605.14186) — so capability is measured by the free
//!   verifier, never the model's self-report.
//! - Practice at the **capability frontier / Zone of Proximal Development**: items the agent fails
//!   alone but solves *with help* are maximally informative (AgentFrontier 2510.24695; Voyager
//!   2305.16291's automatic curriculum).
//! - Assess by **self-exploration** (Automated Capability Discovery 2502.07577); self-generated tasks
//!   carry their own verifier (Self-Challenging Agents 2506.01716).
//!
//! Pure + model-free (green-gate safe). The cold/taught probing happens in foreground bins.

/// A **goal as data**: instances + cold/taught prompts + a FREE verifier. Implement this (a small
/// struct + a verifier) to add a new goal — the awareness layer and the ratchet are generic over it,
/// so adding a goal needs no engine change. The cold/taught split is what makes the ZPD measurable:
/// `cold` (no help) is what the floor is measured on; `taught` (rule/scaffold in context) is how the
/// agent self-generates verified traces to practice.
pub trait Goal {
    type Instance: Clone;
    fn name(&self) -> &str;
    fn train(&self) -> Vec<Self::Instance>;
    /// Held-out instances — a floor-rise here is generalization, not memorization.
    fn test(&self) -> Vec<Self::Instance>;
    /// Prompt with NO help — what the cold floor is measured on.
    fn cold_prompt(&self, instance: &Self::Instance) -> String;
    /// Prompt with the rule/scaffold in context — used to self-generate verified traces.
    fn taught_prompt(&self, instance: &Self::Instance) -> String;
    /// FREE deterministic verifier: is `output` correct for this instance?
    fn verify(&self, instance: &Self::Instance, output: &str) -> bool;
}

/// Where an item sits relative to the agent's ability — measured by the verifier, not introspection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mastery {
    /// Solved WITHOUT help → already in the floor.
    Mastered,
    /// Failed alone but solved WITH help (rule/scaffold in context) → the ZPD: **learnable now**.
    Frontier,
    /// Failed even WITH help → beyond reach; needs an easier curriculum or a teacher first.
    Beyond,
}

/// Classify an item from its verifier outcomes: cold (no help) and taught (help in context).
pub fn classify(cold_pass: bool, taught_pass: bool) -> Mastery {
    if cold_pass {
        Mastery::Mastered
    } else if taught_pass {
        Mastery::Frontier
    } else {
        Mastery::Beyond
    }
}

/// A verifier-grounded capability map over items of type `I` — the agent's self-knowledge.
pub struct CapabilityMap<I> {
    entries: Vec<(I, Mastery)>,
}

impl<I: Clone> Default for CapabilityMap<I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Clone> CapabilityMap<I> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record one item's measured outcomes.
    pub fn record(&mut self, item: I, cold_pass: bool, taught_pass: bool) {
        self.entries.push((item, classify(cold_pass, taught_pass)));
    }

    fn of(&self, m: Mastery) -> Vec<I> {
        self.entries
            .iter()
            .filter(|(_, x)| *x == m)
            .map(|(i, _)| i.clone())
            .collect()
    }

    /// **The practice curriculum**: the ZPD — items learnable *now* (fail cold, pass taught).
    pub fn frontier(&self) -> Vec<I> {
        self.of(Mastery::Frontier)
    }
    pub fn mastered(&self) -> Vec<I> {
        self.of(Mastery::Mastered)
    }
    pub fn beyond(&self) -> Vec<I> {
        self.of(Mastery::Beyond)
    }

    pub fn total(&self) -> usize {
        self.entries.len()
    }

    /// `(mastered, frontier, beyond)` counts — the agent's awareness summary.
    pub fn summary(&self) -> (usize, usize, usize) {
        let mut m = (0, 0, 0);
        for (_, x) in &self.entries {
            match x {
                Mastery::Mastered => m.0 += 1,
                Mastery::Frontier => m.1 += 1,
                Mastery::Beyond => m.2 += 1,
            }
        }
        m
    }

    /// Current capability floor = fraction mastered (no help).
    pub fn floor(&self) -> f64 {
        if self.entries.is_empty() {
            return 0.0;
        }
        self.summary().0 as f64 / self.total() as f64
    }

    /// What the agent should DO next, derived from its self-knowledge:
    /// practice the frontier if any; else if items remain beyond reach, it needs scaffolding/teacher;
    /// else everything probed is mastered.
    pub fn next_action(&self) -> NextAction {
        let (_, frontier, beyond) = self.summary();
        if frontier > 0 {
            NextAction::PracticeFrontier(frontier)
        } else if beyond > 0 {
            NextAction::NeedScaffolding(beyond)
        } else {
            NextAction::AllMastered
        }
    }
}

/// The metacognitive decision: what to do given the current capability map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextAction {
    /// Practice these N frontier (learnable-now) items — the ratchet runs here.
    PracticeFrontier(usize),
    /// Nothing learnable alone right now; these N items need an easier curriculum or a teacher first.
    NeedScaffolding(usize),
    /// Everything probed is already in the floor.
    AllMastered,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_three_zones() {
        assert_eq!(classify(true, true), Mastery::Mastered);
        assert_eq!(classify(true, false), Mastery::Mastered); // cold pass dominates
        assert_eq!(classify(false, true), Mastery::Frontier); // the learnable-now zone
        assert_eq!(classify(false, false), Mastery::Beyond);
    }

    #[test]
    fn map_reports_frontier_as_the_curriculum() {
        let mut m = CapabilityMap::new();
        m.record("easy", true, true); // mastered
        m.record("op9", false, true); // frontier — fails cold, solves taught
        m.record("op7", false, true); // frontier
        m.record("hard", false, false); // beyond
        assert_eq!(m.summary(), (1, 2, 1));
        assert_eq!(m.frontier().len(), 2);
        assert!(m.frontier().contains(&"op9"));
        assert!((m.floor() - 0.25).abs() < 1e-9);
        assert_eq!(m.next_action(), NextAction::PracticeFrontier(2));
    }

    #[test]
    fn next_action_reflects_self_knowledge() {
        let mut all_mastered = CapabilityMap::new();
        all_mastered.record(1, true, true);
        assert_eq!(all_mastered.next_action(), NextAction::AllMastered);

        let mut stuck = CapabilityMap::new();
        stuck.record(1, false, false); // can't even do it with help
        assert_eq!(stuck.next_action(), NextAction::NeedScaffolding(1));
    }
}

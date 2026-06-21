//! `being-core-policy` — the §3.9 **policy / trust model**: one `Beta(alpha, beta)` per [`EffectClass`],
//! tracking *earned* trust per effect kind. Trust rises on attested-accepted effects and falls (harder)
//! on damage/rejection, decays geometrically toward a floor, and yields a conservative `TrustLevel` =
//! the **2.5th percentile of the Beta CDF** (one-sided lower 95% bound) via `statrs` for bit-stable
//! replay. This is the *dynamic* counterpart to the static `RiskPolicyCommitter` ceiling: trust is
//! learned over interactions and gates each class's capability ceiling. Pure, loop-safe, no model.

use statrs::distribution::{Beta, ContinuousCDF};

// §7 constants (defaults consistent with the spec's worked example; the exact §7 table is operator-set).
const TRUST_ALPHA_0: f64 = 0.5; // Jeffreys prior for the default classes
const TRUST_BETA_0: f64 = 0.5;
const TRUST_ALPHA_0_HS: f64 = 0.5; // high-stakes classes start more pessimistic (lower TrustLevel)
const TRUST_BETA_0_HS: f64 = 2.0;
const W_UP: f64 = 1.0; // trust earned per attested-accepted effect
const W_DOWN: f64 = 2.0; // trust lost per damage/rejection — ASYMMETRIC: W_DOWN > W_UP
const RHO: f64 = 0.99; // geometric decay per control-loop iteration
const TRUST_PCTILE: f64 = 0.025; // 2.5th percentile (two-sided 0.05 → one-sided lower 95%)

/// The trust axis: one Beta per effect kind. High-stakes classes (`Payment`/`Sign`/`Http`) carry a
/// more pessimistic prior, so they must *earn* their way up from a lower start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectClass {
    Query,
    MemoryWrite,
    Http,
    Sign,
    Payment,
}

impl EffectClass {
    pub const ALL: [EffectClass; 5] = [
        EffectClass::Query,
        EffectClass::MemoryWrite,
        EffectClass::Http,
        EffectClass::Sign,
        EffectClass::Payment,
    ];

    pub fn is_high_stakes(self) -> bool {
        matches!(
            self,
            EffectClass::Payment | EffectClass::Sign | EffectClass::Http
        )
    }

    fn prior(self) -> (f64, f64) {
        if self.is_high_stakes() {
            (TRUST_ALPHA_0_HS, TRUST_BETA_0_HS)
        } else {
            (TRUST_ALPHA_0, TRUST_BETA_0)
        }
    }

    fn idx(self) -> usize {
        match self {
            EffectClass::Query => 0,
            EffectClass::MemoryWrite => 1,
            EffectClass::Http => 2,
            EffectClass::Sign => 3,
            EffectClass::Payment => 4,
        }
    }
}

/// A trust ledger: per-`EffectClass` `Beta(alpha, beta)`. Live trust is fully derivable by folding the
/// journaled observations over replay (no separate snapshot event), matching §3.9.
#[derive(Clone, Debug)]
pub struct TrustLedger {
    /// `(alpha, beta)` per class, indexed by [`EffectClass::idx`]. Both always `> 0` (priors + clamped
    /// decay), so the `statrs` inverse-CDF never panics.
    betas: [(f64, f64); 5],
}

impl Default for TrustLedger {
    fn default() -> Self {
        Self::genesis()
    }
}

impl TrustLedger {
    /// Genesis state: each class seeded with its prior (high-stakes lower).
    pub fn genesis() -> Self {
        let mut betas = [(0.0, 0.0); 5];
        for c in EffectClass::ALL {
            betas[c.idx()] = c.prior();
        }
        Self { betas }
    }

    /// The conservative `TrustLevel` for `class`: the 2.5th percentile of its `Beta(alpha, beta)` — a
    /// one-sided lower 95% bound, so a class only reads as trusted once it has *consistent* evidence.
    pub fn trust_level(&self, class: EffectClass) -> f64 {
        let (a, b) = self.betas[class.idx()];
        Beta::new(a, b)
            .expect("alpha,beta > 0 by construction")
            .inverse_cdf(TRUST_PCTILE)
    }

    /// Trust band L0..L4 (each 0.2 wide), for coarse ceiling gating.
    pub fn band(&self, class: EffectClass) -> u8 {
        let t = self.trust_level(class);
        ((t.clamp(0.0, 0.999_999) / 0.2) as u8).min(4)
    }

    /// An attested-accepted effect of `class` raises its trust (`alpha += W_UP`).
    pub fn observe_accepted(&mut self, class: EffectClass) {
        self.betas[class.idx()].0 += W_UP;
    }

    /// Damage / rejection / overrun for `class` lowers its trust (`beta += W_DOWN`), harder than a
    /// success raises it (`W_DOWN > W_UP`) — trust is slow to earn, quick to lose.
    pub fn observe_damage(&mut self, class: EffectClass) {
        self.betas[class.idx()].1 += W_DOWN;
    }

    /// One geometric decay tick (once per control-loop iteration), clamped to the per-class prior floor
    /// so both parameters stay `> 0` (inverse-CDF bit-stability on long idle runs).
    pub fn decay(&mut self) {
        for c in EffectClass::ALL {
            let (a0, b0) = c.prior();
            let (a, b) = &mut self.betas[c.idx()];
            *a = (*a * RHO).max(a0);
            *b = (*b * RHO).max(b0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_default_prior_matches_the_worked_example() {
        let l = TrustLedger::genesis();
        // Beta(0.5,0.5) 2.5th percentile ≈ 0.0015 → band L0.
        let t = l.trust_level(EffectClass::Query);
        assert!((t - 0.0015).abs() < 0.001, "got {t}");
        assert_eq!(l.band(EffectClass::Query), 0);
    }

    #[test]
    fn one_success_raises_trust_but_not_out_of_l0() {
        let mut l = TrustLedger::genesis();
        let genesis = l.trust_level(EffectClass::Query);
        l.observe_accepted(EffectClass::Query); // Beta(1.5, 0.5)
        let t = l.trust_level(EffectClass::Query);
        // One success raises trust off the floor but the conservative lower bound stays in band L0.
        assert!(t > genesis && t < 0.2, "got {t}");
        assert_eq!(l.band(EffectClass::Query), 0);
    }

    #[test]
    fn trust_is_earned_slowly_and_lost_quickly() {
        // Compare a success vs a damage delta from the SAME near-symmetric mid-range point, where the
        // percentile responds ~linearly to the Beta params — so the W_DOWN > W_UP asymmetry shows.
        fn mid() -> TrustLedger {
            let mut l = TrustLedger::genesis();
            for _ in 0..8 {
                l.observe_accepted(EffectClass::Query); // alpha → 8.5
            }
            for _ in 0..4 {
                l.observe_damage(EffectClass::Query); // beta → 0.5 + 4*W_DOWN = 8.5
            }
            l
        }
        let base = mid().trust_level(EffectClass::Query);
        let up = {
            let mut l = mid();
            l.observe_accepted(EffectClass::Query);
            l.trust_level(EffectClass::Query)
        };
        let down = {
            let mut l = mid();
            l.observe_damage(EffectClass::Query);
            l.trust_level(EffectClass::Query)
        };
        assert!(up > base, "a success should raise trust");
        assert!(down < base, "damage should lower trust");
        assert!(
            base - down > up - base,
            "W_DOWN > W_UP: damage {} should outweigh a single success {}",
            base - down,
            up - base
        );
    }

    #[test]
    fn high_stakes_classes_start_less_trusted() {
        let l = TrustLedger::genesis();
        assert!(l.trust_level(EffectClass::Payment) < l.trust_level(EffectClass::Query));
        assert!(EffectClass::Payment.is_high_stakes() && !EffectClass::Query.is_high_stakes());
    }

    #[test]
    fn many_successes_climb_bands() {
        let mut l = TrustLedger::genesis();
        for _ in 0..50 {
            l.observe_accepted(EffectClass::Query);
        }
        assert!(
            l.band(EffectClass::Query) >= 3,
            "band {}",
            l.band(EffectClass::Query)
        );
    }

    #[test]
    fn decay_relaxes_toward_the_prior_floor_without_underflow() {
        let mut l = TrustLedger::genesis();
        for _ in 0..10 {
            l.observe_accepted(EffectClass::Query);
        }
        let high = l.trust_level(EffectClass::Query);
        for _ in 0..10_000 {
            l.decay();
        }
        let relaxed = l.trust_level(EffectClass::Query);
        assert!(relaxed < high);
        let genesis = TrustLedger::genesis().trust_level(EffectClass::Query);
        assert!(
            (relaxed - genesis).abs() < 1e-9,
            "relaxed {relaxed} vs genesis {genesis}"
        );
    }
}

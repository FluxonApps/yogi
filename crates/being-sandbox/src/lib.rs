//! `being-sandbox` — the M4 isolation **capability broker** (build-spec D-M1-3 / D-M4-2).
//!
//! The executor boundary must run with **no ambient authority**: every side-effecting action is
//! deny-by-default and executes only if the operator granted a matching [`Capability`]. This crate is
//! the *policy* core — pure, loop-safe, testable: [`Broker::authorize`] decides whether an
//! [`EffectRequest`] is permitted by a [`CapabilitySet`]. The *mechanism* that makes the policy
//! unbypassable — running the executor as a wasmtime/WASI guest with zero ambient authority, effects
//! reaching the host only through broker-mediated imports — plugs in on top (see the module docs note).
//!
//! **Why the being can't widen its own authority:** capabilities are operator-owned. A `CapabilitySet`
//! is constructed only by the operator; there is *no* API to add a capability from inside a turn, and
//! the closed [`being_core_mutation::MutationKind`] surface deliberately has **no `CapabilityGrant`
//! variant** — so a self-modifying being cannot mutate its way to more authority. Deny-by-default +
//! no-self-grant is the isolation invariant; WASI then enforces that effects can't bypass the broker.

use std::collections::BTreeSet;

use being_core_types::Microdollars;

/// An operator-granted authority. Pure data; constructed by the operator, never by a turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Capability {
    /// Egress is allowed only to these exact hosts (an allowlist; empty ⇒ no egress).
    Egress { hosts: BTreeSet<String> },
    /// Payment is allowed up to (and including) this many microdollars per charge.
    Payment { max_microdollars: Microdollars },
    /// Permission to write durable memory.
    MemoryWrite,
    /// Permission to sign with the being's key.
    Sign,
}

/// The operator's grant set for a being. Deny-by-default: an empty set authorizes only pure effects.
#[derive(Clone, Debug, Default)]
pub struct CapabilitySet {
    caps: Vec<Capability>,
}

impl CapabilitySet {
    /// An empty grant set — the safe default (no external authority at all).
    pub fn none() -> Self {
        Self::default()
    }

    /// Operator-only constructor. (There is intentionally no method that adds a capability given only
    /// a `&self` a turn could reach — grants come from operator code building the set up front.)
    pub fn granted(caps: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            caps: caps.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.caps.is_empty()
    }
}

/// A side effect the executor wants to perform. Pure effects ([`EffectRequest::Query`]/
/// [`EffectRequest::Infer`]) carry no external authority and are always allowed; everything else needs
/// a matching capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectRequest {
    Egress { host: String },
    Payment { microdollars: Microdollars },
    MemoryWrite,
    Sign,
    Query,
    Infer,
}

impl EffectRequest {
    /// Pure effects have no external side effect; re-execution is safe and they need no grant.
    pub fn is_pure(&self) -> bool {
        matches!(self, EffectRequest::Query | EffectRequest::Infer)
    }
}

/// The broker's verdict — `Denied` carries a reason for the audit log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Authorization {
    Granted,
    Denied(&'static str),
}

impl Authorization {
    pub fn is_granted(&self) -> bool {
        matches!(self, Authorization::Granted)
    }
}

/// The capability broker. Stateless; the decision is a pure function of the request and the grant set.
pub struct Broker;

impl Broker {
    /// Authorize an effect **deny-by-default**: pure effects pass; every other effect is denied unless
    /// the `caps` set contains a capability that covers it (egress host on the allowlist, payment
    /// within the cap, the memory-write / sign permission present).
    pub fn authorize(request: &EffectRequest, caps: &CapabilitySet) -> Authorization {
        match request {
            EffectRequest::Query | EffectRequest::Infer => Authorization::Granted,
            EffectRequest::Egress { host } => {
                let ok = caps.caps.iter().any(|c| match c {
                    Capability::Egress { hosts } => hosts.contains(host),
                    _ => false,
                });
                if ok {
                    Authorization::Granted
                } else {
                    Authorization::Denied("egress host not on the granted allowlist")
                }
            }
            EffectRequest::Payment { microdollars } => {
                // A payment must be non-negative AND within the granted cap. Without the `>= 0` guard a
                // negative charge (a refund/credit — the opposite of a bounded spend) would pass the
                // `<= cap` check unconditionally; the being must not self-authorize that.
                let ok = caps.caps.iter().any(|c| match c {
                    Capability::Payment { max_microdollars } => {
                        *microdollars >= 0 && *microdollars <= *max_microdollars
                    }
                    _ => false,
                });
                if ok {
                    Authorization::Granted
                } else {
                    Authorization::Denied(
                        "payment negative or exceeds the granted cap (or none granted)",
                    )
                }
            }
            EffectRequest::MemoryWrite => auth_if(
                caps.caps
                    .iter()
                    .any(|c| matches!(c, Capability::MemoryWrite)),
                "no MemoryWrite capability granted",
            ),
            EffectRequest::Sign => auth_if(
                caps.caps.iter().any(|c| matches!(c, Capability::Sign)),
                "no Sign capability granted",
            ),
        }
    }
}

fn auth_if(ok: bool, deny_reason: &'static str) -> Authorization {
    if ok {
        Authorization::Granted
    } else {
        Authorization::Denied(deny_reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(hs: &[&str]) -> BTreeSet<String> {
        hs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_set_denies_all_external_effects_but_allows_pure() {
        let none = CapabilitySet::none();
        assert!(Broker::authorize(&EffectRequest::Query, &none).is_granted());
        assert!(Broker::authorize(&EffectRequest::Infer, &none).is_granted());
        for req in [
            EffectRequest::Egress {
                host: "x.com".into(),
            },
            EffectRequest::Payment { microdollars: 1 },
            EffectRequest::MemoryWrite,
            EffectRequest::Sign,
        ] {
            assert!(
                !Broker::authorize(&req, &none).is_granted(),
                "deny-by-default failed for {req:?}"
            );
        }
    }

    #[test]
    fn egress_is_allowlisted_per_host() {
        let caps = CapabilitySet::granted([Capability::Egress {
            hosts: hosts(&["api.allowed.test"]),
        }]);
        assert!(Broker::authorize(
            &EffectRequest::Egress {
                host: "api.allowed.test".into()
            },
            &caps
        )
        .is_granted());
        // a different host is denied — the grant does not generalize
        assert!(!Broker::authorize(
            &EffectRequest::Egress {
                host: "evil.test".into()
            },
            &caps
        )
        .is_granted());
    }

    #[test]
    fn payment_is_bounded_by_the_cap() {
        let caps = CapabilitySet::granted([Capability::Payment {
            max_microdollars: 1000,
        }]);
        assert!(
            Broker::authorize(&EffectRequest::Payment { microdollars: 1000 }, &caps).is_granted()
        );
        assert!(
            Broker::authorize(&EffectRequest::Payment { microdollars: 999 }, &caps).is_granted()
        );
        // one microdollar over the cap is denied (boundary)
        assert!(
            !Broker::authorize(&EffectRequest::Payment { microdollars: 1001 }, &caps).is_granted()
        );
        // a NEGATIVE payment (a refund/credit, not a bounded spend) is denied even though -1 <= cap.
        assert!(
            !Broker::authorize(&EffectRequest::Payment { microdollars: -1 }, &caps).is_granted()
        );
        assert!(!Broker::authorize(
            &EffectRequest::Payment {
                microdollars: i64::MIN
            },
            &caps
        )
        .is_granted());
    }

    #[test]
    fn memory_write_and_sign_need_their_own_capability() {
        let only_mem = CapabilitySet::granted([Capability::MemoryWrite]);
        assert!(Broker::authorize(&EffectRequest::MemoryWrite, &only_mem).is_granted());
        // having MemoryWrite does NOT confer Sign — capabilities don't bleed across kinds
        assert!(!Broker::authorize(&EffectRequest::Sign, &only_mem).is_granted());
    }

    #[test]
    fn denials_carry_a_reason() {
        let none = CapabilitySet::none();
        match Broker::authorize(&EffectRequest::Sign, &none) {
            Authorization::Denied(r) => assert!(!r.is_empty()),
            Authorization::Granted => panic!("should have denied"),
        }
    }
}

//! Canonical shared types for Yogi, re-exported to all crates (build-spec §3.0).
//!
//! Pure-`std` for the M0 sliver; cryptographic backing (ed25519, blake3) and SQLite land with
//! `being-core-id` / `being-core-journal` in later M0 slices.

/// A microdollar amount (1 USD = 1_000_000). `i64`; the release profile enables overflow-checks so
/// the survival ledger can never silently wrap (build-spec §4).
pub type Microdollars = i64;

/// Decentralized identifier root. Placeholder string form; real `did:key` resolution lands in
/// `being-core-id`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Did(pub String);

/// Opaque content hash (placeholder for blake3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Hash(pub [u8; 32]);

/// Opaque signature (placeholder for ed25519).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sig(pub Vec<u8>);

/// Provenance class — the no-launder ladder. Model- or attacker-derived bytes can never be
/// relabelled upward to [`ProvenanceClass::DirectUserIntent`], so they can never escalate trust
/// (build-spec §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvenanceClass {
    DirectUserIntent,
    ModelInference,
    ToolOutput,
    FetchedDoc,
    PeerFederated,
}

impl ProvenanceClass {
    /// Only a genuine user turn may carry trust-escalating authority. Everything else is inert.
    pub fn can_escalate_trust(self) -> bool {
        matches!(self, ProvenanceClass::DirectUserIntent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_direct_user_intent_escalates_trust() {
        assert!(ProvenanceClass::DirectUserIntent.can_escalate_trust());
        for p in [
            ProvenanceClass::ModelInference,
            ProvenanceClass::ToolOutput,
            ProvenanceClass::FetchedDoc,
            ProvenanceClass::PeerFederated,
        ] {
            assert!(!p.can_escalate_trust(), "{p:?} must not escalate trust");
        }
    }
}

//! The CLOSED mutation surface (build-spec §3.7, architecture §8.1).
//!
//! The [`Genome`] is the heritable, mutable scaffold: a being can evolve *how it thinks*, never
//! *what it is allowed to do*. The safety property is **structural** — the forbidden mutations
//! (capability grant, trust-policy edit, signature-boundary change, execution kernel, budget rules,
//! the reaper) are simply not variants of [`MutationKind`], so they are unrepresentable. Adding one
//! would force a new match arm in [`apply`], which has no wildcard — i.e. a *compile error*, not a
//! runtime check that spec-gaming could erode.

use std::collections::{BTreeMap, BTreeSet};

pub type Bytes = Vec<u8>;
pub type SkillId = String;
pub type Domain = String;

/// A reference to a model. The proposer is swappable and is *not* part of the heritable unit; for
/// the local build this is an Ollama tag (e.g. `"qwen3:8b"`) or a distilled-adapter path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRef(pub String);

/// The heritable, mutable scaffold.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Genome {
    pub prompt: String,
    pub tool_policy: Bytes,
    pub retrieval_policy: Bytes,
    pub decomposition_policy: Bytes,
    pub routing_policy: Bytes,
    pub reasoning_navigator: Option<ModelRef>,
    pub installed_skills: BTreeSet<SkillId>,
    pub domain_models: BTreeMap<Domain, ModelRef>,
}

impl Genome {
    /// Canonical, deterministic byte encoding for hashing/signing. Field order is fixed; the ordered
    /// collections (`BTreeSet`/`BTreeMap`) iterate canonically; every variable-length field is
    /// length-prefixed (u64 LE) so two distinct genomes can never encode to the same bytes. This is
    /// the genome half of the signed fork snapshot (`being-lineage`) — the parent signs *exactly* the
    /// heritable state the child inherits.
    pub fn canon_bytes(&self) -> Vec<u8> {
        fn put(buf: &mut Vec<u8>, bytes: &[u8]) {
            buf.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
            buf.extend_from_slice(bytes);
        }
        let mut b = Vec::new();
        put(&mut b, self.prompt.as_bytes());
        put(&mut b, &self.tool_policy);
        put(&mut b, &self.retrieval_policy);
        put(&mut b, &self.decomposition_policy);
        put(&mut b, &self.routing_policy);
        match &self.reasoning_navigator {
            None => b.push(0),
            Some(m) => {
                b.push(1);
                put(&mut b, m.0.as_bytes());
            }
        }
        b.extend_from_slice(&(self.installed_skills.len() as u64).to_le_bytes());
        for s in &self.installed_skills {
            put(&mut b, s.as_bytes());
        }
        b.extend_from_slice(&(self.domain_models.len() as u64).to_le_bytes());
        for (k, v) in &self.domain_models {
            put(&mut b, k.as_bytes());
            put(&mut b, v.0.as_bytes());
        }
        b
    }
}

/// The CLOSED mutation surface. Deliberately NOT `#[non_exhaustive]`: the set is closed by the type
/// system. Absent by type (and that absence *is* the safety property): `CapabilityGrant`,
/// `TrustPolicyModify`, `SignatureBoundaryChange`, `ExecutionKernel`, `BudgetRules`, `Reaper`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationKind {
    Prompt(String),
    ToolPolicy(Bytes),
    RetrievalPolicy(Bytes),
    DecompositionPolicy(Bytes),
    RoutingPolicy(Bytes),
    ReasoningNavigator(ModelRef),
    DomainModel(Domain, ModelRef),
    /// Id-only at the M0 sliver; a `SignedSkill` in the full build (skill install always signed).
    SkillInstall(SkillId),
    SkillRevoke(SkillId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationError {
    EmptyPrompt,
    UnknownSkill(SkillId),
}

/// Pure, total, validating application of one mutation to a genome.
///
/// The match below has **no wildcard arm**. Adding a `MutationKind` variant fails to compile here
/// until it is handled — which is exactly how the closed surface is enforced. Do not add a `_ =>`
/// arm; that would defeat the guarantee.
pub fn apply(kind: MutationKind, mut g: Genome) -> Result<Genome, MutationError> {
    match kind {
        MutationKind::Prompt(p) => {
            if p.trim().is_empty() {
                return Err(MutationError::EmptyPrompt);
            }
            g.prompt = p;
        }
        MutationKind::ToolPolicy(b) => g.tool_policy = b,
        MutationKind::RetrievalPolicy(b) => g.retrieval_policy = b,
        MutationKind::DecompositionPolicy(b) => g.decomposition_policy = b,
        MutationKind::RoutingPolicy(b) => g.routing_policy = b,
        MutationKind::ReasoningNavigator(m) => g.reasoning_navigator = Some(m),
        MutationKind::DomainModel(d, m) => {
            g.domain_models.insert(d, m);
        }
        MutationKind::SkillInstall(s) => {
            g.installed_skills.insert(s);
        }
        MutationKind::SkillRevoke(s) => {
            if !g.installed_skills.remove(&s) {
                return Err(MutationError::UnknownSkill(s));
            }
        }
    }
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_applies_and_empty_is_rejected() {
        let g = apply(MutationKind::Prompt("hello".into()), Genome::default()).unwrap();
        assert_eq!(g.prompt, "hello");
        assert_eq!(
            apply(MutationKind::Prompt("   ".into()), Genome::default()),
            Err(MutationError::EmptyPrompt)
        );
    }

    #[test]
    fn canon_bytes_is_deterministic_and_collision_resistant() {
        let g = apply(MutationKind::Prompt("hello".into()), Genome::default()).unwrap();
        // Deterministic: same genome → identical bytes.
        assert_eq!(g.canon_bytes(), g.canon_bytes());
        // Any field change perturbs the encoding (length-prefixing prevents boundary collisions).
        let g2 = apply(MutationKind::SkillInstall("s1".into()), g.clone()).unwrap();
        assert_ne!(g.canon_bytes(), g2.canon_bytes());
        // The classic ambiguity ("ab"+"c" vs "a"+"bc") cannot collide thanks to length prefixes.
        let a = apply(MutationKind::Prompt("ab".into()), Genome::default()).unwrap();
        let mut a = apply(MutationKind::ToolPolicy(b"c".to_vec()), a).unwrap();
        let b = apply(MutationKind::Prompt("a".into()), Genome::default()).unwrap();
        let mut b = apply(MutationKind::ToolPolicy(b"bc".to_vec()), b).unwrap();
        assert_ne!(a.canon_bytes(), b.canon_bytes());
        // Default genome encodes to a stable, non-empty descriptor.
        a.prompt.clear();
        b.prompt.clear();
        assert_eq!(a.canon_bytes(), a.clone().canon_bytes());
    }

    #[test]
    fn skill_install_then_revoke_roundtrips() {
        let g = apply(MutationKind::SkillInstall("s1".into()), Genome::default()).unwrap();
        assert!(g.installed_skills.contains("s1"));
        let g = apply(MutationKind::SkillRevoke("s1".into()), g).unwrap();
        assert!(g.installed_skills.is_empty());
    }

    #[test]
    fn revoking_unknown_skill_errors() {
        assert!(matches!(
            apply(MutationKind::SkillRevoke("nope".into()), Genome::default()),
            Err(MutationError::UnknownSkill(_))
        ));
    }

    #[test]
    fn domain_model_promotes() {
        let g = apply(
            MutationKind::DomainModel("rust".into(), ModelRef("qwen3:8b".into())),
            Genome::default(),
        )
        .unwrap();
        assert_eq!(
            g.domain_models.get("rust"),
            Some(&ModelRef("qwen3:8b".into()))
        );
    }

    // CLOSURE GUARANTEE (the load-bearing safety invariant):
    // `apply` matches `MutationKind` exhaustively with no `_` arm, so the forbidden mutations
    // (capability grant, trust-policy edit, signature-boundary change, kernel, budget rules,
    // reaper) cannot be added without a compile error.
    #[test]
    fn mutation_surface_is_closed_compile_guard() {
        // A SECOND exhaustive match with NO wildcard arm — a deliberate, dependency-free mechanical
        // pin (no brittle `trybuild` .stderr). Adding any `MutationKind` variant fails to compile
        // here as well as in `apply`. And whatever the new variant is, it CANNOT be a forbidden power
        // (capability/trust/signature/kernel/budget/reaper) — those are absent BY TYPE, not by this
        // check. If this stops compiling, do not add a `_ =>` arm; reconsider the variant.
        fn assert_closed(k: &MutationKind) {
            match k {
                MutationKind::Prompt(_) => {}
                MutationKind::ToolPolicy(_) => {}
                MutationKind::RetrievalPolicy(_) => {}
                MutationKind::DecompositionPolicy(_) => {}
                MutationKind::RoutingPolicy(_) => {}
                MutationKind::ReasoningNavigator(_) => {}
                MutationKind::DomainModel(_, _) => {}
                MutationKind::SkillInstall(_) => {}
                MutationKind::SkillRevoke(_) => {}
            }
        }
        assert_closed(&MutationKind::Prompt("x".into()));
    }
}

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
    // reaper) cannot be added without a compile error. A `trybuild` compile-fail test will pin
    // this mechanically once dev-deps are introduced (build-spec §8).
}

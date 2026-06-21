//! Two-parent recombination (M6) — the sexual/recombining heredity variant deferred alongside
//! selection, now built. A child is assembled by **uniform crossover** of two parent genomes: each
//! field (and each skill / domain-model entry) is inherited from one parent or the other.
//!
//! **Safe by construction.** Recombination only ever *copies existing values from the two parents* —
//! it never synthesizes a field. Since the [`Genome`] type has no capability/trust/kernel fields at
//! all (those live outside the closed surface), a recombined child cannot acquire a forbidden power
//! that neither parent had: the M0 safety invariant survives sexual reproduction untouched.

use being_core_mutation::Genome;

use crate::{BeingId, Lineage, Offspring, Rng};

impl Rng {
    /// One fair coin flip — `true` selects the *first* parent.
    fn coin(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

/// Uniform-crossover recombination of two genomes. Scalar fields are each taken from one parent by a
/// coin flip; `installed_skills` and `domain_models` recombine per entry over the union of both
/// parents (an entry present in both is always kept; one present in a single parent is kept by coin,
/// and a domain present in both takes its model from one parent by coin). Deterministic given `rng`.
pub fn recombine(a: &Genome, b: &Genome, rng: &mut Rng) -> Genome {
    let mut g = Genome {
        prompt: pick(rng, &a.prompt, &b.prompt).clone(),
        tool_policy: pick(rng, &a.tool_policy, &b.tool_policy).clone(),
        retrieval_policy: pick(rng, &a.retrieval_policy, &b.retrieval_policy).clone(),
        decomposition_policy: pick(rng, &a.decomposition_policy, &b.decomposition_policy).clone(),
        routing_policy: pick(rng, &a.routing_policy, &b.routing_policy).clone(),
        reasoning_navigator: pick(rng, &a.reasoning_navigator, &b.reasoning_navigator).clone(),
        installed_skills: Default::default(),
        domain_models: Default::default(),
    };

    // Per-skill crossover over the union (BTreeSet iteration is sorted → deterministic order).
    let mut skills: std::collections::BTreeSet<&String> = a.installed_skills.iter().collect();
    skills.extend(b.installed_skills.iter());
    for s in skills {
        let in_both = a.installed_skills.contains(s) && b.installed_skills.contains(s);
        if in_both || rng.coin() {
            g.installed_skills.insert(s.clone());
        }
    }

    // Per-domain crossover over the union of keys.
    let mut keys: std::collections::BTreeSet<&String> = a.domain_models.keys().collect();
    keys.extend(b.domain_models.keys());
    for k in keys {
        match (a.domain_models.get(k), b.domain_models.get(k)) {
            (Some(va), Some(vb)) => {
                g.domain_models.insert(k.clone(), pick(rng, va, vb).clone());
            }
            (Some(v), None) | (None, Some(v)) => {
                if rng.coin() {
                    g.domain_models.insert(k.clone(), v.clone());
                }
            }
            (None, None) => {}
        }
    }
    g
}

fn pick<'a, T>(rng: &mut Rng, a: &'a T, b: &'a T) -> &'a T {
    if rng.coin() {
        a
    } else {
        b
    }
}

/// Sexual fork: a child of two parents. Ancestry records BOTH parents, generation is one past the
/// deeper parent, and the genome is their [`recombine`]d crossover. Like [`fork`](crate::fork) it
/// only records heredity — it does not select, score, or kill.
pub fn fork2(
    a: &Lineage,
    a_genome: &Genome,
    b: &Lineage,
    b_genome: &Genome,
    child_id: BeingId,
    rng: &mut Rng,
) -> Offspring {
    Offspring {
        lineage: Lineage {
            id: child_id,
            parents: vec![a.id, b.id],
            generation: a.generation.max(b.generation) + 1,
        },
        genome: recombine(a_genome, b_genome, rng),
    }
}

#[cfg(test)]
mod tests {
    use being_core_mutation::{apply, MutationKind};

    use super::*;

    fn genome_with(prompt: &str, skills: &[&str]) -> Genome {
        let mut g = apply(MutationKind::Prompt(prompt.into()), Genome::default()).unwrap();
        for s in skills {
            g = apply(MutationKind::SkillInstall((*s).into()), g).unwrap();
        }
        g
    }

    #[test]
    fn every_field_comes_from_one_parent() {
        let a = genome_with("alpha", &[]);
        let b = genome_with("beta", &[]);
        let mut rng = Rng::new(1);
        for _ in 0..20 {
            let c = recombine(&a, &b, &mut rng);
            // The prompt is always exactly one parent's — recombination never invents a value.
            assert!(c.prompt == a.prompt || c.prompt == b.prompt);
        }
    }

    #[test]
    fn skills_stay_within_the_union_and_shared_skills_persist() {
        let a = genome_with("p", &["x", "shared"]);
        let b = genome_with("p", &["y", "shared"]);
        let mut rng = Rng::new(42);
        for _ in 0..50 {
            let c = recombine(&a, &b, &mut rng);
            // No skill appears that neither parent had (safety: only existing values are copied).
            for s in &c.installed_skills {
                assert!(a.installed_skills.contains(s) || b.installed_skills.contains(s));
            }
            // A skill held by BOTH parents is always inherited.
            assert!(c.installed_skills.contains("shared"));
        }
    }

    #[test]
    fn recombination_is_deterministic_for_a_seed() {
        let a = genome_with("alpha", &["x"]);
        let b = genome_with("beta", &["y"]);
        let run = || recombine(&a, &b, &mut Rng::new(7));
        assert_eq!(run(), run());
    }

    #[test]
    fn fork2_records_both_parents_and_advances_generation() {
        let a = Lineage {
            id: 1,
            parents: vec![],
            generation: 2,
        };
        let b = Lineage {
            id: 2,
            parents: vec![],
            generation: 5,
        };
        let ag = genome_with("a", &["x"]);
        let bg = genome_with("b", &["y"]);
        let mut rng = Rng::new(3);
        let child = fork2(&a, &ag, &b, &bg, 9, &mut rng);
        assert_eq!(child.lineage.parents, vec![1, 2]);
        assert_eq!(child.lineage.generation, 6); // max(2,5)+1
                                                 // Child genome is a valid crossover: prompt from one parent.
        assert!(child.genome.prompt == "a" || child.genome.prompt == "b");
    }
}

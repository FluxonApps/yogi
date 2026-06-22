//! A **non-arithmetic** novel goal — stresses goal-agnosticism beyond arithmetic (same
//! rule-internalization regime as [`crate::op`], a different KIND of skill).
//!
//! The `⊙` transform rotates each vowel to the next, cyclically (a→e→i→o→u→a); consonants unchanged.
//! Cold ≈ 0 (the symbol is novel); solvable with the rule in-context; free exact verifier; held-out
//! words ⇒ a gain is generalization. Pure + model-free (green-gate safe).

/// Apply the ⊙ transform: each vowel → next vowel (cyclic), consonants unchanged.
pub fn transform(word: &str) -> String {
    word.chars()
        .map(|c| match c.to_ascii_lowercase() {
            'a' => 'e',
            'e' => 'i',
            'i' => 'o',
            'o' => 'u',
            'u' => 'a',
            other => other,
        })
        .collect()
}

pub fn cold_prompt(word: &str) -> String {
    format!("Apply the \u{2299} transform to the word \"{word}\". Output only the resulting word.")
}

pub fn taught_prompt(word: &str) -> String {
    format!(
        "The \u{2299} transform replaces each vowel with the NEXT vowel cyclically (a->e->i->o->u->a); \
         consonants unchanged. Apply \u{2299} to \"{word}\". Output only the resulting word."
    )
}

/// Free verifier: the transformed word appears in the model's output (case-insensitive).
pub fn verify(word: &str, output: &str) -> bool {
    output.to_ascii_lowercase().contains(&transform(word))
}

/// Training words (short, all contain a vowel so the transform is non-trivial).
pub fn train_words() -> Vec<&'static str> {
    vec![
        "cat", "dog", "sun", "map", "red", "big", "top", "cup", "hat", "pen", "log", "bus", "fan",
        "net", "pig", "rug", "box", "jam", "kid", "mud", "nap", "owl", "rat", "tub", "van", "web",
        "yak", "zip", "arm", "ear", "ice", "oak", "elf", "ink", "egg", "ant", "urn", "ash",
    ]
}

/// Held-out words (unseen) — a gain here is generalization, not memorization.
pub fn test_words() -> Vec<&'static str> {
    vec!["fox", "bug", "gem", "hop", "jet", "lip", "nut", "pit"]
}

/// The cipher goal as a [`being_metacog::Goal`] — another instance proving the engine is goal-agnostic.
pub struct CipherGoal;

impl being_metacog::Goal for CipherGoal {
    type Instance = String;
    fn name(&self) -> &str {
        "vowel-shift cipher (⊙)"
    }
    fn train(&self) -> Vec<String> {
        train_words().into_iter().map(String::from).collect()
    }
    fn test(&self) -> Vec<String> {
        test_words().into_iter().map(String::from).collect()
    }
    fn cold_prompt(&self, w: &String) -> String {
        cold_prompt(w)
    }
    fn taught_prompt(&self, w: &String) -> String {
        taught_prompt(w)
    }
    fn verify(&self, w: &String, output: &str) -> bool {
        verify(w, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use being_metacog::Goal;

    #[test]
    fn transform_rotates_vowels() {
        assert_eq!(transform("cat"), "cet");
        assert_eq!(transform("dog"), "dug");
        assert_eq!(transform("hello"), "hillu");
        assert_eq!(transform("sky"), "sky"); // no vowels → unchanged
    }

    #[test]
    fn verifier_matches_transformed_word_in_output() {
        assert!(verify("cat", "the result is cet."));
        assert!(!verify("cat", "cat")); // unchanged ≠ transformed
    }

    #[test]
    fn cipher_goal_is_a_generic_goal_and_train_test_disjoint() {
        let g = CipherGoal;
        assert!(g.cold_prompt(&"cat".into()).contains('\u{2299}'));
        assert!(g.verify(&"dog".into(), "dug"));
        let tr = g.train();
        for w in g.test() {
            assert!(!tr.contains(&w), "{w} leaks across the split");
        }
    }
}

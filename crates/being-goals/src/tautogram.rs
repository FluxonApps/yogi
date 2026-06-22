//! `being-tautogram` — a **free-verifier goal** for the democratization ratchet (docs/plan P1).
//!
//! Goal: *given a letter `L` and a count `K`, write a `K`-word sentence where every word starts with
//! `L`* (a tautogram). The point is a **deterministic, free, Goodhart-resistant verifier** — no frontier
//! judge, no salary — so we can test "does distilling the being's own verified successes raise its
//! held-out floor?" at zero cloud cost (the purest democratization setup). Train on one set of letters,
//! test on a **held-out** set, so a gain reflects a learned *skill*, not memorized letters.
//!
//! This module is **pure + model-free** (green-gate safe). The model runs only in foreground bins.

/// Default word count for the task.
pub const K: usize = 5;

/// Letters the being trains on (verified-success traces are generated for these).
pub fn train_letters() -> Vec<char> {
    "bcdfghmprstw".chars().collect()
}

/// **Held-out** letters used only for evaluation — a floor-rise here is generalization, not memory.
pub fn test_letters() -> Vec<char> {
    "lnak".chars().collect()
}

/// The instruction shown to the model for one instance.
pub fn task_prompt(letter: char, k: usize) -> String {
    let u = letter.to_ascii_uppercase();
    format!(
        "Write a sentence of exactly {k} words where EVERY word begins with the letter \"{u}\". \
         Use real, distinct words of at least 3 letters. Output ONLY the sentence."
    )
}

/// Lowercase alphabetic word tokens (punctuation stripped).
fn words(sentence: &str) -> Vec<String> {
    sentence
        .split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_ascii_alphabetic())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// **Free verifier** → score in `[0, 1]`. `coverage × purity`:
/// - *coverage* = distinct words that start with `letter` and are ≥3 chars, over the target `k`;
/// - *purity*   = fraction of all words that start with `letter` (mixing in a non-`L` word is penalized).
///
/// Goodhart-resistant: too-short words and duplicates don't earn coverage; off-letter words cut purity.
pub fn verify(letter: char, k: usize, sentence: &str) -> f64 {
    let letter = letter.to_ascii_lowercase();
    let ws = words(sentence);
    if ws.is_empty() {
        return 0.0;
    }
    let starts = |w: &String| w.starts_with(letter);
    let purity = ws.iter().filter(|w| starts(w)).count() as f64 / ws.len() as f64;

    let mut distinct_good = std::collections::BTreeSet::new();
    for w in &ws {
        if starts(w) && w.chars().count() >= 3 {
            distinct_good.insert(w.clone());
        }
    }
    let coverage = (distinct_good.len().min(k) as f64) / (k as f64);
    (coverage * purity).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_tautogram_scores_one() {
        assert_eq!(verify('l', 4, "lazy lions leap loudly"), 1.0);
    }

    #[test]
    fn off_letter_word_cuts_purity() {
        // 3/4 words start with l, 3 distinct-good → 0.75 * 0.75
        let s = verify('l', 4, "lazy lions cats leap");
        assert!((s - 0.5625).abs() < 1e-9, "got {s}");
    }

    #[test]
    fn goodhart_shortcuts_earn_little() {
        assert_eq!(verify('l', 4, "l l l l"), 0.0); // too short → no coverage
        assert_eq!(verify('l', 4, "lll lll lll lll"), 0.25); // duplicates → coverage 1/4
        assert_eq!(verify('l', 4, ""), 0.0);
    }

    #[test]
    fn held_out_letters_are_disjoint_from_train() {
        let tr = train_letters();
        for c in test_letters() {
            assert!(!tr.contains(&c), "{c} leaks across the split");
        }
    }

    #[test]
    fn prompt_names_the_letter_and_count() {
        let p = task_prompt('b', 5);
        assert!(p.contains('B') && p.contains('5'));
    }
}

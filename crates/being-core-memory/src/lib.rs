//! Memory: episodic / semantic / procedural stores (build-spec §3.8, §4.4) — within-life learning.
//!
//! Research build: in-memory, naive substring retrieval. SQLite persistence and embedding retrieval
//! (the `nomic-embed-text` shared embedding) land at M2/M3; the store APIs and the no-launder
//! invariant are stable.
//!
//! **No-launder, made structural.** Episodic provenance is bound by *which method you call*
//! (`record_user_turn` ⇒ `DirectUserIntent`, `record_model_inference` ⇒ `ModelInference`, …). There
//! is no API that lets a caller attach `DirectUserIntent` to model- or tool-derived content, so
//! trust-escalating provenance cannot be forged — it is unrepresentable, not merely checked.

use std::collections::{BTreeMap, BTreeSet};

use being_core_types::ProvenanceClass;

/// Milliseconds since the Unix epoch (clock injected by the caller; the substrate has no clock).
pub type Ms = i64;
pub type SkillId = String;

// ---------------------------------------------------------------------------------------------
// Episodic — append-only, bitemporal (valid time + transaction time), provenance-tagged.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpisodicEntry {
    pub id: u64,
    pub valid_at_ms: Ms,
    pub txn_at_ms: Ms,
    pub provenance: ProvenanceClass,
    pub text: String,
}

#[derive(Default)]
pub struct EpisodicStore {
    entries: Vec<EpisodicEntry>,
    next_id: u64,
}

impl EpisodicStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn append(
        &mut self,
        provenance: ProvenanceClass,
        text: String,
        valid_at_ms: Ms,
        txn_at_ms: Ms,
    ) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(EpisodicEntry {
            id,
            valid_at_ms,
            txn_at_ms,
            provenance,
            text,
        });
        id
    }

    /// Record a genuine user turn — the only path that yields `DirectUserIntent`.
    pub fn record_user_turn(
        &mut self,
        text: impl Into<String>,
        valid_at_ms: Ms,
        txn_at_ms: Ms,
    ) -> u64 {
        self.append(
            ProvenanceClass::DirectUserIntent,
            text.into(),
            valid_at_ms,
            txn_at_ms,
        )
    }

    /// Record model output — always `ModelInference`; cannot escalate trust.
    pub fn record_model_inference(
        &mut self,
        text: impl Into<String>,
        valid_at_ms: Ms,
        txn_at_ms: Ms,
    ) -> u64 {
        self.append(
            ProvenanceClass::ModelInference,
            text.into(),
            valid_at_ms,
            txn_at_ms,
        )
    }

    /// Record a tool result — always `ToolOutput`.
    pub fn record_tool_output(
        &mut self,
        text: impl Into<String>,
        valid_at_ms: Ms,
        txn_at_ms: Ms,
    ) -> u64 {
        self.append(
            ProvenanceClass::ToolOutput,
            text.into(),
            valid_at_ms,
            txn_at_ms,
        )
    }

    pub fn all(&self) -> &[EpisodicEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Naive substring retrieval, most-recent-first. Embedding retrieval lands at M3.
    pub fn retrieve(&self, query: &str, limit: usize) -> Vec<&EpisodicEntry> {
        self.entries
            .iter()
            .rev()
            .filter(|e| e.text.contains(query))
            .take(limit)
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Semantic — consolidated knowledge. Written ONLY via consolidation, ALWAYS ModelInference.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticEntry {
    pub id: u64,
    pub fact: String,
}

impl SemanticEntry {
    /// Consolidated knowledge can never escalate trust.
    pub const PROVENANCE: ProvenanceClass = ProvenanceClass::ModelInference;
}

#[derive(Default)]
pub struct SemanticStore {
    entries: Vec<SemanticEntry>,
    next_id: u64,
}

impl SemanticStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Write a consolidated fact. The only write path; provenance is fixed at `ModelInference`.
    pub fn write_consolidated(&mut self, fact: impl Into<String>) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(SemanticEntry {
            id,
            fact: fact.into(),
        });
        id
    }

    pub fn all(&self) -> &[SemanticEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------------------------
// Consolidation — episodic → semantic (D-M3-1). The deterministic core promotes content that
// recurs; a model-based summarizing variant is foreground/feature-gated later. Consolidation exists
// to make retrieval cleaner, not to be the compounding win itself.
// ---------------------------------------------------------------------------------------------

/// Consolidates episodic entries into semantic facts.
pub trait Consolidator {
    fn consolidate(&self, episodic: &EpisodicStore, semantic: &mut SemanticStore);
}

fn normalize_text(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Deterministic: any normalized text seen >= `min_count` times across episodic entries is written
/// once as a semantic fact. Idempotent — facts already present are skipped.
pub struct FrequencyConsolidator {
    pub min_count: usize,
}

impl Consolidator for FrequencyConsolidator {
    fn consolidate(&self, episodic: &EpisodicStore, semantic: &mut SemanticStore) {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for e in episodic.all() {
            *counts.entry(normalize_text(&e.text)).or_insert(0) += 1;
        }
        let existing: BTreeSet<String> = semantic
            .all()
            .iter()
            .map(|s| normalize_text(&s.fact))
            .collect();
        for (text, n) in counts {
            if n >= self.min_count && !existing.contains(&text) {
                semantic.write_consolidated(text);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Procedural — installed skills, population-based: variants branch from any ancestor, and a
// revision never overwrites the ancestor it came from.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub id: SkillId,
    pub parent: Option<SkillId>,
    pub body: String,
}

/// A verifier-graded task trajectory: which task class, whether it passed, and the lesson learned.
/// In the loop the `passed` flag comes from the M2 bench (the verifier) — the decisive compounding
/// lever in D-M3-1.
pub struct Trajectory<'a> {
    pub task_class: &'a str,
    pub passed: bool,
    pub lesson: &'a str,
}

/// Parse the trailing `#vN` version from a skill id (0 if absent).
fn skill_version(id: &str) -> usize {
    id.rsplit("#v")
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

#[derive(Default)]
pub struct ProceduralStore {
    skills: Vec<Skill>,
}

impl ProceduralStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a skill. `parent = Some(ancestor)` records a branch; the ancestor is left intact.
    pub fn install(
        &mut self,
        id: impl Into<SkillId>,
        parent: Option<SkillId>,
        body: impl Into<String>,
    ) {
        self.skills.push(Skill {
            id: id.into(),
            parent,
            body: body.into(),
        });
    }

    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.id == id)
    }

    /// All skills that branched from `parent_id` (the population of variants).
    pub fn variants_of(&self, parent_id: &str) -> Vec<&Skill> {
        self.skills
            .iter()
            .filter(|s| s.parent.as_deref() == Some(parent_id))
            .collect()
    }

    pub fn all(&self) -> &[Skill] {
        &self.skills
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Learn from a verifier-graded trajectory (D-M3-1's decisive lever). A passing trajectory
    /// installs a new `[ok]` skill variant for `task_class` (branching from the prior best, so good
    /// skills are not overwritten); a failing one installs a `[fail]` cautionary variant. Returns
    /// the new variant's id.
    pub fn learn_from(&mut self, traj: &Trajectory<'_>) -> SkillId {
        let prefix = format!("{}#v", traj.task_class);
        let parent = self
            .skills
            .iter()
            .filter(|s| s.id.starts_with(&prefix))
            .max_by_key(|s| skill_version(&s.id))
            .map(|s| s.id.clone());
        let next = self
            .skills
            .iter()
            .filter(|s| s.id.starts_with(&prefix))
            .count()
            + 1;
        let id = format!("{prefix}{next}");
        let marker = if traj.passed { "[ok]" } else { "[fail]" };
        self.skills.push(Skill {
            id: id.clone(),
            parent,
            body: format!("{marker} {}", traj.lesson),
        });
        id
    }

    /// The latest passing skill for a task class — what retrieval surfaces on the next similar task.
    pub fn best_for(&self, task_class: &str) -> Option<&Skill> {
        let prefix = format!("{task_class}#v");
        self.skills
            .iter()
            .filter(|s| s.id.starts_with(&prefix) && s.body.starts_with("[ok]"))
            .max_by_key(|s| skill_version(&s.id))
    }
}

// ---------------------------------------------------------------------------------------------
// Semantic retrieval — the highest-ROI compounding lever (D-M3-1). Cosine over L2-normalized
// embeddings, blended with a recency prior so stale-but-similar entries don't crowd out fresh ones
// (LongMemEval: retrieval quality dominates). Pure + deterministic; the live `nomic-embed-text` that
// produces the vectors is wired separately (foreground/feature-gated), never in the automated loop.
// ---------------------------------------------------------------------------------------------

/// Produces an embedding vector for a piece of text. Implemented by a backend crate
/// (e.g. `being-embed-openai` over `nomic-embed-text`). The live call is foreground / feature-gated,
/// never run inside the automated loop (CLAUDE.md).
pub trait Embedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, String>;
}

/// Cosine similarity of two equal-length vectors. Returns 0.0 on length mismatch or a zero vector.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// Recency prior in (0,1]: `0.5^(age/half_life)`. Future/equal timestamps and a non-positive
/// half-life both give 1.0 (recency neutral).
fn recency_weight(now_ms: Ms, valid_at_ms: Ms, half_life_ms: i64) -> f32 {
    if half_life_ms <= 0 {
        return 1.0;
    }
    let age = (now_ms - valid_at_ms).max(0) as f64;
    0.5f64.powf(age / half_life_ms as f64) as f32
}

/// Whitespace tokens, lowercased, ASCII-punctuation trimmed from the ends — but symbols like `⊕`
/// (non-ASCII) survive, which is exactly what the lexical channel needs to catch.
fn lexical_tokens(s: &str) -> BTreeSet<String> {
    s.split_whitespace()
        .map(|t| {
            t.trim_matches(|c: char| c.is_ascii_punctuation())
                .to_lowercase()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// One embedded memory item.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorItem {
    pub id: u64,
    pub embedding: Vec<f32>,
    pub text: String,
    pub valid_at_ms: Ms,
}

/// A retrieval hit with its blended score.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub id: u64,
    pub text: String,
    pub score: f32,
}

/// A flat semantic index. `search` blends similarity with recency:
/// `score = alpha*cos + (1-alpha)*0.5^(age/half_life)` (D-M3-1; alpha≈0.7, half_life≈14d).
#[derive(Default)]
pub struct SemanticIndex {
    items: Vec<VectorItem>,
}

impl SemanticIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, id: u64, embedding: Vec<f32>, text: impl Into<String>, valid_at_ms: Ms) {
        self.items.push(VectorItem {
            id,
            embedding,
            text: text.into(),
            valid_at_ms,
        });
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Top-`k` hits by blended cosine + recency score, highest first.
    pub fn search(
        &self,
        query: &[f32],
        now_ms: Ms,
        k: usize,
        alpha: f32,
        half_life_ms: i64,
    ) -> Vec<Hit> {
        let mut scored: Vec<Hit> = self
            .items
            .iter()
            .map(|it| {
                let cos = cosine_similarity(query, &it.embedding);
                let recency = recency_weight(now_ms, it.valid_at_ms, half_life_ms);
                Hit {
                    id: it.id,
                    text: it.text.clone(),
                    score: alpha * cos + (1.0 - alpha) * recency,
                }
            })
            .collect();
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored.truncate(k);
        scored
    }

    /// Hybrid retrieval: blends the semantic+recency score with an IDF-weighted lexical match so
    /// rare/exact tokens (symbols like `⊕`, IDs, code names) retrieve reliably — embedding-only search
    /// misses these (D-M3-3 research). `lex_weight` in [0,1] mixes the lexical channel in; the lexical
    /// score is the rarest matched query token's IDF, normalized by the rarest indexable query token.
    #[allow(clippy::too_many_arguments)]
    pub fn search_hybrid(
        &self,
        query: &[f32],
        query_text: &str,
        now_ms: Ms,
        k: usize,
        alpha: f32,
        half_life_ms: i64,
        lex_weight: f32,
    ) -> Vec<Hit> {
        let q = lexical_tokens(query_text);
        let item_tokens: Vec<BTreeSet<String>> = self
            .items
            .iter()
            .map(|it| lexical_tokens(&it.text))
            .collect();
        let n = self.items.len().max(1) as f32;
        let idf = |tok: &str| -> f32 {
            let df = item_tokens.iter().filter(|s| s.contains(tok)).count();
            ((n + 1.0) / (df as f32 + 1.0)).ln()
        };
        // Normalizer: the rarest query token that appears anywhere in the index (so a single rare
        // exact match scores ~1.0). 1e-6 floor avoids div-by-zero when nothing matches.
        let norm = q
            .iter()
            .filter(|t| item_tokens.iter().any(|s| s.contains(*t)))
            .map(|t| idf(t))
            .fold(0.0f32, f32::max)
            .max(1e-6);
        let mut scored: Vec<Hit> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let cos = cosine_similarity(query, &it.embedding);
                let recency = recency_weight(now_ms, it.valid_at_ms, half_life_ms);
                let sem = alpha * cos + (1.0 - alpha) * recency;
                let lex = q
                    .iter()
                    .filter(|t| item_tokens[i].contains(*t))
                    .map(|t| idf(t))
                    .fold(0.0f32, f32::max)
                    / norm;
                Hit {
                    id: it.id,
                    text: it.text.clone(),
                    score: (1.0 - lex_weight) * sem + lex_weight * lex,
                }
            })
            .collect();
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        scored.truncate(k);
        scored
    }
}

#[cfg(test)]
mod vector_tests {
    use super::*;

    const EPS: f32 = 1e-6;

    struct StubEmbedder;
    impl Embedder for StubEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
            Ok(if text.contains("cat") {
                vec![1.0, 0.0]
            } else {
                vec![0.0, 1.0]
            })
        }
    }

    #[test]
    fn embedder_feeds_index() {
        let e = StubEmbedder;
        let mut idx = SemanticIndex::new();
        idx.add(1, e.embed("about cats").unwrap(), "about cats", 0);
        idx.add(2, e.embed("about dogs").unwrap(), "about dogs", 0);
        let q = e.embed("a cat question").unwrap();
        let hits = idx.search(&q, 0, 1, 1.0, 1000);
        assert_eq!(hits[0].id, 1);
    }

    #[test]
    fn hybrid_surfaces_rare_symbol_when_embeddings_miss() {
        let mut idx = SemanticIndex::new();
        idx.add(1, vec![0.0, 1.0], "Rule for ⊕: a ⊕ b = a*b + a + b", 0);
        idx.add(2, vec![1.0, 0.0], "unrelated note about the weather", 0);
        let q = vec![1.0, 0.0]; // cosine favors item 2
        assert_eq!(idx.search(&q, 0, 1, 1.0, 1000)[0].id, 2); // pure semantic misses the rule
        let hyb = idx.search_hybrid(&q, "what is 5 ⊕ 6?", 0, 1, 1.0, 1000, 0.7);
        assert_eq!(hyb[0].id, 1); // lexical ⊕ match pulls the rule to the top
    }

    #[test]
    fn cosine_basics() {
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < EPS);
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < EPS); // orthogonal
        assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).abs() < EPS); // length mismatch → 0
        assert!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]).abs() < EPS); // zero vector → 0
    }

    #[test]
    fn search_returns_most_similar_first() {
        let mut idx = SemanticIndex::new();
        idx.add(1, vec![1.0, 0.0], "a", 0);
        idx.add(2, vec![0.0, 1.0], "b", 0);
        idx.add(3, vec![0.9, 0.1], "c", 0);
        let hits = idx.search(&[1.0, 0.0], 0, 2, 1.0, 1000); // alpha=1 → pure cosine
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, 1);
        assert_eq!(hits[1].id, 3);
    }

    #[test]
    fn recency_breaks_ties_on_equal_similarity() {
        let mut idx = SemanticIndex::new();
        idx.add(1, vec![1.0, 0.0], "old", 0);
        idx.add(2, vec![1.0, 0.0], "new", 1000);
        let hits = idx.search(&[1.0, 0.0], 1000, 2, 0.5, 1000);
        assert_eq!(hits[0].id, 2); // equally similar, but fresher wins
    }

    #[test]
    fn fresh_moderate_beats_stale_exact() {
        // The stale-but-similar guard: a perfectly-similar but very old item must not crowd out a
        // fresh, only-moderately-similar one once the recency prior is in play.
        let mut idx = SemanticIndex::new();
        idx.add(1, vec![1.0, 0.0], "stale exact", 0); // cos 1.0, age 10 half-lives
        idx.add(2, vec![0.6, 0.8], "fresh", 10_000); // cos 0.6, age 0
        let hits = idx.search(&[1.0, 0.0], 10_000, 2, 0.7, 1000);
        assert_eq!(hits[0].id, 2);
    }
}

#[cfg(test)]
mod learning_tests {
    use super::*;

    #[test]
    fn consolidator_promotes_repeated_episodes_idempotently() {
        let mut ep = EpisodicStore::new();
        ep.record_model_inference("Paris is the capital of France", 1, 1);
        ep.record_model_inference("paris is the capital of france", 2, 2); // same after normalize
        ep.record_model_inference("an unrelated one-off", 3, 3);
        let mut sem = SemanticStore::new();
        FrequencyConsolidator { min_count: 2 }.consolidate(&ep, &mut sem);
        assert_eq!(sem.len(), 1);
        assert!(sem.all()[0].fact.contains("capital"));
        // re-running consolidation adds nothing (idempotent)
        FrequencyConsolidator { min_count: 2 }.consolidate(&ep, &mut sem);
        assert_eq!(sem.len(), 1);
    }

    #[test]
    fn skills_compound_on_passing_trajectories() {
        let mut p = ProceduralStore::new();
        let v1 = p.learn_from(&Trajectory {
            task_class: "sql",
            passed: true,
            lesson: "use indexes",
        });
        let v2 = p.learn_from(&Trajectory {
            task_class: "sql",
            passed: true,
            lesson: "use indexes and joins",
        });
        assert_ne!(v1, v2);
        assert_eq!(p.get(&v2).unwrap().parent.as_deref(), Some(v1.as_str())); // branches, ancestor kept
        assert!(p.get(&v1).is_some());
        assert_eq!(p.best_for("sql").unwrap().id, v2); // latest passing wins
    }

    #[test]
    fn failing_trajectory_does_not_become_best() {
        let mut p = ProceduralStore::new();
        p.learn_from(&Trajectory {
            task_class: "sql",
            passed: true,
            lesson: "good approach",
        });
        p.learn_from(&Trajectory {
            task_class: "sql",
            passed: false,
            lesson: "this failed",
        });
        let best = p.best_for("sql").unwrap();
        assert!(best.body.starts_with("[ok]"));
        assert!(best.body.contains("good approach"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episodic_provenance_is_bound_by_method_no_launder() {
        let mut ep = EpisodicStore::new();
        let u = ep.record_user_turn("hi", 1, 1);
        let m = ep.record_model_inference("a plan", 2, 2);
        let t = ep.record_tool_output("result", 3, 3);
        assert_eq!(
            ep.all()[(u - 1) as usize].provenance,
            ProvenanceClass::DirectUserIntent
        );
        assert_eq!(
            ep.all()[(m - 1) as usize].provenance,
            ProvenanceClass::ModelInference
        );
        assert_eq!(
            ep.all()[(t - 1) as usize].provenance,
            ProvenanceClass::ToolOutput
        );
        // Structural no-launder: exactly one entry can escalate trust — the user turn.
        let escalating = ep
            .all()
            .iter()
            .filter(|e| e.provenance.can_escalate_trust())
            .count();
        assert_eq!(escalating, 1);
    }

    #[test]
    fn episodic_is_bitemporal_and_retrieves_recent_first() {
        let mut ep = EpisodicStore::new();
        ep.record_user_turn("the cat sat", 10, 100);
        ep.record_model_inference("the cat ran", 20, 200);
        let hits = ep.retrieve("cat", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].text, "the cat ran"); // most recent first
        assert_eq!(hits[0].valid_at_ms, 20);
        assert_eq!(hits[0].txn_at_ms, 200);
    }

    #[test]
    fn semantic_is_always_model_inference() {
        assert_eq!(SemanticEntry::PROVENANCE, ProvenanceClass::ModelInference);
        assert!(!SemanticEntry::PROVENANCE.can_escalate_trust());
        let mut s = SemanticStore::new();
        s.write_consolidated("yogi prefers concise answers");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn procedural_variants_branch_without_overwriting_ancestor() {
        let mut p = ProceduralStore::new();
        p.install("greet", None, "say hi");
        p.install("greet.v2", Some("greet".into()), "say hello warmly");
        p.install("greet.v3", Some("greet".into()), "say hi briefly");
        assert!(p.get("greet").is_some()); // ancestor intact
        assert_eq!(p.variants_of("greet").len(), 2);
        assert_eq!(p.all().len(), 3);
    }
}

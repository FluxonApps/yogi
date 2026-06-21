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
// Procedural — installed skills, population-based: variants branch from any ancestor, and a
// revision never overwrites the ancestor it came from.
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub id: SkillId,
    pub parent: Option<SkillId>,
    pub body: String,
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

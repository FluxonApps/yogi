//! The journal: a single-writer-per-DID, hash-chained, signed append log (build-spec §3.3).
//!
//! Each entry commits to its predecessor via `prev_hash`, and the per-entry `entry_hash` (a blake3
//! digest over all fields) is signed by the being's root signer — so the whole chain is
//! tamper-evident and attributable. One DID owns exactly one chain head.
//!
//! Research build: in-memory store. Durability (fsync-before-return) and the SQLite-backed store
//! land with the per-step state machine (build-spec §5, Appendix A); the entry layout and
//! [`MemoryJournal::verify_chain`] semantics are stable.

use being_core_id::{verify, Signer};
use being_core_types::{Did, Hash, Sig};

pub type Seq = u64;

/// The genesis predecessor hash (all zeroes); `prev_hash` of the first entry (seq 1).
pub const GENESIS_PREV: Hash = Hash([0u8; 32]);

/// One signed, chained journal entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    pub seq: Seq,
    pub prev_hash: Hash,
    /// Event kind tag. Typed event variants (Commitment, Attestation, Settle, …) are layered in
    /// `being-runtime`; the journal itself stays event-agnostic.
    pub kind: String,
    pub payload: Vec<u8>,
    pub creator: Did,
    pub entry_hash: Hash,
    pub sig: Sig,
}

fn compute_hash(seq: Seq, prev: &Hash, kind: &str, payload: &[u8], creator: &Did) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(&seq.to_le_bytes());
    h.update(&prev.0);
    h.update(&(kind.len() as u64).to_le_bytes());
    h.update(kind.as_bytes());
    h.update(&(payload.len() as u64).to_le_bytes());
    h.update(payload);
    h.update(creator.0.as_bytes());
    Hash(*h.finalize().as_bytes())
}

/// An in-memory, single-writer journal bound to one signer (one DID, one chain head).
pub struct MemoryJournal<S: Signer> {
    signer: S,
    entries: Vec<JournalEntry>,
}

impl<S: Signer> MemoryJournal<S> {
    pub fn new(signer: S) -> Self {
        Self {
            signer,
            entries: Vec::new(),
        }
    }

    pub fn did(&self) -> &Did {
        self.signer.did()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `(last_seq, head_hash)`. Before any append: `(0, GENESIS_PREV)`; the first entry is seq 1.
    pub fn head(&self) -> (Seq, Hash) {
        match self.entries.last() {
            Some(e) => (e.seq, e.entry_hash.clone()),
            None => (0, GENESIS_PREV),
        }
    }

    /// Append a signed entry chained to the current head; returns its sequence number.
    pub fn append(&mut self, kind: &str, payload: Vec<u8>) -> Seq {
        let (last_seq, prev_hash) = self.head();
        let seq = last_seq + 1;
        let creator = self.signer.did().clone();
        let entry_hash = compute_hash(seq, &prev_hash, kind, &payload, &creator);
        let sig = self.signer.sign(&entry_hash.0);
        self.entries.push(JournalEntry {
            seq,
            prev_hash,
            kind: kind.to_string(),
            payload,
            creator,
            entry_hash,
            sig,
        });
        seq
    }

    /// Replay entries in order (the basis for crash recovery; build-spec §5).
    pub fn replay(&self) -> impl Iterator<Item = &JournalEntry> {
        self.entries.iter()
    }

    pub fn get(&self, seq: Seq) -> Option<&JournalEntry> {
        seq.checked_sub(1)
            .and_then(|i| self.entries.get(i as usize))
    }

    /// Verify the entire chain: contiguous sequence, `prev_hash` linkage, recomputed `entry_hash`,
    /// and a valid signature by `creator` over each `entry_hash`. Returns false on any break.
    pub fn verify_chain(&self) -> bool {
        let mut prev = GENESIS_PREV;
        for (i, e) in self.entries.iter().enumerate() {
            let expected_seq = i as u64 + 1;
            if e.seq != expected_seq || e.prev_hash != prev {
                return false;
            }
            if compute_hash(e.seq, &e.prev_hash, &e.kind, &e.payload, &e.creator) != e.entry_hash {
                return false;
            }
            if !verify(&e.creator, &e.entry_hash.0, &e.sig) {
                return false;
            }
            prev = e.entry_hash.clone();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use being_core_id::Ed25519Signer;

    fn journal() -> MemoryJournal<Ed25519Signer> {
        MemoryJournal::new(Ed25519Signer::from_seed([42u8; 32]))
    }

    #[test]
    fn appends_increment_seq_and_advance_head() {
        let mut j = journal();
        assert_eq!(j.head().0, 0);
        assert_eq!(j.append("commitment", b"a".to_vec()), 1);
        assert_eq!(j.append("attestation", b"b".to_vec()), 2);
        assert_eq!(j.head().0, 2);
        assert_eq!(j.len(), 2);
    }

    #[test]
    fn entries_chain_prev_to_predecessor_hash() {
        let mut j = journal();
        j.append("x", b"1".to_vec());
        j.append("y", b"2".to_vec());
        let e1 = j.get(1).unwrap();
        let e2 = j.get(2).unwrap();
        assert_eq!(e1.prev_hash, GENESIS_PREV);
        assert_eq!(e2.prev_hash, e1.entry_hash);
    }

    #[test]
    fn fresh_chain_verifies() {
        let mut j = journal();
        for k in 0..5 {
            j.append("step", vec![k]);
        }
        assert!(j.verify_chain());
    }

    #[test]
    fn tampered_payload_breaks_verification() {
        let mut j = journal();
        j.append("step", b"genuine".to_vec());
        j.append("step", b"also genuine".to_vec());
        // Tamper the stored payload (private field, accessible from this child module).
        j.entries[0].payload = b"forged".to_vec();
        assert!(!j.verify_chain());
    }

    #[test]
    fn reordered_chain_breaks_verification() {
        let mut j = journal();
        j.append("a", b"1".to_vec());
        j.append("b", b"2".to_vec());
        j.entries.swap(0, 1);
        assert!(!j.verify_chain());
    }

    #[test]
    fn replay_yields_entries_in_order() {
        let mut j = journal();
        j.append("a", b"1".to_vec());
        j.append("b", b"2".to_vec());
        let kinds: Vec<&str> = j.replay().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, vec!["a", "b"]);
    }

    #[test]
    fn one_did_one_head() {
        let j = journal();
        assert!(j.did().0.starts_with("did:key:"));
    }

    #[test]
    fn tampered_signature_alone_breaks_verification() {
        // Corrupt ONLY the signature, leaving payload/seq/prev_hash/entry_hash intact. The sequence,
        // linkage, and hash-recompute checks all pass, so this isolates the signature-verification
        // branch of verify_chain (which the payload-tamper test never reaches).
        let mut j = journal();
        j.append("step", b"genuine".to_vec());
        j.append("step", b"more".to_vec());
        assert!(j.verify_chain());
        j.entries[1].sig = being_core_types::Sig(vec![0u8; 64]); // valid length, wrong signature
        assert!(!j.verify_chain());
    }

    #[test]
    fn empty_chain_verifies_vacuously() {
        let j = journal();
        assert_eq!(j.len(), 0);
        assert!(j.verify_chain());
        assert_eq!(j.head(), (0, GENESIS_PREV));
    }
}

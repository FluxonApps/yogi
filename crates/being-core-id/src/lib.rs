//! Identity: DID + Ed25519 signing / verification (build-spec §3.2).
//!
//! Research build: the DID is `did:key:hex:<pubkey-hex>` and keys are derived deterministically
//! from a 32-byte seed. Full `did:key` multibase/multicodec encoding and OS-keystore storage land
//! later; the [`Signer`] trait and the [`verify`] semantics are stable.

use std::fmt::Write as _;

use being_core_types::{Did, Sig};
use ed25519_dalek::{
    Signature, Signer as DalekSigner, SigningKey, Verifier as DalekVerifier, VerifyingKey,
};

const DID_PREFIX: &str = "did:key:hex:";

/// A signing identity. The being's root signer — one DID, one journal chain head
/// (see `being-core-journal`).
pub trait Signer {
    fn did(&self) -> &Did;
    fn sign(&self, bytes: &[u8]) -> Sig;
}

/// Ed25519 signer, derived deterministically from a 32-byte seed.
pub struct Ed25519Signer {
    signing_key: SigningKey,
    did: Did,
}

impl Ed25519Signer {
    /// Construct from a 32-byte seed. Deterministic: the same seed always yields the same DID.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let did = did_from_pubkey(&signing_key.verifying_key().to_bytes());
        Self { signing_key, did }
    }

    /// Raw Ed25519 public-key bytes.
    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

impl Signer for Ed25519Signer {
    fn did(&self) -> &Did {
        &self.did
    }

    fn sign(&self, bytes: &[u8]) -> Sig {
        Sig(self.signing_key.sign(bytes).to_bytes().to_vec())
    }
}

/// Verify a signature against the public key embedded in `did`. Returns `false` on any malformation
/// (bad DID, wrong signature length, invalid key) rather than panicking.
pub fn verify(did: &Did, bytes: &[u8], sig: &Sig) -> bool {
    let Some(verifying_key) = verifying_key_from_did(did) else {
        return false;
    };
    let sig_bytes: [u8; 64] = match sig.0.as_slice().try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };
    verifying_key
        .verify(bytes, &Signature::from_bytes(&sig_bytes))
        .is_ok()
}

fn did_from_pubkey(pk: &[u8; 32]) -> Did {
    Did(format!("{DID_PREFIX}{}", to_hex(pk)))
}

fn verifying_key_from_did(did: &Did) -> Option<VerifyingKey> {
    let hex = did.0.strip_prefix(DID_PREFIX)?;
    let arr: [u8; 32] = from_hex(hex)?.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(n: u8) -> [u8; 32] {
        [n; 32]
    }

    #[test]
    fn sign_then_verify_roundtrips() {
        let s = Ed25519Signer::from_seed(seed(7));
        let msg = b"yogi attests this";
        let sig = s.sign(msg);
        assert!(verify(s.did(), msg, &sig));
    }

    #[test]
    fn tampered_message_fails() {
        let s = Ed25519Signer::from_seed(seed(7));
        let sig = s.sign(b"original");
        assert!(!verify(s.did(), b"tampered", &sig));
    }

    #[test]
    fn wrong_identity_fails() {
        let a = Ed25519Signer::from_seed(seed(1));
        let b = Ed25519Signer::from_seed(seed(2));
        let sig = a.sign(b"msg");
        assert!(!verify(b.did(), b"msg", &sig));
    }

    #[test]
    fn did_is_deterministic_and_well_formed() {
        let a = Ed25519Signer::from_seed(seed(9));
        let b = Ed25519Signer::from_seed(seed(9));
        assert_eq!(a.did(), b.did());
        assert!(a.did().0.starts_with(DID_PREFIX));
    }

    #[test]
    fn malformed_signature_is_rejected_not_panicked() {
        let s = Ed25519Signer::from_seed(seed(3));
        assert!(!verify(s.did(), b"x", &Sig(vec![0u8; 10])));
    }
}

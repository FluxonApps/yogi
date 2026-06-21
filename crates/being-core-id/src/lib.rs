//! Identity: DID + Ed25519 signing / verification (build-spec §3.2).
//!
//! The DID is a standard W3C **`did:key`** for Ed25519 — `did:key:z<base58btc(0xed01 ++ pubkey)>`
//! (multicodec `ed25519-pub` varint `0xED 0x01` + multibase base58btc, prefix `z`) — derived
//! deterministically from a 32-byte seed. OS-keystore storage lands later; the [`Signer`] trait and
//! [`verify`] semantics are stable.

use being_core_types::{Did, Sig};
use ed25519_dalek::{
    Signature, Signer as DalekSigner, SigningKey, Verifier as DalekVerifier, VerifyingKey,
};

/// `did:key` prefix for a base58btc-multibase value (the `z`).
const DID_KEY_PREFIX: &str = "did:key:z";
/// Multicodec varint for `ed25519-pub` (0xED), little-endian varint `[0xED, 0x01]`.
const MULTICODEC_ED25519_PUB: [u8; 2] = [0xED, 0x01];

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
    let mut multicodec = Vec::with_capacity(2 + pk.len());
    multicodec.extend_from_slice(&MULTICODEC_ED25519_PUB);
    multicodec.extend_from_slice(pk);
    Did(format!("{DID_KEY_PREFIX}{}", base58btc_encode(&multicodec)))
}

fn verifying_key_from_did(did: &Did) -> Option<VerifyingKey> {
    let mb = did.0.strip_prefix(DID_KEY_PREFIX)?;
    let bytes = base58btc_decode(mb)?;
    // Expect the ed25519-pub multicodec prefix followed by exactly 32 key bytes.
    let key = bytes.strip_prefix(&MULTICODEC_ED25519_PUB[..])?;
    let arr: [u8; 32] = key.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Base58btc (Bitcoin alphabet) encode — the multibase `z` value. Pure, no external deps.
fn base58btc_encode(input: &[u8]) -> String {
    let mut digits: Vec<u8> = Vec::new(); // base58 digits, little-endian
    for &byte in input {
        let mut carry = byte as u32;
        for d in digits.iter_mut() {
            carry += (*d as u32) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::new();
    for &b in input {
        if b == 0 {
            out.push('1'); // leading zero bytes → leading '1'
        } else {
            break;
        }
    }
    for &d in digits.iter().rev() {
        out.push(BASE58_ALPHABET[d as usize] as char);
    }
    out
}

/// Base58btc decode. Returns `None` on any character outside the alphabet.
fn base58btc_decode(s: &str) -> Option<Vec<u8>> {
    let mut bytes: Vec<u8> = Vec::new(); // base256, little-endian
    for ch in s.bytes() {
        let val = BASE58_ALPHABET.iter().position(|&a| a == ch)? as u32;
        let mut carry = val;
        for b in bytes.iter_mut() {
            carry += (*b as u32) * 58;
            *b = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let mut out: Vec<u8> = Vec::new();
    for ch in s.bytes() {
        if ch == b'1' {
            out.push(0); // leading '1' → leading zero byte
        } else {
            break;
        }
    }
    out.extend(bytes.iter().rev());
    Some(out)
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
        // Real W3C did:key for ed25519: did:key:z + base58btc(0xED01 ++ pubkey). The 0xED01 prefix
        // makes every ed25519 did:key begin with the canonical "did:key:z6Mk" — a standards check.
        assert!(
            a.did().0.starts_with("did:key:z6Mk"),
            "not a canonical ed25519 did:key: {}",
            a.did().0
        );
        // The encoded value round-trips back to the signer's public key (so verify works).
        let vk = verifying_key_from_did(a.did()).expect("did:key decodes");
        assert_eq!(vk.to_bytes(), a.verifying_key_bytes());
    }

    #[test]
    fn base58btc_roundtrips_including_leading_zeros() {
        for case in [
            vec![],
            vec![0u8],
            vec![0, 0, 1, 2, 3],
            vec![255, 254, 0, 7],
            (0u8..34).collect::<Vec<_>>(),
        ] {
            assert_eq!(base58btc_decode(&base58btc_encode(&case)).unwrap(), case);
        }
        // Reject a non-alphabet character (0 and O are not in the base58 alphabet).
        assert!(base58btc_decode("0OIl").is_none());
    }

    #[test]
    fn malformed_signature_is_rejected_not_panicked() {
        let s = Ed25519Signer::from_seed(seed(3));
        assert!(!verify(s.did(), b"x", &Sig(vec![0u8; 10])));
    }
}

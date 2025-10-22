//! primitives-zk-elgamal
//!
//! Shared, `no_std`-friendly primitives for zk-ElGamal style confidential transfers.
//! - Stable byte layouts (points/scalars) and ciphertext types
//! - Canonical Merlin transcript labels + bind order for domain separation
//! - Minimal helpers (no heavy crypto) so both prover and verifier stay in lockstep
//!
//! Use this crate from BOTH:
//! - prover library (std OK) and
//! - on-chain verifier pallet (`no_std`)
//!
//! Make sure both sides use the SAME Pedersen params (G, H) and the SAME transcript labels.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use core::fmt;

use curve25519_dalek::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use merlin::Transcript;
use subtle::ConstantTimeEq;

/// 32-byte compressed Ristretto encoding.
pub type CompressedPoint = [u8; 32];

/// 32-byte scalar encoding (canonical preferred).
pub type ScalarBytes = [u8; 32];

/// Version/tag this protocol instance. Bump on any incompatible change.
pub const SDK_VERSION: u32 = 1;

/// Domain / label strings — KEEP STABLE.
pub mod labels {
    pub const PROTOCOL: &[u8] = b"zk-elgamal-conf-xfer";
    pub const PROTOCOL_V: &[u8] = b"zk-elgamal-conf-xfer/v1";

    // transcript sections
    pub const SECTION_CVP: &[u8] = b"cvp"; // ciphertext validity
    pub const SECTION_EQ: &[u8] = b"eq"; // equality
    pub const SECTION_RP: &[u8] = b"range"; // range proof

    // challenge labels
    pub const CHAL_CVP: &[u8] = b"cvp_chal";
    pub const CHAL_EQ: &[u8] = b"eq_chal";
}

/// Minimal Pedersen parameter bag. You decide how to source these (deterministic hash-to-point, fixed constants, etc.).
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
pub struct PedersenParams {
    pub G: RistrettoPoint,
    pub H: RistrettoPoint,
}

/// ElGamal public key for decrypt handle relation (D = r * pk).
#[derive(Clone, Copy)]
pub struct ElGamalPk {
    pub pk: RistrettoPoint,
}

/// Grouped ElGamal ciphertext: commitment part C and decrypt-handle D.
/// Layout matches common SDK practice: both compressed to 32 bytes when serialized.
#[derive(Clone, Copy)]
#[allow(non_snake_case)]
pub struct Ciphertext {
    pub C: RistrettoPoint,
    pub D: RistrettoPoint,
}

impl Ciphertext {
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut out = [0u8; 64];
        out[0..32].copy_from_slice(self.C.compress().as_bytes());
        out[32..64].copy_from_slice(self.D.compress().as_bytes());
        out
    }

    #[allow(non_snake_case)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != 64 {
            return Err(Error::Malformed);
        }
        let mut c = [0u8; 32];
        let mut d = [0u8; 32];
        c.copy_from_slice(&bytes[0..32]);
        d.copy_from_slice(&bytes[32..64]);
        let C = CompressedRistretto(c)
            .decompress()
            .ok_or(Error::Malformed)?;
        let D = CompressedRistretto(d)
            .decompress()
            .ok_or(Error::Malformed)?;
        Ok(Self { C, D })
    }
}

/// Public context that BOTH sides bind into the transcript before any challenges.
/// Keep fields/ordering stable; modify only behind a version bump.
#[derive(Clone)]
pub struct PublicContext {
    /// Chain/domain tag (e.g., blake2(genesis_hash || pallet_name)), 32 bytes recommended.
    pub network_id: [u8; 32],
    /// Protocol version (should equal SDK_VERSION on both sides).
    pub sdk_version: u32,

    /// Asset namespace (choose your encoding; 32 bytes is future proof).
    pub asset_id: [u8; 32],

    pub sender_pk: RistrettoPoint,
    pub receiver_pk: RistrettoPoint,
    pub auditor_pk: Option<RistrettoPoint>,

    /// Fee commitment (C_fee) if you model fees as commitments.
    pub fee_commitment: RistrettoPoint,

    /// Output ciphertext (must be bound).
    pub ciphertext_out: Ciphertext,

    /// Optional input ciphertext (bind if present; else bind the "absent" marker).
    pub ciphertext_in: Option<Ciphertext>,
}

impl PublicContext {
    /// Bind this context into a Merlin transcript in a stable, canonical order.
    pub fn bind_to_transcript(&self, t: &mut Transcript) {
        use crate::labels::*;
        t.append_message(b"proto", PROTOCOL_V);
        t.append_message(b"sdk_version", &self.sdk_version.to_le_bytes());
        t.append_message(b"network_id", &self.network_id);
        t.append_message(b"asset_id", &self.asset_id);

        append_point(t, b"sender_pk", &self.sender_pk);
        append_point(t, b"receiver_pk", &self.receiver_pk);
        match self.auditor_pk {
            Some(pk) => append_point(t, b"auditor_pk", &pk),
            None => t.append_message(b"auditor_pk", b"none"),
        }

        append_point(t, b"fee_C", &self.fee_commitment);
        append_point(t, b"out_C", &self.ciphertext_out.C);
        append_point(t, b"out_D", &self.ciphertext_out.D);

        if let Some(cin) = &self.ciphertext_in {
            append_point(t, b"in_C", &cin.C);
            append_point(t, b"in_D", &cin.D);
        } else {
            t.append_message(b"in_ciphertext", b"absent");
        }
    }
}

/// Error type shared by prover/verifier helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Malformed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed => write!(f, "malformed input"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// ----- Merlin transcript helpers -----

/// Start a transcript already seeded with the protocol label and bound public context.
pub fn new_transcript(ctx: &PublicContext) -> Transcript {
    let mut t = Transcript::new(labels::PROTOCOL);
    ctx.bind_to_transcript(&mut t);
    t
}

/// Append a compressed Ristretto point under a label.
pub fn append_point(t: &mut Transcript, label: &'static [u8], p: &RistrettoPoint) {
    t.append_message(label, p.compress().as_bytes());
}

/// Derive a Fiat–Shamir challenge scalar from the transcript with a label.
pub fn challenge_scalar(t: &mut Transcript, label: &'static [u8]) -> Scalar {
    let mut buf = [0u8; 64];
    t.challenge_bytes(label, &mut buf);
    Scalar::from_bytes_mod_order_wide(&buf)
}

/// Constant-time equality on compressed points (for assertions).
pub fn ct_eq_point(a: &RistrettoPoint, b: &RistrettoPoint) -> bool {
    a.compress()
        .as_bytes()
        .ct_eq(b.compress().as_bytes())
        .into()
}

/// ----- Encoding helpers (stable across prover/verifier) -----

/// Decode a compressed Ristretto point (32 bytes).
pub fn point_from_bytes(bytes: &CompressedPoint) -> Result<RistrettoPoint, Error> {
    CompressedRistretto(*bytes)
        .decompress()
        .ok_or(Error::Malformed)
}

/// Encode a point to 32 bytes (compressed).
pub fn point_to_bytes(p: &RistrettoPoint) -> CompressedPoint {
    *p.compress().as_bytes()
}

/// Decode a scalar from canonical 32-byte encoding.
/// If you *must* accept non-canonical, explicitly swap to `from_bytes_mod_order`.
pub fn scalar_from_canonical(bytes: &ScalarBytes) -> Result<Scalar, Error> {
    let maybe_scalar = Scalar::from_canonical_bytes(*bytes);
    if maybe_scalar.is_some().into() {
        Ok(maybe_scalar.unwrap())   
    } else {
        Err(Error::Malformed)
    }
}

/// Encode scalar to 32 bytes (canonical).
pub fn scalar_to_bytes(x: &Scalar) -> ScalarBytes {
    x.to_bytes()
}

/// Concatenate two compressed points (e.g., for fixed-size proof parts).
pub fn concat_points(a: &RistrettoPoint, b: &RistrettoPoint) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[0..32].copy_from_slice(a.compress().as_bytes());
    out[32..64].copy_from_slice(b.compress().as_bytes());
    out
}

/// ----- Proof byte “contracts” (optional but recommended) -----
///
/// Define fixed encodings so both sides agree on sizes and order. These match
/// the Σ-proof skeletons discussed earlier. You can use these directly or
/// treat them as documentation for your own types.

/// CVP (ciphertext validity) proof bytes:
/// R1(32) || R2(32) || s_v(32) || s_r(32)  => total 128 bytes.
pub const CVP_PROOF_LEN: usize = 128;

/// EQ (equality) proof bytes:
/// R(32) || s_v(32) || s_r(32) => total 96 bytes.
pub const EQ_PROOF_LEN: usize = 96;

/// Simple wrapper enforcing correct lengths at construction.
pub struct FixedProof<const N: usize> {
    inner: [u8; N],
}

impl<const N: usize> FixedProof<N> {
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != N {
            return Err(Error::Malformed);
        }
        let mut inner = [0u8; N];
        inner.copy_from_slice(bytes);
        Ok(Self { inner })
    }
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.inner
    }
    pub fn into_bytes(self) -> [u8; N] {
        self.inner
    }
}

/// Convenience aliases
pub type CvpProofBytes = FixedProof<{ CVP_PROOF_LEN }>;
pub type EqProofBytes = FixedProof<{ EQ_PROOF_LEN }>;

/// ----- Feature-gated serde for off-chain code -----
#[cfg(feature = "std")]
mod serde_impls {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    impl Serialize for Ciphertext {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(&self.to_bytes())
        }
    }
    impl<'de> Deserialize<'de> for Ciphertext {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            let bytes: alloc::borrow::Cow<'de, [u8]> = serde::Deserialize::deserialize(d)?;
            Ciphertext::from_bytes(&bytes).map_err(serde::de::Error::custom)
        }
    }
}

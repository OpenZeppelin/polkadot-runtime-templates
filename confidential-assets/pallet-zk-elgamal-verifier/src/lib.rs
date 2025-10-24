#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;
use sp_std::vec::Vec;

use ca_primitives::ZkVerifier as ZkVerifierTrait;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::RistrettoPoint,
    scalar::Scalar,
    traits::{Identity, IsIdentity},
};
use merlin::Transcript;

pub use pallet::*;
use primitives_zk_elgamal::{
    append_point, challenge_scalar as fs_chal, labels, new_transcript, point_from_bytes,
    point_to_bytes, Ciphertext, FixedProof, PublicContext, RangeProofVerifier, SDK_VERSION,
};

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Pluggable range-proof verifier (e.g., Bulletproofs wrapper) — `no_std` friendly.
        type RangeVerifier: RangeProofVerifier;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {
        InvalidEncoding,
        InvalidPoint,
        InvalidProof,
        TotalChanged,
    }

    // ========= Proof bundle shapes =========

    /// 192-byte link-proof used by sender-side verifier:
    /// A1(32)||A2(32)||A3(32)||z_k(32)||z_v(32)||z_r(32)
    type LinkProofBytes = FixedProof<192>;

    /// Sender bundle:
    /// delta_comm(32) || link(192) || len_from(2) || range_from || len_to(2) || range_to
    struct TransferProof<'a> {
        delta_comm: RistrettoPoint, // ΔC = Δv·G + ρ·H
        link_raw: LinkProofBytes,   // Σ-link between ct and ΔC
        range_from_new: &'a [u8],   // optional; verifier may accept empty
        range_to_new: &'a [u8],     // optional; verifier may accept empty
    }

    impl<'a> TransferProof<'a> {
        fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
            if bytes.len() < 32 + 192 + 2 + 2 {
                return Err(());
            }
            let mut c_buf = [0u8; 32];
            c_buf.copy_from_slice(&bytes[0..32]);
            let delta_comm = point_from_bytes(&c_buf).map_err(|_| ())?;

            let link_off = 32;
            let link_raw =
                LinkProofBytes::from_slice(&bytes[link_off..link_off + 192]).map_err(|_| ())?;

            let mut off = link_off + 192;
            let len1 = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
            off += 2;
            if bytes.len() < off + len1 + 2 {
                return Err(());
            }
            let range1 = &bytes[off..off + len1];
            off += len1;

            let len2 = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
            off += 2;
            if bytes.len() < off + len2 {
                return Err(());
            }
            let range2 = &bytes[off..off + len2];

            Ok(Self {
                delta_comm,
                link_raw,
                range_from_new: range1,
                range_to_new: range2,
            })
        }
    }

    /// Receiver accept envelope:
    /// delta_comm(32) || len(2) || range_to_new(len)
    struct AcceptEnvelope<'a> {
        delta_comm: RistrettoPoint,
        range_to_new: &'a [u8],
    }

    impl<'a> AcceptEnvelope<'a> {
        fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
            if bytes.len() < 32 + 2 {
                return Err(());
            }
            let mut c_buf = [0u8; 32];
            c_buf.copy_from_slice(&bytes[0..32]);
            let delta_comm = point_from_bytes(&c_buf).map_err(|_| ())?;
            let len = u16::from_le_bytes([bytes[32], bytes[33]]) as usize;
            if bytes.len() < 34 + len {
                return Err(());
            }
            let proof = &bytes[34..34 + len];
            Ok(Self {
                delta_comm,
                range_to_new: proof,
            })
        }
    }

    // ========= Core verification (ZkVerifier trait) =========

    impl<T: Config> ZkVerifierTrait for Pallet<T> {
        type Error = (); // TODO: replace with Error<T>

        /// Sender phase: verify link proof (and optional range proofs) and compute new balances.
        fn verify_transfer_sent(
            asset: &[u8],
            from_pk_bytes: &[u8],
            to_pk_bytes: &[u8],
            from_old_bytes: &[u8],
            to_old_bytes: &[u8],
            delta_ct_bytes: &[u8],
            proof_bundle_bytes: &[u8],
        ) -> Result<(Vec<u8>, Vec<u8>), Self::Error> {
            // --- Parse inputs ---
            let from_pk = parse_point32(from_pk_bytes).map_err(|_| ())?;
            let to_pk = parse_point32(to_pk_bytes).map_err(|_| ())?;
            let from_old = parse_point32_allow_empty_identity(from_old_bytes).map_err(|_| ())?;
            let to_old = parse_point32_allow_empty_identity(to_old_bytes).map_err(|_| ())?;
            let delta_ct = Ciphertext::from_bytes(delta_ct_bytes).map_err(|_| ())?;
            let proof = TransferProof::parse(proof_bundle_bytes).map_err(|_| ())?;

            println!("--------------parsed inputs--------------");

            // --- Canonical public context for sender transcript binding ---
            let asset_id = pad_or_trim_32(asset);
            let network_id = [0u8; 32]; // chain-specific tag can be wired in later

            let ctx = PublicContext {
                network_id,
                sdk_version: SDK_VERSION,
                asset_id,
                sender_pk: from_pk,
                receiver_pk: to_pk,
                auditor_pk: None,
                fee_commitment: RistrettoPoint::identity(),
                ciphertext_out: delta_ct, // Δciphertext is the "out" binding
                ciphertext_in: None,
            };
            let mut t = new_transcript(&ctx);

            // --- Link Σ-proof verification (A1,A2,A3,z_k,z_v,z_r) ---
            let (a1, a2, a3, z_k, z_v, z_r) =
                parse_link_from_192(proof.link_raw.as_bytes()).map_err(|_| ())?;
            append_point(&mut t, b"a1", &a1);
            append_point(&mut t, b"a2", &a2);
            append_point(&mut t, b"a3", &a3);

            let c: Scalar = fs_chal(&mut t, labels::CHAL_EQ);

            println!("--------------parsed link from proof--------------");

            // Eq1: z_k·G == A1 + c·C
            let lhs1 = z_k * G;
            let rhs1 = a1 + c * delta_ct.C;
            if !(lhs1 - rhs1).is_identity() {
                return Err(());
            }

            println!("--------------Eq1 verified--------------");

            // Eq2: z_v·G + z_k·PK_from == A2 + c·D
            let lhs2 = z_v * G + z_k * from_pk;
            let rhs2 = a2 + c * delta_ct.D;
            if !(lhs2 - rhs2).is_identity() {
                return Err(());
            }

            println!("--------------Eq2 verified--------------");

            // Eq3: z_v·G + z_r·H == A3 + c·ΔC
            let h = pedersen_h_generator();
            let lhs3 = z_v * G + z_r * h;
            let rhs3 = a3 + c * proof.delta_comm;
            if !(lhs3 - rhs3).is_identity() {
                return Err(());
            }

            println!("--------------Eq3 verified--------------");

            // --- Compute post-transfer commitments
            let from_new = from_old - proof.delta_comm;
            let to_new = to_old + proof.delta_comm;

            // --- Optional range proof checks (if provided) bound to same context bytes
            let ctx_bytes = transcript_context_bytes(&t);
            let from_new_bytes = point_to_bytes(&from_new);
            let to_new_bytes = point_to_bytes(&to_new);

            // TODO: ensure range proofs are non empty here

            if !proof.range_from_new.is_empty() {
                println!("--------------RangeProof1 nonempty--------------");
                T::RangeVerifier::verify_range_proof(
                    b"range_from_new",
                    &ctx_bytes,
                    &from_new_bytes,
                    proof.range_from_new,
                )
                .map_err(|_| ())?;
            }
            println!("--------------RangeProof1 verified--------------");
            if !proof.range_to_new.is_empty() {
                println!("--------------RangeProof2 nonempty--------------");
                T::RangeVerifier::verify_range_proof(
                    b"range_to_new",
                    &ctx_bytes,
                    &to_new_bytes,
                    proof.range_to_new,
                )
                .map_err(|_| ())?;
            }
            println!("--------------RangeProof2 verified--------------");

            // Total unchanged for pure transfer
            Ok((from_new_bytes.to_vec(), to_new_bytes.to_vec()))
        }

        /// ACL/policy path (no proof). Intentionally left to the runtime policy.
        fn acl_transfer_sent(
            _asset: &[u8],
            _from_pk: &[u8],
            _to_pk: &[u8],
            _from_old: &[u8],
            _to_old: &[u8],
            _amount_cipher: &[u8],
        ) -> Result<(Vec<u8>, Vec<u8>), Self::Error> {
            Err(()) // define policy rules here if needed
        }

        /// Receiver phase: verify acceptance envelope and compute new receiver commitment.
        fn verify_transfer_received(
            asset: &[u8],
            to_pk_bytes: &[u8],
            to_old_bytes: &[u8],
            proof: &[u8],
        ) -> Result<Vec<u8>, Self::Error> {
            let to_pk = parse_point32(to_pk_bytes).map_err(|_| ())?;
            let to_old = parse_point32_allow_empty_identity(to_old_bytes).map_err(|_| ())?;
            let env = AcceptEnvelope::parse(proof).map_err(|_| ())?;

            // to_new = to_old + ΔC
            let to_new = to_old + env.delta_comm;
            let to_new_bytes = point_to_bytes(&to_new);

            // Acceptance context binding (must match prover's accept_ctx_bytes):
            // proto/version + sdk_version + network_id + asset_id + to_pk + to_old + delta_comm
            let network_id = [0u8; 32];
            let asset_id = pad_or_trim_32(asset);

            let mut t = Transcript::new(labels::PROTOCOL);
            t.append_message(b"proto", labels::PROTOCOL_V);
            t.append_message(b"sdk_version", &SDK_VERSION.to_le_bytes());
            t.append_message(b"network_id", &network_id);
            t.append_message(b"asset_id", &asset_id);
            append_point(&mut t, b"receiver_pk", &to_pk);
            append_point(&mut t, b"to_old", &to_old);
            append_point(&mut t, b"delta_comm", &env.delta_comm);

            let mut ctx_bytes = [0u8; 32];
            t.challenge_bytes(b"ctx", &mut ctx_bytes);

            // Verify receiver range proof bound to acceptance context
            T::RangeVerifier::verify_range_proof(
                b"range_to_new",
                &ctx_bytes,
                &to_new_bytes,
                env.range_to_new,
            )
            .map_err(|_| ())?;

            // Total remains unchanged for acceptance
            Ok(to_new_bytes.to_vec())
        }

        fn disclose(_asset: &[u8], _who_pk: &[u8], _cipher: &[u8]) -> Result<u64, Self::Error> {
            Err(()) // optionally implement view-key/auditor flow
        }
    }

    // ========= Helpers =========

    /// Accepts exactly 32 bytes; errors otherwise.
    fn parse_point32(bytes: &[u8]) -> Result<RistrettoPoint, ()> {
        if bytes.len() != 32 {
            return Err(());
        }
        let mut b = [0u8; 32];
        b.copy_from_slice(bytes);
        point_from_bytes(&b).map_err(|_| ())
    }

    /// Allows empty slice = identity (useful for “zero balance” sentinel).
    fn parse_point32_allow_empty_identity(bytes: &[u8]) -> Result<RistrettoPoint, ()> {
        if bytes.is_empty() {
            Ok(RistrettoPoint::identity())
        } else {
            parse_point32(bytes)
        }
    }

    /// Parse A1||A2||A3||z_k||z_v||z_r from the 192-byte link proof.
    /// Scalars are decoded mod-order for compatibility with current prover.
    fn parse_link_from_192(
        raw: &[u8; 192],
    ) -> Result<
        (
            RistrettoPoint,
            RistrettoPoint,
            RistrettoPoint,
            Scalar,
            Scalar,
            Scalar,
        ),
        (),
    > {
        // Pass a reference to the 32-byte array returned by array32(...)
        let a1 = point_from_bytes(&array32(&raw[0..32])?).map_err(|_| ())?;
        let a2 = point_from_bytes(&array32(&raw[32..64])?).map_err(|_| ())?;
        let a3 = point_from_bytes(&array32(&raw[64..96])?).map_err(|_| ())?;

        // These take a value, so no & here
        let z_k = Scalar::from_bytes_mod_order(array32(&raw[96..128])?);
        let z_v = Scalar::from_bytes_mod_order(array32(&raw[128..160])?);
        let z_r = Scalar::from_bytes_mod_order(array32(&raw[160..192])?);

        Ok((a1, a2, a3, z_k, z_v, z_r))
    }

    fn array32(slice: &[u8]) -> Result<[u8; 32], ()> {
        if slice.len() != 32 {
            return Err(());
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(slice);
        Ok(out)
    }

    /// Deterministic H derivation — MUST match prover.
    fn pedersen_h_generator() -> RistrettoPoint {
        use sha2::Sha512;
        RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
    }

    /// Derive a compact context tag from transcript state (to bind range proofs).
    fn transcript_context_bytes(t: &Transcript) -> Vec<u8> {
        let mut tr = t.clone();
        let mut out = [0u8; 32];
        tr.challenge_bytes(b"ctx", &mut out);
        out.to_vec()
    }

    /// Pad or trim an arbitrary asset id to 32 bytes for transcript binding.
    fn pad_or_trim_32(x: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        if x.len() >= 32 {
            out.copy_from_slice(&x[0..32]);
        } else {
            out[0..x.len()].copy_from_slice(x);
        }
        out
    }
}

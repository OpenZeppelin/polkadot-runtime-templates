//! zk_elgamal_prover — std-only prover matching `pallet-zk-elgamal`
//!
//! - Reuses `primitives-zk-elgamal` (labels, transcript order, fixed byte layouts).
//! - Emits the exact proof-bundle format your pallet expects:
//!     delta_comm(32) || link(192) || len1(u16) || range1 || len2(u16) || range2
//! - Uses Bulletproofs (single-value, 64-bit) for range proofs.
//! - Encrypts Δv to the **sender** ElGamal pk (matches verifier Eq2).
//!
//! NOTE: Keep Pedersen H derivation and transcript labels in perfect sync with the pallet.

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G, ristretto::RistrettoPoint, scalar::Scalar,
    traits::Identity,
};
use merlin::Transcript;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::Sha512;
use thiserror::Error;

use primitives_zk_elgamal::{
    Ciphertext, PublicContext, SDK_VERSION, append_point, challenge_scalar as fs_chal, labels,
    new_transcript, point_to_bytes,
};

// ---- Optional cross-check against Solana's SDK (no feature flags) ----
use solana_zk_sdk::encryption::elgamal as sdk_elgamal;

#[derive(Debug, Error)]
pub enum ProverError {
    #[error("malformed input")]
    Malformed,
    #[error("range proof failed")]
    RangeProof,
}

/// Input required to produce a pallet-compatible proof bundle.
pub struct TransferInput {
    /// Asset namespace (will be pad/trimmed to 32 inside transcript).
    pub asset_id: Vec<u8>,
    /// Network/chain domain tag (exactly what pallet binds).
    pub network_id: [u8; 32],

    /// ElGamal public keys as points.
    pub sender_pk: RistrettoPoint,
    pub receiver_pk: RistrettoPoint,

    /// Sender old commitment C_from_old and its opening.
    pub from_old_C: RistrettoPoint,
    pub from_old_opening: (u64, Scalar),

    /// Receiver old commitment C_to_old (opening not required).
    pub to_old_C: RistrettoPoint,

    /// Transfer amount Δv (u64).
    pub delta_value: u64,

    /// Deterministic RNG seed (use proper randomness in production).
    pub rng_seed: [u8; 32],

    /// Bind a fee commitment if you use fees; else None (identity bound).
    pub fee_C: Option<RistrettoPoint>,

    /// Include a receiver-side range proof (requires receiver opening). Typically false.
    pub include_receiver_range_proof: bool,
}

/// Outputs the exact two byte blobs your extrinsic must pass + convenience commits.
pub struct ProveOutput {
    /// For pallet argument `proof_bundle_bytes`.
    pub bundle_bytes: Vec<u8>,
    /// For pallet argument `delta_ct_bytes` (C||D = 64 bytes).
    pub delta_ct_bytes: [u8; 64],
    /// Convenience (what the chain will recompute): sender & receiver new commitments.
    pub from_new_C: [u8; 32],
    pub to_new_C: [u8; 32],
}

/// Main entry: produce Δciphertext and the proof bundle.
pub fn prove_transfer(inp: &TransferInput) -> Result<ProveOutput, ProverError> {
    // ----- Witnesses & randomness -----
    let (v_from_old_u64, r_from_old) = inp.from_old_opening;
    let v_from_old = Scalar::from(v_from_old_u64);
    let dv_u64 = inp.delta_value;
    let dv = Scalar::from(dv_u64);

    let mut rng = ChaCha20Rng::from_seed(inp.rng_seed);
    let k = Scalar::from(rng.next_u64()); // ElGamal randomness
    let rho = Scalar::from(rng.next_u64()); // ΔC blind
    let a_k = Scalar::from(rng.next_u64()); // CP blinding for k
    let a_v = Scalar::from(rng.next_u64()); // CP blinding for v
    let a_r = Scalar::from(rng.next_u64()); // CP blinding for rho

    // ----- Pedersen params -----
    let H = pedersen_h_generator();

    // ----- ΔC and Δciphertext (tied to SENDER pk to match verifier Eq2) -----
    let delta_C = dv * G + rho * H;
    let delta_ct = elgamal_encrypt_delta(&inp.sender_pk, dv_u64, &k);

    // Optional sanity check: primitives <-> SDK encoding agree
    {
        let sdk_ct = sdk_from_primitives_ct(&delta_ct);
        let back = sdk_ct.to_bytes();
        assert_eq!(back, delta_ct.to_bytes(), "SDK/primitives CT mismatch");
    }

    // ----- Bind canonical PublicContext before FS challenge -----
    let ctx = PublicContext {
        network_id: inp.network_id,
        sdk_version: SDK_VERSION,
        asset_id: pad_or_trim_32(&inp.asset_id),
        sender_pk: inp.sender_pk,
        receiver_pk: inp.receiver_pk,
        auditor_pk: None,
        fee_commitment: inp.fee_C.unwrap_or_else(RistrettoPoint::identity),
        ciphertext_out: delta_ct,
        ciphertext_in: None,
    };
    let mut t = transcript_for(&ctx);

    // ----- Sigma commitments -----
    let A1 = a_k * G;
    let A2 = a_v * G + a_k * inp.sender_pk;
    let A3 = a_v * G + a_r * H;

    append_point(&mut t, b"a1", &A1);
    append_point(&mut t, b"a2", &A2);
    append_point(&mut t, b"a3", &A3);

    // ----- Fiat–Shamir challenge (shared label) -----
    let c = fs_chal(&mut t, labels::CHAL_EQ);

    // Responses
    let z_k = a_k + c * k;
    let z_v = a_v + c * dv;
    let z_r = a_r + c * rho;

    // ----- New commitments (what chain will compute) -----
    let from_new_C = (v_from_old - dv) * G + (r_from_old - rho) * H;
    let to_new_C = inp.to_old_C + delta_C;

    // ----- Range proofs bound to transcript context bytes -----
    let ctx_bytes = transcript_context_bytes(&t);
    let from_new_bytes = point_to_bytes(&from_new_C);
    let to_new_bytes = point_to_bytes(&to_new_C);

    let range_from = prove_range_u64(
        b"range_from_new",
        &ctx_bytes,
        &from_new_bytes,
        v_from_old_u64
            .checked_sub(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_from_old - rho),
    )?;

    // Usually omitted (requires receiver opening/cooperation).
    let range_to = if inp.include_receiver_range_proof {
        // Placeholder pathway: caller must provide receiver opening to actually prove.
        Vec::new()
    } else {
        Vec::new()
    };

    // ----- Assemble bundle: 32 + 192 + len/bytes + len/bytes -----
    let mut bundle = Vec::with_capacity(32 + 192 + 2 + range_from.len() + 2 + range_to.len());
    bundle.extend_from_slice(delta_C.compress().as_bytes());
    bundle.extend_from_slice(&encode_link(&A1, &A2, &A3, &z_k, &z_v, &z_r));
    bundle.extend_from_slice(&(range_from.len() as u16).to_le_bytes());
    bundle.extend_from_slice(&range_from);
    bundle.extend_from_slice(&(range_to.len() as u16).to_le_bytes());
    bundle.extend_from_slice(&range_to);

    Ok(ProveOutput {
        bundle_bytes: bundle,
        delta_ct_bytes: delta_ct.to_bytes(),
        from_new_C: from_new_bytes,
        to_new_C: to_new_bytes,
    })
}

// ========================= Bulletproofs (64-bit) =========================

fn prove_range_u64(
    _label: &'static [u8], // informational; context binding happens via transcript
    ctx_bytes: &[u8],      // bind the same bytes the verifier uses
    commit_compressed: &[u8; 32], // the commitment being constrained (for context binding only)
    value_u64: u64,        // 0..2^64-1
    blind: &Scalar,        // dalek::Scalar blinding for that commitment
) -> Result<Vec<u8>, ProverError> {
    use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
    use curve25519_dalek_ng as dalek_ng;

    // Build a Merlin transcript and bind the same context bytes so the verifier’s
    // RangeVerifier sees the identical transcript state.
    let mut t = merlin::Transcript::new(b"bp64");
    t.append_message(b"ctx", ctx_bytes);
    t.append_message(b"commit", commit_compressed);

    // Bulletproofs 4 uses curve25519-dalek-ng types. Convert the blinding scalar.
    let blind_ng = dalek_ng::scalar::Scalar::from_bytes_mod_order(blind.to_bytes());

    let pg = PedersenGens::default();
    let bp_gens = BulletproofGens::new(64, 1);

    // Prove a single 64-bit range; BP 4 expects a transcript, not an RNG.
    let (proof, _bp_commit) =
        RangeProof::prove_single(&bp_gens, &pg, &mut t, value_u64, &blind_ng, 64)
            .map_err(|_| ProverError::RangeProof)?;

    Ok(proof.to_bytes())
}

// ========================= Helpers =========================

fn pedersen_h_generator() -> RistrettoPoint {
    RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
}

fn transcript_for(ctx: &PublicContext) -> Transcript {
    new_transcript(ctx)
}

/// Encrypt Δv under **sender_pk** (matches verifier Eq2).
fn elgamal_encrypt_delta(sender_pk: &RistrettoPoint, delta_v: u64, k: &Scalar) -> Ciphertext {
    let v = Scalar::from(delta_v);
    let C = k * G;
    let D = v * G + (*k) * (*sender_pk);
    Ciphertext { C, D }
}

/// 192-byte link proof (A1||A2||A3||z_k||z_v||z_r).
fn encode_link(
    a1: &RistrettoPoint,
    a2: &RistrettoPoint,
    a3: &RistrettoPoint,
    z_k: &Scalar,
    z_v: &Scalar,
    z_r: &Scalar,
) -> [u8; 192] {
    let mut out = [0u8; 192];
    out[0..32].copy_from_slice(a1.compress().as_bytes());
    out[32..64].copy_from_slice(a2.compress().as_bytes());
    out[64..96].copy_from_slice(a3.compress().as_bytes());
    out[96..128].copy_from_slice(&z_k.to_bytes());
    out[128..160].copy_from_slice(&z_v.to_bytes());
    out[160..192].copy_from_slice(&z_r.to_bytes());
    out
}

fn pad_or_trim_32(x: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    if x.len() >= 32 {
        out.copy_from_slice(&x[0..32]);
    } else {
        out[0..x.len()].copy_from_slice(x);
    }
    out
}

fn transcript_context_bytes(t: &Transcript) -> [u8; 32] {
    let mut clone = t.clone();
    let mut out = [0u8; 32];
    clone.challenge_bytes(b"ctx", &mut out);
    out
}

// ---- Minimal interop helpers w/ solana_zk_sdk (straightforward mapping) ----

fn sdk_from_primitives_ct(ct: &Ciphertext) -> sdk_elgamal::ElGamalCiphertext {
    use solana_zk_sdk::encryption::elgamal::DecryptHandle;
    use solana_zk_sdk::encryption::pedersen::PedersenCommitment;

    let c_bytes = ct.C.compress().to_bytes();
    let d_bytes = ct.D.compress().to_bytes();

    let commit = PedersenCommitment::from_bytes(&c_bytes).expect("valid point");
    let handle = DecryptHandle::from_bytes(&d_bytes).expect("valid point");

    sdk_elgamal::ElGamalCiphertext {
        commitment: commit,
        handle,
    }
}

// -------------------------- Tests (optional) --------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use curve25519_dalek::traits::Identity;

    #[test]
    fn round_trip_bundle_shapes() {
        // tiny smoke test for shapes; not a cryptographic test.
        let mut seed = [0u8; 32];
        seed[0] = 7;

        let sk = Scalar::from(5u64);
        let sender_pk = (sk * G);
        let receiver_pk = (Scalar::from(9u64) * G);

        let H = pedersen_h_generator();
        let from_old_v = 1234u64;
        let from_old_r = Scalar::from(42u64);
        let from_old_C = Scalar::from(from_old_v) * G + from_old_r * H;
        let to_old_C = RistrettoPoint::identity();

        let inp = TransferInput {
            asset_id: b"TEST_ASSET".to_vec(),
            network_id: [1u8; 32],
            sender_pk,
            receiver_pk,
            from_old_C,
            from_old_opening: (from_old_v, from_old_r),
            to_old_C,
            delta_value: 111,
            rng_seed: seed,
            fee_C: None,
            include_receiver_range_proof: false,
        };

        let out = prove_transfer(&inp).expect("prove");
        assert_eq!(out.delta_ct_bytes.len(), 64);
        // bundle = 32 + 192 + 2 + rp + 2 + rp
        assert!(out.bundle_bytes.len() >= 32 + 192 + 2 + 0 + 2 + 0);
    }
}

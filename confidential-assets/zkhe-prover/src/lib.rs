//! zk_elgamal_prover — std-only prover matching the two-phase flow
//!
//! Phase 1 (sender initiates):
//!   - `prove_sender_transfer(...)`
//!     Emits:
//!       * Δciphertext (64 bytes: C||D)
//!       * sender bundle bytes:
//!           delta_comm(32) || link(192) || len1(2) || range_from_new || len2(2)=0
//!       * delta_comm(32) explicitly
//!
//! Phase 2 (receiver accepts):
//!   - `prove_receiver_accept(...)`
//!     Emits a compact envelope (Option A to match verifier):
//!       * delta_comm(32) || len1(2) || range_avail_new || len2(2) || range_pending_new
//!
//! Notes:
//! - Bulletproofs are single 64-bit range proofs bound to a transcript context identical to
//!   the verifier's binding logic.
//! - For simplicity, `prove_receiver_accept` requires the receiver openings for both
//!   available and pending commitments, plus the transfer witnesses (Δv, ρ). In production,
//!   the Δv, ρ can be reconstructed from consumed UTXOs if desired.

#[cfg(test)]
mod tests;

use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G, ristretto::RistrettoPoint, scalar::Scalar,
    traits::Identity,
};
use merlin::Transcript;
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::Sha512;
use thiserror::Error;

use zkhe_primitives::{
    Ciphertext, PublicContext, SDK_VERSION, append_point, challenge_scalar as fs_chal, labels,
    new_transcript, point_to_bytes,
};

// Interop check (optional)
use solana_zk_sdk::encryption::elgamal as sdk_elgamal;

#[derive(Debug, Error)]
pub enum ProverError {
    #[error("malformed input")]
    Malformed,
    #[error("range proof failed")]
    RangeProof,
}

fn pedersen_h_generator() -> RistrettoPoint {
    RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
}

fn transcript_for(ctx: &PublicContext) -> Transcript {
    new_transcript(ctx)
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

// Updated: acceptance context must match verifier (receiver_pk, avail_old, pending_old, delta_comm)
fn accept_ctx_bytes(
    network_id: [u8; 32],
    asset_id: [u8; 32],
    receiver_pk: &RistrettoPoint,
    avail_old: &RistrettoPoint,
    pending_old: &RistrettoPoint,
    delta_comm: &RistrettoPoint,
) -> [u8; 32] {
    let mut t = Transcript::new(labels::PROTOCOL);
    t.append_message(b"proto", labels::PROTOCOL_V);
    t.append_message(b"sdk_version", &SDK_VERSION.to_le_bytes());
    t.append_message(b"network_id", &network_id);
    t.append_message(b"asset_id", &asset_id);
    append_point(&mut t, b"receiver_pk", receiver_pk);
    append_point(&mut t, b"avail_old", avail_old);
    append_point(&mut t, b"pending_old", pending_old);
    append_point(&mut t, b"delta_comm", delta_comm);
    let mut out = [0u8; 32];
    t.challenge_bytes(b"ctx", &mut out);
    out
}

/// Encrypt Δv under **sender_pk** (matches verifier Eq2).
fn elgamal_encrypt_delta(sender_pk: &RistrettoPoint, delta_v: u64, k: &Scalar) -> Ciphertext {
    let v = Scalar::from(delta_v);
    let c = k * G;
    let d = v * G + (*k) * (*sender_pk);
    Ciphertext { C: c, D: d }
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

/// Produce a 64-bit single-value Bulletproof range proof, with an explicit
/// `transcript_label` folded into the transcript so sender/receiver proofs use
/// distinct transcript RNG streams.
fn prove_range_u64(
    transcript_label: &[u8],
    ctx_bytes: &[u8],
    commit_compressed: &[u8; 32],
    value_u64: u64,
    blind: &Scalar,
) -> Result<Vec<u8>, ProverError> {
    use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
    use curve25519_dalek_ng as dalek_ng;

    // derive H in non-ng dalek, then convert to ng
    fn pedersen_h_generator_ng() -> dalek_ng::ristretto::RistrettoPoint {
        let h_std = curve25519_dalek::ristretto::RistrettoPoint::hash_from_bytes::<sha2::Sha512>(
            b"Zether/PedersenH",
        );
        let bytes = h_std.compress().to_bytes();
        dalek_ng::ristretto::CompressedRistretto(bytes)
            .decompress()
            .expect("valid H")
    }

    let mut t = merlin::Transcript::new(b"bp64");
    // IMPORTANT: fold in the caller-provided label (must match verifier usage).
    t.append_message(b"label", transcript_label);
    t.append_message(b"ctx", ctx_bytes);
    t.append_message(b"commit", commit_compressed);

    let blind_ng = dalek_ng::scalar::Scalar::from_bytes_mod_order(blind.to_bytes());

    let pg = PedersenGens {
        B: dalek_ng::constants::RISTRETTO_BASEPOINT_POINT,
        B_blinding: pedersen_h_generator_ng(),
    };
    let bp_gens = BulletproofGens::new(64, 1);

    let (proof, _bp_commit) =
        RangeProof::prove_single(&bp_gens, &pg, &mut t, value_u64, &blind_ng, 64)
            .map_err(|_| ProverError::RangeProof)?;

    Ok(proof.to_bytes())
}

// ========================= Sender Phase (unchanged) =========================

pub struct SenderInput {
    pub asset_id: Vec<u8>,
    pub network_id: [u8; 32],

    pub sender_pk: RistrettoPoint,
    pub receiver_pk: RistrettoPoint,

    pub from_old_c: RistrettoPoint,
    pub from_old_opening: (u64, Scalar),

    /// Receiver old commitment (opening not needed in sender phase).
    pub to_old_c: RistrettoPoint,

    /// Δv to send.
    pub delta_value: u64,

    /// Deterministic RNG seed (tests).
    pub rng_seed: [u8; 32],

    /// Optional fee commitment.
    pub fee_c: Option<RistrettoPoint>,
}

pub struct SenderOutput {
    pub delta_ct_bytes: [u8; 64],
    pub sender_bundle_bytes: Vec<u8>,
    pub delta_comm_bytes: [u8; 32],
    pub from_new_c: [u8; 32],
    pub to_new_c: [u8; 32], // computed for convenience (not applied on-chain in phase 1)
}

pub fn prove_sender_transfer(inp: &SenderInput) -> Result<SenderOutput, ProverError> {
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

    let h = pedersen_h_generator();
    let delta_c = dv * G + rho * h;
    let delta_ct = elgamal_encrypt_delta(&inp.sender_pk, dv_u64, &k);

    // SDK interop check
    {
        let sdk_ct = {
            use solana_zk_sdk::encryption::elgamal::DecryptHandle;
            use solana_zk_sdk::encryption::pedersen::PedersenCommitment;
            let c_bytes = delta_ct.C.compress().to_bytes();
            let d_bytes = delta_ct.D.compress().to_bytes();
            let commit = PedersenCommitment::from_bytes(&c_bytes).expect("valid point");
            let handle = DecryptHandle::from_bytes(&d_bytes).expect("valid point");
            sdk_elgamal::ElGamalCiphertext {
                commitment: commit,
                handle,
            }
        };
        assert_eq!(sdk_ct.to_bytes(), delta_ct.to_bytes());
    }

    // Public context
    let ctx = PublicContext {
        network_id: inp.network_id,
        sdk_version: SDK_VERSION,
        asset_id: pad_or_trim_32(&inp.asset_id),
        sender_pk: inp.sender_pk,
        receiver_pk: inp.receiver_pk,
        auditor_pk: None,
        fee_commitment: inp.fee_c.unwrap_or_else(RistrettoPoint::identity),
        ciphertext_out: delta_ct,
        ciphertext_in: None,
    };
    let mut t = transcript_for(&ctx);

    // Σ-commitments
    let a1 = a_k * G;
    let a2 = a_v * G + a_k * inp.sender_pk;
    let a3 = a_v * G + a_r * h;

    append_point(&mut t, b"a1", &a1);
    append_point(&mut t, b"a2", &a2);
    append_point(&mut t, b"a3", &a3);

    // Challenge
    let c = fs_chal(&mut t, labels::CHAL_EQ);

    // Responses
    let z_k = a_k + c * k;
    let z_v = a_v + c * dv;
    let z_r = a_r + c * rho;

    // New commitments
    let from_new_c = (v_from_old - dv) * G + (r_from_old - rho) * h;
    let to_new_c = inp.to_old_c + delta_c;

    // Sender range proof bound to sender transcript context bytes
    let ctx_bytes = transcript_context_bytes(&t);
    let from_new_bytes = point_to_bytes(&from_new_c);
    let to_new_bytes = point_to_bytes(&to_new_c);

    let range_from = prove_range_u64(
        b"range_from_new", // MUST match verifier call-site label
        &ctx_bytes,
        &from_new_bytes,
        v_from_old_u64
            .checked_sub(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_from_old - rho),
    )?;

    // Assemble sender bundle (receiver range len = 0)
    let mut bundle = Vec::with_capacity(32 + 192 + 2 + range_from.len() + 2);
    bundle.extend_from_slice(delta_c.compress().as_bytes());
    bundle.extend_from_slice(&encode_link(&a1, &a2, &a3, &z_k, &z_v, &z_r));
    bundle.extend_from_slice(&(range_from.len() as u16).to_le_bytes());
    bundle.extend_from_slice(&range_from);
    bundle.extend_from_slice(&(0u16).to_le_bytes()); // len2 = 0

    let mut delta_comm_bytes = [0u8; 32];
    delta_comm_bytes.copy_from_slice(delta_c.compress().as_bytes());

    Ok(SenderOutput {
        delta_ct_bytes: delta_ct.to_bytes(),
        sender_bundle_bytes: bundle,
        delta_comm_bytes,
        from_new_c: from_new_bytes,
        to_new_c: to_new_bytes,
    })
}

// ========================= Receiver Phase (updated) =========================

pub struct ReceiverAcceptInput {
    pub asset_id: Vec<u8>,
    pub network_id: [u8; 32],

    pub receiver_pk: RistrettoPoint,

    // Old commitments and their openings:
    pub avail_old_c: RistrettoPoint,
    pub avail_old_opening: (u64, Scalar),

    pub pending_old_c: RistrettoPoint,
    pub pending_old_opening: (u64, Scalar),

    /// ΔC commitment (sum of selected pending-UTXO C parts) and its witnesses (Δv, ρ).
    pub delta_comm: RistrettoPoint,
    pub delta_value: u64,
    pub delta_rho: Scalar,
}

pub struct ReceiverAcceptOutput {
    /// Envelope expected by verifier `verify_transfer_received`:
    ///   delta_comm(32) || len1(2) || rp_avail_new || len2(2) || rp_pending_new
    pub accept_envelope: Vec<u8>,
    pub avail_new_c: [u8; 32],
    pub pending_new_c: [u8; 32],
}

pub fn prove_receiver_accept(
    inp: &ReceiverAcceptInput,
) -> Result<ReceiverAcceptOutput, ProverError> {
    let (v_av_u64, r_av_old) = inp.avail_old_opening;
    let (v_pend_u64, r_pend_old) = inp.pending_old_opening;

    let dv_u64 = inp.delta_value;
    let dv = Scalar::from(dv_u64);
    let rho = inp.delta_rho;

    // Sanity: ΔC = dv*G + rho*H (not strictly required by verifier, but catches input bugs)
    let h = pedersen_h_generator();
    let delta_c_recomputed = dv * G + rho * h;
    debug_assert_eq!(
        delta_c_recomputed.compress().to_bytes(),
        inp.delta_comm.compress().to_bytes()
    );

    // Compute new commitments/openings (must match verifier semantics):
    // avail_new = avail_old + ΔC, pending_new = pending_old - ΔC
    let avail_new_c = inp.avail_old_c + inp.delta_comm;
    let pending_new_c = inp.pending_old_c - inp.delta_comm;

    // Acceptance transcript context (shared by both proofs; MUST match verifier)
    let ctx_bytes = accept_ctx_bytes(
        inp.network_id,
        pad_or_trim_32(&inp.asset_id),
        &inp.receiver_pk,
        &inp.avail_old_c,
        &inp.pending_old_c,
        &inp.delta_comm,
    );

    let avail_new_bytes = point_to_bytes(&avail_new_c);
    let pending_new_bytes = point_to_bytes(&pending_new_c);

    // Produce both range proofs with the exact labels the verifier expects.
    let rp_avail_new = prove_range_u64(
        b"range_avail_new",
        &ctx_bytes,
        &avail_new_bytes,
        v_av_u64
            .checked_add(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_av_old + rho),
    )?;

    let rp_pending_new = prove_range_u64(
        b"range_pending_new",
        &ctx_bytes,
        &pending_new_bytes,
        v_pend_u64
            .checked_sub(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_pend_old - rho),
    )?;

    // Envelope: ΔC(32) || len1(2) || rp_avail_new || len2(2) || rp_pending_new
    let mut env = Vec::with_capacity(32 + 2 + rp_avail_new.len() + 2 + rp_pending_new.len());
    env.extend_from_slice(inp.delta_comm.compress().as_bytes());
    env.extend_from_slice(&(rp_avail_new.len() as u16).to_le_bytes());
    env.extend_from_slice(&rp_avail_new);
    env.extend_from_slice(&(rp_pending_new.len() as u16).to_le_bytes());
    env.extend_from_slice(&rp_pending_new);

    Ok(ReceiverAcceptOutput {
        accept_envelope: env,
        avail_new_c: avail_new_bytes,
        pending_new_c: pending_new_bytes,
    })
}

// ... (file header + existing code unchanged above)

// ========================= Mint (public -> confidential) =========================

pub struct MintInput {
    pub asset_id: Vec<u8>,
    pub network_id: [u8; 32],

    pub to_pk: RistrettoPoint,

    // Old commitments + openings
    pub to_pending_old_c: RistrettoPoint,
    pub to_pending_old_opening: (u64, Scalar),

    pub total_old_c: RistrettoPoint,
    pub total_old_opening: (u64, Scalar),

    /// Amount to mint (move from transparent into confidential)
    pub mint_value: u64,

    /// Deterministic seed for tests
    pub rng_seed: [u8; 32],
}

pub struct MintOutput {
    pub minted_ct_bytes: [u8; 64],
    pub proof_bytes: Vec<u8>,       // matches verifier's verify_mint layout
    pub to_pending_new_c: [u8; 32], // convenience
    pub total_new_c: [u8; 32],      // convenience
}

pub fn prove_mint(inp: &MintInput) -> Result<MintOutput, ProverError> {
    let (v_to_old_u64, r_to_old) = inp.to_pending_old_opening;
    let (v_total_old_u64, r_total_old) = inp.total_old_opening;

    // Randomness
    let mut rng = ChaCha20Rng::from_seed(inp.rng_seed);
    let k = Scalar::from(rng.next_u64()); // ElGamal nonce
    let rho = Scalar::from(rng.next_u64()); // ΔC blind

    // ΔC and ciphertext to `to_pk`
    let h = pedersen_h_generator();
    let dv_u64 = inp.mint_value;
    let dv = Scalar::from(dv_u64);

    let delta_c = dv * G + rho * h;
    let minted_ct = elgamal_encrypt_delta(&inp.to_pk, dv_u64, &k);

    // Public context identical to verifier binding
    let ctx = PublicContext {
        network_id: inp.network_id,
        sdk_version: SDK_VERSION,
        asset_id: pad_or_trim_32(&inp.asset_id),
        sender_pk: inp.to_pk,   // bind to to_pk
        receiver_pk: inp.to_pk, // domain sep (harmless duplicate)
        auditor_pk: None,
        fee_commitment: RistrettoPoint::identity(),
        ciphertext_out: minted_ct,
        ciphertext_in: None,
    };
    let mut t = transcript_for(&ctx);

    // Σ-proof commitments
    let a_k = Scalar::from(rng.next_u64());
    let a_v = Scalar::from(rng.next_u64());
    let a_r = Scalar::from(rng.next_u64());

    let a1 = a_k * G;
    let a2 = a_v * G + a_k * inp.to_pk;
    let a3 = a_v * G + a_r * h;

    append_point(&mut t, b"a1", &a1);
    append_point(&mut t, b"a2", &a2);
    append_point(&mut t, b"a3", &a3);

    let c = fs_chal(&mut t, labels::CHAL_EQ);
    let z_k = a_k + c * k;
    let z_v = a_v + c * dv;
    let z_r = a_r + c * rho;

    // New commitments
    let to_new = inp.to_pending_old_c + delta_c;
    let total_new = inp.total_old_c + delta_c;

    let ctx_bytes = transcript_context_bytes(&t);
    let to_new_bytes = point_to_bytes(&to_new);
    let total_new_bytes = point_to_bytes(&total_new);

    // Range proofs
    let rp_to_new = prove_range_u64(
        b"range_to_pending_new",
        &ctx_bytes,
        &to_new_bytes,
        v_to_old_u64
            .checked_add(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_to_old + rho),
    )?;

    let rp_total_new = prove_range_u64(
        b"range_total_new",
        &ctx_bytes,
        &total_new_bytes,
        v_total_old_u64
            .checked_add(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_total_old + rho),
    )?;

    // Assemble proof bytes:
    // minted_ct(64) || delta_comm(32) || link(192) || len1 || rp_to_new || len2 || rp_total_new
    let mut proof =
        Vec::with_capacity(64 + 32 + 192 + 2 + rp_to_new.len() + 2 + rp_total_new.len());
    {
        let ct_bytes = minted_ct.to_bytes();
        proof.extend_from_slice(&ct_bytes);
    }
    proof.extend_from_slice(delta_c.compress().as_bytes());
    proof.extend_from_slice(&encode_link(&a1, &a2, &a3, &z_k, &z_v, &z_r));

    proof.extend_from_slice(&(rp_to_new.len() as u16).to_le_bytes());
    proof.extend_from_slice(&rp_to_new);

    proof.extend_from_slice(&(rp_total_new.len() as u16).to_le_bytes());
    proof.extend_from_slice(&rp_total_new);

    Ok(MintOutput {
        minted_ct_bytes: minted_ct.to_bytes(),
        proof_bytes: proof,
        to_pending_new_c: to_new_bytes,
        total_new_c: total_new_bytes,
    })
}

// ========================= Burn (confidential -> public) =========================

pub struct BurnInput {
    pub asset_id: Vec<u8>,
    pub network_id: [u8; 32],

    pub from_pk: RistrettoPoint,

    // Old commitments + openings
    pub from_avail_old_c: RistrettoPoint,
    pub from_avail_old_opening: (u64, Scalar),

    pub total_old_c: RistrettoPoint,
    pub total_old_opening: (u64, Scalar),

    /// Amount to burn (move from confidential into transparent)
    pub burn_value: u64,

    /// Deterministic seed for tests
    pub rng_seed: [u8; 32],
}

pub struct BurnOutput {
    pub amount_ct_bytes: [u8; 64],  // ciphertext of v to from_pk
    pub proof_bytes: Vec<u8>,       // matches verifier's verify_burn layout
    pub from_avail_new_c: [u8; 32], // convenience
    pub total_new_c: [u8; 32],      // convenience
}

pub fn prove_burn(inp: &BurnInput) -> Result<BurnOutput, ProverError> {
    let (v_from_old_u64, r_from_old) = inp.from_avail_old_opening;
    let (v_total_old_u64, r_total_old) = inp.total_old_opening;

    let mut rng = ChaCha20Rng::from_seed(inp.rng_seed);
    let k = Scalar::from(rng.next_u64()); // ElGamal nonce
    let rho = Scalar::from(rng.next_u64()); // ΔC blind

    let h = pedersen_h_generator();
    let dv_u64 = inp.burn_value;
    let dv = Scalar::from(dv_u64);

    let delta_c = dv * G + rho * h;
    let amount_ct = elgamal_encrypt_delta(&inp.from_pk, dv_u64, &k);

    let ctx = PublicContext {
        network_id: inp.network_id,
        sdk_version: SDK_VERSION,
        asset_id: pad_or_trim_32(&inp.asset_id),
        sender_pk: inp.from_pk,
        receiver_pk: inp.from_pk,
        auditor_pk: None,
        fee_commitment: RistrettoPoint::identity(),
        ciphertext_out: amount_ct,
        ciphertext_in: None,
    };
    let mut t = transcript_for(&ctx);

    // Σ-proof commitments
    let a_k = Scalar::from(rng.next_u64());
    let a_v = Scalar::from(rng.next_u64());
    let a_r = Scalar::from(rng.next_u64());

    let a1 = a_k * G;
    let a2 = a_v * G + a_k * inp.from_pk;
    let a3 = a_v * G + a_r * h;

    append_point(&mut t, b"a1", &a1);
    append_point(&mut t, b"a2", &a2);
    append_point(&mut t, b"a3", &a3);

    let c = fs_chal(&mut t, labels::CHAL_EQ);
    let z_k = a_k + c * k;
    let z_v = a_v + c * dv;
    let z_r = a_r + c * rho;

    // New commitments (subtract Δ)
    let from_new = inp.from_avail_old_c - delta_c;
    let total_new = inp.total_old_c - delta_c;

    let ctx_bytes = transcript_context_bytes(&t);
    let from_new_bytes = point_to_bytes(&from_new);
    let total_new_bytes = point_to_bytes(&total_new);

    // Range proofs for decreased values
    let rp_from_new = prove_range_u64(
        b"range_from_avail_new",
        &ctx_bytes,
        &from_new_bytes,
        v_from_old_u64
            .checked_sub(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_from_old - rho),
    )?;

    let rp_total_new = prove_range_u64(
        b"range_total_new",
        &ctx_bytes,
        &total_new_bytes,
        v_total_old_u64
            .checked_sub(dv_u64)
            .ok_or(ProverError::RangeProof)?,
        &(r_total_old - rho),
    )?;

    // Assemble proof:
    // delta_comm(32) || link(192) || len1 || rp_from_new || len2 || rp_total_new || v_le_u64(8)
    let mut proof =
        Vec::with_capacity(32 + 192 + 2 + rp_from_new.len() + 2 + rp_total_new.len() + 8);
    proof.extend_from_slice(delta_c.compress().as_bytes());
    proof.extend_from_slice(&encode_link(&a1, &a2, &a3, &z_k, &z_v, &z_r));

    proof.extend_from_slice(&(rp_from_new.len() as u16).to_le_bytes());
    proof.extend_from_slice(&rp_from_new);

    proof.extend_from_slice(&(rp_total_new.len() as u16).to_le_bytes());
    proof.extend_from_slice(&rp_total_new);

    proof.extend_from_slice(&dv_u64.to_le_bytes());

    Ok(BurnOutput {
        amount_ct_bytes: amount_ct.to_bytes(),
        proof_bytes: proof,
        from_avail_new_c: from_new_bytes,
        total_new_c: total_new_bytes,
    })
}

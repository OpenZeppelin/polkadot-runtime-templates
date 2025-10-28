//! no_std on-chain verification for ZK El Gamal proofs (sender + accept, Option A).
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod range;
pub use range::BulletproofRangeVerifier;

#[cfg(test)]
mod tests;

use alloc::vec::Vec;
use confidential_assets_primitives::ZkVerifier;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::RistrettoPoint,
    scalar::Scalar,
    traits::{Identity, IsIdentity},
};
use merlin::Transcript;
use zkhe_primitives::{
    append_point, challenge_scalar as fs_chal, labels, new_transcript, point_from_bytes,
    point_to_bytes, Ciphertext, FixedProof, PublicContext, RangeProofVerifier, SDK_VERSION,
};

pub struct ZkheVerifier;

impl ZkVerifier for ZkheVerifier {
    type Error = (); // replace with concrete error type as needed

    // ---------------- Sender path (unchanged) ----------------
    fn verify_transfer_sent(
        asset: &[u8],
        from_pk_bytes: &[u8],
        to_pk_bytes: &[u8],
        from_old_bytes: &[u8],
        to_old_bytes: &[u8],
        delta_ct_bytes: &[u8],
        proof_bundle_bytes: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error> {
        let from_pk = parse_point32(from_pk_bytes).map_err(|_| ())?;
        let to_pk = parse_point32(to_pk_bytes).map_err(|_| ())?;
        let from_old = parse_point32_allow_empty_identity(from_old_bytes).map_err(|_| ())?;
        let to_old = parse_point32_allow_empty_identity(to_old_bytes).map_err(|_| ())?;
        let delta_ct = Ciphertext::from_bytes(delta_ct_bytes).map_err(|_| ())?;
        let proof = TransferProof::parse(proof_bundle_bytes).map_err(|_| ())?;

        // public context
        let asset_id = pad_or_trim_32(asset);
        let network_id = [0u8; 32];
        let ctx = PublicContext {
            network_id,
            sdk_version: SDK_VERSION,
            asset_id,
            sender_pk: from_pk,
            receiver_pk: to_pk,
            auditor_pk: None,
            fee_commitment: RistrettoPoint::identity(),
            ciphertext_out: delta_ct,
            ciphertext_in: None,
        };
        let mut t = new_transcript(&ctx);

        // link Σ-proof
        let (a1, a2, a3, z_k, z_v, z_r) =
            parse_link_from_192(proof.link_raw.as_bytes()).map_err(|_| ())?;
        append_point(&mut t, b"a1", &a1);
        append_point(&mut t, b"a2", &a2);
        append_point(&mut t, b"a3", &a3);
        let c: Scalar = fs_chal(&mut t, labels::CHAL_EQ);

        // Eq1
        if !((z_k * G) - (a1 + c * delta_ct.C)).is_identity() {
            return Err(());
        }
        // Eq2
        if !((z_v * G + z_k * from_pk) - (a2 + c * delta_ct.D)).is_identity() {
            return Err(());
        }
        // Eq3
        let h = pedersen_h_generator();
        if !((z_v * G + z_r * h) - (a3 + c * proof.delta_comm)).is_identity() {
            return Err(());
        }

        // compute new commitments
        let from_new = from_old - proof.delta_comm;
        let to_new = to_old + proof.delta_comm;

        // optional range proofs
        let ctx_bytes = transcript_context_bytes(&t);
        let from_new_bytes = point_to_bytes(&from_new);
        let to_new_bytes = point_to_bytes(&to_new);

        if !proof.range_from_new.is_empty() {
            BulletproofRangeVerifier::verify_range_proof(
                b"range_from_new",
                &ctx_bytes,
                &from_new_bytes,
                proof.range_from_new,
            )
            .map_err(|_| ())?;
        }
        if !proof.range_to_new.is_empty() {
            BulletproofRangeVerifier::verify_range_proof(
                b"range_to_new",
                &ctx_bytes,
                &to_new_bytes,
                proof.range_to_new,
            )
            .map_err(|_| ())?;
        }

        Ok((from_new_bytes.to_vec(), to_new_bytes.to_vec()))
    }

    // ---------------- Receiver path ----------------
    //
    // The pallet passes the consumed pending UTXOs as compressed Pedersen commitments:
    //     pending_commits: &[[u8; 32]]
    //
    // Envelope: delta_comm(32) || len1(2) || rp_avail_new || len2(2) || rp_pending_new
    //
    // Returns (available_new, pending_new) as compressed points.
    fn verify_transfer_received(
        asset: &[u8],
        who_pk_bytes: &[u8],
        avail_old_bytes: &[u8],
        pending_old_bytes: &[u8],
        pending_commits: &[[u8; 32]],
        accept_envelope_bytes: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>), Self::Error> {
        let who_pk = parse_point32(who_pk_bytes).map_err(|_| ())?;
        let avail_old = parse_point32_allow_empty_identity(avail_old_bytes).map_err(|_| ())?;
        let pending_old = parse_point32_allow_empty_identity(pending_old_bytes).map_err(|_| ())?;
        let env = AcceptEnvelope::parse(accept_envelope_bytes).map_err(|_| ())?;

        // 1) Σ pending commitments must equal ΔC
        let mut sum = RistrettoPoint::identity();
        for c_bytes in pending_commits {
            let c = point_from_bytes(c_bytes).map_err(|_| ())?;
            sum += c;
        }
        if !points_eq(&sum, &env.delta_comm) {
            return Err(());
        }

        // 2) Acceptance context shared by both range proofs
        let network_id = [0u8; 32];
        let asset_id = pad_or_trim_32(asset);
        let mut t = Transcript::new(labels::PROTOCOL);
        t.append_message(b"proto", labels::PROTOCOL_V);
        t.append_message(b"sdk_version", &SDK_VERSION.to_le_bytes());
        t.append_message(b"network_id", &network_id);
        t.append_message(b"asset_id", &asset_id);
        append_point(&mut t, b"receiver_pk", &who_pk);
        append_point(&mut t, b"avail_old", &avail_old);
        append_point(&mut t, b"pending_old", &pending_old);
        append_point(&mut t, b"delta_comm", &env.delta_comm);

        let mut ctx_bytes = [0u8; 32];
        {
            let mut t2 = t.clone();
            t2.challenge_bytes(b"ctx", &mut ctx_bytes);
        }

        // 3) Compute new commitments and verify range proofs
        let avail_new = avail_old + env.delta_comm;
        let pending_new = pending_old - env.delta_comm;

        let avail_new_bytes = point_to_bytes(&avail_new);
        let pending_new_bytes = point_to_bytes(&pending_new);

        BulletproofRangeVerifier::verify_range_proof(
            b"range_avail_new",
            &ctx_bytes,
            &avail_new_bytes,
            env.range_avail_new,
        )
        .map_err(|_| ())?;

        BulletproofRangeVerifier::verify_range_proof(
            b"range_pending_new",
            &ctx_bytes,
            &pending_new_bytes,
            env.range_pending_new,
        )
        .map_err(|_| ())?;

        Ok((avail_new_bytes.to_vec(), pending_new_bytes.to_vec()))
    }

    fn disclose(_asset: &[u8], _who_pk: &[u8], _cipher: &[u8]) -> Result<u64, Self::Error> {
        Err(())
    }
}

// ---------------- Proof byte “contracts” ----------------

/// 192-byte link-proof: A1(32)||A2(32)||A3(32)||z_k(32)||z_v(32)||z_r(32)
type LinkProofBytes = FixedProof<192>;

/// Sender bundle: delta_comm(32) || link(192) || len1(2) || range_from || len2(2) || range_to
struct TransferProof<'a> {
    delta_comm: RistrettoPoint,
    link_raw: LinkProofBytes,
    range_from_new: &'a [u8],
    range_to_new: &'a [u8],
}

impl<'a> TransferProof<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
        if bytes.len() < 32 + 192 + 2 + 2 {
            return Err(());
        }
        let delta_comm = point_from_bytes(&array32(&bytes[0..32])?).map_err(|_| ())?;
        let link_raw = LinkProofBytes::from_slice(&bytes[32..32 + 192]).map_err(|_| ())?;

        let mut off = 32 + 192;
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

/// Accept envelope (Option A):
/// delta_comm(32) || len1(2) || rp_avail_new || len2(2) || rp_pending_new
struct AcceptEnvelope<'a> {
    delta_comm: RistrettoPoint,
    range_avail_new: &'a [u8],
    range_pending_new: &'a [u8],
}

impl<'a> AcceptEnvelope<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
        if bytes.len() < 32 + 2 + 2 {
            return Err(());
        }
        let delta_comm = point_from_bytes(&array32(&bytes[0..32])?).map_err(|_| ())?;

        let mut off = 32;
        let len1 = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
        off += 2;
        if bytes.len() < off + len1 + 2 {
            return Err(());
        }
        let rp1 = &bytes[off..off + len1];
        off += len1;

        let len2 = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
        off += 2;
        if bytes.len() < off + len2 {
            return Err(());
        }
        let rp2 = &bytes[off..off + len2];

        Ok(Self {
            delta_comm,
            range_avail_new: rp1,
            range_pending_new: rp2,
        })
    }
}

// ---------------- Helpers ----------------

fn parse_point32(bytes: &[u8]) -> Result<RistrettoPoint, ()> {
    if bytes.len() != 32 {
        return Err(());
    }
    let mut b = [0u8; 32];
    b.copy_from_slice(bytes);
    point_from_bytes(&b).map_err(|_| ())
}

fn parse_point32_allow_empty_identity(bytes: &[u8]) -> Result<RistrettoPoint, ()> {
    if bytes.is_empty() {
        Ok(RistrettoPoint::identity())
    } else {
        parse_point32(bytes)
    }
}

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
    let a1 = point_from_bytes(&array32(&raw[0..32])?).map_err(|_| ())?;
    let a2 = point_from_bytes(&array32(&raw[32..64])?).map_err(|_| ())?;
    let a3 = point_from_bytes(&array32(&raw[64..96])?).map_err(|_| ())?;
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

fn pedersen_h_generator() -> RistrettoPoint {
    use sha2::Sha512;
    RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
}

fn transcript_context_bytes(t: &Transcript) -> Vec<u8> {
    let mut tr = t.clone();
    let mut out = [0u8; 32];
    tr.challenge_bytes(b"ctx", &mut out);
    out.to_vec()
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

fn points_eq(a: &RistrettoPoint, b: &RistrettoPoint) -> bool {
    a.compress().to_bytes() == b.compress().to_bytes()
}

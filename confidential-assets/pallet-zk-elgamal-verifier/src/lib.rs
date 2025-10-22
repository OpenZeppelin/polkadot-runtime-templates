#![cfg_attr(not(feature = "std"), no_std)]

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

use primitives_zk_elgamal::{
    append_point, challenge_scalar as fs_chal, labels, new_transcript, point_from_bytes,
    point_to_bytes, Ciphertext, FixedProof, PublicContext, SDK_VERSION,
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
    }

    // --------- Range proof trait remains pluggable ---------
    pub trait RangeProofVerifier {
        /// Verify a range proof bound to `commit` and transcript context bytes.
        /// `transcript_label` can distinguish "from_new" vs "to_new".
        fn verify_range_proof(
            transcript_label: &[u8],
            context: &[u8],
            commit_compressed: &[u8; 32],
            proof_bytes: &[u8],
        ) -> core::result::Result<(), ()>;
    }

    // ========= New, primitives-backed parsing/bundle types =========

    /// 192-byte link-proof used by verifier:
    /// A1(32)||A2(32)||A3(32)||z_k(32)||z_v(32)||z_r(32)
    type LinkProofBytes = FixedProof<192>;

    /// Bundle carried on-chain for transfer verification:
    /// delta_comm(32) || link(192) || len1(2) || range1 || len2(2) || range2
    struct TransferProof<'a> {
        delta_comm: RistrettoPoint, // ΔC
        link_raw: LinkProofBytes,   // serialized link proof (192 bytes)
        range_from_new: &'a [u8],   // opaque; delegated to RangeVerifier
        range_to_new: &'a [u8],
    }

    impl<'a> TransferProof<'a> {
        fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
            // delta C (32)
            if bytes.len() < 32 {
                return Err(());
            }
            let mut c_buf = [0u8; 32];
            c_buf.copy_from_slice(&bytes[0..32]);
            let delta_comm = point_from_bytes(&c_buf).map_err(|_| ())?;

            // link (192)
            if bytes.len() < 32 + 192 + 2 + 2 {
                return Err(());
            }
            let link_raw = LinkProofBytes::from_slice(&bytes[32..32 + 192]).map_err(|_| ())?;

            // ranges (len1|bytes|len2|bytes)
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

    // ========= Core verification =========

    impl<T: Config> ZkVerifierTrait for Pallet<T> {
        type Error = ();

        fn verify_transfer_and_apply(
            asset: &[u8],
            from_pk_bytes: &[u8],
            to_pk_bytes: &[u8],
            from_old_bytes: &[u8],
            to_old_bytes: &[u8],
            delta_ct_bytes: &[u8],
            proof_bundle_bytes: &[u8],
        ) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), Self::Error> {
            // --- Parse inputs using shared primitives ---
            let from_pk = parse_point32(from_pk_bytes).map_err(|_| ())?;
            let to_pk = parse_point32(to_pk_bytes).map_err(|_| ())?;
            let from_old = parse_point32_allow_empty_identity(from_old_bytes).map_err(|_| ())?;
            let to_old = parse_point32_allow_empty_identity(to_old_bytes).map_err(|_| ())?;
            let delta_ct = Ciphertext::from_bytes(delta_ct_bytes).map_err(|_| ())?;
            let proof = TransferProof::parse(proof_bundle_bytes).map_err(|_| ())?;

            // --- Bind canonical PublicContext into Merlin transcript ---
            // NOTE: Ensure ALL fields match the prover's binding logic!
            // Fill asset_id and network_id deterministically (pad/trim to 32).
            let asset_id = pad_or_trim_32(asset);
            // TODO: replace with real chain/para id tag (e.g., blake2(genesis_hash||pallet_name))
            let network_id = [0u8; 32];

            let ctx = PublicContext {
                network_id,
                sdk_version: SDK_VERSION,
                asset_id,
                sender_pk: from_pk,
                receiver_pk: to_pk,
                auditor_pk: None, // if add auditor later, bind here
                fee_commitment: RistrettoPoint::identity(), // bind fee model if any
                ciphertext_out: delta_ct, // we treat Δciphertext as "out"
                ciphertext_in: None, // optional (unused in this scheme)
            };
            let mut t = new_transcript(&ctx);

            // --- Absorb link proof's A1/A2/A3 before challenge ---
            let (a1, a2, a3, z_k, z_v, z_r) =
                parse_link_from_192(proof.link_raw.as_bytes()).map_err(|_| ())?;
            append_point(&mut t, b"a1", &a1);
            append_point(&mut t, b"a2", &a2);
            append_point(&mut t, b"a3", &a3);

            // --- Fiat–Shamir challenge ---
            // Prefer a stable, named label (shared labels::CHAL_EQ keeps things consistent).
            let c: Scalar = fs_chal(&mut t, labels::CHAL_EQ);

            // --- Verify the three equations (same algebra as before) ---
            // Eq1: z_k·G == A1 + c·C1
            let lhs1 = z_k * G;
            let rhs1 = a1 + c * delta_ct.C;
            if !(lhs1 - rhs1).is_identity() {
                return Err(());
            }

            // Eq2: z_v·G + z_k·PK == A2 + c·C2
            // Convention: tie to sender's pk (same as previous code). If switch sides, update prover too.
            let lhs2 = z_v * G + z_k * from_pk;
            let rhs2 = a2 + c * delta_ct.D;
            if !(lhs2 - rhs2).is_identity() {
                return Err(());
            }

            // Eq3: z_v·G + z_r·H == A3 + c·ΔC
            let h = pedersen_h_generator(); // MUST MATCH PROVER
            let lhs3 = z_v * G + z_r * h;
            let rhs3 = a3 + c * proof.delta_comm;
            if !(lhs3 - rhs3).is_identity() {
                return Err(());
            }

            // --- Apply balance deltas (commitments add in the group) ---
            let from_new = from_old - proof.delta_comm;
            let to_new = to_old + proof.delta_comm;

            // --- Range proofs (optional but recommended) ---
            // Bind them to the SAME transcript context bytes so the proof can’t be replayed elsewhere.
            let ctx_bytes = transcript_context_bytes(&t);
            let from_new_bytes = point_to_bytes(&from_new);
            let to_new_bytes = point_to_bytes(&to_new);

            T::RangeVerifier::verify_range_proof(
                b"range_from_new",
                &ctx_bytes,
                &from_new_bytes,
                proof.range_from_new,
            )
            .map_err(|_| ())?;
            T::RangeVerifier::verify_range_proof(
                b"range_to_new",
                &ctx_bytes,
                &to_new_bytes,
                proof.range_to_new,
            )
            .map_err(|_| ())?;

            // --- Encode outputs (unchanged_total sentinel = empty Vec) ---
            Ok((from_new_bytes.to_vec(), to_new_bytes.to_vec(), Vec::new()))
        }

        fn apply_acl_transfer(
            _asset: &[u8],
            _from_pk: &[u8],
            _to_pk: &[u8],
            _from_old: &[u8],
            _to_old: &[u8],
            _amount_ct: &[u8],
        ) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), Self::Error> {
            Err(()) // implement ACL policy if needed
        }

        fn disclose(_asset: &[u8], _who_pk: &[u8], _cipher: &[u8]) -> Result<u64, Self::Error> {
            Err(()) // implement auditor/view-key flow if add it
        }
    }

    // ========= Helpers (now all via shared primitives) =========

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
    /// NOTE: We currently decode scalars with *mod order*, matching original code path.
    /// If prefer canonical-only, switch to `Scalar::from_canonical_bytes` and reject non-canonical.
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
        // Points
        let a1 = point_from_bytes(array32(&raw[0..32])?).map_err(|_| ())?;
        let a2 = point_from_bytes(array32(&raw[32..64])?).map_err(|_| ())?;
        let a3 = point_from_bytes(array32(&raw[64..96])?).map_err(|_| ())?;

        // Scalars (mod order for compatibility with current prover)
        let z_k = Scalar::from_bytes_mod_order(*array32(&raw[96..128])?);
        let z_v = Scalar::from_bytes_mod_order(*array32(&raw[128..160])?);
        let z_r = Scalar::from_bytes_mod_order(*array32(&raw[160..192])?);

        Ok((a1, a2, a3, z_k, z_v, z_r))
    }

    fn array32(slice: &[u8]) -> Result<&[u8; 32], ()> {
        if slice.len() != 32 {
            return Err(());
        }
        // SAFETY: just checked length
        Ok(unsafe { &*(slice.as_ptr() as *const [u8; 32]) })
    }

    /// Deterministic H derivation — MUST match prover.
    /// Swap this out for a shared `PedersenParams { G, H }` source when wire that in.
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

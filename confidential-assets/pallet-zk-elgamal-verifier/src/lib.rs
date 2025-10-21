#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use sp_std::vec::Vec;

use ca_primitives::ZkVerifier as ZkVerifierTrait;
use curve25519_dalek::{
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
    traits::{Identity, IsIdentity},
};
use merlin::Transcript;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Optional: a range-proof verifier you can plug later.
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

    // ---------- Public types (fixed-size, PoV-friendly) ----------

    #[derive(Clone, Copy)]
    struct ElGamalCipher {
        c1: RistrettoPoint,
        c2: RistrettoPoint,
    }
    impl ElGamalCipher {
        /// Bytes layout = c1||c2 as 32+32 compressed
        fn parse(bytes: &[u8]) -> Result<Self, ()> {
            if bytes.len() != 64 {
                return Err(());
            }
            let c1 = decompress_point(&bytes[0..32])?;
            let c2 = decompress_point(&bytes[32..64])?;
            Ok(Self { c1, c2 })
        }
    }

    #[derive(Clone, Copy)]
    struct Commitment(RistrettoPoint);
    impl Commitment {
        fn parse(bytes: &[u8]) -> Result<Self, ()> {
            if bytes.is_empty() {
                // Treat empty as identity (zero balance); keep consistent with storage default.
                return Ok(Self(RistrettoPoint::identity()));
            }
            if bytes.len() != 32 {
                return Err(());
            }
            Ok(Self(decompress_point(bytes)?))
        }
        fn encode(&self) -> [u8; 32] {
            self.0.compress().to_bytes()
        }
        fn add(&self, other: &Commitment) -> Commitment {
            Commitment(self.0 + other.0)
        }
        fn sub(&self, other: &Commitment) -> Commitment {
            Commitment(self.0 - other.0)
        }
    }

    #[derive(Clone, Copy)]
    struct PublicKey(RistrettoPoint);
    impl PublicKey {
        fn parse(bytes: &[u8]) -> Result<Self, ()> {
            if bytes.len() != 32 {
                return Err(());
            }
            Ok(Self(decompress_point(bytes)?))
        }
    }

    // Link/equality proof tying ElGamal plaintext Δv to Pedersen delta ΔC
    struct LinkProof {
        a1: RistrettoPoint,
        a2: RistrettoPoint,
        a3: RistrettoPoint,
        z_k: Scalar,
        z_v: Scalar,
        z_r: Scalar, // ρ response
                     // NOTE: we require ΔC to be included as part of the bundle (see TransferProof below).
    }

    impl LinkProof {
        // Layout:
        // a1(32) || a2(32) || a3(32) || z_k(32) || z_v(32) || z_r(32)
        fn parse(bytes: &[u8]) -> Result<Self, ()> {
            if bytes.len() != 32 * 6 {
                return Err(());
            }
            let a1 = decompress_point(&bytes[0..32])?;
            let a2 = decompress_point(&bytes[32..64])?;
            let a3 = decompress_point(&bytes[64..96])?;
            let z_k = scalar_from_bytes(&bytes[96..128])?;
            let z_v = scalar_from_bytes(&bytes[128..160])?;
            let z_r = scalar_from_bytes(&bytes[160..192])?;
            Ok(Self {
                a1,
                a2,
                a3,
                z_k,
                z_v,
                z_r,
            })
        }
    }

    // Bundle carried on-chain for transfer verification
    struct TransferProof<'a> {
        delta_comm: Commitment, // ΔC
        link: LinkProof,
        range_from_new: &'a [u8], // opaque; delegated to RangeVerifier
        range_to_new: &'a [u8],
    }
    impl<'a> TransferProof<'a> {
        // Bytes layout you commit to off-chain (simple, fixed):
        // delta_comm(32) || link(192) || len1(2) || range1 || len2(2) || range2
        fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
            if bytes.len() < 32 + 192 + 2 + 2 {
                return Err(());
            }
            let delta_comm = Commitment::parse(&bytes[0..32])?;
            let link = LinkProof::parse(&bytes[32..224])?;

            let mut off = 224;
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
                link,
                range_from_new: range1,
                range_to_new: range2,
            })
        }
    }

    // A trait you can back with Bulletproofs (or your preferred scheme) later.
    pub trait RangeProofVerifier {
        /// Verify a range proof binding to `commit` and transcript context bytes.
        /// Return Ok(()) if in range (e.g., 0..2^64), Err otherwise.
        fn verify_range_proof(
            transcript_label: &[u8],
            context: &[u8],
            commit_compressed: &[u8; 32],
            proof_bytes: &[u8],
        ) -> core::result::Result<(), ()>;
    }

    // ---------- Utilities ----------

    fn decompress_point(bytes: &[u8]) -> Result<RistrettoPoint, ()> {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        CompressedRistretto(arr).decompress().ok_or(())
    }

    fn scalar_from_bytes(bytes: &[u8]) -> Result<Scalar, ()> {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Ok(Scalar::from_bytes_mod_order(arr))
    }

    fn append_msg(t: &mut Transcript, k: &'static [u8], v: &[u8]) {
        t.append_message(k, v);
    }
    fn challenge_scalar(t: &mut Transcript, label: &'static [u8]) -> Scalar {
        let mut buf = [0u8; 64];
        t.challenge_bytes(label, &mut buf);
        Scalar::from_bytes_mod_order_wide(&buf)
    }

    fn domain_transcript(
        mut t: Transcript,
        asset: &[u8],
        from_pk: &PublicKey,
        to_pk: &PublicKey,
        from_old: &Commitment,
        to_old: &Commitment,
        ct: &ElGamalCipher,
        delta_c: &Commitment,
    ) -> Transcript {
        append_msg(&mut t, b"ds.chain", b"your-chain-id"); // replace if you pass real id
        append_msg(&mut t, b"ds.pallet", b"pallet-zether");
        append_msg(&mut t, b"ds.op", b"transfer.v1");
        append_msg(&mut t, b"ds.asset", asset);
        append_msg(&mut t, b"from_pk", &from_pk.0.compress().to_bytes());
        append_msg(&mut t, b"to_pk", &to_pk.0.compress().to_bytes());
        append_msg(&mut t, b"from_old", &from_old.encode());
        append_msg(&mut t, b"to_old", &to_old.encode());
        append_msg(&mut t, b"ct.c1", &ct.c1.compress().to_bytes());
        append_msg(&mut t, b"ct.c2", &ct.c2.compress().to_bytes());
        append_msg(&mut t, b"delta_c", &delta_c.encode());
        t
    }

    // ---------- Implement the verifier trait used by pallet-zether ----------

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
            // 0) Parse inputs
            let from_pk = PublicKey::parse(from_pk_bytes).map_err(|_| ())?;
            let to_pk = PublicKey::parse(to_pk_bytes).map_err(|_| ())?;
            let from_old = Commitment::parse(from_old_bytes).map_err(|_| ())?;
            let to_old = Commitment::parse(to_old_bytes).map_err(|_| ())?;
            let ct = ElGamalCipher::parse(delta_ct_bytes).map_err(|_| ())?;
            let proof = TransferProof::parse(proof_bundle_bytes).map_err(|_| ())?;

            // 1) Build transcript
            let mut t = domain_transcript(
                Transcript::new(b"zether.transfer"),
                asset,
                &from_pk,
                &to_pk,
                &from_old,
                &to_old,
                &ct,
                &proof.delta_comm,
            );

            // 2) Absorb A1, A2, A3
            append_msg(&mut t, b"a1", &proof.link.a1.compress().to_bytes());
            append_msg(&mut t, b"a2", &proof.link.a2.compress().to_bytes());
            append_msg(&mut t, b"a3", &proof.link.a3.compress().to_bytes());

            // 3) Challenge
            let c = challenge_scalar(&mut t, b"chal");

            // 4) Verify the three equations
            // Eq1: z_k·G == A1 + c·C1
            let lhs1 = proof.link.z_k * curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
            let rhs1 = proof.link.a1 + c * ct.c1;
            if !(lhs1 - rhs1).is_identity() {
                return Err(());
            }

            // Eq2: z_v·G + z_k·PK == A2 + c·C2
            let lhs2 = proof.link.z_v * curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT
                + proof.link.z_k * from_pk.0; // choose PK – either from_pk or to_pk depending on your convention
            let rhs2 = proof.link.a2 + c * ct.c2;
            if (lhs2 - rhs2).is_identity() == false {
                return Err(());
            }

            // Eq3: z_v·G + z_r·H == A3 + c·ΔC
            // We derive H as hash-to-curve to avoid shipping another constant. Must match the prover.
            let h = pedersen_h_generator();
            let lhs3 = proof.link.z_v * curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT
                + proof.link.z_r * h;
            let rhs3 = proof.link.a3 + c * proof.delta_comm.0;
            if (lhs3 - rhs3).is_identity() == false {
                return Err(());
            }

            // 5) Compute new commitments
            let from_new = from_old.sub(&proof.delta_comm);
            let to_new = to_old.add(&proof.delta_comm);

            // 6) Range proofs (optional now, strongly recommended in prod)
            // Bind to the SAME transcript context; pass commit bytes and proof bytes to the verifier.
            T::RangeVerifier::verify_range_proof(
                b"range_from_new",
                &transcript_context_bytes(&t),
                &from_new.encode(),
                proof.range_from_new,
            )
            .map_err(|_| ())?;
            T::RangeVerifier::verify_range_proof(
                b"range_to_new",
                &transcript_context_bytes(&t),
                &to_new.encode(),
                proof.range_to_new,
            )
            .map_err(|_| ())?;

            // 7) Output (encode commitments back to 32B each). total_new = "unchanged" sentinel (empty)
            Ok((
                from_new.encode().to_vec(),
                to_new.encode().to_vec(),
                Vec::new(),
            ))
        }

        fn apply_acl_transfer(
            _asset: &[u8],
            _from_pk: &[u8],
            _to_pk: &[u8],
            _from_old: &[u8],
            _to_old: &[u8],
            _amount_ct: &[u8],
        ) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), Self::Error> {
            Err(()) // implement your policy flow (allowlists / static limits) if you want ACL mode
        }

        fn disclose(_asset: &[u8], _who_pk: &[u8], _cipher: &[u8]) -> Result<u64, Self::Error> {
            Err(()) // implement if you add auditor/view keys
        }
    }

    // ---------- Pedersen H generator & transcript binding ----------

    fn pedersen_h_generator() -> RistrettoPoint {
        // Derive H = hash_to_curve("Zether/H")
        use curve25519_dalek::ristretto::RistrettoPoint as RP;
        use sha2::Sha512; // dalek re-exports a Digest trait; if you want to avoid sha2, use Merlin as DST

        // If you want to avoid sha2 dep entirely, derive via Merlin transcript:
        // let mut tr = Transcript::new(b"Zether/PedersenH");
        // let mut buf = [0u8;64];
        // tr.challenge_bytes(b"H", &mut buf);
        // RP::from_hash(Sha512::new().chain_update(&buf))

        RP::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
    }

    /// Extract a stable, short "context bytes" from the transcript for range proofs.
    /// You can also reconstruct the transcript in the range verifier instead.
    fn transcript_context_bytes(t: &Transcript) -> Vec<u8> {
        // Derive a 32-byte context tag from the current transcript state.
        let mut tr = t.clone();
        let mut out = [0u8; 32];
        tr.challenge_bytes(b"ctx", &mut out);
        out.to_vec()
    }
}

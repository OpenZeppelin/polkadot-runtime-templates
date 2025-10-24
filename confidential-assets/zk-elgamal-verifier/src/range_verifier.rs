//! Range Proof Verifier (no_std compatible)

use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use core::result::Result;
use curve25519_dalek_ng::ristretto::CompressedRistretto;
use merlin::Transcript;
use primitives_zk_elgamal::RangeProofVerifier;
use rand_core::{CryptoRng, Error as RandError, RngCore};

/// Simple deterministic RNG seeded once from a transcript to avoid lifetime aliasing.
struct SeededXorShift64 {
    state: u64,
}

impl SeededXorShift64 {
    fn from_transcript(t: &mut Transcript) -> Self {
        let mut seed = [0u8; 32];
        t.challenge_bytes(b"rng_seed", &mut seed);
        let mut s = 0u64;
        for chunk in seed.chunks_exact(8) {
            let mut w = [0u8; 8];
            w.copy_from_slice(chunk);
            s ^= u64::from_le_bytes(w);
        }
        if s == 0 {
            s = 0x9E3779B97F4A7C15;
        }
        Self { state: s }
    }

    #[inline(always)]
    fn next_u64_inner(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

impl RngCore for SeededXorShift64 {
    fn next_u32(&mut self) -> u32 {
        (self.next_u64_inner() & 0xFFFF_FFFF) as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.next_u64_inner()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut i = 0;
        while i < dest.len() {
            let word = self.next_u64_inner().to_le_bytes();
            let take = core::cmp::min(8, dest.len() - i);
            dest[i..i + take].copy_from_slice(&word[..take]);
            i += take;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> core::result::Result<(), RandError> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for SeededXorShift64 {}

/// Verifies a single-party 2^n range proof deterministically with no external RNG or seed.
/// Works in `no_std` and avoids mutable aliasing of the transcript.
fn verify_single_no_entropy(
    proof: &RangeProof,
    bp_gens: &BulletproofGens,
    pc_gens: &PedersenGens,
    t: &mut Transcript,
    v: &CompressedRistretto,
    n: usize,
) -> Result<(), ()> {
    t.append_message(b"V", v.as_bytes());
    t.append_u64(b"n_bits", n as u64);

    let mut det_rng = SeededXorShift64::from_transcript(t);

    proof
        .verify_single_with_rng(bp_gens, pc_gens, t, v, n, &mut det_rng)
        .map_err(|_| ())
}

/// Bulletproofs-backed range verifier for 64-bit single-value proofs.
pub struct BulletproofRangeVerifier;

impl RangeProofVerifier for BulletproofRangeVerifier {
    fn verify_range_proof(
        _transcript_label: &[u8], // unused to keep parity with prover
        context: &[u8],
        commit_compressed: &[u8; 32],
        proof_bytes: &[u8],
    ) -> Result<(), ()> {
        use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
        use curve25519_dalek_ng::ristretto::CompressedRistretto;
        use merlin::Transcript;

        // IMPORTANT: must match prover exactly
        let mut t = Transcript::new(b"bp64");
        t.append_message(b"ctx", context);
        t.append_message(b"commit", commit_compressed);

        let proof = RangeProof::from_bytes(proof_bytes).map_err(|_| ())?;
        let bp_gens = BulletproofGens::new(64, 1);
        let pedersen_gens = PedersenGens::default();
        let v = CompressedRistretto(*commit_compressed);

        // Deterministic RNG seeded from transcript is fine; keep your helper
        let mut det_rng = SeededXorShift64::from_transcript(&mut t);

        proof
            .verify_single_with_rng(&bp_gens, &pedersen_gens, &mut t, &v, 64, &mut det_rng)
            .map_err(|_| ())
    }
}
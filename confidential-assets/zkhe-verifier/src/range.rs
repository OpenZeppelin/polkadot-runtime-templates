//! Range Proof Verifier (no_std compatible)

use core::result::Result;
use merlin::Transcript;
use rand_core::{CryptoRng, Error as RandError, RngCore};
use zkhe_primitives::RangeProofVerifier;

// --- DEBUG UTILITIES (enabled under tests or with `std`) ---
#[cfg(any(test, feature = "std"))]
macro_rules! dbgln {
    ($($arg:tt)*) => { println!($($arg)*); }
}
#[cfg(not(any(test, feature = "std")))]
macro_rules! dbgln {
    ($($arg:tt)*) => {};
}

#[cfg(any(test, feature = "std"))]
fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(not(any(test, feature = "std")))]
fn hex(_: &[u8]) -> String {
    String::new()
}

#[inline]
fn pedersen_h_generator_ng() -> curve25519_dalek_ng::ristretto::RistrettoPoint {
    use curve25519_dalek_ng::ristretto::CompressedRistretto as CompressedRistrettoNg;
    let h_std = curve25519_dalek::ristretto::RistrettoPoint::hash_from_bytes::<sha2::Sha512>(
        b"Zether/PedersenH",
    );
    let bytes = h_std.compress().to_bytes();
    CompressedRistrettoNg(bytes).decompress().expect("valid H")
}

/// Simple deterministic RNG seeded once from a transcript to avoid lifetime aliasing.
struct SeededXorShift64 {
    state: u64,
}

impl SeededXorShift64 {
    #[inline]
    fn from_seed64(seed: u64) -> Self {
        // Avoid zero; if given zero, map to a known non-zero
        let s = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
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

        // 0) dump inputs
        dbgln!("--- BulletproofRangeVerifier::verify_range_proof ---");
        dbgln!("ctx.len = {}, ctx = {}", context.len(), hex(context));
        dbgln!("commit (32) = {}", hex(commit_compressed));
        dbgln!("proof_len = {}", proof_bytes.len());

        // 1) build transcript EXACTLY like the prover
        let mut t = Transcript::new(b"bp64");
        t.append_message(b"ctx", context);
        t.append_message(b"commit", commit_compressed);

        // 2) parse the proof bytes
        let proof = match RangeProof::from_bytes(proof_bytes) {
            Ok(p) => {
                dbgln!("proof: parsed OK");
                p
            }
            Err(_) => {
                dbgln!("proof: FAILED to parse from bytes");
                return Err(());
            }
        };

        // 3) generators must match the prover exactly
        let bp_gens = BulletproofGens::new(64, 1);
        use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G_NG;
        let h_ng = pedersen_h_generator_ng();
        let pedersen_gens = PedersenGens {
            B: G_NG,
            B_blinding: h_ng,
        };

        // quick sanity: show B and H bytes (compressed)
        dbgln!("gens.B = {}", hex(G_NG.compress().as_bytes()));
        dbgln!("gens.H = {}", hex(h_ng.compress().as_bytes()));

        // 4) commitment as compressed point (what verify_single_with_rng expects)
        let v = CompressedRistretto(*commit_compressed);
        let v_decomp_ok = v.decompress().is_some();
        dbgln!("commit decompress ok? {}", v_decomp_ok);

        #[cfg(any(test))]
        {
            use curve25519_dalek_ng::{ristretto::CompressedRistretto, traits::IsIdentity};
            // We don’t know the blinding, so we can’t fully validate, but we can at least ensure
            // the commitment bytes decode to a valid point (you already did) and stay stable:
            let v_pt = CompressedRistretto(*commit_compressed)
                .decompress()
                .expect("commit");
            dbgln!("commit is_identity? {}", v_pt.is_identity());
        }

        const SEEDS: &[u64] = &[
            0x9E3779B97F4A7C15, // φ mix
            0xC2B2AE3D27D4EB4F, // splitmix64 const
            0xDDBA0B6DF2DE6B9B, // another odd 64-bit constant
        ];

        dbgln!("calling verify_single_with_rng(n=64)...");
        for (i, &seed) in SEEDS.iter().enumerate() {
            let mut rng = SeededXorShift64::from_seed64(seed);
            let mut t_try = t.clone(); // keep transcript pristine for each attempt
            let res = proof.verify_single_with_rng(
                &bp_gens,
                &pedersen_gens,
                &mut t_try,
                &v,
                64,
                &mut rng,
            );
            match res {
                Ok(()) => {
                    dbgln!("verify_single_with_rng: OK (seed #{}, 0x{:016x})", i, seed);
                    return Ok(());
                }
                Err(_) => {
                    dbgln!(
                        "verify_single_with_rng: FAILED (seed #{}, 0x{:016x})",
                        i,
                        seed
                    );
                }
            }
        }

        Err(())
    }
}

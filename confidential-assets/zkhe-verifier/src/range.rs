//! Range Proof Verifier (no_std compatible)

use core::result::Result;
use merlin::Transcript;
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

/// Bulletproofs-backed range verifier for 64-bit single-value proofs.
pub struct BulletproofRangeVerifier;

impl RangeProofVerifier for BulletproofRangeVerifier {
    fn verify_range_proof(
        _transcript_label: &[u8], // kept for API parity with the prover, not used here
        context: &[u8],
        commit_compressed: &[u8; 32],
        proof_bytes: &[u8],
    ) -> Result<(), ()> {
        use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
        use curve25519_dalek_ng::ristretto::CompressedRistretto;

        // 0) Input trace (guarded by cfg)
        dbgln!("-- verify_range_proof --");
        dbgln!("ctx.len = {}", context.len());
        dbgln!("commit = {}", hex(commit_compressed));
        dbgln!("proof_len = {}", proof_bytes.len());

        // 1) Rebuild the transcript exactly like the prover
        let mut t = Transcript::new(b"bp64");
        t.append_message(b"ctx", context);
        t.append_message(b"commit", commit_compressed);

        // 2) Parse the proof
        let proof = RangeProof::from_bytes(proof_bytes).map_err(|_| {
            dbgln!("proof: failed to parse");
        })?;

        // 3) Generators must match the prover exactly
        let bp_gens = BulletproofGens::new(64, 1);
        use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G_NG;
        let h_ng = pedersen_h_generator_ng();
        let pedersen_gens = PedersenGens {
            B: G_NG,
            B_blinding: h_ng,
        };

        dbgln!("gens.B = {}", hex(G_NG.compress().as_bytes()));
        dbgln!("gens.H = {}", hex(h_ng.compress().as_bytes()));

        // 4) Commitment as compressed point
        let v = CompressedRistretto(*commit_compressed);
        let v_ok = v.decompress().is_some();
        dbgln!("commit decompress ok? {}", v_ok);

        // 5) Deterministic verifier RNG derived from the transcript
        //
        // Important: The verifier does not have secret witness bytes to rekey with, so we finalize
        // the builder with a deterministic, fixed external RNG seed. This produces an RNG that is
        // *fully pinned* by the transcript state (which already absorbed public context/commitment).
        //
        // The transcript rng implements `RngCore + CryptoRng` and is suitable for
        // `verify_single_with_rng` in no_std.
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;
        let mut ext = ChaCha20Rng::from_seed([0u8; 32]);
        let mut rng = t.build_rng().finalize(&mut ext);

        // 6) Verify
        dbgln!("calling verify_single_with_rng(n=64)...");
        proof
            .verify_single_with_rng(&bp_gens, &pedersen_gens, &mut t, &v, 64, &mut rng)
            .map_err(|_| {
                dbgln!("verify_single_with_rng: FAILED");
            })?;

        dbgln!("verify_single_with_rng: OK");
        Ok(())
    }
}

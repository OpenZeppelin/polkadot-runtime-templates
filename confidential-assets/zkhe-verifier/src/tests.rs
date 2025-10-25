//! Unit tests for the no_std Zk ElGamal verifier.
//! - Uses the root `ZkheVerifier` marker struct implementing `ZkVerifier`.
//! - Prover is std-only and used here to generate test vectors.
//!
//! Covered:
//!   1) Happy path: sender + receiver proofs verify and new commitments match prover outputs
//!   2) Rejection: tampered sender bundle is rejected
//!   3) Range proof only: parse sender bundle, reconstruct transcript context, and verify range proof

use confidential_assets_primitives::ZkVerifier as ZkVerifierTrait;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::RistrettoPoint,
    scalar::Scalar,
    traits::{Identity, IsIdentity},
};
use sha2::Sha512;
use zkhe_primitives::RangeProofVerifier;

// Import the verifier marker struct from the crate root and its range verifier.
use crate::{BulletproofRangeVerifier, ZkheVerifier};

// Prover (std-only) used in tests to generate canonical inputs/proofs.
use zkhe_prover::{prove_receiver_accept, prove_sender_transfer, ReceiverAcceptInput, SenderInput};

// Helpers (tests-only)
fn pedersen_h_generator() -> RistrettoPoint {
    RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
}
fn compress(pt: &RistrettoPoint) -> [u8; 32] {
    pt.compress().to_bytes()
}

// ---------- Bundle parsing (mirrors the on-chain verifier’s parsing logic) ----------
#[derive(Debug)]
struct ParsedSenderBundle<'a> {
    delta_comm: RistrettoPoint, // ΔC
    link_raw_192: [u8; 192],    // A1||A2||A3||z_k||z_v||z_r
    range_from_new: &'a [u8],   // bytes
    range_to_new: &'a [u8],     // bytes (often empty in these tests)
}

fn parse_sender_bundle(bytes: &[u8]) -> Result<ParsedSenderBundle<'_>, ()> {
    if bytes.len() < 32 + 192 + 2 + 2 {
        return Err(());
    }
    // ΔC
    let mut c = [0u8; 32];
    c.copy_from_slice(&bytes[0..32]);
    let delta_comm = zkhe_primitives::point_from_bytes(&c).map_err(|_| ())?;

    // Link proof (192 bytes)
    let mut link_raw_192 = [0u8; 192];
    link_raw_192.copy_from_slice(&bytes[32..224]);

    // Lengths and slices
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

    Ok(ParsedSenderBundle {
        delta_comm,
        link_raw_192,
        range_from_new: range1,
        range_to_new: range2,
    })
}

// Rebuild the exact transcript context the prover used for the sender’s range proof.
// t := new_transcript(ctx); append a1,a2,a3; c := fs_chal(...); ctx_bytes := t.challenge_bytes("ctx")
fn sender_range_context_from_bundle(
    asset_id_raw: &[u8],
    sender_pk: &RistrettoPoint,
    receiver_pk: &RistrettoPoint,
    delta_ct_bytes: &[u8; 64],
    link_raw_192: &[u8; 192],
) -> [u8; 32] {
    use zkhe_primitives::{
        append_point, challenge_scalar as fs_chal, labels, new_transcript, Ciphertext,
        PublicContext, SDK_VERSION,
    };

    // Parse Δciphertext (64 bytes -> C||D)
    let ct = Ciphertext::from_bytes(delta_ct_bytes).expect("valid delta ct");

    // Canonical public context (verifier fixes network_id = [0;32])
    let mut asset_id = [0u8; 32];
    if asset_id_raw.len() >= 32 {
        asset_id.copy_from_slice(&asset_id_raw[..32]);
    } else {
        asset_id[..asset_id_raw.len()].copy_from_slice(asset_id_raw);
    }
    let ctx = PublicContext {
        network_id: [0u8; 32],
        sdk_version: SDK_VERSION,
        asset_id,
        sender_pk: *sender_pk,
        receiver_pk: *receiver_pk,
        auditor_pk: None,
        fee_commitment: RistrettoPoint::identity(),
        ciphertext_out: ct,
        ciphertext_in: None,
    };
    let mut t = new_transcript(&ctx);

    // Unpack A1||A2||A3 from the 192-byte link proof prefix.
    use curve25519_dalek::ristretto::CompressedRistretto;
    let a1 = CompressedRistretto(link_raw_192[0..32].try_into().unwrap())
        .decompress()
        .expect("A1");
    let a2 = CompressedRistretto(link_raw_192[32..64].try_into().unwrap())
        .decompress()
        .expect("A2");
    let a3 = CompressedRistretto(link_raw_192[64..96].try_into().unwrap())
        .decompress()
        .expect("A3");

    append_point(&mut t, b"a1", &a1);
    append_point(&mut t, b"a2", &a2);
    append_point(&mut t, b"a3", &a3);

    // inside sender_range_context_from_bundle, after decoding a1/a2/a3 and before fs_chal:
    #[cfg(any(test, feature = "std"))]
    {
        fn hex(x: &[u8]) -> String {
            const T: &[u8; 16] = b"0123456789abcdef";
            let mut s = String::with_capacity(x.len() * 2);
            for &b in x {
                s.push(T[(b >> 4) as usize] as char);
                s.push(T[(b & 0x0f) as usize] as char);
            }
            s
        }

        eprintln!("CTXBUILD: A1 = {}", hex(a1.compress().as_bytes()));
        eprintln!("CTXBUILD: A2 = {}", hex(a2.compress().as_bytes()));
        eprintln!("CTXBUILD: A3 = {}", hex(a3.compress().as_bytes()));
        eprintln!("CTXBUILD: Δct.C = {}", hex(&ct.C.compress().to_bytes()));
        eprintln!("CTXBUILD: Δct.D = {}", hex(&ct.D.compress().to_bytes()));
    }

    // Challenge for the Σ-link (this advances transcript)
    let _c = fs_chal(&mut t, labels::CHAL_EQ);

    // Squeeze the same 32 bytes the prover used to bind the range proof
    let mut ctx_bytes = [0u8; 32];
    let mut t_clone = t.clone();
    t_clone.challenge_bytes(b"ctx", &mut ctx_bytes);
    ctx_bytes
}

// ----------------------------- TESTS -----------------------------

#[test]
fn verify_sender_and_receiver_happy_path() {
    // ----------------- Keys & openings -----------------
    let sk_sender = Scalar::from(5u64);
    let pk_sender = sk_sender * G;
    let pk_receiver = Scalar::from(9u64) * G;

    let h = pedersen_h_generator();

    // Sender starts with a positive balance
    let from_old_v = 1_234u64;
    let from_old_r = Scalar::from(42u64);
    let from_old_c = Scalar::from(from_old_v) * G + from_old_r * h;

    // Receiver starts at zero (identity commitment)
    let to_old_v = 0u64;
    let to_old_r = Scalar::from(0u64);
    let to_old_c = RistrettoPoint::default(); // identity

    // Transfer amount
    let dv = 111u64;

    // IMPORTANT: The verifier currently fixes `network_id = [0;32]`.
    // So we pass the same `[0;32]` here to the prover to match transcript binding.
    let network_id = [0u8; 32];
    let asset_id = b"TEST_ASSET".to_vec();

    // Deterministic seed so sender and receiver agree on witnesses for the test
    let mut seed = [0u8; 32];
    seed[0] = 7;

    // ----------------- Sender phase (prove) -----------------
    let s_in = SenderInput {
        asset_id: asset_id.clone(),
        network_id,
        sender_pk: pk_sender,
        receiver_pk: pk_receiver,
        from_old_c,
        from_old_opening: (from_old_v, from_old_r),
        to_old_c,
        delta_value: dv,
        rng_seed: seed,
        fee_c: None,
    };

    let s_out = prove_sender_transfer(&s_in).expect("sender prover should succeed");

    // Quick shape sanity
    assert_eq!(s_out.delta_ct_bytes.len(), 64);
    // bundle = 32 + 192 + 2 + range_from_len + 2
    assert!(s_out.sender_bundle_bytes.len() >= 32 + 192 + 2 + 1 + 2);

    // ----------------- Sender phase (verify) -----------------
    let (from_new_bytes_v, to_new_bytes_v) =
        <ZkheVerifier as ZkVerifierTrait>::verify_transfer_sent(
            &asset_id,
            &compress(&pk_sender),
            &compress(&pk_receiver),
            &compress(&from_old_c),
            &compress(&to_old_c),
            &s_out.delta_ct_bytes,
            &s_out.sender_bundle_bytes,
        )
        .expect("sender-side verification did not pass");

    // Check verifier matches prover's expected commitments
    assert_eq!(from_new_bytes_v.as_slice(), &s_out.from_new_c);
    assert_eq!(to_new_bytes_v.as_slice(), &s_out.to_new_c);

    // ----------------- Receiver phase (prove) -----------------
    use rand_chacha::ChaCha20Rng;
    use rand_core::{RngCore, SeedableRng};

    let mut chacha = ChaCha20Rng::from_seed(seed);
    let delta_rho = Scalar::from(chacha.next_u64());

    // Deserialize ΔC from sender output
    let delta_comm = {
        use curve25519_dalek::ristretto::CompressedRistretto;
        CompressedRistretto(s_out.delta_comm_bytes)
            .decompress()
            .expect("valid ΔC")
    };

    let r_in = ReceiverAcceptInput {
        asset_id: asset_id.clone(),
        network_id,
        receiver_pk: pk_receiver,
        to_old_c, // same as sender used
        to_old_opening: (to_old_v, to_old_r),
        delta_comm,
        delta_value: dv,
        delta_rho,
    };

    let r_out = prove_receiver_accept(&r_in).expect("receiver prover should succeed");
    assert!(r_out.accept_envelope.len() > 34); // 32 + 2 + >=1

    // ----------------- Receiver phase (verify) -----------------
    let to_new_bytes_recv_v = <ZkheVerifier as ZkVerifierTrait>::verify_transfer_received(
        &asset_id,
        &compress(&pk_receiver),
        &compress(&to_old_c),
        &r_out.accept_envelope,
    )
    .expect("receiver acceptance did not verify");

    // Should match the prover's to_new commitment from sender computation / receiver recompute
    assert_eq!(to_new_bytes_recv_v.as_slice(), &r_out.to_new_c);
}

#[test]
fn rejects_tampered_sender_bundle() {
    // Setup identical to the happy path, then flip one byte in the link proof section.
    let sk_sender = Scalar::from(5u64);
    let pk_sender = sk_sender * G;
    let pk_receiver = Scalar::from(9u64) * G;

    let h = pedersen_h_generator();

    let from_old_v = 555u64;
    let from_old_r = Scalar::from(7u64);
    let from_old_c = Scalar::from(from_old_v) * G + from_old_r * h;

    let to_old_c = RistrettoPoint::default();

    let dv = 10u64;
    let network_id = [0u8; 32];
    let asset_id = b"TEST_ASSET".to_vec();

    let mut seed = [0u8; 32];
    seed[1] = 99;

    let s_in = SenderInput {
        asset_id: asset_id.clone(),
        network_id,
        sender_pk: pk_sender,
        receiver_pk: pk_receiver,
        from_old_c,
        from_old_opening: (from_old_v, from_old_r),
        to_old_c,
        delta_value: dv,
        rng_seed: seed,
        fee_c: None,
    };

    let mut s_out = prove_sender_transfer(&s_in).expect("sender prover should succeed");

    // Tamper a byte inside the link proof region of the bundle:
    // bundle layout: [0..32)=ΔC | [32..224)=link(192) | [224..]=ranges...
    if s_out.sender_bundle_bytes.len() >= 33 {
        s_out.sender_bundle_bytes[32 + 10] ^= 0x01;
    }

    let err = <ZkheVerifier as ZkVerifierTrait>::verify_transfer_sent(
        &asset_id,
        &compress(&pk_sender),
        &compress(&pk_receiver),
        &compress(&from_old_c),
        &compress(&to_old_c),
        &s_out.delta_ct_bytes,
        &s_out.sender_bundle_bytes,
    );

    assert!(err.is_err(), "tampered sender bundle must be rejected");
}

#[test]
fn range_proof_from_sender_bundle_verifies() {
    // Produce a sender proof, then extract and verify the *sender* range proof
    // directly via BulletproofRangeVerifier using the exact transcript context.
    let sk_sender = Scalar::from(123u64);
    let pk_sender = sk_sender * G;
    let pk_receiver = Scalar::from(777u64) * G;

    let h = pedersen_h_generator();

    let from_old_v = 999u64;
    let from_old_r = Scalar::from(4242u64);
    let from_old_c = Scalar::from(from_old_v) * G + from_old_r * h;

    let to_old_c = RistrettoPoint::identity();
    let dv = 100u64;

    let asset_id = b"RANGE_ONLY".to_vec();
    let network_id = [0u8; 32];

    let mut seed = [0u8; 32];
    seed[0] = 3;
    seed[7] = 9;

    let s_in = SenderInput {
        asset_id: asset_id.clone(),
        network_id,
        sender_pk: pk_sender,
        receiver_pk: pk_receiver,
        from_old_c,
        from_old_opening: (from_old_v, from_old_r),
        to_old_c,
        delta_value: dv,
        rng_seed: seed,
        fee_c: None,
    };

    let s_out = prove_sender_transfer(&s_in).expect("sender prover should succeed");

    // Parse bundle and rebuild the exact context bytes
    let parsed = parse_sender_bundle(&s_out.sender_bundle_bytes).expect("parse bundle");
    let ctx_bytes = sender_range_context_from_bundle(
        &asset_id,
        &pk_sender,
        &pk_receiver,
        &s_out.delta_ct_bytes,
        &parsed.link_raw_192,
    );

    // Verify the sender’s range proof against the prover-computed from_new commitment
    let mut commit32 = [0u8; 32];
    commit32.copy_from_slice(&s_out.from_new_c);
    use curve25519_dalek_ng::ristretto::CompressedRistretto as CNg;
    let v = CNg(commit32);
    let v_pt = v.decompress().expect("commit dec");
    let rt = v_pt.compress().to_bytes();
    assert_eq!(rt, commit32, "commit roundtrip mismatch (ng)");

    // ==== EXTRA DEBUG: confirm from_new_c == (from_old_c - ΔC) and try alt-commit ====
    {
        use curve25519_dalek::ristretto::CompressedRistretto;
        // Reconstruct ΔC from bytes
        let delta_c = CompressedRistretto(s_out.delta_comm_bytes)
            .decompress()
            .expect("ΔC dec");

        // Recompute from_new' and to_new' algebraically
        let from_new_recomp = from_old_c - delta_c;
        let to_new_recomp = to_old_c + delta_c;

        fn hex(x: &[u8]) -> String {
            const T: &[u8; 16] = b"0123456789abcdef";
            let mut s = String::with_capacity(x.len() * 2);
            for &b in x {
                s.push(T[(b >> 4) as usize] as char);
                s.push(T[(b & 0x0f) as usize] as char);
            }
            s
        }

        let from_new_recomp_bytes = from_new_recomp.compress().to_bytes();
        let to_new_recomp_bytes = to_new_recomp.compress().to_bytes();

        eprintln!("CHECK: from_new(prover)   = {}", hex(&s_out.from_new_c));
        eprintln!(
            "CHECK: from_new(recomp)   = {}",
            hex(&from_new_recomp_bytes)
        );
        eprintln!("CHECK: to_new(recomp)     = {}", hex(&to_new_recomp_bytes));

        // Hard assert that prover's from_new equals recomputed
        assert_eq!(
            s_out.from_new_c, from_new_recomp_bytes,
            "from_new bytes differ from (from_old - ΔC); this will break range proof binding"
        );

        // Control experiment: try verifying with to_new (should FAIL if proof is for from_new).
        // If it PASSES with to_new, the prover accidentally bound the wrong commitment.
        let mut to_commit32 = [0u8; 32];
        to_commit32.copy_from_slice(&to_new_recomp_bytes);

        let to_res = BulletproofRangeVerifier::verify_range_proof(
            b"range_from_new",
            &ctx_bytes,
            &to_commit32,
            parsed.range_from_new,
        );
        eprintln!("CONTROL: verify with to_new commit => {:?}", to_res);
    }

    {
        use curve25519_dalek_ng::ristretto::CompressedRistretto as CNg;
        let v_ng = CNg(commit32);
        let v_pt_ng = v_ng.decompress().expect("commit dec (ng)");
        let rt_ng = v_pt_ng.compress().to_bytes();
        assert_eq!(rt_ng, commit32, "ng roundtrip mismatch");
    }

    // std-control: verify via bulletproofs using the same transcript RNG pattern
    #[cfg(feature = "std")]
    {
        use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
        use curve25519_dalek_ng::constants::RISTRETTO_BASEPOINT_POINT as G_NG;
        use curve25519_dalek_ng::ristretto::CompressedRistretto;
        use merlin::Transcript;
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;

        let mut t_std = Transcript::new(b"bp64");
        t_std.append_message(b"ctx", &ctx_bytes);
        t_std.append_message(b"commit", &commit32);

        let proof = RangeProof::from_bytes(parsed.range_from_new).expect("parse proof std");
        let bp_gens = BulletproofGens::new(64, 1);
        let pedersen_gens = PedersenGens {
            B: G_NG,
            B_blinding: {
                // same H as prod:
                use curve25519_dalek_ng::ristretto::CompressedRistretto as CompressedRistrettoNg;
                let h_std = curve25519_dalek::ristretto::RistrettoPoint::hash_from_bytes::<
                    sha2::Sha512,
                >(b"Zether/PedersenH");
                let bytes = h_std.compress().to_bytes();
                CompressedRistrettoNg(bytes).decompress().unwrap()
            },
        };

        // build a transcript-derived RNG, like the verifier
        let mut ext = ChaCha20Rng::from_seed([0u8; 32]);
        let mut rng = t_std.build_rng().finalize(&mut ext);

        let v = CompressedRistretto(commit32);
        let res_std =
            proof.verify_single_with_rng(&bp_gens, &pedersen_gens, &mut t_std, &v, 64, &mut rng);
        eprintln!("CONTROL: verify_single_with_rng => {:?}", res_std);
    }

    match BulletproofRangeVerifier::verify_range_proof(
        b"range_from_new",
        &ctx_bytes,
        &commit32,
        parsed.range_from_new,
    ) {
        Ok(()) => { /* great */ }
        Err(()) => {
            // helper hex (copy these two tiny helpers at the top of the test file if you don't already have them)
            fn hex(bytes: &[u8]) -> String {
                const T: &[u8; 16] = b"0123456789abcdef";
                let mut out = String::with_capacity(bytes.len() * 2);
                for &b in bytes {
                    out.push(T[(b >> 4) as usize] as char);
                    out.push(T[(b & 0x0f) as usize] as char);
                }
                out
            }

            let a1b = &parsed.link_raw_192[0..32];
            let a2b = &parsed.link_raw_192[32..64];
            let a3b = &parsed.link_raw_192[64..96];
            let delta_ct_c = &s_out.delta_ct_bytes[0..32];
            let delta_ct_d = &s_out.delta_ct_bytes[32..64];

            eprintln!("TEST: A1     = {}", hex(a1b));
            eprintln!("TEST: A2     = {}", hex(a2b));
            eprintln!("TEST: A3     = {}", hex(a3b));
            eprintln!("TEST: Δct.C  = {}", hex(delta_ct_c));
            eprintln!("TEST: Δct.D  = {}", hex(delta_ct_d));

            eprintln!("---- RANGE VERIFY DEBUG ----");
            eprintln!("ctx (32)     = {}", hex(&ctx_bytes));
            eprintln!("commit (32)  = {}", hex(&commit32));
            eprintln!("proof_len    = {}", parsed.range_from_new.len());
            eprintln!(
                "proof_head   = {}",
                hex(&parsed
                    .range_from_new
                    .get(0..core::cmp::min(64, parsed.range_from_new.len()))
                    .unwrap_or(&[]))
            );
            eprintln!("sender_pk    = {}", hex(&pk_sender.compress().to_bytes()));
            eprintln!("receiver_pk  = {}", hex(&pk_receiver.compress().to_bytes()));
            eprintln!("Δct (64)     = {}", hex(&s_out.delta_ct_bytes));
            eprintln!("ΔC (32)      = {}", hex(&s_out.delta_comm_bytes));
            panic!("range proof should verify (see dumps above)");
        }
    }
}

// (Optional) tiny sanity for pedersen identity commitment path.
#[test]
fn identity_commitment_is_zero_point() {
    let zero = RistrettoPoint::default();
    assert!(zero.is_identity());
}

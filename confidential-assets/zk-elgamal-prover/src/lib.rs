use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use merlin::Transcript;
use rand::{RngCore, SeedableRng, rngs::StdRng};
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 32-byte compressed point
pub type Compressed = [u8; 32];

#[derive(Debug, Error)]
pub enum ProverError {
    #[error("invalid input")]
    InvalidInput,
    #[error("range proof error")]
    RangeProof,
}

/// Public key (Ristretto point)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicKey(#[serde(with = "serde_bytes")] Vec<u8>); // 32 bytes

/// Secret key (scalar) — keep this client-side only
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecretKey(#[serde(with = "serde_bytes")] Vec<u8>); // 32 bytes

/// ElGamal ciphertext: C1 = k·G, C2 = Δv·G + k·PK
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElGamalCipher {
    pub c1: Compressed,
    pub c2: Compressed,
}

/// Pedersen commitment: C = v·G + r·H
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PedersenCommitment {
    pub c: Compressed,
}

/// Link proof (Chaum–Pedersen-style)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkProof {
    pub a1: Compressed,
    pub a2: Compressed,
    pub a3: Compressed,
    pub z_k: [u8; 32],
    pub z_v: [u8; 32],
    pub z_r: [u8; 32],
}

/// Transfer proof bundle (what you put on-chain)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransferProofBundle {
    /// ΔC = Δv·G + ρ·H (the Pedersen delta commitment)
    pub delta_comm: Compressed,
    /// Link proof tying ElGamal Δv to ΔC
    pub link: LinkProof,
    /// Sender new-balance range proof bytes (opaque; verifier plugs a range verifier)
    pub range_from_new: Vec<u8>,
    /// (Optional) Receiver range proof, typically empty unless you add receiver cooperation
    pub range_to_new: Vec<u8>,
    /// The ElGamal ciphertext of Δv (put alongside your extrinsic input)
    pub delta_ct: ElGamalCipher,
}

/// Inputs the prover needs to construct a transfer:
/// - sender knows their current balance opening (v_from_old, r_from_old)
/// - sender chooses Δv (the amount to transfer), and a fresh ρ for ΔC
/// - sender encrypts Δv to the chosen PK (usually receiver's PK)
#[derive(Clone, Debug)]
pub struct TransferProveInput<'a> {
    pub asset_domain: &'a [u8], // asset id bytes for domain sep
    pub from_pk: PublicKey,
    pub to_pk: PublicKey,
    pub from_old_commitment: PedersenCommitment, // 32 bytes commitment of sender bal
    pub from_old_opening: (u64, Scalar),         // (v_from_old, r_from_old) known by sender
    pub to_old_commitment: PedersenCommitment,   // used in transcript; opening unknown to sender
    pub delta_value: u64,                        // Δv
    pub elgamal_pk_for_delta: PublicKey,         // PK to encrypt Δv to (usually receiver's)
    pub rng_seed: [u8; 32],                      // to make proofs deterministic in tests
}

/// Simple API to generate keys (for testing/demo)
pub fn gen_keypair(seed: [u8; 32]) -> (SecretKey, PublicKey) {
    let mut rng = ChaCha20Rng::from_seed(seed);
    let mut sk_bytes = [0u8; 32];
    rng.fill_bytes(&mut sk_bytes);
    let sk = Scalar::from_bytes_mod_order(sk_bytes);
    let pk = (sk * G).compress().to_bytes();
    (SecretKey(sk_bytes.to_vec()), PublicKey(pk.to_vec()))
}

fn decode_pk(pk: &PublicKey) -> Result<RistrettoPoint, ProverError> {
    let arr = <&[u8; 32]>::try_from(pk.0.as_slice()).map_err(|_| ProverError::InvalidInput)?;
    CompressedRistretto(*arr)
        .decompress()
        .ok_or(ProverError::InvalidInput)
}

fn decode_commit(c: &PedersenCommitment) -> Result<RistrettoPoint, ProverError> {
    let arr = <&[u8; 32]>::try_from(c.c.as_slice()).map_err(|_| ProverError::InvalidInput)?;
    CompressedRistretto(*arr)
        .decompress()
        .ok_or(ProverError::InvalidInput)
}

fn scalar_from_u64(x: u64) -> Scalar {
    Scalar::from(x)
}

/// Derive H using hash-to-curve so both sides agree without shipping a second generator.
/// Must match your verifier exactly.
fn pedersen_h() -> RistrettoPoint {
    use sha2::Sha512;
    RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
}

/// Build the same transcript the verifier expects.
fn transcript_for_transfer(
    asset: &[u8],
    from_pk: &RistrettoPoint,
    to_pk: &RistrettoPoint,
    from_old: &RistrettoPoint,
    to_old: &RistrettoPoint,
    ct: &ElGamalCipher,
    delta_c: &RistrettoPoint,
) -> Transcript {
    let mut t = Transcript::new(b"zether.transfer");
    t.append_message(b"ds.chain", b"your-chain-id"); // keep in sync with verifier
    t.append_message(b"ds.pallet", b"pallet-zether");
    t.append_message(b"ds.op", b"transfer.v1");
    t.append_message(b"ds.asset", asset);
    t.append_message(b"from_pk", &from_pk.compress().to_bytes());
    t.append_message(b"to_pk", &to_pk.compress().to_bytes());
    t.append_message(b"from_old", &from_old.compress().to_bytes());
    t.append_message(b"to_old", &to_old.compress().to_bytes());
    t.append_message(b"ct.c1", &ct.c1);
    t.append_message(b"ct.c2", &ct.c2);
    t.append_message(b"delta_c", &delta_c.compress().to_bytes());
    t
}

/// Create the prover’s linkage proof + sender range proof.
/// Returns the on-chain bundle (ΔC, link proof, range proof, and the Δv ElGamal ciphertext).
pub fn prove_transfer(input: &TransferProveInput) -> Result<TransferProofBundle, ProverError> {
    // Decode points
    let from_pk = decode_pk(&input.from_pk)?;
    let to_pk = decode_pk(&input.to_pk)?;
    let elgamal_pk = decode_pk(&input.elgamal_pk_for_delta)?;
    let from_old_c = decode_commit(&input.from_old_commitment)?;
    let to_old_c = decode_commit(&input.to_old_commitment)?;

    // Witness
    let v_from_old = scalar_from_u64(input.from_old_opening.0);
    let r_from_old = input.from_old_opening.1;
    let dv = scalar_from_u64(input.delta_value);

    // Fresh randomness
    let mut rng = StdRng::from_seed(input.rng_seed);
    let k = Scalar::from(rng.next_u64()); // ElGamal randomness
    let rho = Scalar::from(rng.next_u64()); // Pedersen delta randomness for ΔC
    let alpha_k = Scalar::from(rng.next_u64());
    let alpha_v = Scalar::from(rng.next_u64());
    let alpha_r = Scalar::from(rng.next_u64());

    // ElGamal Δv to chosen pk (typically receiver)
    let c1 = (k * G).compress().to_bytes();
    let c2 = (dv * G + k * elgamal_pk).compress().to_bytes();
    let delta_ct = ElGamalCipher { c1, c2 };

    // Pedersen ΔC
    let H = pedersen_h();
    let delta_c = dv * G + rho * H;

    // Chaum–Pedersen commitments
    let a1 = (alpha_k * G).compress().to_bytes();
    let a2 = (alpha_v * G + alpha_k * elgamal_pk).compress().to_bytes();
    let a3 = (alpha_v * G + alpha_r * H).compress().to_bytes();

    // Transcript & challenge
    let mut tr = transcript_for_transfer(
        input.asset_domain,
        &from_pk,
        &to_pk,
        &from_old_c,
        &to_old_c,
        &delta_ct,
        &delta_c,
    );
    tr.append_message(b"a1", &a1);
    tr.append_message(b"a2", &a2);
    tr.append_message(b"a3", &a3);

    let mut chal = [0u8; 64];
    tr.challenge_bytes(b"chal", &mut chal);
    let c = Scalar::from_bytes_mod_order_wide(&chal);

    // Responses
    let z_k = (alpha_k + c * k).to_bytes();
    let z_v = (alpha_v + c * dv).to_bytes();
    let z_r = (alpha_r + c * rho).to_bytes();

    let link = LinkProof {
        a1,
        a2,
        a3,
        z_k,
        z_v,
        z_r,
    };

    // Sender new balance commitment opening (for sender-only RP):
    // from_new = (v_from_old - dv)·G + (r_from_old - rho)·H
    let v_from_new = v_from_old - dv;
    let r_from_new = r_from_old - rho;
    let from_new_c = v_from_new * G + r_from_new * H;

    // Build sender range proof over v_from_new using Bulletproofs (example).
    // You can replace this with your range-proof system of choice.
    let (range_from_new_bytes, _commit_check) = bp_prove_range_u64(
        &mut StdRng::from_seed(input.rng_seed), // deterministic for tests
        v_from_new,
        r_from_new,
    )
    .map_err(|_| ProverError::RangeProof)?;

    Ok(TransferProofBundle {
        delta_comm: delta_c.compress().to_bytes(),
        link,
        range_from_new: range_from_new_bytes,
        range_to_new: Vec::new(), // not provided by sender
        delta_ct,
    })
}

/// Example Bulletproofs prover for a 64-bit range proof.
/// Returns (proof_bytes, commitment_bytes) — we only need the proof; the chain already has the commitment.
fn bp_prove_range_u64<R: RngCore>(
    rng: &mut R,
    value: Scalar,
    blind: Scalar,
) -> Result<(Vec<u8>, [u8; 32]), ProverError> {
    use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
    use curve25519_dalek::ristretto::CompressedRistretto;

    // Convert Scalar value to u64 (we bound it to 64-bit amounts in this example)
    // In prod, ensure amounts are <= 2^64 - 1
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(value.to_bytes().as_slice());
    let v64 = u64::from_le_bytes(bytes[0..8].try_into().unwrap());

    let pg = PedersenGens::default();
    let bp_gens = BulletproofGens::new(64, 1);

    // Dalek’s BP uses its own gens; for compatibility with your on-chain Pedersen,
    // ensure the verifier uses the same commitment scheme OR keep RP separate from on-chain Pedersen.
    let (proof, commit) = RangeProof::prove_single(bp_gens.borrow(), &pg, rng, v64, &blind, 64)
        .map_err(|_| ProverError::RangeProof)?;

    let proof_bytes = proof.to_bytes();
    let commit_bytes: [u8; 32] = commit.to_bytes();
    Ok((proof_bytes, commit_bytes))
}

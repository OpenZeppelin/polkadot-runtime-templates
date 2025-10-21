use clap::Parser;
use hex::{decode, encode};
use zk_elgamal_prover::*;

#[derive(Parser, Debug)]
struct Args {
    /// Hex-encoded from_pk (32B)
    #[arg(long)]
    from_pk: String,
    /// Hex-encoded to_pk (32B)
    #[arg(long)]
    to_pk: String,
    /// Hex-encoded elgamal pk used to encrypt Δv (32B) – usually same as to_pk
    #[arg(long)]
    ct_pk: String,
    /// Hex-encoded from_old_commitment (32B)
    #[arg(long)]
    from_old: String,
    /// Hex-encoded to_old_commitment (32B)
    #[arg(long)]
    to_old: String,
    /// Decimal Δv (u64)
    #[arg(long)]
    amount: u64,
    /// Decimal v_from_old (u64) – for testing
    #[arg(long)]
    v_from_old: u64,
    /// Hex-encoded r_from_old blinding (32B scalar)
    #[arg(long)]
    r_from_old: String,
    /// Hex-encoded 32B RNG seed
    #[arg(
        long,
        default_value = "0000000000000000000000000000000000000000000000000000000000000001"
    )]
    seed: String,
    /// Hex-encoded asset/domain bytes (any length)
    #[arg(long, default_value = "00")]
    asset: String,
}

fn main() -> anyhow::Result<()> {
    let a = Args::parse();

    let from_pk = PublicKey(decode(&a.from_pk)?);
    let to_pk = PublicKey(decode(&a.to_pk)?);
    let ct_pk = PublicKey(decode(&a.ct_pk)?);
    let from_old = PedersenCommitment {
        c: decode(&a.from_old)?.try_into()?,
    };
    let to_old = PedersenCommitment {
        c: decode(&a.to_old)?.try_into()?,
    };
    let mut rbytes = [0u8; 32];
    rbytes.copy_from_slice(&decode(&a.r_from_old)?);
    let r_from_old = curve25519_dalek::scalar::Scalar::from_bytes_mod_order(rbytes);
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&decode(&a.seed)?);

    let input = TransferProveInput {
        asset_domain: &decode(&a.asset)?,
        from_pk,
        to_pk,
        from_old_commitment: from_old,
        from_old_opening: (a.v_from_old, r_from_old),
        to_old_commitment: to_old,
        delta_value: a.amount,
        elgamal_pk_for_delta: ct_pk,
        rng_seed: seed,
    };

    let bundle = prove_transfer(&input)?;
    println!("{}", serde_json::to_string_pretty(&bundle)?);
    Ok(())
}

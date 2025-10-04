use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use once_cell::sync::OnceCell;
use std::hint::black_box;

// --- MW stack (pure Rust): Pedersen commitments + Bulletproofs over Ristretto
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek::ristretto::{CompressedRistretto, RistrettoPoint};
use curve25519_dalek::scalar::Scalar;
use merlin::Transcript;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

// -----------------------------
// One-time global generators
// -----------------------------
static PG: OnceCell<PedersenGens> = OnceCell::new();
static BG64: OnceCell<BulletproofGens> = OnceCell::new();

fn init_gens_once() -> (&'static PedersenGens, &'static BulletproofGens) {
    let pg = PG.get_or_init(PedersenGens::default);
    // One 64-bit party capacity is enough for single-value range proofs.
    let bg = BG64.get_or_init(|| BulletproofGens::new(64, 1));
    (pg, bg)
}

// -----------------------------
// Helpers
// -----------------------------

// Commit to a 64-bit value using Pedersen commitment: C = v*H + r*B
fn mw_commit64(pg: &PedersenGens, v: u64, r: Scalar) -> RistrettoPoint {
    pg.commit(Scalar::from(v), r)
}

// Produce a single-value 64-bit Bulletproof and corresponding *compressed* commitment.
fn mw_prove_64(
    pg: &PedersenGens,
    bg: &BulletproofGens,
    v: u64,
    mut rng: ChaCha20Rng,
) -> (RangeProof, CompressedRistretto) {
    let blinding = Scalar::random(&mut rng);
    let mut transcript = Transcript::new(b"bench_mw_rangeproof_64");
    // (RangeProof, CompressedRistretto)
    RangeProof::prove_single(bg, pg, &mut transcript, v, &blinding, 64).expect("prove_single")
}

// Verify a 64-bit Bulletproof for a single *compressed* commitment.
fn mw_verify_64(
    pg: &PedersenGens,
    bg: &BulletproofGens,
    proof: &RangeProof,
    com: &CompressedRistretto,
) -> bool {
    let mut transcript = Transcript::new(b"bench_mw_rangeproof_64");
    proof
        .verify_single(bg, pg, &mut transcript, com, 64)
        .is_ok()
}

// Deterministic RNG per iteration (so each sample is reproducible but independent).
fn iter_rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

// -----------------------------
// Criterion benches
// -----------------------------
fn bench_mw_vs_u128(c: &mut Criterion) {
    let (pg, bg) = init_gens_once();

    let v_a: u64 = 1_000_000;
    let v_b: u64 = 777;

    // Precompute deterministic commitments for add/sub benches (so we only time group ops there).
    let mut rng_fix = iter_rng(42);
    let com_a = mw_commit64(pg, v_a, Scalar::random(&mut rng_fix));
    let com_b = mw_commit64(pg, v_b, Scalar::random(&mut rng_fix));

    // Precompute a proof+commitment pair for the verify bench.
    let (proof64, com64) = mw_prove_64(pg, bg, v_a, iter_rng(7));

    // --- MW: commit (one op) ---
    c.bench_function("mw_commit64 (one op)", |bch| {
        bch.iter_batched(
            || iter_rng(123456789), // per-iter fresh RNG
            |mut rng| {
                let r = Scalar::random(&mut rng);
                let cmt = mw_commit64(pg, v_a, r);
                black_box(cmt)
            },
            BatchSize::SmallInput,
        )
    });

    // --- MW: add/sub on commitments (one group op) ---
    c.bench_function("mw_commit_add (one op)", |bch| {
        bch.iter_batched(
            || (com_a, com_b),
            |(x, y)| black_box(x + y),
            BatchSize::SmallInput,
        )
    });

    c.bench_function("mw_commit_sub (one op)", |bch| {
        bch.iter_batched(
            || (com_a, com_b),
            |(x, y)| black_box(x - y),
            BatchSize::SmallInput,
        )
    });

    // --- MW: Bulletproofs single-value 64-bit prove ---
    c.bench_function("mw_range_prove_64 (one proof)", |bch| {
        bch.iter_batched(
            || iter_rng(9999), // fresh RNG for randomness per sample
            |rng| {
                let (proof, com) = mw_prove_64(pg, bg, v_b, rng);
                black_box((proof, com))
            },
            BatchSize::SmallInput,
        )
    });

    // --- MW: Bulletproofs single-value 64-bit verify ---
    c.bench_function("mw_range_verify_64 (one verify)", |bch| {
        bch.iter_batched(
            || (proof64.clone(), com64),
            |(p, cmt)| {
                let ok = mw_verify_64(pg, bg, &p, &cmt);
                black_box(ok)
            },
            BatchSize::SmallInput,
        )
    });

    // --- Plain u128 baselines (match the TFHE file) ---
    c.bench_function("u128_add (1000 iters)", |bch| {
        bch.iter(|| {
            let mut x: u128 = 0;
            for _ in 0..1000 {
                x = x.wrapping_add(black_box(777));
            }
            black_box(x)
        })
    });

    c.bench_function("u128_sub (1000 iters)", |bch| {
        bch.iter(|| {
            let mut x: u128 = 1_000_000;
            for _ in 0..1000 {
                x = x.wrapping_sub(black_box(1));
            }
            black_box(x)
        })
    });
}

criterion_group!(benches, bench_mw_vs_u128);
criterion_main!(benches);

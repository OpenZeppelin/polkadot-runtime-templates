use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use once_cell::sync::OnceCell;
use std::hint::black_box;
use tfhe::{
    generate_keys, prelude::*, set_server_key, ClientKey, ConfigBuilder, FheBool, FheUint128,
};

// One-time key init (per process).
static CK: OnceCell<ClientKey> = OnceCell::new();

fn init_keys_once() -> &'static ClientKey {
    CK.get_or_init(|| {
        // Default config supports FheUint128 + FheBool in high-level API.
        let config = ConfigBuilder::default().build();
        let (ck, sk) = generate_keys(config);
        // IMPORTANT: set by value (consumes sk), not by &sk
        set_server_key(sk);
        ck
    })
}

// Helpers to make encrypted fixtures.
fn enc_u128(x: u128) -> FheUint128 {
    let ck = init_keys_once();
    FheUint128::encrypt(x, ck)
}
fn enc_bool(b: bool) -> FheBool {
    let ck = init_keys_once();
    FheBool::encrypt(b, ck)
}

fn bench_fhe_vs_u128(c: &mut Criterion) {
    // Pre-encrypt fixed operands once (outside the timed region).
    let a = enc_u128(1_000_000);
    let b = enc_u128(777);
    let c_true = enc_bool(true);

    // --- FHE: add ---
    c.bench_function("fhe_add (one op)", |bch| {
        bch.iter_batched(
            || (a.clone(), b.clone()),
            |(x, y)| {
                let sum = &x + &y;
                black_box(sum)
            },
            BatchSize::SmallInput,
        )
    });

    // --- FHE: sub ---
    c.bench_function("fhe_sub (one op)", |bch| {
        bch.iter_batched(
            || (a.clone(), b.clone()),
            |(x, y)| {
                let diff = &x - &y;
                black_box(diff)
            },
            BatchSize::SmallInput,
        )
    });

    // --- FHE: select (cond ? x : y) ---
    c.bench_function("fhe_select_true (one op)", |bch| {
        bch.iter_batched(
            || (c_true.clone(), a.clone(), b.clone()),
            |(cond, x, y)| {
                let out = cond.if_then_else(&x, &y);
                black_box(out)
            },
            BatchSize::SmallInput,
        )
    });

    // --- Plain u128 baselines ---
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

criterion_group!(benches, bench_fhe_vs_u128);
criterion_main!(benches);

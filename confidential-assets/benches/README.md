# Benchmarking Cryptographic Backends for Confidential Arithmetic

These results indicate Mimble-Wimble(MW) style range proofs may be less expensive than FHE as a compute backend for on-chain confidential payment implementations.

Summary of Results:
- FHE integer add/sub/select: basically one op burns ~1/6 of an entire parachain block by itself.
- MW range proof: ~0.1% of blockspace → ~1000 proofs per block.
- MW range verify: ~0.015% → ~6600 verifies per block.
- MW commit ops: negligible, you could pack millions per block.
- u128 ops: effectively free in block terms.

This aligns with intuition: FHE is orders of magnitude heavier (seconds), MW range proofs are moderately heavy (ms), and pure arithmetic is trivial.

| Operation          | Time (s)  | ps/op     | % of 6s block |
| ------------------ | --------- | --------- | ------------- |
| FHE add/sub/select | ~1.0 s    | 1.0e12 ps | **16.7%**     |
| MW commit64        | 3.2e-5 s  | 3.2e7 ps  | 0.00053%      |
| MW commit_add/sub  | 1.7e-7 s  | 1.7e5 ps  | 0.0000029%    |
| MW range_prove_64  | 6.15e-3 s | 6.15e9 ps | 0.1025%       |
| MW range_verify_64 | 8.7e-4 s  | 8.7e8 ps  | 0.0145%       |
| u128 add/sub       | 4.8e-7 s  | 4.8e5 ps  | 0.000008%     |

## Run Yourself

Run `fhe_vs_u128` benchmark:
```
cargo bench --bench fhe_vs_u128 -- \
  --sample-size 10 \
  --warm-up-time 1 \
  --measurement-time 20
```

Run `mw_vs_u128` benchmark:
```
cargo bench --bench mw_vs_u128
```
# Optimization Notes

**Low(est) priority until single encrypted transfer verified on-chain is implemented securely.**

## Get Faster On-Chain Verification for Slower Off-Chain Prover Times
- encode `zkhe-verifier` into arithmetic circuits and prove verification off-chain using Halo2 such that chain only verifies Halo2 proof (ZK Proof-of-Proof)
- the ZK Proof of Proof is better for proof aggregation AFAIU (TODO: how does proof aggregation scale aggregated proof size and verification time vs standard proof aggregation of ZK ElGamal proofs)

## Premature Low-Level Optimizations
> PRIORITY <<<< SECURITY

Most of these assume batching which is not implemented for any existing CT implementation. It is a significant extension of the current design and should not be prioritized until the existing single encrypted transfer with on-chain verification is stable and deemed to be sufficiently secure.

### Batch everything you can
Bulletproofs: verify N range proofs together with randomized batching (one transcript, combine IPA checks). This replaces N MSMs with ~1 larger MSM.
Per-block aggregator: let extrinsics enqueue proof items; run a single on_finalize batch verify with a bounded cap (e.g., “verify up to 64 proofs”). If the block is full, spillover to the next block.
Batch inversions: where your verifier needs multiple scalar inversions, use Montgomery’s trick to do 1 inversion + O(n) mults.

### Use variable-time math for public data
On-chain verification manipulates public inputs only—so use variable-time scalar muls/MSMs safely.
In curve25519-dalek, prefer:
RistrettoPoint::vartime_double_scalar_mul_basepoint
RistrettoPoint::vartime_multiscalar_mul
These are noticeably faster in Wasm than constant-time versions.

### Exploit fixed bases
Pedersen generators (G, H, H_i…) and any fixed basepoints should use fixed-base scalarmul with precomputed tables.
Generate wNAF tables at build time and ship them as const arrays; never rebuild at runtime.

### Reduce proof size / work
Aggregated range proofs drastically reduce per-proof verifier work.
Choose the smallest bit range that satisfies your asset economics (e.g., 32 or 48 bits instead of 64 if you can).
Tighten transcripts
Keep one Merlin transcript per batch, not per proof, when the soundness story allows.
Avoid re-encoding points/labels repeatedly; write a thin “no-alloc” transcript helper with stack buffers.

### Wasm / toolchain knobs
Compile for Wasm SIMD
Build runtime with -C target-feature=+simd128 and ensure the chain enables Wasm SIMD. It often gives 1.2–1.6× on field ops.
Pick the right dalek backend for Wasm
On wasm32, the u32 or fiat backends are typically faster than generic u64 emulation. Benchmark both; lock the faster one via features.
LTO + size-opt tradeoff
Use lto = "fat" for your runtime crate and set codegen-units = 1; even in Wasm this can shave cycles in tight loops.

### Avoid heap churn
No Vec growth in hot paths; pre-size all buffers. Use stack arrays and arrayvec for transcript I/O and point lists.
Host hashing where possible
For commitments/transcripts that allow it, use sp_io::hashing::blake2_256/blake2_128 rather than a pure-Rust BLAKE inside Wasm.

### Curve / representation choices
Hardcode generators
Ship canonical bytes for G, H, H_i as const [u8;32] and decompress once per block (or cache in a lazy static guarded by block number).
Compressed I/O only
Accept compressed points on extrinsics; reject early if decompression fails; never serialize/unserialize more than once.
MSM (multi-scalar mul) focus
Use MSM everywhere
Rewrite verifier checks (both ElGamal consistency and IPA verifier) into a single MSM per proof (or per batch).
Right window size
For Wasm backends, smaller windows often win (e.g., 4–5 bits) vs native. Benchmark Pippenger/Straus window sizes and pin them.

### Bulletproofs-specific
One IPA, many ranges
Verify aggregated range proofs (k proofs of size n bits) = one IPA with vectors of length k * n. Big MSM once, fewer rounds.
Minimize transcript traffic
Feed points/scalars in the order the paper requires without additional labels; Merlin work dominates more than you expect in Wasm.
Bounded aggregation buckets
Choose batch sizes tied to your block weights (e.g., 8, 16, 32). Expose a WeightToFee curve that nudges users toward batching.

### ZK-ElGamal-specific
Vartime double-mul forms
Rearrange your verification equations so basepoint terms hit vartime_double_scalar_mul_basepoint paths (fastest code paths in dalek).
Pedersen precompute
If your ciphertext check has many a_i * H_i, store H_i in precomputed tables and feed scalars as slices into MSM.
Avoid re-randomization when unnecessary
Some “defensive re-binding” patterns cost transcript and MSM time; keep the minimal bind set for domain separation.

### Pallet architecture patterns
Verify-later extrinsics
Users submit (ct, proof) into a queue (cheap). A privileged or incentivized “verifier” extrinsic performs the batch verification and finalizes state. This keeps worst-case costs off user calls and unlocks batching.
Bounded queues + fairness
Use a ring buffer; process up to MAX_VERIFIES_PER_BLOCK. Emit events for leftovers so indexers/users can resubmit if urgent.
Deterministic weights
Proof length is fixed; keep weight linear in the number of proofs in batch, not their content. Benchmark to derive constants.
Storage & PoV hygiene
Zero storage on failure
Never write intermediate state before verification passes. Keeps PoV minimal and avoids PoV bloat under spam.
Don’t persist proofs
Accept proofs as call args; verify; store only commitments/nullifiers. If you must persist, keep it off-chain (indexer).
If you’re open to “bigger” changes
Wrap with a succinct proof
Verify Bulletproofs off-chain; submit a tiny Groth16/Plonk proof that “I verified these N Bulletproofs correctly.” On-chain cost becomes a few pairings. This is a larger engineering lift, but it’s the largest speedup.
Switch the hash inside Bulletproofs
A Bulletproofs variant with a lighter permutation (e.g., Rescue/Blake2s) in transcript could help in Wasm. Requires prover changes too.

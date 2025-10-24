# TODO

1. Impl
2. Bench
3. Docs

## Impl TODO

- get bulletproofs to pass => full on-chain verification passes on happy path
- more tests of the prover/verifier functionality
- maximize size of zkhe-primitives to minimize non-overlapping helper surface area used in zkhe prover and verifier

## Bench TODO
1. benchmarks per pallet using production configured runtime
2. micro-benchmark for pure crypto verification in no_std `--target=wasm32-unknown-unknown`

Micro-benchmarks for sigma and bulletproof verification which varies:
- dalek backend: u32 vs fiat
- SIMD on/off
Use the results to determine the best cargo features for the prover and verifier and pin them in both.

## Docs TODO

TODO Update to match latest code refactor outlined above
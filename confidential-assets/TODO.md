# TODO

1. Impl
2. Bench
3. Docs

## Impl TODO

### E2E solution mirroring Solana CT backend as is
- update pallets
- runtime tests
- node tests
- increase prover + verifier test coverage in general

### HIGH: detail and explain accept_pending UX issue with prospective solutions
- should be a way ideally to include the accept_pending in the send so accept_pending does NOT add an additional step (UX)

Add sigma proof for verify accept pending path:
- add sigma proof to verify_transfer_received which may need to be renamed to verify_accept_pending if it is used that way

### LOW
- more tests of the prover/verifier functionality
- configurable NetworkId pallet config (move out of hardcoded)
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
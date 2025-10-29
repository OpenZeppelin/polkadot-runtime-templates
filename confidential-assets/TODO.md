# TODO

## Impl TODOs In Progress

### Now

- pallet-confidential-xcm-bridge
- demo using xcm-simulator with runtimes running in CLI

### Next
- extract acl pallet for ERC 7984
- ERC 7984: swaps between confidential <> regular && confidential <> confidential
- configurable NetworkId pallet config (move out of hardcoded)

### Future
- Frontier precompile
- PolkaVM precompile
- Per precompile: Optional receiver hook (like IERC7984Receiver.onConfidentialTransferReceived)
- rampv1 with privacy (using merkle trees + batched execution)
- impl view keys like Solana for read-only actions (auditor key)
- pay for transaction fee design with confidential asset
- pay for gas fee with confidential asset
- more tests of the prover/verifier functionality
- maximize size of zkhe-primitives to minimize non-overlapping helper surface area used in zkhe prover and verifier


## Bench TODOs
1. benchmarks per pallet using production configured runtime
2. micro-benchmark for pure crypto verification in no_std `--target=wasm32-unknown-unknown`

Micro-benchmarks for sigma and bulletproof verification which varies:
- dalek backend: u32 vs fiat
- SIMD on/off
Use the results to determine the best cargo features for the prover and verifier and pin them in both.

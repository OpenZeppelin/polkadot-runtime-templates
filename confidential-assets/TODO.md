# TODO

## Impl TODOs In Progress

### Current TODO
- pallet-confidential-xcm-bridge
- demo using xcm-simulator with runtimes running in CLI

### Remaining TODO for Exact ERC 7984 Parity
- configurable NetworkId pallet config (move out of hardcoded)
- Frontier precompile
- Per precompile: Optional receiver hook (like IERC7984Receiver.onConfidentialTransferReceived)


### Next Steps
- include operators, acl types for confidential-swaps pallet

- PolkaVM precompile
- impl view keys like Solana for read-only actions (auditor key)
- pay for transaction fee design with confidential asset
- pay for gas fee with confidential asset
- rampv1 with privacy (using merkle trees + batched execution)

## Bench TODOs
1. benchmarks per pallet using production configured runtime
2. micro-benchmark for pure crypto verification in no_std `--target=wasm32-unknown-unknown`

Micro-benchmarks for sigma and bulletproof verification which varies:
- dalek backend: u32 vs fiat
- SIMD on/off
Use the results to determine the best cargo features for the prover and verifier and pin them in both.

# TODO

### Current TODO
- pallet-confidential-xcm-bridge
- configurable NetworkId pallet config (move out of hardcoded)
- demo
- runtime
- node with zkhe-prover extension configured
- run benchmarks -> generate and update default weights
- update docs and include usage disclaimer until audited

### precompiles
- Frontier precompile
- Per precompile: Optional receiver hook (like IERC7984Receiver.onConfidentialTransferReceived)
- PolkaVM precompile

### confidential-assets improvements
- impl view keys like Solana for read-only actions (auditor key)
- pay for transaction fee design with confidential asset
- pay for gas fee with confidential asset
- batch confidential transfers verified off-chain (wrap in Halo2)
- rampv1 with privacy (batched execution using merkle tree proofs verified on-chain)

### confidential-swaps pallet
- include operators, acl types for confidential-swaps pallet
- batched execution using merkle tree proofs verified on-chain

### confidential-xcm-bridge improvements

- Refund Appendix: add RefundSurplus + DepositAsset(sovereign) after BuyExecution.
- Fee discovery: store fee_per_second and simple per-dest “extra weight” like Moonbeam’s TransactInfoWithWeightLimit.
- Signed variant: prepend DescendOrigin and map user’s AccountId → Location.
- Derivative: wrap inner call with utility::as_derivative(index, call) and use a derivative account model.

## Bench TODOs
1. benchmarks per pallet using production configured runtime
2. micro-benchmark for pure crypto verification in no_std `--target=wasm32-unknown-unknown`

Micro-benchmarks for sigma and bulletproof verification which varies:
- dalek backend: u32 vs fiat
- SIMD on/off
Use the results to determine the best cargo features for the prover and verifier and pin them in both.

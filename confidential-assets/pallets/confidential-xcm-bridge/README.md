# Confidential XCM Bridge

### Useful Features Out of Scope for V0

- Refund Appendix: add RefundSurplus + DepositAsset(sovereign) after BuyExecution.
- Fee discovery: store fee_per_second and simple per-dest “extra weight” like Moonbeam’s TransactInfoWithWeightLimit.
- Signed variant: prepend DescendOrigin and map user’s AccountId → Location.
- Derivative: wrap inner call with utility::as_derivative(index, call) and use a derivative account model.
# Polkadot Confidential Assets

Confidential Assets Overview:
```
\pallets
	\zkhe:
		ZK proof authenticated storage of encrypted balances verified on-chain
	\confidential-assets:
		confidential multi-assets pallet supporting encrypted transfers
\primitives
	\zkhe:
		shared primitives between zkhe prover and verifier crates
	\confidential-assets:
		shared primitives between decoupled, interoperable pallets
\zkhe-prover
	off-chain client library to construct ZK proofs of encrypted transfers
\zkhe-verifier
	no_std library to verify ZK proofs on-chain
```

- `pallet-confidential-assets` public interface follows the Confidential Transfers Standard (ERC 7984) by OpenZeppelin
- `pallet-confidential-assets` has a generic backend for its cryptography
	- generic backend is assigned at runtime to `Zkhe`, runtime instance of `pallet_zkhe` representing the runtime's implementation of `pallet_zkhe::Config`
- `pallet-zkhe` has a generic verifier for its cryptography
	- generic verifier assigned at runtime to `zkhe_verifier::ZkheVerifier`

## Documentation

NOT updated since latest code refactor:
* [Polkadot Confidential Assets Design Overview](./DESIGN.md)
* [Notes for Reviewers](./AUDIT.md)
* [Optimization Notes](./NEXT.md)

## References

* [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol)
* [Solana Confidential Balances Overview](https://www.solana-program.com/docs/confidential-balances/overview)

## Extensions

The following modules represent future extensions of the confidential framework. They are listed here for repository scope planning and naming consistency only.
	•	pallet-confidential-airdrop
	•	pallet-confidential-dex
	•	pallet-confidential-perp-dex
	•	pallet-confidential-nfts
	•	pallet-confidential-tickets
	•	pallet-confidential-auction
	•	pallet-confidential-voting

Primitive cryptographic pallets supporting these extensions may include but are not limited to:
	•	pallet-merkle-tree
	•	pallet-kite-vote
	•	pallet-kite-snapshot

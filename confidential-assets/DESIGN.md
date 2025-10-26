# Confidential Assets Design Trade-offs and Rationale

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

## Choice of ZK-ElGamal Backend

The ZK-ElGamal scheme was selected for its close alignment with the ERC-7984 Confidential Contracts standard and its proven use in Solana’s Confidential Token implementation.

Advantages
	•	Address transparency: Sender and receiver remain visible, satisfying regulatory and interoperability constraints while keeping transfer amounts confidential.
	•	Verifiable execution: Zero-knowledge proofs guarantee that encrypted transfers preserve balance correctness without revealing plaintext amounts.
	•	Account model compatibility: Works naturally with Substrate’s account-based state and existing asset registries, avoiding UTXO complexity.
	•	Extensibility: Better suited for composable systems, account abstraction, and future programmable privacy use cases compared to MimbleWimble-style designs.
	•	Proven production precedent: Mirrors Solana’s Confidential Token architecture, leveraging the same twisted ElGamal + Pedersen commitment combination for efficient on-chain verification.

Compared to MimbleWimble
	•	MimbleWimble hides sender and receiver information, making it unsuitable for general-purpose on-chain use or composable smart contract environments.
	•	MimbleWimble’s cut-through model favors off-chain transaction aggregation rather than real-time execution and event emission expected in ERC-compatible systems.
	•	ZK-ElGamal provides deterministic, verifiable encrypted transactions that integrate smoothly with runtime logic, governance, and cross-chain standards.

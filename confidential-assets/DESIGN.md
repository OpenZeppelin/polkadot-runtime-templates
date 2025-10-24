# Confidential Assets Design

TODO Update since most recent refactor especially the Impl Risks and Mitigations section at the bottom.

# Design Trade-offs and Rationale

Choice of ZK-ElGamal Backend

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

# Implementation Risks and Mitigations

Risk	Description	Mitigation
Cryptographic correctness	Incorrect elliptic-curve definitions, domain parameters, or point decompression logic could cause consensus divergence or asset loss.	Seek early review from OpenZeppelin internal experts and external cryptographers; reuse proven libraries (e.g., curve25519-dalek); add extensive property tests.
no_std verification bugs	Limited runtime debugging and serialization complexity may cause subtle verification mismatches.	Maintain symmetry tests between zk-elgamal-prover and pallet-zk-elgamal-verifier; integrate CI pipelines for cross-checking proof/verification pairs.
Proof size and weight overhead	On-chain verification may increase block weight and storage usage.	Use benchmark-driven optimization; parameterize proof size limits via runtime constants.
Cross-pallet coupling	Tight coupling between pallet-confidential-assets and pallet-zk-elgamal could complicate upgrades.	Maintain backend abstraction traits; use feature gating and versioned trait interfaces.

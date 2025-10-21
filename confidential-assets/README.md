# Confidential Assets

Rust crates used to implement confidential assets on Polkadot.

- [`pallet-zk-elgamal`](./pallet-zk-elgamal): implements an on-chain cryptographic backend for confidential transfers mirroring the [Solana Confidential Token implementation](https://www.solana-program.com/docs/confidential-balances/overview).
- [`pallet-zk-elgamal-verifier`](./pallet-zk-elgamal-verifier/) implements ZK verification and is assigned at runtime to pallet_zk_elgamal::Config::ZKVerifier.
- [`pallet-confidential-assets`](./pallet-confidential-assets/): implements the [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol) for multiple assets.
- [`ca-primitives`](./ca-primitives/): types and traits shared between the decoupled pallets described above.

## Prover

- `client-zk-elgamal-prover`: client library intended for usage by the node to generate ZK proofs that pass verification checks enforced by `pallet-zk-elgamal-verifier`.

## Precompiles

The precompile interfaces follow the [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol).

- `frontier-precompile-confidential-assets`
- `polkavm-precompile-confidential-assets`

## References

* [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol)
* [Solana Confidential Balances Overview](https://www.solana-program.com/docs/confidential-balances/overview)


## Notes for Auditors

*This section is a WIP and the code is not yet ready for auditor review. It will be updated before then to reflect the latest state of the code.*

Please closely review the following Rust crates:
- pallet-zk-elgamal-verifier: curve type defs, expected proof format, verification algorithm
- pallet-zk-elgamal: authenticated and verified storage for zk-elgamal processing
- zk-elgamal-prover: proof generation matches verifier expectations

The construction of Merlin transcripts in particular need to be closely reviewed (TODO link to code sections once stable).

*In addition, Solana did have some bug/issue related to curve choice or something like that when its Confidential Token standard was initially launched so ideally we review all fixes that have been identified and applied there to closely ensure that we do not repeat any mistakes that have already been made.*
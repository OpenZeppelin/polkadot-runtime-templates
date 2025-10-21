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
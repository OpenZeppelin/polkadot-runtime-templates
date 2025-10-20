# Confidential Assets

Primitives, pallets, and precompiles used to implement confidential assets on Polkadot.

## Pallets

- [`pallet-zether`](./pallet-zether): implements an on-chain cryptographic backend for confidential transfers
- [`pallet-confidential-assets`](./pallet-confidential-assets/): implements and exposes the [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol) for multiple assets
- [`ca-primitives`](./ca-primitives/): types and traits shared between the decoupled pallets described above

## Precompiles

The precompile interfaces follow the [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol).

- `frontier-precompile-confidential-assets`
- `polkavm-precompile-confidential-assets`

## Research

* [Research benchmarking cryptographic backends for confidential assets](https://github.com/4meta5/polkadot-confidential-assets-backends)
* [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol)
# Confidential Assets

Primitives, pallets, and precompiles used to implement confidential assets on Polkadot.

## Pallets

- [`pallet-multi-utxo`](./pallets/pallet-multi-utxo/)
- [`pallet-mimblewimble`](./pallets/pallet-mimblewimble/)
- [`pallet-confidential-assets`](./pallets/pallet-confidential-assets/)

## Precompiles

The precompile interfaces follow the [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol).

- [`evm-precompile-confidential-assets`](./precompiles/evm-precompile-confidential-assets/)
- [`pvm-precompile-confidential-assets`](./precompiles/pvm-precompile-confidential-assets/)

## Research

* [Research benchmarking cryptographic backends for confidential assets](https://github.com/4meta5/polkadot-confidential-assets-backends)
* [OpenZeppelin Confidential Contracts Standard](https://github.com/OpenZeppelin/openzeppelin-confidential-contracts/blob/master/contracts/interfaces/IERC7984.sol)
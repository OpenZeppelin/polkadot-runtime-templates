# OpenZeppelin Runtime Templates for Substrate

[Polkadot SDK](https://github.com/paritytech/polkadot-sdk) has a steep learning curve. In order to make it easier for newcomers to get started, OpenZeppelin is providing runtime templates that can be used to quickly bootstrap a [Substrate](https://github.com/paritytech/polkadot-sdk/tree/master/substrate) project using Polkadot SDK.

Along with the templates, OpenZeppelin also provides a [documentation website](https://docs.openzeppelin.com/substrate-runtimes) that explains the templates and the details.

## About this repository and templates

This repository contains all of our templates. Each template has its own sub-directory, is completely standalone, and can be used independently.

For example: if you want to use our `generic runtime template`, you can just copy that subdirectory, and it will work on its own. You don't need anything from other templates nor from the root directory of this repository for your selected template to work.

---

> [!WARNING]
> Avoid using `main` for production deployments and use release branches instead. `main` is a development branch that should be avoided in favor of tagged releases. The release process involves security measures that the `main` branch does not guarantee.

### Generic Runtime Template

This template has all the basic features you expect to find on a typical L1 blockchain or parachain. Basic, yet preserving the most important pallets that are used in the Polkadot ecosystem today and a safe runtime base configuration.
You can find a full list of the pallets included in this template in our [docs](https://docs.openzeppelin.com/substrate-runtimes/runtimes/generic)

### EVM template

This template uses [frontier](https://github.com/polkadot-evm/frontier) pallets and has EVM compatibility out of the box. You can migrate your solidity contracts or EVM compatible dapps easily to your parachain using this template. Here are some of the key features included:

- 20 byte addresses: Existing tooling works out of the box, no more awkward conversion, this template handles that for you.
- Account Abstraction support: The Entrypoint contract is included as part of the pre-deployed contracts that will be in the genesis block.
- Extensible pre-deployed contracts: In addition to the entrypoint, you can add your own smart contract to be included in the genesis block.

For a step by step guide on how to deploy this to your own local environment using Zombienet check this [tutorial](https://docs.openzeppelin.com/substrate-runtimes/guides/testing_with_zombienet)

### How to use

Please refer to our docs for a `quick start` [guide](https://docs.openzeppelin.com/substrate-runtimes/guides/quick_start).

## Security

Past audits can be found in `/audits` directory in the respective template's directory.

Refer to [SECURITY.md](SECURITY.md) for more details.

## License

OpenZeppelin Runtime Templates are released under the [GNU v3 License](LICENSE).

# OpenZeppelin Runtime Templates for Substrate

Substrate has a steep learning curve. In order to make it easier for newcomers to get started, OpenZeppelin is providing runtime templates that can be used to quickly bootstrap a Substrate project.

Along with the templates, OpenZeppelin also provides a [documentation website](https://docs.openzeppelin.com/substrate-runtimes) that explains the templates and the details.

## About this repository and templates

This repository contains all of our templates. Each template has its own sub-directory, is completely standalone, and can be used independently.

For example: if you want to use our `generic runtime template`, you can just copy that subdirectory, and it will work on its own. You don't need anything from other templates nor from the root directory of this repository for your selected template to work.

---
> [!WARNING]
> Avoid using `main` for production deployments and use release branches instead. `main` is a development branch that should be avoided in favor of tagged releases. The release process involves security measures that the `main` branch does not guarantee.
### Generic Runtime Template

This template has all the basic features you expect to find on a typical L1 blockchain or parachain. Basic, yet preserving the most important pallets that are used in the Polkadot ecosystem today and a safe runtime base configuration.
You can find a full list of the pallets included in this template in our [docs](https://docs.openzeppelin.com/substrate-runtimes/1.0.0/)


### EVM template

> [!WARNING]
> EVM template is still under development.

This template has evm compatibility out of the box. You can migrate your solidity contracts or evm compatible dapps easily to your blockchain using this template.


How it compares to:
- frontier:
- moonbeam:
- acala:


### How to use

Please refer to our docs for a `quick start` [guide](https://docs.openzeppelin.com/substrate-runtimes/):

## Security

Past audits can be found in `/audits` directory in the respective template's directory.

Refer to [SECURITY.md](SECURITY.md) for more details.

## License

OpenZeppelin Runtime Templates are released under the [GNU v3 License](LICENSE).











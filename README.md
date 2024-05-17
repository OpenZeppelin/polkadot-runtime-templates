# OpenZeppelin Runtime Templates for Substrate

Substrate has a steep learning curve. In order to make it easier for newcomers to get started, OpenZeppelin is providing runtime templates that can be used to quickly bootstrap a Substrate project.

Along with the templates, OpenZeppelin also provides a documentation website that explains the templates and the niche details.

## About this repository and templates

This is a monorepo, listing all of our templates. Each template has its own sub-directory, and completely independent from other templates.

For example: if you want to use our `generic runtime template`, you can just copy that subdirectory, and it will work on its own. You don't need anything from other templates nor from the root directory of this repository for your selected template to work.

---

### Generic Runtime Template

This template has all the key features you expect to find on a typical L1 blockchain. Basic, yet comprehensive pallet set and runtime configuration.


### EVM template

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











# Zombienet configuration

Zombienet aims to be a testing framework for Substrate based blockchains, providing a simple cli tool that allows users to spawn and test ephemeral networks.

## Start a development chain

Firstly build Polkadot binaries with:

```sh
$ scripts/zombienet.sh build
```

This process can take some time, so please be patient. If on Linux, you can alternatively download the binaries to speed up the process with:

```shell
$ scripts/zombinet.sh init
```

Once Polkadot binaries are in place you can spawn a local testnet by running the following command:

```shell
$ scripts/zombienet.sh devnet
```

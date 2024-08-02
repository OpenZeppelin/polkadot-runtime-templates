# Zombienet Configuration

Zombienet configuration can be used by:
- CLI to spawn and test ephemeral networks.
- `zombienet-sdk` to write programmatic integration tests.


You can test/play with ephemeral networks by following the below steps:


1. Build Polkadot binaries with:

    ```sh
    $ scripts/zombienet.sh build
    ```

   - This process can take some time, so please be patient. If on Linux, you can alternatively download the binaries to speed up the process with:

        ```shell
        $ scripts/zombienet.sh init
        ```

2. Once Polkadot binaries are in place you can spawn a local testnet by running the following command:

    ```shell
    $ scripts/zombienet.sh devnet
    ```

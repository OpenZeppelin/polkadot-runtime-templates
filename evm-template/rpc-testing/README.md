## RPC Testing Toolkit

This is a Postman collection to test your the RPC deployment of you smart contract. To use it you should prepare your environment and test toolkit for testing:

* Open the collection in Postman, and open the variables in it.
* Fill the `url` with HTTP RPC url and `chain_id` with your chain's chain id.
* Obtain three funded accounts. You may provide them through the chainspec. 
  * Put first address to the `alice_addr` variable and their balance to the `alice_balance`.
  * Make a transfer transaction from the second address to any address.
    * Put its hash to the `tx_hash` variable
    * Put the block number of this transaction to `tx_block_number` and `block_number`. Put the number of transactions in this block (highly likely it will be 1) to the `block_tx_count`.
    * Put the hash of the block with this transaction to `tx_block_hash` and `block_hash`.
    * Put the index of the transaction in the block to `tx_index`. Highly likely it will be 0.
    * Put the address you have sent the transaction from to `tx_from` variable.
  * Take a third address and deploy the smart contract `TestContract.sol` from it.
    * Put the address of the deployed contract to the `test_contract` variable.
    * Put the code from the compiled contract to the `contract_code` variable. It can be found in a resulting JSON file under `deployedBytecode` key.

When you complete these preparation steps, run the requests from the collection. If some of them are not passing, run them as a single request and see what the request have returned to debug it.
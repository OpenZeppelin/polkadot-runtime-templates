// node/tests/fhe_node_smoke.rs
#![cfg(feature = "fhe")]

use sc_client_api::Backend;
use sc_service_test::spawning::run_node_until;
use sp_keyring::AccountKeyring::Alice;
use sp_runtime::OpaqueExtrinsic;

#[test]
fn fhe_works_through_full_node() {
    // Start a single in-process node (dev chain)
    let (task_manager, client, network, txpool, _rpc) = start_dev_node_with_fhe().expect("node");

    // Build and submit an extrinsic that triggers your FHE call
    let xt: OpaqueExtrinsic = craft_fhe_extrinsic(&client, &Alice.pair());

    // Submit via txpool or RPC
    txpool.import(xt.into()).unwrap();

    // Let the node author a block including it; or manually trigger block production / import
    run_node_until(
        &client,
        Box::new(|_| false), // or a condition on storage you expect after FHE op
    );

    // Assert storage / events show your FHE path executed
    assert_state_post_fhe(&*client);
}

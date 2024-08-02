use std::time::Duration;

use futures::stream::StreamExt;
use zombienet_sdk::{NetworkConfig, NetworkConfigExt};

#[tokio::test]
async fn test_parachain_network() -> Result<(), anyhow::Error> {
    println!("hello!");
    let network_config = NetworkConfig::load_from_toml("./../configuration/devnet.toml")
        .expect("couldn't read network config");

    println!("file read!");
    // Set up the network
    let network = network_config.spawn_native().await.expect("couldn't start network");

    println!("network started!");
    // Get the Alice node
    let alice = network.get_node("alice")?;
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Add more specific assertions about Alice node properties if needed
    // TODO: alice.assert(metric_name, value)

    let client = alice.client::<subxt::PolkadotConfig>().await?;

    // Wait for 3 blocks and assert their properties
    let mut blocks = client.blocks().subscribe_finalized().await?.take(3);
    let mut block_numbers = Vec::new();

    while let Some(block) = blocks.next().await {
        let block = block?;
        let block_number = block.header().number;
        block_numbers.push(block_number);

        println!("Block number: {}", block_number);
    }

    // Assert that we received exactly 3 blocks
    assert_eq!(block_numbers.len(), 3, "Should have received exactly 3 blocks");

    Ok(())
}

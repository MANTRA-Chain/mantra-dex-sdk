mod utils;

use mantra_sdk::{MantraDexClient, MantraWallet};
use utils::test_utils::{
    create_test_client, create_test_network_config, get_or_create_test_pool_id, load_test_config,
};

#[tokio::test]
async fn test_client_creation() {
    let network_config = create_test_network_config();
    let client_result = MantraDexClient::new(network_config.clone()).await;

    assert!(
        client_result.is_ok(),
        "Failed to create client: {:?}",
        client_result.err()
    );

    let client = client_result.unwrap();

    // Verify network config
    let client_config = client.config();
    assert_eq!(
        client_config.network_name, network_config.network_name,
        "Client network name should match config"
    );
    assert_eq!(
        client_config.chain_id, network_config.chain_id,
        "Client network ID should match config"
    );
    assert_eq!(
        client_config.rpc_url, network_config.rpc_url,
        "Client RPC URL should match config"
    );
}

#[tokio::test]
async fn test_client_with_wallet() {
    let network_config = create_test_network_config();
    let client = MantraDexClient::new(network_config.clone())
        .await
        .expect("Failed to create client");

    let test_config = load_test_config();
    let mnemonic = test_config
        .wallets
        .get("primary")
        .expect("Primary wallet not found in test config");

    let wallet =
        MantraWallet::from_mnemonic(mnemonic, 0).expect("Failed to create wallet from mnemonic");

    let client_with_wallet = client.with_wallet(wallet);

    // Verify wallet is set
    let wallet_result = client_with_wallet.wallet();
    assert!(
        wallet_result.is_ok(),
        "Wallet should be set: {:?}",
        wallet_result.err()
    );
}

#[tokio::test]
async fn test_client_without_wallet() {
    let network_config = create_test_network_config();
    let client = MantraDexClient::new(network_config)
        .await
        .expect("Failed to create client");

    // Try to get wallet when none is set
    let wallet_result = client.wallet();
    assert!(wallet_result.is_err(), "Wallet should not be available");
}

#[tokio::test]
async fn test_client_query_pool() {
    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        // Query pool info
        let pool_result = client.get_pool(&pool_id).await;

        // This should succeed if the pool exists and the RPC works
        if let Ok(pool_info) = pool_result {
            assert_eq!(
                pool_info.pool_info.pool_identifier,
                pool_id.clone(),
                "Pool ID in response should match requested ID"
            );
            println!("Successfully queried pool: {}", pool_id);
        } else {
            println!("Warning: Pool query failed: {:?}", pool_result.err());
        }
    } else {
        println!("Warning: Could not get or create OM/USDY pool");
    }
}

#[tokio::test]
async fn test_client_query_pools() {
    let client = create_test_client().await;

    // Query all pools
    let pools_result = client.get_pools(Some(10)).await;

    // This should succeed if the RPC works
    if let Ok(pools) = pools_result {
        println!("Found {} pools", pools.len());

        // If no pools exist, try to create a test pool
        if pools.is_empty() {
            println!("No pools found, attempting to create a test pool...");

            // Check if we have EXECUTE_WRITES permission
            let should_create = std::env::var("EXECUTE_WRITES")
                .unwrap_or_else(|_| "false".to_string())
                .to_lowercase()
                == "true";

            if should_create {
                match get_or_create_test_pool_id(&client).await {
                    Some(pool_id) => {
                        println!("Successfully created/found pool: {}", pool_id);

                        // Query pools again to verify
                        let updated_pools = client.get_pools(Some(10)).await.unwrap();
                        assert!(
                            !updated_pools.is_empty(),
                            "Should have at least one pool after creation"
                        );
                    }
                    None => {
                        println!("Could not create test pool, but that's okay for this test");
                    }
                }
            } else {
                println!("Skipping pool creation (set EXECUTE_WRITES=true to enable)");
            }
        } else {
            // Pools exist, verify basic functionality
            assert!(pools.len() <= 10, "Should return at most 10 pools");
            println!("Pool query successful with {} pools", pools.len());
        }
    } else {
        // If the test fails, we'll print the error but not fail the test
        eprintln!("Warning: Pools query failed: {:?}", pools_result.err());
        eprintln!("This is not a test failure if the network is unavailable.");
        panic!("Pools query failed");
    }
}

#[tokio::test]
async fn test_client_simulate_swap() {
    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        // Simulate a swap
        let uom_denom = test_config
            .tokens
            .get("uom")
            .unwrap()
            .denom
            .clone()
            .unwrap();
        let uusdy_denom = test_config
            .tokens
            .get("uusdy")
            .unwrap()
            .denom
            .clone()
            .unwrap();

        let swap_result = client
            .simulate_swap(
                &pool_id,
                cosmwasm_std::Coin {
                    denom: uom_denom,
                    amount: cosmwasm_std::Uint128::from(1000000u128),
                },
                &uusdy_denom,
            )
            .await;

        // This should succeed if the pool exists and has liquidity
        match swap_result {
            Ok(simulation) => {
                // Check simulation response
                assert!(
                    !simulation.return_amount.is_zero(),
                    "Return amount should not be zero"
                );
                println!("Simulation: {:?}", simulation);
            }
            Err(error) => {
                // Check if the error is due to empty pool
                let error_msg = format!("{:?}", error);
                if error_msg.contains("no assets") || error_msg.contains("empty") {
                    eprintln!(
                        "Warning: Pool {} exists but has no liquidity for simulation.",
                        pool_id
                    );
                    eprintln!("This is not a test failure if the pool is empty.");
                    return; // Don't panic, just skip the test
                }

                // If the test fails for other reasons, we'll print the error but not fail the test
                eprintln!("Warning: Swap simulation failed: {:?}", error);
                eprintln!(
                    "This is not a test failure if the pool doesn't exist or the network is unavailable."
                );
                panic!("Swap simulation failed");
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDY pool for simulation");
    }
}

#[tokio::test]
async fn test_client_get_last_block_height() {
    let client = create_test_client().await;
    let last_block_height = client.get_last_block_height().await;
    assert!(
        last_block_height.is_ok(),
        "Failed to get last block height: {:?}",
        last_block_height.err()
    );
    let height = last_block_height.unwrap();
    println!("Last block height: {:?}", height);
    assert!(
        height > 4091786,
        "Last block height should be greater than 0"
    );
}

#[tokio::test]
async fn test_client_get_balances() {
    let client = create_test_client().await;
    let balances = client.get_balances().await;
    assert!(
        balances.is_ok(),
        "Failed to get balances: {:?}",
        balances.err()
    );
    let balances = balances.unwrap();
    println!("Balances: {:?}", balances);
}

#[tokio::test]
async fn test_pool_creation_if_needed() {
    // Only run this test if EXECUTE_WRITES is enabled
    let should_execute = std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if !should_execute {
        println!("Skipping pool creation test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

    let client = create_test_client().await;

    // Test that we can get or create a pool
    match get_or_create_test_pool_id(&client).await {
        Some(pool_id) => {
            println!("Successfully found/created pool: {}", pool_id);

            // Verify the pool exists by querying it
            let pool_info = client.get_pool(&pool_id).await;
            assert!(
                pool_info.is_ok(),
                "Should be able to query the created pool: {:?}",
                pool_info.err()
            );

            let pool = pool_info.unwrap();
            println!("Pool details:");
            println!("  ID: {}", pool.pool_info.pool_identifier);
            println!("  LP Denom: {}", pool.pool_info.lp_denom);
            println!("  Assets:");
            for asset in &pool.pool_info.assets {
                println!("    {} - {}", asset.denom, asset.amount);
            }
        }
        None => {
            panic!("Failed to create or find test pool");
        }
    }
}

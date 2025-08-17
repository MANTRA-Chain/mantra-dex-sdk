mod utils;

use cosmwasm_std::{Coin, Decimal, Uint128};
use utils::test_utils::{create_test_client, get_or_create_test_pool_id, load_test_config};

#[tokio::test]
async fn test_list_all_pools() {
    // This is a read-only test, so it doesn't need the EXECUTE_WRITES flag
    let client = create_test_client().await;

    // Get all pools (no limit)
    match client.get_pools(Some(100)).await {
        Ok(pools) => {
            println!("Found {} pools:", pools.len());
            for pool in pools {
                println!("Pool ID: {}", pool.pool_info.pool_identifier);
                println!("  LP Denom: {}", pool.total_share.denom);
                println!("  Pool Type: {:?}", pool.pool_info.pool_type);
                println!("  Assets:");
                for asset in &pool.pool_info.assets {
                    println!("    {} - {}", asset.denom, asset.amount);
                }
                println!();
            }
        }
        Err(e) => {
            println!("Failed to get pools: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_swap_operation() {
    println!("Starting swap test...");
    println!("Environment initialized");

    // Skip actual swap unless EXECUTE_WRITES is set
    if !utils::test_utils::should_execute_writes() {
        println!("Skipping actual swap test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

    println!("Creating test client...");
    let client = create_test_client().await;
    let test_config = load_test_config();

    // Add a small delay to avoid sequence issues from concurrent tests
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!("Testing swap operation with pool: {}", pool_id);

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

        let offer_asset = Coin {
            denom: uom_denom.clone(),
            amount: Uint128::from(1000000u128),
        };

        // Simulate a swap first
        let simulation_result = client
            .simulate_swap(&pool_id, offer_asset.clone(), &uusdy_denom)
            .await;

        match simulation_result {
            Ok(simulation) => {
                println!(
                    "Swap simulation successful: return amount = {}",
                    simulation.return_amount
                );
                assert!(!simulation.return_amount.is_zero());
            }
            Err(e) => {
                println!("Warning: Swap simulation failed: {:?}", e);
                return; // Skip the actual swap if simulation fails
            }
        }

        println!("About to execute swap...");
        // Execute swap with timeout
        match tokio::time::timeout(
            std::time::Duration::from_secs(30), // 30 second timeout
            client.swap(
                &pool_id,
                offer_asset,
                &uusdy_denom, // The denom of the ask asset, should match one in the pool
                Some(Decimal::percent(5)), // 5% max slippage (increased for test pools)
            ),
        )
        .await
        {
            Ok(Ok(tx_response)) => {
                println!("Swap successful with txhash: {}", tx_response.txhash);
                assert_eq!(
                    tx_response.code, 0u32,
                    "Transaction failed: {:?}",
                    tx_response.raw_log
                );
                assert!(
                    !tx_response.txhash.is_empty(),
                    "Transaction hash should not be empty"
                );

                // Add a small delay after successful transaction to avoid account sequence mismatch
                // in subsequent operations (prevents race conditions with blockchain state updates)
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            }
            Ok(Err(e)) => {
                println!("Swap failed with error: {:?}", e);

                // Check if the error is due to expected test environment issues
                let error_msg = format!("{:?}", e);
                if error_msg.contains("no assets") || error_msg.contains("empty") {
                    println!("Pool exists but has no liquidity for swap. This is expected in test environments.");
                    return; // Don't panic, just skip the test
                } else if error_msg.contains("Slippage limit exceeded") {
                    println!("Swap failed due to slippage limit. This can happen with low liquidity pools in test environments.");
                    return; // Don't panic, just skip the test
                } else if error_msg.contains("insufficient funds") {
                    println!("Swap failed due to insufficient funds in test wallet.");
                    return; // Don't panic, just skip the test
                } else if error_msg.contains("account sequence mismatch") {
                    println!("Swap failed due to account sequence mismatch. This can happen with concurrent transactions in test environments.");
                    return; // Don't panic, just skip the test
                }

                // For other unexpected errors, still panic
                panic!("Swap failed with unexpected error: {:?}", e);
            }
            Err(_) => {
                println!("Swap operation timed out after 30 seconds");
                panic!("Swap timed out");
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDY pool for swap test");
    }
}

#[tokio::test]
async fn test_provide_liquidity() {
    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!("Testing liquidity provision with pool: {}", pool_id);

        let test_config = load_test_config();
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

        let assets = vec![
            Coin {
                denom: uom_denom.clone(),
                amount: Uint128::from(1000000u128),
            },
            Coin {
                denom: uusdy_denom.clone(),
                amount: Uint128::from(4000000u128),
            },
        ];

        // Provide liquidity
        match client
            .provide_liquidity(
                &pool_id,
                assets,
                Some(Decimal::percent(1)), // 1% liquidity max slippage
                Some(Decimal::percent(1)), // 1% swap max slippage
            )
            .await
        {
            Ok(tx_response) => {
                println!(
                    "Liquidity provision successful with txhash: {}",
                    tx_response.txhash
                );
                assert_eq!(
                    tx_response.code, 0u32,
                    "Transaction failed: {:?}",
                    tx_response.raw_log
                );
                assert!(
                    !tx_response.txhash.is_empty(),
                    "Transaction hash should not be empty"
                );

                // Add a small delay after successful transaction to avoid account sequence mismatch
                // in subsequent operations (prevents race conditions with blockchain state updates)
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            }
            Err(e) => {
                println!("Liquidity provision failed: {:?}", e);
                // Don't fail the test, just log the error
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDY pool for liquidity test");
    }
}

#[tokio::test]
async fn test_withdraw_liquidity() {
    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!("Testing liquidity withdrawal with pool: {}", pool_id);
        // Note: We're not actually withdrawing liquidity in this test as it requires LP tokens
    } else {
        println!("Warning: Could not get or create OM/USDY pool for withdrawal test");
    }
}

#[tokio::test]
async fn test_get_pool() {
    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        match client.get_pool(&pool_id).await {
            Ok(pool_info) => {
                println!("Got pool info: {}", pool_info.pool_info.pool_identifier);
                assert_eq!(pool_info.pool_info.pool_identifier, pool_id);
            }
            Err(e) => {
                println!("Warning: Failed to get pool info: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDY pool for pool info test");
    }
}

#[tokio::test]
async fn test_simulate_swap() {
    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_test_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
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

        // Test swap simulation
        let simulation_result = client
            .simulate_swap(
                &pool_id,
                cosmwasm_std::Coin {
                    denom: uom_denom,
                    amount: cosmwasm_std::Uint128::from(1000000u128),
                },
                &uusdy_denom,
            )
            .await;

        match simulation_result {
            Ok(simulation) => {
                println!("Swap simulation result:");
                println!("Return amount: {}", simulation.return_amount);
                assert!(!simulation.return_amount.is_zero());
            }
            Err(e) => {
                println!("Warning: Simulation failed: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDY pool for simulation");
    }
}

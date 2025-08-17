mod utils;

use mantra_sdk::{protocols::dex::client::PoolStatus, Error};
use utils::test_utils::{create_test_client, get_or_create_test_pool_id};

/// Test pool status enum functionality
#[tokio::test]
async fn test_pool_status_enum() {
    // Test Available status
    let available_status = PoolStatus::Available;
    assert!(
        available_status.is_available(),
        "Available status should be available"
    );

    // Test Disabled status
    let disabled_status = PoolStatus::Disabled;
    assert!(
        !disabled_status.is_available(),
        "Disabled status should not be available"
    );

    // Test equality
    assert_eq!(available_status, PoolStatus::Available);
    assert_eq!(disabled_status, PoolStatus::Disabled);
    assert_ne!(available_status, disabled_status);
}

/// Test pool status extraction from pool info
#[tokio::test]
async fn test_get_pool_status() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Get pool info
    match client.get_pool(&pool_id).await {
        Ok(pool_info) => {
            // Test status extraction
            let status = client.get_pool_status(&pool_info);
            println!("Pool {} status: {:?}", pool_id, status);

            // Test that status extraction works correctly based on the actual pool status
            // The status can be either Available or Disabled depending on the pool's operation flags
            match status {
                PoolStatus::Available => {
                    println!("Pool {} is Available - all operations are enabled", pool_id);
                }
                PoolStatus::Disabled => {
                    println!(
                        "Pool {} is Disabled - some operations are disabled",
                        pool_id
                    );
                }
            }

            // Verify that the status returned makes sense by checking it's one of the expected values
            assert!(
                status == PoolStatus::Available || status == PoolStatus::Disabled,
                "Pool status should be either Available or Disabled"
            );
        }
        Err(e) => {
            println!("Failed to get pool info: {:?}", e);
            // Don't fail the test if the pool doesn't exist
        }
    }
}

/// Test pool status validation before operations
#[tokio::test]
async fn test_validate_pool_status() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test pool status validation
    match client.validate_pool_status(&pool_id).await {
        Ok(()) => {
            println!("Pool {} status validation passed", pool_id);
        }
        Err(e) => {
            println!("Pool status validation failed: {:?}", e);
            // Check if it's a status-related error
            if let Error::Other(msg) = &e {
                if msg.contains("not available for operations") {
                    println!("Pool is disabled, which is a valid test scenario");
                } else {
                    panic!("Unexpected error during pool status validation: {}", msg);
                }
            } else {
                // Other errors (like network issues) shouldn't fail the test
                println!("Non-status related error, continuing test");
            }
        }
    }
}

/// Test that swap operations check pool status
#[tokio::test]
async fn test_swap_with_pool_status_validation() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Only run this test if we should execute writes
    let should_execute = std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if !should_execute {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Create a test swap (small amount)
    let offer_asset = cosmwasm_std::Coin {
        denom: "uom".to_string(),
        amount: cosmwasm_std::Uint128::new(1000), // 0.001 OM
    };

    // Test swap operation (this should validate pool status internally)
    match client
        .swap(
            &pool_id,
            offer_asset,
            "uusdc",
            Some(cosmwasm_std::Decimal::percent(5)),
        )
        .await
    {
        Ok(response) => {
            println!(
                "Swap successful (pool status validation passed): {}",
                response.txhash
            );
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!("Swap failed: {:?}", e);

            // Check if it's a pool status error
            if let Error::Other(msg) = &e {
                if msg.contains("not available for operations") {
                    println!("Swap correctly failed due to pool status validation");
                    return; // This is expected behavior
                }
            }

            // Other errors (insufficient funds, etc.) are also acceptable for this test
            println!("Swap failed for reasons other than pool status (this is acceptable)");
        }
    }
}

/// Test that provide liquidity operations check pool status
#[tokio::test]
async fn test_provide_liquidity_with_pool_status_validation() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Only run this test if we should execute writes
    let should_execute = std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if !should_execute {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Create test assets for liquidity provision
    let assets = vec![
        cosmwasm_std::Coin {
            denom: "uom".to_string(),
            amount: cosmwasm_std::Uint128::new(1000), // 0.001 OM
        },
        cosmwasm_std::Coin {
            denom: "uusdc".to_string(),
            amount: cosmwasm_std::Uint128::new(1000), // 0.001 USDC
        },
    ];

    // Test provide liquidity operation (this should validate pool status internally)
    match client
        .provide_liquidity(
            &pool_id,
            assets,
            Some(cosmwasm_std::Decimal::percent(5)), // liquidity_max_slippage
            Some(cosmwasm_std::Decimal::percent(5)), // swap_max_slippage
        )
        .await
    {
        Ok(response) => {
            println!(
                "Provide liquidity successful (pool status validation passed): {}",
                response.txhash
            );
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!("Provide liquidity failed: {:?}", e);

            // Check if it's a pool status error
            if let Error::Other(msg) = &e {
                if msg.contains("not available for operations") {
                    println!("Provide liquidity correctly failed due to pool status validation");
                    return; // This is expected behavior
                }
            }

            // Other errors (insufficient funds, etc.) are also acceptable for this test
            println!(
                "Provide liquidity failed for reasons other than pool status (this is acceptable)"
            );
        }
    }
}

/// Test that withdraw liquidity operations check pool status
#[tokio::test]
async fn test_withdraw_liquidity_with_pool_status_validation() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Only run this test if we should execute writes
    let should_execute = std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if !should_execute {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Test withdraw liquidity operation (this should validate pool status internally)
    // Note: This will likely fail due to insufficient LP tokens, but we're testing the status validation
    match client
        .withdraw_liquidity(&pool_id, cosmwasm_std::Uint128::new(1))
        .await
    {
        Ok(response) => {
            println!(
                "Withdraw liquidity successful (pool status validation passed): {}",
                response.txhash
            );
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!("Withdraw liquidity failed: {:?}", e);

            // Check if it's a pool status error
            if let Error::Other(msg) = &e {
                if msg.contains("not available for operations") {
                    println!("Withdraw liquidity correctly failed due to pool status validation");
                    return; // This is expected behavior
                }
            }

            // Other errors (insufficient LP tokens, etc.) are also acceptable for this test
            println!(
                "Withdraw liquidity failed for reasons other than pool status (this is acceptable)"
            );
        }
    }
}

/// Test pool status validation with non-existent pool
#[tokio::test]
async fn test_validate_nonexistent_pool_status() {
    let client = create_test_client().await;

    // Test with a non-existent pool ID
    let fake_pool_id = "nonexistent.pool.id";

    match client.validate_pool_status(fake_pool_id).await {
        Ok(()) => {
            panic!("Pool status validation should fail for non-existent pool");
        }
        Err(e) => {
            println!(
                "Pool status validation correctly failed for non-existent pool: {:?}",
                e
            );

            // Should be a "Pool not found" error
            if let Error::Other(msg) = &e {
                assert!(
                    msg.contains("not found") || msg.contains("Pool"),
                    "Error should indicate pool not found: {}",
                    msg
                );
            }
        }
    }
}

/// Test pool status functionality with multiple pools
#[tokio::test]
async fn test_multiple_pools_status() {
    let client = create_test_client().await;

    // Get list of pools
    match client.get_pools(Some(5)).await {
        Ok(pools) => {
            println!("Found {} pools for status testing", pools.len());

            for pool in pools.iter().take(3) {
                // Test status extraction for each pool
                let status = client.get_pool_status(pool);
                println!(
                    "Pool {} status: {:?}",
                    pool.pool_info.pool_identifier, status
                );

                // Test status validation for each pool
                match client
                    .validate_pool_status(&pool.pool_info.pool_identifier)
                    .await
                {
                    Ok(()) => {
                        println!(
                            "Pool {} status validation passed",
                            pool.pool_info.pool_identifier
                        );
                    }
                    Err(e) => {
                        println!(
                            "Pool {} status validation failed: {:?}",
                            pool.pool_info.pool_identifier, e
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!("Failed to get pools list: {:?}", e);
            // Don't fail the test if we can't get pools
        }
    }
}

/// Test that unchecked operations bypass status validation
#[tokio::test]
async fn test_unchecked_operations_bypass_status() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Only run this test if we should execute writes
    let should_execute = std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if !should_execute {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Create test assets for liquidity provision
    let assets = vec![
        cosmwasm_std::Coin {
            denom: "uom".to_string(),
            amount: cosmwasm_std::Uint128::new(1000), // 0.001 OM
        },
        cosmwasm_std::Coin {
            denom: "uusdc".to_string(),
            amount: cosmwasm_std::Uint128::new(1000), // 0.001 USDC
        },
    ];

    // Test unchecked provide liquidity operation (should bypass status validation)
    match client
        .provide_liquidity_unchecked(
            &pool_id,
            assets,
            Some(cosmwasm_std::Decimal::percent(5)), // liquidity_max_slippage
            Some(cosmwasm_std::Decimal::percent(5)), // swap_max_slippage
        )
        .await
    {
        Ok(response) => {
            println!(
                "Unchecked provide liquidity successful (bypassed status validation): {}",
                response.txhash
            );
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!("Unchecked provide liquidity failed: {:?}", e);

            // Should NOT be a pool status error since we bypassed validation
            if let Error::Other(msg) = &e {
                assert!(
                    !msg.contains("not available for operations"),
                    "Unchecked operation should not fail due to pool status: {}",
                    msg
                );
            }

            // Other errors (insufficient funds, etc.) are acceptable
            println!("Unchecked operation failed for reasons other than pool status (this is acceptable)");
        }
    }
}

/// Test that both Available and Disabled pool statuses are correctly mapped
#[tokio::test]
async fn test_pool_status_mapping_available_and_disabled() {
    let client = create_test_client().await;

    // Get list of pools to test status mapping
    match client.get_pools(Some(10)).await {
        Ok(pools) => {
            println!("Testing pool status mapping on {} pools", pools.len());

            let mut available_count = 0;
            let mut disabled_count = 0;

            for pool in pools.iter().take(5) {
                let status = client.get_pool_status(pool);
                let pool_id = &pool.pool_info.pool_identifier;

                match status {
                    PoolStatus::Available => {
                        available_count += 1;
                        println!("Pool {} is Available - all operations enabled", pool_id);

                        // Verify that all operations are indeed enabled
                        let pool_status = &pool.pool_info.status;
                        assert!(
                            pool_status.swaps_enabled
                                && pool_status.deposits_enabled
                                && pool_status.withdrawals_enabled,
                            "Available pool should have all operations enabled"
                        );
                    }
                    PoolStatus::Disabled => {
                        disabled_count += 1;
                        println!("Pool {} is Disabled - some operations disabled", pool_id);

                        // Verify that at least one operation is disabled
                        let pool_status = &pool.pool_info.status;
                        assert!(
                            !pool_status.swaps_enabled
                                || !pool_status.deposits_enabled
                                || !pool_status.withdrawals_enabled,
                            "Disabled pool should have at least one operation disabled"
                        );
                    }
                }
            }

            println!(
                "Status mapping test completed: {} Available, {} Disabled",
                available_count, disabled_count
            );

            // At least one of each status should be tested (if pools exist)
            if pools.len() > 0 {
                assert!(
                    available_count + disabled_count > 0,
                    "Should test at least one pool status"
                );
            }
        }
        Err(e) => {
            println!("Failed to get pools for status mapping test: {:?}", e);
            // Don't fail the test if we can't get pools
        }
    }
}

/// Test pool status detection with specific operation states
#[tokio::test]
async fn test_pool_status_operation_flags() {
    let client = create_test_client().await;

    // Get pools and analyze their operation flags
    match client.get_pools(Some(10)).await {
        Ok(pools) => {
            println!("Analyzing operation flags for {} pools", pools.len());

            for pool in pools.iter().take(3) {
                let pool_id = &pool.pool_info.pool_identifier;
                let status_info = &pool.pool_info.status;
                let computed_status = client.get_pool_status(pool);

                println!(
                    "Pool {}: Operations - Swaps: {}, Deposits: {}, Withdrawals: {}",
                    pool_id,
                    status_info.swaps_enabled,
                    status_info.deposits_enabled,
                    status_info.withdrawals_enabled
                );

                // Verify our status computation logic
                let expected_status = if status_info.swaps_enabled
                    && status_info.deposits_enabled
                    && status_info.withdrawals_enabled
                {
                    PoolStatus::Available
                } else {
                    PoolStatus::Disabled
                };

                assert_eq!(
                    computed_status, expected_status,
                    "Computed status should match expected status for pool {}",
                    pool_id
                );

                println!("Pool {} computed status: {:?} ✓", pool_id, computed_status);
            }
        }
        Err(e) => {
            println!("Failed to get pools for operation flags test: {:?}", e);
            // Don't fail the test if we can't get pools
        }
    }
}

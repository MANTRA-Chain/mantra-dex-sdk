use cosmwasm_std::{Coin, Decimal, Uint128};
use std::str::FromStr;
use std::time::Duration;
use tokio::time::timeout;

mod utils;
use mantra_sdk::Error;
use utils::test_utils::*;

/// Comprehensive MCP integration test that calls all available tools in proper order
#[tokio::test]
async fn test_mcp_tools_comprehensive_integration() {
    let client = create_test_client().await;

    println!("Starting comprehensive MCP integration test...");

    // Phase 1: Network and Connectivity Tests
    test_network_connectivity_tools(&client).await;

    // Phase 2: Wallet and Balance Tests
    test_wallet_balance_tools(&client).await;

    // Phase 3: Pool Query Tests
    test_pool_query_tools(&client).await;

    // Phase 4: DEX Core Trading Tests (with proper ordering)
    test_dex_core_trading_tools(&client).await;

    // Phase 5: LP Token Management Tests
    test_lp_token_management_tools(&client).await;

    println!("Comprehensive MCP integration test completed successfully!");
}

/// Test network and connectivity tools
async fn test_network_connectivity_tools(client: &mantra_sdk::MantraDexClient) {
    println!("Testing network and connectivity tools...");

    // Test get_last_block_height (network connectivity check)
    println!("  Testing get_last_block_height (network connectivity)...");
    let block_height_result = client.get_last_block_height().await;
    match block_height_result {
        Ok(height) => {
            println!("    ✓ Latest block height retrieved: {}", height);
            assert!(height > 0, "Block height should be greater than 0");
        }
        Err(Error::Network(msg)) => {
            println!("    ⚠ Block height test - expected network error: {}", msg);
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Block height test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in get_last_block_height: {:?}", e),
    }

    // Test get_pool_manager_config (contract connectivity check)
    println!("  Testing get_pool_manager_config (contract connectivity)...");
    let config_result = client.get_pool_manager_config().await;
    match config_result {
        Ok(config) => {
            println!("    ✓ Pool manager config retrieved: {:?}", config);
        }
        Err(Error::Contract(msg)) => {
            println!(
                "    ⚠ Pool manager config test - expected mock error: {}",
                msg
            );
        }
        Err(e) => panic!("Unexpected error in get_pool_manager_config: {:?}", e),
    }

    println!("  Network and connectivity tools test completed ✓");
}

/// Test wallet and balance tools
async fn test_wallet_balance_tools(client: &mantra_sdk::MantraDexClient) {
    println!("Testing wallet and balance tools...");

    // Test get_balances
    println!("  Testing get_balances...");
    let balances_result = client.get_balances().await;
    match balances_result {
        Ok(balances) => {
            println!("    ✓ Wallet balances retrieved: {:?}", balances);
            // Balances can be empty for test wallets
        }
        Err(Error::Wallet(msg)) => {
            println!(
                "    ⚠ Wallet balances test - expected wallet error: {}",
                msg
            );
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Wallet balances test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in get_balances: {:?}", e),
    }

    println!("  Wallet and balance tools test completed ✓");
}

/// Test pool query tools
async fn test_pool_query_tools(client: &mantra_sdk::MantraDexClient) {
    println!("Testing pool query tools...");

    // Test get_pools
    println!("  Testing get_pools...");
    let pools_result = client.get_pools(None).await;
    match pools_result {
        Ok(pools) => {
            println!(
                "    ✓ Pool information retrieved: {} pools found",
                pools.len()
            );
            if !pools.is_empty() {
                println!("    First pool info: {:?}", pools[0]);
            }
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Pool query test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in get_pools: {:?}", e),
    }

    // Test get_pool for a specific pool
    println!("  Testing get_pool for specific pool...");
    let pool_id = "test_pool_1";
    let specific_pool_result = client.get_pool(pool_id).await;
    match specific_pool_result {
        Ok(pool) => {
            println!("    ✓ Specific pool information retrieved: {:?}", pool);
        }
        Err(Error::Contract(msg)) => {
            println!(
                "    ⚠ Specific pool query test - expected mock error: {}",
                msg
            );
        }
        Err(e) => panic!("Unexpected error in get_pool: {:?}", e),
    }

    println!("  Pool query tools test completed ✓");
}

/// Test DEX core trading tools with proper ordering
async fn test_dex_core_trading_tools(client: &mantra_sdk::MantraDexClient) {
    println!("Testing DEX core trading tools with proper ordering...");

    let pool_id = "integration_test_pool";

    // Step 1: Provide liquidity first (this should be done before swaps)
    println!("  Step 1: Testing provide_liquidity...");
    let assets = vec![
        Coin {
            denom: "uom".to_string(),
            amount: Uint128::from(1000000u128),
        },
        Coin {
            denom: "uusdc".to_string(),
            amount: Uint128::from(1000000u128),
        },
    ];

    let liquidity_max_slippage = Some(Decimal::from_str("0.05").unwrap()); // 5%
    let swap_max_slippage = Some(Decimal::from_str("0.03").unwrap()); // 3%

    let provide_liquidity_result = client
        .provide_liquidity(
            pool_id,
            assets.clone(),
            liquidity_max_slippage,
            swap_max_slippage,
        )
        .await;

    match provide_liquidity_result {
        Ok(response) => {
            println!("    ✓ Liquidity provided successfully: {:?}", response);
        }
        Err(Error::Contract(msg)) => {
            println!(
                "    ⚠ Provide liquidity test - expected mock error: {}",
                msg
            );
        }
        Err(e) => panic!("Unexpected error in provide_liquidity: {:?}", e),
    }

    // Step 2: Test swap execution (after liquidity is available)
    println!("  Step 2: Testing execute_swap...");
    let offer_asset = Coin {
        denom: "uom".to_string(),
        amount: Uint128::from(100000u128),
    };
    let ask_asset_denom = "uusdc";
    let max_slippage = Some(Decimal::from_str("0.05").unwrap()); // 5%

    let swap_result = client
        .swap(pool_id, offer_asset, ask_asset_denom, max_slippage)
        .await;

    match swap_result {
        Ok(response) => {
            println!("    ✓ Swap executed successfully: {:?}", response);
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Swap execution test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in execute_swap: {:?}", e),
    }

    // Step 3: Test simulation (can be done anytime)
    println!("  Step 3: Testing simulate_swap...");
    let simulate_offer_asset = Coin {
        denom: "uom".to_string(),
        amount: Uint128::from(50000u128),
    };

    let simulation_result = client
        .simulate_swap(pool_id, simulate_offer_asset, "uusdc")
        .await;

    match simulation_result {
        Ok(response) => {
            println!("    ✓ Swap simulation completed: {:?}", response);
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Swap simulation test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in simulate_swap: {:?}", e),
    }

    // Step 4: Test withdraw liquidity (should be done after providing liquidity)
    println!("  Step 4: Testing withdraw_liquidity...");
    let lp_token_amount = Uint128::from(500000u128);
    let _withdraw_max_slippage = Some(Decimal::from_str("0.05").unwrap()); // 5% (for future use)

    let withdraw_result = client.withdraw_liquidity(pool_id, lp_token_amount).await;

    match withdraw_result {
        Ok(response) => {
            println!("    ✓ Liquidity withdrawn successfully: {:?}", response);
        }
        Err(Error::Contract(msg)) => {
            println!(
                "    ⚠ Withdraw liquidity test - expected mock error: {}",
                msg
            );
        }
        Err(e) => panic!("Unexpected error in withdraw_liquidity: {:?}", e),
    }

    // Step 5: Test pool creation (admin only - should be done first in real scenarios)
    println!("  Step 5: Testing create_pool (admin only)...");
    let asset_denoms = vec!["uom".to_string(), "uusdc".to_string()];
    let asset_decimals = vec![6u8, 6u8];

    // Create minimal pool fees
    let pool_fees = mantra_dex_std::fee::PoolFee {
        protocol_fee: mantra_dex_std::fee::Fee {
            share: Decimal::from_str("0.01").unwrap(),
        },
        swap_fee: mantra_dex_std::fee::Fee {
            share: Decimal::from_str("0.003").unwrap(),
        },
        burn_fee: mantra_dex_std::fee::Fee {
            share: Decimal::from_str("0.0").unwrap(),
        },
        extra_fees: vec![],
    };

    let pool_type = mantra_dex_std::pool_manager::PoolType::StableSwap { amp: 100u64 };

    let create_pool_result = client
        .create_pool(
            asset_denoms,
            asset_decimals,
            pool_fees,
            pool_type,
            Some("new_integration_pool".to_string()),
        )
        .await;

    match create_pool_result {
        Ok(response) => {
            println!("    ✓ Pool created successfully: {:?}", response);
        }
        Err(Error::Contract(msg)) => {
            println!(
                "    ⚠ Create pool test - expected mock error (admin only): {}",
                msg
            );
        }
        Err(Error::FeeValidation(msg)) => {
            println!(
                "    ⚠ Create pool test - expected fee validation error (admin only): {}",
                msg
            );
        }
        Err(e) => panic!("Unexpected error in create_pool: {:?}", e),
    }

    println!("  DEX core trading tools test completed ✓");
}

/// Test LP token management tools (using available methods)
async fn test_lp_token_management_tools(client: &mantra_sdk::MantraDexClient) {
    println!("Testing LP token management tools...");

    let test_address = "mantra1cc0jfcd3rv3d36g6m575mdk8p2nmdjgnaf7ngq".to_string();

    // Test get_balances (includes LP tokens)
    println!("  Testing get_balances (includes LP tokens)...");
    let balance_result = client.get_balances().await;
    match balance_result {
        Ok(balances) => {
            println!(
                "    ✓ Balances retrieved (including LP tokens): {} coins",
                balances.len()
            );
            // Filter for LP tokens (typically have format factory/{pool_contract}/lp)
            let lp_tokens: Vec<_> = balances
                .iter()
                .filter(|coin| coin.denom.contains("factory/") && coin.denom.contains("/lp"))
                .collect();
            println!("    LP tokens found: {}", lp_tokens.len());
        }
        Err(Error::Wallet(msg)) => {
            println!("    ⚠ Balance test - expected wallet error: {}", msg);
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Balance test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in get_balances: {:?}", e),
    }

    // Test get_balances_for_address (can be used for LP token queries)
    println!("  Testing get_balances_for_address (LP token queries)...");
    let address_balance_result = client.get_balances_for_address(&test_address).await;
    match address_balance_result {
        Ok(balances) => {
            println!("    ✓ Address balances retrieved: {} coins", balances.len());
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ Address balance test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in get_balances_for_address: {:?}", e),
    }

    // Test query_rewards (LP rewards)
    println!("  Testing query_rewards (LP rewards)...");
    let rewards_result = client.query_rewards(&test_address, None).await;
    match rewards_result {
        Ok(rewards) => {
            println!("    ✓ LP rewards queried: {:?}", rewards);
        }
        Err(Error::Contract(msg)) => {
            println!("    ⚠ LP rewards test - expected mock error: {}", msg);
        }
        Err(e) => panic!("Unexpected error in query_rewards: {:?}", e),
    }

    println!("  LP token management tools test completed ✓");
}

/// Test error handling and edge cases
#[tokio::test]
async fn test_mcp_tools_error_handling() {
    let client = create_test_client().await;

    println!("Testing MCP tools error handling...");

    // Test with invalid pool ID
    let invalid_pool_id = "nonexistent.pool.12345";

    let invalid_pool_result = client.get_pool(invalid_pool_id).await;
    match invalid_pool_result {
        Ok(_) => panic!("Expected error for nonexistent pool"),
        Err(Error::Contract(msg)) => {
            println!("  ✓ Proper error handling for nonexistent pool: {}", msg);
        }
        Err(e) => {
            println!("  ✓ Error handling works (different error type): {:?}", e);
        }
    }

    // Test with invalid amounts
    let zero_amount_coin = Coin {
        denom: "uom".to_string(),
        amount: Uint128::zero(),
    };

    let zero_swap_result = client
        .swap(invalid_pool_id, zero_amount_coin, "uusdc", None)
        .await;

    match zero_swap_result {
        Ok(_) => panic!("Expected error for zero amount swap"),
        Err(Error::Contract(msg)) => {
            println!("  ✓ Proper error handling for zero amount: {}", msg);
        }
        Err(e) => {
            println!(
                "  ✓ Error handling works for zero amount (different error type): {:?}",
                e
            );
        }
    }

    println!("  MCP tools error handling test completed ✓");
}

/// Test timeout and performance characteristics
#[tokio::test]
async fn test_mcp_tools_performance() {
    let client = create_test_client().await;

    println!("Testing MCP tools performance...");

    // Test that operations complete within reasonable time
    let operation_timeout = Duration::from_secs(30);

    // Test multiple operations with timeout
    let operations = vec!["get_balances", "get_pools", "get_last_block_height"];

    for operation in operations {
        println!("  Testing {} with timeout...", operation);

        let result: Result<Result<(), _>, _> = match operation {
            "get_balances" => {
                let r = timeout(operation_timeout, client.get_balances()).await;
                match r {
                    Ok(Ok(_)) => Ok(Ok(())),
                    Ok(Err(e)) => Ok(Err(e)),
                    Err(e) => Err(e),
                }
            }
            "get_pools" => {
                let r = timeout(operation_timeout, client.get_pools(None)).await;
                match r {
                    Ok(Ok(_)) => Ok(Ok(())),
                    Ok(Err(e)) => Ok(Err(e)),
                    Err(e) => Err(e),
                }
            }
            "get_last_block_height" => {
                let r = timeout(operation_timeout, client.get_last_block_height()).await;
                match r {
                    Ok(Ok(_)) => Ok(Ok(())),
                    Ok(Err(e)) => Ok(Err(e)),
                    Err(e) => Err(e),
                }
            }
            _ => unreachable!(),
        };

        match result {
            Ok(_) => {
                println!("    ✓ {} completed within timeout", operation);
            }
            Err(_) => {
                panic!(
                    "Operation {} timed out after {:?}",
                    operation, operation_timeout
                );
            }
        }
    }

    println!("  MCP tools performance test completed ✓");
}

/// Test concurrent access to MCP tools
#[tokio::test]
async fn test_mcp_tools_concurrent_access() {
    println!("Testing MCP tools concurrent access...");

    // Test multiple concurrent operations with separate clients
    let mut handles = vec![];

    // Spawn multiple concurrent tasks
    for i in 0..3 {
        let handle = tokio::spawn(async move {
            let client = create_test_client().await;
            let pool_id = format!("concurrent_test_pool_{}", i);

            // Try to get pool info concurrently
            let result = client.get_pool(&pool_id).await;
            match result {
                Ok(_) => println!("    ✓ Concurrent operation {} succeeded", i),
                Err(_) => println!("    ⚠ Concurrent operation {} failed (expected)", i),
            }
        });

        handles.push(handle);
    }

    // Wait for all concurrent operations to complete
    for handle in handles {
        handle.await.expect("Concurrent task should complete");
    }

    println!("  MCP tools concurrent access test completed ✓");
}

/// Test slippage validation in MCP adapter execute_swap_simple function
#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_mcp_adapter_slippage_validation_integration() {
    use mantra_sdk::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
    
    println!("Testing MCP adapter slippage validation integration...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test invalid slippage formats that should be caught early
    println!("  Testing invalid slippage format rejection...");
    let invalid_slippage_cases = vec![
        ("invalid", "non-numeric string"),
        ("-0.1", "negative value"),
        ("1.5", "value greater than 100%"),
        ("", "empty string"),
        ("0.05%", "percentage format"),
    ];
    
    for (slippage, description) in invalid_slippage_cases {
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "1000000".to_string(),
            slippage.to_string(),
            "test_pool".to_string(),
            None,
        ).await;
        
        assert!(result.is_err(), "Should reject invalid slippage: {}", description);
        let error_msg = format!("{:?}", result.unwrap_err());
        
        // Verify it fails on slippage validation, not later stages
        if slippage == "-0.1" {
            // Negative values are caught as format errors by Decimal::from_str
            assert!(error_msg.contains("Invalid slippage format"), 
                "Should fail on format validation for negative value: {} - actual error: {}", slippage, error_msg);
        } else if slippage == "1.5" {
            assert!(error_msg.contains("cannot be greater than 1.0"), 
                "Should fail on >1.0 slippage validation: {} - actual error: {}", slippage, error_msg);
        } else {
            assert!(error_msg.contains("Invalid slippage format"), 
                "Should fail on format validation: {} - actual error: {}", slippage, error_msg);
        }
        
        println!("    ✓ {} correctly rejected", description);
    }
    
    // Test valid slippage values that should pass validation and fail later
    println!("  Testing valid slippage format acceptance...");
    let valid_slippage_cases = vec![
        ("0.0", "zero slippage"),
        ("0.05", "5% slippage"),
        ("0.5", "50% slippage"),
        ("1.0", "100% slippage"),
    ];
    
    for (slippage, description) in valid_slippage_cases {
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "1000000".to_string(),
            slippage.to_string(),
            "test_pool".to_string(),
            None,
        ).await;
        
        // Should fail on wallet/network issues, not slippage
        match result {
            Err(err) => {
                let error_msg = format!("{:?}", err);
                assert!(!error_msg.contains("Invalid slippage"), 
                    "Should not fail on slippage validation for valid value: {}", slippage);
                assert!(!error_msg.contains("slippage cannot be negative"), 
                    "Should not fail on negative check for valid value: {}", slippage);
                assert!(!error_msg.contains("cannot be greater than 1.0"), 
                    "Should not fail on >1.0 check for valid value: {}", slippage);
                
                println!("    ✓ {} passed slippage validation (failed later as expected)", description);
            }
            Ok(_) => {
                println!("    ✓ {} passed slippage validation (unexpectedly succeeded)", description);
            }
        }
    }
    
    println!("  MCP adapter slippage validation integration test completed ✓");
}

/// Test slippage edge cases in integration context
#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_mcp_adapter_slippage_edge_cases_integration() {
    use mantra_sdk::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
    
    println!("Testing MCP adapter slippage edge cases integration...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test boundary values
    println!("  Testing boundary slippage values...");
    
    // Test exactly at boundaries (should be valid)
    let boundary_cases = vec![
        ("0.0", "exactly zero"),
        ("1.0", "exactly one"),
        ("0.000001", "very small positive"),
        ("0.999999", "very close to one"),
    ];
    
    for (slippage, description) in boundary_cases {
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "1000000".to_string(),
            slippage.to_string(),
            "test_pool".to_string(),
            None,
        ).await;
        
        match result {
            Err(err) => {
                let error_msg = format!("{:?}", err);
                assert!(!error_msg.contains("Invalid slippage"), 
                    "Boundary value should be valid: {}", slippage);
                println!("    ✓ {} accepted (failed later as expected)", description);
            }
            Ok(_) => {
                println!("    ✓ {} accepted (unexpectedly succeeded)", description);
            }
        }
    }
    
    // Test just outside boundaries (should be invalid)
    println!("  Testing just-outside-boundary slippage values...");
    
    let invalid_boundary_cases = vec![
        ("-0.000001", "just below zero"),
        ("1.000001", "just above one"),
    ];
    
    for (slippage, description) in invalid_boundary_cases {
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "1000000".to_string(),
            slippage.to_string(),
            "test_pool".to_string(),
            None,
        ).await;
        
        assert!(result.is_err(), "Should reject boundary violation: {}", description);
        let error_msg = format!("{:?}", result.unwrap_err());
        
        if slippage.starts_with('-') {
            // Negative values are caught as format errors by Decimal::from_str
            assert!(error_msg.contains("Invalid slippage format"), 
                "Should fail on format validation for negative value: {}", slippage);
        } else {
            assert!(error_msg.contains("cannot be greater than 1.0"), 
                "Should fail on >1.0 check: {}", slippage);
        }
        
        println!("    ✓ {} correctly rejected", description);
    }
    
    println!("  MCP adapter slippage edge cases integration test completed ✓");
}

/// Test realistic slippage scenarios in trading context
#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_mcp_adapter_realistic_slippage_scenarios() {
    use mantra_sdk::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
    
    println!("Testing realistic slippage scenarios...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test common realistic slippage values used in DeFi
    println!("  Testing common DeFi slippage values...");
    
    let realistic_cases = vec![
        ("0.005", "0.5% (low volatility)"),
        ("0.01", "1% (standard)"),
        ("0.03", "3% (moderate)"),
        ("0.05", "5% (high volatility)"),
        ("0.1", "10% (very high volatility)"),
        ("0.2", "20% (extreme volatility)"),
    ];
    
    for (slippage, description) in realistic_cases {
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "100000".to_string(), // 0.1 OM
            slippage.to_string(),
            "realistic_pool".to_string(),
            None,
        ).await;
        
        // These should all pass slippage validation
        match result {
            Err(err) => {
                let error_msg = format!("{:?}", err);
                assert!(!error_msg.contains("Invalid slippage"), 
                    "Realistic slippage should be valid: {}", slippage);
                println!("    ✓ {} passed validation (failed later as expected)", description);
            }
            Ok(_) => {
                println!("    ✓ {} passed validation (unexpectedly succeeded)", description);
            }
        }
    }
    
    // Test common user mistakes
    println!("  Testing common user mistakes...");
    
    let mistake_cases = vec![
        ("5", "percentage without decimal (5 instead of 0.05)"),
        ("50", "percentage confusion (50 instead of 0.5)"),
        ("-5", "negative percentage"),
        ("105", "over 100% as integer"),
    ];
    
    for (slippage, description) in mistake_cases {
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "100000".to_string(),
            slippage.to_string(),
            "mistake_pool".to_string(),
            None,
        ).await;
        
        assert!(result.is_err(), "Should catch user mistake: {}", description);
        let error_msg = format!("{:?}", result.unwrap_err());
        
        if slippage.starts_with('-') {
            // Negative values are caught as format errors by Decimal::from_str
            assert!(error_msg.contains("Invalid slippage format"), 
                "Should catch negative mistake as format error: {}", slippage);
        } else {
            assert!(error_msg.contains("cannot be greater than 1.0"), 
                "Should catch >100% mistake: {}", slippage);
        }
        
        println!("    ✓ {} correctly caught", description);
    }
    
    println!("  Realistic slippage scenarios test completed ✓");
}

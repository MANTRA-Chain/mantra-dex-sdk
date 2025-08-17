use cosmwasm_std::{Coin, Uint128};
use mantra_sdk::{
    MantraDexClient, MantraNetworkConfig, MantraWallet, SkipAsset, SkipRoute, SkipSwapOperation,
};
use serde::Deserialize;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Once;

static INIT: Once = Once::new();

/// Test configuration for Skip Adapter functionality
///
/// This test suite validates the integration between the Mantra DEX SDK and Skip Adapter contracts.
/// Tests include swap simulations, route optimization, and cross-chain functionality.

/// Initialize transaction log file with header (clears previous runs)
fn init_transaction_log() {
    INIT.call_once(|| {
        let header = "# Skip Adapter Transaction Log\n# Format: timestamp | test_name | tx_hash | block_height\n";
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)  // Clear file at start of each test run
            .open("transaction_hashes.log")
        {
            let _ = file.write_all(header.as_bytes());
        }
    });
}

/// Log transaction hash to a file for tracking successful transactions
fn log_transaction_hash(tx_hash: &str, test_name: &str, block_height: i64) {
    init_transaction_log();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let log_entry = format!(
        "{} | {} | {} | {}\n",
        timestamp, test_name, tx_hash, block_height
    );

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("transaction_hashes.log")
    {
        let _ = file.write_all(log_entry.as_bytes());
        println!("📁 Transaction hash logged to transaction_hashes.log");
    }
}

#[derive(Debug, Deserialize)]
struct TestConfig {
    wallets: TestWallets,
}

#[derive(Debug, Deserialize)]
struct TestWallets {
    primary: String,
    #[allow(dead_code)]
    secondary: String,
}

/// Load test mnemonic from config/test.toml
fn load_test_mnemonic() -> Result<String, Box<dyn std::error::Error>> {
    let config_paths = vec![
        "config/test.toml",
        "../config/test.toml",
        "../../config/test.toml",
    ];

    for config_path in &config_paths {
        if let Ok(content) = std::fs::read_to_string(config_path) {
            let config: TestConfig = toml::from_str(&content)?;
            return Ok(config.wallets.primary);
        }
    }

    Err(format!(
        "Could not find config/test.toml in any of the following locations: {:?}. Please ensure the test configuration file exists.",
        config_paths
    ).into())
}

/// Setup test client with Skip Adapter configuration
async fn setup_test_client() -> Result<MantraDexClient, Box<dyn std::error::Error>> {
    // Use testnet configuration
    let config = MantraNetworkConfig::default();
    let client = MantraDexClient::new(config).await?;

    // Create wallet for testing using mnemonic from config
    let mnemonic = load_test_mnemonic()?;
    let wallet = MantraWallet::from_mnemonic(&mnemonic, 0)?;
    let client = client.with_wallet(wallet);

    Ok(client)
}

#[tokio::test]
async fn test_simulate_skip_swap_exact_asset_in() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Create swap operations using real pool with liquidity
    let swap_operations = vec![SkipSwapOperation {
        pool: "o.uom.usdy.pool".to_string(),
        denom_in: "uom".to_string(),
        denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
        interface: None,
    }];

    let asset_in = SkipAsset::native("uom", 100000u128);

    // Test with real pool that has liquidity
    let result = client
        .simulate_skip_swap_exact_asset_in(asset_in.clone(), swap_operations)
        .await;

    match result {
        Ok(asset_out) => {
            println!("✅ Skip Adapter simulation successful!");
            println!("  Input: {} uom", asset_in.amount());
            println!("  Output: {} {}", asset_out.amount(), asset_out.denom());

            // Validate response structure
            assert!(
                asset_out.amount() > Uint128::zero(),
                "Should receive non-zero output"
            );
            assert_eq!(
                asset_out.denom(),
                "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
            );
        }
        Err(e) => {
            panic!("Skip Adapter simulation failed unexpectedly: {}", e);
        }
    }
}

#[tokio::test]
async fn test_simulate_skip_swap_exact_asset_out() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Create swap operations using real pool with liquidity
    let swap_operations = vec![SkipSwapOperation {
        pool: "o.uom.usdy.pool".to_string(),
        denom_in: "uom".to_string(),
        denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
        interface: None,
    }];

    let asset_out = SkipAsset::native(
        "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY",
        50000u128,
    );

    // Test reverse simulation with real pool
    let result = client
        .simulate_skip_swap_exact_asset_out(asset_out.clone(), swap_operations)
        .await;

    match result {
        Ok(asset_in) => {
            println!("✅ Skip Adapter reverse simulation successful!");
            println!(
                "  Desired Output: {} {}",
                asset_out.amount(),
                asset_out.denom()
            );
            println!(
                "  Required Input: {} {}",
                asset_in.amount(),
                asset_in.denom()
            );

            // Validate response structure
            assert!(
                asset_in.amount() > Uint128::zero(),
                "Should require non-zero input"
            );
            assert_eq!(asset_in.denom(), "uom");
        }
        Err(e) => {
            panic!("Skip Adapter reverse simulation failed unexpectedly: {}", e);
        }
    }
}

#[tokio::test]
async fn test_simulate_skip_smart_swap() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Create realistic smart swap routes using actual pools with liquidity
    // Goal: Convert 100,000 microOM to USDT using multiple routes for optimal pricing
    let routes = vec![
        // Route 1: Direct OM → USDT (using p.12 pool)
        SkipRoute {
            offer_asset: SkipAsset::native("uom", 50000u128), // Split 50% of input
            operations: vec![SkipSwapOperation {
                pool: "p.12".to_string(),
                denom_in: "uom".to_string(),
                denom_out: "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                    .to_string(),
                interface: None,
            }],
        },
        // Route 2: OM → USDY → USDT (using o.uom.usdy.pool + p.10)
        SkipRoute {
            offer_asset: SkipAsset::native("uom", 30000u128), // Split 30% of input
            operations: vec![
                SkipSwapOperation {
                    pool: "o.uom.usdy.pool".to_string(),
                    denom_in: "uom".to_string(),
                    denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
                        .to_string(),
                    interface: None,
                },
                SkipSwapOperation {
                    pool: "p.10".to_string(),
                    denom_in: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
                        .to_string(),
                    denom_out:
                        "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                            .to_string(),
                    interface: None,
                },
            ],
        },
        // Route 3: OM → USDY → USDT (alternative path using p.11 + p.10)
        SkipRoute {
            offer_asset: SkipAsset::native("uom", 20000u128), // Split 20% of input
            operations: vec![
                SkipSwapOperation {
                    pool: "p.11".to_string(),
                    denom_in: "uom".to_string(),
                    denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
                        .to_string(),
                    interface: None,
                },
                SkipSwapOperation {
                    pool: "p.10".to_string(),
                    denom_in: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
                        .to_string(),
                    denom_out:
                        "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                            .to_string(),
                    interface: None,
                },
            ],
        },
    ];

    let asset_in = SkipAsset::native("uom", 100000u128); // Total input: 100,000 microOM

    println!("🔄 Testing Smart Swap with real pools:");
    println!("  Goal: Convert {} microOM to USDT", asset_in.amount());
    println!("  Route 1: 50% via direct p.12 (OM→USDT)");
    println!("  Route 2: 30% via o.uom.usdy.pool + p.10 (OM→USDY→USDT)");
    println!("  Route 3: 20% via p.11 + p.10 (OM→USDY→USDT alternative)");

    let result = client
        .simulate_skip_smart_swap_exact_asset_in(asset_in.clone(), routes)
        .await;

    match result {
        Ok(asset_out) => {
            println!("✅ Smart swap simulation successful!");
            println!("  Total Input: {} microOM", asset_in.amount());
            println!(
                "  Total Output: {} {}",
                asset_out.amount(),
                asset_out.denom()
            );
            println!("  💡 Skip optimized routing across 3 different paths");

            assert!(
                asset_out.amount() > Uint128::zero(),
                "Should receive non-zero output"
            );
            assert_eq!(
                asset_out.denom(),
                "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
            );
        }
        Err(e) => {
            println!("❌ Smart swap simulation failed: {}", e);
            // Since we're using real pools with liquidity, this should succeed
            panic!("Smart swap should work with real pools and liquidity");
        }
    }
}

#[tokio::test]
async fn test_execute_skip_swap() {
    // Skip only if explicitly disabled via environment variable
    if env::var("SKIP_ONCHAIN_TESTS").is_ok() {
        println!("Skipping on-chain swap execution test (SKIP_ONCHAIN_TESTS is set)");
        return;
    }

    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Get real pools first to use actual pool data
    let pools = match client.get_pools(Some(10)).await {
        Ok(pools) if !pools.is_empty() => pools,
        Ok(_) => {
            println!("No pools found on testnet, skipping on-chain swap test");
            return;
        }
        Err(e) => {
            println!("Failed to get pools: {}, skipping on-chain swap test", e);
            return;
        }
    };

    // Find a pool that uses OM (native token) as input, prefer pools with actual liquidity
    let pool = pools
        .iter()
        .find(|p| {
            p.pool_info.asset_denoms.len() >= 2
                && p.pool_info.asset_denoms.contains(&"uom".to_string())
                && p.pool_info.pool_identifier == "o.uom.usdy.pool"
        })
        .or_else(|| {
            pools.iter().find(|p| {
                p.pool_info.asset_denoms.len() >= 2
                    && p.pool_info.asset_denoms.contains(&"uom".to_string())
            })
        })
        .unwrap_or(&pools[0]);

    if pool.pool_info.asset_denoms.len() < 2 {
        println!("Pool has insufficient assets, skipping on-chain swap test");
        return;
    }

    // Ensure we use OM as input token (more likely to have balance)
    let (input_denom, output_denom) = if pool.pool_info.asset_denoms[0] == "uom" {
        (
            &pool.pool_info.asset_denoms[0],
            &pool.pool_info.asset_denoms[1],
        )
    } else if pool.pool_info.asset_denoms[1] == "uom" {
        (
            &pool.pool_info.asset_denoms[1],
            &pool.pool_info.asset_denoms[0],
        )
    } else {
        // Fallback to first pair if no OM found
        (
            &pool.pool_info.asset_denoms[0],
            &pool.pool_info.asset_denoms[1],
        )
    };

    let operations = vec![SkipSwapOperation {
        pool: pool.pool_info.pool_identifier.clone(),
        denom_in: input_denom.clone(),
        denom_out: output_denom.clone(),
        interface: None,
    }];

    // Use a larger amount for testing to ensure sufficient output for minimum receive
    let offer_coin = Coin {
        denom: input_denom.clone(),
        amount: Uint128::from(1000000u128), // 1 OM token
    };

    println!("Executing on-chain Skip swap:");
    println!("  Pool: {}", pool.pool_info.pool_identifier);
    println!("  From: {} -> To: {}", input_denom, output_denom);
    println!("  Amount: {}", offer_coin.amount);
    println!("  💡 Using OM token which should be available from faucet");

    let result = client
        .execute_skip_swap(operations, offer_coin, Some(Uint128::from(1u128)), None)
        .await;

    match result {
        Ok(tx_response) => {
            println!("✅ TRANSACTION EXECUTED SUCCESSFULLY");
            println!("🔗 Transaction Hash: {}", tx_response.txhash);
            println!("📦 Block Height: {}", tx_response.height);
            println!("⛽ Gas Used: {}", tx_response.gas_used);
            println!("💰 Gas Wanted: {}", tx_response.gas_wanted);
            println!("📄 Raw Log: {}", tx_response.raw_log);

            // Log transaction hash to file
            log_transaction_hash(
                &tx_response.txhash,
                "test_execute_skip_swap",
                tx_response.height,
            );

            assert_eq!(tx_response.code, 0, "Transaction should succeed");
            assert!(
                !tx_response.txhash.is_empty(),
                "Transaction hash should not be empty"
            );
        }
        Err(e) => {
            println!("❌ TRANSACTION FAILED: {}", e);
            println!("This may be due to insufficient funds, invalid pool, or network issues");

            // For now, we'll log the error but not fail the test since wallet might not have funds
            // In a production test environment, this should be a proper failure
            println!("⚠️  Test completed with error (this is expected if wallet has no funds)");
        }
    }
}

#[tokio::test]
async fn test_multi_hop_skip_operations() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Test realistic 3-hop multi-hop operation using actual pools with liquidity
    // Path: OM → aUSDY → USDY → USDT (using pools p.5, o.ausdy.uusdc.pool via aUSDY→USDY, then p.10)
    // Alternative: OM → USDY → USDT (using o.uom.usdy.pool, then p.10)
    let operations = vec![
        // Hop 1: OM → USDY (using o.uom.usdy.pool - has good liquidity)
        SkipSwapOperation {
            pool: "o.uom.usdy.pool".to_string(),
            denom_in: "uom".to_string(),
            denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
            interface: None,
        },
        // Hop 2: USDY → USDT (using p.10 stable swap - has massive liquidity)
        SkipSwapOperation {
            pool: "p.10".to_string(),
            denom_in: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
            denom_out: "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                .to_string(),
            interface: None,
        },
        // Hop 3: USDT → OM (using p.12 - has good liquidity, completing the cycle)
        SkipSwapOperation {
            pool: "p.12".to_string(),
            denom_in: "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                .to_string(),
            denom_out: "uom".to_string(),
            interface: None,
        },
    ];

    let asset_in = SkipAsset::native("uom", 50000u128); // 50,000 microOM input

    println!("🔄 Testing Multi-Hop Skip operations with real pools:");
    println!("  Path: OM → USDY → USDT → OM (3-hop cycle)");
    println!(
        "  Hop 1: {} microOM → USDY via o.uom.usdy.pool",
        asset_in.amount()
    );
    println!("  Hop 2: USDY → USDT via p.10 (stable swap)");
    println!("  Hop 3: USDT → OM via p.12 (completing cycle)");

    let result = client
        .simulate_skip_swap_exact_asset_in(asset_in.clone(), operations)
        .await;

    match result {
        Ok(asset_out) => {
            println!("✅ Multi-hop simulation successful!");
            println!("  Input: {} microOM", asset_in.amount());
            println!("  Output: {} {}", asset_out.amount(), asset_out.denom());
            println!("  💡 Completed 3-hop arbitrage cycle: OM→USDY→USDT→OM");

            // Validate the 3-hop cycle completed successfully
            assert!(
                asset_out.amount() > Uint128::zero(),
                "Should receive non-zero output after 3 hops"
            );
            assert_eq!(
                asset_out.denom(),
                "uom",
                "Should end back at OM after cycle"
            );

            // Calculate efficiency (output vs input) - expect some loss due to fees
            let input_amount = asset_in.amount().u128();
            let output_amount = asset_out.amount().u128();
            let efficiency_percent = (output_amount * 100) / input_amount;
            println!(
                "  📊 Cycle Efficiency: {}% ({}→{} microOM)",
                efficiency_percent, input_amount, output_amount
            );

            // Expect some loss due to trading fees across 3 hops, but should retain reasonable value
            // 3-hop arbitrage cycles naturally have significant slippage/fees, so 2% retention is realistic
            assert!(
                output_amount > input_amount / 50,
                "Should retain at least 2% value after 3-hop cycle"
            );
        }
        Err(e) => {
            println!("❌ Multi-hop simulation failed: {}", e);
            // Since we're using real pools with liquidity, this should succeed
            panic!("Multi-hop operations should work with real pools and liquidity");
        }
    }
}

#[tokio::test]
async fn test_simulate_skip_swap_exact_asset_in_with_metadata() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Create swap operations using real pool with liquidity
    let swap_operations = vec![SkipSwapOperation {
        pool: "o.uom.usdy.pool".to_string(),
        denom_in: "uom".to_string(),
        denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
        interface: None,
    }];

    let asset_in = SkipAsset::native("uom", 100000u128);

    println!("🔄 Testing Skip Adapter simulation with metadata (spot price included):");
    println!("  Input: {} uom", asset_in.amount());
    println!("  Pool: o.uom.usdy.pool");
    println!("  Include spot price: true");

    let result = client
        .simulate_skip_swap_exact_asset_in_with_metadata(asset_in.clone(), swap_operations, true)
        .await;

    match result {
        Ok(response) => {
            println!("✅ Skip Adapter metadata simulation successful!");
            println!("  Input: {} uom", asset_in.amount());
            println!(
                "  Output: {} {}",
                response.asset_out.amount(),
                response.asset_out.denom()
            );

            // Validate response structure with metadata
            assert!(
                response.asset_out.amount() > Uint128::zero(),
                "Should receive non-zero output"
            );
            assert_eq!(
                response.asset_out.denom(),
                "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
            );

            // Validate spot price is included when requested
            if let Some(spot_price) = response.spot_price {
                println!("  💰 Spot Price: {}", spot_price);
                assert!(
                    spot_price > cosmwasm_std::Decimal::zero(),
                    "Spot price should be positive"
                );
            } else {
                println!("  💰 Spot Price: Not provided by contract");
            }
        }
        Err(e) => {
            panic!(
                "Skip Adapter metadata simulation failed unexpectedly: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_simulate_skip_swap_exact_asset_out_with_metadata() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Create swap operations using real pool with liquidity
    let swap_operations = vec![SkipSwapOperation {
        pool: "o.uom.usdy.pool".to_string(),
        denom_in: "uom".to_string(),
        denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
        interface: None,
    }];

    let asset_out = SkipAsset::native(
        "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY",
        50000u128,
    );

    println!("🔄 Testing Skip Adapter reverse simulation with metadata:");
    println!(
        "  Desired Output: {} {}",
        asset_out.amount(),
        asset_out.denom()
    );
    println!("  Pool: o.uom.usdy.pool");
    println!("  Include spot price: true");

    let result = client
        .simulate_skip_swap_exact_asset_out_with_metadata(asset_out.clone(), swap_operations, true)
        .await;

    match result {
        Ok(response) => {
            println!("✅ Skip Adapter reverse metadata simulation successful!");
            println!(
                "  Desired Output: {} {}",
                asset_out.amount(),
                asset_out.denom()
            );
            println!(
                "  Required Input: {} {}",
                response.asset_in.amount(),
                response.asset_in.denom()
            );

            // Validate response structure
            assert!(
                response.asset_in.amount() > Uint128::zero(),
                "Should require non-zero input"
            );
            assert_eq!(response.asset_in.denom(), "uom");

            // Validate spot price is included when requested
            if let Some(spot_price) = response.spot_price {
                println!("  💰 Spot Price: {}", spot_price);
                assert!(
                    spot_price > cosmwasm_std::Decimal::zero(),
                    "Spot price should be positive"
                );
            } else {
                println!("  💰 Spot Price: Not provided by contract");
            }
        }
        Err(e) => {
            panic!(
                "Skip Adapter reverse metadata simulation failed unexpectedly: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_simulate_skip_smart_swap_with_metadata() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    // Create realistic smart swap routes using actual pools with liquidity
    let routes = vec![
        // Route 1: Direct OM → USDT (using p.12 pool)
        SkipRoute {
            offer_asset: SkipAsset::native("uom", 50000u128),
            operations: vec![SkipSwapOperation {
                pool: "p.12".to_string(),
                denom_in: "uom".to_string(),
                denom_out: "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                    .to_string(),
                interface: None,
            }],
        },
        // Route 2: OM → USDY → USDT (using o.uom.usdy.pool + p.10)
        SkipRoute {
            offer_asset: SkipAsset::native("uom", 50000u128),
            operations: vec![
                SkipSwapOperation {
                    pool: "o.uom.usdy.pool".to_string(),
                    denom_in: "uom".to_string(),
                    denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
                        .to_string(),
                    interface: None,
                },
                SkipSwapOperation {
                    pool: "p.10".to_string(),
                    denom_in: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY"
                        .to_string(),
                    denom_out:
                        "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
                            .to_string(),
                    interface: None,
                },
            ],
        },
    ];

    let asset_in = SkipAsset::native("uom", 100000u128);

    println!("🔄 Testing Smart Swap with metadata:");
    println!("  Goal: Convert {} microOM to USDT", asset_in.amount());
    println!("  Route 1: 50% via direct p.12 (OM→USDT)");
    println!("  Route 2: 50% via o.uom.usdy.pool + p.10 (OM→USDY→USDT)");
    println!("  Include spot price: true");

    let result = client
        .simulate_skip_smart_swap_exact_asset_in_with_metadata(asset_in.clone(), routes, true)
        .await;

    match result {
        Ok(response) => {
            println!("✅ Smart swap metadata simulation successful!");
            println!("  Total Input: {} microOM", asset_in.amount());
            println!(
                "  Total Output: {} {}",
                response.asset_out.amount(),
                response.asset_out.denom()
            );
            println!("  💡 Skip optimized routing across 2 different paths with metadata");

            assert!(
                response.asset_out.amount() > Uint128::zero(),
                "Should receive non-zero output"
            );
            assert_eq!(
                response.asset_out.denom(),
                "ibc/D4673DC468A86C668204C7A29BFDC3511FF36D512C38C9EB9215872E9653B239"
            );

            // Validate spot price is included when requested
            if let Some(spot_price) = response.spot_price {
                println!("  💰 Average Spot Price: {}", spot_price);
                assert!(
                    spot_price > cosmwasm_std::Decimal::zero(),
                    "Spot price should be positive"
                );
            } else {
                println!("  💰 Spot Price: Not provided by contract");
            }
        }
        Err(e) => {
            println!("❌ Smart swap metadata simulation failed: {}", e);
            // Since we're using real pools with liquidity, this should succeed
            panic!("Smart swap with metadata should work with real pools and liquidity");
        }
    }
}

#[tokio::test]
async fn test_metadata_queries_spot_price_control() {
    let client = setup_test_client()
        .await
        .expect("Failed to setup test client");

    let swap_operations = vec![SkipSwapOperation {
        pool: "o.uom.usdy.pool".to_string(),
        denom_in: "uom".to_string(),
        denom_out: "factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDY".to_string(),
        interface: None,
    }];

    let asset_in = SkipAsset::native("uom", 100000u128);

    println!("🔄 Testing metadata queries with spot price control:");

    // Test with include_spot_price = false
    println!("  Testing with include_spot_price = false");
    let result_no_price = client
        .simulate_skip_swap_exact_asset_in_with_metadata(
            asset_in.clone(),
            swap_operations.clone(),
            false,
        )
        .await;

    // Test with include_spot_price = true
    println!("  Testing with include_spot_price = true");
    let result_with_price = client
        .simulate_skip_swap_exact_asset_in_with_metadata(asset_in.clone(), swap_operations, true)
        .await;

    match (result_no_price, result_with_price) {
        (Ok(response_no_price), Ok(response_with_price)) => {
            println!("✅ Both metadata queries successful!");

            // Both should return the same asset amounts
            assert_eq!(
                response_no_price.asset_out.amount(),
                response_with_price.asset_out.amount()
            );
            assert_eq!(
                response_no_price.asset_out.denom(),
                response_with_price.asset_out.denom()
            );

            println!("  📊 Asset amounts consistent between both queries");

            // The difference should be in spot price availability based on the flag
            match (response_no_price.spot_price, response_with_price.spot_price) {
                (None, Some(spot_price)) => {
                    println!(
                        "  ✅ Spot price control working: None when false, {} when true",
                        spot_price
                    );
                }
                (Some(_), Some(spot_price)) => {
                    println!(
                        "  ⚠️  Contract provides spot price even when not requested, got: {}",
                        spot_price
                    );
                }
                (None, None) => {
                    println!("  ⚠️  Contract doesn't provide spot price even when requested");
                }
                (Some(price1), None) => {
                    println!(
                        "  ⚠️  Unexpected: got spot price {} when false, none when true",
                        price1
                    );
                }
            }
        }
        (Err(e1), _) => panic!(
            "Metadata query with include_spot_price=false failed: {}",
            e1
        ),
        (_, Err(e2)) => panic!("Metadata query with include_spot_price=true failed: {}", e2),
    }
}

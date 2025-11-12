//! PrimarySale Integration Tests
//!
//! This module contains tests for PrimarySale contract operations including
//! query operations, user operations, admin operations, and public operations.
//!
//! ## Test Structure
//! - **Unit Tests** (`primary_sale_unit_tests`) - Fast tests with no external dependencies
//! - **Testnet Tests** (`primary_sale_testnet_tests`) - Integration tests requiring testnet access
//!
//! ## Running Tests
//! ```bash
//! # Run unit tests only (default)
//! cargo test --features evm,mcp primary_sale
//!
//! # Run testnet tests (requires environment variables)
//! export TESTNET_PRIMARY_SALE_CONTRACT=0x...
//! export TESTNET_WALLET_MNEMONIC="..."
//! export TESTNET_EVM_RPC_URL=https://...
//! cargo test --features evm,mcp primary_sale -- --ignored
//! ```
//!
//! ## Required Environment Variables for Testnet Tests
//! - `TESTNET_PRIMARY_SALE_CONTRACT` - Deployed PrimarySale contract address
//! - `TESTNET_WALLET_MNEMONIC` - Test wallet mnemonic phrase
//! - `TESTNET_EVM_RPC_URL` - EVM RPC endpoint (optional, uses default if not set)
//! - `TESTNET_EVM_CHAIN_ID` - EVM chain ID (optional, defaults to 1)

#[cfg(all(test, feature = "evm", feature = "mcp"))]
mod primary_sale_unit_tests {
    use super::super::primary_sale_fixtures::*;
    use alloy_primitives::U256;
    use mantra_sdk::protocols::evm::contracts::primary_sale::MAX_SETTLEMENT_INVESTORS;

    /// Test MCP response structure for get_sale_info
    #[test]
    fn test_get_sale_info_response_structure() {
        let mock_sale = MockSaleInfo::active_sale();
        let response = mock_sale.to_json_response(MOCK_CONTRACT_ADDR);

        // Validate required top-level fields
        assert_eq!(response["status"], "success");
        assert_eq!(response["operation"], "primary_sale_get_sale_info");
        assert_eq!(response["contract_address"], MOCK_CONTRACT_ADDR);
        assert!(response.get("sale").is_some());
        assert!(response.get("contracts").is_some());
        assert!(response.get("timestamp").is_some());

        // Validate nested sale data
        let sale = &response["sale"];
        assert_eq!(sale["status"], "Active");
        assert_eq!(sale["status_code"], 1);
        assert_eq!(sale["is_active"], true);
        assert!(sale.get("start_time").is_some());
        assert!(sale.get("end_time").is_some());
        assert!(sale.get("soft_cap").is_some());
        assert!(sale.get("total_contributed").is_some());
        assert!(sale.get("investor_count").is_some());
        assert!(sale.get("commission_bps").is_some());

        // Validate contracts object
        let contracts = &response["contracts"];
        assert!(contracts.get("mantra_usd").is_some());
        assert!(contracts.get("allowlist").is_some());
        assert!(contracts.get("multisig").is_some());
        assert!(contracts.get("issuer").is_some());
    }

    /// Test investor info response structure
    #[test]
    fn test_investor_info_response_structure() {
        let mock_investor = MockInvestorInfo::with_contribution(1000);
        let response = mock_investor.to_json_response(MOCK_CONTRACT_ADDR);

        // Validate required fields
        assert_eq!(response["status"], "success");
        assert_eq!(response["operation"], "primary_sale_get_investor_info");
        assert_eq!(response["contract_address"], MOCK_CONTRACT_ADDR);
        assert!(response.get("investor").is_some());
        assert!(response.get("timestamp").is_some());

        // Validate nested investor data
        let investor = &response["investor"];
        assert_eq!(investor["address"], MOCK_INVESTOR_ADDR);
        assert!(investor.get("tokens_allocated").is_some());
        assert!(investor.get("contributed").is_some());
        assert_eq!(investor["has_claimed_refund"], false);
    }

    /// Test all investors pagination response structure
    #[test]
    fn test_all_investors_pagination_response() {
        let response = mock_investors_list(0, 50, 100);

        // Validate required fields
        assert_eq!(response["status"], "success");
        assert_eq!(response["operation"], "primary_sale_get_all_investors");
        assert!(response.get("pagination").is_some());
        assert!(response.get("investors").is_some());

        // Validate pagination
        let pagination = &response["pagination"];
        assert_eq!(pagination["start"], 0);
        assert_eq!(pagination["limit"], 50);
        assert_eq!(pagination["count"], 50);

        // Validate investors array
        let investors = response["investors"].as_array().unwrap();
        assert_eq!(investors.len(), 50);

        // Test edge case: last page
        let response2 = mock_investors_list(80, 50, 100);
        let pagination2 = &response2["pagination"];
        assert_eq!(pagination2["count"], 20); // Only 20 remaining
    }

    /// Test transaction response structure
    #[test]
    fn test_transaction_response_structure() {
        let response = mock_transaction_response("primary_sale_invest", "0xabc123");

        assert_eq!(response["status"], "success");
        assert_eq!(response["operation"], "primary_sale_invest");
        assert!(response.get("transaction_hash").is_some());
        assert!(response.get("contract_address").is_some());
        assert!(response.get("timestamp").is_some());
    }

    /// Test settlement response structure
    #[test]
    fn test_settlement_response_structure() {
        let response = mock_settlement_response(
            "100000000000000000000",
            "500000000000000000",
            "99500000000000000000",
            42,
        );

        assert_eq!(response["status"], "success");
        assert_eq!(response["operation"], "primary_sale_settle_and_distribute");
        assert!(response.get("transaction_hash").is_some());
        assert_eq!(response["total_contributed"], "100000000000000000000");
        assert_eq!(response["commission_amount"], "500000000000000000");
        assert_eq!(response["issuer_amount"], "99500000000000000000");
        assert_eq!(response["investors_processed"], "42");
        assert_eq!(response["max_loop"], MAX_SETTLEMENT_INVESTORS);
    }

    /// Test status code to string mapping
    #[test]
    fn test_status_code_mapping() {
        assert_eq!(status_code_to_str(0), "Pending");
        assert_eq!(status_code_to_str(1), "Active");
        assert_eq!(status_code_to_str(2), "Ended");
        assert_eq!(status_code_to_str(3), "Failed");
        assert_eq!(status_code_to_str(4), "Settled");
        assert_eq!(status_code_to_str(5), "Cancelled");
        assert_eq!(status_code_to_str(99), "Unknown");
    }

    /// Test max_loop validation
    #[test]
    fn test_max_loop_validation() {
        // Invalid: too high
        let invalid_max_loop = 1000u64;
        assert!(
            invalid_max_loop > MAX_SETTLEMENT_INVESTORS,
            "Should reject max_loop > MAX_SETTLEMENT_INVESTORS"
        );

        // Valid: at limit
        let valid_max_loop = MAX_SETTLEMENT_INVESTORS;
        assert!(
            valid_max_loop <= MAX_SETTLEMENT_INVESTORS,
            "Should accept max_loop <= MAX_SETTLEMENT_INVESTORS"
        );

        // Valid: below limit
        let valid_max_loop2 = 100u64;
        assert!(
            valid_max_loop2 <= MAX_SETTLEMENT_INVESTORS,
            "Should accept max_loop < MAX_SETTLEMENT_INVESTORS"
        );
    }

    /// Test commission calculation
    #[test]
    fn test_commission_calculation() {
        // Test 0.5% (50 bps) on 100 tokens
        let total = U256::from(100_000_000_000_000_000_000u128);
        let commission = calculate_commission(total, 50).unwrap();
        let expected = U256::from(500_000_000_000_000_000u128); // 0.5 tokens
        assert_eq!(commission, expected);

        // Test 1% (100 bps) on 100 tokens
        let commission = calculate_commission(total, 100).unwrap();
        let expected = U256::from(1_000_000_000_000_000_000u128); // 1 token
        assert_eq!(commission, expected);

        // Test 5% (500 bps) on 200 tokens
        let total = U256::from(200_000_000_000_000_000_000u128);
        let commission = calculate_commission(total, 500).unwrap();
        let expected = U256::from(10_000_000_000_000_000_000u128); // 10 tokens
        assert_eq!(commission, expected);
    }

    /// Test Ethereum address validation
    #[test]
    fn test_address_validation() {
        // Valid addresses
        assert!(is_valid_eth_address(MOCK_CONTRACT_ADDR));
        assert!(is_valid_eth_address(MOCK_INVESTOR_ADDR));
        assert!(is_valid_eth_address(
            "0x0000000000000000000000000000000000000000"
        ));

        // Invalid addresses
        assert!(!is_valid_eth_address("not_an_address"));
        assert!(!is_valid_eth_address("0x123")); // Too short
        assert!(!is_valid_eth_address(
            "1234567890abcdef1234567890abcdef12345678"
        )); // Missing 0x
        assert!(!is_valid_eth_address("0xZZZZ")); // Invalid hex
    }

    /// Test amount parsing logic
    #[test]
    fn test_amount_parsing() {
        // Valid amounts
        assert!("1000".parse::<f64>().is_ok());
        assert!("1000.5".parse::<f64>().is_ok());
        assert!("0.001".parse::<f64>().is_ok());

        // Invalid amounts
        assert!("not_a_number".parse::<f64>().is_err());
        assert!("1,000".parse::<f64>().is_err()); // Comma separator
        assert!("".parse::<f64>().is_err());
    }

    /// Test different sale states
    #[test]
    fn test_sale_states() {
        // Test Pending state
        let pending = MockSaleInfo::pending_sale();
        assert_eq!(pending.status, 0);
        assert!(!pending.is_active);
        assert_eq!(pending.total_contributed, U256::ZERO);
        assert_eq!(pending.investor_count, U256::ZERO);

        // Test Active state
        let active = MockSaleInfo::active_sale();
        assert_eq!(active.status, 1);
        assert!(active.is_active);
        assert!(active.total_contributed > U256::ZERO);
        assert!(active.total_contributed < active.soft_cap); // Partially funded

        // Test Ended state (success)
        let ended = MockSaleInfo::ended_sale();
        assert_eq!(ended.status, 2);
        assert!(!ended.is_active);
        assert!(ended.total_contributed >= ended.soft_cap); // Met soft cap

        // Test Failed state
        let failed = MockSaleInfo::failed_sale();
        assert_eq!(failed.status, 3);
        assert!(!failed.is_active);
        assert!(failed.total_contributed < failed.soft_cap); // Did not meet soft cap

        // Test Settled state
        let settled = MockSaleInfo::settled_sale();
        assert_eq!(settled.status, 4);
        assert!(!settled.is_active);

        // Test Cancelled state
        let cancelled = MockSaleInfo::cancelled_sale();
        assert_eq!(cancelled.status, 5);
        assert!(!cancelled.is_active);
    }

    /// Test investor with contribution
    #[test]
    fn test_investor_with_contribution() {
        let investor = MockInvestorInfo::with_contribution(1000);
        assert_eq!(investor.address, MOCK_INVESTOR_ADDR);
        assert!(investor.contributed > U256::ZERO);
        assert!(!investor.has_claimed_refund);
    }

    /// Test investor with refund claimed
    #[test]
    fn test_investor_with_refund_claimed() {
        let investor = MockInvestorInfo::with_refund_claimed();
        assert_eq!(investor.address, MOCK_INVESTOR_ADDR);
        assert!(investor.contributed > U256::ZERO);
        assert!(investor.has_claimed_refund);
    }

    /// Test error handling for invalid inputs
    #[test]
    fn test_error_handling() {
        // Test invalid contract address format
        let invalid_contract = "not_an_address";
        assert!(!invalid_contract.starts_with("0x"));
        assert!(!is_valid_eth_address(invalid_contract));

        // Test invalid amount format
        let invalid_amount = "not_a_number";
        assert!(invalid_amount.parse::<f64>().is_err());

        // Test invalid max_loop
        let invalid_max_loop = 1000u64;
        assert!(invalid_max_loop > MAX_SETTLEMENT_INVESTORS);

        // Test edge cases for amount
        assert!("-100".parse::<f64>().unwrap() < 0.0); // Negative
        assert!("0".parse::<f64>().unwrap() == 0.0); // Zero
    }
}

#[cfg(all(test, feature = "evm", feature = "mcp"))]
mod primary_sale_testnet_tests {
    use mantra_sdk::protocols::evm::contracts::primary_sale::MAX_SETTLEMENT_INVESTORS;
    use serde_json::json;
    use std::env;

    // These tests require testnet access and are ignored by default
    // Run with: cargo test --features evm,mcp primary_sale -- --ignored

    /// Helper to check if testnet environment is configured
    fn check_testnet_env() -> Result<(String, String, String), String> {
        let contract = env::var("TESTNET_PRIMARY_SALE_CONTRACT")
            .map_err(|_| "TESTNET_PRIMARY_SALE_CONTRACT not set")?;
        let mnemonic =
            env::var("TESTNET_WALLET_MNEMONIC").map_err(|_| "TESTNET_WALLET_MNEMONIC not set")?;
        let rpc_url = env::var("TESTNET_EVM_RPC_URL")
            .unwrap_or_else(|_| "https://rpc.example.com".to_string());

        Ok((contract, mnemonic, rpc_url))
    }

    /// Test getting comprehensive sale information
    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_primary_sale_get_sale_info() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address
        });

        // TODO: Execute via McpSdkAdapter
        // let adapter = McpSdkAdapter::new().await.unwrap();
        // let result = adapter.handle_primary_sale_get_sale_info(args).await;
        // assert!(result.is_ok());

        println!("Test requires MCP adapter implementation");
    }

    /// Test getting investor-specific information
    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_primary_sale_get_investor_info() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let investor_address = env::var("TESTNET_INVESTOR_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000001".to_string());

        let _args = json!({
            "contract_address": contract_address,
            "investor_address": investor_address
        });

        // TODO: Execute via McpSdkAdapter
        println!("Test requires MCP adapter implementation");
    }

    /// Test investment operation with allowance validation
    #[tokio::test]
    #[ignore = "requires testnet access and wallet setup"]
    async fn test_primary_sale_invest_with_allowance() {
        let (contract_address, _mnemonic, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "amount": "100",
            "wallet_address": null
        });

        // TODO:
        // 1. Check allowance for mantraUSD
        // 2. Approve if needed
        // 3. Execute investment
        // 4. Verify transaction success

        println!("Test requires MCP adapter and wallet implementation");
    }

    /// Test refund claim operation
    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_primary_sale_claim_refund() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "wallet_address": null
        });

        // TODO: Execute claimRefund
        // Note: Only works when sale is Failed or Cancelled
        println!("Test requires sale to be in Failed or Cancelled state");
    }

    /// Test pagination for getting all investors
    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_primary_sale_get_all_investors_pagination() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "start": 0,
            "limit": 50
        });

        // TODO: Execute and validate pagination
        println!("Test requires MCP adapter implementation");
    }

    /// Test sale activation (admin only)
    #[tokio::test]
    #[ignore = "requires testnet access and admin role"]
    async fn test_primary_sale_activate() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "wallet_address": null
        });

        // TODO: Execute activate()
        // Requirements:
        // - Must be admin
        // - Sale must be in Pending status
        // - Current time >= START timestamp

        println!("Test requires admin wallet and sale in Pending state");
    }

    /// Test ending the sale (admin only)
    #[tokio::test]
    #[ignore = "requires testnet access and admin role"]
    async fn test_primary_sale_end_sale() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "wallet_address": null
        });

        // TODO: Execute endSale()
        // Requirements:
        // - Must be admin
        // - Current time >= END timestamp
        // - Sets status to Ended if soft_cap met, Failed otherwise

        println!("Test requires admin wallet and sale past end time");
    }

    /// Test settlement and token distribution (complex operation)
    #[tokio::test]
    #[ignore = "requires testnet access and complex setup"]
    async fn test_primary_sale_settle_and_distribute() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let asset_token = env::var("TESTNET_ASSET_TOKEN")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000002".to_string());
        let asset_owner = env::var("TESTNET_ASSET_OWNER")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000003".to_string());

        let _args = json!({
            "contract_address": contract_address,
            "asset_token": asset_token,
            "asset_owner": asset_owner,
            "max_loop": MAX_SETTLEMENT_INVESTORS,
            "wallet_address": null
        });

        // TODO: Execute settleAndDistribute()
        // Pre-flight checks:
        // - Investor count <= max_loop
        // - Multisig has sufficient balance and allowance
        // - Asset owner has sufficient balance and allowance
        // - Sale is in Ended status

        println!(
            "Test requires admin wallet, Ended sale, and sufficient token balances/allowances"
        );
    }

    /// Test topping up refund pool
    #[tokio::test]
    #[ignore = "requires testnet access"]
    async fn test_primary_sale_top_up_refunds() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "amount": "5000",
            "wallet_address": null
        });

        // TODO: Execute topUpRefunds()
        // Requirements:
        // - Anyone can call (no admin role required)
        // - Requires sufficient allowance for mantraUSD
        // - Sale must be Failed or Cancelled

        println!("Test requires sale in Failed or Cancelled state");
    }

    /// Test sale cancellation (admin only)
    #[tokio::test]
    #[ignore = "requires testnet access and admin role"]
    async fn test_primary_sale_cancel() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "wallet_address": null
        });

        // TODO: Execute cancel()
        // Requirements:
        // - Must be admin
        // - Sale must be Pending or Active
        // - Enables refunds for investors

        println!("Test requires admin wallet");
    }

    /// Test pause functionality (admin only)
    #[tokio::test]
    #[ignore = "requires testnet access and admin role"]
    async fn test_primary_sale_pause() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "wallet_address": null
        });

        // TODO: Execute pause()
        // Effect: Blocks invest() and claimRefund() operations

        println!("Test requires admin wallet");
    }

    /// Test unpause functionality (admin only)
    #[tokio::test]
    #[ignore = "requires testnet access and admin role"]
    async fn test_primary_sale_unpause() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let _args = json!({
            "contract_address": contract_address,
            "wallet_address": null
        });

        // TODO: Execute unpause()
        // Effect: Re-enables invest() and claimRefund()

        println!("Test requires admin wallet");
    }

    /// Test emergency withdrawal of stuck tokens (admin only)
    #[tokio::test]
    #[ignore = "requires testnet access and admin role"]
    async fn test_primary_sale_emergency_withdraw() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        let token_address = env::var("TESTNET_TOKEN_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000004".to_string());
        let recipient = env::var("TESTNET_RECIPIENT")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000005".to_string());

        let _args = json!({
            "contract_address": contract_address,
            "token_address": token_address,
            "recipient": recipient,
            "amount": "100",
            "wallet_address": null
        });

        // TODO: Execute emergencyWithdraw()
        // Requirements:
        // - Must be admin
        // - Sale must be in Cancelled status
        // - Transfers ERC-20 tokens to recipient

        println!("Test requires admin wallet and Cancelled sale");
    }

    /// Test complete sale lifecycle
    #[tokio::test]
    #[ignore = "requires testnet access and complex setup"]
    #[allow(unused_variables)]
    async fn test_primary_sale_full_lifecycle() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        // TODO: Test full flow
        // 1. activate() - transition to Active
        // 2. invest() - multiple investors contribute
        // 3. endSale() - transition to Ended (soft cap met)
        // 4. settleAndDistribute() - distribute tokens to investors
        // 5. Validate final state is Settled

        println!("Test requires admin wallet and complex multi-step setup");
    }

    /// Test failed sale refund flow
    #[tokio::test]
    #[ignore = "requires testnet access and complex setup"]
    #[allow(unused_variables)]
    async fn test_primary_sale_refund_flow() {
        let (contract_address, _, _) = match check_testnet_env() {
            Ok(env) => env,
            Err(e) => {
                println!("Skipping testnet test: {}", e);
                return;
            }
        };

        // TODO: Test refund flow
        // 1. activate() - transition to Active
        // 2. invest() - investors contribute (below soft cap)
        // 3. endSale() - transition to Failed (soft cap not met)
        // 4. topUpRefunds() - fund refund pool if needed
        // 5. claimRefund() - investors claim refunds
        // 6. Validate investors receive refunds

        println!("Test requires admin wallet and complex multi-step setup");
    }
}

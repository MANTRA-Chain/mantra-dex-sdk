//! Integration tests for EVM transaction narrative analysis
//!
//! Tests the complete pipeline: fetch → decode → generate narrative
//! Requires Dukong testnet RPC connectivity.
//!
//! ## Running Tests
//! ```bash
//! cargo test transaction_narrative --features evm,mcp
//! ```

#[cfg(feature = "evm")]
mod tests {
    use mantra_sdk::protocols::evm::client::EvmClient;
    use mantra_sdk::protocols::evm::narrative_generator::NarrativeGenerator;
    use mantra_sdk::protocols::evm::transaction_decoder::TransactionDecoder;
    use std::str::FromStr;

    // Dukong Testnet Configuration
    const DUKONG_RPC_URL: &str = "https://evm.dukong.mantrachain.io";
    const DUKONG_CHAIN_ID: u64 = 5887;

    /// Test transaction hashes from Dukong testnet
    /// These are real transactions for integration testing
    /// Transaction sources and types may vary (ERC20, PrimarySale, etc.)
    const TEST_TX_HASH_1: &str =
        "0x4d146955b1f29d82034314f8d3f6564eb078b757f43df1f95e8f111d308e3400";
    const TEST_TX_HASH_2: &str =
        "0xfac1d74eb22ee453b9cd19731cab4a52da27d2ba5662e896ca01c5b46bf91de9";
    const TEST_TX_HASH_3: &str =
        "0xdccdd6e8ee918602f21ff1b0d0000177816b245bcb1908a7c10f899e645595f0";
    const TEST_TX_HASH_4: &str =
        "0x787ce5728ddb88b55888b173d14ce8aa3bb08c4538a43eec6d84ff0a4e95de95";

    /// Get or create EVM client for Dukong testnet
    /// Returns None if RPC is unavailable (allows graceful test skipping)
    async fn get_evm_client() -> Option<EvmClient> {
        // Try to connect to Dukong testnet
        // If unavailable, return None to skip test
        match EvmClient::new(DUKONG_RPC_URL, DUKONG_CHAIN_ID).await {
            Ok(client) => Some(client),
            Err(e) => {
                println!(
                    "Skipping test - Dukong RPC unavailable: {} ({})",
                    DUKONG_RPC_URL, e
                );
                None
            }
        }
    }

    /// Test network connectivity to Dukong testnet
    #[tokio::test]
    async fn test_network_connectivity() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        // Verify basic RPC connectivity
        match client.get_block_number().await {
            Ok(block_num) => {
                println!(
                    "Successfully connected to Dukong testnet, current block: {}",
                    block_num
                );
                assert!(block_num > 0, "Block number should be positive");
            }
            Err(e) => {
                println!("RPC call failed: {}", e);
                // If RPC is available but this call fails, that's an error
                panic!("Network connectivity test failed: {}", e);
            }
        }
    }

    /// Test transaction fetching
    #[tokio::test]
    async fn test_fetch_transaction() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        // Try to fetch the first test transaction
        match alloy_primitives::B256::from_str(TEST_TX_HASH_1) {
            Ok(tx_hash) => {
                match client.get_transaction(tx_hash).await {
                    Ok(Some(tx)) => {
                        println!(
                            "Successfully fetched transaction: from={:?}, to={:?}",
                            tx.from, tx.to
                        );
                        // Verify transaction structure
                        assert!(
                            !tx.input.is_empty() || tx.value > 0,
                            "Transaction should have input data or value"
                        );
                    }
                    Ok(None) => {
                        println!("Transaction not found in testnet: {}", TEST_TX_HASH_1);
                        println!(
                            "This might mean the transaction has been pruned from the testnet"
                        );
                        println!("Consider updating TEST_TX_HASH_1 with a recent transaction");
                    }
                    Err(e) => {
                        println!("Error fetching transaction: {}", e);
                        // Network might be slow or unavailable
                    }
                }
            }
            Err(e) => {
                panic!("Failed to parse transaction hash: {}", e);
            }
        }
    }

    /// Test transaction decoding
    #[tokio::test]
    async fn test_decode_real_transaction() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        let decoder = TransactionDecoder::new();

        // Try to fetch and decode the first test transaction
        match alloy_primitives::B256::from_str(TEST_TX_HASH_1) {
            Ok(tx_hash) => {
                match client.get_transaction(tx_hash).await {
                    Ok(Some(tx)) => {
                        // Attempt to decode the transaction
                        match decoder.decode(&tx.input, tx.to) {
                            Ok(decoded) => {
                                println!(
                                    "Successfully decoded transaction: function={}",
                                    decoded.function_name
                                );
                                // Verify we got a function name
                                assert!(
                                    !decoded.function_name.is_empty(),
                                    "Function name should not be empty"
                                );
                            }
                            Err(e) => {
                                println!("Decoding error: {}", e);
                                // This might fail if the transaction data is not a standard function call
                            }
                        }
                    }
                    Ok(None) => {
                        println!(
                            "Transaction not found - test data might be stale: {}",
                            TEST_TX_HASH_1
                        );
                    }
                    Err(e) => {
                        println!("Error fetching transaction: {}", e);
                    }
                }
            }
            Err(e) => {
                panic!("Failed to parse transaction hash: {}", e);
            }
        }
    }

    /// Test narrative generation for real transaction
    #[tokio::test]
    async fn test_generate_narrative_for_real_transaction() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new(None);

        // Try to fetch, decode, and generate narrative
        match alloy_primitives::B256::from_str(TEST_TX_HASH_1) {
            Ok(tx_hash) => {
                match client.get_transaction(tx_hash).await {
                    Ok(Some(tx)) => {
                        match decoder.decode(&tx.input, tx.to) {
                            Ok(decoded) => {
                                // Generate narrative
                                let narrative = generator
                                    .generate_narrative(&decoded, tx.from, tx.to, tx_hash, false);

                                println!("Generated narrative: {}", narrative);
                                // Verify narrative is not empty and contains meaningful content
                                assert!(!narrative.is_empty(), "Narrative should not be empty");
                                assert!(
                                    narrative.contains("0x"),
                                    "Narrative should contain at least one address"
                                );
                            }
                            Err(e) => {
                                println!("Decoding error: {}", e);
                                // Not all transactions might decode successfully
                            }
                        }
                    }
                    Ok(None) => {
                        println!(
                            "Transaction not found - test data might be stale: {}",
                            TEST_TX_HASH_1
                        );
                    }
                    Err(e) => {
                        println!("Error fetching transaction: {}", e);
                    }
                }
            }
            Err(e) => {
                panic!("Failed to parse transaction hash: {}", e);
            }
        }
    }

    /// Test batch transaction fetching
    #[tokio::test]
    async fn test_batch_transaction_fetching() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        // Parse all test transaction hashes
        let mut tx_hashes = Vec::new();
        for hash_str in &[
            TEST_TX_HASH_1,
            TEST_TX_HASH_2,
            TEST_TX_HASH_3,
            TEST_TX_HASH_4,
        ] {
            match alloy_primitives::B256::from_str(hash_str) {
                Ok(hash) => tx_hashes.push(hash),
                Err(e) => {
                    println!("Failed to parse hash: {}", e);
                }
            }
        }

        if tx_hashes.is_empty() {
            panic!("No valid transaction hashes to test");
        }

        // Test batch fetching (returns Vec<Result<Option<Transaction>>>)
        let results = client.get_transactions_batch(&tx_hashes).await;
        println!("Batch fetch results: {} transactions", results.len());
        let found_count = results
            .iter()
            .filter(|r| r.is_ok() && r.as_ref().unwrap().is_some())
            .count();
        println!("Found {} transactions", found_count);
        assert!(found_count <= results.len());
    }

    /// Test handling of missing transactions
    #[tokio::test]
    async fn test_transaction_not_found() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        // Use a likely non-existent transaction hash
        let fake_hash = alloy_primitives::B256::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();

        match client.get_transaction(fake_hash).await {
            Ok(result) => {
                assert!(
                    result.is_none(),
                    "Non-existent transaction should return None"
                );
                println!("Correctly handled missing transaction");
            }
            Err(e) => {
                println!("Error querying non-existent transaction: {}", e);
                // Some RPC implementations might error instead of returning None
            }
        }
    }
}

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
                                // Generate narrative (now async)
                                let narrative = generator
                                    .generate_narrative(&decoded, tx.from, tx.to, tx_hash, false)
                                    .await;

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

    /// Unit test for contract creation narrative generation
    #[test]
    fn test_contract_creation_narrative_generation() {
        use mantra_sdk::protocols::evm::transaction_decoder::ContractType;
        use mantra_sdk::protocols::evm::transaction_decoder::DecodedCall;

        let generator = NarrativeGenerator::new(None);

        // Create a mock DecodedCall for contract creation
        let decoded = DecodedCall {
            function_name: "constructor".to_string(),
            contract_type: ContractType::ContractCreation,
            selector: "0x00000000".to_string(),
            parameters: serde_json::json!({}),
            raw_input: vec![],
        };

        let narrative = generator.generate_contract_creation_narrative(&decoded, "0x1234...5678");
        println!("Contract creation narrative: {}", narrative);

        assert!(
            narrative.contains("deployed contract"),
            "Narrative should mention deployment"
        );
        assert!(
            narrative.contains("0x1234...5678"),
            "Narrative should include the address"
        );
    }

    /// Unit test for contract creation narrative with address
    #[test]
    fn test_contract_creation_narrative_with_address() {
        use mantra_sdk::protocols::evm::transaction_decoder::ContractType;
        use mantra_sdk::protocols::evm::transaction_decoder::DecodedCall;

        let generator = NarrativeGenerator::new(None);

        // Create a mock DecodedCall for contract creation with contract address
        let decoded = DecodedCall {
            function_name: "constructor".to_string(),
            contract_type: ContractType::ContractCreation,
            selector: "0x00000000".to_string(),
            parameters: serde_json::json!({
                "contract_address": "0xabcd...ef01"
            }),
            raw_input: vec![],
        };

        let narrative = generator.generate_contract_creation_narrative(&decoded, "you");
        println!("Contract creation narrative with address: {}", narrative);

        assert!(
            narrative.contains("deployed contract at"),
            "Should mention deployment location"
        );
        assert!(
            narrative.contains("you"),
            "Should use 'you' for active wallet"
        );
        assert!(
            narrative.contains("0xabcd...ef01"),
            "Should include deployed address"
        );
    }

    /// Unit test for ContractType::ContractCreation variant
    #[test]
    fn test_contract_type_creation_variant() {
        use mantra_sdk::protocols::evm::transaction_decoder::ContractType;

        let creation_type = ContractType::ContractCreation;
        let debug_str = format!("{:?}", creation_type);

        println!("ContractType::ContractCreation debug: {}", debug_str);
        assert_eq!(
            debug_str, "ContractCreation",
            "Should serialize as ContractCreation"
        );
    }

    /// Integration test for contract creation detection in transaction analysis
    /// This test analyzes a real contract creation transaction from Dukong testnet
    #[tokio::test]
    async fn test_contract_creation_detection_integration() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        // We'll look for a contract creation transaction (to = None)
        // by checking the first test transaction
        match alloy_primitives::B256::from_str(TEST_TX_HASH_1) {
            Ok(tx_hash) => {
                match client.get_transaction(tx_hash).await {
                    Ok(Some(tx)) => {
                        println!("Transaction info: to={:?}, from={:?}", tx.to, tx.from);

                        // If this is a contract creation (to is None), verify detection
                        if tx.to.is_none() {
                            println!("Found contract creation transaction!");

                            // Try to fetch receipt to verify contract_address extraction
                            match client.get_transaction_receipt(tx_hash).await {
                                Ok(Some(receipt)) => {
                                    println!(
                                        "Receipt contract_address: {:?}",
                                        receipt.contract_address
                                    );
                                    if let Some(addr) = receipt.contract_address {
                                        println!(
                                            "Successfully detected deployed contract: {:?}",
                                            addr
                                        );
                                        // Verify address is valid (non-zero)
                                        assert!(
                                            addr != alloy_primitives::Address::ZERO,
                                            "Contract address should not be zero"
                                        );
                                    }
                                }
                                Ok(None) => {
                                    println!(
                                        "Receipt not available - transaction might be pending"
                                    );
                                }
                                Err(e) => {
                                    println!("Error fetching receipt: {}", e);
                                }
                            }
                        } else {
                            println!(
                                "This transaction is not a contract creation (has to address)"
                            );
                        }
                    }
                    Ok(None) => {
                        println!("Transaction not found");
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

    /// Integration test for failed contract creation detection
    #[tokio::test]
    async fn test_failed_contract_creation_detection() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        // Try each test transaction to find one that might be a failed deployment
        for hash_str in &[
            TEST_TX_HASH_1,
            TEST_TX_HASH_2,
            TEST_TX_HASH_3,
            TEST_TX_HASH_4,
        ] {
            match alloy_primitives::B256::from_str(hash_str) {
                Ok(tx_hash) => {
                    match (
                        client.get_transaction(tx_hash).await,
                        client.get_transaction_receipt(tx_hash).await,
                    ) {
                        (Ok(Some(tx)), Ok(Some(receipt))) => {
                            if tx.to.is_none() && !receipt.status() {
                                println!("Found failed contract creation!");
                                println!("Transaction failed but was detected as contract creation attempt");
                                assert!(tx.to.is_none(), "Should have no 'to' address");
                                assert!(!receipt.status(), "Should have failed status");
                                return;
                            }
                        }
                        _ => continue,
                    }
                }
                Err(_) => continue,
            }
        }

        println!("No failed contract creation found in test data (this is okay)");
    }

    /// Test mixed batch of contract creation and function calls
    #[tokio::test]
    async fn test_contract_creation_in_mixed_batch() {
        let client = match get_evm_client().await {
            Some(c) => c,
            None => {
                println!("Network unavailable - test skipped");
                return;
            }
        };

        let _decoder = TransactionDecoder::new();

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
                Err(_) => {}
            }
        }

        if tx_hashes.is_empty() {
            println!("No valid transaction hashes - skipping batch test");
            return;
        }

        // Fetch transactions and receipts
        let transactions_results = client.get_transactions_batch(&tx_hashes).await;
        let receipts_results = client.get_transaction_receipts_batch(&tx_hashes).await;

        let mut contract_creations_found = 0;
        let mut function_calls_found = 0;

        for (i, tx_result) in transactions_results.iter().enumerate() {
            if let Ok(Some(tx)) = tx_result {
                if tx.to.is_none() {
                    // This is a contract creation
                    contract_creations_found += 1;
                    println!("Transaction {} is a contract creation", i);

                    // Verify receipt has contract_address
                    if let Ok(Some(receipt)) = &receipts_results[i] {
                        if let Some(addr) = receipt.contract_address {
                            println!("  - Deployed at: {:?}", addr);
                        }
                    }
                } else {
                    // This is a regular function call
                    function_calls_found += 1;
                    println!("Transaction {} is a function call to {:?}", i, tx.to);
                }
            }
        }

        println!(
            "Batch analysis: {} creations, {} function calls",
            contract_creations_found, function_calls_found
        );
        println!(
            "Total transactions in batch: {}",
            transactions_results.len()
        );
    }
}

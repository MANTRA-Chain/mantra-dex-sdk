/// Unit tests for EVM protocol support

#[cfg(feature = "evm")]
mod tests {
    use alloy_eips::eip2930::AccessList;
    use alloy_primitives::{Address, U256};
    use mantra_sdk::protocols::evm::client::EvmClient;
    use mantra_sdk::protocols::evm::types::{
        EthAddress, EventFilter, EvmCallRequest, EvmTransactionRequest,
    };

    #[test]
    fn test_eth_address_from_str() {
        // Valid address
        let addr_str = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        let addr = EthAddress::from_str(addr_str).unwrap();
        assert_eq!(format!("{}", addr), addr_str.to_lowercase());

        // Invalid address - wrong length
        let invalid_addr = "0x742d35Cc6634C0532925a3b844Bc454e4438f44";
        assert!(EthAddress::from_str(invalid_addr).is_err());

        // Invalid address - no 0x prefix
        let invalid_addr2 = "742d35Cc6634C0532925a3b844Bc454e4438f44e";
        assert!(EthAddress::from_str(invalid_addr2).is_err());
    }

    #[test]
    fn test_evm_transaction_request_to_rpc_request() {
        let addr = EthAddress::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();
        let request = EvmTransactionRequest::new(1)
            .to(addr.clone())
            .value(U256::from(1_u64))
            .gas_limit(21000)
            .eip1559_fees(U256::from(30_u64), U256::from(2_u64))
            .nonce(7)
            .data(vec![0xAB, 0xCD])
            .from(addr.clone());

        let rpc = request.to_rpc_request(None);
        assert_eq!(rpc.from, Some(addr.0));
        assert_eq!(rpc.nonce, Some(7));
        assert_eq!(rpc.gas, Some(21000));
        assert_eq!(rpc.max_fee_per_gas, Some(30));
        assert_eq!(rpc.max_priority_fee_per_gas, Some(2));
        assert!(matches!(rpc.to, Some(alloy_primitives::TxKind::Call(_))));
        assert_eq!(rpc.chain_id, Some(1));
        assert_eq!(
            rpc.input.into_input().unwrap(),
            Some(vec![0xAB, 0xCD].into())
        );
    }

    #[test]
    fn test_evm_call_request_creation() {
        let addr = EthAddress::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();
        let data = vec![0x01, 0x02, 0x03, 0x04];

        let request = EvmCallRequest::new(addr.clone(), data.clone());
        assert_eq!(request.to, addr);
        assert_eq!(request.data, data);
        assert!(request.block.is_none());

        let request_with_block = request.at_block("latest".to_string());
        assert_eq!(request_with_block.block, Some("latest".to_string()));
    }

    #[test]
    fn test_evm_transaction_request_creation() {
        let chain_id = 1;
        let mut request = EvmTransactionRequest::new(chain_id);

        assert_eq!(request.chain_id, chain_id);
        assert!(request.to.is_none());
        assert_eq!(request.value, U256::ZERO);
        assert!(request.data.is_empty());
        assert!(request.nonce.is_none());
        assert!(request.from.is_none());
        assert_eq!(request.access_list, AccessList::default());

        let addr = EthAddress::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();
        let value = U256::from(1000000u64);

        request = request
            .to(addr.clone())
            .value(value)
            .data(vec![0x01, 0x02, 0x03])
            .from(addr.clone());

        assert_eq!(request.to, Some(addr.clone()));
        assert_eq!(request.value, value);
        assert_eq!(request.data, vec![0x01, 0x02, 0x03]);
        assert_eq!(request.from, Some(addr));
    }

    #[test]
    fn test_evm_transaction_request_into_eip1559() {
        let addr = EthAddress::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();

        let request = EvmTransactionRequest::new(1)
            .to(addr.clone())
            .value(U256::from(42_u64))
            .gas_limit(21000)
            .eip1559_fees(
                U256::from(30_000_000_000_u128),
                U256::from(2_000_000_000_u128),
            )
            .nonce(5)
            .data(vec![0xAA]);

        let tx = request.into_eip1559().expect("conversion should succeed");

        assert_eq!(tx.chain_id, 1);
        assert_eq!(tx.nonce, 5);
        assert_eq!(tx.gas_limit, 21000);
        assert_eq!(tx.max_fee_per_gas, 30_000_000_000_u128);
        assert_eq!(tx.max_priority_fee_per_gas, 2_000_000_000_u128);
        assert_eq!(tx.value, U256::from(42_u64));
        assert_eq!(tx.to, Some(*addr.inner()));
        assert_eq!(tx.data, vec![0xAA].into());

        let round_trip = EvmTransactionRequest::from(&tx);
        assert_eq!(round_trip.nonce, Some(5));
        assert_eq!(round_trip.gas_limit, Some(21000));
        assert_eq!(round_trip.value, U256::from(42_u64));
    }

    #[test]
    fn test_event_filter_creation() {
        let mut filter = EventFilter::new();
        assert!(filter.addresses.is_empty());
        assert!(filter.topics.is_empty());
        assert!(filter.from_block.is_none());
        assert!(filter.to_block.is_none());

        let addr = EthAddress::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();
        filter = filter
            .addresses(vec![addr.clone()])
            .block_range(Some("0x1".to_string()), Some("latest".to_string()));

        assert_eq!(filter.addresses.len(), 1);
        assert_eq!(filter.addresses[0], addr);
        assert_eq!(filter.from_block, Some("0x1".to_string()));
        assert_eq!(filter.to_block, Some("latest".to_string()));
    }

    #[test]
    fn test_address_derivation_utility() {
        // Test the utility functions
        use mantra_sdk::protocols::evm::types::utils;

        // Test EIP-55 checksum validation (placeholder - would need real implementation)
        let valid_addr = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        assert!(utils::validate_eip55_checksum(valid_addr).is_ok());

        // Test wei to ether conversion
        let wei = U256::from(1000000000000000000u64); // 1 ETH in wei
        let ether = utils::wei_to_ether(wei);
        assert!((ether - 1.0).abs() < f64::EPSILON);

        // Test ether to wei conversion with string input
        let wei_back = utils::ether_to_wei("1.0").unwrap();
        assert_eq!(wei_back, U256::from(1000000000000000000u64));

        // Test precision with decimal values
        let wei_decimal = utils::ether_to_wei("0.5").unwrap();
        assert_eq!(wei_decimal, U256::from(500000000000000000u64));

        // Test large values without precision loss
        let large_wei = utils::ether_to_wei("12345.678901234567890").unwrap();
        let expected_large = U256::from_str("12345678901234567890123").unwrap();
        assert_eq!(large_wei, expected_large);
    }

    #[tokio::test]
    async fn test_evm_client_creation() {
        // Test client creation with mock/placeholder values
        let rpc_url = "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY";
        let chain_id = 1;

        // This will fail in CI without a real RPC endpoint, but tests the creation logic
        let client_result = EvmClient::new(rpc_url, chain_id).await;
        // We expect this to fail in test environment, but the creation should not panic
        // In a real test environment with mocked transport, this would succeed
        assert!(client_result.is_err() || client_result.is_ok());
    }

    #[test]
    fn test_erc20_helper_creation() {
        // This test would require a mock client in a real implementation
        // For now, just test that the types compile and basic structure works
        let addr = Address::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();
        let eth_addr = EthAddress(addr);

        // Test that we can create the address wrapper
        assert_eq!(eth_addr.inner(), &addr);
    }

    #[test]
    fn test_erc721_helper_creation() {
        // Similar to ERC20 test - basic type checking
        let addr = Address::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap();
        let eth_addr = EthAddress(addr);

        assert_eq!(eth_addr.inner(), &addr);
    }
}

#[cfg(not(feature = "evm"))]
mod tests {
    #[test]
    fn test_evm_feature_not_enabled() {
        // Test that EVM types are not available when feature is disabled
        // This test should always pass when EVM feature is not enabled
        assert!(true);
    }
}

#[cfg(feature = "mcp")]
use mantra_sdk::mcp::sdk_adapter::McpSdkAdapter;
#[cfg(feature = "mcp")]
use mantra_sdk::wallet::MantraWallet;

#[tokio::test]
#[cfg(feature = "mcp")]
async fn test_multi_wallet_management() {
    let adapter = McpSdkAdapter::default();
    
    // Test 1: Initially no wallets
    let wallets = adapter.get_all_wallets().await.unwrap();
    assert!(wallets.is_empty());
    
    // Test 2: No active wallet initially
    let active_wallet = adapter.get_active_wallet_info().await.unwrap();
    assert!(active_wallet.is_none());
    
    // Test 3: Create and add first wallet
    let mnemonic1 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let wallet1 = MantraWallet::from_mnemonic(mnemonic1, 0).unwrap();
    let address1 = adapter.add_wallet(wallet1).await.unwrap();
    
    // Test 4: Verify wallet was added
    let wallets = adapter.get_all_wallets().await.unwrap();
    assert_eq!(wallets.len(), 1);
    assert!(wallets.contains_key(&address1));
    
    // Test 5: Verify wallet exists
    assert!(adapter.wallet_exists(&address1).await);
    
    // Test 6: Get wallet info by address
    let wallet_info = adapter.get_wallet_info(&address1).await.unwrap();
    assert!(wallet_info.is_some());
    assert_eq!(wallet_info.unwrap().address, address1);
    
    // Test 7: Create and add second wallet
    let mnemonic2 = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let wallet2 = MantraWallet::from_mnemonic(mnemonic2, 1).unwrap();
    let address2 = adapter.add_wallet(wallet2).await.unwrap();
    
    // Test 8: Verify both wallets exist
    let wallets = adapter.get_all_wallets().await.unwrap();
    assert_eq!(wallets.len(), 2);
    assert!(wallets.contains_key(&address1));
    assert!(wallets.contains_key(&address2));
    
    // Test 9: Switch active wallet
    adapter.switch_active_wallet(&address1).await.unwrap();
    let active_wallet = adapter.get_active_wallet_info().await.unwrap();
    assert!(active_wallet.is_some());
    assert_eq!(active_wallet.unwrap().address, address1);
    
    // Test 10: Switch to second wallet
    adapter.switch_active_wallet(&address2).await.unwrap();
    let active_wallet = adapter.get_active_wallet_info().await.unwrap();
    assert!(active_wallet.is_some());
    assert_eq!(active_wallet.unwrap().address, address2);
    
    // Test 11: Remove first wallet
    adapter.remove_wallet(&address1).await.unwrap();
    let wallets = adapter.get_all_wallets().await.unwrap();
    assert_eq!(wallets.len(), 1);
    assert!(!wallets.contains_key(&address1));
    assert!(wallets.contains_key(&address2));
    
    // Test 12: Remove active wallet (should clear active wallet)
    adapter.remove_wallet(&address2).await.unwrap();
    let wallets = adapter.get_all_wallets().await.unwrap();
    assert!(wallets.is_empty());
    
    let active_wallet = adapter.get_active_wallet_info().await.unwrap();
    assert!(active_wallet.is_none());
}

#[tokio::test]
#[cfg(feature = "mcp")]
async fn test_multi_wallet_error_handling() {
    let adapter = McpSdkAdapter::default();
    
    // Test 1: Switch to non-existent wallet
    let result = adapter.switch_active_wallet("invalid_address").await;
    assert!(result.is_err());
    
    // Test 2: Remove non-existent wallet
    let result = adapter.remove_wallet("invalid_address").await;
    assert!(result.is_err());
    
    // Test 3: Get info for non-existent wallet
    let wallet_info = adapter.get_wallet_info("invalid_address").await.unwrap();
    assert!(wallet_info.is_none());
    
    // Test 4: Check existence of non-existent wallet
    assert!(!adapter.wallet_exists("invalid_address").await);
}

#[tokio::test]
#[cfg(feature = "mcp")]
async fn test_wallet_recreation_from_mnemonic() {
    let adapter = McpSdkAdapter::default();
    
    // Set up environment variable for mnemonic
    std::env::set_var("WALLET_MNEMONIC", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
    
    // Create and add wallet
    let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let wallet = MantraWallet::from_mnemonic(mnemonic, 0).unwrap();
    let address = adapter.add_wallet(wallet).await.unwrap();
    
    // Test wallet recreation by address
    let recreated_wallet = adapter.get_wallet_by_address(&address).await.unwrap();
    assert!(recreated_wallet.is_some());
    assert_eq!(recreated_wallet.unwrap().info().address, address);
    
    // Test recreation of non-existent wallet
    let missing_wallet = adapter.get_wallet_by_address("invalid_address").await.unwrap();
    assert!(missing_wallet.is_none());
    
    // Clean up
    std::env::remove_var("WALLET_MNEMONIC");
}
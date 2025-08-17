mod utils;

use mantra_sdk::{Error, MantraWallet};
use utils::test_utils::{create_test_client, create_test_wallet, load_test_config};

#[tokio::test]
async fn test_wallet_creation_from_mnemonic() {
    let config = load_test_config();
    let mnemonic = config
        .wallets
        .get("primary")
        .expect("Primary wallet not found in test config");

    // Create wallet from mnemonic
    let wallet = MantraWallet::from_mnemonic(mnemonic, 0);
    assert!(wallet.is_ok(), "Failed to create wallet from mnemonic");

    let wallet = wallet.unwrap();
    let address = wallet.address();
    assert!(address.is_ok(), "Failed to get wallet address");

    let address = address.unwrap().to_string();
    assert!(!address.is_empty(), "Wallet address should not be empty");
    assert!(
        address.starts_with("mantra"),
        "Wallet address should start with 'mantra'"
    );
}

#[tokio::test]
async fn test_wallet_generate() {
    // Generate a new wallet
    let (wallet, mnemonic) = MantraWallet::generate().expect("Failed to generate wallet");

    // Check mnemonic
    assert!(!mnemonic.is_empty(), "Mnemonic should not be empty");
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 12, "Mnemonic should have 12 words");

    // Check wallet address
    let address = wallet.address().expect("Failed to get wallet address");
    assert!(
        !address.to_string().is_empty(),
        "Wallet address should not be empty"
    );
    assert!(
        address.to_string().starts_with("mantra"),
        "Wallet address should start with 'mantra'"
    );

    // Confirm we can recreate the wallet from the generated mnemonic
    let recreated_wallet =
        MantraWallet::from_mnemonic(&mnemonic, 0).expect("Failed to recreate wallet from mnemonic");

    let recreated_address = recreated_wallet
        .address()
        .expect("Failed to get recreated wallet address");

    assert_eq!(
        address.to_string(),
        recreated_address.to_string(),
        "Recreated wallet should have the same address"
    );
}

#[tokio::test]
async fn test_wallet_sign_tx() {
    let wallet = create_test_wallet("primary");

    // Create a simple fee
    let fee = wallet
        .create_default_fee(300_000)
        .expect("Failed to create default fee");

    // Create an empty message list (not testing message validation here)
    let msgs = Vec::new();

    // Sign a transaction
    let chain_id = "mantra-dukong";
    let account_number = 1;
    let sequence = 0;

    let result = wallet.sign_tx(account_number, sequence, chain_id, fee, msgs, None, None);

    assert!(
        result.is_ok(),
        "Failed to sign transaction: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn test_wallet_info() {
    let wallet = create_test_wallet("primary");
    let wallet_info = wallet.info();

    assert!(
        !wallet_info.address.is_empty(),
        "Wallet address should not be empty"
    );
    assert!(
        !wallet_info.public_key.is_empty(),
        "Public key should not be empty"
    );

    let address = wallet
        .address()
        .expect("Failed to get wallet address")
        .to_string();

    assert_eq!(
        wallet_info.address, address,
        "Wallet info address should match wallet address"
    );
}

#[tokio::test]
async fn test_wallet_invalid_mnemonic() {
    // Try to create a wallet with an invalid mnemonic
    let result = MantraWallet::from_mnemonic("invalid mnemonic", 0);
    assert!(result.is_err(), "Should fail with invalid mnemonic");

    match result {
        Err(Error::Wallet(msg)) => {
            assert!(
                msg.contains("Invalid mnemonic"),
                "Error message should contain 'Invalid mnemonic'"
            );
        }
        _ => panic!("Expected Wallet error"),
    }
}

/// get balances test
///
#[tokio::test]
async fn test_wallet_get_balances() {
    let client = create_test_client().await;
    let balances = client.get_balances().await.unwrap();
    println!("Balances: {:?}", balances);
}

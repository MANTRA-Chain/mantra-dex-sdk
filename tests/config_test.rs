mod utils;

use mantra_sdk::config::{MantraNetworkConfig, NetworkConstants};
use utils::test_utils::{create_test_network_config, load_contract_addresses, load_test_config};

#[test]
fn test_network_config_loading() {
    // Test loading network constants
    let network_result = NetworkConstants::load("mantra-dukong");
    assert!(network_result.is_ok(), "Failed to load network constants");

    let network = network_result.unwrap();
    assert_eq!(
        network.network_name, "mantra-dukong",
        "Network name should match"
    );
    assert_eq!(
        network.chain_id, "mantra-dukong-1",
        "Network ID should match"
    );
    assert!(
        !network.default_rpc.is_empty(),
        "RPC URL should not be empty"
    );
    assert!(
        network.default_gas_price > 0.0,
        "Gas price should be positive"
    );
    assert!(
        network.default_gas_adjustment > 0.0,
        "Gas adjustment should be positive"
    );
    assert_eq!(network.native_denom, "uom", "Native denom should be uom");
}

#[test]
fn test_contract_addresses_loading() {
    // Test loading contract addresses
    let contract_addresses = load_contract_addresses("mantra-dukong");

    // Print for debugging
    println!("Contract addresses: {:?}", contract_addresses);

    // Check that pool manager is set (required)
    assert!(
        !contract_addresses.pool_manager.is_empty(),
        "Pool manager address should not be empty"
    );
}

#[test]
fn test_test_config_loading() {
    // Test loading test configuration
    let test_config = load_test_config();

    // Check test network
    assert_eq!(
        test_config.test.network, "mantra-dukong",
        "Test network should be mantra-dukong"
    );

    // Check wallets
    assert!(
        test_config.wallets.contains_key("primary"),
        "Primary wallet should exist"
    );
    assert!(
        test_config.wallets.contains_key("secondary"),
        "Secondary wallet should exist"
    );

    // Check tokens
    assert!(
        test_config.tokens.contains_key("uom"),
        "uom token should exist"
    );
    let uom_token = test_config.tokens.get("uom").unwrap();
    assert_eq!(uom_token.name, "OM", "OM token name should match");
    assert_eq!(uom_token.decimals, 6, "OM token decimals should be 6");
}

#[test]
fn test_create_network_config() {
    // Test creating a network config
    let network_config = create_test_network_config();

    // Check config fields
    assert_eq!(
        network_config.network_name, "mantra-dukong",
        "Network name should match"
    );
    assert_eq!(
        network_config.chain_id, "mantra-dukong-1",
        "Network ID should match"
    );
    assert!(
        !network_config.rpc_url.is_empty(),
        "RPC URL should not be empty"
    );
    assert!(
        network_config.gas_price > 0.0,
        "Gas price should be positive"
    );
    assert!(
        network_config.gas_adjustment > 0.0,
        "Gas adjustment should be positive"
    );
    assert_eq!(
        network_config.native_denom, "uom",
        "Native denom should be uom"
    );

    // Check contract addresses
    assert!(
        !network_config.contracts.pool_manager.is_empty(),
        "Pool manager address should not be empty"
    );
}

#[test]
fn test_default_network_config() {
    // Test default network config
    let default_config = MantraNetworkConfig::default();

    // Check default values
    assert_eq!(
        default_config.network_name, "mantra-dukong",
        "Default network name should be mantra-dukong"
    );
    assert_eq!(
        default_config.chain_id, "mantra-dukong-1",
        "Default network ID should be mantra-dukong"
    );
    assert_eq!(
        default_config.rpc_url, "https://rpc.dukong.mantrachain.io:443",
        "Default RPC URL should match"
    );
    assert_eq!(
        default_config.gas_price, 0.01,
        "Default gas price should be 0.01"
    );
    assert_eq!(
        default_config.gas_adjustment, 1.5,
        "Default gas adjustment should be 1.5"
    );
    assert_eq!(
        default_config.native_denom, "uom",
        "Default native denom should be uom"
    );
}

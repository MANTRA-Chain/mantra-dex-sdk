use config::{Config as ConfigLoader, File};
use cosmwasm_std::{Coin, Decimal, Uint128};
use mantra_sdk::{
    config::{ContractAddresses, MantraNetworkConfig, NetworkConstants},
    MantraDexClient, MantraWallet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
pub mod test_utils {
    use super::*;

    /// Test configuration loaded from config/test.toml
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestConfig {
        /// Test settings
        pub test: TestSettings,
        /// Test wallets
        pub wallets: HashMap<String, String>,
        /// Test tokens
        pub tokens: HashMap<String, TestToken>,
    }

    /// Test settings
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestSettings {
        /// Network to use for testing
        pub network: String,
    }

    /// Test token information
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestToken {
        /// Token name
        pub name: String,
        /// Token symbol
        pub symbol: String,
        /// Token decimals
        pub decimals: u8,
        /// Token denom
        pub denom: Option<String>,
    }

    /// Load contract addresses from config/contracts.toml
    pub fn load_contract_addresses(network: &str) -> ContractAddresses {
        let config_dir =
            std::env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let settings = ConfigLoader::builder()
            .add_source(File::with_name(&format!("{}/contracts", config_dir)))
            .build()
            .expect("Failed to load contracts config");

        // First, try to get the nested structure
        let pool_manager = settings.get::<String>(&format!("{}.pool_manager.address", network));
        let farm_manager = settings.get::<String>(&format!("{}.farm_manager.address", network));
        let fee_collector = settings.get::<String>(&format!("{}.fee_collector.address", network));
        let epoch_manager = settings.get::<String>(&format!("{}.epoch_manager.address", network));

        ContractAddresses {
            pool_manager: pool_manager.unwrap_or_default(),
            farm_manager: farm_manager.ok(),
            fee_collector: fee_collector.ok(),
            epoch_manager: epoch_manager.ok(),
            skip_entry_point: None,
            skip_ibc_hooks_adapter: None,
            skip_mantra_dex_adapter: None,
        }
    }

    /// Load test configuration from config/test.toml
    pub fn load_test_config() -> TestConfig {
        let config_dir =
            std::env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let settings = ConfigLoader::builder()
            .add_source(File::with_name(&format!("{}/test", config_dir)))
            .build()
            .expect("Failed to load test config");

        settings
            .try_deserialize::<TestConfig>()
            .expect("Failed to deserialize test config")
    }

    /// Create a network config for testing
    pub fn create_test_network_config() -> MantraNetworkConfig {
        let test_config = load_test_config();
        let network = &test_config.test.network;

        // Load network constants
        let network_constants =
            NetworkConstants::load(network).expect("Failed to load network constants");

        // Create network config from constants
        let mut network_config = MantraNetworkConfig::from_constants(&network_constants)
            .unwrap_or_else(|_| {
                // Fallback to using the network constants with default contract addresses
                MantraNetworkConfig {
                    network_name: network_constants.network_name.clone(),
                    chain_id: network_constants.chain_id.clone(),
                    rpc_url: network_constants.default_rpc.clone(),
                    gas_price: network_constants.default_gas_price,
                    gas_adjustment: network_constants.default_gas_adjustment,
                    native_denom: network_constants.native_denom.clone(),
                    contracts: Default::default(),
                    evm_rpc_url: None,
                    evm_chain_id: None,
                }
            });

        // Load contract addresses
        network_config.contracts = load_contract_addresses(network);

        network_config
    }

    /// Create a client with the primary test wallet
    #[cfg(test)]
    #[allow(dead_code)]
    pub async fn create_test_client() -> MantraDexClient {
        let network_config = create_test_network_config();
        let test_config = load_test_config();

        // Log contract addresses for debugging
        println!(
            "Pool Manager Contract: {}",
            network_config.contracts.pool_manager
        );
        println!(
            "Farm Manager Contract: {:?}",
            network_config.contracts.farm_manager
        );
        println!(
            "Fee Collector Contract: {:?}",
            network_config.contracts.fee_collector
        );
        println!(
            "Epoch Manager Contract: {:?}",
            network_config.contracts.epoch_manager
        );

        // Get the primary wallet mnemonic
        let primary_mnemonic = test_config
            .wallets
            .get("primary")
            .expect("Primary wallet not found in test config");

        // Create wallet
        let wallet = MantraWallet::from_mnemonic(primary_mnemonic, 0)
            .expect("Failed to create wallet from mnemonic");

        println!("Wallet address: {}", wallet.address().unwrap());

        // Create client with wallet
        MantraDexClient::new(network_config)
            .await
            .expect("Failed to create client")
            .with_wallet(wallet)
    }

    /// Create a wallet from the test config
    #[allow(dead_code)]
    pub fn create_test_wallet(wallet_name: &str) -> MantraWallet {
        let test_config = load_test_config();

        // Get the wallet mnemonic
        let mnemonic = test_config
            .wallets
            .get(wallet_name)
            .unwrap_or_else(|| panic!("Wallet '{}' not found in test config", wallet_name));

        // Create wallet
        MantraWallet::from_mnemonic(mnemonic, 0).expect("Failed to create wallet from mnemonic")
    }

    /// Get or create the OM/USDY pool for testing
    #[allow(dead_code)]
    pub async fn get_or_create_test_pool_id(client: &MantraDexClient) -> Option<String> {
        // Try to create or find the test pool
        match create_test_pool_if_needed(client).await {
            Ok(pool_id) => Some(pool_id),
            Err(e) => {
                println!("Failed to create or find test pool: {:?}", e);
                None
            }
        }
    }

    /// Create a test pool with OM and USDY if one doesn't exist
    #[allow(dead_code)]
    pub async fn create_test_pool_if_needed(
        client: &MantraDexClient,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let test_config = load_test_config();
        let uom_denom = test_config
            .tokens
            .get("uom")
            .unwrap()
            .denom
            .clone()
            .unwrap();
        let uusdy_denom = test_config
            .tokens
            .get("uusdy")
            .unwrap()
            .denom
            .clone()
            .unwrap();

        println!(
            "Looking for pool with assets: {} and {}",
            uom_denom, uusdy_denom
        );

        // First, try to find an existing pool
        let pools = client.get_pools(Some(100)).await?;
        for pool in pools {
            if pool.pool_info.assets.iter().any(|a| a.denom == uom_denom)
                && pool.pool_info.assets.iter().any(|a| a.denom == uusdy_denom)
            {
                println!("Found existing pool: {}", pool.pool_info.pool_identifier);
                return Ok(pool.pool_info.pool_identifier);
            }
        }

        // No pool found, create one by providing initial liquidity
        println!("No existing pool found, creating new pool...");

        // Create a unique pool identifier
        // Generate a simple, valid pool ID (only alphanumeric, dots, and slashes allowed)
        let pool_id = "uom.usdy.pool".to_string();
        println!("Creating pool with ID: {}", pool_id);

        // First create the pool
        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::percent(1), // 1% protocol fee
            },
            swap_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::percent(2), // 2% swap fee
            },
            burn_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::zero(), // 0% burn fee
            },
            extra_fees: vec![], // No extra fees
        };

        let pool_type = mantra_dex_std::pool_manager::PoolType::ConstantProduct {};

        println!("Creating pool with fees: {:?}", pool_fees);

        let create_result = client
            .create_pool(
                vec![uom_denom.clone(), uusdy_denom.clone()],
                vec![6, 6], // Both tokens have 6 decimals
                pool_fees,
                pool_type,
                Some(pool_id.clone()),
            )
            .await?;

        println!("Pool created successfully: {:?}", create_result.txhash);

        // The contract adds "o." prefix to custom pool identifiers
        let actual_pool_id = format!("o.{}", pool_id);
        println!("Actual pool ID with prefix: {}", actual_pool_id);

        // Now provide initial liquidity to the newly created pool
        let initial_assets = vec![
            Coin {
                denom: uom_denom.clone(),
                amount: Uint128::new(1_000_000), // 1 OM (6 decimals)
            },
            Coin {
                denom: uusdy_denom.clone(),
                amount: Uint128::new(4_000_000), // 4 USDY (6 decimals)
            },
        ];

        println!(
            "Providing initial liquidity with assets: {:?}",
            initial_assets
        );

        let tx_result = client
            .provide_liquidity_unchecked(
                &actual_pool_id,
                initial_assets,
                Some(Decimal::percent(5)), // 5% liquidity max slippage
                Some(Decimal::percent(5)), // 5% swap max slippage
            )
            .await?;

        println!(
            "Pool created successfully! Transaction hash: {}",
            tx_result.txhash
        );
        Ok(actual_pool_id)
    }

    /// Check if we should execute write operations (create pools, swaps, etc.)
    #[allow(dead_code)]
    pub fn should_execute_writes() -> bool {
        std::env::var("EXECUTE_WRITES")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true"
    }
}

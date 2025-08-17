//! Configuration management for the Mantra DEX SDK
//!
//! This module provides both legacy configuration support and the new modular
//! configuration system for comprehensive management of contracts, protocols, and environment.

// Modular configuration system
pub mod contracts;
pub mod protocols;
pub mod env;

// Re-export key types from modular system
pub use contracts::{ContractInfo, ContractRegistry, ContractType, NetworkContracts};
pub use protocols::{
    FeeConfig, HealthConfig, ProtocolConfig, ProtocolId, ProtocolParameters,
    ProtocolRegistry, RateLimitConfig
};
pub use env::{EnvironmentConfig, LoggingEnvConfig, McpEnvConfig, NetworkEnvConfig};

// Legacy configuration types and functions for backward compatibility
use config::{Config as ConfigLoader, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env as std_env;
use std::fs;
use std::path::PathBuf;

use crate::error::Error;

/// Legacy contract address configuration for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractAddresses {
    /// Pool manager contract address
    pub pool_manager: String,
    /// Farm manager contract address
    pub farm_manager: Option<String>,
    /// Fee collector contract address
    pub fee_collector: Option<String>,
    /// Epoch manager contract address
    pub epoch_manager: Option<String>,
    /// Skip Adapter contracts
    pub skip_entry_point: Option<String>,
    pub skip_ibc_hooks_adapter: Option<String>,
    pub skip_mantra_dex_adapter: Option<String>,
}

impl Default for ContractAddresses {
    fn default() -> Self {
        Self {
            pool_manager: "".to_string(),
            farm_manager: None,
            fee_collector: None,
            epoch_manager: None,
            skip_entry_point: None,
            skip_ibc_hooks_adapter: None,
            skip_mantra_dex_adapter: None,
        }
    }
}

impl From<&NetworkContracts> for ContractAddresses {
    fn from(network_contracts: &NetworkContracts) -> Self {
        Self {
            pool_manager: network_contracts
                .get_address(&ContractType::PoolManager)
                .cloned()
                .unwrap_or_default(),
            farm_manager: network_contracts
                .get_address(&ContractType::FarmManager)
                .cloned(),
            fee_collector: network_contracts
                .get_address(&ContractType::FeeCollector)
                .cloned(),
            epoch_manager: network_contracts
                .get_address(&ContractType::EpochManager)
                .cloned(),
            skip_entry_point: network_contracts
                .get_address(&ContractType::SkipEntryPoint)
                .cloned(),
            skip_ibc_hooks_adapter: network_contracts
                .get_address(&ContractType::SkipIbcHooksAdapter)
                .cloned(),
            skip_mantra_dex_adapter: network_contracts
                .get_address(&ContractType::SkipMantraDexAdapter)
                .cloned(),
        }
    }
}

/// Legacy network constants for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConstants {
    /// Network name
    pub network_name: String,
    /// Chain ID (for transaction signing)
    pub chain_id: String,
    /// Default RPC endpoint
    pub default_rpc: String,
    /// Default gas price (in uaum)
    pub default_gas_price: f64,
    /// Default gas adjustment
    pub default_gas_adjustment: f64,
    /// Native token denom
    pub native_denom: String,
}

impl NetworkConstants {
    /// Load network constants from the configuration file (legacy method)
    pub fn load(network: &str) -> Result<Self, ConfigError> {
        // Try new environment config first
        if let Ok(env_config) = EnvironmentConfig::load() {
            if env_config.get_network_name() == network {
                return Ok(Self {
                    network_name: env_config.get_network_name(),
                    chain_id: env_config.get_chain_id(),
                    default_rpc: env_config.get_rpc_url(),
                    default_gas_price: env_config.get_gas_price(),
                    default_gas_adjustment: env_config.get_gas_adjustment(),
                    native_denom: env_config.get_native_denom(),
                });
            }
        }

        // Fallback to legacy file-based loading
        let config_dir = std_env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let config_paths = vec![
            format!("{}/network", config_dir),
            "config/network".to_string(),
            "../config/network".to_string(),
            "../../config/network".to_string(),
        ];

        for config_path in &config_paths {
            if let Ok(settings) = ConfigLoader::builder()
                .add_source(File::with_name(config_path))
                .build()
            {
                if let Ok(constants) = settings.get::<NetworkConstants>(network) {
                    return Ok(constants);
                }
            }
        }

        // Hardcoded fallback for known networks
        match network {
            "mantra-dukong" => Ok(NetworkConstants {
                network_name: "mantra-dukong".to_string(),
                chain_id: "mantra-dukong-1".to_string(),
                default_rpc: "https://rpc.dukong.mantrachain.io:443".to_string(),
                default_gas_price: 0.01,
                default_gas_adjustment: 1.5,
                native_denom: "uom".to_string(),
            }),
            _ => Err(ConfigError::NotFound(format!(
                "Network configuration for '{}' not found",
                network
            ))),
        }
    }

    /// Get the default Mantra Dukong network constants
    pub fn default_dukong() -> Result<Self, ConfigError> {
        Self::load("mantra-dukong")
    }
}

impl From<&EnvironmentConfig> for NetworkConstants {
    fn from(env_config: &EnvironmentConfig) -> Self {
        Self {
            network_name: env_config.get_network_name(),
            chain_id: env_config.get_chain_id(),
            default_rpc: env_config.get_rpc_url(),
            default_gas_price: env_config.get_gas_price(),
            default_gas_adjustment: env_config.get_gas_adjustment(),
            native_denom: env_config.get_native_denom(),
        }
    }
}

/// Legacy network configuration for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraNetworkConfig {
    /// Network name (e.g., mantra-dukong)
    pub network_name: String,
    /// Chain ID (e.g., mantra-dukong)
    pub chain_id: String,
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Gas price in native token
    pub gas_price: f64,
    /// Gas adjustment for transactions
    pub gas_adjustment: f64,
    /// Native token denom
    pub native_denom: String,
    /// Contract addresses
    pub contracts: ContractAddresses,
}

impl MantraNetworkConfig {
    /// Create a new network config from network constants
    pub fn from_constants(constants: &NetworkConstants) -> Result<Self, Error> {
        // Attempt to load contract addresses for this network
        let contract_registry = ContractRegistry::load().unwrap_or_default();
        let contracts = if let Ok(network_contracts) = contract_registry.get_network(&constants.network_name) {
            ContractAddresses::from(network_contracts)
        } else {
            // Fallback to legacy loading
            Self::load_contract_addresses(&constants.network_name).unwrap_or_default()
        };

        Ok(Self {
            network_name: constants.network_name.clone(),
            chain_id: constants.chain_id.clone(),
            rpc_url: constants.default_rpc.clone(),
            gas_price: constants.default_gas_price,
            gas_adjustment: constants.default_gas_adjustment,
            native_denom: constants.native_denom.clone(),
            contracts,
        })
    }

    /// Create from environment configuration (new method)
    pub fn from_env_config(env_config: &EnvironmentConfig) -> Result<Self, Error> {
        let constants = NetworkConstants::from(env_config);
        Self::from_constants(&constants)
    }

    /// Load contract addresses for the given network from the contracts configuration file.
    /// Legacy method for backward compatibility.
    fn load_contract_addresses(network: &str) -> Result<ContractAddresses, Error> {
        let config_dir = std_env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let config_paths = vec![
            format!("{}/contracts", config_dir),
            "config/contracts".to_string(),
            "../config/contracts".to_string(),
            "../../config/contracts".to_string(),
        ];

        for config_path in &config_paths {
            if let Ok(settings) = ConfigLoader::builder()
                .add_source(File::with_name(config_path))
                .build()
            {
                let pool_manager_key = format!("{}.pool_manager.address", network);
                let farm_manager_key = format!("{}.farm_manager.address", network);
                let fee_collector_key = format!("{}.fee_collector.address", network);
                let epoch_manager_key = format!("{}.epoch_manager.address", network);
                let skip_entry_point_key = format!("{}.skip_entry_point.address", network);
                let skip_ibc_hooks_adapter_key =
                    format!("{}.skip_ibc_hooks_adapter.address", network);
                let skip_mantra_dex_adapter_key =
                    format!("{}.skip_mantra_dex_adapter.address", network);

                if let Ok(pool_manager) = settings.get::<String>(&pool_manager_key) {
                    return Ok(ContractAddresses {
                        pool_manager,
                        farm_manager: settings.get::<String>(&farm_manager_key).ok(),
                        fee_collector: settings.get::<String>(&fee_collector_key).ok(),
                        epoch_manager: settings.get::<String>(&epoch_manager_key).ok(),
                        skip_entry_point: settings.get::<String>(&skip_entry_point_key).ok(),
                        skip_ibc_hooks_adapter: settings
                            .get::<String>(&skip_ibc_hooks_adapter_key)
                            .ok(),
                        skip_mantra_dex_adapter: settings
                            .get::<String>(&skip_mantra_dex_adapter_key)
                            .ok(),
                    });
                }
            }
        }

        Err(Error::Config(format!(
            "Contract addresses for network '{}' not found in configuration",
            network
        )))
    }
}

impl Default for MantraNetworkConfig {
    fn default() -> Self {
        match NetworkConstants::default_dukong() {
            Ok(constants) => Self::from_constants(&constants).unwrap_or_else(|_| Self {
                network_name: constants.network_name,
                chain_id: constants.chain_id,
                rpc_url: constants.default_rpc,
                gas_price: constants.default_gas_price,
                gas_adjustment: constants.default_gas_adjustment,
                native_denom: constants.native_denom,
                contracts: ContractAddresses::default(),
            }),
            Err(_) => Self {
                network_name: "mantra-dukong".to_string(),
                chain_id: "mantra-dukong-1".to_string(),
                rpc_url: "https://rpc.dukong.mantrachain.io:443".to_string(),
                gas_price: 0.01,
                gas_adjustment: 1.5,
                native_denom: "uom".to_string(),
                contracts: ContractAddresses::default(),
            },
        }
    }
}

/// Token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// Token name
    pub name: String,
    /// Token symbol
    pub symbol: String,
    /// Token decimals
    pub decimals: u8,
    /// Token logo URL
    pub logo: Option<String>,
}

/// Complete legacy configuration with wallet info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Active network configuration
    pub network: MantraNetworkConfig,
    /// Wallet mnemonic (seed phrase)
    pub mnemonic: Option<String>,
    /// Known tokens and their metadata
    pub tokens: HashMap<String, TokenInfo>,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            network: MantraNetworkConfig::default(),
            mnemonic: None,
            tokens: HashMap::new(),
        }
    }

    /// Load configuration from a file (legacy method)
    pub fn load(path: &PathBuf) -> Result<Self, Error> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))?;
        Ok(config)
    }

    /// Save configuration to a file (legacy method)
    pub fn save(&self, path: &PathBuf) -> Result<(), Error> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default configuration file path
    pub fn default_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("mantra-dex");
        path.push("config.toml");
        path
    }

    /// Create from new configuration system
    pub fn from_modern_config() -> Result<Self, Error> {
        let env_config = EnvironmentConfig::load()?;
        let network_config = MantraNetworkConfig::from_env_config(&env_config)?;

        Ok(Self {
            network: network_config,
            mnemonic: None,
            tokens: HashMap::new(),
        })
    }
}

/// Unified configuration manager that integrates all configuration systems
#[derive(Debug, Clone)]
pub struct ConfigurationManager {
    /// Environment-based configuration
    pub env_config: EnvironmentConfig,
    /// Contract registry
    pub contract_registry: ContractRegistry,
    /// Protocol registry
    pub protocol_registry: ProtocolRegistry,
    /// Active network
    active_network: Option<String>,
}

impl ConfigurationManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self, Error> {
        let env_config = EnvironmentConfig::load()?;
        let mut contract_registry = ContractRegistry::load().unwrap_or_default();
        let mut protocol_registry = ProtocolRegistry::load().unwrap_or_default();

        // Set active network
        let active_network = env_config.get_network_name();
        if let Err(_) = contract_registry.set_active_network(&active_network) {
            // Network not found in contract registry, that's okay
        }
        protocol_registry.set_active_network(&active_network);

        Ok(Self {
            env_config,
            contract_registry,
            protocol_registry,
            active_network: Some(active_network),
        })
    }

    /// Get the active network name
    pub fn get_active_network(&self) -> Option<&String> {
        self.active_network.as_ref()
    }

    /// Set the active network
    pub fn set_active_network(&mut self, network: String) -> Result<(), Error> {
        // Validate network exists in contract registry
        if let Err(_) = self.contract_registry.set_active_network(&network) {
            // Network not found in contract registry, but we can still set it
        }
        
        self.protocol_registry.set_active_network(&network);
        self.active_network = Some(network);
        Ok(())
    }

    /// Get contract address for a specific contract type
    pub fn get_contract_address(&self, contract_type: &ContractType) -> Result<String, Error> {
        self.contract_registry.get_contract_address(contract_type)
    }

    /// Get protocol configuration
    pub fn get_protocol_config(&self, protocol_id: &ProtocolId) -> Option<ProtocolConfig> {
        self.protocol_registry.get_protocol(protocol_id)
    }

    /// Check if a protocol is enabled
    pub fn is_protocol_enabled(&self, protocol_id: &ProtocolId) -> bool {
        self.protocol_registry.is_protocol_enabled(protocol_id)
    }

    /// Get network configuration for legacy compatibility
    pub fn get_legacy_network_config(&self) -> MantraNetworkConfig {
        MantraNetworkConfig::from_env_config(&self.env_config)
            .unwrap_or_else(|_| MantraNetworkConfig::default())
    }

    /// Get network constants for legacy compatibility
    pub fn get_legacy_network_constants(&self) -> NetworkConstants {
        NetworkConstants::from(&self.env_config)
    }

    /// Validate all configurations
    pub fn validate(&self) -> Result<(), Error> {
        self.env_config.validate()?;
        self.protocol_registry.validate_all()?;
        if let Some(ref network) = self.active_network {
            if let Ok(_) = self.contract_registry.get_network(network) {
                self.contract_registry.validate_active_network()?;
            }
        }
        Ok(())
    }

    /// Reload all configurations
    pub fn reload(&mut self) -> Result<(), Error> {
        self.env_config = EnvironmentConfig::load()?;
        self.contract_registry = ContractRegistry::load().unwrap_or_default();
        self.protocol_registry = ProtocolRegistry::load().unwrap_or_default();

        // Restore active network
        if let Some(ref network) = self.active_network {
            if let Err(_) = self.contract_registry.set_active_network(network) {
                // Network not found, that's okay
            }
            self.protocol_registry.set_active_network(network);
        }

        Ok(())
    }
}

impl Default for ConfigurationManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            env_config: EnvironmentConfig::generate_default_config(),
            contract_registry: ContractRegistry::default(),
            protocol_registry: ProtocolRegistry::default(),
            active_network: Some("mantra-dukong".to_string()),
        })
    }
}

use config::{Config as ConfigLoader, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fmt;

use crate::error::Error;

/// Contract type enumeration for validation and categorization
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContractType {
    /// DEX Pool Manager contract
    PoolManager,
    /// DEX Farm Manager contract
    FarmManager,
    /// DEX Fee Collector contract
    FeeCollector,
    /// DEX Epoch Manager contract
    EpochManager,
    /// Skip Entry Point contract
    SkipEntryPoint,
    /// Skip IBC Hooks Adapter contract
    SkipIbcHooksAdapter,
    /// Skip Mantra DEX Adapter contract
    SkipMantraDexAdapter,
    /// ClaimDrop Factory contract
    ClaimdropFactory,
    /// ClaimDrop Campaign contract
    ClaimdropCampaign,
    /// EVM RPC Endpoint
    #[cfg(feature = "evm")]
    EvmRpcEndpoint,
    /// EVM Chain ID
    #[cfg(feature = "evm")]
    EvmChainId,
    /// EVM USDC Token contract
    #[cfg(feature = "evm")]
    EvmUsdc,
    /// EVM WETH Token contract
    #[cfg(feature = "evm")]
    EvmWeth,
}

impl fmt::Display for ContractType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractType::PoolManager => write!(f, "pool_manager"),
            ContractType::FarmManager => write!(f, "farm_manager"),
            ContractType::FeeCollector => write!(f, "fee_collector"),
            ContractType::EpochManager => write!(f, "epoch_manager"),
            ContractType::SkipEntryPoint => write!(f, "skip_entry_point"),
            ContractType::SkipIbcHooksAdapter => write!(f, "skip_ibc_hooks_adapter"),
            ContractType::SkipMantraDexAdapter => write!(f, "skip_mantra_dex_adapter"),
            ContractType::ClaimdropFactory => write!(f, "claimdrop_factory"),
            ContractType::ClaimdropCampaign => write!(f, "claimdrop_campaign"),
            #[cfg(feature = "evm")]
            ContractType::EvmRpcEndpoint => write!(f, "evm_rpc_endpoint"),
            #[cfg(feature = "evm")]
            ContractType::EvmChainId => write!(f, "evm_chain_id"),
            #[cfg(feature = "evm")]
            ContractType::EvmUsdc => write!(f, "evm_usdc"),
            #[cfg(feature = "evm")]
            ContractType::EvmWeth => write!(f, "evm_weth"),
        }
    }
}

/// Contract information including address, code ID, and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractInfo {
    /// Contract address on the blockchain
    pub address: String,
    /// Code ID of the contract (if available)
    pub code_id: Option<u64>,
    /// Contract version
    pub version: Option<String>,
    /// Whether this contract is required for protocol operation
    pub required: bool,
    /// Protocol this contract belongs to
    pub protocol: String,
}

impl ContractInfo {
    /// Create a new contract info with minimal required data
    pub fn new(address: String, protocol: String) -> Self {
        Self {
            address,
            code_id: None,
            version: None,
            required: true,
            protocol,
        }
    }

    /// Create a new contract info with full metadata
    pub fn with_metadata(
        address: String,
        code_id: Option<u64>,
        version: Option<String>,
        required: bool,
        protocol: String,
    ) -> Self {
        Self {
            address,
            code_id,
            version,
            required,
            protocol,
        }
    }

    /// Validate contract address format (basic Cosmos bech32 validation)
    pub fn validate_address(&self, expected_prefix: &str) -> Result<(), Error> {
        if self.address.is_empty() {
            return Err(Error::Config(
                "Contract address cannot be empty".to_string(),
            ));
        }

        if !self.address.starts_with(expected_prefix) {
            return Err(Error::Config(format!(
                "Contract address '{}' does not have expected prefix '{}'",
                self.address, expected_prefix
            )));
        }

        // Basic length validation for Cosmos addresses
        if self.address.len() < 39 || self.address.len() > 90 {
            return Err(Error::Config(format!(
                "Contract address '{}' has invalid length",
                self.address
            )));
        }

        Ok(())
    }
}

/// Network-specific contract address registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkContracts {
    /// Network name (e.g., "mantra-dukong")
    pub network: String,
    /// Chain ID for validation
    pub chain_id: String,
    /// Map of contract type to contract information
    pub contracts: HashMap<ContractType, ContractInfo>,
    /// Address prefix for this network (e.g., "mantra")
    pub address_prefix: String,
}

impl NetworkContracts {
    /// Create a new empty network contracts registry
    pub fn new(network: String, chain_id: String, address_prefix: String) -> Self {
        Self {
            network,
            chain_id,
            contracts: HashMap::new(),
            address_prefix,
        }
    }

    /// Add a contract to the registry
    pub fn add_contract(
        &mut self,
        contract_type: ContractType,
        contract_info: ContractInfo,
    ) -> Result<(), Error> {
        // Validate address format
        contract_info.validate_address(&self.address_prefix)?;

        self.contracts.insert(contract_type, contract_info);
        Ok(())
    }

    /// Get contract information by type
    pub fn get_contract(&self, contract_type: &ContractType) -> Option<&ContractInfo> {
        self.contracts.get(contract_type)
    }

    /// Get contract address by type
    pub fn get_address(&self, contract_type: &ContractType) -> Option<&String> {
        self.contracts.get(contract_type).map(|info| &info.address)
    }

    /// Check if all required contracts are present
    pub fn validate_required_contracts(&self) -> Result<(), Error> {
        let required_contracts = vec![ContractType::PoolManager];

        for contract_type in required_contracts {
            match self.contracts.get(&contract_type) {
                Some(info) if info.required => {
                    info.validate_address(&self.address_prefix)?;
                }
                Some(_) => {} // Optional contract
                None => {
                    return Err(Error::Config(format!(
                        "Required contract '{}' not found for network '{}'",
                        contract_type, self.network
                    )));
                }
            }
        }

        Ok(())
    }

    /// Get all contracts for a specific protocol
    pub fn get_protocol_contracts(&self, protocol: &str) -> HashMap<ContractType, &ContractInfo> {
        self.contracts
            .iter()
            .filter(|(_, info)| info.protocol == protocol)
            .map(|(contract_type, info)| (contract_type.clone(), info))
            .collect()
    }
}

/// Main contract registry managing multiple networks
#[derive(Debug, Clone)]
pub struct ContractRegistry {
    /// Map of network name to network contracts
    networks: HashMap<String, NetworkContracts>,
    /// Current active network
    active_network: Option<String>,
}

impl ContractRegistry {
    /// Create a new empty contract registry
    pub fn new() -> Self {
        Self {
            networks: HashMap::new(),
            active_network: None,
        }
    }

    /// Load contract registry from configuration files
    pub fn load() -> Result<Self, Error> {
        let mut registry = Self::new();

        // Load from configuration directory
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        // Try multiple paths for the config file
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
                registry.load_from_config(&settings)?;
                break;
            }
        }

        // If no configuration found, load defaults
        if registry.networks.is_empty() {
            registry.load_defaults()?;
        }

        Ok(registry)
    }

    /// Load contracts from configuration settings
    fn load_from_config(&mut self, settings: &ConfigLoader) -> Result<(), Error> {
        // Try to get available networks from config
        let networks = ["mantra-dukong"]; // Can be extended based on config discovery

        for network in &networks {
            let mut network_contracts = NetworkContracts::new(
                network.to_string(),
                format!("{}-1", network), // Default chain ID pattern
                "mantra".to_string(),
            );

            // Load contract addresses for this network
            self.load_network_contracts(&mut network_contracts, settings, network)?;

            // Only add if we found some contracts
            if !network_contracts.contracts.is_empty() {
                self.networks.insert(network.to_string(), network_contracts);
            }
        }

        Ok(())
    }

    /// Load contracts for a specific network from settings
    fn load_network_contracts(
        &self,
        network_contracts: &mut NetworkContracts,
        settings: &ConfigLoader,
        network: &str,
    ) -> Result<(), Error> {
        let contract_types = [
            (ContractType::PoolManager, "pool_manager"),
            (ContractType::FarmManager, "farm_manager"),
            (ContractType::FeeCollector, "fee_collector"),
            (ContractType::EpochManager, "epoch_manager"),
            (ContractType::SkipEntryPoint, "skip_entry_point"),
            (ContractType::SkipIbcHooksAdapter, "skip_ibc_hooks_adapter"),
            (
                ContractType::SkipMantraDexAdapter,
                "skip_mantra_dex_adapter",
            ),
        ];

        for (contract_type, contract_name) in &contract_types {
            let address_key = format!("{}.{}.address", network, contract_name);
            let code_id_key = format!("{}.{}.code_id", network, contract_name);
            let version_key = format!("{}.{}.version", network, contract_name);

            if let Ok(address) = settings.get::<String>(&address_key) {
                let code_id = settings.get::<u64>(&code_id_key).ok();
                let version = settings.get::<String>(&version_key).ok();

                let protocol = match contract_type {
                    ContractType::SkipEntryPoint
                    | ContractType::SkipIbcHooksAdapter
                    | ContractType::SkipMantraDexAdapter => "skip",
                    ContractType::ClaimdropFactory | ContractType::ClaimdropCampaign => "claimdrop",
                    #[cfg(feature = "evm")]
                    ContractType::EvmRpcEndpoint
                    | ContractType::EvmChainId
                    | ContractType::EvmUsdc
                    | ContractType::EvmWeth => "evm",
                    _ => "dex",
                };

                let contract_info = ContractInfo::with_metadata(
                    address,
                    code_id,
                    version,
                    true, // Most contracts are required by default
                    protocol.to_string(),
                );

                network_contracts.add_contract(contract_type.clone(), contract_info)?;
            }
        }

        Ok(())
    }

    /// Load default contract configurations
    fn load_defaults(&mut self) -> Result<(), Error> {
        // Default Mantra Dukong configuration (empty as we rely on config files)
        let dukong_contracts = NetworkContracts::new(
            "mantra-dukong".to_string(),
            "mantra-dukong-1".to_string(),
            "mantra".to_string(),
        );

        self.networks
            .insert("mantra-dukong".to_string(), dukong_contracts);
        Ok(())
    }

    /// Set active network
    pub fn set_active_network(&mut self, network: &str) -> Result<(), Error> {
        if !self.networks.contains_key(network) {
            return Err(Error::Config(format!(
                "Network '{}' not found in contract registry",
                network
            )));
        }
        self.active_network = Some(network.to_string());
        Ok(())
    }

    /// Get active network contracts
    pub fn get_active_network(&self) -> Result<&NetworkContracts, Error> {
        match &self.active_network {
            Some(network) => self.get_network(network),
            None => Err(Error::Config("No active network set".to_string())),
        }
    }

    /// Get contracts for a specific network
    pub fn get_network(&self, network: &str) -> Result<&NetworkContracts, Error> {
        self.networks
            .get(network)
            .ok_or_else(|| Error::Config(format!("Network '{}' not found", network)))
    }

    /// Get contract address for active network
    pub fn get_contract_address(&self, contract_type: &ContractType) -> Result<String, Error> {
        let network = self.get_active_network()?;
        network
            .get_address(contract_type)
            .ok_or_else(|| {
                Error::Config(format!(
                    "Contract '{}' not found for network '{}'",
                    contract_type, network.network
                ))
            })
            .cloned()
    }

    /// Get contract info for active network
    pub fn get_contract_info(&self, contract_type: &ContractType) -> Result<&ContractInfo, Error> {
        let network = self.get_active_network()?;
        network.get_contract(contract_type).ok_or_else(|| {
            Error::Config(format!(
                "Contract '{}' not found for network '{}'",
                contract_type, network.network
            ))
        })
    }

    /// Validate all contract addresses for active network
    pub fn validate_active_network(&self) -> Result<(), Error> {
        let network = self.get_active_network()?;
        network.validate_required_contracts()
    }

    /// Add or update a contract address
    pub fn update_contract(
        &mut self,
        network: &str,
        contract_type: ContractType,
        contract_info: ContractInfo,
    ) -> Result<(), Error> {
        let network_contracts = self
            .networks
            .get_mut(network)
            .ok_or_else(|| Error::Config(format!("Network '{}' not found", network)))?;

        network_contracts.add_contract(contract_type, contract_info)
    }

    /// Get all available networks
    pub fn get_available_networks(&self) -> Vec<&String> {
        self.networks.keys().collect()
    }

    /// Check if a network has all required contracts
    pub fn is_network_ready(&self, network: &str) -> bool {
        self.get_network(network)
            .and_then(|net| net.validate_required_contracts())
            .is_ok()
    }
}

impl Default for ContractRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_info_validation() {
        let contract = ContractInfo::new(
            "mantra1vwj600jud78djej7ttq44dktu4wr3t2yrrsjgmld8v3jq8mud68q5w7455".to_string(),
            "dex".to_string(),
        );

        assert!(contract.validate_address("mantra").is_ok());
        assert!(contract.validate_address("cosmos").is_err());
    }

    #[test]
    fn test_network_contracts() {
        let mut network = NetworkContracts::new(
            "test-network".to_string(),
            "test-1".to_string(),
            "mantra".to_string(),
        );

        let contract = ContractInfo::new(
            "mantra1vwj600jud78djej7ttq44dktu4wr3t2yrrsjgmld8v3jq8mud68q5w7455".to_string(),
            "dex".to_string(),
        );

        assert!(network
            .add_contract(ContractType::PoolManager, contract)
            .is_ok());
        assert!(network.get_address(&ContractType::PoolManager).is_some());
    }

    #[test]
    fn test_contract_registry() {
        let mut registry = ContractRegistry::new();

        let mut network = NetworkContracts::new(
            "test-network".to_string(),
            "test-1".to_string(),
            "mantra".to_string(),
        );

        let contract = ContractInfo::new(
            "mantra1vwj600jud78djej7ttq44dktu4wr3t2yrrsjgmld8v3jq8mud68q5w7455".to_string(),
            "dex".to_string(),
        );

        network
            .add_contract(ContractType::PoolManager, contract)
            .unwrap();
        registry
            .networks
            .insert("test-network".to_string(), network);
        registry.set_active_network("test-network").unwrap();

        assert!(registry
            .get_contract_address(&ContractType::PoolManager)
            .is_ok());
    }
}

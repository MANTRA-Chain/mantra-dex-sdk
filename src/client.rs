/// Generic Mantra SDK Client
///
/// This is the main entry point for interacting with the MANTRA blockchain.
/// It provides access to all supported protocols through a unified interface.
use crate::config::{ConfigurationManager, ContractType, MantraNetworkConfig, ProtocolId};
use crate::error::Error;
use crate::protocols::{
    claimdrop::{ClaimdropFactoryClient, ClaimdropProtocol},
    dex::{DexProtocol, MantraDexClient},
    skip::SkipProtocol,
    Protocol, ProtocolRegistry,
};
use crate::wallet::MantraWallet;
use cosmrs::{rpc::HttpClient, AccountId};
use std::{str::FromStr, sync::Arc};
use tracing::warn;

/// Validate a contract address using cosmrs AccountId parsing
fn validate_contract_address(address: &str) -> Result<(), Error> {
    AccountId::from_str(address)
        .map_err(|e| Error::Config(format!("Invalid contract address '{}': {}", address, e)))?;
    Ok(())
}

/// Configuration changes for selective updates
#[derive(Debug, Default)]
pub struct ConfigurationChanges {
    /// New RPC URL if changed
    pub rpc_url: Option<String>,
    /// Whether DEX configuration changed
    pub dex_config_changed: bool,
    /// Whether Skip configuration changed  
    pub skip_config_changed: bool,
    /// Whether ClaimDrop configuration changed
    pub claimdrop_config_changed: bool,
}

/// Main MANTRA SDK client that provides access to all protocols
pub struct MantraClient {
    /// RPC client for blockchain communication
    rpc_client: Arc<HttpClient>,

    /// Unified configuration manager
    config_manager: ConfigurationManager,

    /// Legacy network configuration for backward compatibility
    network_config: MantraNetworkConfig,

    /// Optional wallet for signing transactions
    wallet: Option<Arc<MantraWallet>>,

    /// Protocol registry containing all available protocols
    protocol_registry: ProtocolRegistry,

    /// DEX protocol instance
    dex_protocol: Option<Arc<DexProtocol>>,

    /// Skip protocol instance
    skip_protocol: Option<Arc<SkipProtocol>>,

    /// ClaimDrop protocol instance
    claimdrop_protocol: Option<Arc<ClaimdropProtocol>>,
}

impl MantraClient {
    /// Create a new MANTRA client using the modern configuration system
    pub async fn new_with_config(
        config_manager: ConfigurationManager,
        wallet: Option<Arc<MantraWallet>>,
    ) -> Result<Self, Error> {
        // Get legacy network config for backward compatibility
        let network_config = config_manager.get_legacy_network_config();

        // Create RPC client
        let rpc_client = Arc::new(
            HttpClient::new(network_config.rpc_url.as_str())
                .map_err(|e| Error::Rpc(e.to_string()))?,
        );

        // Create protocol registry
        let mut protocol_registry = ProtocolRegistry::new();

        // Initialize protocols based on configuration
        let mut dex_protocol = None;
        let mut skip_protocol = None;
        let mut claimdrop_protocol = None;

        // Initialize DEX protocol if enabled
        if config_manager.is_protocol_enabled(&ProtocolId::Dex) {
            let mut dex = DexProtocol::new();
            dex.initialize(rpc_client.clone()).await?;
            let dex_arc = Arc::new(dex);
            protocol_registry.register(dex_arc.clone());
            dex_protocol = Some(dex_arc);
        }

        // Initialize Skip protocol if enabled
        if config_manager.is_protocol_enabled(&ProtocolId::Skip) {
            let mut skip = SkipProtocol::new();
            skip.initialize(rpc_client.clone()).await?;

            // Set contract addresses from configuration with validation
            if let Ok(entry_point_addr) =
                config_manager.get_contract_address(&ContractType::SkipEntryPoint)
            {
                // Validate the contract address before setting it
                if let Err(e) = validate_contract_address(&entry_point_addr) {
                    warn!(
                        contract_address = %entry_point_addr,
                        error = %e,
                        "Invalid Skip entry point contract address, skipping"
                    );
                } else {
                    skip.set_contract_address(entry_point_addr);
                }
            }

            let skip_arc = Arc::new(skip);
            protocol_registry.register(skip_arc.clone());
            skip_protocol = Some(skip_arc);
        }

        // Initialize ClaimDrop protocol if enabled
        if config_manager.is_protocol_enabled(&ProtocolId::ClaimDrop) {
            let mut claimdrop = ClaimdropProtocol::new();
            claimdrop.initialize(rpc_client.clone()).await?;

            // Set factory address from configuration if available
            if let Ok(factory_addr) =
                config_manager.get_contract_address(&ContractType::ClaimdropFactory)
            {
                claimdrop.set_factory_address(factory_addr);
            }

            let claimdrop_arc = Arc::new(claimdrop);
            protocol_registry.register(claimdrop_arc.clone());
            claimdrop_protocol = Some(claimdrop_arc);
        }

        Ok(Self {
            rpc_client,
            config_manager,
            network_config,
            wallet,
            protocol_registry,
            dex_protocol,
            skip_protocol,
            claimdrop_protocol,
        })
    }

    /// Create a new MANTRA client (legacy method for backward compatibility)
    pub async fn new(
        network_config: MantraNetworkConfig,
        wallet: Option<Arc<MantraWallet>>,
    ) -> Result<Self, Error> {
        // Create a configuration manager from the legacy network config
        let mut config_manager = ConfigurationManager::default();

        // Try to set the active network based on network config
        if let Err(_) = config_manager.set_active_network(network_config.network_name.clone()) {
            // If network not found, use default but log the issue
            warn!(
                network_name = %network_config.network_name,
                "Network not found in configuration, using defaults"
            );
        }

        Self::new_with_config(config_manager, wallet).await
    }

    /// Get the RPC client
    pub fn rpc_client(&self) -> &Arc<HttpClient> {
        &self.rpc_client
    }

    /// Get the configuration manager
    pub fn config_manager(&self) -> &ConfigurationManager {
        &self.config_manager
    }

    /// Get mutable reference to configuration manager
    pub fn config_manager_mut(&mut self) -> &mut ConfigurationManager {
        &mut self.config_manager
    }

    /// Get the network configuration (legacy method)
    pub fn network_config(&self) -> &MantraNetworkConfig {
        &self.network_config
    }

    /// Get the current wallet
    pub fn wallet(&self) -> Option<&Arc<MantraWallet>> {
        self.wallet.as_ref()
    }

    /// Set a new wallet
    pub fn set_wallet(&mut self, wallet: Arc<MantraWallet>) {
        self.wallet = Some(wallet);
    }

    /// Get contract address for a specific contract type
    pub fn get_contract_address(&self, contract_type: &ContractType) -> Result<String, Error> {
        self.config_manager.get_contract_address(contract_type)
    }

    /// Check if a protocol is enabled
    pub fn is_protocol_enabled(&self, protocol_id: &ProtocolId) -> bool {
        self.config_manager.is_protocol_enabled(protocol_id)
    }

    /// Get protocol configuration
    pub fn get_protocol_config(
        &self,
        protocol_id: &ProtocolId,
    ) -> Option<crate::config::ProtocolConfig> {
        self.config_manager.get_protocol_config(protocol_id)
    }

    /// Switch to a different network
    pub async fn switch_network(&mut self, network_name: String) -> Result<(), Error> {
        // Update configuration manager
        self.config_manager
            .set_active_network(network_name.clone())?;

        // Update legacy network config
        self.network_config = self.config_manager.get_legacy_network_config();

        // Recreate RPC client with new endpoint
        self.rpc_client = Arc::new(
            HttpClient::new(self.network_config.rpc_url.as_str())
                .map_err(|e| Error::Rpc(e.to_string()))?,
        );

        // Reinitialize protocols
        self.reinitialize_protocols().await
    }

    /// Update configuration selectively without full reinitialization
    pub async fn update_config_selective(
        &mut self,
        config_changes: ConfigurationChanges,
    ) -> Result<(), Error> {
        let mut requires_rpc_restart = false;
        let mut protocols_to_update = Vec::new();

        // Check if RPC endpoint changed
        if let Some(new_rpc_url) = &config_changes.rpc_url {
            if new_rpc_url != &self.network_config.rpc_url {
                self.network_config.rpc_url = new_rpc_url.clone();
                requires_rpc_restart = true;
            }
        }

        // Check protocol-specific changes
        if config_changes.dex_config_changed {
            protocols_to_update.push(ProtocolId::Dex);
        }
        if config_changes.skip_config_changed {
            protocols_to_update.push(ProtocolId::Skip);
        }
        if config_changes.claimdrop_config_changed {
            protocols_to_update.push(ProtocolId::ClaimDrop);
        }

        // Update RPC client if needed
        if requires_rpc_restart {
            self.rpc_client = Arc::new(
                HttpClient::new(self.network_config.rpc_url.as_str())
                    .map_err(|e| Error::Rpc(e.to_string()))?,
            );
            // If RPC changes, all protocols need updating
            protocols_to_update = vec![ProtocolId::Dex, ProtocolId::Skip, ProtocolId::ClaimDrop];
        }

        // Selectively update only changed protocols
        for protocol_id in protocols_to_update {
            self.update_protocol(&protocol_id).await?;
        }

        Ok(())
    }

    /// Update a specific protocol with new configuration
    async fn update_protocol(&mut self, protocol_id: &ProtocolId) -> Result<(), Error> {
        match protocol_id {
            ProtocolId::Dex => {
                if self.config_manager.is_protocol_enabled(protocol_id) {
                    if self.dex_protocol.is_some() {
                        let mut dex = DexProtocol::new();
                        dex.initialize(self.rpc_client.clone()).await?;
                        let dex_arc = Arc::new(dex);

                        // Update in registry
                        self.protocol_registry.register(dex_arc.clone());
                        self.dex_protocol = Some(dex_arc);
                    }
                } else {
                    self.dex_protocol = None;
                }
            }
            ProtocolId::Skip => {
                if self.config_manager.is_protocol_enabled(protocol_id) {
                    if self.skip_protocol.is_some() {
                        let mut skip = SkipProtocol::new();
                        skip.initialize(self.rpc_client.clone()).await?;

                        // Update contract addresses with validation
                        if let Ok(entry_point_addr) = self
                            .config_manager
                            .get_contract_address(&ContractType::SkipEntryPoint)
                        {
                            if let Err(e) = validate_contract_address(&entry_point_addr) {
                                warn!(
                                    contract_address = %entry_point_addr,
                                    error = %e,
                                    "Invalid Skip entry point contract address during update, skipping"
                                );
                            } else {
                                skip.set_contract_address(entry_point_addr);
                            }
                        }

                        let skip_arc = Arc::new(skip);
                        self.protocol_registry.register(skip_arc.clone());
                        self.skip_protocol = Some(skip_arc);
                    }
                } else {
                    self.skip_protocol = None;
                }
            }
            ProtocolId::ClaimDrop => {
                if self.config_manager.is_protocol_enabled(protocol_id) {
                    if self.claimdrop_protocol.is_some() {
                        let mut claimdrop = ClaimdropProtocol::new();
                        claimdrop.initialize(self.rpc_client.clone()).await?;

                        // Update factory address with validation
                        if let Ok(factory_addr) = self
                            .config_manager
                            .get_contract_address(&ContractType::ClaimdropFactory)
                        {
                            if let Err(e) = validate_contract_address(&factory_addr) {
                                warn!(
                                    contract_address = %factory_addr,
                                    error = %e,
                                    "Invalid ClaimDrop factory address during update, skipping"
                                );
                            } else {
                                claimdrop.set_factory_address(factory_addr);
                            }
                        }

                        let claimdrop_arc = Arc::new(claimdrop);
                        self.protocol_registry.register(claimdrop_arc.clone());
                        self.claimdrop_protocol = Some(claimdrop_arc);
                    }
                } else {
                    self.claimdrop_protocol = None;
                }
            }
        }
        Ok(())
    }

    /// Reinitialize all protocols with current configuration
    async fn reinitialize_protocols(&mut self) -> Result<(), Error> {
        // Clear protocol references first to avoid dangling references in registry
        if !self.config_manager.is_protocol_enabled(&ProtocolId::Dex) {
            self.dex_protocol = None;
        }
        if !self.config_manager.is_protocol_enabled(&ProtocolId::Skip) {
            self.skip_protocol = None;
        }
        if !self
            .config_manager
            .is_protocol_enabled(&ProtocolId::ClaimDrop)
        {
            self.claimdrop_protocol = None;
        }

        // Now clear existing protocol registry safely
        self.protocol_registry = ProtocolRegistry::new();

        // Reinitialize DEX protocol if enabled
        if self.config_manager.is_protocol_enabled(&ProtocolId::Dex) {
            if self.dex_protocol.is_some() {
                let mut dex = DexProtocol::new();
                dex.initialize(self.rpc_client.clone()).await?;
                let dex_arc = Arc::new(dex);
                self.protocol_registry.register(dex_arc.clone());
                self.dex_protocol = Some(dex_arc);
            }
        }

        // Reinitialize Skip protocol if enabled
        if self.config_manager.is_protocol_enabled(&ProtocolId::Skip) {
            if self.skip_protocol.is_some() {
                let mut skip = SkipProtocol::new();
                skip.initialize(self.rpc_client.clone()).await?;

                // Update contract addresses with validation
                if let Ok(entry_point_addr) = self
                    .config_manager
                    .get_contract_address(&ContractType::SkipEntryPoint)
                {
                    if let Err(e) = validate_contract_address(&entry_point_addr) {
                        warn!(
                            contract_address = %entry_point_addr,
                            error = %e,
                            "Invalid Skip entry point contract address during reinit, skipping"
                        );
                    } else {
                        skip.set_contract_address(entry_point_addr);
                    }
                }

                let skip_arc = Arc::new(skip);
                self.protocol_registry.register(skip_arc.clone());
                self.skip_protocol = Some(skip_arc);
            }
        }

        // Reinitialize ClaimDrop protocol if enabled
        if self
            .config_manager
            .is_protocol_enabled(&ProtocolId::ClaimDrop)
        {
            if self.claimdrop_protocol.is_some() {
                let mut claimdrop = ClaimdropProtocol::new();
                claimdrop.initialize(self.rpc_client.clone()).await?;

                // Update factory address with validation
                if let Ok(factory_addr) = self
                    .config_manager
                    .get_contract_address(&ContractType::ClaimdropFactory)
                {
                    if let Err(e) = validate_contract_address(&factory_addr) {
                        warn!(
                            contract_address = %factory_addr,
                            error = %e,
                            "Invalid ClaimDrop factory address during reinit, skipping"
                        );
                    } else {
                        claimdrop.set_factory_address(factory_addr);
                    }
                }

                let claimdrop_arc = Arc::new(claimdrop);
                self.protocol_registry.register(claimdrop_arc.clone());
                self.claimdrop_protocol = Some(claimdrop_arc);
            }
        }

        Ok(())
    }

    /// List all available protocols
    pub fn list_protocols(&self) -> Vec<&str> {
        self.protocol_registry.list()
    }

    /// Check if a protocol is available
    pub fn is_protocol_available(&self, protocol_name: &str) -> bool {
        self.protocol_registry.get(protocol_name).is_some()
    }

    // ============ Protocol-specific accessors ============

    /// Get DEX client for DEX operations
    pub async fn dex(&self) -> Result<MantraDexClient, Error> {
        // Create a DEX client with the current configuration
        let client = MantraDexClient::new(self.network_config.clone()).await?;

        // Return client (wallet will be set when transactions are performed)
        Ok(client)
    }

    /// Get Skip client for cross-chain operations
    pub async fn skip(&self) -> Result<crate::protocols::skip::SkipClient, Error> {
        // Create a Skip client with the current configuration
        let mut client = crate::protocols::skip::SkipClient::new(self.wallet.clone()).await?;

        // Set adapter contract address if available
        if let Some(skip_protocol) = &self.skip_protocol {
            if let Some(contract_addr) = skip_protocol.contract_address() {
                client.set_adapter_contract(contract_addr.to_string());
            }
        }

        Ok(client)
    }

    /// Get ClaimDrop factory client
    pub fn claimdrop_factory(&self, factory_address: String) -> ClaimdropFactoryClient {
        use tokio::sync::Mutex;
        ClaimdropFactoryClient::new(
            Arc::new(Mutex::new((*self.rpc_client).clone())),
            factory_address,
            self.wallet.clone(),
        )
    }

    /// Get ClaimDrop campaign client
    pub fn claimdrop_campaign(
        &self,
        campaign_address: String,
    ) -> crate::protocols::claimdrop::ClaimdropClient {
        use tokio::sync::Mutex;
        crate::protocols::claimdrop::ClaimdropClient::new(
            Arc::new(Mutex::new((*self.rpc_client).clone())),
            campaign_address,
            self.wallet.clone(),
        )
    }

    /// Get Skip protocol configuration
    pub fn skip_config(&self) -> Option<serde_json::Value> {
        self.skip_protocol
            .as_ref()
            .and_then(|p| p.get_config().ok())
    }

    /// Set Skip adapter contract address
    pub fn set_skip_contract(&mut self, address: String) {
        if let Some(skip_arc) = &mut self.skip_protocol {
            if let Some(skip) = Arc::get_mut(skip_arc) {
                skip.set_contract_address(address);
            } else {
                // If there are multiple references, create a new instance
                warn!(
                    "Cannot mutate Skip protocol due to multiple references, recreating protocol"
                );
                let mut new_skip = (**skip_arc).clone();
                new_skip.set_contract_address(address);
                *skip_arc = Arc::new(new_skip);
            }
        }
    }

    /// Set ClaimDrop factory address
    pub fn set_claimdrop_factory(&mut self, address: String) {
        if let Some(claimdrop_arc) = &mut self.claimdrop_protocol {
            if let Some(claimdrop) = Arc::get_mut(claimdrop_arc) {
                claimdrop.set_factory_address(address);
            } else {
                // If there are multiple references, create a new instance
                warn!("Cannot mutate ClaimDrop protocol due to multiple references, recreating protocol");
                let mut new_claimdrop = (**claimdrop_arc).clone();
                new_claimdrop.set_factory_address(address);
                *claimdrop_arc = Arc::new(new_claimdrop);
            }
        }
    }

    // ============ Utility methods ============

    /// Check connectivity to all configured protocols
    pub async fn check_connectivity(&self) -> Result<Vec<(String, bool)>, Error> {
        let mut results = Vec::new();

        for protocol_name in self.list_protocols() {
            if let Some(protocol) = self.protocol_registry.get(protocol_name) {
                let available = protocol.is_available(&self.rpc_client).await?;
                results.push((protocol_name.to_string(), available));
            }
        }

        Ok(results)
    }

    /// Get a summary of the client configuration
    pub fn get_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "network": {
                "chain_id": self.network_config.chain_id,
                "rpc_endpoint": self.network_config.rpc_url,
            },
            "protocols": self.list_protocols(),
            "wallet_connected": self.wallet.is_some(),
        })
    }
}

/// Builder pattern for MantraClient construction
pub struct MantraClientBuilder {
    config_manager: Option<ConfigurationManager>,
    network_config: Option<MantraNetworkConfig>, // Legacy support
    wallet: Option<Arc<MantraWallet>>,
    skip_contract: Option<String>,
    claimdrop_factory: Option<String>,
}

impl MantraClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config_manager: None,
            network_config: None,
            wallet: None,
            skip_contract: None,
            claimdrop_factory: None,
        }
    }

    /// Create a new builder with the modern configuration system
    pub fn with_config_manager(mut self, config_manager: ConfigurationManager) -> Self {
        self.config_manager = Some(config_manager);
        self
    }

    /// Set the network configuration (legacy method)
    pub fn with_network(mut self, config: MantraNetworkConfig) -> Self {
        self.network_config = Some(config);
        self
    }

    /// Set the wallet
    pub fn with_wallet(mut self, wallet: Arc<MantraWallet>) -> Self {
        self.wallet = Some(wallet);
        self
    }

    /// Set the Skip contract address
    pub fn with_skip_contract(mut self, address: String) -> Self {
        self.skip_contract = Some(address);
        self
    }

    /// Set the ClaimDrop factory address
    pub fn with_claimdrop_factory(mut self, address: String) -> Self {
        self.claimdrop_factory = Some(address);
        self
    }

    /// Build the MantraClient using the modern configuration system
    pub async fn build(self) -> Result<MantraClient, Error> {
        let mut client = if let Some(config_manager) = self.config_manager {
            // Use modern configuration system
            MantraClient::new_with_config(config_manager, self.wallet).await?
        } else if let Some(network_config) = self.network_config {
            // Use legacy configuration system
            MantraClient::new(network_config, self.wallet).await?
        } else {
            // Use default configuration
            let config_manager = ConfigurationManager::default();
            MantraClient::new_with_config(config_manager, self.wallet).await?
        };

        // Apply builder-specific contract addresses (overrides configuration)
        if let Some(skip_contract) = self.skip_contract {
            client.set_skip_contract(skip_contract);
        }

        if let Some(claimdrop_factory) = self.claimdrop_factory {
            client.set_claimdrop_factory(claimdrop_factory);
        }

        Ok(client)
    }

    /// Build the MantraClient with automatic configuration discovery
    pub async fn build_auto(self) -> Result<MantraClient, Error> {
        if self.config_manager.is_none() && self.network_config.is_none() {
            // Try to load configuration automatically
            match ConfigurationManager::new() {
                Ok(config_manager) => self.with_config_manager(config_manager).build().await,
                Err(_) => {
                    // Fall back to default if auto-loading fails
                    self.build().await
                }
            }
        } else {
            self.build().await
        }
    }
}

impl Default for MantraClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

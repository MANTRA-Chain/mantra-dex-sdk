use std::sync::Arc;

use serde_json::Value as JsonValue;
use tracing::{debug, info, warn};

use crate::config::MantraNetworkConfig;

use super::sdk_adapter::McpSdkAdapter;
use super::server::{McpResult, McpServerError};

/// MCP-specific client wrapper that provides high-level operations
/// for the Mantra DEX SDK in an MCP server context.
///
/// This wrapper bridges the gap between the raw SDK client and MCP server needs,
/// providing async methods optimized for MCP tool calls and resource reads.
#[derive(Clone)]
pub struct McpClientWrapper {
    /// The underlying SDK adapter for connection management
    adapter: Arc<McpSdkAdapter>,
    /// Current network configuration
    network_config: MantraNetworkConfig,
}

impl McpClientWrapper {
    /// Create a new MCP client wrapper
    ///
    /// # Arguments
    ///
    /// * `adapter` - The SDK adapter for connection and cache management
    /// * `network_config` - Current network configuration
    ///
    /// # Returns
    ///
    /// A new `McpClientWrapper` instance
    pub fn new(adapter: Arc<McpSdkAdapter>, network_config: MantraNetworkConfig) -> Self {
        Self {
            adapter,
            network_config,
        }
    }

    /// Get wallet balance with MCP-specific error handling
    ///
    /// # Arguments
    ///
    /// * `address` - Optional wallet address, uses active wallet if None
    ///
    /// # Returns
    ///
    /// JSON value containing balance information or error details
    pub async fn get_wallet_balance(&self, address: Option<&str>) -> McpResult<JsonValue> {
        debug!("Getting wallet balance for address: {:?}", address);

        // If no address provided, use active wallet
        let wallet_address = if let Some(addr) = address {
            addr.to_string()
        } else {
            // Get active wallet info
            let wallet_info = self.adapter.get_active_wallet_info().await?;
            match wallet_info {
                Some(info) => info.address,
                None => {
                    return Ok(serde_json::json!({
                        "error": "No active wallet found",
                        "balances": [],
                        "address": null
                    }));
                }
            }
        };

        // Execute balance query with retry logic
        let adapter = self.adapter.clone();
        let network_config = self.network_config.clone();
        let adapter_clone = adapter.clone();
        let network_config_clone = network_config.clone();
        let result = adapter
            .execute_with_retry(move || {
                let adapter = adapter_clone.clone();
                let network_config = network_config_clone.clone();
                Box::pin(async move {
                    let client = adapter
                        .get_client(&network_config)
                        .await
                        .map_err(|e| McpServerError::Mcp(format!("Failed to get client: {}", e)))?;

                    // Check if client has a wallet configured
                    if client.wallet().is_err() {
                        // Try to set active wallet for this query
                        if let Some(wallet) = adapter.get_active_wallet().await.map_err(|e| {
                            McpServerError::Validation(format!(
                                "Failed to get active wallet: {}",
                                e
                            ))
                        })? {
                            let client_with_wallet = client.with_wallet(wallet);
                            client_with_wallet.get_balances().await.map_err(|e| {
                                McpServerError::Mcp(format!("Failed to get balances: {}", e))
                            })
                        } else {
                            Err(McpServerError::Validation(
                                "No wallet available for balance query".to_string(),
                            ))
                        }
                    } else {
                        client.get_balances().await.map_err(|e| {
                            McpServerError::Mcp(format!("Failed to get balances: {}", e))
                        })
                    }
                })
            })
            .await;

        match result {
            Ok(balances) => {
                info!(
                    "Successfully retrieved {} token balances for address {}",
                    balances.len(),
                    wallet_address
                );

                // Convert balances to JSON format
                let balance_json: Vec<JsonValue> = balances
                    .into_iter()
                    .map(|coin| {
                        serde_json::json!({
                            "denom": coin.denom,
                            "amount": coin.amount.to_string()
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "address": wallet_address,
                    "balances": balance_json,
                    "total_tokens": balance_json.len()
                }))
            }
            Err(e) => {
                warn!("Failed to get wallet balance: {}", e);
                Ok(serde_json::json!({
                    "error": format!("Failed to retrieve balance: {}", e),
                    "address": wallet_address,
                    "balances": []
                }))
            }
        }
    }

    /// Get wallet information with MCP formatting
    ///
    /// # Returns
    ///
    /// JSON value containing wallet information
    pub async fn get_wallet_info(&self) -> McpResult<JsonValue> {
        debug!("Getting wallet information");

        match self.adapter.get_active_wallet_info().await? {
            Some(info) => Ok(serde_json::json!({
                "address": info.address,
                "public_key": info.public_key,
                "network": self.network_config.chain_id,
                "status": "active"
            })),
            None => Ok(serde_json::json!({
                "address": null,
                "public_key": null,
                "network": self.network_config.chain_id,
                "status": "no_active_wallet"
            })),
        }
    }

    /// Get network status information
    ///
    /// # Returns
    ///
    /// JSON value containing network status
    pub async fn get_network_status(&self) -> McpResult<JsonValue> {
        debug!("Getting network status");

        let adapter = self.adapter.clone();
        let network_config = self.network_config.clone();
        let adapter_clone = adapter.clone();
        let network_config_clone = network_config.clone();
        let result = adapter
            .execute_with_retry(move || {
                let adapter = adapter_clone.clone();
                let network_config = network_config_clone.clone();
                Box::pin(async move {
                    let client = adapter
                        .get_client(&network_config)
                        .await
                        .map_err(|e| McpServerError::Mcp(format!("Failed to get client: {}", e)))?;
                    client.get_last_block_height().await.map_err(|e| {
                        McpServerError::Mcp(format!("Failed to get block height: {}", e))
                    })
                })
            })
            .await;

        match result {
            Ok(height) => {
                info!("Network status retrieved - block height: {}", height);
                Ok(serde_json::json!({
                    "network": self.network_config.chain_id,
                    "rpc_url": self.network_config.rpc_url,
                    "block_height": height,
                    "status": "connected"
                }))
            }
            Err(e) => {
                warn!("Failed to get network status: {}", e);
                Ok(serde_json::json!({
                    "network": self.network_config.chain_id,
                    "rpc_url": self.network_config.rpc_url,
                    "block_height": null,
                    "status": "disconnected",
                    "error": format!("Connection error: {}", e)
                }))
            }
        }
    }

    /// Switch to a different network
    ///
    /// # Arguments
    ///
    /// * `new_config` - New network configuration
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub async fn switch_network(
        &mut self,
        new_config: MantraNetworkConfig,
    ) -> McpResult<JsonValue> {
        debug!("Switching network to: {}", new_config.chain_id);

        // Update internal network config
        self.network_config = new_config.clone();

        // Clear adapter caches to force reconnection
        self.adapter.cache_clear().await;
        self.adapter.cleanup().await?;

        info!("Successfully switched to network: {}", new_config.chain_id);

        Ok(serde_json::json!({
            "previous_network": self.network_config.chain_id,
            "current_network": new_config.chain_id,
            "status": "switched"
        }))
    }

    /// Validate wallet operations before execution
    ///
    /// # Arguments
    ///
    /// * `operation` - Operation type to validate
    ///
    /// # Returns
    ///
    /// Validation result
    pub async fn validate_wallet_operation(&self, operation: &str) -> McpResult<JsonValue> {
        debug!("Validating wallet operation: {}", operation);

        // Check if we have an active wallet
        let has_wallet = self.adapter.get_active_wallet_info().await?.is_some();

        if !has_wallet {
            return Ok(serde_json::json!({
                "operation": operation,
                "valid": false,
                "error": "No active wallet available"
            }));
        }

        // Check network connectivity
        let network_status = self.get_network_status().await?;
        let network_connected = network_status
            .get("status")
            .and_then(|s| s.as_str())
            .map(|s| s == "connected")
            .unwrap_or(false);

        if !network_connected {
            return Ok(serde_json::json!({
                "operation": operation,
                "valid": false,
                "error": "Network not connected"
            }));
        }

        Ok(serde_json::json!({
            "operation": operation,
            "valid": true,
            "wallet_available": has_wallet,
            "network_connected": network_connected
        }))
    }

    /// Get contract addresses for current network
    ///
    /// # Returns
    ///
    /// JSON value containing contract addresses
    pub async fn get_contract_addresses(&self) -> McpResult<JsonValue> {
        debug!(
            "Getting contract addresses for network: {}",
            self.network_config.chain_id
        );

        Ok(serde_json::json!({
            "network": self.network_config.chain_id,
            "contracts": {
                "pool_manager": self.network_config.contracts.pool_manager,
                "fee_collector": self.network_config.contracts.fee_collector
            },
            "rpc_url": self.network_config.rpc_url
        }))
    }

    /// Get comprehensive server health status
    ///
    /// # Returns
    ///
    /// JSON value containing health information
    pub async fn get_health_status(&self) -> McpResult<JsonValue> {
        debug!("Getting comprehensive health status");

        // Check wallet status
        let wallet_status = self.adapter.get_active_wallet_info().await?.is_some();

        // Check network connectivity
        let network_result = self.get_network_status().await?;
        let network_connected = network_result
            .get("status")
            .and_then(|s| s.as_str())
            .map(|s| s == "connected")
            .unwrap_or(false);

        // Overall health assessment
        let overall_healthy = wallet_status && network_connected;

        Ok(serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": if overall_healthy { "healthy" } else { "degraded" },
            "components": {
                "wallet": {
                    "status": if wallet_status { "active" } else { "no_wallet" },
                    "healthy": wallet_status
                },
                "network": {
                    "status": if network_connected { "connected" } else { "disconnected" },
                    "healthy": network_connected,
                    "network": self.network_config.chain_id
                },
                "cache": {
                    "status": "operational",
                    "healthy": true
                }
            }
        }))
    }
}

impl Default for McpClientWrapper {
    fn default() -> Self {
        // Create a default testnet configuration
        let testnet_constants = crate::config::NetworkConstants {
            network_name: "mantra-dukong".to_string(),
            chain_id: "mantra-dukong-1".to_string(),
            default_rpc: "https://rpc.dukong.mantrachain.io".to_string(),
            default_gas_price: 0.01,
            default_gas_adjustment: 1.5,
            native_denom: "uaum".to_string(),
        };

        let config = MantraNetworkConfig::from_constants(&testnet_constants).unwrap_or_else(|_| {
            // Fallback to creating a network config manually if loading fails
            MantraNetworkConfig {
                network_name: testnet_constants.network_name.clone(),
                chain_id: testnet_constants.chain_id.clone(),
                rpc_url: testnet_constants.default_rpc.clone(),
                gas_price: testnet_constants.default_gas_price,
                gas_adjustment: testnet_constants.default_gas_adjustment,
                native_denom: testnet_constants.native_denom.clone(),
                contracts: crate::config::ContractAddresses::default(),
                #[cfg(feature = "evm")]
                evm_rpc_url: None,
                #[cfg(feature = "evm")]
                evm_chain_id: None,
            }
        });

        Self::new(Arc::new(McpSdkAdapter::default()), config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MantraNetworkConfig;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_client_wrapper_creation() {
        let adapter = Arc::new(McpSdkAdapter::default());

        // Create a testnet configuration
        let testnet_constants = crate::config::NetworkConstants {
            network_name: "mantra-dukong".to_string(),
            chain_id: "mantra-dukong-1".to_string(),
            default_rpc: "https://rpc.dukong.mantrachain.io".to_string(),
            default_gas_price: 0.01,
            default_gas_adjustment: 1.5,
            native_denom: "uaum".to_string(),
        };
        let config = MantraNetworkConfig::from_constants(&testnet_constants)
            .expect("Failed to create network config in test");

        let wrapper = McpClientWrapper::new(adapter, config);

        // Should be able to create wrapper without errors
        assert!(!wrapper.network_config.chain_id.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_wallet_info_no_wallet() {
        let wrapper = McpClientWrapper::default();
        let result = wrapper.get_wallet_info().await.unwrap();

        // Should return status indicating no active wallet
        assert_eq!(result["status"], "no_active_wallet");
        assert!(result["address"].is_null());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_contract_addresses() {
        let wrapper = McpClientWrapper::default();
        let result = wrapper.get_contract_addresses().await.unwrap();

        // Should return contract information
        assert_eq!(result["network"], "mantra-dukong-1");
        assert!(result["contracts"]["pool_manager"].is_string());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_validate_wallet_operation_no_wallet() {
        let wrapper = McpClientWrapper::default();
        let result = wrapper
            .validate_wallet_operation("balance_query")
            .await
            .unwrap();

        // Should indicate operation is not valid due to no wallet
        assert_eq!(result["valid"], false);
        assert_eq!(result["operation"], "balance_query");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_health_status() {
        let wrapper = McpClientWrapper::default();
        let result = wrapper.get_health_status().await.unwrap();

        // Should return health status structure
        assert!(result["timestamp"].is_string());
        assert!(result["components"]["wallet"]["status"].is_string());
        assert!(result["components"]["network"]["status"].is_string());
    }
}

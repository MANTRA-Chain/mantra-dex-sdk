//! Network validation and configuration methods

use super::*;

impl McpSdkAdapter {
    /// Get the default network configuration
    /// This is a temporary method until proper network configuration management is implemented
    pub(crate) async fn get_default_network_config(&self) -> McpResult<MantraNetworkConfig> {
        // Load environment configuration which includes EVM settings
        use crate::config::env::EnvironmentConfig;

        let env_config = EnvironmentConfig::load().map_err(|e| {
            McpServerError::Internal(format!("Failed to load environment config: {}", e))
        })?;

        let network_config = MantraNetworkConfig::from_env_config(&env_config).map_err(|e| {
            McpServerError::Internal(format!("Failed to create network config: {}", e))
        })?;
        Ok(network_config)
    }

    /// Validate network connectivity (for script execution)
    pub async fn validate_network_connectivity(&self) -> McpResult<Value> {
        debug!("SDK Adapter: Validating network connectivity");

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Try to get a client to validate connectivity
        match self.get_client(&network_config).await {
            Ok(_) => Ok(serde_json::json!({
                "status": "success",
                "network": network_config.network_name,
                "chain_id": network_config.chain_id,
                "connectivity": "healthy",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
            Err(e) => Ok(serde_json::json!({
                "status": "error",
                "network": network_config.network_name,
                "chain_id": network_config.chain_id,
                "connectivity": "failed",
                "error": e.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
        }
    }

    /// Get contract addresses (for script execution)
    pub async fn get_contract_addresses(&self) -> McpResult<Value> {
        debug!("SDK Adapter: Getting contract addresses");

        // Get network config
        let network_config = self.get_default_network_config().await?;

        Ok(serde_json::json!({
            "status": "success",
            "network": network_config.network_name,
            "chain_id": network_config.chain_id,
            "contracts": {
                "pool_manager": network_config.contracts.pool_manager,
                "fee_collector": network_config.contracts.fee_collector,
                "farm_manager": network_config.contracts.farm_manager,
                "epoch_manager": network_config.contracts.epoch_manager
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Monitor a transaction by hash with timeout (for script execution)
    pub async fn monitor_transaction(
        &self,
        tx_hash: String,
        timeout_seconds: Option<u64>,
    ) -> McpResult<Value> {
        debug!("SDK Adapter: Monitoring transaction: {}", tx_hash);

        let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30));
        let start_time = Instant::now();

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Poll the transaction with timeout
        let poll_interval = Duration::from_secs(2);

        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() > timeout {
                return Ok(serde_json::json!({
                    "status": "timeout",
                    "tx_hash": tx_hash,
                    "message": format!("Transaction monitoring timed out after {} seconds", timeout.as_secs()),
                    "elapsed_seconds": start_time.elapsed().as_secs(),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }

            // Query the transaction
            match client.query_transaction(&tx_hash).await {
                Ok(tx_result) => {
                    // Check if the transaction has a result code
                    if let Some(tx_result_obj) = tx_result.get("tx_result") {
                        if let Some(code) = tx_result_obj.get("code").and_then(|c| c.as_u64()) {
                            let status = if code == 0 { "success" } else { "failed" };

                            return Ok(serde_json::json!({
                                "status": status,
                                "tx_hash": tx_hash,
                                "code": code,
                                "height": tx_result.get("height"),
                                "gas_used": tx_result_obj.get("gas_used"),
                                "gas_wanted": tx_result_obj.get("gas_wanted"),
                                "log": tx_result_obj.get("log"),
                                "events": tx_result_obj.get("events"),
                                "elapsed_seconds": start_time.elapsed().as_secs(),
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }));
                        }
                    }

                    // If we can't determine the status, but got a result, it's likely pending
                    debug!(
                        "Transaction {} found but status unclear, continuing to monitor",
                        tx_hash
                    );
                }
                Err(e) => {
                    // If the transaction is not found, it might still be pending
                    debug!("Transaction {} not found or error occurred: {}", tx_hash, e);
                }
            }

            // Wait before polling again
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Execute a custom MCP tool by name with parameters (for script execution)
    pub async fn execute_custom_tool(
        &self,
        tool_name: &str,
        parameters: &HashMap<String, String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Executing custom tool: {} with parameters: {:?}",
            tool_name, parameters
        );

        // Convert string parameters to serde_json::Value
        let mut json_params = serde_json::Map::new();
        for (key, value) in parameters {
            // Try to parse as JSON first, fallback to string
            let json_value = if let Ok(parsed) = serde_json::from_str::<Value>(value) {
                parsed
            } else {
                Value::String(value.clone())
            };
            json_params.insert(key.clone(), json_value);
        }
        let args = Value::Object(json_params);

        // Validate parameters before tool execution
        Self::validate_tool_parameters(tool_name, parameters)?;

        // Route to appropriate tool based on tool_name
        match tool_name {
            "get_balances" => {
                // get_balances needs network_config and wallet_address parameters
                let network_config = self.get_default_network_config().await?;
                let wallet_address = parameters.get("wallet_address").cloned();
                self.get_balances(&network_config, wallet_address).await
            }
            "get_pool" => {
                let pool_id = parameters
                    .get("pool_id")
                    .ok_or_else(|| {
                        McpServerError::InvalidArguments("pool_id parameter required".to_string())
                    })?
                    .clone();
                self.get_pool_info(pool_id).await
            }
            "get_pools" => self.get_pools(args).await,
            "swap" | "execute_swap" => self.execute_swap(args).await,
            "provide_liquidity" => self.provide_liquidity(args).await,
            "withdraw_liquidity" => self.withdraw_liquidity(args).await,
            "create_pool" => self.create_pool(args).await,
            "get_lp_token_balance" => self.get_lp_token_balance(args).await,
            "get_all_lp_token_balances" => self.get_all_lp_token_balances(args).await,
            "validate_network" => self.validate_network_connectivity().await,
            "get_contracts" => self.get_contract_addresses().await,
            "monitor_transaction" => {
                // Special handling for monitor_transaction which needs different parameters
                let tx_hash = parameters
                    .get("tx_hash")
                    .ok_or_else(|| {
                        McpServerError::InvalidArguments("tx_hash parameter required".to_string())
                    })?
                    .clone();
                let timeout_seconds = parameters
                    .get("timeout")
                    .and_then(|t| t.parse::<u64>().ok());

                self.monitor_transaction(tx_hash, timeout_seconds).await
            }
            #[cfg(feature = "evm")]
            "evm_call" => self.evm_call(args).await,
            #[cfg(feature = "evm")]
            "evm_send" => self.evm_send(args).await,
            #[cfg(feature = "evm")]
            "evm_estimate_gas" => self.evm_estimate_gas(args).await,
            #[cfg(feature = "evm")]
            "evm_get_logs" => self.evm_get_logs(args).await,
            #[cfg(feature = "evm")]
            "evm_deploy" => self.evm_deploy(args).await,
            #[cfg(feature = "evm")]
            "evm_load_abi" => self.evm_load_abi(args).await,
            _ => {
                // For unknown tools, return an error result
                Ok(serde_json::json!({
                    "status": "error",
                    "tool_name": tool_name,
                    "message": format!("Unknown tool: {}. Available tools: get_balances, get_pool, get_pools, swap, execute_swap, provide_liquidity, withdraw_liquidity, create_pool, get_lp_token_balance, get_all_lp_token_balances, validate_network, get_contracts, monitor_transaction", tool_name),
                    "parameters": parameters,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
        }
    }

    /// Validate tool parameters before execution
    pub(crate) fn validate_tool_parameters(
        tool_name: &str,
        parameters: &HashMap<String, String>,
    ) -> McpResult<()> {
        match tool_name {
            "get_pool" => {
                if let Some(pool_id) = parameters.get("pool_id") {
                    if pool_id.trim().is_empty() {
                        return Err(McpServerError::InvalidArguments(
                            "pool_id cannot be empty".to_string(),
                        ));
                    }
                } else {
                    return Err(McpServerError::InvalidArguments(
                        "pool_id parameter is required".to_string(),
                    ));
                }
            }
            "monitor_transaction" => {
                if let Some(tx_hash) = parameters.get("tx_hash") {
                    if tx_hash.trim().is_empty() {
                        return Err(McpServerError::InvalidArguments(
                            "tx_hash cannot be empty".to_string(),
                        ));
                    }
                    // Validate tx_hash format (should be hex)
                    if !tx_hash
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() || c.is_ascii_uppercase())
                    {
                        return Err(McpServerError::InvalidArguments(
                            "tx_hash must contain only hexadecimal characters".to_string(),
                        ));
                    }
                } else {
                    return Err(McpServerError::InvalidArguments(
                        "tx_hash parameter is required".to_string(),
                    ));
                }

                // Validate timeout if present
                if let Some(timeout) = parameters.get("timeout") {
                    if let Ok(timeout_val) = timeout.parse::<u64>() {
                        if timeout_val == 0 || timeout_val > 300 {
                            return Err(McpServerError::InvalidArguments(
                                "timeout must be between 1 and 300 seconds".to_string(),
                            ));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(
                            "timeout must be a valid number".to_string(),
                        ));
                    }
                }
            }
            "get_balances" => {
                // Validate wallet_address if present
                if let Some(wallet_addr) = parameters.get("wallet_address") {
                    if !wallet_addr.trim().is_empty() && !wallet_addr.starts_with("mantra") {
                        return Err(McpServerError::InvalidArguments(
                            "wallet_address must be a valid Mantra address (starts with 'mantra')"
                                .to_string(),
                        ));
                    }
                }
            }
            "swap" | "execute_swap" => {
                // Validate required swap parameters
                for required_param in &["asset_in", "asset_out", "amount_in"] {
                    if let Some(value) = parameters.get(*required_param) {
                        if value.trim().is_empty() {
                            return Err(McpServerError::InvalidArguments(format!(
                                "{} cannot be empty",
                                required_param
                            )));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(format!(
                            "{} parameter is required",
                            required_param
                        )));
                    }
                }

                // Validate amount_in is numeric
                if let Some(amount) = parameters.get("amount_in") {
                    if amount.parse::<u128>().is_err() {
                        return Err(McpServerError::InvalidArguments(
                            "amount_in must be a valid number".to_string(),
                        ));
                    }
                }
            }
            _ => {
                // Other tools don't need specific validation
                debug!("No specific validation for tool: {}", tool_name);
            }
        }
        Ok(())
    }
}

//! EVM protocol methods (feature-gated)

#[cfg(feature = "evm")]
use super::*;

#[cfg(feature = "evm")]
impl McpSdkAdapter {
    // EVM Protocol Tools
    // =============================================================================

    /// Execute a read-only contract call on EVM
    #[cfg(feature = "evm")]
    pub async fn evm_call(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Executing EVM call with args: {:?}", args);

        // Parse required parameters
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("contract_address is required".to_string())
            })?;

        let call_data = args
            .get("call_data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("call_data is required".to_string()))?;

        // Parse optional parameters
        let block_number = args.get("block_number").and_then(|v| v.as_str());

        // Validate contract address format
        if !contract_address.starts_with("0x") || contract_address.len() != 42 {
            return Err(McpServerError::InvalidArguments(
                "contract_address must be a valid Ethereum address (0x...)".to_string(),
            ));
        }

        // Validate call data format
        if !call_data.starts_with("0x") {
            return Err(McpServerError::InvalidArguments(
                "call_data must be hex-encoded (start with 0x)".to_string(),
            ));
        }

        // Get EVM client
        let client = MantraClient::new(self.get_default_network_config().await?, None)
            .await
            .map_err(McpServerError::Sdk)?;

        let evm_client = client.evm().await.map_err(McpServerError::Sdk)?;

        // Parse contract address
        let contract_addr = alloy_primitives::Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // Parse call data
        let call_data_bytes = hex::decode(&call_data[2..])
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid call data: {}", e)))?;

        // Create call request
        let call_request = crate::protocols::evm::types::EvmCallRequest::new(
            crate::protocols::evm::types::EthAddress(contract_addr),
            call_data_bytes,
        );

        if let Some(block) = block_number {
            let request = call_request.at_block(block.to_string());
            let result = evm_client
                .call(request)
                .await
                .map_err(McpServerError::Sdk)?;
            Ok(serde_json::json!({
                "status": "success",
                "operation": "evm_call",
                "contract_address": contract_address,
                "call_data": call_data,
                "block_number": block,
                "result": format!("0x{}", hex::encode(&result)),
                "result_length": result.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        } else {
            let result = evm_client
                .call(call_request)
                .await
                .map_err(McpServerError::Sdk)?;
            Ok(serde_json::json!({
                "status": "success",
                "operation": "evm_call",
                "contract_address": contract_address,
                "call_data": call_data,
                "result": format!("0x{}", hex::encode(&result)),
                "result_length": result.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
    }

    /// Submit a transaction to EVM
    #[cfg(feature = "evm")]
    pub async fn evm_send(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Sending EVM transaction with args: {:?}", args);

        // Parse required parameters
        let to_address = args
            .get("to_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("to_address is required".to_string())
            })?;

        let value = args
            .get("value")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("value is required".to_string()))?;

        // Parse optional parameters
        let data = args.get("data").and_then(|v| v.as_str());
        let gas_limit = args.get("gas_limit").and_then(|v| v.as_u64());
        let max_fee_per_gas = args.get("max_fee_per_gas").and_then(|v| v.as_str());
        let max_priority_fee_per_gas = args
            .get("max_priority_fee_per_gas")
            .and_then(|v| v.as_str());

        // Validate addresses
        if !to_address.starts_with("0x") || to_address.len() != 42 {
            return Err(McpServerError::InvalidArguments(
                "to_address must be a valid Ethereum address (0x...)".to_string(),
            ));
        }

        // Parse value
        let value_u256 = alloy_primitives::U256::from_str(value)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid value: {}", e)))?;

        // Get wallet and EVM client
        let wallet = self.get_active_wallet_with_validation().await?;
        let client = MantraClient::new(
            self.get_default_network_config().await?,
            Some(Arc::new(wallet)),
        )
        .await
        .map_err(McpServerError::Sdk)?;

        let evm_client = client.evm().await.map_err(McpServerError::Sdk)?;

        // Parse addresses and data
        let to_addr = alloy_primitives::Address::from_str(to_address)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid to_address: {}", e)))?;

        let tx_data = if let Some(data_str) = data {
            if !data_str.starts_with("0x") {
                return Err(McpServerError::InvalidArguments(
                    "data must be hex-encoded (start with 0x)".to_string(),
                ));
            }
            hex::decode(&data_str[2..])
                .map_err(|e| McpServerError::InvalidArguments(format!("Invalid data: {}", e)))?
        } else {
            Vec::new()
        };

        // Create transaction request
        let mut tx_request =
            crate::protocols::evm::types::EvmTransactionRequest::new(evm_client.chain_id())
                .to(crate::protocols::evm::types::EthAddress(to_addr))
                .value(value_u256)
                .data(tx_data);

        if let Some(gas) = gas_limit {
            tx_request = tx_request.gas_limit(gas);
        }

        if let Some(max_fee) = max_fee_per_gas {
            let max_fee_u256 = alloy_primitives::U256::from_str(max_fee).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid max_fee_per_gas: {}", e))
            })?;
            tx_request = tx_request.eip1559_fees(max_fee_u256, alloy_primitives::U256::from(0));
        }

        if let Some(priority_fee) = max_priority_fee_per_gas {
            let priority_fee_u256 =
                alloy_primitives::U256::from_str(priority_fee).map_err(|e| {
                    McpServerError::InvalidArguments(format!(
                        "Invalid max_priority_fee_per_gas: {}",
                        e
                    ))
                })?;
            // Update the priority fee if max_fee was already set
            if let Some(max_fee) = max_fee_per_gas {
                let max_fee_u256 = alloy_primitives::U256::from_str(max_fee).unwrap();
                tx_request = tx_request.eip1559_fees(max_fee_u256, priority_fee_u256);
            }
        }

        // Estimate gas if not provided
        let final_gas_limit = if let Some(gas) = gas_limit {
            gas
        } else {
            evm_client
                .estimate_gas(tx_request.clone())
                .await
                .map_err(McpServerError::Sdk)?
        };

        // TODO: Implement actual transaction submission
        // For now, return a placeholder response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "evm_send",
            "to_address": to_address,
            "value": value,
            "gas_limit": final_gas_limit,
            "estimated_gas": final_gas_limit,
            "transaction_hash": "0x0000000000000000000000000000000000000000000000000000000000000000", // Placeholder
            "note": "Transaction submission not yet fully implemented",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Estimate gas for an EVM transaction
    #[cfg(feature = "evm")]
    pub async fn evm_estimate_gas(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Estimating EVM gas with args: {:?}", args);

        // Parse required parameters
        let to_address = args
            .get("to_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("to_address is required".to_string())
            })?;

        let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("0");

        // Parse optional parameters
        let data = args.get("data").and_then(|v| v.as_str());

        // Validate address
        if !to_address.starts_with("0x") || to_address.len() != 42 {
            return Err(McpServerError::InvalidArguments(
                "to_address must be a valid Ethereum address (0x...)".to_string(),
            ));
        }

        // Get EVM client
        let client = MantraClient::new(self.get_default_network_config().await?, None)
            .await
            .map_err(McpServerError::Sdk)?;

        let evm_client = client.evm().await.map_err(McpServerError::Sdk)?;

        // Parse parameters
        let to_addr = alloy_primitives::Address::from_str(to_address)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid to_address: {}", e)))?;

        let value_u256 = alloy_primitives::U256::from_str(value)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid value: {}", e)))?;

        let tx_data = if let Some(data_str) = data {
            if !data_str.starts_with("0x") {
                return Err(McpServerError::InvalidArguments(
                    "data must be hex-encoded (start with 0x)".to_string(),
                ));
            }
            hex::decode(&data_str[2..])
                .map_err(|e| McpServerError::InvalidArguments(format!("Invalid data: {}", e)))?
        } else {
            Vec::new()
        };

        // Create transaction request
        let tx_request =
            crate::protocols::evm::types::EvmTransactionRequest::new(evm_client.chain_id())
                .to(crate::protocols::evm::types::EthAddress(to_addr))
                .value(value_u256)
                .data(tx_data);

        // Estimate gas
        let gas_estimate = evm_client
            .estimate_gas(tx_request)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get current fee data
        let (base_fee, priority_fee) = evm_client
            .get_fee_data()
            .await
            .map_err(McpServerError::Sdk)?;

        let gas_price = base_fee + priority_fee;
        let estimated_cost_wei = gas_price.to_string();

        Ok(serde_json::json!({
            "status": "success",
            "operation": "evm_estimate_gas",
            "to_address": to_address,
            "value": value,
            "gas_estimate": gas_estimate,
            "fee_data": {
                "base_fee_per_gas": base_fee.to_string(),
                "suggested_priority_fee": priority_fee.to_string(),
                "estimated_max_fee_per_gas": gas_price.to_string()
            },
            "estimated_cost_wei": estimated_cost_wei,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Query EVM event logs
    #[cfg(feature = "evm")]
    pub async fn evm_get_logs(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting EVM logs with args: {:?}", args);

        // Parse required parameters
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("contract_address is required".to_string())
            })?;

        // Parse optional parameters
        let event_signature = args.get("event_signature").and_then(|v| v.as_str());
        let from_block = args.get("from_block").and_then(|v| v.as_str());
        let to_block = args.get("to_block").and_then(|v| v.as_str());
        let topics = args.get("topics").and_then(|v| v.as_array());

        // Validate contract address
        if !contract_address.starts_with("0x") || contract_address.len() != 42 {
            return Err(McpServerError::InvalidArguments(
                "contract_address must be a valid Ethereum address (0x...)".to_string(),
            ));
        }

        // Get EVM client
        let client = MantraClient::new(self.get_default_network_config().await?, None)
            .await
            .map_err(McpServerError::Sdk)?;

        let evm_client = client.evm().await.map_err(McpServerError::Sdk)?;

        // Parse contract address
        let contract_addr = alloy_primitives::Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract_address: {}", e))
        })?;

        // Build event topics
        let mut event_topics = vec![{
            let mut bytes = [0u8; 32];
            bytes[12..].copy_from_slice(contract_addr.0.as_slice());
            alloy_primitives::B256::from(bytes)
        }];

        if let Some(sig) = event_signature {
            if !sig.starts_with("0x") {
                return Err(McpServerError::InvalidArguments(
                    "event_signature must be hex-encoded (start with 0x)".to_string(),
                ));
            }
            let topic_hash = alloy_primitives::B256::from_str(sig).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid event_signature: {}", e))
            })?;
            event_topics.insert(0, topic_hash);
        }

        if let Some(topics_array) = topics {
            for topic in topics_array {
                if let Some(topic_str) = topic.as_str() {
                    if topic_str.starts_with("0x") {
                        let topic_hash =
                            alloy_primitives::B256::from_str(topic_str).map_err(|e| {
                                McpServerError::InvalidArguments(format!(
                                    "Invalid topic: {}",
                                    topic_str
                                ))
                            })?;
                        event_topics.push(topic_hash);
                    }
                }
            }
        }

        // Create event filter
        let mut filter = crate::protocols::evm::types::EventFilter::new().addresses(vec![
            crate::protocols::evm::types::EthAddress(contract_addr),
        ]);

        if let Some(from) = from_block {
            filter = filter.block_range(Some(from.to_string()), to_block.map(|s| s.to_string()));
        }

        // Set topics (simplified - in practice, this would be more complex)
        let mut topics_vec = Vec::new();
        for topic in event_topics.into_iter().skip(1) {
            topics_vec.push(Some(topic));
        }
        filter.topics = topics_vec;

        // Query logs
        let logs = evm_client
            .get_logs(filter)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "evm_get_logs",
            "contract_address": contract_address,
            "event_signature": event_signature,
            "from_block": from_block,
            "to_block": to_block,
            "logs": logs.iter().map(|log| {
                serde_json::json!({
                    "address": format!("{:?}", log.address()),
                    "topics": log.topics().iter().map(|t| format!("{:?}", t)).collect::<Vec<_>>(),
                    "data": format!("0x{}", hex::encode(log.data().data.0.clone())),
                    "block_number": log.block_number,
                    "transaction_hash": format!("{:?}", log.transaction_hash),
                    "log_index": log.log_index
                })
            }).collect::<Vec<_>>(),
            "log_count": logs.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Deploy a contract to EVM
    #[cfg(feature = "evm")]
    pub async fn evm_deploy(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Deploying EVM contract with args: {:?}", args);

        // Parse required parameters
        let bytecode = args
            .get("bytecode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("bytecode is required".to_string()))?;

        // Parse optional parameters
        let constructor_args = args.get("constructor_args").and_then(|v| v.as_str());
        let value = args.get("value").and_then(|v| v.as_str()).unwrap_or("0");

        // Validate bytecode
        if !bytecode.starts_with("0x") {
            return Err(McpServerError::InvalidArguments(
                "bytecode must be hex-encoded (start with 0x)".to_string(),
            ));
        }

        // Parse value
        let value_u256 = alloy_primitives::U256::from_str(value)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid value: {}", value)))?;

        // Get wallet and EVM client
        let wallet = self.get_active_wallet_with_validation().await?;
        let client = MantraClient::new(
            self.get_default_network_config().await?,
            Some(Arc::new(wallet)),
        )
        .await
        .map_err(McpServerError::Sdk)?;

        let evm_client = client.evm().await.map_err(McpServerError::Sdk)?;

        // Combine bytecode with constructor args if provided
        let mut deploy_data = hex::decode(&bytecode[2..])
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid bytecode: {}", e)))?;

        if let Some(args_hex) = constructor_args {
            if !args_hex.starts_with("0x") {
                return Err(McpServerError::InvalidArguments(
                    "constructor_args must be hex-encoded (start with 0x)".to_string(),
                ));
            }
            let args_bytes = hex::decode(&args_hex[2..]).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid constructor_args: {}", e))
            })?;
            deploy_data.extend(args_bytes);
        }

        // Create deployment transaction
        let tx_request =
            crate::protocols::evm::types::EvmTransactionRequest::new(evm_client.chain_id())
                .value(value_u256)
                .data(deploy_data);

        // Estimate gas
        let gas_estimate = evm_client
            .estimate_gas(tx_request)
            .await
            .map_err(McpServerError::Sdk)?;

        // TODO: Implement actual contract deployment
        Ok(serde_json::json!({
            "status": "success",
            "operation": "evm_deploy",
            "bytecode_length": bytecode.len(),
            "constructor_args_provided": constructor_args.is_some(),
            "value": value,
            "estimated_gas": gas_estimate,
            "contract_address": "0x0000000000000000000000000000000000000000", // Placeholder - would be computed
            "transaction_hash": "0x0000000000000000000000000000000000000000000000000000000000000000", // Placeholder
            "note": "Contract deployment not yet fully implemented",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Load an ABI for contract interaction
    #[cfg(feature = "evm")]
    pub async fn evm_load_abi(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Loading EVM ABI with args: {:?}", args);

        // Parse required parameters
        let abi_json = args
            .get("abi_json")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("abi_json is required".to_string()))?;

        let abi_key = args
            .get("abi_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("abi_key is required".to_string()))?;

        // Parse optional parameters
        let abi_file_path = args.get("abi_file_path").and_then(|v| v.as_str());

        // TODO: Implement ABI loading and caching
        // For now, return a placeholder response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "evm_load_abi",
            "abi_key": abi_key,
            "abi_json_length": abi_json.len(),
            "abi_file_path": abi_file_path,
            "functions_loaded": 0, // Placeholder
            "events_loaded": 0, // Placeholder
            "note": "ABI loading not yet fully implemented",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Query native EVM balance
    #[cfg(feature = "evm")]
    pub async fn get_native_evm_balance(
        &self,
        wallet_address: Option<String>,
    ) -> McpResult<String> {
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;

        // Get EVM client
        let network_config = self.get_default_network_config().await?;
        let evm_rpc_url = network_config.evm_rpc_url.as_ref().ok_or_else(|| {
            McpServerError::InvalidArguments("EVM RPC URL not configured".to_string())
        })?;
        let evm_chain_id = network_config.evm_chain_id.ok_or_else(|| {
            McpServerError::InvalidArguments("EVM chain ID not configured".to_string())
        })?;

        let evm_client = crate::protocols::evm::client::EvmClient::new(evm_rpc_url, evm_chain_id)
            .await
            .map_err(McpServerError::Sdk)?;

        // Query balance
        let evm_address = alloy_primitives::Address::from_str(&evm_addr)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid EVM address: {}", e)))?;

        let balance = evm_client
            .get_balance(crate::protocols::evm::types::EthAddress(evm_address), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Format response
        let balance_om = format_units(balance, 18);
        let mut response = "🔍 **Native EVM Balance**\n\n".to_string();
        response.push_str(&format!("**Cosmos Address:** `{}`\n", cosmos_addr));
        response.push_str(&format!("**EVM Address:** `{}`\n", evm_addr));
        response.push_str(&format!("**Balance:** {} OM\n", balance_om));

        Ok(response)
    }

    /// Get ERC-20 token balance
    #[cfg(feature = "evm")]
    pub async fn get_erc20_balance(
        &self,
        token_address: &str,
        wallet_address: Option<String>,
    ) -> McpResult<String> {
        let (_, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let wallet_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;

        let (evm_client, chain_id) = self.get_evm_client().await?;

        let token_addr = Address::from_str(token_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid token address: {}", e))
        })?;

        let metadata = self
            .ensure_token_metadata(&evm_client, chain_id, token_addr)
            .await?;

        let erc20 = SdkErc20::new(evm_client.clone(), token_addr);
        let balance = erc20.balance_of(wallet_addr).await.map_err(|e| {
            if Self::is_precompile_address(token_addr) {
                McpServerError::Sdk(crate::error::Error::Evm(format!(
                    "Precompile at {:#x} does not implement ERC-20 interface",
                    token_addr
                )))
            } else {
                McpServerError::Sdk(crate::error::Error::Evm(format!(
                    "Failed to query ERC-20 balance for {:#x}: {}",
                    token_addr, e
                )))
            }
        })?;

        let formatted = format_units(balance, metadata.decimals);

        // Format response
        let mut response = "🔍 **ERC-20 Token Balance**\n\n".to_string();
        response.push_str(&format!("**Token:** {}\n", metadata.symbol));
        if let Some(name) = &metadata.name {
            response.push_str(&format!("**Name:** {}\n", name));
        }
        response.push_str(&format!(
            "**Contract:** `{}`\n",
            format!("{:#x}", token_addr)
        ));
        response.push_str(&format!("**Decimals:** {}\n\n", metadata.decimals));
        response.push_str(&format!("**Wallet:** `{}`\n", evm_addr));
        response.push_str(&format!("**Balance:** {} {}\n", formatted, metadata.symbol));
        response.push_str(&format!("**Raw Balance:** {}\n", balance));

        Ok(response)
    }

    /// Get all EVM balances (native + ERC-20 tokens)
    #[cfg(feature = "evm")]
    pub async fn get_all_evm_balances(
        &self,
        wallet_address: Option<String>,
        token_addresses: Option<Vec<String>>,
    ) -> McpResult<EvmBalancesResponse> {
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let wallet_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;

        let (evm_client, chain_id) = self.get_evm_client().await?;

        let native_balance = evm_client
            .get_balance(crate::protocols::evm::types::EthAddress(wallet_addr), None)
            .await
            .map_err(McpServerError::Sdk)?;
        let native_formatted = format_units(native_balance, 18);

        let mut tokens = Vec::new();
        if let Some(addresses) = token_addresses {
            for token_addr in addresses {
                let addr = Address::from_str(&token_addr).map_err(|e| {
                    McpServerError::InvalidArguments(format!(
                        "Invalid token address '{}': {}",
                        token_addr, e
                    ))
                })?;

                let metadata = self
                    .ensure_token_metadata(&evm_client, chain_id, addr)
                    .await?;

                let erc20 = SdkErc20::new(evm_client.clone(), addr);
                let balance = erc20.balance_of(wallet_addr).await.map_err(|e| {
                    McpServerError::Sdk(crate::error::Error::Evm(format!(
                        "Failed to query balance for token {:#x}: {}. Skipping this token.",
                        addr, e
                    )))
                })?;

                tokens.push(Erc20BalanceResponse {
                    token: token_view(&metadata),
                    wallet_address: format!("{:#x}", wallet_addr),
                    raw_balance: balance.to_string(),
                    formatted_balance: format_units(balance, metadata.decimals),
                });
            }
        }

        Ok(EvmBalancesResponse {
            cosmos_address: cosmos_addr,
            evm_address: evm_addr,
            native_balance: native_formatted,
            tokens,
        })
    }

    /// Build, sign, and broadcast an EVM transaction
    ///
    /// # Arguments
    /// * `contract_addr` - Contract address to call
    /// * `call_data` - ABI-encoded function call
    /// * `value` - ETH value to send (usually 0 for token operations)
    /// * `cosmos_addr` - Cosmos wallet address for signing
    /// * `gas_buffer_percent` - Gas estimate buffer (20 for simple, 30 for complex)
    ///
    /// # Returns
    /// Transaction hash on success
    #[cfg(feature = "evm")]
    async fn build_sign_and_broadcast_transaction(
        &self,
        contract_addr: Address,
        call_data: Vec<u8>,
        value: U256,
        cosmos_addr: &str,
        gas_buffer_percent: u64,
    ) -> McpResult<alloy_primitives::B256> {
        use crate::protocols::evm::tx::{Eip1559Transaction, SignedEip1559Transaction};
        use alloy_primitives::Bytes;

        // 1. Get wallet for signing
        let multivm_wallet = self
            .get_multivm_wallet_by_address(cosmos_addr)
            .await?
            .ok_or_else(|| McpServerError::Other("Wallet not found for signing".to_string()))?;

        let from_addr = multivm_wallet.evm_address().map_err(McpServerError::Sdk)?;

        // 2. Get EVM client and chain ID
        let (evm_client, chain_id) = self.get_evm_client().await?;

        // 3. Get nonce
        let nonce = evm_client
            .get_pending_nonce(crate::protocols::evm::types::EthAddress(from_addr))
            .await
            .map_err(McpServerError::Sdk)?;

        // 4. Get fee suggestion
        let fee_suggestion = evm_client
            .fee_suggestion()
            .await
            .map_err(McpServerError::Sdk)?;

        // 5. Build transaction with initial gas limit for estimation
        let mut tx = Eip1559Transaction::new(chain_id, nonce)
            .to(Some(contract_addr))
            .data(Bytes::from(call_data.clone()))
            .value(value)
            .gas_limit(GAS_ESTIMATE_INITIAL)
            .max_fee_per_gas(fee_suggestion.max_fee_per_gas.to::<u128>())
            .max_priority_fee_per_gas(fee_suggestion.max_priority_fee_per_gas.to::<u128>());

        // 6. Estimate gas with buffer
        let gas_estimate = evm_client
            .estimate_gas_with_options(
                (&tx).into(),
                Some(crate::protocols::evm::types::EthAddress(from_addr)),
                None,
            )
            .await
            .map_err(McpServerError::Sdk)?;

        let gas_limit = (gas_estimate * (100 + gas_buffer_percent)) / 100;
        tx = tx.gas_limit(gas_limit);

        // 7. Sign transaction
        let tx_hash = tx.signature_hash();
        let (sig, recid) = multivm_wallet
            .sign_ethereum_tx(tx_hash.as_ref())
            .map_err(McpServerError::Sdk)?;

        // 8. Construct alloy Signature from k256 signature components
        use alloy_primitives::Signature;

        // Convert k256 signature directly to alloy Signature
        #[allow(deprecated)]
        let alloy_sig = Signature::from((sig, recid));

        // 9. Create signed transaction
        let raw_bytes = tx.encode_signed(&alloy_sig);
        let signed_tx = SignedEip1559Transaction::new(tx.into_signed(alloy_sig), raw_bytes);

        // 10. Broadcast
        evm_client
            .send_raw_transaction(&signed_tx)
            .await
            .map_err(McpServerError::Sdk)
    }

    /// Transfer ERC-20 tokens
    #[cfg(feature = "evm")]
    pub async fn transfer_erc20(
        &self,
        token_address: &str,
        recipient: &str,
        amount: &str,
        wallet_address: Option<String>,
    ) -> McpResult<String> {
        // 1. Parse and validate inputs
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let token_addr = Address::from_str(token_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid token address: {}", e))
        })?;
        let to_addr = Address::from_str(recipient)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid recipient: {}", e)))?;

        // 2. Get token metadata and parse amount
        let (evm_client, chain_id) = self.get_evm_client().await?;
        let metadata = self
            .ensure_token_metadata(&evm_client, chain_id, token_addr)
            .await?;
        let amount_u256 = parse_units(amount, metadata.decimals)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // 3. Encode transfer call
        let erc20 = SdkErc20::new(evm_client.clone(), token_addr);
        let call_data = erc20.encode_transfer(to_addr, amount_u256);

        // 4. Use shared helper to build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                token_addr,                // contract address
                call_data,                 // encoded call
                U256::ZERO,                // no ETH value
                &cosmos_addr,              // wallet to use
                GAS_BUFFER_SIMPLE_PERCENT, // 20% buffer
            )
            .await?;

        // 5. Format response
        let formatted_amount = format_units(amount_u256, metadata.decimals);
        let mut response = "✅ **ERC-20 Transfer Submitted**\n\n".to_string();
        response.push_str(&format!(
            "**Token:** {} ({})\n",
            metadata.symbol,
            metadata.name.as_deref().unwrap_or("Unknown")
        ));
        response.push_str(&format!("**Contract:** `{:#x}`\n", token_addr));
        response.push_str(&format!(
            "**From:** `{}` (Cosmos: `{}`)\n",
            evm_addr, cosmos_addr
        ));
        response.push_str(&format!("**To:** `{:#x}`\n", to_addr));
        response.push_str(&format!(
            "**Amount:** {} {}\n",
            formatted_amount, metadata.symbol
        ));
        response.push_str(&format!("**Transaction Hash:** `{:#x}`\n", tx_hash));

        Ok(response)
    }

    /// Approve ERC-20 token spending
    #[cfg(feature = "evm")]
    pub async fn approve_erc20(
        &self,
        token_address: &str,
        spender: &str,
        amount: &str,
        wallet_address: Option<String>,
    ) -> McpResult<String> {
        // 1. Parse and validate inputs
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let token_addr = Address::from_str(token_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid token address: {}", e))
        })?;
        let spender_addr = Address::from_str(spender)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid spender: {}", e)))?;

        // 2. Get token metadata and parse amount
        let (evm_client, chain_id) = self.get_evm_client().await?;
        let metadata = self
            .ensure_token_metadata(&evm_client, chain_id, token_addr)
            .await?;
        let amount_u256 = parse_units(amount, metadata.decimals)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // 3. Encode approve call
        let erc20 = SdkErc20::new(evm_client.clone(), token_addr);
        let call_data = erc20.encode_approve(spender_addr, amount_u256);

        // 4. Use shared helper to build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                token_addr,                // contract address
                call_data,                 // encoded call
                U256::ZERO,                // no ETH value
                &cosmos_addr,              // wallet to use
                GAS_BUFFER_SIMPLE_PERCENT, // 20% buffer
            )
            .await?;

        // 5. Format response
        let formatted_amount = format_units(amount_u256, metadata.decimals);
        let mut response = "✅ **ERC-20 Approval Submitted**\n\n".to_string();
        response.push_str(&format!(
            "**Token:** {} ({})\n",
            metadata.symbol,
            metadata.name.as_deref().unwrap_or("Unknown")
        ));
        response.push_str(&format!("**Contract:** `{:#x}`\n", token_addr));
        response.push_str(&format!(
            "**Owner:** `{}` (Cosmos: `{}`)\n",
            evm_addr, cosmos_addr
        ));
        response.push_str(&format!("**Spender:** `{:#x}`\n", spender_addr));
        response.push_str(&format!(
            "**Amount:** {} {}\n",
            formatted_amount, metadata.symbol
        ));
        response.push_str(&format!("**Transaction Hash:** `{:#x}`\n", tx_hash));
        response.push_str(&format!(
            "\n**Explorer:** https://mantrascan.io/dukong/tx/{:#x}\n",
            tx_hash
        ));

        Ok(response)
    }

    // =============================================================================
    // PrimarySale Protocol Methods
    // =============================================================================

    /// Get comprehensive sale information
    #[cfg(feature = "evm")]
    pub async fn primary_sale_get_sale_info(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting primary sale info with args: {:?}",
            args
        );

        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Query all sale information
        let sale_info = primary_sale
            .get_sale_info()
            .await
            .map_err(McpServerError::Sdk)?;

        // Get additional info
        let mantra_usd = primary_sale
            .mantra_usd()
            .await
            .map_err(McpServerError::Sdk)?;
        let allowlist = primary_sale
            .allowlist()
            .await
            .map_err(McpServerError::Sdk)?;
        let multisig = primary_sale.multisig().await.map_err(McpServerError::Sdk)?;
        let issuer = primary_sale.issuer().await.map_err(McpServerError::Sdk)?;

        // Format status
        let status_str = match sale_info.status {
            0 => "Pending",
            1 => "Active",
            2 => "Ended",
            3 => "Failed",
            4 => "Settled",
            5 => "Cancelled",
            _ => "Unknown",
        };

        Ok(serde_json::json!({
            "contract_address": format!("{:#x}", contract_addr),
            "status": status_str,
            "status_code": sale_info.status,
            "is_active": sale_info.is_active,
            "start_time": sale_info.start,
            "end_time": sale_info.end,
            "remaining_time_seconds": sale_info.remaining_time,
            "soft_cap": sale_info.soft_cap.to_string(),
            "total_contributed": sale_info.total_contributed.to_string(),
            "investor_count": sale_info.investor_count.to_string(),
            "commission_bps": sale_info.commission_bps,
            "contracts": {
                "mantra_usd": format!("{:#x}", mantra_usd),
                "allowlist": format!("{:#x}", allowlist),
                "multisig": format!("{:#x}", multisig),
                "issuer": format!("{:#x}", issuer),
            }
        }))
    }

    /// Get investor-specific information
    #[cfg(feature = "evm")]
    pub async fn primary_sale_get_investor_info(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting investor info with args: {:?}", args);

        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // Get investor address (use active wallet if not provided)
        let investor_addr =
            if let Some(investor_str) = args.get("investor_address").and_then(|v| v.as_str()) {
                Address::from_str(investor_str).map_err(|e| {
                    McpServerError::InvalidArguments(format!("Invalid investor address: {}", e))
                })?
            } else {
                let (_, evm_addr) = self.get_wallet_evm_address(None).await?;
                Address::from_str(&evm_addr).map_err(|e| {
                    McpServerError::InvalidArguments(format!("Invalid EVM address: {}", e))
                })?
            };

        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Query investor-specific data
        let tokens_allocated = primary_sale
            .tokens_for(investor_addr)
            .await
            .map_err(McpServerError::Sdk)?;
        let contributed = primary_sale
            .contributed(investor_addr)
            .await
            .map_err(McpServerError::Sdk)?;
        let refunded = primary_sale
            .refunded(investor_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "investor_address": format!("{:#x}", investor_addr),
            "tokens_allocated": tokens_allocated.to_string(),
            "contributed": contributed.to_string(),
            "has_claimed_refund": refunded,
        }))
    }

    /// Invest in a primary sale
    #[cfg(feature = "evm")]
    pub async fn primary_sale_invest(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Investing in primary sale with args: {:?}",
            args
        );

        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let amount_str = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing amount".to_string()))?;

        let wallet_address = args
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 1. Parse and validate inputs
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Get EVM client and query sale contract
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Get mantraUSD token address and decimals
        let mantra_usd_addr = primary_sale
            .mantra_usd()
            .await
            .map_err(McpServerError::Sdk)?;

        let mantra_usd = evm_client.erc20(mantra_usd_addr);
        let decimals = mantra_usd.decimals().await.map_err(McpServerError::Sdk)?;

        // 3. Parse amount with proper decimals
        let amount_u256 = parse_units(amount_str, decimals)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // 4. Check allowance
        let allowance = mantra_usd
            .allowance(from_addr, contract_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if allowance < amount_u256 {
            return Err(McpServerError::InvalidArguments(
                format!("Insufficient mantraUSD allowance. Current: {}, Required: {}. Please approve the PrimarySale contract first.",
                    format_units(allowance, decimals),
                    amount_str)
            ));
        }

        // 5. Encode invest call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let invest_call = IPrimarySale::investCall {
            amount: amount_u256,
        };
        let call_data = alloy_sol_types::SolCall::abi_encode(&invest_call);

        // 6. Use shared helper to build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,              // contract address
                call_data,                  // encoded call
                U256::ZERO,                 // no ETH value
                &cosmos_addr,               // wallet to use
                GAS_BUFFER_COMPLEX_PERCENT, // 30% buffer for complex contract
            )
            .await?;

        // 7. Format response
        let formatted_amount = format_units(amount_u256, decimals);
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_invest",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "investor": format!("{:#x}", from_addr),
            "amount": formatted_amount,
            "amount_raw": amount_u256.to_string(),
            "mantra_usd_address": format!("{:#x}", mantra_usd_addr),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Claim refund from a failed sale
    #[cfg(feature = "evm")]
    pub async fn primary_sale_claim_refund(&self, args: Value) -> McpResult<Value> {
        use crate::protocols::evm::tx::{Eip1559Transaction, SignedEip1559Transaction};
        use alloy_primitives::Bytes;

        debug!("SDK Adapter: Claiming refund with args: {:?}", args);

        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let wallet_address = args
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get wallet
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;

        // Parse contract address
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // Get EVM client
        let (evm_client, chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Check if refund has already been claimed
        let already_refunded = primary_sale
            .refunded(from_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if already_refunded {
            return Err(McpServerError::InvalidArguments(
                "Refund has already been claimed for this address".to_string(),
            ));
        }

        // Get investor's contribution to show in response
        let contributed = primary_sale
            .contributed(from_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if contributed.is_zero() {
            return Err(McpServerError::InvalidArguments(
                "No contribution found for this address. Nothing to refund.".to_string(),
            ));
        }

        // Get mantraUSD for formatting
        let mantra_usd_addr = primary_sale
            .mantra_usd()
            .await
            .map_err(McpServerError::Sdk)?;
        let mantra_usd = evm_client.erc20(mantra_usd_addr);
        let decimals = mantra_usd.decimals().await.map_err(McpServerError::Sdk)?;

        // Encode claimRefund call data using the PrimarySale interface
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let claim_refund_call = IPrimarySale::claimRefundCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&claim_refund_call);

        // Get nonce
        let nonce = evm_client
            .get_pending_nonce(crate::protocols::evm::types::EthAddress(from_addr))
            .await
            .map_err(McpServerError::Sdk)?;

        // Get fee suggestion
        let fee_suggestion = evm_client
            .fee_suggestion()
            .await
            .map_err(McpServerError::Sdk)?;

        // Build transaction with high initial gas limit for estimation
        let mut tx = Eip1559Transaction::new(chain_id, nonce)
            .to(Some(contract_addr))
            .data(Bytes::from(call_data))
            .gas_limit(120_000) // Refund is simpler than invest
            .max_fee_per_gas(fee_suggestion.max_fee_per_gas.to::<u128>())
            .max_priority_fee_per_gas(fee_suggestion.max_priority_fee_per_gas.to::<u128>());

        // Estimate gas
        let gas_estimate = evm_client
            .estimate_gas_with_options(
                (&tx).into(),
                Some(crate::protocols::evm::types::EthAddress(from_addr)),
                None,
            )
            .await
            .map_err(McpServerError::Sdk)?;

        // Add 30% buffer to gas estimate
        let gas_limit = (gas_estimate * 130) / 100;
        tx = tx.gas_limit(gas_limit);

        // Sign transaction using MultiVM wallet with proper Ethereum derivation
        let multivm_wallet = self
            .get_multivm_wallet_by_address(&cosmos_addr)
            .await?
            .ok_or_else(|| McpServerError::Other("Wallet not found for signing".to_string()))?;

        let tx_hash = tx.signature_hash();
        let (sig, recid) = multivm_wallet
            .sign_ethereum_tx(tx_hash.as_ref())
            .map_err(McpServerError::Sdk)?;

        // Convert k256 signature to alloy Signature
        use alloy_primitives::Signature;

        // Convert k256 signature directly to alloy Signature
        #[allow(deprecated)]
        let alloy_sig = Signature::from((sig, recid));

        let raw_bytes = tx.encode_signed(&alloy_sig);
        let signed_tx = SignedEip1559Transaction::new(tx.into_signed(alloy_sig), raw_bytes);

        // Broadcast transaction
        let tx_hash = evm_client
            .send_raw_transaction(&signed_tx)
            .await
            .map_err(McpServerError::Sdk)?;

        // Format response
        let formatted_amount = format_units(contributed, decimals);
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_claim_refund",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "investor": format!("{:#x}", from_addr),
            "refund_amount": formatted_amount,
            "refund_amount_raw": contributed.to_string(),
            "mantra_usd_address": format!("{:#x}", mantra_usd_addr),
            "gas_limit": gas_limit,
            "max_fee_per_gas": fee_suggestion.max_fee_per_gas.to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Get all investors with pagination
    #[cfg(feature = "evm")]
    pub async fn primary_sale_get_all_investors(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting all investors with args: {:?}", args);

        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        let start = args.get("start").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        let investors = primary_sale
            .get_investors(start, limit)
            .await
            .map_err(McpServerError::Sdk)?;

        let investor_list: Vec<String> = investors
            .iter()
            .map(|addr| format!("{:#x}", addr))
            .collect();

        Ok(serde_json::json!({
            "investors": investor_list,
            "start": start,
            "count": investor_list.len(),
        }))
    }
}

/// Helper function to parse token amounts with proper decimals
#[cfg(feature = "evm")]
fn parse_units(amount: &str, decimals: u8) -> Result<alloy_primitives::U256, String> {
    let parts: Vec<&str> = amount.split('.').collect();

    if parts.len() > 2 {
        return Err("Invalid number format".to_string());
    }

    let whole = parts[0]
        .parse::<u128>()
        .map_err(|e| format!("Invalid whole number: {}", e))?;
    let multiplier = alloy_primitives::U256::from(10).pow(alloy_primitives::U256::from(decimals));

    let mut result = alloy_primitives::U256::from(whole) * multiplier;

    if parts.len() == 2 {
        let fractional = parts[1];
        if fractional.len() > decimals as usize {
            return Err(format!("Too many decimal places (max {})", decimals));
        }

        let fractional_value = fractional
            .parse::<u128>()
            .map_err(|e| format!("Invalid fractional part: {}", e))?;
        let fractional_multiplier = alloy_primitives::U256::from(10).pow(
            alloy_primitives::U256::from(decimals - fractional.len() as u8),
        );
        result += alloy_primitives::U256::from(fractional_value) * fractional_multiplier;
    }

    Ok(result)
}

/// Helper function to format token amounts with proper decimals
#[cfg(feature = "evm")]
fn format_units(value: alloy_primitives::U256, decimals: u8) -> String {
    let divisor = alloy_primitives::U256::from(10).pow(alloy_primitives::U256::from(decimals));
    let whole = value / divisor;
    let remainder = value % divisor;

    if remainder.is_zero() {
        whole.to_string()
    } else {
        let remainder_str = remainder.to_string();
        let padded_remainder = format!("{:0>width$}", remainder_str, width = decimals as usize);

        // Remove trailing zeros
        let trimmed = padded_remainder.trim_end_matches('0');
        if trimmed.is_empty() {
            whole.to_string()
        } else {
            format!("{}.{}", whole, trimmed)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Erc20TokenView {
    pub chain_id: u64,
    pub address: String,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub decimals: u8,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Erc20BalanceResponse {
    pub token: Erc20TokenView,
    pub wallet_address: String,
    pub raw_balance: String,
    pub formatted_balance: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvmBalancesResponse {
    pub cosmos_address: String,
    pub evm_address: String,
    pub native_balance: String,
    pub tokens: Vec<Erc20BalanceResponse>,
}

#[cfg(feature = "evm")]
fn token_source_label(source: &TokenSource) -> &'static str {
    match source {
        TokenSource::BuiltIn => "builtin",
        TokenSource::Custom => "custom",
        TokenSource::Discovered => "discovered",
    }
}

#[cfg(feature = "evm")]
fn token_view(info: &Erc20TokenInfo) -> Erc20TokenView {
    Erc20TokenView {
        chain_id: info.chain_id,
        address: info.checksummed_address(),
        symbol: info.symbol.clone(),
        name: info.name.clone(),
        decimals: info.decimals,
        source: token_source_label(&info.source).to_string(),
    }
}

#[cfg(test)]
#[cfg(feature = "evm")]
mod tests {
    use super::*;

    #[test]
    fn test_format_units() {
        // Test whole numbers
        let value = alloy_primitives::U256::from(1_000_000_000_000_000_000u128); // 1 ether
        assert_eq!(format_units(value, 18), "1");

        // Test decimals
        let value = alloy_primitives::U256::from(1_500_000_000_000_000_000u128); // 1.5 ether
        assert_eq!(format_units(value, 18), "1.5");

        // Test trailing zeros removal
        let value = alloy_primitives::U256::from(1_200_000_000_000_000_000u128); // 1.2 ether
        assert_eq!(format_units(value, 18), "1.2");

        // Test small decimals
        let value = alloy_primitives::U256::from(1_230_456_789_012_345_678u128); // 1.230456789012345678 ether
        assert_eq!(format_units(value, 18), "1.230456789012345678");

        // Test zero
        let value = alloy_primitives::U256::from(0u128);
        assert_eq!(format_units(value, 18), "0");

        // Test different decimals (6 for USDC)
        let value = alloy_primitives::U256::from(1_500_000u128); // 1.5 USDC
        assert_eq!(format_units(value, 6), "1.5");

        // Test very small amounts
        let value = alloy_primitives::U256::from(1u128); // 0.000000000000000001 ether
        assert_eq!(format_units(value, 18), "0.000000000000000001");
    }
}

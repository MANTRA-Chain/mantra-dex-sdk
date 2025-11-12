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
                let max_fee_u256 = alloy_primitives::U256::from_str(max_fee).map_err(|e| {
                    McpServerError::InvalidArguments(format!("Invalid max_fee_per_gas: {}", e))
                })?;
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
                                    "Invalid topic '{}': {}",
                                    topic_str, e
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
        let value_u256 = alloy_primitives::U256::from_str(value).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid value '{}': {}", value, e))
        })?;

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
        // 9. Create signed transaction
        #[allow(deprecated)]
        let signed_tx = {
            use alloy_primitives::Signature;

            // Convert k256 signature directly to alloy Signature
            let alloy_sig = Signature::from((sig, recid));
            let raw_bytes = tx.encode_signed(&alloy_sig);
            SignedEip1559Transaction::new(tx.into_signed(alloy_sig), raw_bytes)
        };

        // 10. Broadcast
        evm_client
            .send_raw_transaction(&signed_tx)
            .await
            .map_err(McpServerError::Sdk)
    }

    // =============================================================================
    // PrimarySale Access Control Validation Helpers
    // =============================================================================

    /// Validate that wallet has DEFAULT_ADMIN_ROLE for contract
    ///
    /// This helper prevents gas waste by checking role membership before
    /// submitting transactions that require admin privileges.
    #[cfg(feature = "evm")]
    async fn validate_admin_role(
        &self,
        contract_addr: Address,
        wallet_addr: Address,
    ) -> McpResult<()> {
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        let has_role = primary_sale
            .has_admin_role(wallet_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if !has_role {
            return Err(McpServerError::InvalidArguments(format!(
                "Wallet {} does not have DEFAULT_ADMIN_ROLE for contract {}. \
                    Only admins can perform this operation.",
                format!("{:#x}", wallet_addr),
                format!("{:#x}", contract_addr)
            )));
        }

        Ok(())
    }

    // =============================================================================
    // ERC-20 Token Operations
    // =============================================================================

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

        // Query all sale information (v2.0 - now includes name, hard_cap, accepted_tokens)
        let sale_info = primary_sale
            .get_sale_info()
            .await
            .map_err(McpServerError::Sdk)?;

        // Get additional contract addresses
        let allowlist = primary_sale
            .allowlist()
            .await
            .map_err(McpServerError::Sdk)?;
        let multisig = primary_sale.multisig().await.map_err(McpServerError::Sdk)?;
        let issuer = primary_sale.issuer().await.map_err(McpServerError::Sdk)?;
        let mantra = primary_sale.mantra().await.map_err(McpServerError::Sdk)?;

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

        // Format accepted tokens as array of addresses
        let accepted_tokens: Vec<String> = sale_info
            .accepted_tokens
            .iter()
            .map(|addr| format!("{:#x}", addr))
            .collect();

        let response = serde_json::json!({
            "status": "success",
            "operation": "primary_sale_get_sale_info",
            "contract_address": format!("{:#x}", contract_addr),
            "sale": {
                "name": sale_info.name,
                "status": status_str,
                "status_code": sale_info.status,
                "is_active": sale_info.is_active,
                "start_time": sale_info.start,
                "end_time": sale_info.end,
                "remaining_time_seconds": sale_info.remaining_time,
                "soft_cap": sale_info.soft_cap.to_string(),
                "hard_cap": sale_info.hard_cap.to_string(),
                "hard_cap_note": if sale_info.hard_cap.is_zero() { "unlimited" } else { "set" },
                "total_contributed_normalized": sale_info.total_contributed_normalized.to_string(),
                "remaining_capacity": sale_info.remaining_capacity.to_string(),
                "investor_count": sale_info.investor_count.to_string(),
                "commission_bps": sale_info.commission_bps,
                "accepted_tokens": accepted_tokens,
                "accepted_tokens_count": sale_info.accepted_tokens.len(),
            },
            "contracts": {
                "allowlist": format!("{:#x}", allowlist),
                "multisig": format!("{:#x}", multisig),
                "issuer": format!("{:#x}", issuer),
                "mantra": format!("{:#x}", mantra),
            },
            "notes": {
                "multi_token": "v2.0 supports multiple payment tokens (USDC, USDT, DAI, etc.)",
                "normalized_amounts": "All amounts normalized to 18 decimals for fair comparison",
                "hard_cap": "Hard cap = 0 means unlimited; otherwise maximum funding cap in normalized decimals"
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(response)
    }

    /// Get investor-specific information
    #[cfg(feature = "evm")]
    pub async fn primary_sale_get_investor_info(&self, args: Value) -> McpResult<Value> {
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

        // Query investor-specific data (v2.0 - now includes per-token breakdown)
        let investor_info = primary_sale
            .get_investor_info(investor_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        let refunded = primary_sale
            .refunded(investor_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        // Format per-token contributions
        let mut contributions_by_token = serde_json::Map::new();
        for (token_addr, amount) in &investor_info.contributions_by_token {
            contributions_by_token.insert(
                format!("{:#x}", token_addr),
                serde_json::json!(amount.to_string()),
            );
        }

        let response = serde_json::json!({
            "status": "success",
            "operation": "primary_sale_get_investor_info",
            "contract_address": format!("{:#x}", contract_addr),
            "investor": {
                "address": format!("{:#x}", investor_addr),
                "contribution_normalized": investor_info.contribution_normalized.to_string(),
                "contributions_by_token": contributions_by_token,
                "tokens_allocated": investor_info.tokens_allocated.to_string(),
                "is_kyc_approved": investor_info.is_kyc_approved,
                "has_received_settlement": investor_info.has_received_settlement,
                "has_claimed_refund": refunded,
            },
            "notes": {
                "normalized": "contribution_normalized is sum across all tokens in 18 decimals",
                "by_token": "contributions_by_token shows raw amounts in each token's native decimals"
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(response)
    }

    /// Invest in a primary sale (v2.0 - now requires token parameter)
    ///
    /// # Breaking Change (v2.0)
    /// Now requires `token` parameter to specify which accepted token to invest with.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_invest",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "investor": "0x...",
    ///   "token": "0x...",
    ///   "amount": "1000.5",
    ///   "amount_raw": "1000500000",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_invest(&self, args: Value) -> McpResult<Value> {
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let token_str = args
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing token parameter (v2.0 requirement). Use get_accepted_tokens to see available tokens.".to_string()))?;

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

        // Parse and validate token address
        let token_addr = Address::from_str(token_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid token address: {}", e))
        })?;

        // Validate token is accepted (v2.0 multi-token support)
        let is_accepted = primary_sale
            .is_accepted_token(token_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if !is_accepted {
            let accepted_tokens = primary_sale
                .get_accepted_tokens()
                .await
                .map_err(McpServerError::Sdk)?;

            let accepted_list: Vec<String> = accepted_tokens
                .iter()
                .map(|addr| format!("{:#x}", addr))
                .collect();

            return Err(McpServerError::InvalidArguments(format!(
                "Token {:#x} is not accepted by this sale.\n\
                \n\
                Accepted tokens:\n{}\n\
                \n\
                Use primary_sale_get_sale_info to see accepted tokens.",
                token_addr,
                accepted_list.join("\n")
            )));
        }

        // Get token contract and decimals
        let token = evm_client.erc20(token_addr);
        let decimals = token.decimals().await.map_err(McpServerError::Sdk)?;

        // 3. Parse amount with proper decimals
        let amount_u256 = parse_units(amount_str, decimals)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // 4. Check allowance (advisory check only - actual validation happens on-chain)
        let current_allowance = token
            .allowance(from_addr, contract_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        // Provide helpful guidance if allowance appears insufficient
        // Note: This is advisory only - actual validation happens on-chain
        if current_allowance < amount_u256 {
            // Calculate the shortfall
            let shortfall = amount_u256 - current_allowance;
            let shortfall_formatted = format_units(shortfall, decimals);

            return Err(McpServerError::InvalidArguments(
                format!(
                    "⚠️  Insufficient token allowance (advisory check).\n\
                    \n\
                    Token: {:#x}\n\
                    Current Allowance: {}\n\
                    Investment Amount: {}\n\
                    Shortfall: {}\n\
                    \n\
                    ℹ️  Note: Allowance is checked before transaction submission but validated on-chain. \
                    If you have pending approval transactions, they must be mined first.\n\
                    \n\
                    To approve the PrimarySale contract:\n\
                    1. Use wallet_approve_erc20 tool\n\
                    2. Token: {:#x}\n\
                    3. Spender: {:#x} (PrimarySale)\n\
                    4. Amount: {} or higher",
                    token_addr,
                    format_units(current_allowance, decimals),
                    amount_str,
                    shortfall_formatted,
                    token_addr,
                    contract_addr,
                    amount_str
                )
            ));
        }

        // Log successful allowance check for debugging
        debug!(
            "Allowance check passed (advisory): {} >= {} for token {:#x}",
            format_units(current_allowance, decimals),
            amount_str,
            token_addr
        );

        // 5. Encode invest call (v2.0 - now requires token parameter)
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let invest_call = IPrimarySale::investCall {
            token: token_addr,
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
            "token": format!("{:#x}", token_addr),
            "amount": formatted_amount,
            "amount_raw": amount_u256.to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Claim refund from a failed or cancelled sale (v2.0 - claims ALL tokens)
    ///
    /// # Behavior Change (v2.0)
    /// In v2.0, this operation automatically claims refunds for ALL accepted tokens
    /// in a single transaction. The contract loops through all tokens internally.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_claim_refund",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "investor": "0x...",
    ///   "refund_amount_normalized": "1000.5",
    ///   "refund_amount_raw": "1000500000000000000000",
    ///   "note": "Refund claimed for all accepted tokens in single transaction",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_claim_refund(&self, args: Value) -> McpResult<Value> {
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
        let (evm_client, _chain_id) = self.get_evm_client().await?;
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

        // Get investor's normalized contribution to show in response (v2.0)
        // In v2.0, all contributions are normalized to 18 decimals regardless of source token
        let contributed_normalized = primary_sale
            .contributed_normalized(from_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if contributed_normalized.is_zero() {
            return Err(McpServerError::InvalidArguments(
                "No contribution found for this address. Nothing to refund.".to_string(),
            ));
        }

        // Encode claimRefund call data using the PrimarySale interface
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let claim_refund_call = IPrimarySale::claimRefundCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&claim_refund_call);

        // Build, sign, and broadcast transaction
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // Format response (v2.0 - normalized amounts are always 18 decimals)
        let formatted_amount = format_units(contributed_normalized, 18);
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_claim_refund",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "investor": format!("{:#x}", from_addr),
            "refund_amount_normalized": formatted_amount,
            "refund_amount_raw": contributed_normalized.to_string(),
            "note": "v2.0: Refund claimed for all accepted tokens in single transaction",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Get all investors with pagination
    #[cfg(feature = "evm")]
    pub async fn primary_sale_get_all_investors(&self, args: Value) -> McpResult<Value> {
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

        // Validate pagination parameters
        if limit == 0 {
            return Err(McpServerError::InvalidArguments(
                "Limit must be greater than 0".to_string(),
            ));
        }
        if limit > 1000 {
            return Err(McpServerError::InvalidArguments(format!(
                "Limit too large: {}. Maximum allowed is 1000.",
                limit
            )));
        }

        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Get total count first for better error messages
        let total_count = primary_sale
            .investor_count()
            .await
            .map_err(McpServerError::Sdk)?;

        let total: usize = total_count.try_into().map_err(|_| {
            McpServerError::Other(format!(
                "Investor count overflow: {} exceeds maximum addressable size",
                total_count
            ))
        })?;

        // Retrieve investors with enhanced error context
        let investors = primary_sale
            .get_investors(start, limit)
            .await
            .map_err(|e| {
                McpServerError::Other(format!(
                    "Failed to retrieve investors (start: {}, limit: {}, total: {}): {}",
                    start, limit, total, e
                ))
            })?;

        let investor_list: Vec<String> = investors
            .iter()
            .map(|addr| format!("{:#x}", addr))
            .collect();

        // Calculate pagination metadata
        let returned_count = investor_list.len();
        let has_more = start + returned_count < total;
        let next_start = if has_more {
            Some(start + returned_count)
        } else {
            None
        };

        let response = serde_json::json!({
            "status": "success",
            "operation": "primary_sale_get_all_investors",
            "contract_address": format!("{:#x}", contract_addr),
            "pagination": {
                "start": start,
                "limit": limit,
                "returned": returned_count,
                "total": total,
                "has_more": has_more,
                "next_start": next_start,
            },
            "investors": investor_list,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(response)
    }

    /// Activate a primary sale (admin only)
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_activate",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_activate(&self, args: Value) -> McpResult<Value> {
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

        // 1. Parse and validate
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Validate admin role
        self.validate_admin_role(contract_addr, from_addr).await?;

        // 3. Validate current state
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);
        let current_status = primary_sale.status().await.map_err(McpServerError::Sdk)?;

        if current_status != 0 {
            let status_str = match current_status {
                1 => "Active",
                2 => "Ended",
                3 => "Failed",
                4 => "Settled",
                5 => "Cancelled",
                _ => "Unknown",
            };
            return Err(McpServerError::InvalidArguments(format!(
                "Cannot activate sale in {} status. Sale must be in Pending status.",
                status_str
            )));
        }

        // 4. Encode activate call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let activate_call = IPrimarySale::activateCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&activate_call);

        // 5. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 6. Return response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_activate",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// End a primary sale (callable after end time)
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_end_sale",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "total_contributed": "50000.0",
    ///   "soft_cap": "10000.0",
    ///   "soft_cap_met": true,
    ///   "new_status": "Ended",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_end_sale(&self, args: Value) -> McpResult<Value> {
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

        // 1. Parse addresses
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Get EVM client and validate state
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        let current_status = primary_sale.status().await.map_err(McpServerError::Sdk)?;
        if current_status != 1 {
            let status_str = match current_status {
                0 => "Pending",
                2 => "Ended",
                3 => "Failed",
                4 => "Settled",
                5 => "Cancelled",
                _ => "Unknown",
            };
            return Err(McpServerError::InvalidArguments(format!(
                "Cannot end sale in {} status. Sale must be in Active status.",
                status_str
            )));
        }

        let total_contributed = primary_sale
            .total_contributed_normalized()
            .await
            .map_err(McpServerError::Sdk)?;
        let soft_cap = primary_sale.soft_cap().await.map_err(McpServerError::Sdk)?;
        let soft_cap_met = total_contributed >= soft_cap;

        // 3. Encode endSale call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let end_sale_call = IPrimarySale::endSaleCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&end_sale_call);

        // 4. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 5. Return response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_end_sale",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "total_contributed": total_contributed.to_string(),
            "soft_cap": soft_cap.to_string(),
            "soft_cap_met": soft_cap_met,
            "new_status": if soft_cap_met { "Ended" } else { "Failed" },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Settle and distribute tokens (DEPRECATED - use 3-step settlement for v2.0)
    ///
    /// # ⚠️ DEPRECATED (v2.0)
    /// This method is deprecated in PrimarySale v2.0. Use the new 3-step settlement process:
    /// 1. `primary_sale_initialize_settlement` - Pull all asset tokens upfront
    /// 2. `primary_sale_settle_batch` - Process investors in batches (idempotent, can retry)
    /// 3. `primary_sale_finalize_settlement` - Complete settlement and pay commission/proceeds
    ///
    /// The old single-step settlement does not exist in v2.0 contracts.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_settle_and_distribute",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "asset_token": "0x...",
    ///   "asset_owner": "0x...",
    ///   "total_contributed": "50000.0",
    ///   "commission_amount": "500.0",
    ///   "issuer_amount": "49500.0",
    ///   "investors_processed": "25",
    ///   "max_loop": 500,
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_settle_and_distribute(&self, _args: Value) -> McpResult<Value> {
        // This method is deprecated in PrimarySale v2.0
        Err(McpServerError::InvalidArguments(
            "⚠️  primary_sale_settle_and_distribute is DEPRECATED in PrimarySale v2.0\n\
            \n\
            The single-step settlement method no longer exists in v2.0 contracts.\n\
            Please use the new 3-step batch settlement process:\n\
            \n\
            Step 1: primary_sale_initialize_settlement\n\
            - Pulls all asset tokens upfront from asset owner\n\
            - Initializes settlement state\n\
            \n\
            Step 2: primary_sale_settle_batch (call multiple times)\n\
            - Processes investors in batches (recommended: 100 per batch)\n\
            - Distributes tokens to KYC-approved investors\n\
            - Refunds contributions to non-approved or restricted wallets\n\
            - Idempotent: safe to retry if transaction fails\n\
            \n\
            Step 3: primary_sale_finalize_settlement\n\
            - Completes settlement after all investors processed\n\
            - Pays commission to MANTRA\n\
            - Pays proceeds to ISSUER\n\
            - Returns remaining tokens to ISSUER\n\
            \n\
            Use primary_sale_get_settlement_progress to monitor progress.\n\
            \n\
            See MIGRATION_GUIDE_V2.md for detailed migration instructions."
                .to_string(),
        ))
    }

    /// Top up refunds pool (anyone can call) - v2.0 requires token parameter
    ///
    /// # Breaking Change (v2.0)
    /// Now requires `token` parameter to specify which token to top up.
    /// Each accepted token has a separate refund pool.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_top_up_refunds",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "funder": "0x...",
    ///   "token": "0x...",
    ///   "amount": "1000.5",
    ///   "amount_raw": "1000500000",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_top_up_refunds(&self, args: Value) -> McpResult<Value> {
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let token_str = args
            .get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing token parameter (v2.0 requirement). Each accepted token has a separate refund pool.".to_string()))?;

        let amount_str = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing amount".to_string()))?;

        let wallet_address = args
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 1. Parse and validate
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Get EVM client and PrimarySale
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Parse and validate token address
        let token_addr = Address::from_str(token_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid token address: {}", e))
        })?;

        // Validate token is accepted (v2.0 multi-token support)
        let is_accepted = primary_sale
            .is_accepted_token(token_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        if !is_accepted {
            let accepted_tokens = primary_sale
                .get_accepted_tokens()
                .await
                .map_err(McpServerError::Sdk)?;

            let accepted_list: Vec<String> = accepted_tokens
                .iter()
                .map(|addr| format!("{:#x}", addr))
                .collect();

            return Err(McpServerError::InvalidArguments(format!(
                "Token {:#x} is not accepted by this sale.\n\
                \n\
                Accepted tokens:\n{}\n\
                \n\
                Use primary_sale_get_sale_info to see accepted tokens.",
                token_addr,
                accepted_list.join("\n")
            )));
        }

        // Get token contract and decimals
        let token = evm_client.erc20(token_addr);
        let decimals = token.decimals().await.map_err(McpServerError::Sdk)?;

        // 4. Parse amount
        let amount_u256 = parse_units(amount_str, decimals)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // 5. Check allowance (advisory check only - actual validation happens on-chain)
        let current_allowance = token
            .allowance(from_addr, contract_addr)
            .await
            .map_err(McpServerError::Sdk)?;

        // Provide helpful guidance if allowance appears insufficient
        // Note: This is advisory only - actual validation happens on-chain
        if current_allowance < amount_u256 {
            // Calculate the shortfall
            let shortfall = amount_u256 - current_allowance;
            let shortfall_formatted = format_units(shortfall, decimals);

            return Err(McpServerError::InvalidArguments(format!(
                "⚠️  Insufficient token allowance (advisory check).\n\
                \n\
                Token: {:#x}\n\
                Current Allowance: {}\n\
                Required Amount: {}\n\
                Shortfall: {}\n\
                \n\
                ℹ️  Note: Allowance is checked before transaction submission but validated on-chain. \
                If you have pending approval transactions, they must be mined first.\n\
                \n\
                To approve the PrimarySale contract:\n\
                1. Use wallet_approve_erc20 tool\n\
                2. Token: {:#x}\n\
                3. Spender: {:#x} (PrimarySale)\n\
                4. Amount: {} or higher",
                token_addr,
                format_units(current_allowance, decimals),
                amount_str,
                shortfall_formatted,
                token_addr,
                contract_addr,
                amount_str
            )));
        }

        // Log successful allowance check for debugging
        debug!(
            "Allowance check passed (advisory): {} >= {} for token {:#x} top-up",
            format_units(current_allowance, decimals),
            amount_str,
            token_addr
        );

        // 6. Encode topUpRefunds call (v2.0 - now requires token parameter)
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let topup_call = IPrimarySale::topUpRefundsCall {
            token: token_addr,
            amount: amount_u256,
        };
        let call_data = alloy_sol_types::SolCall::abi_encode(&topup_call);

        // 7. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 8. Format response
        let formatted_amount = format_units(amount_u256, decimals);
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_top_up_refunds",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "funder": format!("{:#x}", from_addr),
            "token": format!("{:#x}", token_addr),
            "amount": formatted_amount,
            "amount_raw": amount_u256.to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Initialize settlement - Step 1 of 3-step settlement process (v2.0)
    ///
    /// Pulls all asset tokens from asset owner upfront and initializes settlement state.
    /// This is the first step of the v2.0 batch settlement process.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_initialize_settlement",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "asset_token": "0x...",
    ///   "asset_owner": "0x...",
    ///   "total_investors": "150",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_initialize_settlement(&self, args: Value) -> McpResult<Value> {
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let asset_token = args
            .get("asset_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing asset_token".to_string()))?;

        let asset_owner = args
            .get("asset_owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing asset_owner".to_string()))?;

        let wallet_address = args
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse addresses
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;
        let asset_token_addr = Address::from_str(asset_token)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid asset_token: {}", e)))?;
        let asset_owner_addr = Address::from_str(asset_owner)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid asset_owner: {}", e)))?;

        // Get EVM client
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Get total investors for response
        let total_investors = primary_sale
            .investor_count()
            .await
            .map_err(McpServerError::Sdk)?;

        // Encode initializeSettlement call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let init_call = IPrimarySale::initializeSettlementCall {
            assetToken: asset_token_addr,
            assetOwner: asset_owner_addr,
        };
        let call_data = alloy_sol_types::SolCall::abi_encode(&init_call);

        // Build, sign, and broadcast (30% gas buffer for complex operation)
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_COMPLEX_PERCENT,
            )
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_initialize_settlement",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "asset_token": format!("{:#x}", asset_token_addr),
            "asset_owner": format!("{:#x}", asset_owner_addr),
            "total_investors": total_investors.to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Settle batch - Step 2 of 3-step settlement process (v2.0)
    ///
    /// Processes a batch of investors (distributes tokens or refunds). This operation is
    /// idempotent and can be called multiple times to process all investors in batches.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_settle_batch",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "batch_size": "100",
    ///   "restricted_wallets_count": "2",
    ///   "processed": "100",
    ///   "total": "150",
    ///   "progress_percentage": "66.67",
    ///   "is_complete": false,
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_settle_batch(&self, args: Value) -> McpResult<Value> {
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let batch_size = args
            .get("batch_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(100);

        let restricted_wallets: Vec<String> = args
            .get("restricted_wallets")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let wallet_address = args
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse addresses
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // Parse restricted wallet addresses
        let restricted_addrs: Vec<Address> = restricted_wallets
            .iter()
            .map(|addr_str| {
                Address::from_str(addr_str).map_err(|e| {
                    McpServerError::InvalidArguments(format!(
                        "Invalid restricted wallet address {}: {}",
                        addr_str, e
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Get EVM client and settlement progress
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        let (processed, total, _, is_complete) = primary_sale
            .get_settlement_progress()
            .await
            .map_err(McpServerError::Sdk)?;

        // Encode settleBatch call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let batch_call = IPrimarySale::settleBatchCall {
            batchSize: U256::from(batch_size),
            restrictedWallets: restricted_addrs.clone(),
        };
        let call_data = alloy_sol_types::SolCall::abi_encode(&batch_call);

        // Build, sign, and broadcast (30% gas buffer for complex operation)
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_COMPLEX_PERCENT,
            )
            .await?;

        // Calculate progress percentage
        let progress_percentage = if !total.is_zero() {
            (processed.saturating_mul(U256::from(10000)) / total).to::<u64>() as f64 / 100.0
        } else {
            0.0
        };

        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_settle_batch",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "batch_size": batch_size,
            "restricted_wallets_count": restricted_addrs.len(),
            "processed": processed.to_string(),
            "total": total.to_string(),
            "progress_percentage": format!("{:.2}", progress_percentage),
            "is_complete": is_complete,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Finalize settlement - Step 3 of 3-step settlement process (v2.0)
    ///
    /// Completes settlement after all investors have been processed. Pays commission to MANTRA,
    /// pays proceeds to ISSUER, and returns remaining tokens to ISSUER.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_finalize_settlement",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "is_complete": true,
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_finalize_settlement(&self, args: Value) -> McpResult<Value> {
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

        // Parse addresses
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // Get EVM client
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Check if can finalize
        let can_finalize = primary_sale
            .can_finalize_settlement()
            .await
            .map_err(McpServerError::Sdk)?;

        if !can_finalize {
            return Err(McpServerError::InvalidArguments(
                "Cannot finalize settlement yet. Ensure all investors have been processed first."
                    .to_string(),
            ));
        }

        // Encode finalizeSettlement call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let finalize_call = IPrimarySale::finalizeSettlementCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&finalize_call);

        // Build, sign, and broadcast (30% gas buffer for complex operation)
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_COMPLEX_PERCENT,
            )
            .await?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_finalize_settlement",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "is_complete": true,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Get settlement progress - Query settlement state (v2.0)
    ///
    /// Returns the current progress of the settlement process including processed/total
    /// investors and whether settlement is complete.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_get_settlement_progress",
    ///   "contract_address": "0x...",
    ///   "processed_investors": "75",
    ///   "total_investors": "150",
    ///   "is_initialized": true,
    ///   "is_complete": false,
    ///   "progress_percentage": "50.00",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_get_settlement_progress(&self, args: Value) -> McpResult<Value> {
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // Get EVM client
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        // Get settlement progress
        let (processed, total, is_initialized, is_complete) = primary_sale
            .get_settlement_progress()
            .await
            .map_err(McpServerError::Sdk)?;

        // Calculate progress percentage
        let progress_percentage = if !total.is_zero() {
            (processed.saturating_mul(U256::from(10000)) / total).to::<u64>() as f64 / 100.0
        } else {
            0.0
        };

        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_get_settlement_progress",
            "contract_address": format!("{:#x}", contract_addr),
            "processed_investors": processed.to_string(),
            "total_investors": total.to_string(),
            "is_initialized": is_initialized,
            "is_complete": is_complete,
            "progress_percentage": format!("{:.2}", progress_percentage),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Cancel a primary sale (admin only)
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_cancel",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "previous_status": "Active",
    ///   "new_status": "Cancelled",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_cancel(&self, args: Value) -> McpResult<Value> {
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

        // 1. Parse addresses
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Get current status and validate
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        let current_status = primary_sale.status().await.map_err(McpServerError::Sdk)?;

        let status_str = match current_status {
            0 => "Pending",
            1 => "Active",
            2 => "Ended",
            3 => "Failed",
            4 => "Settled",
            5 => "Cancelled",
            _ => "Unknown",
        };

        if current_status != 0 && current_status != 1 {
            return Err(McpServerError::InvalidArguments(format!(
                "Cannot cancel sale in {} status. Sale must be in Pending or Active status.",
                status_str
            )));
        }

        // 3. Encode cancel call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let cancel_call = IPrimarySale::cancelCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&cancel_call);

        // 4. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 5. Return response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_cancel",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "previous_status": status_str,
            "new_status": "Cancelled",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Pause primary sale contract (admin only)
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_pause",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_pause(&self, args: Value) -> McpResult<Value> {
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

        // 1. Parse and validate
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Encode pause call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let pause_call = IPrimarySale::pauseCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&pause_call);

        // 3. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 4. Return response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_pause",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Unpause primary sale contract (admin only)
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_unpause",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_unpause(&self, args: Value) -> McpResult<Value> {
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

        // 1. Parse and validate
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;

        // 2. Encode unpause call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let unpause_call = IPrimarySale::unpauseCall {};
        let call_data = alloy_sol_types::SolCall::abi_encode(&unpause_call);

        // 3. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 4. Return response
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_unpause",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Emergency withdraw ERC-20 tokens (admin only, only when Cancelled)
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "status": "success",
    ///   "operation": "primary_sale_emergency_withdraw",
    ///   "transaction_hash": "0x...",
    ///   "contract_address": "0x...",
    ///   "caller": "0x...",
    ///   "token_address": "0x...",
    ///   "recipient": "0x...",
    ///   "amount": "1000.5",
    ///   "amount_raw": "1000500000",
    ///   "timestamp": "2025-01-01T00:00:00Z"
    /// }
    /// ```
    #[cfg(feature = "evm")]
    pub async fn primary_sale_emergency_withdraw(&self, args: Value) -> McpResult<Value> {
        let contract_address = args
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing contract_address".to_string())
            })?;

        let token_address = args
            .get("token_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing token_address".to_string()))?;

        let recipient = args
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing recipient".to_string()))?;

        let amount_str = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing amount".to_string()))?;

        let wallet_address = args
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 1. Parse all addresses and amount
        let (cosmos_addr, evm_addr) = self.get_wallet_evm_address(wallet_address).await?;
        let from_addr = Address::from_str(&evm_addr).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid wallet address: {}", e))
        })?;
        let contract_addr = Address::from_str(contract_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid contract address: {}", e))
        })?;
        let token_addr = Address::from_str(token_address).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid token_address: {}", e))
        })?;
        let recipient_addr = Address::from_str(recipient)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid recipient: {}", e)))?;

        // 2. Get EVM client, validate state, and get token metadata
        let (evm_client, _chain_id) = self.get_evm_client().await?;
        let primary_sale = evm_client.primary_sale(contract_addr);

        let current_status = primary_sale.status().await.map_err(McpServerError::Sdk)?;
        if current_status != 5 {
            let status_str = match current_status {
                0 => "Pending",
                1 => "Active",
                2 => "Ended",
                3 => "Failed",
                4 => "Settled",
                _ => "Unknown",
            };
            return Err(McpServerError::InvalidArguments(format!(
                "Cannot emergency withdraw in {} status. Sale must be Cancelled.",
                status_str
            )));
        }

        let erc20 = evm_client.erc20(token_addr);
        let decimals = erc20.decimals().await.map_err(McpServerError::Sdk)?;

        // 3. Parse amount
        let amount_u256 = parse_units(amount_str, decimals)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // 4. Encode emergencyWithdrawERC20 call
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
        let withdraw_call = IPrimarySale::emergencyWithdrawERC20Call {
            token: token_addr,
            recipient: recipient_addr,
            amount: amount_u256,
        };
        let call_data = alloy_sol_types::SolCall::abi_encode(&withdraw_call);

        // 5. Build, sign, and broadcast
        let tx_hash = self
            .build_sign_and_broadcast_transaction(
                contract_addr,
                call_data,
                U256::ZERO,
                &cosmos_addr,
                GAS_BUFFER_SIMPLE_PERCENT,
            )
            .await?;

        // 6. Format response
        let formatted_amount = format_units(amount_u256, decimals);
        Ok(serde_json::json!({
            "status": "success",
            "operation": "primary_sale_emergency_withdraw",
            "transaction_hash": format!("{:#x}", tx_hash),
            "contract_address": format!("{:#x}", contract_addr),
            "caller": format!("{:#x}", from_addr),
            "token_address": format!("{:#x}", token_addr),
            "recipient": format!("{:#x}", recipient_addr),
            "amount": formatted_amount,
            "amount_raw": amount_u256.to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Analyze EVM transaction history and generate human-readable narrative
    ///
    /// Fetches multiple transactions, decodes their input data, and generates
    /// a sequential narrative describing what actions were performed.
    ///
    /// # Arguments
    /// * `args` - JSON object containing:
    ///   - `transaction_hashes`: Array of transaction hashes (0x...) [required]
    ///   - `include_failed`: Whether to include failed transactions (default: false)
    ///
    /// # Returns
    /// * JSON object with narrative text and transaction details
    pub async fn evm_analyze_transaction_history(&self, args: Value) -> McpResult<Value> {
        use crate::protocols::evm::narrative_generator::NarrativeGenerator;
        use crate::protocols::evm::transaction_decoder::TransactionDecoder;

        debug!(
            "SDK Adapter: Analyzing transaction history with args: {:?}",
            args
        );

        // Parse transaction hashes
        let tx_hashes_json = args
            .get("transaction_hashes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("transaction_hashes array is required".to_string())
            })?;

        if tx_hashes_json.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "transaction_hashes array cannot be empty".to_string(),
            ));
        }

        // Limit batch size to prevent RPC rate limiting (each tx requires 2 RPC calls: tx + receipt)
        // 20 transactions = ~40 base RPC calls + ~10 token decimal queries = ~50 total calls
        // This is within safe limits for public RPC endpoints (typically 10-20 req/sec)
        if tx_hashes_json.len() > 20 {
            return Err(McpServerError::InvalidArguments(
                "Maximum 20 transaction hashes allowed".to_string(),
            ));
        }

        // Parse include_failed parameter
        let include_failed = args
            .get("include_failed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Convert JSON strings to B256
        let mut tx_hashes = Vec::new();
        for hash_value in tx_hashes_json {
            let hash_str = hash_value.as_str().ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "All transaction hashes must be strings".to_string(),
                )
            })?;

            if !hash_str.starts_with("0x") || hash_str.len() != 66 {
                return Err(McpServerError::InvalidArguments(format!(
                    "Invalid transaction hash format: {}",
                    hash_str
                )));
            }

            let hash = alloy_primitives::B256::from_str(hash_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid transaction hash: {}", e))
            })?;

            tx_hashes.push(hash);
        }

        // Get EVM client
        let (evm_client, _chain_id) = self.get_evm_client().await?;

        // Get active wallet address for narrative context
        let active_wallet = match self.get_active_wallet().await {
            Ok(Some(wallet)) => wallet.ethereum_address().ok(),
            _ => None,
        };

        // Fetch transactions and receipts in parallel
        let transactions_results = evm_client.get_transactions_batch(&tx_hashes).await;
        let receipts_results = evm_client.get_transaction_receipts_batch(&tx_hashes).await;

        // Create decoder and narrative generator with EVM client for token metadata queries
        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new_with_client(active_wallet, evm_client.clone());

        // Process each transaction
        let mut narratives = Vec::new();
        let mut transaction_details = Vec::new();
        let mut errors = Vec::new();

        for (i, hash) in tx_hashes.iter().enumerate() {
            // Get transaction and receipt
            let tx_result = &transactions_results[i];
            let receipt_result = &receipts_results[i];

            match tx_result {
                Ok(Some(tx)) => {
                    // Determine transaction status with proper state tracking
                    let (success, status) = match receipt_result {
                        Ok(Some(receipt)) => {
                            // Receipt available - check if succeeded or failed
                            if receipt.status() {
                                (true, "success")
                            } else {
                                (false, "failed")
                            }
                        }
                        Ok(None) => {
                            // Transaction exists but no receipt - pending (not mined yet)
                            (false, "pending")
                        }
                        Err(_) => {
                            // Error fetching receipt - unknown state
                            (false, "unknown")
                        }
                    };

                    // Skip failed/pending/unknown transactions if not including them
                    if !success && !include_failed {
                        continue;
                    }

                    // Decode transaction input
                    let input = tx.input.as_ref();
                    let from = tx.from;
                    let to = tx.to;

                    let decoded = match decoder.decode(input, to) {
                        Ok(d) => d,
                        Err(_) => {
                            // Failed to decode, create unknown transaction narrative
                            let from_str = format!("{:?}", from);
                            let from_abbrev = if from_str.len() > 10 {
                                format!("{}...{}", &from_str[..6], &from_str[from_str.len() - 4..])
                            } else {
                                from_str
                            };

                            let hash_str = format!("{:?}", hash);
                            let hash_abbrev = if hash_str.len() > 10 {
                                format!("{}...{}", &hash_str[..6], &hash_str[hash_str.len() - 4..])
                            } else {
                                hash_str
                            };

                            let narrative = if let Some(to_addr) = to {
                                let to_str = format!("{:?}", to_addr);
                                let to_abbrev = if to_str.len() > 10 {
                                    format!("{}...{}", &to_str[..6], &to_str[to_str.len() - 4..])
                                } else {
                                    to_str
                                };
                                format!(
                                    "{} called contract at {} [tx: {}]",
                                    from_abbrev, to_abbrev, hash_abbrev
                                )
                            } else {
                                format!("{} deployed contract [tx: {}]", from_abbrev, hash_abbrev)
                            };
                            narratives.push(narrative);

                            transaction_details.push(serde_json::json!({
                                "hash": format!("{:?}", hash),
                                "from": format!("{:?}", from),
                                "to": to.map(|t| format!("{:?}", t)),
                                "success": success,
                                "status": status,
                                "decoded": false
                            }));
                            continue;
                        }
                    };

                    // Generate narrative (now async for token decimal queries)
                    let narrative = generator
                        .generate_narrative(&decoded, from, to, *hash, success)
                        .await;
                    narratives.push(narrative);

                    // Store transaction details
                    transaction_details.push(serde_json::json!({
                        "hash": format!("{:?}", hash),
                        "from": format!("{:?}", from),
                        "to": to.map(|t| format!("{:?}", t)),
                        "function": decoded.function_name,
                        "contract_type": format!("{:?}", decoded.contract_type),
                        "parameters": decoded.parameters,
                        "success": success,
                        "status": status,
                        "decoded": true
                    }));
                }
                Ok(None) => {
                    // Transaction not found
                    let error_msg = "Transaction not found";
                    errors.push(serde_json::json!({
                        "hash": format!("{:?}", hash),
                        "type": "not_found",
                        "message": error_msg
                    }));
                    narratives.push(format!("Transaction {} not found", hash));
                    transaction_details.push(serde_json::json!({
                        "hash": format!("{:?}", hash),
                        "error": error_msg,
                        "error_type": "not_found"
                    }));
                }
                Err(e) => {
                    // Error fetching transaction (network error, timeout, etc.)
                    let error_msg = format!("{}", e);
                    let error_type = if error_msg.contains("timeout")
                        || error_msg.contains("timed out")
                    {
                        "timeout"
                    } else if error_msg.contains("network") || error_msg.contains("connection") {
                        "network_error"
                    } else {
                        "rpc_error"
                    };

                    errors.push(serde_json::json!({
                        "hash": format!("{:?}", hash),
                        "type": error_type,
                        "message": error_msg
                    }));
                    narratives.push(format!("Error fetching transaction {}: {}", hash, e));
                    transaction_details.push(serde_json::json!({
                        "hash": format!("{:?}", hash),
                        "error": error_msg,
                        "error_type": error_type
                    }));
                }
            }
        }

        // Generate sequential narrative
        let mut full_narrative = generator.generate_sequential_narrative(narratives);

        // Add error summary to narrative if any errors occurred
        if !errors.is_empty() {
            let error_summary = format!(
                "\n\nNote: {} transaction(s) failed to process.",
                errors.len()
            );
            full_narrative.push_str(&error_summary);
        }

        // Build response with error tracking
        let mut response = serde_json::json!({
            "status": "success",
            "operation": "evm_analyze_transaction_history",
            "narrative": full_narrative,
            "transactions_analyzed": transaction_details.len() - errors.len(),
            "transactions_failed": errors.len(),
            "transactions": transaction_details,
            "include_failed": include_failed,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        // Add errors array if any errors occurred
        if !errors.is_empty() {
            response["errors"] = serde_json::json!(errors);
        }

        Ok(response)
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

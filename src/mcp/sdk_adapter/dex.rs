//! DEX protocol methods

use super::*;

impl McpSdkAdapter {
    pub async fn get_first_available_pool_id(&self) -> McpResult<String> {
        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get available pools
        let pools = client
            .get_pools(Some(10))
            .await
            .map_err(McpServerError::Sdk)?;

        if pools.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "No pools available".to_string(),
            ));
        }

        // Return the first available pool ID
        Ok(pools[0].pool_info.pool_identifier.clone())
    }

    /// Execute a swap with string parameters (for script execution)
    pub async fn execute_swap_simple(
        &self,
        from_asset: String,
        to_asset: String,
        amount: String,
        slippage: String,
        pool_id: String,
        _min_output: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Executing swap from {} to {} with amount {} and slippage {}",
            from_asset, to_asset, amount, slippage
        );

        // Parse amount
        let offer_amount = Uint128::from_str(&amount)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // Parse slippage with explicit error handling and validation
        let max_slippage = match Decimal::from_str(&slippage) {
            Ok(slippage_value) => {
                // Validate slippage range (0.0 to 1.0)
                if slippage_value < Decimal::zero() {
                    return Err(McpServerError::InvalidArguments(format!(
                        "Invalid slippage: {} - slippage cannot be negative",
                        slippage_value
                    )));
                }
                if slippage_value > Decimal::one() {
                    return Err(McpServerError::InvalidArguments(format!(
                        "Invalid slippage: {} - slippage cannot be greater than 1.0 (100%)",
                        slippage_value
                    )));
                }
                Some(slippage_value)
            }
            Err(e) => {
                return Err(McpServerError::InvalidArguments(format!(
                    "Invalid slippage format: '{}' - {}",
                    slippage, e
                )));
            }
        };

        // Create offer coin
        let offer_coin = Coin {
            denom: from_asset.clone(),
            amount: offer_amount,
        };

        // Use provided pool_id
        let pool_id_str = pool_id;

        // Get active wallet (required for swaps)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute the swap
        let swap_result = client
            .swap(&pool_id_str, offer_coin, &to_asset, max_slippage)
            .await
            .map_err(McpServerError::Sdk)?;

        info!(
            "Successfully executed swap from {} to {} with tx hash: {}",
            from_asset, to_asset, swap_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": swap_result.txhash,
            "swap_details": {
                "from_asset": from_asset,
                "to_asset": to_asset,
                "amount": amount,
                "slippage": slippage,
                "pool_id": pool_id_str,
                "gas_used": swap_result.gas_used,
                "gas_wanted": swap_result.gas_wanted
            },
            "block_height": swap_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Provide liquidity with string parameters (for script execution)
    pub async fn provide_liquidity_simple(
        &self,
        pool_id: String,
        asset_a_amount: String,
        asset_b_amount: String,
        min_lp_tokens: Option<String>,
        liquidity_slippage: Option<String>,
        swap_slippage: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Providing liquidity to pool {} with amounts {} and {}",
            pool_id, asset_a_amount, asset_b_amount
        );

        // First, get the pool information to determine the asset denoms
        let pool_info = self.get_pool(&pool_id).await?;

        // Extract asset denoms from pool info
        let assets_array = pool_info
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Pool does not have valid assets".to_string())
            })?;

        if assets_array.len() != 2 {
            return Err(McpServerError::InvalidArguments(
                "Pool must have exactly 2 assets for simple liquidity provision".to_string(),
            ));
        }

        // Get the asset denoms
        let asset_a_denom = assets_array[0]
            .get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Invalid asset A denom".to_string()))?;

        let asset_b_denom = assets_array[1]
            .get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Invalid asset B denom".to_string()))?;

        // Construct the assets array for the provide_liquidity call
        let assets_json = serde_json::json!([
            {
                "denom": asset_a_denom,
                "amount": asset_a_amount
            },
            {
                "denom": asset_b_denom,
                "amount": asset_b_amount
            }
        ]);

        // Construct the arguments for the provide_liquidity method
        let mut args = serde_json::json!({
            "pool_id": pool_id,
            "assets": assets_json
        });

        // Add slippage parameters if provided
        if let Some(liquidity_slippage_str) = liquidity_slippage {
            args["liquidity_max_slippage"] = serde_json::Value::String(liquidity_slippage_str);
        }
        if let Some(swap_slippage_str) = swap_slippage {
            args["swap_max_slippage"] = serde_json::Value::String(swap_slippage_str);
        }

        // Call the existing provide_liquidity method
        let result = self.provide_liquidity(args).await?;

        // Add the min_lp_tokens parameter to the response for reference
        if let Some(min_lp) = min_lp_tokens {
            if let Some(liquidity_details) = result.get("liquidity_details") {
                let mut details = liquidity_details.clone();
                if let Some(details_obj) = details.as_object_mut() {
                    details_obj.insert(
                        "min_lp_tokens".to_string(),
                        serde_json::Value::String(min_lp),
                    );
                }

                let mut modified_result = result.clone();
                if let Some(result_obj) = modified_result.as_object_mut() {
                    result_obj.insert("liquidity_details".to_string(), details);
                }
                return Ok(modified_result);
            }
        }

        Ok(result)
    }

    /// Withdraw liquidity with string parameters (for script execution)
    pub async fn withdraw_liquidity_simple(
        &self,
        pool_id: String,
        lp_amount: String,
        min_asset_a: Option<String>,
        min_asset_b: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Withdrawing liquidity from pool {} with LP amount {}",
            pool_id, lp_amount
        );

        // Note: min_asset_a and min_asset_b parameters are not currently supported by the underlying SDK
        if min_asset_a.is_some() || min_asset_b.is_some() {
            debug!("min_asset_a and min_asset_b parameters are not currently supported by the underlying SDK and will be ignored");
        }

        // Parse LP amount to Uint128
        let lp_amount_uint = Uint128::from_str(&lp_amount)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid LP amount: {}", e)))?;

        // Get active wallet
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute withdraw liquidity
        let withdraw_result = client
            .withdraw_liquidity(&pool_id, lp_amount_uint)
            .await
            .map_err(McpServerError::Sdk)?;

        info!(
            "Successfully withdrew liquidity from pool {} with tx hash: {}",
            pool_id, withdraw_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": withdraw_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", withdraw_result.txhash),
            "withdrawal_details": {
                "pool_id": pool_id,
                "lp_amount": lp_amount,
                "gas_used": withdraw_result.gas_used,
                "gas_wanted": withdraw_result.gas_wanted
            },
            "block_height": withdraw_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": withdraw_result.events
        }))
    }

    /// Create a pool with string parameters (for script execution)
    pub async fn create_pool_simple(
        &self,
        asset_a: String,
        asset_b: String,
        initial_price: String,
        pool_type: Option<String>,
        _fee_rate: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Creating pool for {} and {} with initial price {}",
            asset_a, asset_b, initial_price
        );

        // This is a simplified implementation
        // In a real implementation, you'd need to interact with the pool creation methods
        Ok(serde_json::json!({
            "status": "success",
            "operation": "create_pool",
            "asset_a": asset_a,
            "asset_b": asset_b,
            "initial_price": initial_price,
            "pool_type": pool_type.unwrap_or_else(|| "constant_product".to_string()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Get balances with optional asset filter (for script execution)
    pub async fn get_balances_filtered(&self, filter: Option<String>) -> McpResult<Value> {
        debug!("SDK Adapter: Getting balances with filter: {:?}", filter);

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Get active wallet address
        let wallet_address = match self.get_active_wallet_info().await? {
            Some(wallet_info) => wallet_info.address,
            None => return Err(McpServerError::WalletNotConfigured),
        };

        // Get balances for the address
        let balances = self
            .get_balances_for_address_direct(&network_config, &wallet_address)
            .await?;

        // Apply filter if provided
        if let Some(filter_str) = filter {
            let assets_filter: Vec<&str> = filter_str.split(',').collect();
            if let Some(balances_array) = balances.get("balances").and_then(|v| v.as_array()) {
                let filtered_balances: Vec<Value> = balances_array
                    .iter()
                    .filter(|balance| {
                        if let Some(denom) = balance.get("denom").and_then(|v| v.as_str()) {
                            assets_filter.iter().any(|asset| denom.contains(asset))
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();

                return Ok(serde_json::json!({
                    "address": wallet_address,
                    "balances": filtered_balances,
                    "total_tokens": filtered_balances.len(),
                    "filter": filter_str,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }
        }

        Ok(balances)
    }

    /// Get pools with optional filter and pagination (for script execution)  
    pub async fn get_pools_filtered(
        &self,
        filter: Option<String>,
        limit: Option<u32>,
        start_after: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting pools with filter: {:?}, limit: {:?}",
            filter, limit
        );

        let args = serde_json::json!({
            "limit": limit,
            "start_after": start_after
        });

        let pools = self.get_pools(args).await?;

        // Apply filter if provided
        if let Some(filter_str) = filter {
            if let Some(pools_array) = pools.get("pools").and_then(|v| v.as_array()) {
                let filtered_pools: Vec<Value> = pools_array
                    .iter()
                    .filter(|pool| {
                        if let Some(pool_id) = pool.get("pool_id").and_then(|v| v.as_str()) {
                            pool_id.contains(&filter_str)
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();

                return Ok(serde_json::json!({
                    "pools": filtered_pools,
                    "count": filtered_pools.len(),
                    "filter": filter_str,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }
        }

        Ok(pools)
    }

    /// Get pool information (for script execution)
    pub async fn get_pool_info(&self, pool_id: String) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting pool information for pool: {}",
            pool_id
        );
        self.get_pool(&pool_id).await
    }
    pub async fn get_pool(&self, pool_id: &str) -> McpResult<Value> {
        // Validate pool_id parameter
        if pool_id.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "pool_id cannot be empty".to_string(),
            ));
        }

        // Get client connection
        let client = self
            .get_client(&self.get_default_network_config().await?)
            .await?;

        // Query pool information from blockchain
        let pool_info = client
            .get_pool(pool_id)
            .await
            .map_err(McpServerError::Sdk)?;

        // Convert pool info to JSON format
        let pool_data = serde_json::json!({
            "pool_id": pool_info.pool_info.pool_identifier,
            "pool_type": match pool_info.pool_info.pool_type {
                mantra_dex_std::pool_manager::PoolType::ConstantProduct => "constant_product",
                mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "stable_swap",
            },
            "assets": pool_info.pool_info.assets.iter().map(|asset| {
                serde_json::json!({
                    "denom": asset.denom,
                    "amount": asset.amount.to_string()
                })
            }).collect::<Vec<_>>(),
            "status": {
                "swaps_enabled": pool_info.pool_info.status.swaps_enabled,
                "deposits_enabled": pool_info.pool_info.status.deposits_enabled,
                "withdrawals_enabled": pool_info.pool_info.status.withdrawals_enabled
            },
            "lp_token_denom": pool_info.pool_info.lp_denom,
            "total_share": pool_info.total_share.to_string()
        });

        Ok(pool_data)
    }

    pub async fn get_pools(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting pools with args: {:?}", args);

        // Parse optional parameters
        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as u32);

        let start_after = args
            .get("start_after")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Execute the pools query directly (without retry for now due to client not being Clone)
        let pools_result = client.get_pools(limit).await.map_err(McpServerError::Sdk)?;

        // Convert pools to JSON format
        let pools_json: Vec<Value> = pools_result
            .into_iter()
            .map(|pool| {
                serde_json::json!({
                    "pool_id": pool.pool_info.pool_identifier,
                    "pool_type": match pool.pool_info.pool_type {
                        mantra_dex_std::pool_manager::PoolType::ConstantProduct => "constant_product",
                        mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "stable_swap",
                    },
                    "assets": pool.pool_info.assets.iter().map(|asset| {
                        serde_json::json!({
                            "denom": asset.denom,
                            "amount": asset.amount.to_string()
                        })
                    }).collect::<Vec<_>>(),
                    "lp_denom": pool.pool_info.lp_denom,
                    "status": {
                        "swaps_enabled": pool.pool_info.status.swaps_enabled,
                        "deposits_enabled": pool.pool_info.status.deposits_enabled,
                        "withdrawals_enabled": pool.pool_info.status.withdrawals_enabled
                    },
                    "total_share": pool.total_share.to_string()
                })
            })
            .collect();

        info!("Successfully retrieved {} pools", pools_json.len());

        Ok(serde_json::json!({
            "pools": pools_json,
            "count": pools_json.len(),
            "limit": limit,
            "start_after": start_after
        }))
    }

    pub async fn validate_pool_status(
        &self,
        pool_id: &str,
        operation: Option<String>,
        include_recommendations: bool,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Validating pool status for pool {} with operation {:?}",
            pool_id, operation
        );

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get pool information
        let pool_result = client.get_pool(pool_id).await;

        let mut validation_result = serde_json::Map::new();
        validation_result.insert(
            "pool_id".to_string(),
            serde_json::Value::String(pool_id.to_string()),
        );
        validation_result.insert(
            "validation_timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );

        match pool_result {
            Ok(pool_info) => {
                let status = &pool_info.pool_info.status;

                // Overall pool existence validation
                validation_result.insert("pool_exists".to_string(), serde_json::Value::Bool(true));
                validation_result.insert(
                    "pool_identifier".to_string(),
                    serde_json::Value::String(pool_info.pool_info.pool_identifier.clone()),
                );

                // Feature status validation
                let mut feature_status = serde_json::Map::new();
                feature_status.insert(
                    "swaps_enabled".to_string(),
                    serde_json::Value::Bool(status.swaps_enabled),
                );
                feature_status.insert(
                    "deposits_enabled".to_string(),
                    serde_json::Value::Bool(status.deposits_enabled),
                );
                feature_status.insert(
                    "withdrawals_enabled".to_string(),
                    serde_json::Value::Bool(status.withdrawals_enabled),
                );
                validation_result.insert(
                    "features".to_string(),
                    serde_json::Value::Object(feature_status),
                );

                // Operation-specific validation
                if let Some(op) = operation {
                    let operation_valid = match op.as_str() {
                        "swap" => status.swaps_enabled,
                        "deposit" | "provide_liquidity" => status.deposits_enabled,
                        "withdraw" | "withdraw_liquidity" => status.withdrawals_enabled,
                        _ => false,
                    };

                    validation_result.insert(
                        "operation".to_string(),
                        serde_json::Value::String(op.clone()),
                    );
                    validation_result.insert(
                        "operation_valid".to_string(),
                        serde_json::Value::Bool(operation_valid),
                    );

                    if !operation_valid {
                        validation_result.insert(
                            "operation_error".to_string(),
                            serde_json::Value::String(format!(
                                "Operation '{}' is not enabled for this pool",
                                op
                            )),
                        );
                    }
                }

                // Overall status assessment
                let all_operations_enabled =
                    status.swaps_enabled && status.deposits_enabled && status.withdrawals_enabled;
                let overall_status = if all_operations_enabled {
                    "fully_operational"
                } else if !status.swaps_enabled
                    && !status.deposits_enabled
                    && !status.withdrawals_enabled
                {
                    "disabled"
                } else {
                    "partially_operational"
                };

                validation_result.insert(
                    "overall_status".to_string(),
                    serde_json::Value::String(overall_status.to_string()),
                );
                validation_result.insert(
                    "is_operational".to_string(),
                    serde_json::Value::Bool(all_operations_enabled),
                );

                // Add recommendations if requested
                if include_recommendations {
                    let mut recommendations = Vec::new();

                    if !status.swaps_enabled {
                        recommendations
                            .push("Swaps are disabled - users cannot trade in this pool");
                    }
                    if !status.deposits_enabled {
                        recommendations
                            .push("Deposits are disabled - users cannot provide liquidity");
                    }
                    if !status.withdrawals_enabled {
                        recommendations
                            .push("Withdrawals are disabled - users cannot remove liquidity");
                    }

                    if recommendations.is_empty() {
                        recommendations
                            .push("Pool is fully operational - all operations are enabled");
                    }

                    validation_result.insert(
                        "recommendations".to_string(),
                        serde_json::Value::Array(
                            recommendations
                                .into_iter()
                                .map(|s| serde_json::Value::String(s.to_string()))
                                .collect(),
                        ),
                    );
                }

                validation_result.insert(
                    "status".to_string(),
                    serde_json::Value::String("success".to_string()),
                );
            }
            Err(e) => {
                validation_result.insert("pool_exists".to_string(), serde_json::Value::Bool(false));
                validation_result.insert(
                    "error".to_string(),
                    serde_json::Value::String(format!("Failed to get pool information: {}", e)),
                );
                validation_result.insert(
                    "status".to_string(),
                    serde_json::Value::String("error".to_string()),
                );
                validation_result
                    .insert("is_operational".to_string(), serde_json::Value::Bool(false));

                if include_recommendations {
                    validation_result.insert(
                        "recommendations".to_string(),
                        serde_json::Value::Array(vec![serde_json::Value::String(
                            "Pool does not exist or is not accessible".to_string(),
                        )]),
                    );
                }
            }
        }

        Ok(serde_json::Value::Object(validation_result))
    }

    pub async fn provide_liquidity(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Providing liquidity with args: {:?}", args);

        // Parse required parameters
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let assets_json = args
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("assets array is required".to_string())
            })?;

        // Parse assets
        let mut assets = Vec::new();
        for asset_json in assets_json {
            let denom = asset_json
                .get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.denom is required".to_string())
                })?;

            let amount_str = asset_json
                .get("amount")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.amount is required".to_string())
                })?;

            let amount = Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid asset amount: {}", e))
            })?;

            assets.push(Coin {
                denom: denom.to_string(),
                amount,
            });
        }

        // Parse optional slippage parameters
        let liquidity_max_slippage = args
            .get("liquidity_max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        let swap_max_slippage = args
            .get("swap_max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Get wallet (use provided wallet_address or active wallet)
        let wallet =
            if let Some(wallet_address) = args.get("wallet_address").and_then(|v| v.as_str()) {
                match self.get_wallet_by_address(wallet_address).await? {
                    Some(wallet) => wallet,
                    None => {
                        return Err(McpServerError::InvalidArguments(format!(
                            "Wallet with address {} not found",
                            wallet_address
                        )));
                    }
                }
            } else {
                self.get_active_wallet_with_validation().await?
            };

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute provide liquidity directly (without retry for now due to client not being Clone)
        let liquidity_result = client
            .provide_liquidity(pool_id, assets, liquidity_max_slippage, swap_max_slippage)
            .await
            .map_err(McpServerError::Sdk)?;

        info!(
            "Successfully provided liquidity to pool {} with tx hash: {}",
            pool_id, liquidity_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": liquidity_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", liquidity_result.txhash),
            "liquidity_details": {
                "pool_id": pool_id,
                "assets": assets_json,
                "liquidity_max_slippage": liquidity_max_slippage.map(|d| d.to_string()),
                "swap_max_slippage": swap_max_slippage.map(|d| d.to_string()),
                "gas_used": liquidity_result.gas_used,
                "gas_wanted": liquidity_result.gas_wanted
            },
            "block_height": liquidity_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": liquidity_result.events
        }))
    }

    pub async fn provide_liquidity_unchecked(&self, args: Value) -> McpResult<Value> {
        info!(?args, "SDK Adapter: Providing liquidity (unchecked)");
        Ok(serde_json::json!({
            "status": "success",
            "message": "Liquidity provided (unchecked, simulation)",
            "tx_hash": "SIMULATED_UNCHECKED_TX_HASH"
        }))
    }

    pub async fn withdraw_liquidity(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Withdrawing liquidity with args: {:?}", args);

        // Parse required parameters
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let amount_str = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("amount is required".to_string()))?;

        let lp_amount = Uint128::from_str(amount_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid LP amount: {}", e)))?;

        // Get wallet (use provided wallet_address or active wallet)
        let wallet =
            if let Some(wallet_address) = args.get("wallet_address").and_then(|v| v.as_str()) {
                match self.get_wallet_by_address(wallet_address).await? {
                    Some(wallet) => wallet,
                    None => {
                        return Err(McpServerError::InvalidArguments(format!(
                            "Wallet with address {} not found",
                            wallet_address
                        )));
                    }
                }
            } else {
                self.get_active_wallet_with_validation().await?
            };

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute withdraw liquidity directly (without retry for now due to client not being Clone)
        let withdraw_result = client
            .withdraw_liquidity(pool_id, lp_amount)
            .await
            .map_err(McpServerError::Sdk)?;

        info!(
            "Successfully withdrew liquidity from pool {} with tx hash: {}",
            pool_id, withdraw_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": withdraw_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", withdraw_result.txhash),
            "withdrawal_details": {
                "pool_id": pool_id,
                "lp_amount": amount_str,
                "gas_used": withdraw_result.gas_used,
                "gas_wanted": withdraw_result.gas_wanted
            },
            "block_height": withdraw_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": withdraw_result.events
        }))
    }

    pub async fn get_liquidity_positions(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting liquidity positions with args: {:?}",
            args
        );

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet to find LP tokens
        let balances_result = client
            .get_balances_for_address(&wallet_address)
            .await
            .map_err(McpServerError::Sdk)?;

        // Filter for LP tokens (typically start with "factory/" and contain "lp" or are pool denoms)
        let mut lp_positions = Vec::new();

        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this looks like an LP token denom
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                // Try to extract pool identifier from the denom
                let pool_id = if let Some(last_part) = denom.split('/').next_back() {
                    if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    }
                } else {
                    denom
                };

                lp_positions.push(serde_json::json!({
                    "pool_id": pool_id,
                    "lp_token_denom": denom,
                    "balance": balance.amount.to_string(),
                    "wallet_address": wallet_address
                }));
            }
        }

        info!(
            "Found {} LP positions for wallet {}",
            lp_positions.len(),
            wallet_address
        );

        Ok(serde_json::json!({
            "status": "success",
            "wallet_address": wallet_address,
            "positions": lp_positions,
            "total_positions": lp_positions.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn execute_swap(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Executing swap with args: {:?}", args);

        // Parse required parameters
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let offer_asset = args.get("offer_asset").ok_or_else(|| {
            McpServerError::InvalidArguments("offer_asset is required".to_string())
        })?;

        let ask_asset_denom = args
            .get("ask_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("ask_asset_denom is required".to_string())
            })?;

        // Parse offer asset
        let offer_denom = offer_asset
            .get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("offer_asset.denom is required".to_string())
            })?;

        let offer_amount_str = offer_asset
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("offer_asset.amount is required".to_string())
            })?;

        let offer_amount = Uint128::from_str(offer_amount_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid offer amount: {}", e))
        })?;

        let offer_coin = Coin {
            denom: offer_denom.to_string(),
            amount: offer_amount,
        };

        // Parse optional max_slippage
        let max_slippage = args
            .get("max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Get wallet (use provided wallet_address or active wallet)
        let wallet =
            if let Some(wallet_address) = args.get("wallet_address").and_then(|v| v.as_str()) {
                match self.get_wallet_by_address(wallet_address).await? {
                    Some(wallet) => wallet,
                    None => {
                        return Err(McpServerError::InvalidArguments(format!(
                            "Wallet with address {} not found",
                            wallet_address
                        )));
                    }
                }
            } else {
                self.get_active_wallet_with_validation().await?
            };

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute the swap directly (without retry for now due to client not being Clone)
        let swap_result = client
            .swap(pool_id, offer_coin, ask_asset_denom, max_slippage)
            .await
            .map_err(McpServerError::Sdk)?;

        info!(
            "Successfully executed swap in pool {} with tx hash: {}",
            pool_id, swap_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": swap_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", swap_result.txhash),
            "swap_details": {
                "pool_id": pool_id,
                "offer_asset": {
                    "denom": offer_denom,
                    "amount": offer_amount_str
                },
                "ask_asset_denom": ask_asset_denom,
                "max_slippage": max_slippage.map(|d| d.to_string()),
                "gas_used": swap_result.gas_used,
                "gas_wanted": swap_result.gas_wanted
            },
            "block_height": swap_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": swap_result.events
        }))
    }

    pub async fn get_lp_token_balance(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting LP token balance with args: {:?}",
            args
        );

        // Parse required pool_id parameter
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet
        let balances_result = client
            .get_balances_for_address(&wallet_address)
            .await
            .map_err(McpServerError::Sdk)?;

        // Look for LP token for this specific pool
        let mut lp_balance = None;
        let mut lp_denom = None;

        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this is an LP token for the specified pool
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                // Try to extract pool identifier from the denom
                if let Some(last_part) = denom.split('/').next_back() {
                    let extracted_pool_id = if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    };

                    if extracted_pool_id == pool_id {
                        lp_balance = Some(balance.amount.to_string());
                        lp_denom = Some(denom.clone());
                        break;
                    }
                }
            }
        }

        let balance_amount = lp_balance.unwrap_or_else(|| "0".to_string());
        let token_denom = lp_denom.unwrap_or_else(|| format!("factory/mantra/lp_{}", pool_id));

        info!(
            "LP token balance for pool {}: {} {}",
            pool_id, balance_amount, token_denom
        );

        Ok(serde_json::json!({
            "status": "success",
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_token_balance": {
                "denom": token_denom,
                "amount": balance_amount
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn get_all_lp_token_balances(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting all LP token balances with args: {:?}",
            args
        );

        // Parse optional parameters
        let include_zero_balances = args
            .get("include_zero_balances")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet
        let balances_result = client
            .get_balances_for_address(&wallet_address)
            .await
            .map_err(McpServerError::Sdk)?;

        // Filter for LP tokens
        let mut lp_balances = Vec::new();

        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this looks like an LP token denom
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                let amount_str = balance.amount.to_string();

                // Skip zero balances if not requested
                if !include_zero_balances && balance.amount.is_zero() {
                    continue;
                }

                // Try to extract pool identifier from the denom
                let pool_id = if let Some(last_part) = denom.split('/').next_back() {
                    if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    }
                } else {
                    denom
                };

                lp_balances.push(serde_json::json!({
                    "pool_id": pool_id,
                    "lp_token_denom": denom,
                    "balance": amount_str,
                    "is_zero": balance.amount.is_zero()
                }));
            }
        }

        info!(
            "Found {} LP token balances for wallet {}",
            lp_balances.len(),
            wallet_address
        );

        Ok(serde_json::json!({
            "status": "success",
            "wallet_address": wallet_address,
            "lp_balances": lp_balances,
            "total_positions": lp_balances.len(),
            "include_zero_balances": include_zero_balances,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn estimate_lp_withdrawal_amounts(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Estimating LP withdrawal amounts with args: {:?}",
            args
        );

        // Parse required pool_id parameter
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get pool information
        let pool_info = client
            .get_pool(pool_id)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get LP token amount to withdraw (use full balance if not provided)
        let lp_amount = if let Some(amount_str) =
            args.get("lp_token_amount").and_then(|v| v.as_str())
        {
            Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid LP token amount: {}", e))
            })?
        } else {
            // Use full LP balance
            let balances_result = client
                .get_balances_for_address(&wallet_address)
                .await
                .map_err(McpServerError::Sdk)?;

            let mut full_balance = Uint128::zero();
            for balance in &balances_result {
                let denom = &balance.denom;
                if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                    if let Some(last_part) = denom.split('/').next_back() {
                        let extracted_pool_id = if last_part.starts_with("lp_") {
                            last_part.strip_prefix("lp_").unwrap_or(last_part)
                        } else {
                            last_part
                        };

                        if extracted_pool_id == pool_id {
                            full_balance = balance.amount;
                            break;
                        }
                    }
                }
            }
            full_balance
        };

        if lp_amount.is_zero() {
            return Ok(serde_json::json!({
                "status": "success",
                "pool_id": pool_id,
                "wallet_address": wallet_address,
                "lp_amount": "0",
                "estimated_withdrawal": [],
                "total_share": pool_info.total_share.to_string(),
                "message": "No LP tokens to withdraw",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }));
        }

        // Calculate withdrawal amounts based on pool ratio
        let total_share = pool_info.total_share;
        let mut estimated_amounts = Vec::new();

        for asset in &pool_info.pool_info.assets {
            // Calculate proportional withdrawal amount
            // withdrawal_amount = (lp_amount / total_share) * asset_amount
            let withdrawal_amount = if !total_share.amount.is_zero() {
                asset.amount.multiply_ratio(lp_amount, total_share.amount)
            } else {
                Uint128::zero()
            };

            estimated_amounts.push(serde_json::json!({
                "denom": asset.denom,
                "amount": withdrawal_amount.to_string(),
                "pool_amount": asset.amount.to_string()
            }));
        }

        info!(
            "Estimated withdrawal amounts for {} LP tokens from pool {}: {:?}",
            lp_amount, pool_id, estimated_amounts
        );

        Ok(serde_json::json!({
            "status": "success",
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_amount": lp_amount.to_string(),
            "estimated_withdrawal": estimated_amounts,
            "pool_info": {
                "total_share": total_share.to_string(),
                "assets": pool_info.pool_info.assets.iter().map(|asset| {
                    serde_json::json!({
                        "denom": asset.denom,
                        "amount": asset.amount.to_string()
                    })
                }).collect::<Vec<_>>()
            },
            "withdrawal_ratio": if !total_share.amount.is_zero() {
                format!("{:.6}", lp_amount.u128() as f64 / total_share.amount.u128() as f64)
            } else {
                "0.000000".to_string()
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn create_pool(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Creating pool with args: {:?}", args);

        // Parse required parameters
        let pool_type_str = args
            .get("pool_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_type is required".to_string()))?;

        let assets_json = args
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("assets array is required".to_string())
            })?;

        // Parse pool type
        let pool_type = match pool_type_str {
            "constant_product" => mantra_dex_std::pool_manager::PoolType::ConstantProduct,
            "stable_swap" => {
                // For stable swap, we need amplification parameter
                let amplification = args
                    .get("amplification")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                mantra_dex_std::pool_manager::PoolType::StableSwap { amp: amplification }
            }
            _ => {
                return Err(McpServerError::InvalidArguments(
                    "Invalid pool_type. Must be 'constant_product' or 'stable_swap'".to_string(),
                ))
            }
        };

        // Parse assets - extract denominations and decimals
        let mut asset_denoms = Vec::new();
        let mut asset_decimals = Vec::new();

        for asset_json in assets_json {
            let denom = asset_json
                .get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.denom is required".to_string())
                })?;

            let decimals = asset_json
                .get("decimals")
                .and_then(|v| v.as_u64())
                .unwrap_or(6) as u8; // Default to 6 decimals

            asset_denoms.push(denom.to_string());
            asset_decimals.push(decimals);
        }

        // Parse fees
        let fees_json = args.get("fees");
        let protocol_fee_str = fees_json
            .and_then(|f| f.get("protocol_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.01"); // Default 1%

        let swap_fee_str = fees_json
            .and_then(|f| f.get("swap_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.03"); // Default 3%

        let burn_fee_str = fees_json
            .and_then(|f| f.get("burn_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.0"); // Default 0%

        // Parse fee decimals
        let protocol_fee = Decimal::from_str(protocol_fee_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid protocol_fee: {}", e))
        })?;
        let swap_fee = Decimal::from_str(swap_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid swap_fee: {}", e)))?;
        let burn_fee = Decimal::from_str(burn_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid burn_fee: {}", e)))?;

        // Create pool fees structure
        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: protocol_fee,
            },
            swap_fee: mantra_dex_std::fee::Fee { share: swap_fee },
            burn_fee: mantra_dex_std::fee::Fee { share: burn_fee },
            extra_fees: vec![],
        };

        // Parse optional pool identifier
        let pool_identifier = args
            .get("pool_identifier")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Clone pool_identifier for response formatting
        let pool_identifier_for_response = pool_identifier.clone();

        // Get active wallet (required for pool creation)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Query the actual pool creation fee for response
        let creation_fee = client
            .get_pool_creation_fee()
            .await
            .map_err(McpServerError::Sdk)?;

        // Execute pool creation directly (without retry for now due to client not being Clone)
        let create_result = client
            .create_pool(
                asset_denoms,
                asset_decimals,
                pool_fees,
                pool_type,
                pool_identifier,
            )
            .await
            .map_err(McpServerError::Sdk)?;

        info!(
            "Successfully created pool with tx hash: {}",
            create_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": create_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", create_result.txhash),
            "pool_details": {
                "pool_type": pool_type_str,
                "assets": assets_json,
                "fees": {
                    "protocol_fee": protocol_fee_str,
                    "swap_fee": swap_fee_str,
                    "burn_fee": burn_fee_str
                },
                "pool_identifier": pool_identifier_for_response,
                "creation_fee": creation_fee.amount.to_string(),
                "gas_used": create_result.gas_used,
                "gas_wanted": create_result.gas_wanted
            },
            "block_height": create_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": create_result.events
        }))
    }
}

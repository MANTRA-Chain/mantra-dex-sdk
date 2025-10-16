//! Skip protocol methods for cross-chain operations

use super::*;

impl McpSdkAdapter {
    // Skip Protocol Tools
    // =============================================================================

    /// Find optimal cross-chain routes between assets
    pub async fn skip_get_route(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting Skip route with args: {:?}", args);

        // Parse required parameters
        let source_asset_denom = args
            .get("source_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_asset_denom is required".to_string())
            })?;

        let source_asset_amount = args
            .get("source_asset_amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_asset_amount is required".to_string())
            })?;

        let source_chain = args
            .get("source_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_chain is required".to_string())
            })?;

        let target_asset_denom = args
            .get("target_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("target_asset_denom is required".to_string())
            })?;

        let target_chain = args
            .get("target_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("target_chain is required".to_string())
            })?;

        // Optional parameters
        let allow_multi_tx = args
            .get("allow_multi_tx")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let smart_relay = args
            .get("smart_relay")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Parse amount
        let amount = Uint128::from_str(source_asset_amount).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid source_asset_amount: {}", e))
        })?;

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get Skip client
        let skip_client = client.skip().await.map_err(McpServerError::Sdk)?;

        // Create source and target assets
        use crate::protocols::skip::types::CrossChainAsset;
        let source_asset = CrossChainAsset {
            denom: source_asset_denom.to_string(),
            amount,
            chain: source_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let target_asset = CrossChainAsset {
            denom: target_asset_denom.to_string(),
            amount: Uint128::zero(), // Amount will be calculated by routing
            chain: target_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        // Create route options
        use crate::protocols::skip::client::RouteOptions;
        let options = Some(RouteOptions {
            allow_multi_tx,
            smart_relay,
            allowed_bridges: None,
            affiliate_fee_bps: None,
        });

        // Get routes
        let routes = skip_client
            .get_route(&source_asset, &target_asset, options)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "get_route",
            "source_asset": {
                "denom": source_asset.denom,
                "amount": source_asset.amount.to_string(),
                "chain": source_asset.chain
            },
            "target_asset": {
                "denom": target_asset.denom,
                "chain": target_asset.chain
            },
            "routes": routes,
            "route_count": routes.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Execute cross-chain asset transfers
    pub async fn skip_execute_transfer(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Executing Skip transfer with args: {:?}", args);

        // Parse required parameters
        let source_asset_denom = args
            .get("source_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_asset_denom is required".to_string())
            })?;

        let source_asset_amount = args
            .get("source_asset_amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_asset_amount is required".to_string())
            })?;

        let source_chain = args
            .get("source_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_chain is required".to_string())
            })?;

        let target_asset_denom = args
            .get("target_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("target_asset_denom is required".to_string())
            })?;

        let target_chain = args
            .get("target_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("target_chain is required".to_string())
            })?;

        let recipient = args
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("recipient is required".to_string()))?;

        // Optional parameters
        let timeout_seconds = args.get("timeout_seconds").and_then(|v| v.as_u64());
        let slippage_tolerance = args
            .get("slippage_tolerance")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Parse amount
        let amount = Uint128::from_str(source_asset_amount).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid source_asset_amount: {}", e))
        })?;

        // Get network config and wallet
        let network_config = self.get_default_network_config().await?;
        let wallet = self.get_active_wallet_with_validation().await?;

        // Create MantraClient with wallet
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet)))
            .await
            .map_err(McpServerError::Sdk)?;

        // Get Skip client
        let skip_client = client.skip().await.map_err(McpServerError::Sdk)?;

        // Create transfer request
        use crate::protocols::skip::types::{CrossChainAsset, TransferRequest};
        let source_asset = CrossChainAsset {
            denom: source_asset_denom.to_string(),
            amount,
            chain: source_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let target_asset = CrossChainAsset {
            denom: target_asset_denom.to_string(),
            amount: Uint128::zero(),
            chain: target_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let transfer_request = TransferRequest {
            source_asset,
            target_asset,
            recipient: recipient.to_string(),
            timeout_seconds,
            slippage_tolerance,
            route: None, // Let the client find the best route
        };

        // Execute transfer
        let result = skip_client
            .execute_cross_chain_transfer(&transfer_request)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "execute_transfer",
            "transfer_id": result.transfer_id,
            "transfer_status": result.status,
            "source_tx_hash": result.source_tx_hash,
            "recipient": recipient,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Monitor transfer status and progress
    pub async fn skip_track_transfer(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Tracking Skip transfer with args: {:?}", args);

        // Parse required parameters
        let transfer_id = args
            .get("transfer_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("transfer_id is required".to_string())
            })?;

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get Skip client
        let skip_client = client.skip().await.map_err(McpServerError::Sdk)?;

        // Track transfer
        let result = skip_client
            .track_transfer(transfer_id)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "track_transfer",
            "transfer_id": result.transfer_id,
            "transfer_status": result.status,
            "source_tx_hash": result.source_tx_hash,
            "dest_tx_hash": result.dest_tx_hash,
            "amount_transferred": result.amount_transferred.map(|a| a.to_string()),
            "error_message": result.error_message,
            "initiated_at": result.initiated_at,
            "completed_at": result.completed_at,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// List available chains and their configurations
    pub async fn skip_get_supported_chains(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting supported chains with args: {:?}",
            args
        );

        // Optional filter parameter
        let filter = args.get("filter").and_then(|v| v.as_str());

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get Skip client
        let skip_client = client.skip().await.map_err(McpServerError::Sdk)?;

        // Get supported chains
        let mut chains = skip_client
            .get_supported_chains()
            .await
            .map_err(McpServerError::Sdk)?;

        // Apply filter if provided
        if let Some(filter_str) = filter {
            chains.retain(|chain| {
                chain
                    .chain_name
                    .to_lowercase()
                    .contains(&filter_str.to_lowercase())
                    || chain
                        .chain_id
                        .to_lowercase()
                        .contains(&filter_str.to_lowercase())
            });
        }

        Ok(serde_json::json!({
            "status": "success",
            "operation": "get_supported_chains",
            "chains": chains,
            "chain_count": chains.len(),
            "filter": filter,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Validate assets across different chains
    pub async fn skip_verify_assets(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Verifying assets with args: {:?}", args);

        // Parse required parameters
        let assets_array = args
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("assets array is required".to_string())
            })?;

        // Parse assets
        use crate::protocols::skip::types::CrossChainAsset;
        let mut assets = Vec::new();

        for asset_value in assets_array {
            let denom = asset_value
                .get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.denom is required".to_string())
                })?;

            let chain = asset_value
                .get("chain")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.chain is required".to_string())
                })?;

            let amount_str = asset_value
                .get("amount")
                .and_then(|v| v.as_str())
                .unwrap_or("0");

            let amount = Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid asset amount: {}", e))
            })?;

            assets.push(CrossChainAsset {
                denom: denom.to_string(),
                amount,
                chain: chain.to_string(),
                decimals: asset_value
                    .get("decimals")
                    .and_then(|v| v.as_u64())
                    .map(|d| d as u8),
                symbol: asset_value
                    .get("symbol")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get Skip client
        let skip_client = client.skip().await.map_err(McpServerError::Sdk)?;

        // Verify assets
        let verification_result = skip_client
            .verify_assets(&assets)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "verify_assets",
            "verified_assets": verification_result.verified_assets,
            "invalid_assets": verification_result.invalid_assets,
            "verification_timestamp": verification_result.verification_timestamp,
            "summary": {
                "total_assets": assets.len(),
                "verified_count": verification_result.verified_assets.len(),
                "invalid_count": verification_result.invalid_assets.len()
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Estimate fees for cross-chain operations
    pub async fn skip_estimate_fees(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Estimating fees with args: {:?}", args);

        // Parse required parameters
        let source_asset_denom = args
            .get("source_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_asset_denom is required".to_string())
            })?;

        let source_asset_amount = args
            .get("source_asset_amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_asset_amount is required".to_string())
            })?;

        let source_chain = args
            .get("source_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("source_chain is required".to_string())
            })?;

        let target_asset_denom = args
            .get("target_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("target_asset_denom is required".to_string())
            })?;

        let target_chain = args
            .get("target_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("target_chain is required".to_string())
            })?;

        let recipient = args
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("recipient is required".to_string()))?;

        // Parse amount
        let amount = Uint128::from_str(source_asset_amount).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid source_asset_amount: {}", e))
        })?;

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get Skip client
        let skip_client = client.skip().await.map_err(McpServerError::Sdk)?;

        // Create transfer request for fee estimation
        use crate::protocols::skip::types::{CrossChainAsset, TransferRequest};
        let source_asset = CrossChainAsset {
            denom: source_asset_denom.to_string(),
            amount,
            chain: source_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let target_asset = CrossChainAsset {
            denom: target_asset_denom.to_string(),
            amount: Uint128::zero(),
            chain: target_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let transfer_request = TransferRequest {
            source_asset,
            target_asset,
            recipient: recipient.to_string(),
            timeout_seconds: None,
            slippage_tolerance: None,
            route: None,
        };

        // Estimate fees
        let fee_estimate = skip_client
            .estimate_fees(&transfer_request)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "estimate_fees",
            "total_fees": fee_estimate.total_fees,
            "gas_estimates": fee_estimate.gas_estimates,
            "route_steps": fee_estimate.route_steps,
            "estimated_time_seconds": fee_estimate.estimated_time_seconds,
            "price_impact": fee_estimate.price_impact,
            "summary": {
                "total_fee_amount": fee_estimate.total_fees.iter()
                    .map(|f| format!("{}{}", f.amount, f.denom))
                    .collect::<Vec<_>>().join(", "),
                "step_count": fee_estimate.route_steps
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    // =============================================================================
}

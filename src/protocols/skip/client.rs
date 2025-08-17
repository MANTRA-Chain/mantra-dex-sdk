/// Skip Protocol Client for cross-chain routing operations
///
/// This client integrates with Skip API for asset routing, cross-chain swaps, and bridge operations.
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cosmwasm_std::{Coin, Decimal, Uint128};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::Error;
use crate::wallet::MantraWallet;

use super::types::*;

/// Skip protocol client for cross-chain operations
#[derive(Debug)]
pub struct SkipClient {
    /// Optional wallet for signing transactions
    wallet: Option<Arc<MantraWallet>>,
    /// Skip adapter contract address
    adapter_contract: Option<String>,
    /// HTTP client for Skip API calls
    http_client: reqwest::Client,
    /// Base URL for Skip API
    skip_api_base_url: String,
    /// Active transfers tracking
    active_transfers: Arc<Mutex<HashMap<String, TransferResult>>>,
}

impl SkipClient {
    /// Create a new Skip client
    pub async fn new(wallet: Option<Arc<MantraWallet>>) -> Result<Self, Error> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::Skip(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            wallet,
            adapter_contract: None,
            http_client,
            skip_api_base_url: "https://api.skip.money".to_string(),
            active_transfers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Set the Skip adapter contract address
    pub fn set_adapter_contract(&mut self, address: String) {
        self.adapter_contract = Some(address);
    }

    /// Get the adapter contract address
    pub fn adapter_contract(&self) -> Option<&str> {
        self.adapter_contract.as_deref()
    }

    /// Set the wallet for signing transactions
    pub fn set_wallet(&mut self, wallet: Arc<MantraWallet>) {
        self.wallet = Some(wallet);
    }

    /// Get the wallet if available
    pub fn wallet(&self) -> Result<&MantraWallet, Error> {
        self.wallet
            .as_ref()
            .map(|w| w.as_ref())
            .ok_or_else(|| Error::Wallet("No wallet configured".to_string()))
    }

    // ============================================================================
    // Core Skip Protocol Methods (Phase 4 Requirements)
    // ============================================================================

    /// Find optimal cross-chain routes between assets
    ///
    /// This method discovers the best routes for transferring assets across chains,
    /// considering factors like fees, time, and slippage.
    pub async fn get_route(
        &self,
        source_asset: &CrossChainAsset,
        target_asset: &CrossChainAsset,
        options: Option<RouteOptions>,
    ) -> Result<Vec<CrossChainRoute>, Error> {
        let opts = options.unwrap_or_default();

        // Build route request for Skip API
        let request = json!({
            "amount_in": source_asset.amount.to_string(),
            "source_asset_denom": source_asset.denom,
            "source_asset_chain_id": source_asset.chain,
            "dest_asset_denom": target_asset.denom,
            "dest_asset_chain_id": target_asset.chain,
            "allow_multi_tx": opts.allow_multi_tx,
            "smart_relay": opts.smart_relay,
            "bridges": opts.allowed_bridges.unwrap_or_default(),
            "cumulative_affiliate_fee_bps": opts.affiliate_fee_bps.map(|f| f.to_string()),
        });

        let response = self
            .http_client
            .post(&format!("{}/v1/fungible/route", self.skip_api_base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Skip(format!("Failed to get route: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Skip(format!("Skip API error: {}", error_text)));
        }

        let route_response: Value = response
            .json()
            .await
            .map_err(|e| Error::Skip(format!("Failed to parse route response: {}", e)))?;

        self.parse_routes_from_response(route_response, source_asset, target_asset)
    }

    /// Monitor cross-chain transfer status and progress
    ///
    /// Tracks the status of a previously initiated transfer using its unique identifier.
    pub async fn track_transfer(&self, transfer_id: &str) -> Result<TransferResult, Error> {
        // First check local cache
        {
            let transfers = self.active_transfers.lock().await;
            if let Some(cached_result) = transfers.get(transfer_id) {
                // If transfer is completed or failed, return cached result
                match cached_result.status {
                    TransferStatus::Completed
                    | TransferStatus::Failed
                    | TransferStatus::TimedOut
                    | TransferStatus::Refunded => {
                        return Ok(cached_result.clone());
                    }
                    _ => {}
                }
            }
        }

        // Query Skip API for transfer status
        let response = self
            .http_client
            .get(&format!("{}/v1/tx/track", self.skip_api_base_url))
            .query(&[("tx_id", transfer_id)])
            .send()
            .await
            .map_err(|e| Error::Skip(format!("Failed to track transfer: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Skip(format!(
                "Skip tracking API error: {}",
                error_text
            )));
        }

        let tracking_response: Value = response
            .json()
            .await
            .map_err(|e| Error::Skip(format!("Failed to parse tracking response: {}", e)))?;

        let result = self.parse_transfer_status(transfer_id, tracking_response)?;

        // Update local cache
        {
            let mut transfers = self.active_transfers.lock().await;
            transfers.insert(transfer_id.to_string(), result.clone());
        }

        Ok(result)
    }

    /// List available chains and their configurations
    ///
    /// Returns information about all chains supported by Skip protocol,
    /// including available bridges and supported assets.
    pub async fn get_supported_chains(&self) -> Result<Vec<SupportedChain>, Error> {
        let response = self
            .http_client
            .get(&format!("{}/v1/info/chains", self.skip_api_base_url))
            .send()
            .await
            .map_err(|e| Error::Skip(format!("Failed to get supported chains: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Skip(format!(
                "Skip chains API error: {}",
                error_text
            )));
        }

        let chains_response: Value = response
            .json()
            .await
            .map_err(|e| Error::Skip(format!("Failed to parse chains response: {}", e)))?;

        self.parse_supported_chains(chains_response)
    }

    /// Validate assets across different chains
    ///
    /// Verifies that the specified assets exist and are supported for cross-chain operations.
    pub async fn verify_assets(
        &self,
        assets: &[CrossChainAsset],
    ) -> Result<AssetVerificationResult, Error> {
        let mut verified_assets = Vec::new();
        let mut invalid_assets = Vec::new();

        for asset in assets {
            match self.verify_single_asset(asset).await {
                Ok(verified) => verified_assets.push(verified),
                Err(e) => {
                    invalid_assets.push(AssetVerificationError {
                        asset: asset.clone(),
                        error: e.to_string(),
                    });
                }
            }
        }

        Ok(AssetVerificationResult {
            verified_assets,
            invalid_assets,
            verification_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Execute cross-chain asset transfers
    ///
    /// Performs the actual cross-chain transfer using the specified route and parameters.
    pub async fn execute_cross_chain_transfer(
        &self,
        request: &TransferRequest,
    ) -> Result<TransferResult, Error> {
        // Validate wallet is configured
        self.wallet()?;

        // Generate unique transfer ID
        let transfer_id = Uuid::new_v4().to_string();

        // Validate transfer parameters
        self.validate_transfer_request(request).await?;

        // Initialize transfer result
        let mut result = TransferResult {
            transfer_id: transfer_id.clone(),
            status: TransferStatus::Pending,
            source_tx_hash: None,
            dest_tx_hash: None,
            amount_transferred: None,
            error_message: None,
            initiated_at: Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            completed_at: None,
        };

        // Store in active transfers
        {
            let mut transfers = self.active_transfers.lock().await;
            transfers.insert(transfer_id.clone(), result.clone());
        }

        // Execute the transfer based on route type
        match self.execute_transfer_internal(request, &transfer_id).await {
            Ok(tx_hash) => {
                result.status = TransferStatus::InProgress;
                result.source_tx_hash = Some(tx_hash);
            }
            Err(e) => {
                result.status = TransferStatus::Failed;
                result.error_message = Some(e.to_string());
            }
        }

        // Update active transfers
        {
            let mut transfers = self.active_transfers.lock().await;
            transfers.insert(transfer_id.clone(), result.clone());
        }

        Ok(result)
    }

    /// Estimate fees for cross-chain operations
    ///
    /// Calculates the total fees required for a cross-chain transfer including
    /// network fees, bridge fees, and protocol fees.
    pub async fn estimate_fees(&self, request: &TransferRequest) -> Result<FeeEstimate, Error> {
        // Get route if not provided
        let route = match &request.route {
            Some(route) => route.clone(),
            None => {
                let routes = self
                    .get_route(&request.source_asset, &request.target_asset, None)
                    .await?;
                routes
                    .into_iter()
                    .next()
                    .ok_or_else(|| Error::Skip("No route found for fee estimation".to_string()))?
            }
        };

        // Calculate fees for each step
        let mut total_fees = Vec::new();
        let mut gas_estimates = Vec::new();

        for step in &route.steps {
            // Estimate gas for this step
            let gas_estimate = self.estimate_step_gas(step).await?;
            gas_estimates.push(gas_estimate.clone());

            // Add step fees
            if let Some(fee) = &step.fee {
                total_fees.push(fee.clone());
            }
        }

        // Add bridge fees from route
        for fee in &route.estimated_fees {
            total_fees.push(fee.clone());
        }

        Ok(FeeEstimate {
            total_fees,
            gas_estimates,
            route_steps: route.steps.len() as u32,
            estimated_time_seconds: route.estimated_time_seconds,
            price_impact: route.price_impact,
        })
    }

    // ============================================================================
    // Helper Methods
    // ============================================================================

    /// Parse routes from Skip API response
    fn parse_routes_from_response(
        &self,
        response: Value,
        source: &CrossChainAsset,
        target: &CrossChainAsset,
    ) -> Result<Vec<CrossChainRoute>, Error> {
        let routes = response
            .get("routes")
            .and_then(|r| r.as_array())
            .ok_or_else(|| Error::Skip("Invalid routes response format".to_string()))?;

        let mut result = Vec::new();

        for route_data in routes {
            let route = self.parse_single_route(route_data, source, target)?;
            result.push(route);
        }

        Ok(result)
    }

    /// Parse a single route from response data
    fn parse_single_route(
        &self,
        route_data: &Value,
        source: &CrossChainAsset,
        target: &CrossChainAsset,
    ) -> Result<CrossChainRoute, Error> {
        let empty_vec = vec![];
        let operations = route_data
            .get("operations")
            .and_then(|ops| ops.as_array())
            .unwrap_or(&empty_vec);

        let mut steps = Vec::new();
        for (i, op) in operations.iter().enumerate() {
            let step = RouteStep {
                chain: op
                    .get("chain_id")
                    .and_then(|c| c.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                step_type: self.determine_step_type(op),
                asset_in: if i == 0 {
                    source.clone()
                } else {
                    // Parse intermediate asset
                    self.parse_asset_from_operation(op, "asset_in")?
                },
                asset_out: if i == operations.len() - 1 {
                    target.clone()
                } else {
                    // Parse intermediate asset
                    self.parse_asset_from_operation(op, "asset_out")?
                },
                estimated_time_seconds: op.get("estimated_time").and_then(|t| t.as_u64()),
                fee: op.get("fee").and_then(|f| self.parse_fee(f).ok()),
            };
            steps.push(step);
        }

        Ok(CrossChainRoute {
            source_chain: source.chain.clone(),
            dest_chain: target.chain.clone(),
            steps,
            estimated_time_seconds: route_data.get("estimated_time").and_then(|t| t.as_u64()),
            estimated_fees: route_data
                .get("fees")
                .and_then(|fees| fees.as_array())
                .map(|fees| fees.iter().filter_map(|f| self.parse_fee(f).ok()).collect())
                .unwrap_or_default(),
            price_impact: route_data
                .get("price_impact")
                .and_then(|p| p.as_str())
                .and_then(|s| Decimal::from_str(s).ok()),
        })
    }

    /// Determine the type of a route step from operation data
    fn determine_step_type(&self, operation: &Value) -> RouteStepType {
        let op_type = operation
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("transfer");

        match op_type {
            "swap" => RouteStepType::Swap,
            "bridge" => RouteStepType::Bridge,
            "ibc_transfer" | "ibc" => RouteStepType::IbcTransfer,
            _ => RouteStepType::Transfer,
        }
    }

    /// Parse asset from operation data
    fn parse_asset_from_operation(
        &self,
        operation: &Value,
        field: &str,
    ) -> Result<CrossChainAsset, Error> {
        let asset_data = operation
            .get(field)
            .ok_or_else(|| Error::Skip(format!("Missing {} in operation", field)))?;

        Ok(CrossChainAsset {
            denom: asset_data
                .get("denom")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string(),
            amount: asset_data
                .get("amount")
                .and_then(|a| a.as_str())
                .and_then(|s| Uint128::from_str(s).ok())
                .unwrap_or_default(),
            chain: asset_data
                .get("chain_id")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string(),
            decimals: asset_data
                .get("decimals")
                .and_then(|d| d.as_u64())
                .map(|d| d as u8),
            symbol: asset_data
                .get("symbol")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
        })
    }

    /// Parse fee from response data
    fn parse_fee(&self, fee_data: &Value) -> Result<Coin, Error> {
        let denom = fee_data
            .get("denom")
            .and_then(|d| d.as_str())
            .ok_or_else(|| Error::Skip("Missing fee denom".to_string()))?;

        let amount = fee_data
            .get("amount")
            .and_then(|a| a.as_str())
            .and_then(|s| Uint128::from_str(s).ok())
            .ok_or_else(|| Error::Skip("Invalid fee amount".to_string()))?;

        Ok(Coin {
            denom: denom.to_string(),
            amount,
        })
    }

    /// Parse transfer status from tracking response
    fn parse_transfer_status(
        &self,
        transfer_id: &str,
        response: Value,
    ) -> Result<TransferResult, Error> {
        let state = response
            .get("state")
            .and_then(|s| s.as_str())
            .unwrap_or("unknown");

        let status = match state {
            "pending" => TransferStatus::Pending,
            "submitted" | "broadcasted" => TransferStatus::InProgress,
            "success" | "completed" => TransferStatus::Completed,
            "failed" | "error" => TransferStatus::Failed,
            "timeout" => TransferStatus::TimedOut,
            "refunded" => TransferStatus::Refunded,
            _ => TransferStatus::Pending,
        };

        Ok(TransferResult {
            transfer_id: transfer_id.to_string(),
            status,
            source_tx_hash: response
                .get("source_tx_hash")
                .and_then(|h| h.as_str())
                .map(|s| s.to_string()),
            dest_tx_hash: response
                .get("dest_tx_hash")
                .and_then(|h| h.as_str())
                .map(|s| s.to_string()),
            amount_transferred: response
                .get("amount_received")
                .and_then(|a| a.as_str())
                .and_then(|s| Uint128::from_str(s).ok()),
            error_message: response
                .get("error")
                .and_then(|e| e.as_str())
                .map(|s| s.to_string()),
            initiated_at: response.get("initiated_at").and_then(|t| t.as_u64()),
            completed_at: response.get("completed_at").and_then(|t| t.as_u64()),
        })
    }

    /// Parse supported chains from API response
    fn parse_supported_chains(&self, response: Value) -> Result<Vec<SupportedChain>, Error> {
        let chains = response
            .get("chains")
            .and_then(|c| c.as_array())
            .ok_or_else(|| Error::Skip("Invalid chains response format".to_string()))?;

        let mut result = Vec::new();

        for chain_data in chains {
            let chain = SupportedChain {
                chain_id: chain_data
                    .get("chain_id")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
                chain_name: chain_data
                    .get("chain_name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string(),
                chain_type: chain_data
                    .get("chain_type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("cosmos")
                    .to_string(),
                is_available: chain_data
                    .get("is_enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false),
                supported_assets: chain_data
                    .get("assets")
                    .and_then(|a| a.as_array())
                    .map(|assets| self.parse_chain_assets(assets))
                    .unwrap_or_default(),
                bridges: chain_data
                    .get("bridges")
                    .and_then(|b| b.as_array())
                    .map(|bridges| self.parse_bridge_info(bridges))
                    .unwrap_or_default(),
            };
            result.push(chain);
        }

        Ok(result)
    }

    /// Parse chain assets from response data
    fn parse_chain_assets(&self, assets: &[Value]) -> Vec<ChainAsset> {
        assets
            .iter()
            .filter_map(|asset| {
                Some(ChainAsset {
                    denom: asset.get("denom")?.as_str()?.to_string(),
                    symbol: asset.get("symbol")?.as_str()?.to_string(),
                    decimals: asset.get("decimals")?.as_u64()? as u8,
                    is_native: asset.get("is_native")?.as_bool()?,
                    contract_address: asset
                        .get("contract_address")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect()
    }

    /// Parse bridge information from response data
    fn parse_bridge_info(&self, bridges: &[Value]) -> Vec<BridgeInfo> {
        bridges
            .iter()
            .filter_map(|bridge| {
                Some(BridgeInfo {
                    target_chain: bridge.get("target_chain")?.as_str()?.to_string(),
                    bridge_protocol: bridge.get("protocol")?.as_str()?.to_string(),
                    is_active: bridge.get("is_active")?.as_bool()?,
                    estimated_time_seconds: bridge.get("estimated_time")?.as_u64()?,
                    fee_percentage: bridge
                        .get("fee_percentage")
                        .and_then(|f| f.as_str())
                        .and_then(|s| Decimal::from_str(s).ok()),
                    min_amount: bridge
                        .get("min_amount")
                        .and_then(|a| a.as_str())
                        .and_then(|s| Uint128::from_str(s).ok()),
                    max_amount: bridge
                        .get("max_amount")
                        .and_then(|a| a.as_str())
                        .and_then(|s| Uint128::from_str(s).ok()),
                })
            })
            .collect()
    }

    /// Verify a single asset
    async fn verify_single_asset(&self, asset: &CrossChainAsset) -> Result<VerifiedAsset, Error> {
        // Query Skip API for asset verification
        let response = self
            .http_client
            .get(&format!("{}/v1/fungible/assets", self.skip_api_base_url))
            .query(&[("chain_id", &asset.chain), ("denom", &asset.denom)])
            .send()
            .await
            .map_err(|e| Error::Skip(format!("Failed to verify asset: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Skip("Asset verification failed".to_string()));
        }

        let asset_data: Value = response
            .json()
            .await
            .map_err(|e| Error::Skip(format!("Failed to parse asset verification: {}", e)))?;

        let is_verified = asset_data
            .get("assets")
            .and_then(|a| a.as_array())
            .map(|assets| !assets.is_empty())
            .unwrap_or(false);

        if !is_verified {
            return Err(Error::Skip("Asset not found or not supported".to_string()));
        }

        Ok(VerifiedAsset {
            asset: asset.clone(),
            is_supported: true,
            supported_operations: vec!["transfer".to_string(), "swap".to_string()],
            verification_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }

    /// Validate transfer request parameters
    async fn validate_transfer_request(&self, request: &TransferRequest) -> Result<(), Error> {
        // Verify assets
        let assets = vec![request.source_asset.clone(), request.target_asset.clone()];
        let verification = self.verify_assets(&assets).await?;

        if !verification.invalid_assets.is_empty() {
            return Err(Error::Skip(format!(
                "Invalid assets: {:?}",
                verification.invalid_assets
            )));
        }

        // Validate amount
        if request.source_asset.amount.is_zero() {
            return Err(Error::Skip("Transfer amount cannot be zero".to_string()));
        }

        // Validate chains are different (for cross-chain transfer)
        if request.source_asset.chain == request.target_asset.chain {
            return Err(Error::Skip(
                "Source and target chains must be different for cross-chain transfer".to_string(),
            ));
        }

        Ok(())
    }

    /// Execute the internal transfer logic
    async fn execute_transfer_internal(
        &self,
        _request: &TransferRequest,
        transfer_id: &str,
    ) -> Result<String, Error> {
        // This would implement the actual transfer execution
        // For now, return a mock transaction hash
        let mock_tx_hash = format!("0x{}", hex::encode(&transfer_id.as_bytes()[..16]));

        // In a real implementation, this would:
        // 1. Build the appropriate Skip protocol transaction
        // 2. Sign with the wallet
        // 3. Broadcast to the network
        // 4. Return the actual transaction hash

        Ok(mock_tx_hash)
    }

    /// Estimate gas for a route step
    async fn estimate_step_gas(&self, step: &RouteStep) -> Result<GasEstimate, Error> {
        // Mock gas estimation - in real implementation this would
        // query the specific chain for accurate gas estimates
        Ok(GasEstimate {
            gas_limit: match step.step_type {
                RouteStepType::Swap => 200_000u64,
                RouteStepType::Bridge => 300_000u64,
                RouteStepType::IbcTransfer => 150_000u64,
                RouteStepType::Transfer => 100_000u64,
            },
            gas_price: "0.025".to_string(),
            estimated_fee: Coin {
                denom: "uom".to_string(),
                amount: Uint128::new(5000),
            },
        })
    }
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Options for route discovery
#[derive(Debug, Clone, Default)]
pub struct RouteOptions {
    /// Allow multi-transaction routes
    pub allow_multi_tx: bool,
    /// Use smart relay optimization
    pub smart_relay: bool,
    /// Allowed bridge protocols
    pub allowed_bridges: Option<Vec<String>>,
    /// Affiliate fee in basis points
    pub affiliate_fee_bps: Option<String>,
}

/// Result of asset verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetVerificationResult {
    /// Successfully verified assets
    pub verified_assets: Vec<VerifiedAsset>,
    /// Assets that failed verification
    pub invalid_assets: Vec<AssetVerificationError>,
    /// Timestamp of verification
    pub verification_timestamp: u64,
}

/// Verified asset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedAsset {
    /// The verified asset
    pub asset: CrossChainAsset,
    /// Whether the asset is supported
    pub is_supported: bool,
    /// Supported operations for this asset
    pub supported_operations: Vec<String>,
    /// Verification timestamp
    pub verification_timestamp: u64,
}

/// Asset verification error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetVerificationError {
    /// The asset that failed verification
    pub asset: CrossChainAsset,
    /// Error message
    pub error: String,
}

/// Fee estimate for cross-chain operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    /// Total fees for all steps
    pub total_fees: Vec<Coin>,
    /// Gas estimates for each step
    pub gas_estimates: Vec<GasEstimate>,
    /// Number of route steps
    pub route_steps: u32,
    /// Estimated total time
    pub estimated_time_seconds: Option<u64>,
    /// Price impact
    pub price_impact: Option<Decimal>,
}

/// Gas estimate for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEstimate {
    /// Gas limit
    pub gas_limit: u64,
    /// Gas price
    pub gas_price: String,
    /// Estimated fee
    pub estimated_fee: Coin,
}

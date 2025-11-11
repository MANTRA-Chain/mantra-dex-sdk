/// Skip protocol types and message structures
/// Based on Skip Go CosmWasm contracts for cross-chain operations
use cosmwasm_std::{Coin, Decimal, Uint128};
use serde::{Deserialize, Serialize};

// ============================================================================
// Cross-Chain Route Types
// ============================================================================

/// Cross-chain route between different blockchain networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainRoute {
    /// Source chain identifier
    pub source_chain: String,
    /// Destination chain identifier  
    pub dest_chain: String,
    /// Route steps across chains
    pub steps: Vec<RouteStep>,
    /// Estimated total time in seconds
    pub estimated_time_seconds: Option<u64>,
    /// Estimated fees for the entire route
    pub estimated_fees: Vec<Coin>,
    /// Price impact percentage
    pub price_impact: Option<Decimal>,
}

/// Individual step in a cross-chain route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    /// Chain where this step executes
    pub chain: String,
    /// Step type (swap, bridge, transfer)
    pub step_type: RouteStepType,
    /// Input asset for this step
    pub asset_in: CrossChainAsset,
    /// Output asset from this step
    pub asset_out: CrossChainAsset,
    /// Estimated execution time in seconds
    pub estimated_time_seconds: Option<u64>,
    /// Fee for this specific step
    pub fee: Option<Coin>,
}

/// Type of operation in a route step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStepType {
    /// Token swap on the same chain
    Swap,
    /// Cross-chain bridge transfer
    Bridge,
    /// Direct transfer (no conversion)
    Transfer,
    /// IBC transfer between Cosmos chains
    IbcTransfer,
}

// ============================================================================
// Cross-Chain Asset Types
// ============================================================================

/// Asset that can exist across multiple chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainAsset {
    /// Asset denomination or contract address
    pub denom: String,
    /// Amount of the asset
    pub amount: Uint128,
    /// Chain where this asset exists
    pub chain: String,
    /// Optional decimals for display
    pub decimals: Option<u8>,
    /// Human-readable symbol
    pub symbol: Option<String>,
}

/// Pairing of assets across chains for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPair {
    /// Source asset
    pub source: CrossChainAsset,
    /// Target asset
    pub target: CrossChainAsset,
}

/// Asset representation for Skip operations (existing type)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipAsset {
    /// Native cosmos coin
    Native(Coin),
    /// CW20 token (not supported yet)
    Cw20(Cw20Coin),
}

impl SkipAsset {
    /// Create a new native asset
    pub fn native(denom: &str, amount: impl Into<Uint128>) -> Self {
        Self::Native(Coin {
            denom: denom.to_string(),
            amount: amount.into(),
        })
    }

    /// Get the denomination of the asset
    pub fn denom(&self) -> String {
        match self {
            SkipAsset::Native(coin) => coin.denom.clone(),
            SkipAsset::Cw20(coin) => coin.address.clone(),
        }
    }

    /// Get the amount of the asset
    pub fn amount(&self) -> Uint128 {
        match self {
            SkipAsset::Native(coin) => coin.amount,
            SkipAsset::Cw20(coin) => coin.amount,
        }
    }
}

/// CW20 token representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cw20Coin {
    /// Contract address
    pub address: String,
    /// Amount
    pub amount: Uint128,
}

// ============================================================================
// Transfer Types
// ============================================================================

/// Request to initiate a cross-chain transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRequest {
    /// Source asset to transfer
    pub source_asset: CrossChainAsset,
    /// Target asset to receive
    pub target_asset: CrossChainAsset,
    /// Recipient address on destination chain
    pub recipient: String,
    /// Optional timeout for the transfer
    pub timeout_seconds: Option<u64>,
    /// Slippage tolerance (percentage)
    pub slippage_tolerance: Option<Decimal>,
    /// Route to use for the transfer
    pub route: Option<CrossChainRoute>,
}

/// Status of a cross-chain transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    /// Transfer is being prepared
    Pending,
    /// Transfer is in progress
    InProgress,
    /// Transfer completed successfully
    Completed,
    /// Transfer failed
    Failed,
    /// Transfer timed out
    TimedOut,
    /// Transfer was refunded
    Refunded,
}

/// Result of a cross-chain transfer operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    /// Unique identifier for the transfer
    pub transfer_id: String,
    /// Current status of the transfer
    pub status: TransferStatus,
    /// Transaction hash on source chain
    pub source_tx_hash: Option<String>,
    /// Transaction hash on destination chain
    pub dest_tx_hash: Option<String>,
    /// Amount actually transferred
    pub amount_transferred: Option<Uint128>,
    /// Error message if transfer failed
    pub error_message: Option<String>,
    /// Timestamp when transfer was initiated
    pub initiated_at: Option<u64>,
    /// Timestamp when transfer was completed
    pub completed_at: Option<u64>,
}

// ============================================================================
// Chain Types
// ============================================================================

/// Information about a supported blockchain network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedChain {
    /// Chain identifier
    pub chain_id: String,
    /// Human-readable chain name
    pub chain_name: String,
    /// Chain type (cosmos, ethereum, etc.)
    pub chain_type: String,
    /// Whether the chain is currently available
    pub is_available: bool,
    /// Supported assets on this chain
    pub supported_assets: Vec<ChainAsset>,
    /// Bridge configurations
    pub bridges: Vec<BridgeInfo>,
}

/// Configuration for a specific chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Chain identifier
    pub chain_id: String,
    /// RPC endpoint
    pub rpc_url: String,
    /// Block confirmation requirements
    pub confirmations_required: u32,
    /// Average block time in seconds
    pub avg_block_time_seconds: u32,
    /// Gas price configuration
    pub gas_price: Option<String>,
    /// Fee token denom
    pub fee_denom: String,
}

/// Asset configuration specific to a chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainAsset {
    /// Asset denomination on this chain
    pub denom: String,
    /// Asset symbol
    pub symbol: String,
    /// Number of decimals
    pub decimals: u8,
    /// Whether asset is native to this chain
    pub is_native: bool,
    /// Contract address (for non-native assets)
    pub contract_address: Option<String>,
}

/// Bridge connection between two chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    /// Target chain for this bridge
    pub target_chain: String,
    /// Bridge protocol name
    pub bridge_protocol: String,
    /// Whether the bridge is currently active
    pub is_active: bool,
    /// Estimated transfer time in seconds
    pub estimated_time_seconds: u64,
    /// Bridge fee percentage
    pub fee_percentage: Option<Decimal>,
    /// Minimum transfer amount
    pub min_amount: Option<Uint128>,
    /// Maximum transfer amount
    pub max_amount: Option<Uint128>,
}

// ============================================================================
// Skip Contract Types
// ============================================================================

/// Skip swap operation for routing through adapters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSwapOperation {
    /// Pool identifier for the swap
    pub pool: String,
    /// Input token denomination
    pub denom_in: String,
    /// Output token denomination
    pub denom_out: String,
    /// Interface type (optional, defaults to None for standard swaps)
    pub interface: Option<String>,
}

/// Route for Skip smart swaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipRoute {
    /// Asset to offer for this route
    pub offer_asset: SkipAsset,
    /// Operations to perform for this route
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip entry point execute messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipEntryPointExecuteMsg {
    /// User swap through Skip protocol
    UserSwap {
        /// The swap to execute
        swap: SkipSwap,
        /// Minimum asset to receive
        min_asset: SkipAsset,
        /// Remaining asset info
        remaining_asset: SkipAsset,
        /// Affiliate addresses for fee sharing
        affiliates: Vec<SkipAffiliate>,
    },
    /// Swap and action combined
    SwapAndAction {
        /// The asset being sent (optional)
        sent_asset: Option<SkipAsset>,
        /// The user swap to execute
        user_swap: SkipSwap,
        /// Minimum asset to receive
        min_asset: SkipAsset,
        /// Timeout timestamp in nanoseconds
        timeout_timestamp: u64,
        /// Action to execute after swap
        post_swap_action: SkipAction,
        /// Affiliate addresses for fee sharing
        affiliates: Vec<SkipAffiliate>,
    },
}

/// Skip swap types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipSwap {
    /// Swap exact amount in
    SwapExactAssetIn(SkipSwapExactAssetIn),
    /// Swap exact amount out
    SwapExactAssetOut(SkipSwapExactAssetOut),
}

/// Skip swap exact asset in
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSwapExactAssetIn {
    /// Swap venue name (e.g., "mantra-dex")
    pub swap_venue_name: String,
    /// Operations to perform
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip swap exact asset out
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSwapExactAssetOut {
    /// Swap venue name (e.g., "mantra-dex")
    pub swap_venue_name: String,
    /// Operations to perform
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip affiliate for fee sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipAffiliate {
    /// Affiliate address
    pub address: String,
    /// Basis points (0-10000, where 10000 = 100%)
    pub basis_points_fee: String,
}

/// Skip action for post-swap execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipAction {
    /// Transfer action
    Transfer {
        /// Recipient address
        to_address: String,
    },
    /// IBC transfer action
    IbcTransfer {
        /// IBC info
        ibc_info: SkipIbcInfo,
        /// Fee swap (optional)
        fee_swap: Option<SkipFeeSwap>,
    },
}

/// Skip IBC info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipIbcInfo {
    /// Source channel
    pub source_channel: String,
    /// Receiver address
    pub receiver: String,
    /// Memo (optional)
    pub memo: Option<String>,
    /// Recover address (optional)
    pub recover_address: Option<String>,
}

/// Skip fee swap info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipFeeSwap {
    /// Fee swap venue name
    pub swap_venue_name: String,
    /// Operations for fee swap
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip entry point query messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipEntryPointQueryMsg {
    /// Simulate a swap exact asset in
    SimulateSwapExactAssetIn {
        /// Asset to swap in
        asset_in: SkipAsset,
        /// Swap operations to perform
        swap_operations: Vec<SkipSwapOperation>,
    },
    /// Simulate a swap exact asset out
    SimulateSwapExactAssetOut {
        /// Asset to get out
        asset_out: SkipAsset,
        /// Swap operations to perform
        swap_operations: Vec<SkipSwapOperation>,
    },
    /// Simulate a smart swap exact asset in
    SimulateSmartSwapExactAssetIn {
        /// Asset to swap in
        asset_in: SkipAsset,
        /// Routes to consider
        routes: Vec<SkipRoute>,
    },
    /// Simulate a swap exact asset in with metadata
    SimulateSwapExactAssetInWithMetadata {
        /// Asset to swap in
        asset_in: SkipAsset,
        /// Swap operations to perform
        swap_operations: Vec<SkipSwapOperation>,
        /// Whether to include spot price in response
        include_spot_price: bool,
    },
    /// Simulate a swap exact asset out with metadata
    SimulateSwapExactAssetOutWithMetadata {
        /// Asset to get out
        asset_out: SkipAsset,
        /// Swap operations to perform
        swap_operations: Vec<SkipSwapOperation>,
        /// Whether to include spot price in response
        include_spot_price: bool,
    },
    /// Simulate a smart swap exact asset in with metadata
    SimulateSmartSwapExactAssetInWithMetadata {
        /// Asset to swap in
        asset_in: SkipAsset,
        /// Routes to consider
        routes: Vec<SkipRoute>,
        /// Whether to include spot price in response
        include_spot_price: bool,
    },
}

// ============================================================================
// Response Types
// ============================================================================

/// Simulate swap exact asset in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateSwapExactAssetInResponse {
    /// The asset out from the simulation
    pub asset_out: SkipAsset,
    /// Optional spot price
    pub spot_price: Option<Decimal>,
}

/// Alternative response format that matches actual contract response
pub type SimulateSwapExactAssetInDirectResponse = SkipAsset;

/// Simulate swap exact asset out response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateSwapExactAssetOutResponse {
    /// The asset in needed for the simulation
    pub asset_in: SkipAsset,
    /// Optional spot price
    pub spot_price: Option<Decimal>,
}

/// Simulate smart swap exact asset in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateSmartSwapExactAssetInResponse {
    /// The asset out from the simulation
    pub asset_out: SkipAsset,
    /// Optional spot price
    pub spot_price: Option<Decimal>,
}

/// Skip adapter instantiate message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipAdapterInstantiateMsg {
    /// Entry point contract address
    pub entry_point_contract_address: String,
    /// Mantra pool manager address
    pub mantra_pool_manager_address: String,
}

// ============================================================================
// Legacy Types (for backward compatibility)
// ============================================================================

/// Skip operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipOperationResult {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub message: String,
    pub route: Option<serde_json::Value>,
}

/// Cross-chain swap parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainSwapParams {
    pub source_asset: Coin,
    pub target_asset_denom: String,
    pub target_chain: String,
    pub receiver: Option<String>,
    pub slippage_tolerance: Option<Decimal>,
}

/// Route simulation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSimulationParams {
    pub amount_in: Uint128,
    pub source_asset_denom: String,
    pub source_chain: String,
    pub dest_asset_denom: String,
    pub dest_chain: String,
    pub cumulative_affiliate_fee_bps: Option<String>,
}

/// Skip route statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipRouteStats {
    pub total_hops: u32,
    pub chains_involved: Vec<String>,
    pub estimated_time_seconds: Option<u64>,
    pub estimated_fees: Vec<Coin>,
    pub price_impact: Option<Decimal>,
}

/// DEX-specific types and structures

use cosmwasm_std::{Coin, Decimal, Uint128};
use mantra_dex_std::{
    fee::PoolFee,
    pool_manager::{PoolType},
};
use serde::{Deserialize, Serialize};

// Re-export commonly used DEX types
pub use mantra_dex_std::pool_manager::{
    ExecuteMsg as PoolManagerExecuteMsg, QueryMsg as PoolManagerQueryMsg,
};

/// Asset information for pool creation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    /// Native Cosmos coin
    Native(String),
    /// CW20 token
    Cw20(String),
}

pub use mantra_dex_std::farm_manager::{
    ExecuteMsg as FarmManagerExecuteMsg, QueryMsg as FarmManagerQueryMsg,
};

/// DEX operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexOperationResult {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// Pool creation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreationParams {
    pub pool_type: PoolType,
    pub asset_infos: Vec<AssetInfo>,
    pub pool_fees: PoolFee,
    pub amp_factor: Option<u64>,
}

/// Liquidity provision parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityParams {
    pub pool_identifier: String,
    pub assets: Vec<Coin>,
    pub slippage_tolerance: Option<Decimal>,
    pub max_spread: Option<Decimal>,
    pub receiver: Option<String>,
}

/// Swap parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapParams {
    pub offer_asset: Coin,
    pub ask_asset_denom: String,
    pub belief_price: Option<Decimal>,
    pub max_spread: Option<Decimal>,
    pub receiver: Option<String>,
    pub pool_identifier: Option<String>,
}

/// Farm rewards information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FarmRewards {
    pub farm_identifier: String,
    pub pending_rewards: Vec<Coin>,
    pub claimed_rewards: Vec<Coin>,
    pub position_value: Uint128,
}

/// DEX statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexStats {
    pub total_pools: u64,
    pub total_liquidity_usd: Option<Decimal>,
    pub daily_volume_usd: Option<Decimal>,
    pub active_farms: u64,
}
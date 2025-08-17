/// ClaimDrop-specific types and structures
use cosmwasm_std::{Coin, Uint128};
use serde::{Deserialize, Serialize};

/// Campaign parameters for creating a new claimdrop campaign
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignParams {
    pub owner: String,
    pub start_time: u64,
    pub end_time: u64,
    pub reward_denom: String,
    pub reward_per_allocation: Uint128,
    pub allocations: Vec<Allocation>,
    pub whitelist: Option<String>,
    pub blacklist: Option<String>,
}

/// User allocation in a campaign
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    pub user: String,
    pub allocated_amount: Uint128,
}

/// Campaign information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignInfo {
    pub address: String,
    pub owner: String,
    pub start_time: u64,
    pub end_time: u64,
    pub reward_denom: String,
    pub reward_per_allocation: Uint128,
    pub total_allocated: Uint128,
    pub total_claimed: Uint128,
    pub is_active: bool,
}

/// User rewards information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRewards {
    pub campaign_address: String,
    pub claimed: Vec<Coin>,
    pub pending: Vec<Coin>,
    pub available_to_claim: Vec<Coin>,
}

/// Campaign-specific rewards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignReward {
    pub campaign_address: String,
    pub claimed: Vec<Coin>,
    pub pending: Vec<Coin>,
    pub available_to_claim: Vec<Coin>,
}

/// Aggregated rewards across all campaigns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedRewards {
    pub total_campaigns: u32,
    pub total_claimed: Vec<Coin>,
    pub total_pending: Vec<Coin>,
    pub total_available: Vec<Coin>,
    pub campaigns: Vec<CampaignReward>,
}

/// ClaimDrop operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimdropOperationResult {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub message: String,
    pub campaign_address: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Claim parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimParams {
    pub campaign_address: String,
    pub amount: Option<Uint128>,
    pub receiver: Option<String>,
}

/// Campaign action for management operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignAction {
    CreateCampaign { params: CampaignParams },
    CloseCampaign,
    TopUpCampaign { amount: Vec<Coin> },
}

/// Blacklist action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlacklistAction {
    AddToBlacklist { addresses: Vec<String> },
    RemoveFromBlacklist { addresses: Vec<String> },
}

/// Factory query responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignsResponse {
    pub campaigns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationsResponse {
    pub allocations: Vec<Allocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRewardsResponse {
    pub rewards: Vec<UserRewards>,
}

/// Campaign statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignStats {
    pub total_campaigns: u32,
    pub active_campaigns: u32,
    pub total_allocated: Uint128,
    pub total_claimed: Uint128,
    pub unique_participants: u32,
}

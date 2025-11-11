/// ClaimDrop Factory client for creating and managing claimdrop campaigns
use crate::error::Error;
use crate::wallet::MantraWallet;
use cosmrs::rpc::{Client as RpcClient, HttpClient};
use cosmrs::tx::Fee;
use std::sync::Arc;
use tokio::sync::Mutex;

// Import local factory types
// Note: These are local types based on claimdrop-factory contract
#[derive(serde::Serialize, Clone)]
pub enum FactoryExecuteMsg {
    CreateCampaign {
        params: mantra_claimdrop_std::msg::CampaignParams,
    },
    UpdateConfig {
        claimdrop_code_id: Option<u64>,
    },
}

#[derive(serde::Serialize, Clone)]
pub enum FactoryQueryMsg {
    Allocations {
        address: String,
    },
    Campaigns {
        start_after: Option<String>,
        limit: Option<u16>,
    },
    UserRewards {
        address: String,
    },
}

#[derive(serde::Deserialize)]
pub struct FactoryCampaignsResponse {
    pub campaigns: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct FactoryAllocationsResponse {
    pub allocations: Vec<(String, cosmwasm_std::Coin)>,
}

#[derive(serde::Deserialize)]
pub struct FactoryUserRewardsResponse {
    pub rewards: Vec<FactoryCampaignRewards>,
}

#[derive(serde::Deserialize)]
pub struct FactoryCampaignRewards {
    pub campaign_address: String,
    pub campaign_type: String,
    pub claimed: Vec<cosmwasm_std::Coin>,
    pub pending: Vec<cosmwasm_std::Coin>,
    pub available_to_claim: Vec<cosmwasm_std::Coin>,
}

use super::client::ClaimdropClient;
use super::types::*;

/// Client for interacting with the ClaimDrop Factory contract
pub struct ClaimdropFactoryClient {
    rpc_client: Arc<Mutex<HttpClient>>,
    factory_address: String,
    wallet: Option<Arc<MantraWallet>>,
    claimdrop_code_id: Option<u64>,
}

impl ClaimdropFactoryClient {
    /// Create a new ClaimDrop Factory client
    pub fn new(
        rpc_client: Arc<Mutex<HttpClient>>,
        factory_address: String,
        wallet: Option<Arc<MantraWallet>>,
    ) -> Self {
        Self {
            rpc_client,
            factory_address,
            wallet,
            claimdrop_code_id: None,
        }
    }

    /// Get the factory contract address
    pub fn factory_address(&self) -> &str {
        &self.factory_address
    }

    /// Set the wallet for signing transactions
    pub fn set_wallet(&mut self, wallet: Arc<MantraWallet>) {
        self.wallet = Some(wallet);
    }

    /// Set the claimdrop contract code ID
    pub fn set_claimdrop_code_id(&mut self, code_id: u64) {
        self.claimdrop_code_id = Some(code_id);
    }

    /// Helper method to query the factory contract
    async fn query<R: serde::de::DeserializeOwned>(
        &self,
        query_msg: &FactoryQueryMsg,
    ) -> Result<R, Error> {
        use cosmos_sdk_proto::cosmwasm::wasm::v1::QuerySmartContractStateRequest;
        use prost::Message;

        let rpc_client = self.rpc_client.lock().await;
        let query = QuerySmartContractStateRequest {
            address: self.factory_address.clone(),
            query_data: serde_json::to_vec(query_msg)?,
        };

        let data = query.encode_to_vec();
        let result = rpc_client
            .abci_query(
                Some("/cosmwasm.wasm.v1.Query/SmartContractState".to_string()),
                data,
                None,
                false,
            )
            .await
            .map_err(|e| Error::Rpc(format!("ABCI query failed: {}", e)))?;

        if !result.code.is_ok() {
            return Err(Error::Contract(format!(
                "Factory query failed with code {:?}: {}",
                result.code, result.log
            )));
        }

        let response_data: R =
            serde_json::from_slice(&result.value).map_err(Error::Serialization)?;

        Ok(response_data)
    }

    /// Helper method to execute a factory contract message
    async fn execute<T: serde::Serialize>(
        &self,
        _msg: &T,
        _funds: Vec<cosmwasm_std::Coin>,
        _fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        // This is a simplified implementation - a full implementation would handle
        // transaction signing and broadcasting properly
        Ok(ClaimdropOperationResult {
            success: false,
            tx_hash: None,
            message: "Factory transaction execution not yet fully implemented".to_string(),
            campaign_address: None,
            data: None,
        })
    }

    // ============ Query Methods ============

    /// Query all campaigns created by the factory
    pub async fn query_campaigns(
        &self,
        start_after: Option<&str>,
        limit: Option<u16>,
    ) -> Result<CampaignsResponse, Error> {
        let query_msg = FactoryQueryMsg::Campaigns {
            start_after: start_after.map(|s| s.to_string()),
            limit,
        };

        let response: FactoryCampaignsResponse = self.query(&query_msg).await?;

        // Convert to our CampaignsResponse type
        Ok(CampaignsResponse {
            campaigns: response.campaigns,
        })
    }

    /// Query user allocations across all campaigns
    pub async fn query_user_allocations(
        &self,
        address: &str,
    ) -> Result<AllocationsResponse, Error> {
        let query_msg = FactoryQueryMsg::Allocations {
            address: address.to_string(),
        };

        let response: FactoryAllocationsResponse = self.query(&query_msg).await?;

        // Convert to our AllocationsResponse type
        let allocations = response
            .allocations
            .into_iter()
            .map(|(addr, coin)| Allocation {
                user: addr,
                allocated_amount: coin.amount,
            })
            .collect();

        Ok(AllocationsResponse { allocations })
    }

    /// Query user rewards across all campaigns
    pub async fn query_user_rewards(&self, address: &str) -> Result<AggregatedRewards, Error> {
        let query_msg = FactoryQueryMsg::UserRewards {
            address: address.to_string(),
        };

        let response: FactoryUserRewardsResponse = self.query(&query_msg).await?;

        // Convert to our AggregatedRewards type
        let campaign_rewards: Vec<CampaignReward> = response
            .rewards
            .into_iter()
            .map(|r| CampaignReward {
                campaign_address: r.campaign_address,
                campaign_type: Some(r.campaign_type),
                claimed: r.claimed,
                pending: r.pending,
                available_to_claim: r.available_to_claim,
            })
            .collect();

        Ok(AggregatedRewards {
            total_campaigns: campaign_rewards.len() as u32,
            total_claimed: campaign_rewards
                .iter()
                .flat_map(|r| r.claimed.iter())
                .cloned()
                .collect(),
            total_pending: campaign_rewards
                .iter()
                .flat_map(|r| r.pending.iter())
                .cloned()
                .collect(),
            total_available: campaign_rewards
                .iter()
                .flat_map(|r| r.available_to_claim.iter())
                .cloned()
                .collect(),
            campaigns: campaign_rewards,
        })
    }

    // ============ Execute Methods ============

    /// Create a new claimdrop campaign
    pub async fn create_campaign(
        &self,
        params: mantra_claimdrop_std::msg::CampaignParams,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        self.wallet.as_ref().ok_or_else(|| Error::WalletNotSet)?;

        let msg = FactoryExecuteMsg::CreateCampaign { params };

        // Execute the create_campaign message on the factory
        self.execute(&msg, vec![], fee).await
    }

    /// Update the factory configuration (admin only)
    pub async fn update_config(
        &self,
        claimdrop_code_id: Option<u64>,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        self.wallet.as_ref().ok_or_else(|| Error::WalletNotSet)?;

        let msg = FactoryExecuteMsg::UpdateConfig { claimdrop_code_id };

        // Execute the update_config message on the factory
        self.execute(&msg, vec![], fee).await
    }

    /// Create a ClaimdropClient for a specific campaign
    pub fn campaign_client(&self, campaign_address: String) -> ClaimdropClient {
        ClaimdropClient::new(
            self.rpc_client.clone(),
            campaign_address,
            self.wallet.clone(),
        )
    }

    /// Get aggregated statistics across all campaigns
    pub async fn get_campaign_stats(&self) -> Result<CampaignStats, Error> {
        // Query all campaigns first
        let campaigns_response = self.query_campaigns(None, None).await?;

        let total_campaigns = campaigns_response.campaigns.len() as u32;
        let total_allocated = cosmwasm_std::Uint128::zero();
        let total_claimed = cosmwasm_std::Uint128::zero();

        // For now, return basic stats based on campaign count
        // In a full implementation, we would query each campaign for detailed stats
        Ok(CampaignStats {
            total_campaigns,
            active_campaigns: total_campaigns, // Assume all are active for now
            total_allocated,
            total_claimed,
            unique_participants: 0,
        })
    }

    /// Helper method to claim from multiple campaigns at once
    pub async fn claim_from_multiple_campaigns(
        &self,
        campaign_addresses: Vec<String>,
        receiver: Option<String>,
        fee: Fee,
    ) -> Result<Vec<ClaimdropOperationResult>, Error> {
        let mut results = Vec::new();

        for campaign_address in campaign_addresses {
            let client = self.campaign_client(campaign_address.clone());
            match client.claim(None, receiver.clone(), fee.clone()).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    results.push(ClaimdropOperationResult {
                        success: false,
                        tx_hash: None,
                        message: format!("Failed to claim from {}: {}", campaign_address, e),
                        campaign_address: Some(campaign_address),
                        data: None,
                    });
                }
            }
        }

        Ok(results)
    }
}

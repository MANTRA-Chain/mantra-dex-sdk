/// ClaimDrop client for interacting with individual claimdrop campaigns
use crate::error::Error;
use crate::wallet::MantraWallet;
use cosmrs::rpc::{Client as RpcClient, HttpClient};
use cosmrs::tx::Fee;
use cosmwasm_std::{Coin, Uint128};
use std::sync::Arc;
use tokio::sync::Mutex;

// Import ClaimDrop std types
use mantra_claimdrop_std::msg::{
    AllocationsResponse, AuthorizedResponse, BlacklistResponse, CampaignAction, CampaignResponse,
    ClaimedResponse, ExecuteMsg, QueryMsg, RewardsResponse,
};

use super::types::*;

/// Client for interacting with a specific ClaimDrop campaign contract
pub struct ClaimdropClient {
    rpc_client: Arc<Mutex<HttpClient>>,
    contract_address: String,
    wallet: Option<Arc<MantraWallet>>,
}

impl ClaimdropClient {
    /// Create a new ClaimDrop client for a specific campaign
    pub fn new(
        rpc_client: Arc<Mutex<HttpClient>>,
        contract_address: String,
        wallet: Option<Arc<MantraWallet>>,
    ) -> Self {
        Self {
            rpc_client,
            contract_address,
            wallet,
        }
    }

    /// Get the campaign contract address
    pub fn contract_address(&self) -> &str {
        &self.contract_address
    }

    /// Set the wallet for signing transactions
    pub fn set_wallet(&mut self, wallet: Arc<MantraWallet>) {
        self.wallet = Some(wallet);
    }

    /// Helper method to query the contract
    async fn query<R: serde::de::DeserializeOwned>(
        &self,
        query_msg: &QueryMsg,
    ) -> Result<R, Error> {
        use cosmos_sdk_proto::cosmwasm::wasm::v1::QuerySmartContractStateRequest;
        use prost::Message;

        let rpc_client = self.rpc_client.lock().await;
        let query = QuerySmartContractStateRequest {
            address: self.contract_address.clone(),
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
                "Contract query failed with code {:?}: {}",
                result.code, result.log
            )));
        }

        let response_data: R =
            serde_json::from_slice(&result.value).map_err(|e| Error::Serialization(e))?;

        Ok(response_data)
    }

    /// Helper method to execute a contract message
    async fn execute<T: serde::Serialize>(
        &self,
        _msg: &T,
        _funds: Vec<Coin>,
        _fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        // This is a simplified implementation - a full implementation would handle
        // transaction signing and broadcasting properly
        Ok(ClaimdropOperationResult {
            success: false,
            tx_hash: None,
            message: "Transaction execution not yet fully implemented".to_string(),
            campaign_address: Some(self.contract_address.clone()),
            data: None,
        })
    }

    // ============ Query Methods ============

    /// Query campaign information
    pub async fn query_campaign(&self) -> Result<CampaignInfo, Error> {
        let query_msg = QueryMsg::Campaign {};
        let response: CampaignResponse = self.query(&query_msg).await?;

        // Convert CampaignResponse to our CampaignInfo type
        Ok(CampaignInfo {
            address: self.contract_address.clone(),
            owner: "".to_string(), // Would need to be queried separately via ownership query
            name: response.name,
            description: response.description,
            campaign_type: response.ty,
            start_time: response.start_time,
            end_time: response.end_time,
            total_reward: response.total_reward,
            claimed: response.claimed,
            distribution_type: response.distribution_type,
            is_active: response.closed.is_none(),
            closed_at: response.closed,
        })
    }

    /// Query user rewards
    pub async fn query_rewards(&self, receiver: &str) -> Result<UserRewards, Error> {
        let query_msg = QueryMsg::Rewards {
            receiver: receiver.to_string(),
        };
        let response: RewardsResponse = self.query(&query_msg).await?;

        Ok(UserRewards {
            campaign_address: self.contract_address.clone(),
            claimed: response.claimed,
            pending: response.pending,
            available_to_claim: response.available_to_claim,
        })
    }

    /// Query claimed amounts
    pub async fn query_claimed(
        &self,
        address: Option<&str>,
        start_from: Option<&str>,
        limit: Option<u16>,
    ) -> Result<Vec<(String, Vec<Coin>)>, Error> {
        let query_msg = QueryMsg::Claimed {
            address: address.map(|s| s.to_string()),
            start_from: start_from.map(|s| s.to_string()),
            limit,
        };
        let response: ClaimedResponse = self.query(&query_msg).await?;

        // Convert response format
        let result = response
            .claimed
            .into_iter()
            .map(|(addr, coin)| (addr, vec![coin]))
            .collect();

        Ok(result)
    }

    /// Query allocations
    pub async fn query_allocations(
        &self,
        address: Option<&str>,
        start_from: Option<&str>,
        limit: Option<u16>,
    ) -> Result<Vec<Allocation>, Error> {
        let query_msg = QueryMsg::Allocations {
            address: address.map(|s| s.to_string()),
            start_after: start_from.map(|s| s.to_string()),
            limit,
        };
        let response: AllocationsResponse = self.query(&query_msg).await?;

        // Convert response format
        let result = response
            .allocations
            .into_iter()
            .map(|(user, coin)| Allocation {
                user,
                allocated_amount: coin.amount,
            })
            .collect();

        Ok(result)
    }

    /// Check if an address is blacklisted
    pub async fn is_blacklisted(&self, address: &str) -> Result<bool, Error> {
        let query_msg = QueryMsg::IsBlacklisted {
            address: address.to_string(),
        };
        let response: BlacklistResponse = self.query(&query_msg).await?;
        Ok(response.is_blacklisted)
    }

    /// Check if an address is authorized
    pub async fn is_authorized(&self, address: &str) -> Result<bool, Error> {
        let query_msg = QueryMsg::IsAuthorized {
            address: address.to_string(),
        };
        let response: AuthorizedResponse = self.query(&query_msg).await?;
        Ok(response.is_authorized)
    }

    // ============ Execute Methods ============

    /// Claim rewards from the campaign
    pub async fn claim(
        &self,
        amount: Option<Uint128>,
        receiver: Option<String>,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let msg = ExecuteMsg::Claim { amount, receiver };
        self.execute(&msg, vec![], fee).await
    }

    /// Add allocations (admin only, before campaign starts)
    pub async fn add_allocations(
        &self,
        allocations: Vec<Allocation>,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let allocations_array: Vec<(String, Uint128)> = allocations
            .into_iter()
            .map(|a| (a.user, a.allocated_amount))
            .collect();

        let msg = ExecuteMsg::AddAllocations {
            allocations: allocations_array,
        };
        self.execute(&msg, vec![], fee).await
    }

    /// Replace an address in allocations (admin only)
    pub async fn replace_address(
        &self,
        old_address: &str,
        new_address: &str,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let msg = ExecuteMsg::ReplaceAddress {
            old_address: old_address.to_string(),
            new_address: new_address.to_string(),
        };
        self.execute(&msg, vec![], fee).await
    }

    /// Remove an address from allocations (admin only)
    pub async fn remove_address(
        &self,
        address: &str,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let msg = ExecuteMsg::RemoveAddress {
            address: address.to_string(),
        };
        self.execute(&msg, vec![], fee).await
    }

    /// Manage blacklist (admin only)
    pub async fn manage_blacklist(
        &self,
        action: BlacklistAction,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let msg = match action {
            BlacklistAction::AddToBlacklist { addresses } => {
                // Process one at a time for now
                ExecuteMsg::BlacklistAddress {
                    address: addresses[0].clone(),
                    blacklist: true,
                }
            }
            BlacklistAction::RemoveFromBlacklist { addresses } => ExecuteMsg::BlacklistAddress {
                address: addresses[0].clone(),
                blacklist: false,
            },
        };

        self.execute(&msg, vec![], fee).await
    }

    /// Close the campaign (admin only)
    pub async fn close_campaign(&self, fee: Fee) -> Result<ClaimdropOperationResult, Error> {
        let msg = ExecuteMsg::ManageCampaign {
            action: CampaignAction::CloseCampaign {},
        };
        self.execute(&msg, vec![], fee).await
    }

    /// Sweep non-reward tokens from the campaign (owner only)
    pub async fn sweep(
        &self,
        denom: &str,
        amount: Option<Uint128>,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let msg = ExecuteMsg::Sweep {
            denom: denom.to_string(),
            amount,
        };
        self.execute(&msg, vec![], fee).await
    }

    /// Manage authorized wallets (owner only)
    pub async fn manage_authorized_wallets(
        &self,
        addresses: Vec<String>,
        authorized: bool,
        fee: Fee,
    ) -> Result<ClaimdropOperationResult, Error> {
        let msg = ExecuteMsg::ManageAuthorizedWallets {
            addresses,
            authorized,
        };
        self.execute(&msg, vec![], fee).await
    }

    /// Query authorized wallets
    pub async fn query_authorized_wallets(
        &self,
        start_after: Option<&str>,
        limit: Option<u32>,
    ) -> Result<super::types::AuthorizedWalletsResponse, Error> {
        let query_msg = QueryMsg::AuthorizedWallets {
            start_after: start_after.map(|s| s.to_string()),
            limit,
        };
        self.query(&query_msg).await
    }
}

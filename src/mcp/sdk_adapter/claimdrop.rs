//! ClaimDrop protocol methods

use super::*;

impl McpSdkAdapter {
    // ClaimDrop Protocol Methods
    // =============================================================================

    /// Create a new claimdrop campaign through the factory
    pub async fn claimdrop_create_campaign(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Creating ClaimDrop campaign with args: {:?}",
            args
        );

        // Parse required parameters
        let factory_address = args
            .get("factory_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("factory_address is required".to_string())
            })?;

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("name is required".to_string()))?;

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("description is required".to_string())
            })?;

        let campaign_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("type is required".to_string()))?;

        let start_time = args
            .get("start_time")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("start_time is required".to_string())
            })?;

        let end_time = args
            .get("end_time")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| McpServerError::InvalidArguments("end_time is required".to_string()))?;

        let reward_denom = args
            .get("reward_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("reward_denom is required".to_string())
            })?;

        let total_reward_str = args
            .get("total_reward")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("total_reward is required".to_string())
            })?;

        let total_reward = Uint128::from_str(total_reward_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid total_reward: {}", e))
        })?;

        // Parse distribution_type array
        let distribution_type = args
            .get("distribution_type")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("distribution_type is required".to_string())
            })?;

        let mut parsed_distributions = Vec::new();
        for dist in distribution_type {
            let dist_type = dist.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
                McpServerError::InvalidArguments("distribution_type.type is required".to_string())
            })?;

            match dist_type {
                "lump_sum" => {
                    let percentage =
                        dist.get("percentage")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                McpServerError::InvalidArguments(
                                    "distribution percentage is required".to_string(),
                                )
                            })?;
                    let percentage_decimal = Decimal::from_str(percentage).map_err(|e| {
                        McpServerError::InvalidArguments(format!("Invalid percentage: {}", e))
                    })?;
                    let start_time =
                        dist.get("start_time")
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                McpServerError::InvalidArguments(
                                    "distribution start_time is required".to_string(),
                                )
                            })?;

                    parsed_distributions.push(
                        mantra_claimdrop_std::msg::DistributionType::LumpSum {
                            percentage: percentage_decimal,
                            start_time,
                        },
                    );
                }
                "linear_vesting" => {
                    let percentage =
                        dist.get("percentage")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                McpServerError::InvalidArguments(
                                    "distribution percentage is required".to_string(),
                                )
                            })?;
                    let percentage_decimal = Decimal::from_str(percentage).map_err(|e| {
                        McpServerError::InvalidArguments(format!("Invalid percentage: {}", e))
                    })?;
                    let start_time =
                        dist.get("start_time")
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                McpServerError::InvalidArguments(
                                    "distribution start_time is required".to_string(),
                                )
                            })?;
                    let end_time =
                        dist.get("end_time")
                            .and_then(|v| v.as_u64())
                            .ok_or_else(|| {
                                McpServerError::InvalidArguments(
                                    "distribution end_time is required".to_string(),
                                )
                            })?;
                    let cliff_duration = dist.get("cliff_duration").and_then(|v| v.as_u64());

                    parsed_distributions.push(
                        mantra_claimdrop_std::msg::DistributionType::LinearVesting {
                            percentage: percentage_decimal,
                            start_time,
                            end_time,
                            cliff_duration,
                        },
                    );
                }
                _ => {
                    return Err(McpServerError::InvalidArguments(format!(
                        "Invalid distribution type: {}",
                        dist_type
                    )));
                }
            }
        }

        // Get network config and active wallet
        let network_config = self.get_default_network_config().await?;
        let wallet = self.get_active_wallet_with_validation().await?;

        // Create MantraClient with wallet
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet)))
            .await
            .map_err(McpServerError::Sdk)?;

        // Create campaign parameters
        let campaign_params = mantra_claimdrop_std::msg::CampaignParams {
            name: name.to_string(),
            description: description.to_string(),
            ty: campaign_type.to_string(),
            total_reward: cosmwasm_std::Coin {
                denom: reward_denom.to_string(),
                amount: total_reward,
            },
            distribution_type: parsed_distributions,
            start_time,
            end_time,
        };

        // Get factory client and create campaign
        let factory_client = client.claimdrop_factory(factory_address.to_string());

        // Use default fee for now
        let fee = cosmrs::tx::Fee::from_amount_and_gas(
            cosmrs::Coin {
                denom: cosmrs::Denom::from_str("uom").unwrap(),
                amount: 5000u64.into(),
            },
            200_000u64,
        );

        let result = factory_client
            .create_campaign(campaign_params, fee)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "create_campaign",
            "factory_address": factory_address,
            "result": result,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Claim rewards from a claimdrop campaign
    pub async fn claimdrop_claim(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Claiming from ClaimDrop campaign with args: {:?}",
            args
        );

        // Parse required parameters
        let campaign_address = args
            .get("campaign_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("campaign_address is required".to_string())
            })?;

        // Parse optional parameters
        let amount = args
            .get("amount")
            .and_then(|v| v.as_str())
            .and_then(|s| Uint128::from_str(s).ok());

        let receiver = args
            .get("receiver")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get network config and active wallet
        let network_config = self.get_default_network_config().await?;
        let wallet = self.get_active_wallet_with_validation().await?;

        // Create MantraClient with wallet
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet)))
            .await
            .map_err(McpServerError::Sdk)?;

        // Get ClaimDrop client for the campaign
        let claimdrop_client = client.claimdrop_campaign(campaign_address.to_string());

        // Use default fee
        let fee = cosmrs::tx::Fee::from_amount_and_gas(
            cosmrs::Coin {
                denom: cosmrs::Denom::from_str("uom").unwrap(),
                amount: 5000u64.into(),
            },
            150_000u64,
        );

        let result = claimdrop_client
            .claim(amount, receiver, fee)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "claim",
            "campaign_address": campaign_address,
            "result": result,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Query user rewards from a claimdrop campaign
    pub async fn claimdrop_query_rewards(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Querying ClaimDrop rewards with args: {:?}",
            args
        );

        // Parse required parameters
        let campaign_address = args
            .get("campaign_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("campaign_address is required".to_string())
            })?;

        let receiver_address = args
            .get("receiver")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("receiver is required".to_string()))?;

        // Get network config and create MantraClient
        let network_config = self.get_default_network_config().await?;
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get ClaimDrop client for the campaign
        let claimdrop_client = client.claimdrop_campaign(campaign_address.to_string());

        let rewards = claimdrop_client
            .query_rewards(receiver_address)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "query_rewards",
            "campaign_address": campaign_address,
            "receiver": receiver_address,
            "rewards": rewards,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Query campaigns from factory
    pub async fn claimdrop_query_campaigns(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Querying ClaimDrop campaigns with args: {:?}",
            args
        );

        // Parse required parameters
        let factory_address = args
            .get("factory_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("factory_address is required".to_string())
            })?;

        // Parse optional parameters
        let start_after = args.get("start_after").and_then(|v| v.as_str());

        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|l| l as u16);

        // Get network config and create MantraClient
        let network_config = self.get_default_network_config().await?;
        let client = MantraClient::new(network_config.clone(), None)
            .await
            .map_err(McpServerError::Sdk)?;

        // Get factory client
        let factory_client = client.claimdrop_factory(factory_address.to_string());

        let campaigns = factory_client
            .query_campaigns(start_after, limit)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "query_campaigns",
            "factory_address": factory_address,
            "campaigns": campaigns,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Add allocations to a claimdrop campaign
    pub async fn claimdrop_add_allocations(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Adding ClaimDrop allocations with args: {:?}",
            args
        );

        // Parse required parameters
        let campaign_address = args
            .get("campaign_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("campaign_address is required".to_string())
            })?;

        let allocations_array = args
            .get("allocations")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("allocations is required".to_string())
            })?;

        // Parse allocations
        let mut allocations = Vec::new();
        for allocation in allocations_array {
            let user = allocation
                .get("user")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("allocation.user is required".to_string())
                })?;
            let amount_str = allocation
                .get("allocated_amount")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments(
                        "allocation.allocated_amount is required".to_string(),
                    )
                })?;

            let amount = Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid allocated_amount: {}", e))
            })?;

            allocations.push(crate::protocols::claimdrop::types::Allocation {
                user: user.to_string(),
                allocated_amount: amount,
            });
        }

        // Get network config and active wallet
        let network_config = self.get_default_network_config().await?;
        let wallet = self.get_active_wallet_with_validation().await?;

        // Create MantraClient with wallet
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet)))
            .await
            .map_err(McpServerError::Sdk)?;

        // Get ClaimDrop client for the campaign
        let claimdrop_client = client.claimdrop_campaign(campaign_address.to_string());

        // Use default fee
        let fee = cosmrs::tx::Fee::from_amount_and_gas(
            cosmrs::Coin {
                denom: cosmrs::Denom::from_str("uom").unwrap(),
                amount: 5000u64.into(),
            },
            200_000u64,
        );

        let result = claimdrop_client
            .add_allocations(allocations, fee)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "add_allocations",
            "campaign_address": campaign_address,
            "result": result,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    // =============================================================================
}

use std::str::FromStr;
use std::sync::Arc;

use base64::{engine::general_purpose, Engine};
use chrono;
use cosmos_sdk_proto::{
    cosmos::auth::v1beta1::{BaseAccount, QueryAccountRequest, QueryAccountResponse},
    cosmos::bank::v1beta1::{QueryAllBalancesRequest, QueryAllBalancesResponse},
    cosmwasm::wasm::v1::QuerySmartContractStateResponse,
};
use cosmrs::{
    proto::{
        cosmos::base::{abci::v1beta1::TxResponse, v1beta1::Coin as CosmosCoin},
        cosmwasm::wasm::v1::{MsgExecuteContract, QuerySmartContractStateRequest},
    },
    rpc::{Client as RpcClient, HttpClient},
    tendermint::{chain::Id, Hash},
    tx::{Body, MessageExt, SignDoc, SignerInfo},
    Any,
};
use cosmwasm_std::{Coin, Decimal, Uint128};
use hex;
use mantra_dex_std::pool_manager::{
    self, PoolInfoResponse, PoolsResponse, SimulationResponse, SwapOperation,
};
use prost::Message;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::config::MantraNetworkConfig;
use crate::error::Error;
use crate::wallet::MantraWallet;

/// Pool status enum for validation
#[derive(Debug, Clone, PartialEq)]
pub enum PoolStatus {
    /// Pool is available for all operations (swaps, deposits, withdrawals)
    Available,
    /// Pool is disabled and cannot be used for operations
    Disabled,
}

impl PoolStatus {
    /// Check if the pool is available for operations
    ///
    /// # Returns
    ///
    /// `true` if the pool status is `Available`, `false` otherwise
    pub fn is_available(&self) -> bool {
        matches!(self, PoolStatus::Available)
    }
}

/// Mantra DEX client for interacting with the network
///
/// This client provides methods to interact with the Mantra DEX v3.0.0,
/// including pool operations, swapping, liquidity provision, and rewards management.
#[derive(Debug)]
pub struct MantraDexClient {
    /// RPC client for the Mantra network
    rpc_client: Arc<Mutex<HttpClient>>,
    /// Network configuration
    config: MantraNetworkConfig,
    /// Wallet for signing transactions
    wallet: Option<MantraWallet>,
}

impl MantraDexClient {
    /// Create a new client with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Network configuration containing RPC endpoints and contract addresses
    ///
    /// # Returns
    ///
    /// A new `MantraDexClient` instance ready for use
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC client cannot be created
    pub async fn new(config: MantraNetworkConfig) -> Result<Self, Error> {
        let rpc_client = HttpClient::new(config.rpc_url.as_str())
            .map_err(|e| Error::Rpc(format!("Failed to create RPC client: {}", e)))?;

        Ok(Self {
            rpc_client: Arc::new(Mutex::new(rpc_client)),
            config,
            wallet: None,
        })
    }

    /// Set the wallet for signing transactions
    ///
    /// # Arguments
    ///
    /// * `wallet` - The wallet to use for signing transactions
    ///
    /// # Returns
    ///
    /// The client instance with the wallet configured
    pub fn with_wallet(mut self, wallet: MantraWallet) -> Self {
        self.wallet = Some(wallet);
        self
    }

    /// Set the wallet for signing transactions
    ///
    /// # Arguments
    ///
    /// * `wallet` - The wallet to use for signing transactions
    pub fn set_wallet(&mut self, wallet: MantraWallet) {
        self.wallet = Some(wallet);
    }

    /// Get the wallet if available
    pub fn wallet(&self) -> Result<&MantraWallet, Error> {
        self.wallet
            .as_ref()
            .ok_or_else(|| Error::Wallet("No wallet configured".to_string()))
    }

    /// Get the wallet address if wallet is configured
    pub async fn get_wallet_address(&self) -> Option<String> {
        match &self.wallet {
            Some(wallet) => wallet.address().ok().map(|addr| addr.to_string()),
            None => None,
        }
    }

    /// Get balance for a specific denom for the configured wallet
    pub async fn get_balance(&self, denom: &str) -> Result<cosmwasm_std::Coin, Error> {
        // Get wallet balances and find the specific denom
        let balances = self.get_balances().await?;

        // Find the balance for the specific denomination
        for balance in balances {
            if balance.denom == denom {
                return Ok(balance);
            }
        }

        // If not found, return zero balance
        Ok(cosmwasm_std::Coin {
            denom: denom.to_string(),
            amount: cosmwasm_std::Uint128::zero(),
        })
    }

    /// Get last block height
    pub async fn get_last_block_height(&self) -> Result<u64, Error> {
        let rpc_client = self.rpc_client.lock().await;
        let height = rpc_client
            .latest_block()
            .await
            .map_err(|e| Error::Rpc(format!("Failed to get last block height: {}", e)))?;
        Ok(height.block.header.height.value())
    }

    /// Get the Wallet balances
    pub async fn get_balances(&self) -> Result<Vec<Coin>, Error> {
        let wallet = self.wallet()?;
        let address = wallet.address().unwrap().to_string();
        self.get_balances_for_address(&address).await
    }

    /// Get balances for a specific address
    pub async fn get_balances_for_address(&self, address: &str) -> Result<Vec<Coin>, Error> {
        let rpc_client = self.rpc_client.lock().await;

        // Create a request to get all balances
        let request = QueryAllBalancesRequest {
            address: address.to_string(),
            pagination: None,
            resolve_denom: false,
        };

        // Encode the request to protobuf
        let encoded_request = request.encode_to_vec();

        // Execute the query
        let response = rpc_client
            .abci_query(
                Some("/cosmos.bank.v1beta1.Query/AllBalances".to_string()),
                encoded_request,
                None,
                false,
            )
            .await
            .map_err(|e| Error::Rpc(format!("Failed to get balances: {}", e)))?;

        if !response.code.is_ok() {
            return Err(Error::Rpc(format!("Query failed: {}", response.log)));
        }

        // Decode the response
        let balances_response = QueryAllBalancesResponse::decode(response.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode balances response: {}", e)))?;

        // Convert from cosmos proto coins to cosmwasm coins
        let balances = balances_response
            .balances
            .into_iter()
            .map(|coin| Coin {
                denom: coin.denom,
                amount: Uint128::from_str(&coin.amount).unwrap_or_default(),
            })
            .collect();

        Ok(balances)
    }

    /// Get the network configuration
    pub fn config(&self) -> &MantraNetworkConfig {
        &self.config
    }

    /// Query a transaction by hash
    pub async fn query_transaction(&self, tx_hash: &str) -> Result<serde_json::Value, Error> {
        let rpc_client = self.rpc_client.lock().await;

        // Parse the transaction hash
        let hash = Hash::from_hex_upper(
            cosmrs::tendermint::hash::Algorithm::Sha256,
            tx_hash.trim_start_matches("0x"),
        )
        .map_err(|e| Error::Other(format!("Invalid transaction hash: {}", e)))?;

        // Query the transaction
        let tx_response = rpc_client
            .tx(hash, false)
            .await
            .map_err(|e| Error::Rpc(format!("Failed to query transaction: {}", e)))?;

        // Create a simplified response structure
        let result = serde_json::json!({
            "hash": tx_hash,
            "height": tx_response.height.value(),
            "index": tx_response.index,
            "tx_result": {
                "code": tx_response.tx_result.code.value(),
                "data": hex::encode(&tx_response.tx_result.data),
                "log": tx_response.tx_result.log,
                "info": tx_response.tx_result.info,
                "gas_wanted": tx_response.tx_result.gas_wanted,
                "gas_used": tx_response.tx_result.gas_used,
                "events": tx_response.tx_result.events.iter().map(|event| {
                    serde_json::json!({
                        "type": event.kind,
                        "attributes": event.attributes.iter().map(|attr| {
                            serde_json::json!({
                                "key": attr.key_str().unwrap_or(""),
                                "value": attr.value_str().unwrap_or("")
                            })
                        }).collect::<Vec<_>>()
                    })
                }).collect::<Vec<_>>(),
                "codespace": tx_response.tx_result.codespace
            },
            "tx_raw": {
                "size": tx_response.tx.len(),
                "note": "Full transaction parsing not implemented - use specialized tools for detailed analysis"
            }
        });

        Ok(result)
    }

    /// Query a smart contract
    pub async fn query<Q: serde::Serialize + Clone, R: DeserializeOwned>(
        &self,
        contract_addr: &str,
        query_msg: &Q,
    ) -> Result<R, Error> {
        let rpc_client = self.rpc_client.lock().await;
        let query = QuerySmartContractStateRequest {
            address: contract_addr.to_string(),
            query_data: serde_json::to_vec(query_msg)?,
        };

        // Now that we're using the same Prost version as cosmos-sdk-proto,
        // we can use the Message trait directly
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
                "Contract query failed: {}",
                String::from_utf8_lossy(result.log.as_bytes())
            )));
        }
        let resp: QuerySmartContractStateResponse =
            QuerySmartContractStateResponse::decode(result.value.as_slice())
                .map_err(|e| Error::Rpc(format!("Failed to decode query response: {}", e)))?;
        serde_json::from_slice::<R>(resp.data.as_slice()).map_err(Into::into)
    }

    /// Execute a contract message
    pub async fn execute<T: serde::Serialize>(
        &self,
        contract_addr: &str,
        msg: &T,
        funds: Vec<Coin>,
    ) -> Result<TxResponse, Error> {
        let wallet = self.wallet()?;
        let sender = wallet.address().unwrap().to_string();

        let cosmos_coins = funds
            .iter()
            .map(|c| CosmosCoin {
                denom: c.denom.clone(),
                amount: c.amount.to_string(),
            })
            .collect();
        let execute_msg = MsgExecuteContract {
            sender: sender.clone(),
            contract: contract_addr.to_string(),
            msg: serde_json::to_vec(msg)?,
            funds: cosmos_coins,
        };

        self.broadcast_tx(vec![Any {
            type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
            value: execute_msg.to_bytes().unwrap(),
        }])
        .await
    }

    /// Broadcast a transaction to the network
    async fn broadcast_tx(&self, msgs: Vec<Any>) -> Result<TxResponse, Error> {
        let _height = self.get_last_block_height().await?;
        let wallet = self.wallet()?;
        let rpc_client = self.rpc_client.lock().await;

        let tx_body = Body::new(msgs, String::new(), 0u32);

        // Get account info for signing
        let addr = wallet.address().unwrap().to_string();

        // Create request using the proper protobuf type
        let request = QueryAccountRequest { address: addr };

        // Encode the request to protobuf
        let encoded_request = request.encode_to_vec();

        let account_info = rpc_client
            .abci_query(
                Some("/cosmos.auth.v1beta1.Query/Account".to_string()),
                encoded_request,
                None,
                false,
            )
            .await
            .map_err(|e| Error::Rpc(format!("Failed to get account info: {}", e)))?;

        if !account_info.code.is_ok() {
            return Err(Error::Rpc(format!(
                "Account query failed: {}",
                account_info.log
            )));
        }

        // Decode the response using the correct protobuf type
        let account_response = QueryAccountResponse::decode(account_info.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode account response: {}", e)))?;

        // Extract the account data - account.value contains a serialized BaseAccount
        let account_any = account_response.account.unwrap();

        // Decode the BaseAccount from the Any object's value
        let base_account = BaseAccount::decode(account_any.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode BaseAccount: {}", e)))?;

        let account_number = base_account.account_number;
        let sequence = base_account.sequence;
        // Create the fee
        let fee = wallet.create_default_fee(2_000_000)?;

        // Create signer info with sequence number
        let signer_info = SignerInfo::single_direct(Some(wallet.public_key()), sequence);

        // Create auth info with fee
        let auth_info = signer_info.auth_info(fee);

        let chain_id = Id::try_from(self.config.chain_id.as_str())
            .map_err(|e| Error::Tx(format!("Invalid chain ID: {}", e)))?;

        let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, account_number)
            .map_err(|e| Error::Tx(format!("Failed to create sign doc: {}", e)))?;

        // Sign the transaction
        let tx_raw = sign_doc
            .sign(wallet.signing_key())
            .map_err(|e| Error::Tx(format!("Failed to sign transaction: {}", e)))?;
        // Broadcast the transaction
        let response = rpc_client
            .broadcast_tx_commit(tx_raw.to_bytes().unwrap())
            .await
            .map_err(|e| Error::Rpc(format!("Failed to broadcast transaction: {}", e)))?;
        // Get the transaction response
        let tx_response = if response.check_tx.code.is_err() {
            return Err(Error::Contract(format!(
                "Transaction check failed: {}",
                response.check_tx.log
            )));
        } else if response.tx_result.code.is_err() {
            return Err(Error::Contract(format!(
                "Transaction execution failed: {}",
                response.tx_result.log
            )));
        } else {
            // Query the tx
            let tx_result = rpc_client
                .tx(
                    Hash::try_from(response.hash.as_bytes().to_vec())
                        .map_err(|e| Error::Tx(format!("Invalid tx hash: {}", e)))?,
                    false,
                )
                .await
                .map_err(|e| Error::Rpc(format!("Failed to get transaction: {}", e)))?;

            // Transform the response to TxResponse
            TxResponse {
                height: tx_result.height.value() as i64,
                txhash: hex::encode(response.hash.as_bytes()),
                codespace: "".to_string(),
                code: 0,
                data: general_purpose::STANDARD.encode(tx_result.tx_result.data),
                raw_log: tx_result.tx_result.log.to_string(),
                logs: vec![],
                info: "".to_string(),
                gas_wanted: tx_result.tx_result.gas_wanted,
                gas_used: tx_result.tx_result.gas_used,
                tx: None,
                timestamp: "".to_string(),
                events: vec![],
            }
        };

        Ok(tx_response)
    }

    /// Query asset decimals for a specific asset in a pool
    ///
    /// This method uses the pool manager's AssetDecimals query to get accurate
    /// decimal information for an asset within a specific pool context.
    ///
    /// # Arguments
    ///
    /// * `pool_identifier` - The pool identifier to query from
    /// * `denom` - The asset denomination to get decimals for
    ///
    /// # Returns
    ///
    /// The number of decimal places for the asset
    ///
    /// # Errors
    ///
    /// Returns error if the query fails or the asset is not found in the pool
    pub async fn query_asset_decimals(
        &self,
        pool_identifier: &str,
        denom: &str,
    ) -> Result<u8, Error> {
        let query = pool_manager::QueryMsg::AssetDecimals {
            pool_identifier: pool_identifier.to_string(),
            denom: denom.to_string(),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();

        // Query and expect AssetDecimalsResponse
        let response: serde_json::Value = self.query(&pool_manager_address, &query).await?;

        // Extract decimals from response
        let decimals = response
            .get("decimals")
            .and_then(|v| v.as_u64())
            .map(|d| d as u8)
            .ok_or_else(|| Error::Other("Invalid asset decimals response format".to_string()))?;

        Ok(decimals)
    }

    /// Get asset decimals from multiple pools for comprehensive mapping
    ///
    /// This method queries all available pools and builds a comprehensive map of
    /// asset denominations to their decimal places using the official AssetDecimals query.
    ///
    /// # Returns
    ///
    /// A HashMap mapping asset denominations to their decimal places
    ///
    /// # Errors
    ///
    /// Returns error if pools cannot be queried
    pub async fn get_asset_decimals_from_pools(
        &self,
    ) -> Result<std::collections::HashMap<String, u8>, Error> {
        use std::collections::HashMap;

        let pools = self.get_pools(Some(100)).await?; // Query up to 100 pools
        let mut asset_decimals_map = HashMap::new();

        for pool in pools {
            let pool_id = &pool.pool_info.pool_identifier;
            let assets = &pool.pool_info.assets;

            // For each asset in the pool, query its decimals using the proper API
            for asset in assets {
                let denom = &asset.denom;

                // Skip if we already have this denomination
                if asset_decimals_map.contains_key(denom) {
                    continue;
                }

                // Query asset decimals using the pool manager API
                match self.query_asset_decimals(pool_id, denom).await {
                    Ok(decimals) => {
                        asset_decimals_map.insert(denom.clone(), decimals);
                    }
                    Err(e) => {
                        // Log error but continue with other assets
                        eprintln!(
                            "Failed to query decimals for {} in pool {}: {}",
                            denom, pool_id, e
                        );

                        // Fallback to reasonable defaults for known tokens
                        let fallback_decimals = match denom.as_str() {
                            "uom" => 6,
                            d if d.starts_with("factory/") => 6,
                            d if d.starts_with("ibc/") => 6,
                            _ => 6,
                        };
                        asset_decimals_map.insert(denom.clone(), fallback_decimals);
                    }
                }
            }
        }

        // Ensure native token is always present
        asset_decimals_map.entry("uom".to_string()).or_insert(6);

        Ok(asset_decimals_map)
    }

    /// Get asset decimals for a specific denomination
    ///
    /// This method attempts to find a pool containing the asset and query its decimals.
    /// If multiple pools contain the asset, it uses the first one found.
    ///
    /// # Arguments
    ///
    /// * `denom` - The asset denomination to get decimals for
    ///
    /// # Returns
    ///
    /// The number of decimal places for the asset
    pub async fn get_asset_decimals(&self, denom: &str) -> Result<u8, Error> {
        // First try to find a pool that contains this asset
        let pools = self.get_pools(Some(50)).await?;

        for pool in pools {
            let pool_id = &pool.pool_info.pool_identifier;
            let assets = &pool.pool_info.assets;

            // Check if this pool contains the requested asset
            if assets.iter().any(|asset| asset.denom == denom) {
                // Found a pool with this asset, query its decimals
                return self.query_asset_decimals(pool_id, denom).await;
            }
        }

        // If not found in any pool, return reasonable default
        Ok(match denom {
            "uom" => 6,
            d if d.starts_with("factory/") => 6,
            d if d.starts_with("ibc/") => 6,
            _ => 6,
        })
    }

    /// Get pool information by ID
    pub async fn get_pool(&self, pool_id: &str) -> Result<PoolInfoResponse, Error> {
        let query = pool_manager::QueryMsg::Pools {
            pool_identifier: Some(pool_id.to_string()),
            start_after: None,
            limit: None,
        };
        let pool_manager_address = self.config.contracts.pool_manager.clone();
        let response: PoolsResponse = self.query(&pool_manager_address, &query).await?;
        if response.pools.is_empty() {
            return Err(Error::Other(format!("Pool {} not found", pool_id)));
        }

        let pool = &response.pools[0];
        Ok(pool.clone())
    }

    /// Get list of pools
    pub async fn get_pools(&self, limit: Option<u32>) -> Result<Vec<PoolInfoResponse>, Error> {
        let query = pool_manager::QueryMsg::Pools {
            pool_identifier: None,
            start_after: None,
            limit,
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        let response: PoolsResponse = self.query(&pool_manager_address, &query).await?;

        Ok(response.pools)
    }

    /// Extract pool status from PoolInfoResponse
    pub fn get_pool_status(&self, pool: &PoolInfoResponse) -> PoolStatus {
        // Map the actual status from pool.pool_info.status to our PoolStatus enum
        // The status field in the mantra_dex_std::pool_manager::PoolInfo structure
        // contains information about the pool's operational state
        let status = &pool.pool_info.status;

        // If all operations are enabled, the pool is Available
        // In v3.0.0, we check if swaps, deposits, and withdrawals are all enabled
        if status.swaps_enabled && status.deposits_enabled && status.withdrawals_enabled {
            PoolStatus::Available
        } else {
            // If any operation is disabled, the pool is considered Disabled
            PoolStatus::Disabled
        }
    }

    /// Validate that a pool is available for operations
    pub async fn validate_pool_status(&self, pool_id: &str) -> Result<(), Error> {
        let pool = self.get_pool(pool_id).await?;
        let status = self.get_pool_status(&pool);

        if !status.is_available() {
            return Err(Error::Other(format!(
                "Pool {} is not available for operations (status: {:?})",
                pool_id, status
            )));
        }

        Ok(())
    }

    /// Simulate a swap to see the expected amount
    pub async fn simulate_swap(
        &self,
        pool_id: &str,
        offer_asset: Coin,
        ask_asset_denom: &str,
    ) -> Result<SimulationResponse, Error> {
        let query = pool_manager::QueryMsg::Simulation {
            pool_identifier: pool_id.to_string(),
            ask_asset_denom: ask_asset_denom.to_string(),
            offer_asset: offer_asset.clone(),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.query(&pool_manager_address, &query).await
    }

    /// Swap tokens
    /// Execute a swap operation on a pool
    ///
    /// **v3.0.0 Breaking Change**: The `max_spread` parameter has been renamed to `max_slippage`
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The identifier of the pool to swap in
    /// * `offer_asset` - The asset being offered for swap
    /// * `ask_asset_denom` - The denomination of the asset being requested
    /// * `max_slippage` - Optional maximum slippage tolerance (replaces `max_spread` from v2.x)
    ///
    /// # Returns
    ///
    /// Transaction response containing the swap result
    ///
    /// # Errors
    ///
    /// * Returns error if pool status validation fails (pool must be Available)
    /// * Returns error if the swap transaction fails
    /// * Returns error if no wallet is configured
    pub async fn swap(
        &self,
        pool_id: &str,
        offer_asset: Coin,
        ask_asset_denom: &str,
        max_slippage: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        // Input validation
        if pool_id.trim().is_empty() {
            return Err(Error::Other("Pool ID cannot be empty".to_string()));
        }
        if offer_asset.amount.is_zero() {
            return Err(Error::Other(
                "Offer amount must be greater than zero".to_string(),
            ));
        }
        if offer_asset.denom.trim().is_empty() {
            return Err(Error::Other(
                "Offer asset denom cannot be empty".to_string(),
            ));
        }
        if ask_asset_denom.trim().is_empty() {
            return Err(Error::Other("Ask asset denom cannot be empty".to_string()));
        }

        // Validate pool status before executing swap
        self.validate_pool_status(pool_id).await?;

        let msg = pool_manager::ExecuteMsg::Swap {
            pool_identifier: pool_id.to_string(),
            belief_price: None,
            receiver: None,
            ask_asset_denom: ask_asset_denom.to_string(),
            max_slippage: max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, vec![offer_asset])
            .await
    }

    /// Provide liquidity to a pool
    ///
    /// **v3.0.0 Breaking Changes**:
    /// - `slippage_tolerance` parameter renamed to `liquidity_max_slippage`
    /// - `max_spread` parameter renamed to `swap_max_slippage`
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The identifier of the pool to provide liquidity to
    /// * `assets` - Vector of assets to provide as liquidity
    /// * `liquidity_max_slippage` - Optional maximum slippage for liquidity provision (replaces `slippage_tolerance`)
    /// * `swap_max_slippage` - Optional maximum slippage for internal swaps (replaces `max_spread`)
    ///
    /// # Returns
    ///
    /// Transaction response containing the liquidity provision result
    ///
    /// # Errors
    ///
    /// * Returns error if pool status validation fails (pool must be Available)
    /// * Returns error if the liquidity provision transaction fails
    /// * Returns error if no wallet is configured
    pub async fn provide_liquidity(
        &self,
        pool_id: &str,
        assets: Vec<Coin>,
        liquidity_max_slippage: Option<Decimal>,
        swap_max_slippage: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        // Validate pool status before providing liquidity
        self.validate_pool_status(pool_id).await?;

        let msg = pool_manager::ExecuteMsg::ProvideLiquidity {
            pool_identifier: pool_id.to_string(),
            liquidity_max_slippage: liquidity_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            swap_max_slippage: swap_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            receiver: None,
            unlocking_duration: None,
            lock_position_identifier: None,
        };

        let mut coins: Vec<Coin> = assets
            .into_iter()
            .map(|a| Coin {
                denom: a.denom,
                amount: a.amount,
            })
            .collect();

        // Sort coins by denomination as required by Cosmos SDK
        coins.sort_by(|a, b| a.denom.cmp(&b.denom));

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, coins).await
    }

    /// Provide liquidity to a pool without status validation (for creating new pools)
    ///
    /// This method bypasses pool status validation and should only be used when creating new pools
    /// or in scenarios where pool status checking is not required.
    ///
    /// **v3.0.0 Breaking Changes**: Same parameter renames as `provide_liquidity`
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The identifier of the pool to provide liquidity to
    /// * `assets` - Vector of assets to provide as liquidity
    /// * `liquidity_max_slippage` - Optional maximum slippage for liquidity provision
    /// * `swap_max_slippage` - Optional maximum slippage for internal swaps
    ///
    /// # Returns
    ///
    /// Transaction response containing the liquidity provision result
    ///
    /// # Errors
    ///
    /// * Returns error if the liquidity provision transaction fails
    /// * Returns error if no wallet is configured
    pub async fn provide_liquidity_unchecked(
        &self,
        pool_id: &str,
        assets: Vec<Coin>,
        liquidity_max_slippage: Option<Decimal>,
        swap_max_slippage: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        let msg = pool_manager::ExecuteMsg::ProvideLiquidity {
            pool_identifier: pool_id.to_string(),
            liquidity_max_slippage: liquidity_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            swap_max_slippage: swap_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            receiver: None,
            unlocking_duration: None,
            lock_position_identifier: None,
        };

        let mut coins: Vec<Coin> = assets
            .into_iter()
            .map(|a| Coin {
                denom: a.denom,
                amount: a.amount,
            })
            .collect();

        // Sort coins by denomination as required by Cosmos SDK
        coins.sort_by(|a, b| a.denom.cmp(&b.denom));

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, coins).await
    }

    /// Withdraw liquidity from a pool
    pub async fn withdraw_liquidity(
        &self,
        pool_id: &str,
        lp_amount: Uint128,
    ) -> Result<TxResponse, Error> {
        // Get pool info and validate status in one call
        let pool = self.get_pool(pool_id).await?;
        let status = self.get_pool_status(&pool);

        if !status.is_available() {
            return Err(Error::Other(format!(
                "Pool {} is not available for operations (status: {:?})",
                pool_id, status
            )));
        }

        let lp_denom = pool.pool_info.lp_denom;
        let msg = pool_manager::ExecuteMsg::WithdrawLiquidity {
            pool_identifier: pool_id.to_string(),
        };
        let funds = vec![Coin {
            denom: lp_denom,
            amount: lp_amount,
        }];
        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, funds).await
    }

    /// Query the pool manager configuration
    pub async fn get_pool_manager_config(
        &self,
    ) -> Result<mantra_dex_std::pool_manager::Config, Error> {
        let query = pool_manager::QueryMsg::Config {};
        let pool_manager_address = self.config.contracts.pool_manager.clone();
        // The contract returns Config directly, not wrapped in ConfigResponse
        let config: mantra_dex_std::pool_manager::Config =
            self.query(&pool_manager_address, &query).await?;
        Ok(config)
    }

    /// Get the pool creation fee from the pool manager configuration
    pub async fn get_pool_creation_fee(&self) -> Result<Coin, Error> {
        let config = self.get_pool_manager_config().await?;
        Ok(config.pool_creation_fee)
    }

    /// Create a new pool with the specified assets and configuration
    ///
    /// **v3.0.0 New Feature**: Enhanced fee validation ensures total fees ≤ 20%
    ///
    /// # Arguments
    ///
    /// * `asset_denoms` - Vector of asset denominations for the pool
    /// * `asset_decimals` - Vector of decimal places for each asset
    /// * `pool_fees` - Fee structure for the pool (validated against v3.0.0 requirements)
    /// * `pool_type` - Type of pool to create
    /// * `pool_identifier` - Optional custom identifier for the pool
    ///
    /// # Returns
    ///
    /// Transaction response containing the pool creation result
    ///
    /// # Errors
    ///
    /// * Returns `FeeValidation` error if pool fees exceed 20% total
    /// * Returns error if pool creation transaction fails
    /// * Returns error if no wallet is configured
    ///
    /// # Notes
    ///
    /// Pool creation requires a fee that is determined by querying the pool manager configuration
    pub async fn create_pool(
        &self,
        asset_denoms: Vec<String>,
        asset_decimals: Vec<u8>,
        pool_fees: mantra_dex_std::fee::PoolFee,
        pool_type: mantra_dex_std::pool_manager::PoolType,
        pool_identifier: Option<String>,
    ) -> Result<TxResponse, Error> {
        // Validate pool fees before creating the pool (v3.0.0 requirement)
        self.validate_pool_fees(&pool_fees)?;

        let msg = pool_manager::ExecuteMsg::CreatePool {
            asset_denoms,
            asset_decimals,
            pool_fees,
            pool_type,
            pool_identifier,
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();

        // Query the actual pool creation fee from the contract configuration
        let creation_fee = self.get_pool_creation_fee().await?;

        // Handle case where contract config shows 0 but contract actually expects 88 OM
        let pool_creation_fee = if creation_fee.amount.is_zero() {
            // Fallback to known testnet pool creation fee of 88 OM
            vec![Coin {
                denom: "uom".to_string(),
                amount: Uint128::new(88_000_000), // 88 OM
            }]
        } else {
            vec![creation_fee]
        };

        self.execute(&pool_manager_address, &msg, pool_creation_fee)
            .await
    }

    /// Execute multiple swap operations
    pub async fn execute_swap_operations(
        &self,
        operations: Vec<SwapOperation>,
        amount: Uint128,
    ) -> Result<TxResponse, Error> {
        let first_op = operations
            .first()
            .ok_or_else(|| Error::Other("Swap operations list cannot be empty".to_string()))?;

        // Validate pool status for the first operation
        self.validate_pool_status(&first_op.get_pool_identifer())
            .await?;

        let msg = pool_manager::ExecuteMsg::Swap {
            ask_asset_denom: first_op.get_target_asset_info().clone(),
            belief_price: None,
            max_slippage: None,
            receiver: None,
            pool_identifier: first_op.get_pool_identifer().clone(),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(
            &pool_manager_address,
            &msg,
            vec![Coin {
                denom: first_op.get_input_asset_info().clone(),
                amount,
            }],
        )
        .await
    }

    // =========================
    // Farm Manager Functionality
    // =========================

    /// Claim rewards from farm manager with optional epoch parameter
    ///
    /// **v3.0.0 New Feature**: Enhanced claim functionality with epoch-based claiming
    ///
    /// # Arguments
    ///
    /// * `until_epoch` - Optional epoch limit for claiming rewards. If provided, only claims rewards up to that epoch
    ///
    /// # Returns
    ///
    /// Transaction response containing the claim result
    ///
    /// # Errors
    ///
    /// * Returns error if farm manager contract is not configured
    /// * Returns error if the claim transaction fails
    /// * Returns error if no wallet is configured
    ///
    /// # Backward Compatibility
    ///
    /// When `until_epoch` is `None`, behaves like the v2.x parameterless claim
    pub async fn claim_rewards(&self, until_epoch: Option<u64>) -> Result<TxResponse, Error> {
        let farm_manager_address =
            self.config.contracts.farm_manager.as_ref().ok_or_else(|| {
                Error::Other("Farm manager contract address not configured".to_string())
            })?;

        let msg = if let Some(epoch) = until_epoch {
            serde_json::json!({
                "claim": {
                    "until_epoch": epoch
                }
            })
        } else {
            // Backward compatibility: parameterless claim
            serde_json::json!({
                "claim": {}
            })
        };

        self.execute(farm_manager_address, &msg, vec![]).await
    }

    /// Claim rewards without epoch parameter (backward compatibility)
    ///
    /// This is a convenience method that calls `claim_rewards(None)` for backward compatibility
    /// with v2.x code that used parameterless claim methods.
    ///
    /// # Returns
    ///
    /// Transaction response containing the claim result
    pub async fn claim_rewards_all(&self) -> Result<TxResponse, Error> {
        self.claim_rewards(None).await
    }

    /// Claim rewards up to a specific epoch
    ///
    /// This is a convenience method that calls `claim_rewards(Some(until_epoch))`.
    ///
    /// # Arguments
    ///
    /// * `until_epoch` - The epoch limit for claiming rewards
    ///
    /// # Returns
    ///
    /// Transaction response containing the claim result
    pub async fn claim_rewards_until_epoch(&self, until_epoch: u64) -> Result<TxResponse, Error> {
        self.claim_rewards(Some(until_epoch)).await
    }

    /// Query rewards for an address with optional epoch parameter
    ///
    /// **v3.0.0 New Feature**: Enhanced rewards query with epoch range support
    ///
    /// # Arguments
    ///
    /// * `address` - The address to query rewards for
    /// * `until_epoch` - Optional epoch limit for querying rewards. If provided, only returns rewards up to that epoch
    ///
    /// # Returns
    ///
    /// JSON value containing the rewards information
    ///
    /// # Errors
    ///
    /// * Returns error if farm manager contract is not configured
    /// * Returns error if the query fails
    ///
    /// # Backward Compatibility
    ///
    /// When `until_epoch` is `None`, behaves like the v2.x parameterless rewards query
    pub async fn query_rewards(
        &self,
        address: &str,
        until_epoch: Option<u64>,
    ) -> Result<serde_json::Value, Error> {
        let farm_manager_address =
            self.config.contracts.farm_manager.as_ref().ok_or_else(|| {
                Error::Other("Farm manager contract address not configured".to_string())
            })?;

        let query = if let Some(epoch) = until_epoch {
            serde_json::json!({
                "rewards": {
                    "address": address,
                    "until_epoch": epoch
                }
            })
        } else {
            serde_json::json!({
                "rewards": {
                    "address": address
                }
            })
        };

        self.query(farm_manager_address, &query).await
    }

    /// Query all rewards for an address (backward compatibility)
    pub async fn query_all_rewards(&self, address: &str) -> Result<serde_json::Value, Error> {
        self.query_rewards(address, None).await
    }

    /// Query rewards for an address up to a specific epoch
    pub async fn query_rewards_until_epoch(
        &self,
        address: &str,
        until_epoch: u64,
    ) -> Result<serde_json::Value, Error> {
        self.query_rewards(address, Some(until_epoch)).await
    }

    /// Get current epoch from epoch manager contract
    pub async fn get_current_epoch(&self) -> Result<u64, Error> {
        let epoch_manager_address =
            self.config
                .contracts
                .epoch_manager
                .as_ref()
                .ok_or_else(|| {
                    Error::Other("Epoch manager contract address not configured".to_string())
                })?;

        let query = serde_json::json!({
            "current_epoch": {}
        });

        let response: serde_json::Value = self.query(epoch_manager_address, &query).await?;

        // Extract epoch number from response
        response
            .get("epoch")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| Error::Other("Failed to parse epoch from response".to_string()))
    }

    /// Validate epoch parameter for claim/query operations
    pub async fn validate_epoch(&self, epoch: u64) -> Result<(), Error> {
        let current_epoch = self.get_current_epoch().await?;

        if epoch > current_epoch {
            return Err(Error::Other(format!(
                "Cannot specify future epoch {}. Current epoch is {}",
                epoch, current_epoch
            )));
        }

        Ok(())
    }

    // =========================
    // Feature Toggle Functionality
    // =========================

    /// Update pool feature toggles with per-pool control
    /// This allows fine-tuned control over which operations are allowed for a specific pool
    /// Update feature toggles for a specific pool
    ///
    /// **v3.0.0 New Feature**: Per-pool feature toggles with pool_identifier targeting
    ///
    /// # Arguments
    ///
    /// * `pool_identifier` - The identifier of the pool to update features for
    /// * `withdrawals_enabled` - Optional flag to enable/disable withdrawals for this pool
    /// * `deposits_enabled` - Optional flag to enable/disable deposits for this pool
    /// * `swaps_enabled` - Optional flag to enable/disable swaps for this pool
    ///
    /// # Returns
    ///
    /// Transaction response containing the feature update result
    ///
    /// # Errors
    ///
    /// * Returns error if the feature update transaction fails
    /// * Returns error if no wallet is configured
    ///
    /// # Notes
    ///
    /// In v3.0.0, all feature toggles must target specific pools via pool_identifier
    pub async fn update_pool_features(
        &self,
        pool_identifier: &str,
        withdrawals_enabled: Option<bool>,
        deposits_enabled: Option<bool>,
        swaps_enabled: Option<bool>,
    ) -> Result<TxResponse, Error> {
        let feature_toggle = mantra_dex_std::pool_manager::FeatureToggle {
            pool_identifier: pool_identifier.to_string(),
            withdrawals_enabled,
            deposits_enabled,
            swaps_enabled,
        };

        let msg = pool_manager::ExecuteMsg::UpdateConfig {
            fee_collector_addr: None,
            farm_manager_addr: None,
            pool_creation_fee: None,
            feature_toggle: Some(feature_toggle),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, vec![]).await
    }

    /// Enable withdrawals for a specific pool
    pub async fn enable_pool_withdrawals(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(true), None, None)
            .await
    }

    /// Disable withdrawals for a specific pool
    pub async fn disable_pool_withdrawals(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(false), None, None)
            .await
    }

    /// Enable deposits for a specific pool
    pub async fn enable_pool_deposits(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, Some(true), None)
            .await
    }

    /// Disable deposits for a specific pool
    pub async fn disable_pool_deposits(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, Some(false), None)
            .await
    }

    /// Enable swaps for a specific pool
    pub async fn enable_pool_swaps(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, None, Some(true))
            .await
    }

    /// Disable swaps for a specific pool
    pub async fn disable_pool_swaps(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, None, Some(false))
            .await
    }

    /// Enable all operations for a specific pool
    pub async fn enable_all_pool_operations(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(true), Some(true), Some(true))
            .await
    }

    /// Disable all operations for a specific pool
    pub async fn disable_all_pool_operations(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(false), Some(false), Some(false))
            .await
    }

    /// Update global feature toggles (backward compatibility)
    /// This method maintains compatibility with the v2.x approach but uses per-pool targeting
    /// The pool_identifier parameter allows targeting specific pools for global-style operations
    #[deprecated(
        since = "3.0.0",
        note = "Use update_pool_features for per-pool control"
    )]
    pub async fn update_global_features(
        &self,
        pool_identifier: &str, // Required in v3.0.0 - pools must be targeted specifically
        withdrawals_enabled: Option<bool>,
        deposits_enabled: Option<bool>,
        swaps_enabled: Option<bool>,
    ) -> Result<TxResponse, Error> {
        // Delegate to the new per-pool method
        self.update_pool_features(
            pool_identifier,
            withdrawals_enabled,
            deposits_enabled,
            swaps_enabled,
        )
        .await
    }

    // =========================
    // Fee Validation Functionality for v3.0.0
    // =========================

    /// Validate pool fee structure to ensure total fees don't exceed 20%
    /// This is a critical requirement for v3.0.0 compatibility
    /// Validate pool fees according to v3.0.0 requirements
    ///
    /// **v3.0.0 New Feature**: Enhanced fee validation ensures total fees ≤ 20%
    ///
    /// This method validates that the sum of all fees (protocol_fee + swap_fee + burn_fee + extra_fees)
    /// does not exceed 20% (0.2) as required by the v3.0.0 specification.
    ///
    /// # Arguments
    ///
    /// * `pool_fees` - The pool fee structure to validate
    ///
    /// # Returns
    ///
    /// `Ok(())` if fees are valid, otherwise returns a `FeeValidation` error
    ///
    /// # Errors
    ///
    /// * Returns `FeeValidation` error if total fees exceed 20%
    /// * Returns `FeeValidation` error if any individual fee is negative
    ///
    /// # Fee Structure
    ///
    /// The v3.0.0 fee structure includes:
    /// - `protocol_fee`: Fee for the protocol
    /// - `swap_fee`: Fee for swaps
    /// - `burn_fee`: Optional fee that gets burned
    /// - `extra_fees`: Optional array of additional fees
    pub fn validate_pool_fees(
        &self,
        pool_fees: &mantra_dex_std::fee::PoolFee,
    ) -> Result<(), Error> {
        let protocol_fee = pool_fees.protocol_fee.share;
        let swap_fee = pool_fees.swap_fee.share;
        let burn_fee = pool_fees.burn_fee.share;

        // Calculate total from extra fees
        let extra_fees_total: cosmwasm_std::Decimal =
            pool_fees.extra_fees.iter().map(|fee| fee.share).sum();

        // Calculate total fees
        let total_fees = protocol_fee + swap_fee + burn_fee + extra_fees_total;

        // Maximum allowed total fees is 20% (0.2)
        let max_total_fees = cosmwasm_std::Decimal::percent(20);

        if total_fees > max_total_fees {
            return Err(Error::FeeValidation(format!(
                "Total fees ({}) exceed maximum allowed ({}). Protocol: {}, Swap: {}, Burn: {}, Extra: {}",
                total_fees,
                max_total_fees,
                protocol_fee,
                swap_fee,
                burn_fee,
                extra_fees_total
            )));
        }

        // Individual fee validation (each fee should be non-negative)
        // Note: cosmwasm_std::Decimal is always non-negative by design, so this validation
        // is primarily for completeness and future-proofing
        let zero = cosmwasm_std::Decimal::zero();
        if protocol_fee < zero || swap_fee < zero || burn_fee < zero {
            return Err(Error::FeeValidation(
                "Individual fees cannot be negative".to_string(),
            ));
        }

        // Validate extra fees
        for (i, fee) in pool_fees.extra_fees.iter().enumerate() {
            if fee.share < zero {
                return Err(Error::FeeValidation(format!(
                    "Extra fee {} cannot be negative: {}",
                    i, fee.share
                )));
            }
        }

        Ok(())
    }

    /// Create a validated PoolFee structure with automatic total fee checking
    /// Create a validated pool fee structure
    ///
    /// **v3.0.0 New Feature**: Helper method to create and validate pool fees in one step
    ///
    /// This method creates a `PoolFee` structure from the provided fee components and validates
    /// that the total fees do not exceed 20% as required by v3.0.0.
    ///
    /// # Arguments
    ///
    /// * `protocol_fee` - Protocol fee percentage
    /// * `swap_fee` - Swap fee percentage
    /// * `burn_fee` - Optional burn fee percentage
    /// * `extra_fees` - Optional vector of additional fee percentages
    ///
    /// # Returns
    ///
    /// A validated `PoolFee` structure ready for use in pool creation
    ///
    /// # Errors
    ///
    /// * Returns `FeeValidation` error if total fees exceed 20%
    /// * Returns `FeeValidation` error if any individual fee is negative
    pub fn create_validated_pool_fees(
        &self,
        protocol_fee: cosmwasm_std::Decimal,
        swap_fee: cosmwasm_std::Decimal,
        burn_fee: Option<cosmwasm_std::Decimal>,
        extra_fees: Option<Vec<cosmwasm_std::Decimal>>,
    ) -> Result<mantra_dex_std::fee::PoolFee, Error> {
        let burn_fee = burn_fee.unwrap_or_else(cosmwasm_std::Decimal::zero);
        let extra_fees = extra_fees.unwrap_or_default();

        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: protocol_fee,
            },
            swap_fee: mantra_dex_std::fee::Fee { share: swap_fee },
            burn_fee: mantra_dex_std::fee::Fee { share: burn_fee },
            extra_fees: extra_fees
                .into_iter()
                .map(|share| mantra_dex_std::fee::Fee { share })
                .collect(),
        };

        // Validate the created fee structure
        self.validate_pool_fees(&pool_fees)?;

        Ok(pool_fees)
    }

    // =========================
    // Skip Adapter Functionality
    // =========================

    /// Get the Skip Mantra DEX adapter contract address
    fn get_skip_mantra_dex_adapter(&self) -> Result<&str, Error> {
        self.config
            .contracts
            .skip_mantra_dex_adapter
            .as_deref()
            .ok_or_else(|| {
                Error::Other("Skip Mantra DEX adapter contract address not configured".to_string())
            })
    }

    /// Internal helper method for executing Skip swaps with common logic
    async fn execute_skip_swap_internal(
        &self,
        operations: Vec<crate::protocols::skip::SkipSwapOperation>,
        offer_coin: Coin,
        min_receive_amount: Uint128,
        post_swap_action: crate::protocols::skip::SkipAction,
        affiliates: Vec<crate::protocols::skip::SkipAffiliate>,
    ) -> Result<TxResponse, Error> {
        let skip_entry_point =
            self.config
                .contracts
                .skip_entry_point
                .as_ref()
                .ok_or_else(|| {
                    Error::Other("Skip entry point contract address not configured".to_string())
                })?;

        // Get the output denom from the last operation
        let output_denom = operations
            .last()
            .map(|op| op.denom_out.clone())
            .ok_or_else(|| Error::Other("No swap operations provided".to_string()))?;

        // Create the swap structure
        let swap = crate::protocols::skip::SkipSwap::SwapExactAssetIn(
            crate::protocols::skip::SkipSwapExactAssetIn {
                swap_venue_name: "mantra-dex".to_string(),
                operations,
            },
        );

        // Create assets
        let remaining_asset = crate::protocols::skip::SkipAsset::Native(offer_coin.clone());
        let min_asset = crate::protocols::skip::SkipAsset::Native(Coin {
            denom: output_denom,
            amount: min_receive_amount,
        });

        let msg = crate::protocols::skip::SkipEntryPointExecuteMsg::SwapAndAction {
            sent_asset: Some(remaining_asset),
            user_swap: swap,
            min_asset,
            // Timeout: current time + 15 minutes, converted to nanoseconds (improved UX)
            timeout_timestamp: (chrono::Utc::now().timestamp() as u64 + 900) * 1_000_000_000,
            post_swap_action,
            affiliates,
        };

        self.execute(skip_entry_point, &msg, vec![offer_coin]).await
    }

    /// Execute a swap through Skip Adapter
    ///
    /// This method enables cross-chain and advanced routing functionality through Skip's adapter system.
    /// The swap is routed through the Skip entry point contract which manages multi-hop operations.
    ///
    /// # Arguments
    ///
    /// * `operations` - Vector of skip swap operations defining the swap path
    /// * `offer_coin` - The coin to offer for the swap
    /// * `min_receive_amount` - Minimum amount to receive (for slippage protection)
    /// * `receiver` - Optional receiver address (defaults to sender if None)
    ///
    /// # Returns
    ///
    /// Transaction response containing the swap result
    ///
    /// # Errors
    ///
    /// * Returns error if Skip adapter contracts are not configured
    /// * Returns error if the swap transaction fails
    /// * Returns error if no wallet is configured
    pub async fn execute_skip_swap(
        &self,
        operations: Vec<crate::protocols::skip::SkipSwapOperation>,
        offer_coin: Coin,
        min_receive_amount: Option<Uint128>,
        receiver: Option<String>,
    ) -> Result<TxResponse, Error> {
        // Input validation
        if operations.is_empty() {
            return Err(Error::Other("Swap operations cannot be empty".to_string()));
        }
        if offer_coin.amount.is_zero() {
            return Err(Error::Other(
                "Offer amount must be greater than zero".to_string(),
            ));
        }
        if offer_coin.denom.trim().is_empty() {
            return Err(Error::Other("Offer coin denom cannot be empty".to_string()));
        }

        // Calculate minimum receive amount (default to 95% of input for basic slippage protection)
        let min_amount = min_receive_amount.unwrap_or_else(|| {
            // Basic slippage protection: expect at least 95% of input value
            let slippage_factor = Decimal::from_str("0.95").unwrap_or(Decimal::percent(95));
            offer_coin
                .amount
                .multiply_ratio(slippage_factor.atomics(), Decimal::one().atomics())
        });

        // Get receiver address
        let receiver_addr = match receiver {
            Some(addr) => addr,
            None => {
                let wallet = self.wallet.as_ref().ok_or_else(|| {
                    Error::Other("No wallet configured for Skip swap".to_string())
                })?;
                wallet.address()?.to_string()
            }
        };

        let post_swap_action = crate::protocols::skip::SkipAction::Transfer {
            to_address: receiver_addr,
        };

        self.execute_skip_swap_internal(
            operations,
            offer_coin,
            min_amount,
            post_swap_action,
            vec![],
        )
        .await
    }

    /// Execute a swap with cross-chain action through Skip Adapter
    ///
    /// This method combines a swap with a subsequent action like IBC transfer.
    ///
    /// # Arguments
    ///
    /// * `operations` - Vector of skip swap operations
    /// * `offer_coin` - The coin to offer for the swap
    /// * `min_receive_amount` - Minimum amount to receive
    /// * `action` - Post-swap action to execute
    /// * `affiliates` - Optional affiliate addresses for fee sharing
    ///
    /// # Returns
    ///
    /// Transaction response containing the swap and action result
    pub async fn execute_skip_swap_and_action(
        &self,
        operations: Vec<crate::protocols::skip::SkipSwapOperation>,
        offer_coin: Coin,
        min_receive_amount: Uint128,
        action: crate::protocols::skip::SkipAction,
        affiliates: Option<Vec<crate::protocols::skip::SkipAffiliate>>,
    ) -> Result<TxResponse, Error> {
        self.execute_skip_swap_internal(
            operations,
            offer_coin,
            min_receive_amount,
            action,
            affiliates.unwrap_or_default(),
        )
        .await
    }

    /// Simulate a swap exact asset in through Skip Adapter
    ///
    /// This method simulates a swap through the Skip adapter system without executing it.
    /// Useful for getting expected output amounts and fees before executing the actual swap.
    ///
    /// # Arguments
    ///
    /// * `asset_in` - The asset to swap in
    /// * `swap_operations` - Vector of swap operations to perform
    ///
    /// # Returns
    ///
    /// The expected output asset from the simulation
    pub async fn simulate_skip_swap_exact_asset_in(
        &self,
        asset_in: crate::protocols::skip::SkipAsset,
        swap_operations: Vec<crate::protocols::skip::SkipSwapOperation>,
    ) -> Result<crate::protocols::skip::SkipAsset, Error> {
        let skip_mantra_dex_adapter = self.get_skip_mantra_dex_adapter()?;

        let query = crate::protocols::skip::SkipEntryPointQueryMsg::SimulateSwapExactAssetIn {
            asset_in,
            swap_operations,
        };

        // Contract returns the output asset directly
        self.query(skip_mantra_dex_adapter, &query).await
    }

    /// Simulate a swap exact asset out through Skip Adapter
    ///
    /// This method simulates a reverse swap through the Skip adapter system.
    /// It calculates how much input asset is needed to get a specific output amount.
    ///
    /// # Arguments
    ///
    /// * `asset_out` - The desired output asset and amount
    /// * `swap_operations` - Vector of swap operations to perform
    ///
    /// # Returns
    ///
    /// The required input asset for the simulation
    pub async fn simulate_skip_swap_exact_asset_out(
        &self,
        asset_out: crate::protocols::skip::SkipAsset,
        swap_operations: Vec<crate::protocols::skip::SkipSwapOperation>,
    ) -> Result<crate::protocols::skip::SkipAsset, Error> {
        let skip_mantra_dex_adapter = self.get_skip_mantra_dex_adapter()?;

        let query = crate::protocols::skip::SkipEntryPointQueryMsg::SimulateSwapExactAssetOut {
            asset_out,
            swap_operations,
        };

        // Contract returns the required input asset directly
        self.query(skip_mantra_dex_adapter, &query).await
    }

    /// Simulate a smart swap exact asset in through Skip Adapter
    ///
    /// This method simulates an intelligent multi-route swap that can split the input
    /// across multiple paths for optimal execution through the Skip adapter system.
    ///
    /// # Arguments
    ///
    /// * `asset_in` - The asset to swap in
    /// * `routes` - Vector of possible routes to consider for the swap
    ///
    /// # Returns
    ///
    /// The expected output asset from the optimal route simulation
    pub async fn simulate_skip_smart_swap_exact_asset_in(
        &self,
        asset_in: crate::protocols::skip::SkipAsset,
        routes: Vec<crate::protocols::skip::SkipRoute>,
    ) -> Result<crate::protocols::skip::SkipAsset, Error> {
        let skip_mantra_dex_adapter = self.get_skip_mantra_dex_adapter()?;

        let query = crate::protocols::skip::SkipEntryPointQueryMsg::SimulateSmartSwapExactAssetIn {
            asset_in,
            routes,
        };

        self.query(skip_mantra_dex_adapter, &query).await
    }

    /// Simulate a swap operation with exact asset in and metadata through Skip Adapter
    ///
    /// This method provides additional metadata like spot price information
    /// in addition to the basic simulation results.
    ///
    /// # Arguments
    ///
    /// * `asset_in` - The asset to be swapped in
    /// * `swap_operations` - Vector of swap operations to perform  
    /// * `include_spot_price` - Whether to include spot price calculation
    ///
    /// # Returns
    ///
    /// Response containing the output asset and optional spot price metadata
    pub async fn simulate_skip_swap_exact_asset_in_with_metadata(
        &self,
        asset_in: crate::protocols::skip::SkipAsset,
        swap_operations: Vec<crate::protocols::skip::SkipSwapOperation>,
        include_spot_price: bool,
    ) -> Result<crate::protocols::skip::SimulateSwapExactAssetInResponse, Error> {
        let skip_mantra_dex_adapter = self.get_skip_mantra_dex_adapter()?;

        let query =
            crate::protocols::skip::SkipEntryPointQueryMsg::SimulateSwapExactAssetInWithMetadata {
                asset_in,
                swap_operations,
                include_spot_price,
            };

        self.query(skip_mantra_dex_adapter, &query).await
    }

    /// Simulate a swap operation with exact asset out and metadata through Skip Adapter
    ///
    /// This method provides additional metadata like spot price information
    /// in addition to the basic simulation results.
    ///
    /// # Arguments
    ///
    /// * `asset_out` - The desired output asset amount
    /// * `swap_operations` - Vector of swap operations to perform
    /// * `include_spot_price` - Whether to include spot price calculation
    ///
    /// # Returns
    ///
    /// Response containing the required input asset and optional spot price metadata
    pub async fn simulate_skip_swap_exact_asset_out_with_metadata(
        &self,
        asset_out: crate::protocols::skip::SkipAsset,
        swap_operations: Vec<crate::protocols::skip::SkipSwapOperation>,
        include_spot_price: bool,
    ) -> Result<crate::protocols::skip::SimulateSwapExactAssetOutResponse, Error> {
        let skip_mantra_dex_adapter = self.get_skip_mantra_dex_adapter()?;

        let query =
            crate::protocols::skip::SkipEntryPointQueryMsg::SimulateSwapExactAssetOutWithMetadata {
                asset_out,
                swap_operations,
                include_spot_price,
            };

        self.query(skip_mantra_dex_adapter, &query).await
    }

    /// Simulate a smart swap operation with exact asset in and metadata through Skip Adapter
    ///
    /// This method provides additional metadata like spot price information
    /// for multi-route smart swap operations.
    ///
    /// # Arguments
    ///
    /// * `asset_in` - The asset to be swapped in
    /// * `routes` - Vector of routes for smart swap optimization
    /// * `include_spot_price` - Whether to include spot price calculation
    ///
    /// # Returns
    ///
    /// Response containing the output asset and optional spot price metadata
    pub async fn simulate_skip_smart_swap_exact_asset_in_with_metadata(
        &self,
        asset_in: crate::protocols::skip::SkipAsset,
        routes: Vec<crate::protocols::skip::SkipRoute>,
        include_spot_price: bool,
    ) -> Result<crate::protocols::skip::SimulateSmartSwapExactAssetInResponse, Error> {
        let skip_mantra_dex_adapter = self.get_skip_mantra_dex_adapter()?;

        let query = crate::protocols::skip::SkipEntryPointQueryMsg::SimulateSmartSwapExactAssetInWithMetadata {
            asset_in,
            routes,
            include_spot_price,
        };

        self.query(skip_mantra_dex_adapter, &query).await
    }

    /// Check if Skip Adapter functionality is available
    ///
    /// This method checks if all required Skip adapter contract addresses are configured.
    ///
    /// # Returns
    ///
    /// `true` if Skip adapter contracts are configured, `false` otherwise
    pub fn is_skip_adapter_available(&self) -> bool {
        self.config.contracts.skip_entry_point.is_some()
            && self.config.contracts.skip_ibc_hooks_adapter.is_some()
            && self.config.contracts.skip_mantra_dex_adapter.is_some()
    }

    /// Get Skip adapter contract addresses
    ///
    /// # Returns
    ///
    /// A tuple containing (entry_point, ibc_hooks_adapter, mantra_dex_adapter) addresses if available
    pub fn get_skip_adapter_addresses(&self) -> Option<(String, String, String)> {
        if let (Some(entry_point), Some(ibc_hooks), Some(mantra_dex)) = (
            self.config.contracts.skip_entry_point.as_ref(),
            self.config.contracts.skip_ibc_hooks_adapter.as_ref(),
            self.config.contracts.skip_mantra_dex_adapter.as_ref(),
        ) {
            Some((entry_point.clone(), ibc_hooks.clone(), mantra_dex.clone()))
        } else {
            None
        }
    }
}

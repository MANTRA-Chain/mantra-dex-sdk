//! Event Handling System
//!
//! This module manages keyboard and mouse events for the TUI application,
//! providing a structured way to handle user input and system events.

#[cfg(feature = "tui-dex")]
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};

#[cfg(feature = "tui-dex")]
use std::time::Duration;
#[cfg(feature = "tui-dex")]
use tokio::sync::mpsc;

/// Focus direction for keyboard navigation
#[derive(Debug, Clone, PartialEq)]
pub enum FocusDirection {
    Next,
    Previous,
    Up,
    Down,
    Left,
    Right,
    First,
    Last,
}

/// Focusable component types for navigation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FocusableComponent {
    /// Text input field
    TextInput(String), // field name/id
    /// Dropdown/select component
    Dropdown(String),
    /// Checkbox component
    Checkbox(String),
    /// Button component
    Button(String),
    /// Table component
    Table(String),
    /// List component
    List(String),
    /// Tab navigation
    TabBar,
    /// Modal dialog
    Modal,
    /// Custom component
    Custom(String),
}

/// Application events that can be handled
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Quit the application
    Quit,
    /// Navigate to next tab
    Tab,
    /// Navigate to previous tab (Shift+Tab)
    BackTab,
    /// Enter/confirm action
    Enter,
    /// Escape/cancel action
    Escape,
    /// Character input
    Char(char),
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Home key
    Home,
    /// End key
    End,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Insert key
    Insert,
    /// Function keys
    F(u8),
    /// Ctrl+key combinations
    Ctrl(char),
    /// Alt+key combinations
    Alt(char),
    /// Refresh/reload action (typically F5)
    Refresh,
    /// Help action (typically F1)
    Help,
    /// Mouse events (placeholder for future implementation)
    Mouse,
    /// Pasted text (bracketed paste)
    Paste(String),
    /// Custom application events
    Custom(String),

    // === Focus Management Events ===
    /// Move focus in a specific direction
    MoveFocus(FocusDirection),
    /// Set focus to a specific component
    SetFocus(FocusableComponent),
    /// Clear all focus
    ClearFocus,
    /// Focus next component in tab order
    FocusNext,
    /// Focus previous component in tab order
    FocusPrevious,
    /// Jump to first focusable component
    FocusFirst,
    /// Jump to last focusable component
    FocusLast,
    /// Activate/interact with focused component
    ActivateFocused,
    /// Context-sensitive action (Space bar)
    ContextAction,

    // === DEX-specific Action Events ===
    /// Execute a swap operation
    ExecuteSwap {
        from_asset: String,
        to_asset: String,
        amount: String,
        pool_id: Option<String>,
        slippage_tolerance: Option<String>,
    },
    /// Execute a swap operation asynchronously
    ExecuteSwapAsync {
        from_asset: String,
        to_asset: String,
        amount: String,
        pool_id: Option<String>,
        slippage_tolerance: Option<String>,
    },
    /// Provide liquidity to a pool
    ProvideLiquidity {
        pool_id: String,
        asset_1_amount: String,
        asset_2_amount: String,
        slippage_tolerance: Option<String>,
    },
    /// Withdraw liquidity from a pool
    WithdrawLiquidity {
        pool_id: String,
        lp_token_amount: String,
        slippage_tolerance: Option<String>,
    },
    /// Claim rewards for specific epochs
    ClaimRewards {
        pool_id: Option<String>,
        epochs: Option<Vec<u64>>,
        claim_all: bool,
    },
    /// Execute multi-hop swap
    ExecuteMultiHopSwap { operations: Vec<SwapOperation> },
    /// Create a new pool (admin)
    CreatePool {
        asset_1: String,
        asset_2: String,
        swap_fee: String,
        exit_fee: String,
        pool_features: Vec<String>,
    },
    /// Update pool features (admin)
    UpdatePoolFeatures {
        pool_id: String,
        features: Vec<String>,
        enabled: bool,
    },
    /// Simulate swap to get preview
    SimulateSwap {
        from_asset: String,
        to_asset: String,
        amount: String,
        pool_id: Option<String>,
    },
    /// Simulate liquidity provision
    SimulateLiquidity {
        pool_id: String,
        asset_1_amount: String,
        asset_2_amount: String,
    },

    // === Async Blockchain Events ===
    /// Blockchain operation completed successfully
    BlockchainSuccess {
        operation: String,
        result: String,
        transaction_hash: Option<String>,
        enhanced_data: Option<String>,
    },
    /// Blockchain operation failed
    BlockchainError { operation: String, error: String },
    /// Blockchain operation is in progress
    BlockchainProgress {
        operation: String,
        status: String,
        progress: Option<f32>, // 0.0 to 1.0
    },
    /// Data refresh completed
    DataRefresh {
        data_type: String,
        success: bool,
        error: Option<String>,
    },
    /// Trigger simulation based on input changes
    TriggerSimulation,

    /// Retry swap with increased slippage tolerance
    RetryWithIncreasedSlippage,

    /// Show swap confirmation modal
    ShowSwapConfirmation,
}

/// Swap operation details for multi-hop swaps
#[derive(Debug, Clone, PartialEq)]
pub struct SwapOperation {
    pub from_asset: String,
    pub to_asset: String,
    pub pool_id: String,
    pub amount: String,
}

/// Wrapper for provide liquidity result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProvideResultWrapper {
    pub txhash: String,
    pub result: Option<String>,
    // Enhanced LP token information
    pub lp_tokens_received: Option<cosmwasm_std::Uint128>,
    pub lp_token_denom: Option<String>,
    pub pool_id: String,
    pub user_lp_balance_after: Option<cosmwasm_std::Uint128>,
    pub pool_total_supply: Option<cosmwasm_std::Uint128>,
}

/// Event handler for processing terminal events
pub struct EventHandler {
    /// Receiver for events
    receiver: mpsc::UnboundedReceiver<Event>,
    /// Sender for events (for custom events)
    sender: mpsc::UnboundedSender<Event>,
    /// Handle for the background terminal event processing task
    _terminal_task: tokio::task::JoinHandle<()>,
}

/// Async blockchain processor for handling blockchain operations
pub struct AsyncBlockchainProcessor {
    /// Event sender to communicate with the main event loop
    event_sender: mpsc::UnboundedSender<Event>,
    /// Optional client reference for real blockchain operations
    client: Option<std::sync::Arc<crate::protocols::dex::MantraDexClient>>,
}

impl AsyncBlockchainProcessor {
    /// Create a new async blockchain processor with event sender only
    pub fn new(event_sender: mpsc::UnboundedSender<Event>) -> Self {
        Self {
            event_sender,
            client: None,
        }
    }

    /// Create a new async blockchain processor with both event sender and client
    pub fn with_client(
        event_sender: mpsc::UnboundedSender<Event>,
        client: std::sync::Arc<crate::protocols::dex::MantraDexClient>,
    ) -> Self {
        Self {
            event_sender,
            client: Some(client),
        }
    }

    /// Execute a swap operation asynchronously
    pub async fn execute_swap(
        &self,
        from_asset: String,
        to_asset: String,
        amount: String,
        _pool_id: Option<String>,
        _slippage_tolerance: Option<String>,
    ) {
        let operation = "swap".to_string();

        // Send progress event
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Initiating swap...".to_string(),
            progress: Some(0.1),
        });

        // TODO: Replace with actual SDK call
        // For now, simulate async operation
        tokio::time::sleep(Duration::from_millis(500)).await;

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Broadcasting transaction...".to_string(),
            progress: Some(0.5),
        });

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Confirming transaction...".to_string(),
            progress: Some(0.8),
        });

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate success/failure (in real implementation, check actual SDK result)
        let success = true; // TODO: Replace with actual SDK call result

        if success {
            let _ = self.event_sender.send(Event::BlockchainSuccess {
                operation: operation.clone(),
                result: format!(
                    "Swapped {} {} for {} {}",
                    amount, from_asset, "calculated_amount", to_asset
                ),
                transaction_hash: Some("0x1234567890abcdef...".to_string()),
                enhanced_data: None,
            });
        } else {
            let _ = self.event_sender.send(Event::BlockchainError {
                operation: operation.clone(),
                error: "Insufficient liquidity in pool".to_string(),
            });
        }
    }

    /// Provide liquidity to a pool asynchronously
    pub async fn provide_liquidity(
        &self,
        pool_id: String,
        asset_1_amount: String,
        asset_2_amount: String,
        slippage_tolerance: Option<String>,
    ) {
        let operation = "provide_liquidity".to_string();

        // Send initial progress
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Preparing liquidity transaction...".to_string(),
            progress: Some(0.1),
        });

        // Get the client from the shared state (we'll need to implement this)
        // For now, we'll simulate the actual call structure
        let result = self
            .execute_provide_liquidity_transaction(
                pool_id.clone(),
                asset_1_amount,
                asset_2_amount,
                slippage_tolerance,
            )
            .await;

        match result {
            Ok(tx_response) => {
                // Create enhanced success message with LP token details
                let mut success_message =
                    format!("Successfully provided liquidity to pool {}", pool_id);

                // Add LP token information if available
                if let Some(lp_amount) = &tx_response.lp_tokens_received {
                    let lp_display = self.format_token_amount(lp_amount, 6); // LP tokens typically use 6 decimals
                    success_message.push_str(&format!(". LP tokens received: {}", lp_display));
                }

                // Send enhanced success event with LP token details
                let tx_hash = tx_response.txhash.clone();
                let _ = self.event_sender.send(Event::BlockchainSuccess {
                    operation: operation.clone(),
                    result: success_message,
                    transaction_hash: Some(tx_hash),
                    // Pass the enhanced result data as JSON string in the result field
                    enhanced_data: Some(serde_json::to_string(&tx_response).unwrap_or_default()),
                });
            }
            Err(e) => {
                let _ = self.event_sender.send(Event::BlockchainError {
                    operation: operation.clone(),
                    error: format!("Failed to provide liquidity: {}", e),
                });
            }
        }
    }

    /// Format token amount for display (convert from micro units)
    fn format_token_amount(&self, amount: &cosmwasm_std::Uint128, decimals: u8) -> String {
        let amount_f64 = amount.u128() as f64 / 10_f64.powi(decimals as i32);
        format!("{:.6}", amount_f64)
    }

    /// Execute the actual provide liquidity transaction using the SDK client
    async fn execute_provide_liquidity_transaction(
        &self,
        pool_id: String,
        asset_1_amount: String,
        asset_2_amount: String,
        slippage_tolerance: Option<String>,
    ) -> Result<ProvideResultWrapper, String> {
        use cosmwasm_std::{Coin, Decimal, Uint128};
        use std::str::FromStr;

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "provide_liquidity".to_string(),
            status: "Converting amounts and denominations...".to_string(),
            progress: Some(0.2),
        });

        // Get the actual asset denominations from the selected pool's info
        let (denom_1, denom_2) = self.get_pool_denominations_from_cache(&pool_id).await?;

        // Convert decimal amounts to micro amounts based on asset type
        let amount_1_micro = self.convert_to_micro_amount(&asset_1_amount, &denom_1)?;
        let amount_2_micro = self.convert_to_micro_amount(&asset_2_amount, &denom_2)?;

        crate::tui_dex::utils::logger::log_info(&format!(
            "Converted amounts: {} {} -> {} {}, {} {} -> {} {}",
            asset_1_amount,
            denom_1,
            amount_1_micro,
            denom_1,
            asset_2_amount,
            denom_2,
            amount_2_micro,
            denom_2
        ));

        // Parse slippage tolerance (convert from percentage to decimal)
        let slippage = if let Some(slippage_str) = slippage_tolerance {
            let slippage_percent = slippage_str
                .parse::<f64>()
                .map_err(|e| format!("Invalid slippage percentage: {}", e))?;

            // Convert percentage to decimal (1.0% -> 0.01)
            let slippage_decimal = slippage_percent / 100.0;
            Some(
                Decimal::from_str(&slippage_decimal.to_string())
                    .map_err(|e| format!("Invalid slippage decimal conversion: {}", e))?,
            )
        } else {
            None
        };

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "provide_liquidity".to_string(),
            status: "Preparing assets for liquidity provision...".to_string(),
            progress: Some(0.4),
        });

        // Create the assets vector with correct denominations and micro amounts
        let assets = vec![
            Coin {
                denom: denom_1.clone(),
                amount: amount_1_micro,
            },
            Coin {
                denom: denom_2.clone(),
                amount: amount_2_micro,
            },
        ];

        crate::tui_dex::utils::logger::log_info(&format!(
            "Prepared assets for liquidity provision: {:?}",
            assets
        ));

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "provide_liquidity".to_string(),
            status: "Broadcasting transaction to blockchain...".to_string(),
            progress: Some(0.7),
        });

        // Execute actual blockchain transaction if client is available
        if let Some(client) = &self.client {
            // Get pool info before transaction to capture LP token denom and current supply
            let pool_info = client
                .get_pool(&pool_id)
                .await
                .map_err(|e| format!("Failed to get pool info before transaction: {}", e))?;

            let lp_token_denom = pool_info.pool_info.lp_denom.clone();
            let pool_total_supply_before = pool_info.total_share.amount;

            // Get user's LP balance before transaction (if wallet is configured)
            let user_lp_balance_before =
                if let Some(wallet_address) = client.get_wallet_address().await {
                    // Fetch user's LP token balance
                    match client.get_balance(&lp_token_denom).await {
                        Ok(balance) => balance.amount,
                        Err(_) => Uint128::zero(), // If balance query fails, assume zero
                    }
                } else {
                    Uint128::zero()
                };

            match client
                .provide_liquidity(&pool_id, assets, slippage, None)
                .await
            {
                Ok(tx_response) => {
                    // Send final progress update
                    let _ = self.event_sender.send(Event::BlockchainProgress {
                        operation: "provide_liquidity".to_string(),
                        status: "Transaction confirmed, processing results...".to_string(),
                        progress: Some(0.9),
                    });

                    crate::tui_dex::utils::logger::log_info(&format!(
                        "Liquidity provision successful! TX Hash: {}",
                        tx_response.txhash
                    ));

                    // Extract LP tokens received from transaction events
                    let lp_tokens_received =
                        self.extract_lp_tokens_from_events(&tx_response, &lp_token_denom);

                    // Get updated pool info and user balance after transaction
                    let (user_lp_balance_after, pool_total_supply_after) = self
                        .get_post_transaction_info(client, &pool_id, &lp_token_denom)
                        .await;

                    // Log the detailed information
                    crate::tui_dex::utils::logger::log_info(&format!(
                        "LP Token Details - Received: {:?}, LP Denom: {}, User Balance Before: {}, User Balance After: {:?}, Pool Total Supply After: {:?}",
                        lp_tokens_received, lp_token_denom, user_lp_balance_before, user_lp_balance_after, pool_total_supply_after
                    ));

                    Ok(ProvideResultWrapper {
                        txhash: tx_response.txhash,
                        result: Some(format!(
                            "LP tokens received (check transaction for details)"
                        )),
                        lp_tokens_received,
                        lp_token_denom: Some(lp_token_denom),
                        pool_id: pool_id.clone(),
                        user_lp_balance_after,
                        pool_total_supply: pool_total_supply_after,
                    })
                }
                Err(e) => {
                    crate::tui_dex::utils::logger::log_error(&format!(
                        "Blockchain transaction failed: {}",
                        e
                    ));
                    Err(format!("Blockchain transaction failed: {}", e))
                }
            }
        } else {
            // Fallback to mock implementation when no client is available
            crate::tui_dex::utils::logger::log_warning(
                "No client available, using mock implementation",
            );

            // Simulate network delay
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

            // Send final progress update
            let _ = self.event_sender.send(Event::BlockchainProgress {
                operation: "provide_liquidity".to_string(),
                status: "Transaction confirmed, processing results...".to_string(),
                progress: Some(0.9),
            });

            // Return mock success
            Ok(ProvideResultWrapper {
                txhash: format!("mantra{}", chrono::Utc::now().timestamp()),
                result: Some("Mock LP tokens (no real client connected)".to_string()),
                lp_tokens_received: Some(Uint128::new(1000000)), // Mock 1 LP token
                lp_token_denom: Some(format!("factory/contract/{}/lp", pool_id)),
                pool_id: pool_id.clone(),
                user_lp_balance_after: Some(Uint128::new(1000000)), // Mock balance
                pool_total_supply: Some(Uint128::new(100000000)),   // Mock total supply
            })
        }
    }

    /// Extract LP tokens received from transaction events
    fn extract_lp_tokens_from_events(
        &self,
        tx_response: &cosmrs::proto::cosmos::base::abci::v1beta1::TxResponse,
        lp_token_denom: &str,
    ) -> Option<cosmwasm_std::Uint128> {
        // Parse transaction events to find LP token mints or transfers
        for event in &tx_response.events {
            if event.r#type == "coin_received" || event.r#type == "transfer" {
                let mut found_lp_denom = false;
                let mut amount = None;

                for attr in &event.attributes {
                    match attr.key.as_str() {
                        "denom" if attr.value == lp_token_denom => {
                            found_lp_denom = true;
                        }
                        "amount" if found_lp_denom => {
                            if let Ok(parsed_amount) = attr.value.parse::<u128>() {
                                amount = Some(cosmwasm_std::Uint128::new(parsed_amount));
                            }
                        }
                        _ => {}
                    }
                }

                if found_lp_denom && amount.is_some() {
                    return amount;
                }
            }

            // Also check wasm events for contract-specific LP token info
            if event.r#type == "wasm" {
                let mut lp_amount = None;

                for attr in &event.attributes {
                    if attr.key == "liquidity_token_amount" || attr.key == "lp_token_amount" {
                        if let Ok(parsed_amount) = attr.value.parse::<u128>() {
                            lp_amount = Some(cosmwasm_std::Uint128::new(parsed_amount));
                        }
                    }
                }

                if lp_amount.is_some() {
                    return lp_amount;
                }
            }
        }

        // If we couldn't extract from events, log and return None
        crate::tui_dex::utils::logger::log_warning(&format!(
            "Could not extract LP tokens received from transaction events for denom: {}",
            lp_token_denom
        ));
        None
    }

    /// Get post-transaction information (user balance and pool total supply)
    async fn get_post_transaction_info(
        &self,
        client: &std::sync::Arc<crate::protocols::dex::MantraDexClient>,
        pool_id: &str,
        lp_token_denom: &str,
    ) -> (Option<cosmwasm_std::Uint128>, Option<cosmwasm_std::Uint128>) {
        // Get updated pool info for total supply
        let pool_total_supply = match client.get_pool(pool_id).await {
            Ok(pool_info) => Some(pool_info.total_share.amount),
            Err(e) => {
                crate::tui_dex::utils::logger::log_warning(&format!(
                    "Failed to get updated pool info: {}",
                    e
                ));
                None
            }
        };

        // Get user's updated LP balance
        let user_lp_balance = if let Some(wallet_address) = client.get_wallet_address().await {
            match client.get_balance(lp_token_denom).await {
                Ok(balance) => Some(balance.amount),
                Err(e) => {
                    crate::tui_dex::utils::logger::log_warning(&format!(
                        "Failed to get user LP balance: {}",
                        e
                    ));
                    None
                }
            }
        } else {
            None
        };

        (user_lp_balance, pool_total_supply)
    }

    /// Get pool denominations from pool ID
    async fn get_pool_denominations_from_cache(
        &self,
        pool_id: &str,
    ) -> Result<(String, String), String> {
        // Query the pool from the blockchain to get the actual asset denominations
        // This ensures we always have the correct, up-to-date denominations
        if let Some(client) = &self.client {
            match client.get_pool(pool_id).await {
                Ok(pool_info) => {
                    let assets = &pool_info.pool_info.assets;
                    if assets.len() >= 2 {
                        let denom_1 = assets[0].denom.clone();
                        let denom_2 = assets[1].denom.clone();

                        crate::tui_dex::utils::logger::log_info(&format!(
                            "Pool {} denominations from blockchain: {} and {}",
                            pool_id, denom_1, denom_2
                        ));

                        Ok((denom_1, denom_2))
                    } else {
                        Err(format!(
                            "Pool {} does not have enough assets (found {})",
                            pool_id,
                            assets.len()
                        ))
                    }
                }
                Err(e) => {
                    crate::tui_dex::utils::logger::log_error(&format!(
                        "Failed to query pool {} from blockchain: {:?}",
                        pool_id, e
                    ));
                    Err(format!("Failed to query pool {}: {:?}", pool_id, e))
                }
            }
        } else {
            Err("No blockchain client available to query pool denominations".to_string())
        }
    }

    /// Convert human-readable amount to micro amount
    /// Uses standard decimal places for Mantra network tokens
    fn convert_to_micro_amount(
        &self,
        amount_str: &str,
        denom: &str,
    ) -> Result<cosmwasm_std::Uint128, String> {
        let amount_f64 = amount_str
            .parse::<f64>()
            .map_err(|e| format!("Invalid amount format: {}", e))?;

        // Most tokens on Mantra use 6 decimals as the standard
        // This includes OM (uom), USDC (factory tokens), and most other assets
        let decimals = self.get_token_decimals_for_denom(denom);

        // Convert to micro amount
        let micro_amount = (amount_f64 * 10_f64.powi(decimals as i32)) as u128;

        crate::tui_dex::utils::logger::log_debug(&format!(
            "Amount conversion: {} {} -> {} micro units (10^{})",
            amount_str, denom, micro_amount, decimals
        ));

        Ok(cosmwasm_std::Uint128::new(micro_amount))
    }

    /// Get the number of decimal places for a given denomination
    /// Most Mantra network tokens use 6 decimals
    fn get_token_decimals_for_denom(&self, denom: &str) -> u8 {
        match denom {
            // Native OM token
            "uom" => 6,
            // Factory tokens (USDC, USDT, etc.) typically use 6 decimals
            d if d.starts_with("factory/") => {
                // Could be enhanced to read from config in the future
                // For now, use the standard 6 decimals for all factory tokens
                6
            }
            // IBC tokens - most use 6 decimals but could vary
            d if d.starts_with("ibc/") => {
                // Could be enhanced to maintain an IBC token registry
                // For now, default to 6 decimals
                6
            }
            // Default case
            _ => 6,
        }
    }

    /// Withdraw liquidity from a pool asynchronously
    pub async fn withdraw_liquidity(
        &self,
        pool_id: String,
        lp_token_amount: String,
        slippage_tolerance: Option<String>,
    ) {
        let operation = "withdraw_liquidity".to_string();

        // Send initial progress
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Preparing liquidity withdrawal...".to_string(),
            progress: Some(0.1),
        });

        let result = self
            .execute_withdraw_liquidity_transaction(
                pool_id.clone(),
                lp_token_amount,
                slippage_tolerance,
            )
            .await;

        match result {
            Ok(tx_response) => {
                let _ = self.event_sender.send(Event::BlockchainSuccess {
                    operation: operation.clone(),
                    result: format!(
                        "Successfully withdrew liquidity from pool {}. Assets received: {}",
                        pool_id,
                        tx_response.result.unwrap_or_default()
                    ),
                    transaction_hash: Some(tx_response.txhash),
                    enhanced_data: None, // No enhanced data for withdraw operations yet
                });
            }
            Err(e) => {
                let _ = self.event_sender.send(Event::BlockchainError {
                    operation: operation.clone(),
                    error: format!("Failed to withdraw liquidity: {}", e),
                });
            }
        }
    }

    /// Execute the actual withdraw liquidity transaction using the SDK client
    async fn execute_withdraw_liquidity_transaction(
        &self,
        pool_id: String,
        lp_token_amount: String,
        _slippage_tolerance: Option<String>,
    ) -> Result<ProvideResultWrapper, String> {
        use cosmwasm_std::Uint128;
        use std::str::FromStr;

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "withdraw_liquidity".to_string(),
            status: "Converting LP token amount...".to_string(),
            progress: Some(0.2),
        });

        // Parse LP token amount
        let lp_amount = Uint128::from_str(&lp_token_amount)
            .map_err(|e| format!("Invalid LP token amount: {}", e))?;

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "withdraw_liquidity".to_string(),
            status: "Calculating withdrawal amounts...".to_string(),
            progress: Some(0.4),
        });

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "withdraw_liquidity".to_string(),
            status: "Broadcasting withdrawal transaction...".to_string(),
            progress: Some(0.7),
        });

        // Execute actual blockchain transaction if client is available
        if let Some(client) = &self.client {
            match client.withdraw_liquidity(&pool_id, lp_amount).await {
                Ok(tx_response) => {
                    // Send final progress update
                    let _ = self.event_sender.send(Event::BlockchainProgress {
                        operation: "withdraw_liquidity".to_string(),
                        status: "Transaction confirmed, processing results...".to_string(),
                        progress: Some(0.9),
                    });

                    Ok(ProvideResultWrapper {
                        txhash: tx_response.txhash,
                        result: Some(format!("Assets withdrawn (check transaction for details)")),
                        lp_tokens_received: None,
                        lp_token_denom: None,
                        pool_id: pool_id.clone(),
                        user_lp_balance_after: None,
                        pool_total_supply: None,
                    })
                }
                Err(e) => Err(format!("Blockchain transaction failed: {}", e)),
            }
        } else {
            // Fallback to mock implementation when no client is available
            crate::tui_dex::utils::logger::log_warning(
                "No client available, using mock implementation",
            );

            // Simulate network delay
            tokio::time::sleep(std::time::Duration::from_millis(1200)).await;

            // Send final progress update
            let _ = self.event_sender.send(Event::BlockchainProgress {
                operation: "withdraw_liquidity".to_string(),
                status: "Transaction confirmed, processing results...".to_string(),
                progress: Some(0.9),
            });

            // Return mock success
            Ok(ProvideResultWrapper {
                txhash: format!("mantra{}", chrono::Utc::now().timestamp() + 100),
                result: Some("Mock assets (no real client connected)".to_string()),
                lp_tokens_received: None,
                lp_token_denom: None,
                pool_id: pool_id.clone(),
                user_lp_balance_after: None,
                pool_total_supply: None,
            })
        }
    }

    /// Claim rewards asynchronously
    pub async fn claim_rewards(
        &self,
        pool_id: Option<String>,
        _epochs: Option<Vec<u64>>,
        claim_all: bool,
    ) {
        let operation = "claim_rewards".to_string();

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Calculating claimable rewards...".to_string(),
            progress: Some(0.4),
        });

        tokio::time::sleep(Duration::from_millis(400)).await;

        let success = true; // TODO: Replace with actual SDK call result

        if success {
            let result = if claim_all {
                "Claimed all available rewards".to_string()
            } else if let Some(pool) = pool_id {
                format!("Claimed rewards from pool {}", pool)
            } else {
                "Claimed rewards for specified epochs".to_string()
            };

            let _ = self.event_sender.send(Event::BlockchainSuccess {
                operation: operation.clone(),
                result,
                transaction_hash: Some("0x1111222233334444...".to_string()),
                enhanced_data: None, // No enhanced data for rewards operations yet
            });
        } else {
            let _ = self.event_sender.send(Event::BlockchainError {
                operation: operation.clone(),
                error: "No rewards available to claim".to_string(),
            });
        }
    }

    /// Create a new pool asynchronously
    pub async fn create_pool(
        &self,
        asset_1: String,
        asset_2: String,
        swap_fee: String,
        exit_fee: String,
        pool_features: Vec<String>,
    ) {
        let operation = "create_pool".to_string();

        // Send initial progress
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Preparing pool creation...".to_string(),
            progress: Some(0.1),
        });

        let result = self
            .execute_create_pool_transaction(
                asset_1.clone(),
                asset_2.clone(),
                swap_fee,
                exit_fee,
                pool_features,
            )
            .await;

        match result {
            Ok(tx_response) => {
                let _ = self.event_sender.send(Event::BlockchainSuccess {
                    operation: operation.clone(),
                    result: format!("Successfully created pool for {} / {}", asset_1, asset_2),
                    transaction_hash: Some(tx_response.txhash),
                    enhanced_data: Some(tx_response.result.unwrap_or_default()),
                });
            }
            Err(e) => {
                let _ = self.event_sender.send(Event::BlockchainError {
                    operation: operation.clone(),
                    error: format!("Failed to create pool: {}", e),
                });
            }
        }
    }

    /// Execute the actual create pool transaction using the SDK client
    async fn execute_create_pool_transaction(
        &self,
        asset_1: String,
        asset_2: String,
        swap_fee: String,
        _exit_fee: String,
        pool_features: Vec<String>,
    ) -> Result<ProvideResultWrapper, String> {
        use cosmwasm_std::Decimal;
        use std::str::FromStr;

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "create_pool".to_string(),
            status: "Parsing pool parameters...".to_string(),
            progress: Some(0.2),
        });

        // Parse swap fee
        let swap_fee_decimal =
            Decimal::from_str(&swap_fee).map_err(|e| format!("Invalid swap fee: {}", e))?;

        // Parse protocol fee from pool features
        let mut protocol_fee_decimal = Decimal::zero();
        let mut pool_type_str = "ConstantProduct".to_string();

        for feature in &pool_features {
            if let Some(protocol_fee_str) = feature.strip_prefix("protocol_fee:") {
                protocol_fee_decimal = Decimal::from_str(protocol_fee_str)
                    .map_err(|e| format!("Invalid protocol fee: {}", e))?;
            } else if let Some(pool_type_val) = feature.strip_prefix("pool_type:") {
                pool_type_str = pool_type_val.to_string();
            }
        }

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "create_pool".to_string(),
            status: "Preparing pool configuration...".to_string(),
            progress: Some(0.4),
        });

        // Create pool fees structure
        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: protocol_fee_decimal,
            },
            swap_fee: mantra_dex_std::fee::Fee {
                share: swap_fee_decimal,
            },
            burn_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::zero(),
            },
            extra_fees: vec![], // No extra fees for basic pool creation
        };

        // Create pool type
        let pool_type = match pool_type_str.as_str() {
            "ConstantProduct" => mantra_dex_std::pool_manager::PoolType::ConstantProduct,
            "StableSwap" => mantra_dex_std::pool_manager::PoolType::StableSwap {
                amp: 100, // Default amplification parameter
            },
            _ => mantra_dex_std::pool_manager::PoolType::ConstantProduct,
        };

        // Send progress update
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: "create_pool".to_string(),
            status: "Broadcasting pool creation transaction...".to_string(),
            progress: Some(0.7),
        });

        // Execute actual blockchain transaction if client is available
        if let Some(client) = &self.client {
            crate::tui_dex::utils::logger::log_info(&format!(
                "Creating pool with assets: {} / {}, swap fee: {}, protocol fee: {}, pool type: {}",
                asset_1, asset_2, swap_fee, protocol_fee_decimal, pool_type_str
            ));

            match client
                .create_pool(
                    vec![asset_1.clone(), asset_2.clone()],
                    vec![6, 6], // Default to 6 decimals for both assets
                    pool_fees,
                    pool_type,
                    None, // No custom pool identifier
                )
                .await
            {
                Ok(tx_response) => {
                    // Send final progress update
                    let _ = self.event_sender.send(Event::BlockchainProgress {
                        operation: "create_pool".to_string(),
                        status: "Transaction confirmed, pool created successfully!".to_string(),
                        progress: Some(0.9),
                    });

                    crate::tui_dex::utils::logger::log_info(&format!(
                        "Pool creation successful! TX Hash: {}",
                        tx_response.txhash
                    ));

                    Ok(ProvideResultWrapper {
                        txhash: tx_response.txhash,
                        result: Some(format!(
                            "Pool created for {} / {} with swap fee {}%",
                            asset_1, asset_2, swap_fee
                        )),
                        lp_tokens_received: None,
                        lp_token_denom: None,
                        pool_id: "new_pool".to_string(), // Pool ID will be in transaction events
                        user_lp_balance_after: None,
                        pool_total_supply: None,
                    })
                }
                Err(e) => {
                    crate::tui_dex::utils::logger::log_error(&format!(
                        "Pool creation transaction failed: {}",
                        e
                    ));
                    Err(format!("Pool creation transaction failed: {}", e))
                }
            }
        } else {
            // Fallback to mock implementation when no client is available
            crate::tui_dex::utils::logger::log_warning(
                "No client available, using mock implementation for pool creation",
            );

            // Simulate network delay
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

            // Send final progress update
            let _ = self.event_sender.send(Event::BlockchainProgress {
                operation: "create_pool".to_string(),
                status: "Transaction confirmed, pool created successfully!".to_string(),
                progress: Some(0.9),
            });

            // Return mock success
            Ok(ProvideResultWrapper {
                txhash: format!("mantra{}", chrono::Utc::now().timestamp()),
                result: Some(format!(
                    "Mock pool created for {} / {} (no real client connected)",
                    asset_1, asset_2
                )),
                lp_tokens_received: None,
                lp_token_denom: None,
                pool_id: "mock_pool".to_string(),
                user_lp_balance_after: None,
                pool_total_supply: None,
            })
        }
    }

    /// Refresh data from blockchain
    pub async fn refresh_data(&self, data_type: String) {
        tokio::time::sleep(Duration::from_millis(300)).await;

        let success = true; // TODO: Replace with actual SDK call result (90% success rate in real implementation)

        let _ = self.event_sender.send(Event::DataRefresh {
            data_type: data_type.clone(),
            success,
            error: if success {
                None
            } else {
                Some(format!("Failed to refresh {}", data_type))
            },
        });
    }
}

impl EventHandler {
    /// Create a new event handler
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        // Spawn a task to handle terminal events
        let event_sender = sender.clone();
        let terminal_task = tokio::spawn(async move {
            loop {
                // Poll for events with a timeout to avoid blocking
                if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    if let Ok(terminal_event) = event::read() {
                        if let Some(app_event) = Self::convert_terminal_event(terminal_event) {
                            if event_sender.send(app_event).is_err() {
                                break; // Channel closed, exit the loop
                            }
                        }
                    }
                }

                // Small delay to prevent high CPU usage
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        Self {
            receiver,
            sender,
            _terminal_task: terminal_task,
        }
    }

    /// Get the next event
    pub async fn next(&mut self) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| "Event channel closed".into())
    }

    /// Convert a terminal event to an application event
    fn convert_terminal_event(terminal_event: event::Event) -> Option<Event> {
        match terminal_event {
            event::Event::Key(key_event) => Self::convert_key_event(key_event),
            event::Event::Mouse(_) => Some(Event::Mouse),
            event::Event::Resize(_, _) => None, // Handle resize events if needed
            event::Event::Paste(data) => Some(Event::Paste(data)),
            _ => None,
        }
    }

    /// Convert a key event to an application event
    fn convert_key_event(key_event: KeyEvent) -> Option<Event> {
        match key_event {
            // Quit events - removed automatic 'q' conversion to prevent interference with text input
            // 'q' will be handled by the application based on context
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(Event::Quit),

            // Tab navigation - Enhanced for focus management
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Tab), // Screen navigation

            KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(Event::BackTab), // Reverse screen navigation

            // Action keys
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Enter),

            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Escape),

            // Arrow keys - Enhanced for directional focus movement
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::MoveFocus(FocusDirection::Up)),

            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::MoveFocus(FocusDirection::Down)),

            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::MoveFocus(FocusDirection::Left)),

            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::MoveFocus(FocusDirection::Right)),

            // Navigation keys with enhanced focus support
            KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::FocusFirst),

            KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::FocusLast),

            // Space bar for context-sensitive actions
            KeyEvent {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::ContextAction),

            // Editing keys
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Backspace),

            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Delete),

            KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::PageUp),

            KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::PageDown),

            KeyEvent {
                code: KeyCode::Insert,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Insert),

            // Function keys with enhanced shortcuts
            KeyEvent {
                code: KeyCode::F(n),
                modifiers: KeyModifiers::NONE,
                ..
            } => match n {
                1 => Some(Event::Help),
                5 => Some(Event::Refresh),
                _ => Some(Event::F(n)),
            },

            // Enhanced keyboard shortcuts for accessibility
            // Ctrl+Home/End for navigation
            KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(Event::FocusFirst),

            KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(Event::FocusLast),

            // Character input - including shifted characters
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Char(c)),

            // Shift + character combinations (for uppercase letters and symbols)
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(Event::Char(c)),

            // Ctrl + character combinations
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Skip 'c' since it's already handled as quit
                if c != 'c' {
                    Some(Event::Ctrl(c))
                } else {
                    None
                }
            }

            // Alt + character combinations
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::ALT,
                ..
            } => Some(Event::Alt(c)),

            // Ignore other key combinations
            _ => None,
        }
    }

    /// Send a custom event
    pub fn send_custom_event(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        self.sender.send(event)
    }

    /// Get a clone of the event sender for use in background tasks
    pub fn get_sender(&self) -> mpsc::UnboundedSender<Event> {
        self.sender.clone()
    }

    /// Handle crossterm events and convert them to application events
    pub fn handle_crossterm_event(
        &self,
        crossterm_event: crossterm::event::Event,
    ) -> Option<Event> {
        Self::convert_terminal_event(crossterm_event)
    }

    /// Get an async blockchain processor for performing blockchain operations
    pub fn get_blockchain_processor(&self) -> AsyncBlockchainProcessor {
        AsyncBlockchainProcessor::new(self.sender.clone())
    }

    /// Process a DEX action event by spawning an async operation
    pub fn process_action_event(&self, event: Event) {
        let processor = self.get_blockchain_processor();

        match event {
            Event::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                pool_id,
                slippage_tolerance,
            } => {
                tokio::spawn(async move {
                    processor
                        .execute_swap(from_asset, to_asset, amount, pool_id, slippage_tolerance)
                        .await;
                });
            }
            Event::ProvideLiquidity {
                pool_id,
                asset_1_amount,
                asset_2_amount,
                slippage_tolerance,
            } => {
                tokio::spawn(async move {
                    processor
                        .provide_liquidity(
                            pool_id,
                            asset_1_amount,
                            asset_2_amount,
                            slippage_tolerance,
                        )
                        .await;
                });
            }
            Event::WithdrawLiquidity {
                pool_id,
                lp_token_amount,
                slippage_tolerance,
            } => {
                tokio::spawn(async move {
                    processor
                        .withdraw_liquidity(pool_id, lp_token_amount, slippage_tolerance)
                        .await;
                });
            }
            Event::ClaimRewards {
                pool_id,
                epochs,
                claim_all,
            } => {
                tokio::spawn(async move {
                    processor.claim_rewards(pool_id, epochs, claim_all).await;
                });
            }
            Event::CreatePool {
                asset_1,
                asset_2,
                swap_fee,
                exit_fee,
                pool_features,
            } => {
                tokio::spawn(async move {
                    processor
                        .create_pool(asset_1, asset_2, swap_fee, exit_fee, pool_features)
                        .await;
                });
            }
            Event::Refresh => {
                tokio::spawn(async move {
                    processor.refresh_data("all".to_string()).await;
                });
            }
            _ => {
                // Send the event directly for non-blockchain actions
                let _ = self.send_custom_event(event);
            }
        }
    }

    /// Check if an event is a blockchain action that requires async processing
    pub fn is_blockchain_action(event: &Event) -> bool {
        matches!(
            event,
            Event::ExecuteSwap { .. }
                | Event::ExecuteSwapAsync { .. }
                | Event::ProvideLiquidity { .. }
                | Event::WithdrawLiquidity { .. }
                | Event::ClaimRewards { .. }
                | Event::ExecuteMultiHopSwap { .. }
                | Event::CreatePool { .. }
                | Event::UpdatePoolFeatures { .. }
                | Event::SimulateSwap { .. }
                | Event::SimulateLiquidity { .. }
        )
    }

    /// Check if an event is a blockchain response
    pub fn is_blockchain_response(event: &Event) -> bool {
        matches!(
            event,
            Event::BlockchainSuccess { .. }
                | Event::BlockchainError { .. }
                | Event::BlockchainProgress { .. }
                | Event::DataRefresh { .. }
        )
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_conversion() {
        // Test quit events
        let quit_q = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(
            EventHandler::convert_key_event(quit_q),
            Some(Event::Char('q'))
        );

        let quit_ctrl_c = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(
            EventHandler::convert_key_event(quit_ctrl_c),
            Some(Event::Quit)
        );

        // Test navigation
        let tab = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(EventHandler::convert_key_event(tab), Some(Event::Tab));

        // Test character input
        let char_a = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(
            EventHandler::convert_key_event(char_a),
            Some(Event::Char('a'))
        );
    }

    #[test]
    fn test_blockchain_action_detection() {
        // Test DEX action events
        let swap_event = Event::ExecuteSwap {
            from_asset: "USDC".to_string(),
            to_asset: "OM".to_string(),
            amount: "100".to_string(),
            pool_id: Some("o.uom.usdc.pool".to_string()),
            slippage_tolerance: Some("0.01".to_string()),
        };
        assert!(EventHandler::is_blockchain_action(&swap_event));

        let liquidity_event = Event::ProvideLiquidity {
            pool_id: "1".to_string(),
            asset_1_amount: "100".to_string(),
            asset_2_amount: "50".to_string(),
            slippage_tolerance: Some("0.01".to_string()),
        };
        assert!(EventHandler::is_blockchain_action(&liquidity_event));

        // Test non-blockchain events
        let quit_event = Event::Quit;
        assert!(!EventHandler::is_blockchain_action(&quit_event));

        let char_event = Event::Char('a');
        assert!(!EventHandler::is_blockchain_action(&char_event));
    }

    #[test]
    fn test_blockchain_response_detection() {
        // Test blockchain response events
        let success_event = Event::BlockchainSuccess {
            operation: "swap".to_string(),
            result: "Success".to_string(),
            transaction_hash: Some("0x123".to_string()),
            enhanced_data: None,
        };
        assert!(EventHandler::is_blockchain_response(&success_event));

        let error_event = Event::BlockchainError {
            operation: "swap".to_string(),
            error: "Failed".to_string(),
        };
        assert!(EventHandler::is_blockchain_response(&error_event));

        let progress_event = Event::BlockchainProgress {
            operation: "swap".to_string(),
            status: "In progress".to_string(),
            progress: Some(0.5),
        };
        assert!(EventHandler::is_blockchain_response(&progress_event));

        // Test non-response events
        let quit_event = Event::Quit;
        assert!(!EventHandler::is_blockchain_response(&quit_event));
    }

    #[test]
    fn test_swap_operation() {
        let swap_op = SwapOperation {
            from_asset: "USDC".to_string(),
            to_asset: "OM".to_string(),
            pool_id: "1".to_string(),
            amount: "100".to_string(),
        };

        assert_eq!(swap_op.from_asset, "USDC");
        assert_eq!(swap_op.to_asset, "OM");
        assert_eq!(swap_op.pool_id, "1");
        assert_eq!(swap_op.amount, "100");
    }

    #[tokio::test]
    async fn test_async_blockchain_processor() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let processor = AsyncBlockchainProcessor::new(sender);

        // Test that async operations send events
        tokio::spawn(async move {
            processor.refresh_data("pools".to_string()).await;
        });

        // Should receive a DataRefresh event
        if let Some(event) = receiver.recv().await {
            match event {
                Event::DataRefresh { data_type, .. } => {
                    assert_eq!(data_type, "pools");
                }
                _ => panic!("Expected DataRefresh event"),
            }
        }
    }
}

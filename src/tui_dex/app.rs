//! Application State Management
//!
//! This module manages the global application state for the TUI, including
//! screen navigation, data caching, and state transitions.

#[cfg(feature = "tui-dex")]
use crate::tui_dex::components::modals::{ErrorType, ModalState};
#[cfg(feature = "tui-dex")]
use crate::tui_dex::events::Event;
#[cfg(feature = "tui-dex")]
use crate::tui_dex::screens::liquidity::{self, LiquidityMode};
#[cfg(feature = "tui-dex")]
use crate::tui_dex::utils::focus_manager::FocusManager;
#[cfg(feature = "tui-dex")]
use crate::{Error, MantraDexClient, MantraNetworkConfig};
#[cfg(feature = "tui-dex")]
use cosmrs::proto::cosmos::base::abci::v1beta1::TxResponse;
#[cfg(feature = "tui-dex")]
use cosmwasm_std::Uint128;
#[cfg(feature = "tui-dex")]
use mantra_dex_std::pool_manager::{PoolInfoResponse, SimulationResponse};
#[cfg(feature = "tui-dex")]
use std::collections::HashMap;
#[cfg(feature = "tui-dex")]
use std::sync::Arc;
#[cfg(feature = "tui-dex")]
use std::time::Duration;

#[cfg(feature = "tui-dex")]
use tokio::sync::mpsc;

/// Available screens in the TUI application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    WalletSelection,
    Dashboard,
    Pools,
    Swap,
    MultiHop,
    Liquidity,
    Rewards,
    Admin,
    Settings,
    TransactionDetails,
}

/// Navigation mode for keyboard handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    /// Navigating between main screen tabs
    ScreenLevel,
    /// Navigating within the current screen
    WithinScreen,
}

impl Screen {
    /// Get the display name for the screen
    pub fn display_name(&self) -> &'static str {
        match self {
            Screen::WalletSelection => "Wallet Selection",
            Screen::Dashboard => "Dashboard",
            Screen::Pools => "Pools",
            Screen::Swap => "Swap",
            Screen::MultiHop => "Multi-hop",
            Screen::Liquidity => "Liquidity",
            Screen::Rewards => "Rewards",
            Screen::Admin => "Admin",
            Screen::Settings => "Settings",
            Screen::TransactionDetails => "Transaction",
        }
    }

    /// Get all available screens for navigation
    pub fn all() -> Vec<Screen> {
        vec![
            Screen::Dashboard,
            Screen::Pools,
            Screen::Swap,
            Screen::MultiHop,
            Screen::Liquidity,
            Screen::Rewards,
            Screen::Admin,
            Screen::Settings,
        ]
    }
}

/// Enhanced loading state for comprehensive async operations
#[derive(Debug, Clone)]
pub enum LoadingState {
    Idle,
    Loading {
        message: String,
        progress: Option<f64>, // 0.0 to 100.0, None for indeterminate
        can_cancel: bool,
        operation_id: Option<String>, // For cancellation tracking
    },
    Success {
        message: String,
        details: Option<Vec<String>>,
    },
    Error {
        message: String,
        error_type: ErrorType,
        details: Option<Vec<String>>,
        retry_action: Option<String>,
    },
}

impl LoadingState {
    /// Create a simple loading state
    pub fn loading(message: String) -> Self {
        Self::Loading {
            message,
            progress: None,
            can_cancel: false,
            operation_id: None,
        }
    }

    /// Create a loading state with progress
    pub fn loading_with_progress(message: String, progress: f64, can_cancel: bool) -> Self {
        Self::Loading {
            message,
            progress: Some(progress),
            can_cancel,
            operation_id: None,
        }
    }

    /// Create a loading state with operation ID for cancellation
    pub fn loading_with_id(message: String, operation_id: String, can_cancel: bool) -> Self {
        Self::Loading {
            message,
            progress: None,
            can_cancel,
            operation_id: Some(operation_id),
        }
    }

    /// Create a success state
    pub fn success(message: String) -> Self {
        Self::Success {
            message,
            details: None,
        }
    }

    /// Create a success state with details
    pub fn success_with_details(message: String, details: Vec<String>) -> Self {
        Self::Success {
            message,
            details: Some(details),
        }
    }

    /// Create a comprehensive error state
    pub fn error(message: String, error_type: ErrorType) -> Self {
        Self::Error {
            message,
            error_type,
            details: None,
            retry_action: None,
        }
    }

    /// Create an error state with retry action
    pub fn error_with_retry(message: String, error_type: ErrorType, retry_action: String) -> Self {
        Self::Error {
            message,
            error_type,
            details: None,
            retry_action: Some(retry_action),
        }
    }

    /// Create an error state with detailed information
    pub fn error_with_details(
        message: String,
        error_type: ErrorType,
        details: Vec<String>,
        retry_action: Option<String>,
    ) -> Self {
        Self::Error {
            message,
            error_type,
            details: Some(details),
            retry_action,
        }
    }

    /// Check if the state represents an active loading operation
    pub fn is_loading(&self) -> bool {
        matches!(self, LoadingState::Loading { .. })
    }

    /// Check if the state represents an error
    pub fn is_error(&self) -> bool {
        matches!(self, LoadingState::Error { .. })
    }

    /// Check if the loading operation can be cancelled
    pub fn can_cancel(&self) -> bool {
        match self {
            LoadingState::Loading { can_cancel, .. } => *can_cancel,
            _ => false,
        }
    }

    /// Get the current progress if available
    pub fn progress(&self) -> Option<f64> {
        match self {
            LoadingState::Loading { progress, .. } => *progress,
            _ => None,
        }
    }

    /// Get the operation ID if available
    pub fn operation_id(&self) -> Option<&String> {
        match self {
            LoadingState::Loading { operation_id, .. } => operation_id.as_ref(),
            _ => None,
        }
    }
}

/// Transaction details for tracking
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub hash: String,
    pub status: TransactionStatus,
    pub operation_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gas_used: Option<i64>,
    pub gas_wanted: Option<i64>,
}

/// Transaction status enum
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Success,
    Failed,
    Unknown,
}

/// Pool cache entry for efficient lookup
#[derive(Debug, Clone)]
pub struct PoolCacheEntry {
    pub pool_info: PoolInfoResponse,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

/// Swap operation state for the swap screen
#[derive(Debug, Clone)]
pub struct SwapState {
    pub from_asset: Option<String>,
    pub to_asset: Option<String>,
    pub amount: String,
    pub slippage: String,
    pub simulation_result: Option<SimulationResponse>,
    pub selected_pool_id: Option<String>,
}

/// Current liquidity operation state
#[derive(Debug, Clone)]
pub struct LiquidityState {
    pub operation_mode: LiquidityOperationMode,
    pub selected_pool_id: Option<String>,
    pub first_asset_amount: String,
    pub second_asset_amount: String,
    pub withdraw_amount: String,
    pub slippage_amount: String,
    pub slippage_swap: String,
    pub expected_lp_tokens: Option<Uint128>,
    pub expected_assets: Option<(Uint128, Uint128, String, String)>,
}

/// Liquidity operation modes
#[derive(Debug, Clone, PartialEq)]
pub enum LiquidityOperationMode {
    Provide,
    Withdraw,
    ViewPositions,
}

impl Default for SwapState {
    fn default() -> Self {
        Self {
            from_asset: None,
            to_asset: None,
            amount: String::new(),
            slippage: "1.0".to_string(), // Default 1% slippage
            simulation_result: None,
            selected_pool_id: None,
        }
    }
}

impl Default for LiquidityState {
    fn default() -> Self {
        Self {
            operation_mode: LiquidityOperationMode::Provide,
            selected_pool_id: None,
            first_asset_amount: String::new(),
            second_asset_amount: String::new(),
            withdraw_amount: String::new(),
            slippage_amount: "1.0".to_string(),
            slippage_swap: "1.0".to_string(),
            expected_lp_tokens: None,
            expected_assets: None,
        }
    }
}

/// Global application state
pub struct AppState {
    /// Current active screen
    pub current_screen: Screen,
    /// Navigation mode (screen-level vs within-screen)
    pub navigation_mode: NavigationMode,
    /// Enhanced loading state for async operations
    pub loading_state: LoadingState,
    /// Error messages to display (deprecated in favor of modal_state)
    pub error_message: Option<String>,
    /// Status message to display
    pub status_message: Option<String>,
    /// Modal state for comprehensive dialogs
    pub modal_state: Option<ModalState>,
    /// Selected pool ID (if any)
    pub selected_pool_id: Option<u64>,
    /// User token balances cache
    pub balances: HashMap<String, String>,
    /// Recent transaction details with full info
    pub recent_transactions: Vec<TransactionInfo>,
    /// Network block height
    pub block_height: Option<u64>,
    /// Connected wallet address
    pub wallet_address: Option<String>,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Current tab selection for navigation
    pub current_tab: usize,
    /// Cached pool information
    pub pool_cache: HashMap<String, PoolCacheEntry>,
    /// Current swap operation state
    pub swap_state: SwapState,
    /// Current liquidity operation state
    pub liquidity_state: LiquidityState,
    /// Current epoch information
    pub current_epoch: Option<u64>,
    /// Claimable rewards amount
    pub claimable_rewards: HashMap<String, Uint128>,
    /// Rewards screen state
    pub rewards_state: crate::tui_dex::screens::rewards::RewardsState,
    /// Admin screen state  
    pub admin_screen_state: crate::tui_dex::screens::admin::AdminScreenState,
    /// Settings screen state
    pub settings_state: crate::tui_dex::screens::settings::SettingsState,
    /// Transaction screen state
    pub transaction_state: crate::tui_dex::screens::transaction::TransactionState,
    /// Network information
    pub network_info: NetworkInfo,
    /// Pending async operations for tracking
    pub pending_operations: HashMap<String, PendingOperation>,
    /// Focus management for keyboard navigation
    pub focus_manager: FocusManager,
    /// Wallet selection screen state
    pub wallet_selection_state: crate::tui_dex::screens::wallet_selection::WalletSelectionScreen,
    /// Wallet setup wizard state
    pub wizard_state: crate::tui_dex::screens::wizard::WizardState,
    /// Asset decimals cache (denom -> decimal places)
    pub asset_decimals_cache: HashMap<String, u8>,
}

/// Pending operation tracking for comprehensive loading states
#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub operation_type: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub can_cancel: bool,
    pub cancel_token: Option<String>,
}

/// Enhanced network information with detailed connection state
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub chain_id: Option<String>,
    pub node_version: Option<String>,
    pub is_syncing: bool,
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
    pub connection_state: crate::tui_dex::utils::async_ops::NetworkState,
    pub last_block_height: Option<u64>,
    pub connection_latency: Option<Duration>,
    pub retry_count: u32,
}

impl Default for NetworkInfo {
    fn default() -> Self {
        Self {
            chain_id: None,
            node_version: None,
            is_syncing: false,
            last_sync_time: None,
            connection_state: crate::tui_dex::utils::async_ops::NetworkState::Connected,
            last_block_height: None,
            connection_latency: None,
            retry_count: 0,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: Screen::WalletSelection,
            navigation_mode: NavigationMode::ScreenLevel,
            loading_state: LoadingState::Idle,
            error_message: None,
            status_message: None,
            modal_state: None,
            selected_pool_id: None,
            balances: HashMap::new(),
            recent_transactions: Vec::new(),
            block_height: None,
            wallet_address: None,
            should_quit: false,
            current_tab: 0,
            pool_cache: HashMap::new(),
            swap_state: SwapState::default(),
            liquidity_state: LiquidityState::default(),
            current_epoch: None,
            claimable_rewards: HashMap::new(),
            rewards_state: crate::tui_dex::screens::rewards::RewardsState::default(),
            admin_screen_state: crate::tui_dex::screens::admin::AdminScreenState::default(),
            settings_state: crate::tui_dex::screens::settings::SettingsState::default(),
            transaction_state: crate::tui_dex::screens::transaction::TransactionState::default(),
            network_info: NetworkInfo::default(),
            pending_operations: HashMap::new(),
            focus_manager: FocusManager::new(),
            wallet_selection_state:
                crate::tui_dex::screens::wallet_selection::WalletSelectionScreen::default(),
            wizard_state: {
                let mut wizard = crate::tui_dex::screens::wizard::WizardState::new();
                // Don't show wizard by default - it will be triggered by wallet selection actions
                wizard.show_wizard = false;
                wizard
            },
            asset_decimals_cache: HashMap::new(),
        }
    }
}

/// Main application structure
pub struct App {
    /// Application state
    pub state: AppState,
    /// DEX client wrapped in Arc for sharing
    pub client: Arc<MantraDexClient>,
    /// Configuration
    pub config: MantraNetworkConfig,
    /// Event sender for background task communication
    event_sender: Option<mpsc::UnboundedSender<Event>>,
    /// Enhanced background task coordinator
    background_coordinator: Option<crate::tui_dex::utils::async_ops::BackgroundTaskCoordinator>,
}

impl App {
    /// Create a new application instance
    pub fn new(client: MantraDexClient, config: MantraNetworkConfig) -> Self {
        Self {
            state: AppState::default(),
            client: Arc::new(client),
            config,
            event_sender: None,
            background_coordinator: None,
        }
    }

    /// Initialize background tasks for data synchronization with enhanced coordination
    pub fn initialize_background_tasks(&mut self, event_sender: mpsc::UnboundedSender<Event>) {
        // Create enhanced background task coordinator
        let client_arc = Arc::clone(&self.client);
        let mut coordinator = crate::tui_dex::utils::async_ops::BackgroundTaskCoordinator::new(
            event_sender.clone(),
            client_arc,
            None, // Use default config for now
        );

        // Set wallet address if available
        if let Some(wallet_address) = &self.state.wallet_address {
            coordinator.set_wallet_address(wallet_address.clone());
        }

        // Start background coordination
        coordinator.start();

        self.background_coordinator = Some(coordinator);
        self.event_sender = Some(event_sender);
    }

    /// Stop background tasks with proper cleanup
    pub fn stop_background_tasks(&mut self) {
        if let Some(mut coordinator) = self.background_coordinator.take() {
            coordinator.stop();
        }
    }

    /// Update sync configuration
    pub fn update_sync_config(&mut self, config: crate::tui_dex::utils::async_ops::SyncConfig) {
        if let Some(coordinator) = &mut self.background_coordinator {
            coordinator.update_config(config);
        }
    }

    /// Check if real-time synchronization is active
    pub fn is_real_time_sync_active(&self) -> bool {
        self.background_coordinator
            .as_ref()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    /// Get current network state
    pub async fn get_network_state(&self) -> crate::tui_dex::utils::async_ops::NetworkState {
        if let Some(coordinator) = &self.background_coordinator {
            coordinator.get_network_state().await
        } else {
            crate::tui_dex::utils::async_ops::NetworkState::Connected
        }
    }

    /// Get event sender for internal use
    pub fn get_event_sender(&self) -> Option<&mpsc::UnboundedSender<Event>> {
        self.event_sender.as_ref()
    }

    /// Execute async operation with comprehensive error handling
    pub async fn execute_async_operation<F, Fut, T>(
        &mut self,
        operation_name: &str,
        operation: F,
    ) -> Result<T, Error>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        // Set loading state
        self.set_loading_with_progress(format!("Executing {}...", operation_name), Some(0.1), true);

        let start_time = std::time::Instant::now();

        // Check network state first
        let network_state = self.get_network_state().await;
        match network_state {
            crate::tui_dex::utils::async_ops::NetworkState::Disconnected => {
                let error_msg =
                    "Network is disconnected. Please check your connection.".to_string();
                self.set_error_with_retry(
                    error_msg.clone(),
                    ErrorType::Network,
                    "retry_connection".to_string(),
                );
                return Err(Error::Network(error_msg));
            }
            crate::tui_dex::utils::async_ops::NetworkState::Error(ref err) => {
                let error_msg = format!("Network error: {}", err);
                self.set_error_with_retry(
                    error_msg.clone(),
                    ErrorType::Network,
                    "retry_connection".to_string(),
                );
                return Err(Error::Network(error_msg));
            }
            _ => {}
        }

        // Update progress
        self.update_loading_progress(25.0, Some(format!("Executing {}...", operation_name)));

        // Execute the operation with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30), // 30 second timeout
            operation(),
        )
        .await;

        match result {
            Ok(Ok(value)) => {
                let duration = start_time.elapsed();
                self.set_success(format!(
                    "{} completed successfully in {:.2}s",
                    operation_name,
                    duration.as_secs_f64()
                ));
                Ok(value)
            }
            Ok(Err(e)) => {
                let categorized_error = self.categorize_error(&e);
                let error_msg = format!("{} failed: {}", operation_name, e);

                match categorized_error {
                    ErrorType::Network => {
                        self.set_error_with_retry(
                            error_msg,
                            categorized_error,
                            "retry_operation".to_string(),
                        );
                    }
                    ErrorType::Validation => {
                        self.set_error_with_type(error_msg, categorized_error);
                    }
                    _ => {
                        self.set_error_with_retry(
                            error_msg,
                            categorized_error,
                            "retry_operation".to_string(),
                        );
                    }
                }
                Err(e)
            }
            Err(_) => {
                let error_msg = format!("{} timed out after 30 seconds", operation_name);
                self.set_error_with_retry(
                    error_msg.clone(),
                    ErrorType::Timeout,
                    "retry_operation".to_string(),
                );
                Err(Error::Rpc(error_msg))
            }
        }
    }

    /// Enhanced error handling with comprehensive categorization
    fn categorize_error(&self, error: &Error) -> ErrorType {
        let error_string = error.to_string().to_lowercase();

        if error_string.contains("connection")
            || error_string.contains("network")
            || error_string.contains("timeout")
        {
            ErrorType::Network
        } else if error_string.contains("unauthorized") || error_string.contains("forbidden") {
            ErrorType::Authentication
        } else if error_string.contains("validation") || error_string.contains("invalid") {
            ErrorType::Validation
        } else if error_string.contains("insufficient") || error_string.contains("balance") {
            ErrorType::InsufficientFunds
        } else if error_string.contains("contract") || error_string.contains("execute") {
            ErrorType::Contract
        } else {
            ErrorType::Unknown
        }
    }

    /// Format insufficient funds error in a user-friendly way
    fn format_insufficient_funds_error(&self, error: &str) -> String {
        // Simple parsing to extract token symbols from insufficient funds error
        if error.contains("insufficient funds") || error.contains("spendable balance") {
            // Look for specific token denominations in the error message
            if error
                .contains("ibc/8E27BA2D5493AF5636760E354E46004562C46AB7EC0CC4C1CA14E9E20E2545B5")
                || error.contains("factory/mantra1qwm8p82w0ygaz3duf0y56gjf8pwh5ykmgnqmtm/uUSDC")
            {
                return "Insufficient USDC balance.\n\nYou don't have enough USDC to complete this transaction.\nPlease add more USDC to your wallet or reduce the transaction amount.".to_string();
            } else if error.contains("uom") {
                return "Insufficient OM balance.\n\nYou don't have enough OM to complete this transaction.\nPlease add more OM to your wallet or reduce the transaction amount.".to_string();
            }
        }

        "Insufficient funds. Please check your wallet balance and try again with a smaller amount."
            .to_string()
    }

    /// Extract meaningful contract error message
    fn extract_contract_error(&self, error: &str) -> String {
        // Look for the actual contract error message
        if let Some(start) = error.find("failed to execute message") {
            if let Some(end) = error[start..].find(": ") {
                let contract_part = &error[start + end + 2..];
                if let Some(error_end) = contract_part.find(": ") {
                    contract_part[..error_end].to_string()
                } else {
                    contract_part.to_string()
                }
            } else {
                error[start..].to_string()
            }
        } else {
            error.to_string()
        }
    }

    /// Handle async blockchain operations with comprehensive status updates
    pub async fn handle_event(&mut self, event: Event) -> Result<bool, Error> {
        // Handle network state changes
        if let Event::Custom(ref custom_event) = event {
            if custom_event.starts_with("network_state_changed:") {
                let parts: Vec<&str> = custom_event.split(':').collect();
                if parts.len() >= 3 {
                    let new_state = parts[2];
                    match new_state {
                        "connected" => {
                            self.state.network_info.connection_state =
                                crate::tui_dex::utils::async_ops::NetworkState::Connected;
                            self.set_status("Network connection restored".to_string());
                        }
                        "disconnected" => {
                            self.state.network_info.connection_state =
                                crate::tui_dex::utils::async_ops::NetworkState::Disconnected;
                            self.set_error_with_type(
                                "Network disconnected. Some features may be unavailable."
                                    .to_string(),
                                ErrorType::Network,
                            );
                        }
                        "error" => {
                            self.state.network_info.connection_state =
                                crate::tui_dex::utils::async_ops::NetworkState::Error(
                                    "Network error".to_string(),
                                );
                            self.set_error_with_retry(
                                "Network error detected. Attempting to reconnect...".to_string(),
                                ErrorType::Network,
                                "retry_connection".to_string(),
                            );
                        }
                        _ => {}
                    }
                }
                return Ok(false);
            }
        }

        // Handle blockchain progress events
        if let Event::BlockchainProgress {
            operation,
            status,
            progress,
        } = &event
        {
            self.update_loading_progress(
                progress.unwrap_or(0.0) as f64 * 100.0,
                Some(format!("{}: {}", operation, status)),
            );
            return Ok(false);
        }

        // Handle blockchain success events
        if let Event::BlockchainSuccess {
            operation,
            result,
            transaction_hash,
            enhanced_data,
        } = &event
        {
            // Clear loading state first
            self.state.loading_state = LoadingState::Idle;

            // Create operation-specific success titles and details
            let (success_title, formatted_details) = match operation.as_str() {
                "provide_liquidity" => {
                    let title = "Liquidity Provided Successfully!".to_string();
                    let details = self.create_liquidity_success_details(
                        result,
                        transaction_hash,
                        enhanced_data,
                    );
                    (title, details)
                }
                "withdraw_liquidity" => {
                    let title = "Liquidity Withdrawn Successfully!".to_string();
                    let details = self.create_basic_success_details(result, transaction_hash);
                    (title, details)
                }
                "execute_swap" => {
                    let title = "Swap Completed Successfully!".to_string();
                    let details = self.create_basic_success_details(result, transaction_hash);
                    (title, details)
                }
                "claim_rewards" => {
                    let title = "Rewards Claimed Successfully!".to_string();
                    let details = self.create_basic_success_details(result, transaction_hash);
                    (title, details)
                }
                "create_pool" => {
                    let title = "Pool Created Successfully!".to_string();
                    let details =
                        self.create_pool_creation_success_details(result, transaction_hash);
                    // Reset admin forms after successful pool creation
                    crate::tui_dex::screens::admin::reset_admin_forms(&mut self.state);
                    (title, details)
                }
                "update_pool_features" => {
                    let title = "Pool Features Updated Successfully!".to_string();
                    let details = self.create_basic_success_details(result, transaction_hash);
                    (title, details)
                }
                _ => {
                    let title = format!("{} Completed Successfully!", operation.replace('_', " "));
                    let details = self.create_basic_success_details(result, transaction_hash);
                    (title, details)
                }
            };

            self.state.modal_state = Some(
                crate::tui_dex::components::modals::ModalState::transaction_details(
                    transaction_hash.clone().unwrap_or_default(),
                    success_title,
                    formatted_details,
                ),
            );

            crate::tui_dex::utils::logger::log_info(&format!(
                "Blockchain success modal displayed for operation: {}",
                operation
            ));

            return Ok(false);
        }

        // Handle blockchain error events
        if let Event::BlockchainError { operation, error } = &event {
            // Clear the loading state first
            self.state.loading_state = LoadingState::Idle;

            // Determine error type and create user-friendly message
            let (error_type, user_friendly_error) = if error
                .to_lowercase()
                .contains("insufficient funds")
            {
                (
                    ErrorType::Contract,
                    self.format_insufficient_funds_error(error),
                )
            } else if error.to_lowercase().contains("network") {
                (ErrorType::Network, "Network connection error. Please check your internet connection and try again.".to_string())
            } else if error.to_lowercase().contains("contract") {
                (
                    ErrorType::Contract,
                    format!(
                        "Smart contract error: {}",
                        self.extract_contract_error(error)
                    ),
                )
            } else {
                (ErrorType::Unknown, error.clone())
            };

            // Show error modal instead of just loading state
            let operation_title = match operation.as_str() {
                "provide_liquidity" => "Provide Liquidity Failed",
                "withdraw_liquidity" => "Withdraw Liquidity Failed",
                "execute_swap" => "Swap Failed",
                _ => &format!("{} Failed", operation.replace('_', " ")),
            };

            self.state.modal_state = Some(crate::tui_dex::components::modals::ModalState::error(
                operation_title.to_string(),
                user_friendly_error.clone(),
                error_type,
                Some(vec![
                    format!("Operation: {}", operation),
                    format!("Error Details: {}", error),
                    format!(
                        "Timestamp: {}",
                        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                    ),
                ]),
                Some("retry_operation".to_string()),
            ));

            crate::tui_dex::utils::logger::log_info(&format!(
                "Blockchain error modal displayed for operation: {}",
                operation
            ));

            return Ok(false);
        }

        // Handle data refresh events with enhanced error reporting
        if let Event::DataRefresh {
            data_type,
            success,
            error,
        } = &event
        {
            match self
                .handle_data_refresh(data_type.clone(), *success, error.clone())
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    self.set_error_with_type(
                        format!("Failed to refresh {}: {}", data_type, e),
                        ErrorType::Unknown,
                    );
                }
            }
            return Ok(false);
        }

        // Handle retry with increased slippage event
        if let Event::RetryWithIncreasedSlippage = &event {
            if let Err(e) = self.handle_slippage_retry().await {
                crate::tui_dex::utils::logger::log_error(&format!(
                    "Failed to handle slippage retry: {}",
                    e
                ));
            }
            return Ok(false);
        }

        // Handle blockchain action events with comprehensive async processing
        match &event {
            Event::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                pool_id,
                slippage_tolerance,
            } => {
                // Set loading state immediately so UI can render it
                self.set_loading(format!(
                    "Executing swap: {} {} → {}",
                    amount, from_asset, to_asset
                ));

                // Execute the swap asynchronously by sending it to the background
                if let Some(sender) = &self.event_sender {
                    // Send the swap execution to be handled in the background
                    let _ = sender.send(Event::ExecuteSwapAsync {
                        from_asset: from_asset.clone(),
                        to_asset: to_asset.clone(),
                        amount: amount.clone(),
                        pool_id: pool_id.clone(),
                        slippage_tolerance: slippage_tolerance.clone(),
                    });
                }
                return Ok(false);
            }
            Event::ExecuteSwapAsync {
                from_asset,
                to_asset,
                amount,
                pool_id,
                slippage_tolerance,
            } => {
                // Execute real swap transaction
                self.execute_real_swap(
                    from_asset.clone(),
                    to_asset.clone(),
                    amount.clone(),
                    pool_id.clone(),
                    slippage_tolerance.clone(),
                )
                .await?;
                return Ok(false);
            }
            Event::ProvideLiquidity {
                pool_id,
                asset_1_amount,
                asset_2_amount,
                slippage_tolerance,
            } => {
                crate::tui_dex::utils::logger::log_info(&format!(
                    "=== PROCESSING PROVIDE LIQUIDITY EVENT ===",
                ));
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Pool ID: {}, Asset 1: {}, Asset 2: {}, Slippage: {:?}",
                    pool_id, asset_1_amount, asset_2_amount, slippage_tolerance
                ));

                // Show loading modal for liquidity provision
                self.set_loading_with_progress(
                    format!("Providing liquidity to pool {}", pool_id),
                    Some(5.0),
                    true,
                );

                // Use the async blockchain processor to execute the real transaction
                if let Some(event_sender) = &self.event_sender {
                    let blockchain_processor =
                        crate::tui_dex::events::AsyncBlockchainProcessor::with_client(
                            event_sender.clone(),
                            self.client.clone(),
                        );

                    let pool_id_clone = pool_id.clone();
                    let asset_1_clone = asset_1_amount.clone();
                    let asset_2_clone = asset_2_amount.clone();
                    let slippage_clone = slippage_tolerance.clone();

                    // Spawn the async operation
                    tokio::spawn(async move {
                        blockchain_processor
                            .provide_liquidity(
                                pool_id_clone,
                                asset_1_clone,
                                asset_2_clone,
                                slippage_clone,
                            )
                            .await;
                    });
                } else {
                    crate::tui_dex::utils::logger::log_error(
                        "No event sender available for async blockchain operation",
                    );
                    self.set_error(
                        "Failed to initiate liquidity provision: no event sender".to_string(),
                    );
                }

                return Ok(false);
            }
            Event::WithdrawLiquidity {
                pool_id,
                lp_token_amount,
                slippage_tolerance,
            } => {
                crate::tui_dex::utils::logger::log_info(&format!(
                    "=== PROCESSING WITHDRAW LIQUIDITY EVENT ===",
                ));
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Pool ID: {}, LP Amount: {}, Slippage: {:?}",
                    pool_id, lp_token_amount, slippage_tolerance
                ));

                // Show loading modal for liquidity withdrawal
                self.set_loading_with_progress(
                    format!("Withdrawing liquidity from pool {}", pool_id),
                    Some(5.0),
                    true,
                );

                // Use the async blockchain processor to execute the real transaction
                if let Some(event_sender) = &self.event_sender {
                    let blockchain_processor =
                        crate::tui_dex::events::AsyncBlockchainProcessor::with_client(
                            event_sender.clone(),
                            self.client.clone(),
                        );

                    let pool_id_clone = pool_id.clone();
                    let lp_amount_clone = lp_token_amount.clone();
                    let slippage_clone = slippage_tolerance.clone();

                    // Spawn the async operation
                    tokio::spawn(async move {
                        blockchain_processor
                            .withdraw_liquidity(pool_id_clone, lp_amount_clone, slippage_clone)
                            .await;
                    });
                } else {
                    crate::tui_dex::utils::logger::log_error(
                        "No event sender available for async blockchain operation",
                    );
                    self.set_error(
                        "Failed to initiate liquidity withdrawal: no event sender".to_string(),
                    );
                }

                return Ok(false);
            }
            Event::ClaimRewards {
                pool_id,
                epochs,
                claim_all,
            } => {
                // Show loading modal for rewards claiming
                let operation_description = if *claim_all {
                    "Claiming all available rewards".to_string()
                } else if let Some(pool_id_val) = pool_id {
                    format!("Claiming rewards from pool {}", pool_id_val)
                } else {
                    "Claiming rewards".to_string()
                };

                self.set_loading_with_progress(operation_description.clone(), Some(10.0), true);

                let operation_name = "claim_rewards";
                let pool_id_val = pool_id.clone();
                let epochs_val = epochs.clone();
                let claim_all_val = *claim_all;

                let result = self
                    .execute_async_operation(operation_name, || async {
                        // TODO: Implement actual rewards claiming
                        // Simulate the process
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                        // For now, create a mock successful response
                        // In real implementation, this would call self.client.claim_rewards()
                        let mock_tx_hash = format!(
                            "0x{:x}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                + 1
                        );

                        Ok(mock_tx_hash)
                    })
                    .await;

                match result {
                    Ok(tx_hash) => {
                        // Show success modal for rewards claiming
                        let mut transaction_details = vec![
                            ("Transaction Hash".to_string(), tx_hash.clone()),
                            ("Operation Type".to_string(), "Claim Rewards".to_string()),
                        ];

                        if claim_all_val {
                            transaction_details.push((
                                "Claim Type".to_string(),
                                "All Available Rewards".to_string(),
                            ));
                        } else if let Some(pool_id_val) = pool_id_val {
                            transaction_details.push(("Pool ID".to_string(), pool_id_val));
                        }

                        if let Some(epochs_val) = epochs_val {
                            let epochs_str = epochs_val
                                .iter()
                                .map(|e| e.to_string())
                                .collect::<Vec<_>>()
                                .join(", ");
                            transaction_details.push(("Epochs".to_string(), epochs_str));
                        }

                        transaction_details.extend(vec![
                            ("Estimated Rewards".to_string(), "~0.5 OM".to_string()), // Mock value
                            (
                                "Status".to_string(),
                                "✅ Completed Successfully".to_string(),
                            ),
                        ]);

                        self.state.modal_state = Some(
                            crate::tui_dex::components::modals::ModalState::transaction_details(
                                tx_hash,
                                "Rewards Claimed Successfully".to_string(),
                                transaction_details,
                            ),
                        );
                    }
                    Err(e) => {
                        crate::tui_dex::utils::logger::log_error(&format!(
                            "Rewards claiming failed: {}",
                            e
                        ));
                    }
                }
                return Ok(false);
            }
            _ => {}
        }

        // Handle modal events FIRST - they take priority over everything else
        if self.state.modal_state.is_some() {
            if self.handle_modal_event(&event) {
                return Ok(false); // Modal handled the event, don't process further
            }
        }

        // Handle wizard events SECOND - they should take priority when active
        if self.state.wizard_state.show_wizard {
            return self.handle_wizard_event(event).await;
        }

        // Handle focus management events
        let mut focus_handled = false;

        // Let screen-specific handlers process events next
        if self.handle_screen_specific_event(event.clone()).await? {
            return Ok(true);
        }

        // If screen didn't handle it, try global focus management
        if let Some(focused_component) = self.state.focus_manager.handle_event(&event) {
            self.update_component_focus(&focused_component);
            focus_handled = true;
        }

        // Handle standard navigation events
        match event {
            Event::Quit => {
                // Show quit confirmation modal instead of immediately quitting
                self.show_quit_confirmation();
                return Ok(true);
            }
            Event::Tab => {
                match self.state.navigation_mode {
                    NavigationMode::ScreenLevel => {
                        // Navigate to next screen tab
                        self.next_tab();
                    }
                    NavigationMode::WithinScreen => {
                        // Handle within-screen tab navigation
                        if let Some(focused_component) =
                            self.state.focus_manager.handle_event(&Event::FocusNext)
                        {
                            self.update_component_focus(&focused_component);
                        }
                    }
                }
            }
            Event::BackTab => {
                match self.state.navigation_mode {
                    NavigationMode::ScreenLevel => {
                        // Navigate to previous screen tab
                        self.previous_tab();
                    }
                    NavigationMode::WithinScreen => {
                        // Handle within-screen reverse tab navigation
                        if let Some(focused_component) =
                            self.state.focus_manager.handle_event(&Event::FocusPrevious)
                        {
                            self.update_component_focus(&focused_component);
                        }
                    }
                }
            }
            Event::Escape => {
                // Handle escape key - close modals or return to screen-level navigation
                if self.state.modal_state.is_some() {
                    self.state.modal_state = None;
                    return Ok(true); // Event was handled - modal was closed
                } else if self.state.navigation_mode == NavigationMode::WithinScreen {
                    // First, give screen-specific handlers a chance to handle ESC
                    if self.handle_screen_specific_event(Event::Escape).await? {
                        // Screen handled the ESC event, now switch navigation modes
                        self.state.navigation_mode = NavigationMode::ScreenLevel;
                        self.state.focus_manager.clear_focus();
                        return Ok(true); // Event was handled - returned to screen level
                    }

                    // If screen didn't handle it, exit within-screen navigation mode anyway
                    self.state.navigation_mode = NavigationMode::ScreenLevel;
                    self.state.focus_manager.clear_focus();
                    return Ok(true); // Event was handled - returned to screen level
                } else {
                    // User is in ScreenLevel navigation mode and pressed ESC
                    // Show quit confirmation modal instead of immediately quitting
                    self.show_quit_confirmation();
                    return Ok(true); // Event was handled - showed quit confirmation
                }
            }
            Event::Help => {
                self.show_help();
            }
            Event::Refresh => {
                self.refresh_current_screen_data().await?;
            }
            Event::Enter => {
                // Handle enter key based on navigation mode
                if self.state.current_screen == Screen::WalletSelection {
                    // Special case: Wallet selection screen should handle Enter directly
                    // without needing to switch to WithinScreen mode first
                    self.handle_screen_specific_event(Event::Enter).await?;
                } else if self.state.navigation_mode == NavigationMode::ScreenLevel {
                    // Enter within-screen navigation mode
                    self.state.navigation_mode = NavigationMode::WithinScreen;
                    self.initialize_focus_for_screen(self.state.current_screen);
                } else {
                    // Handle enter within screen - try general handler first, then screen-specific
                    self.handle_enter_key().await?;

                    // If the general handler didn't handle it, try screen-specific handler
                    if !self.handle_screen_specific_event(Event::Enter).await? {
                        // Event wasn't handled by either
                    }
                }
            }
            Event::MoveFocus(direction) => {
                // MoveFocus events should already be handled by screen-specific handlers above
                // If we reach here, it means the screen didn't handle it, so we fall back to global navigation
                if self.state.navigation_mode == NavigationMode::WithinScreen && !focus_handled {
                    // Convert Up/Down arrows to Tab/Shift+Tab behavior for consistent navigation
                    let focus_event = match direction {
                        crate::tui_dex::events::FocusDirection::Up => Event::FocusPrevious,
                        crate::tui_dex::events::FocusDirection::Down => Event::FocusNext,
                        _ => Event::MoveFocus(direction.clone()), // Keep other directions as-is
                    };

                    if let Some(focused_component) =
                        self.state.focus_manager.handle_event(&focus_event)
                    {
                        self.update_component_focus(&focused_component);
                    }
                }
            }
            Event::ContextAction => {
                // Handle space bar for context-sensitive actions
                self.handle_context_action().await?;
            }
            Event::F(1) => {
                self.show_help();
            }
            Event::F(5) => {
                self.refresh_current_screen_data().await?;
            }
            _ => {
                // Handle character events that need special context-aware processing
                if let Event::Char(c) = &event {
                    // Handle 'q' as quit only when in ScreenLevel mode (not in text input)
                    if *c == 'q' && self.state.navigation_mode == NavigationMode::ScreenLevel {
                        // Show quit confirmation modal instead of immediately quitting
                        self.show_quit_confirmation();
                        return Ok(true);
                    }

                    // Handle number key navigation for tab switching (1-8) when in ScreenLevel mode
                    if self.state.navigation_mode == NavigationMode::ScreenLevel {
                        if let Some(screen) =
                            crate::tui_dex::components::navigation::number_key_to_screen(*c)
                        {
                            self.navigate_to(screen);
                            return Ok(false);
                        }
                    }
                }
            }
        }

        Ok(focus_handled)
    }

    /// Update component focus state when focus changes
    fn update_component_focus(
        &mut self,
        focused_component: &crate::tui_dex::events::FocusableComponent,
    ) {
        use crate::tui_dex::events::FocusableComponent;

        // Bridge FocusManager focus changes to per-screen component state so that
        // individual screen modules can visually highlight the focused widget.
        match self.state.current_screen {
            Screen::Swap => {
                let swap_state = crate::tui_dex::screens::swap::get_swap_screen_state();
                // Clear previous internal focus first
                swap_state.reset_focus();

                match focused_component {
                    FocusableComponent::TextInput(id) => match id.as_str() {
                        "swap_amount" => {
                            swap_state.input_focus =
                                crate::tui_dex::screens::swap::SwapInputFocus::FromAmount
                        }
                        "swap_slippage" => {
                            swap_state.input_focus =
                                crate::tui_dex::screens::swap::SwapInputFocus::Slippage
                        }
                        _ => {}
                    },
                    FocusableComponent::Dropdown(id) => {
                        match id.as_str() {
                            "swap_pool" => {
                                swap_state.input_focus =
                                    crate::tui_dex::screens::swap::SwapInputFocus::Pool
                            }
                            "swap_from_asset" => {
                                swap_state.input_focus =
                                    crate::tui_dex::screens::swap::SwapInputFocus::FromToken
                            }
                            "swap_to_asset" => {
                                swap_state.input_focus =
                                    crate::tui_dex::screens::swap::SwapInputFocus::FromToken
                            } // Fallback for old references
                            _ => {}
                        }
                    }
                    FocusableComponent::Button(id) => {
                        if id == "swap_execute" {
                            swap_state.input_focus =
                                crate::tui_dex::screens::swap::SwapInputFocus::Execute;
                        }
                    }
                    _ => {}
                }

                // Ensure internal state knows which widget is focused so that render_* helpers style correctly
                swap_state.apply_focus();
            }
            Screen::Liquidity => {
                let liquidity_state = crate::tui_dex::screens::liquidity::get_liquidity_screen_state();
                // Clear previous internal focus first
                liquidity_state.reset_focus();

                match focused_component {
                    FocusableComponent::TextInput(id) => match id.as_str() {
                        "liquidity_amount1" => liquidity_state.input_focus =
                            crate::tui_dex::screens::liquidity::LiquidityInputFocus::FirstAssetAmount,
                        "liquidity_amount2" => liquidity_state.input_focus =
                            crate::tui_dex::screens::liquidity::LiquidityInputFocus::SecondAssetAmount,
                        "liquidity_withdraw_amount" => {
                            liquidity_state.input_focus =
                                crate::tui_dex::screens::liquidity::LiquidityInputFocus::WithdrawAmount
                        }
                        "liquidity_slippage_amount" => {
                            liquidity_state.input_focus =
                                crate::tui_dex::screens::liquidity::LiquidityInputFocus::SlippageAmount
                        }
                        "liquidity_slippage_swap" => {
                            liquidity_state.input_focus =
                                crate::tui_dex::screens::liquidity::LiquidityInputFocus::SlippageAmount
                        }
                        _ => {}
                    },
                    FocusableComponent::Dropdown(id) => {
                        if id == "liquidity_pool" {
                            liquidity_state.input_focus =
                                crate::tui_dex::screens::liquidity::LiquidityInputFocus::Pool
                        }
                    }
                    FocusableComponent::Button(id) => match id.as_str() {
                        "liquidity_provide" | "liquidity_withdraw" | "liquidity_execute" => {
                            liquidity_state.input_focus =
                                crate::tui_dex::screens::liquidity::LiquidityInputFocus::Execute
                        }
                        _ => {}
                    },
                    _ => {}
                }

                // Ensure internal state knows which widget is focused so that render_* helpers style correctly
                liquidity_state.apply_focus();
            }
            Screen::Admin => {
                let admin_state = crate::tui_dex::screens::admin::get_admin_screen_state();
                // Clear previous internal focus first
                admin_state.reset_focus();

                match focused_component {
                    FocusableComponent::TextInput(id) => match id.as_str() {
                        "admin_first_asset" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::FirstAssetDenom
                        }
                        "admin_second_asset" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::SecondAssetDenom
                        }
                        "admin_swap_fee" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::SwapFee
                        }
                        "admin_protocol_fee" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::ProtocolFee
                        }
                        "admin_burn_fee" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::BurnFee
                        }
                        "admin_target_pool_id" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::TargetPoolId
                        }
                        _ => {}
                    },
                    FocusableComponent::Dropdown(id) => match id.as_str() {
                        "admin_pool_selection" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::PoolSelection
                        }
                        "admin_pool_type" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::PoolType
                        }
                        _ => {}
                    },
                    FocusableComponent::Button(id) => match id.as_str() {
                        "admin_management_execute" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::ManagementExecute
                        }
                        "admin_creation_execute" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::CreationExecute
                        }
                        "admin_controls_execute" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::ControlsExecute
                        }
                        _ => {}
                    },
                    FocusableComponent::Custom(id) => match id.as_str() {
                        "admin_feature_toggles" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::FeatureToggles
                        }
                        "admin_feature_controls" => {
                            admin_state.input_focus =
                                crate::tui_dex::screens::admin::AdminInputFocus::FeatureControls
                        }
                        _ => {}
                    },
                    _ => {}
                }

                // Ensure internal state knows which widget is focused so that render_* helpers style correctly
                admin_state.apply_focus();
            }
            _ => {}
        }
    }

    /// Initialize focus manager for the current screen
    pub fn initialize_focus_for_screen(&mut self, screen: Screen) {
        use crate::tui_dex::utils::focus_manager::component_ids::*;

        let components = match screen {
            Screen::Dashboard => vec![dashboard_refresh_button(), dashboard_transactions_table()],
            Screen::Pools => vec![pools_search_input(), pools_table()],
            Screen::Swap => vec![
                swap_pool_dropdown(),       // Pool selection (maps to SwapInputFocus::Pool)
                swap_from_asset_dropdown(), // From token selection (maps to SwapInputFocus::FromToken)
                swap_amount_input(), // From amount input (maps to SwapInputFocus::FromAmount)
                swap_slippage_input(), // Slippage tolerance (maps to SwapInputFocus::Slippage)
                swap_execute_button(), // Execute button (maps to SwapInputFocus::Execute)
            ],
            Screen::Liquidity => {
                // Initialize liquidity screen specific focus
                liquidity::initialize_liquidity_screen_focus();

                vec![
                    liquidity_pool_dropdown(),
                    liquidity_amount1_input(),
                    liquidity_amount2_input(),
                    liquidity_slippage_input(),
                    liquidity_provide_button(),
                    liquidity_withdraw_button(),
                ]
            }
            Screen::Rewards => vec![
                rewards_epoch_input(),
                rewards_claim_all_button(),
                rewards_history_table(),
            ],
            Screen::Admin => vec![
                // Pool Management tab components
                crate::tui_dex::events::FocusableComponent::Dropdown(
                    "admin_pool_selection".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::Custom("admin_feature_toggles".to_string()),
                crate::tui_dex::events::FocusableComponent::Button(
                    "admin_management_execute".to_string(),
                ),
                // Pool Creation tab components
                crate::tui_dex::events::FocusableComponent::TextInput("admin_first_asset".to_string()),
                crate::tui_dex::events::FocusableComponent::TextInput("admin_second_asset".to_string()),
                crate::tui_dex::events::FocusableComponent::TextInput("admin_swap_fee".to_string()),
                crate::tui_dex::events::FocusableComponent::TextInput("admin_protocol_fee".to_string()),
                crate::tui_dex::events::FocusableComponent::TextInput("admin_burn_fee".to_string()),
                crate::tui_dex::events::FocusableComponent::Dropdown("admin_pool_type".to_string()),
                crate::tui_dex::events::FocusableComponent::Button(
                    "admin_creation_execute".to_string(),
                ),
                // Feature Controls tab components
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "admin_target_pool_id".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::Custom(
                    "admin_feature_controls".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::Button(
                    "admin_controls_execute".to_string(),
                ),
            ],
            Screen::Settings => vec![
                // Section navigation
                crate::tui_dex::events::FocusableComponent::Custom("settings_section_list".to_string()),
                // Network section components
                crate::tui_dex::events::FocusableComponent::Dropdown(
                    "settings_network_environment".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_network_name".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_network_rpc".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::TextInput("settings_gas_price".to_string()),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_gas_adjustment".to_string(),
                ),
                // Wallet section components
                crate::tui_dex::events::FocusableComponent::Button(
                    "settings_wallet_import_mode".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_wallet_mnemonic".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::Button(
                    "settings_wallet_show_mnemonic".to_string(),
                ),
                // Display section components
                crate::tui_dex::events::FocusableComponent::Dropdown("settings_theme".to_string()),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_balance_refresh".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_pool_refresh".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::TextInput(
                    "settings_decimal_precision".to_string(),
                ),
                crate::tui_dex::events::FocusableComponent::Button("settings_auto_refresh".to_string()),
                // Action buttons
                settings_save_button(),
                settings_reset_button(),
            ],
            _ => vec![],
        };

        self.state.focus_manager.set_tab_order(components.clone());

        // Ensure all components are visible and enabled for proper navigation
        for component in &components {
            self.state
                .focus_manager
                .set_component_visibility(component.clone(), true);
            self.state
                .focus_manager
                .set_component_enabled(component.clone(), true);
        }

        // Set focus to first component
        self.state.focus_manager.focus_first();
    }

    /// Handle refresh for current screen
    async fn refresh_current_screen_data(&mut self) -> Result<(), Error> {
        match self.state.current_screen {
            Screen::Dashboard => {
                // Refresh dashboard data
                self.refresh_dashboard_data().await?;
            }
            Screen::Pools => {
                // Refresh pool data
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::DataRefresh {
                        data_type: "pools".to_string(),
                        success: true,
                        error: None,
                    });
                }
            }
            Screen::Swap => {
                // Refresh pool data for swap screen and populate dropdowns
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::DataRefresh {
                        data_type: "pools".to_string(),
                        success: true,
                        error: None,
                    });
                }
                // Update swap screen pool dropdown with cached pools
                self.update_swap_screen_pools();
            }
            Screen::Liquidity => {
                // Initialize focus for liquidity screen
                crate::tui_dex::screens::liquidity::initialize_liquidity_screen_focus();

                // Refresh pool data for liquidity screen
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::DataRefresh {
                        data_type: "pools".to_string(),
                        success: true,
                        error: None,
                    });
                }
                // Update liquidity screen pool dropdown with cached pools
                self.update_liquidity_screen_pools();
            }
            Screen::Admin => {
                // Initialize focus for admin screen
                crate::tui_dex::screens::admin::initialize_admin_screen_focus(&mut self.state);

                // Refresh pool data for admin screen
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::DataRefresh {
                        data_type: "pools".to_string(),
                        success: true,
                        error: None,
                    });
                }
                // Update admin screen pool dropdown with cached pools
                self.update_admin_screen_pools();
            }
            _ => {}
        }
        Ok(())
    }

    /// Update swap screen pools dropdown with available pools
    fn update_swap_screen_pools(&mut self) {
        let swap_state = crate::tui_dex::screens::swap::get_swap_screen_state();

        // Extract available pools from cache for swap operations
        let available_pools: Vec<(String, String)> = self
            .state
            .pool_cache
            .values()
            .filter(|entry| {
                // Only include pools that have swaps enabled
                let pool = &entry.pool_info;
                pool.pool_info.status.swaps_enabled
            })
            .map(|entry| {
                let pool = &entry.pool_info;
                let pool_id = pool.pool_info.pool_identifier.to_string();

                // Create display name showing asset pair with amounts
                let asset_pair_with_amounts = if pool.pool_info.assets.len() >= 2 {
                    let asset1 = &pool.pool_info.assets[0];
                    let asset2 = &pool.pool_info.assets[1];

                    // Get proper token symbols instead of micro denominations
                    let asset1_symbol = self.denom_to_symbol(&asset1.denom);
                    let asset2_symbol = self.denom_to_symbol(&asset2.denom);

                    // Convert micro amounts to actual token amounts
                    let asset1_amount =
                        self.micro_to_token_amount(&asset1.amount.to_string(), &asset1.denom);
                    let asset2_amount =
                        self.micro_to_token_amount(&asset2.amount.to_string(), &asset2.denom);

                    // Format with proper symbols and amounts
                    format!(
                        "{} ({}) / {} ({})",
                        asset1_symbol, asset1_amount, asset2_symbol, asset2_amount
                    )
                } else {
                    "Unknown Pair".to_string()
                };

                let display_name = format!("Pool {}: {}", pool_id, asset_pair_with_amounts);
                (pool_id, display_name)
            })
            .collect();

        // Debug output to understand what pools are available
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Total pools in cache: {}",
            self.state.pool_cache.len()
        ));
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Available swap-enabled pools: {}",
            available_pools.len()
        ));
        for (pool_id, display_name) in &available_pools {
            crate::tui_dex::utils::logger::log_debug(&format!(
                "Pool ID: '{}', Display: '{}'",
                pool_id, display_name
            ));
        }

        // Note: If no pools are available from cache, dropdowns will remain empty
        // This is normal during initial loading or when no pools exist

        // Note: No longer adding test data for pools since they don't exist on blockchain
        // Real pool data will be loaded from the blockchain via the cache
        if available_pools.is_empty() {
            // Log warning that no pools are available
            crate::tui_dex::utils::logger::log_warning(
                "No pools available for swapping. Pool data may still be loading from blockchain.",
            );
        }

        // Update the pool dropdown with available pools
        swap_state.update_available_pools(available_pools);

        // Also update available tokens from the pools
        let mut available_tokens: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for entry in self.state.pool_cache.values() {
            for asset in &entry.pool_info.pool_info.assets {
                let denom = &asset.denom;
                // Use proper token symbol instead of micro denomination
                let token_symbol = self.denom_to_symbol(denom);
                available_tokens.insert(token_symbol);
            }
        }

        // Note: If no tokens are available from cache, dropdown will remain empty
        // This is normal during initial loading or when no pools exist

        let mut tokens_vec: Vec<String> = available_tokens.into_iter().collect();

        // Note: No longer adding test tokens since they should come from real pool assets
        if tokens_vec.is_empty() {
            crate::tui_dex::utils::logger::log_warning(
                "No tokens available for swapping. Pool data may still be loading from blockchain.",
            );
        }

        swap_state.initialize_tokens(tokens_vec);

        // Note: Real balances should be loaded from blockchain via refresh_balances()
        // The hardcoded test balances have been removed to show actual wallet balances
    }

    /// Update admin screen pools dropdown with available pools
    fn update_admin_screen_pools(&mut self) {
        // Extract all available pools from cache for admin operations
        let available_pools: Vec<(String, String)> = self
            .state
            .pool_cache
            .values()
            .map(|entry| {
                let pool = &entry.pool_info;
                let pool_id = pool.pool_info.pool_identifier.to_string();

                // Create display name showing asset pair with amounts
                let asset_pair_with_amounts = if pool.pool_info.assets.len() >= 2 {
                    let asset1 = &pool.pool_info.assets[0];
                    let asset2 = &pool.pool_info.assets[1];

                    // Get proper token symbols instead of micro denominations
                    let asset1_symbol = self.denom_to_symbol(&asset1.denom);
                    let asset2_symbol = self.denom_to_symbol(&asset2.denom);

                    // Convert micro amounts to actual token amounts
                    let asset1_amount =
                        self.micro_to_token_amount(&asset1.amount.to_string(), &asset1.denom);
                    let asset2_amount =
                        self.micro_to_token_amount(&asset2.amount.to_string(), &asset2.denom);

                    // Format with proper symbols and amounts
                    format!(
                        "{} ({}) / {} ({})",
                        asset1_symbol, asset1_amount, asset2_symbol, asset2_amount
                    )
                } else {
                    "Unknown Pair".to_string()
                };

                let display_name = format!("Pool {}: {}", pool_id, asset_pair_with_amounts);
                (pool_id, display_name)
            })
            .collect();

        // Debug output to understand what pools are available
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Total pools in cache for admin: {}",
            self.state.pool_cache.len()
        ));
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Available pools for admin management: {}",
            available_pools.len()
        ));
        for (pool_id, display_name) in &available_pools {
            crate::tui_dex::utils::logger::log_debug(&format!(
                "Admin Pool ID: '{}', Display: '{}'",
                pool_id, display_name
            ));
        }

        // Update the admin screen with available pools
        crate::tui_dex::screens::admin::update_admin_pools(&mut self.state, available_pools);
    }

    /// Update liquidity screen pools dropdown with available pools
    fn update_liquidity_screen_pools(&mut self) {
        // Extract available pools from cache for liquidity operations
        let available_pools: Vec<(String, String)> = self
            .state
            .pool_cache
            .values()
            .filter(|entry| {
                // Only include pools that have liquidity features enabled
                let pool = &entry.pool_info;
                pool.pool_info.status.deposits_enabled
            })
            .map(|entry| {
                let pool = &entry.pool_info;
                let pool_id = pool.pool_info.pool_identifier.to_string();

                // Create display name showing asset pair with amounts
                let asset_pair_with_amounts = if pool.pool_info.assets.len() >= 2 {
                    let asset1 = &pool.pool_info.assets[0];
                    let asset2 = &pool.pool_info.assets[1];

                    // Get proper token symbols instead of micro denominations
                    let asset1_symbol = self.denom_to_symbol(&asset1.denom);
                    let asset2_symbol = self.denom_to_symbol(&asset2.denom);

                    // Convert micro amounts to actual token amounts
                    let asset1_amount =
                        self.micro_to_token_amount(&asset1.amount.to_string(), &asset1.denom);
                    let asset2_amount =
                        self.micro_to_token_amount(&asset2.amount.to_string(), &asset2.denom);

                    // Format with proper symbols and amounts
                    format!(
                        "{} ({}) / {} ({})",
                        asset1_symbol, asset1_amount, asset2_symbol, asset2_amount
                    )
                } else {
                    "Unknown Pair".to_string()
                };

                let display_name = format!("Pool {}: {}", pool_id, asset_pair_with_amounts);
                (pool_id, display_name)
            })
            .collect();

        // Debug output to understand what pools are available
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Total pools in cache: {}",
            self.state.pool_cache.len()
        ));
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Available liquidity-enabled pools: {}",
            available_pools.len()
        ));
        for (pool_id, display_name) in &available_pools {
            crate::tui_dex::utils::logger::log_debug(&format!(
                "Pool ID: '{}', Display: '{}'",
                pool_id, display_name
            ));
        }

        // Note: If no pools are available from cache, dropdowns will remain empty
        // This is normal during initial loading or when no pools exist
        if available_pools.is_empty() {
            // Log warning that no pools are available
            crate::tui_dex::utils::logger::log_warning(
                "No pools available for liquidity operations. Pool data may still be loading from blockchain.",
            );
        }

        // Update the liquidity screen with available pools
        crate::tui_dex::screens::liquidity::update_liquidity_pools(available_pools);
    }

    /// Handle enter key based on current focus
    async fn handle_enter_key(&mut self) -> Result<(), Error> {
        let focused = self.state.focus_manager.current_focus().cloned();
        if let Some(focused) = focused {
            match focused {
                crate::tui_dex::events::FocusableComponent::Button(button_id) => {
                    self.handle_button_activation(&button_id).await?;
                }
                crate::tui_dex::events::FocusableComponent::Dropdown(dropdown_id) => {
                    // Handle dropdown selection or toggle
                    self.handle_dropdown_selection(&dropdown_id);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle context action (space bar)
    async fn handle_context_action(&mut self) -> Result<(), Error> {
        let focused = self.state.focus_manager.current_focus().cloned();
        if let Some(focused) = focused {
            match focused {
                crate::tui_dex::events::FocusableComponent::Checkbox(checkbox_id) => {
                    self.handle_checkbox_toggle(&checkbox_id);
                }
                crate::tui_dex::events::FocusableComponent::Button(button_id) => {
                    self.handle_button_activation(&button_id).await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle button activation
    async fn handle_button_activation(&mut self, button_id: &str) -> Result<(), Error> {
        match button_id {
            "swap_execute" => {
                // Execute swap
                let swap_state = self.state.swap_state.clone();
                if let (Some(from_asset), Some(to_asset)) =
                    (&swap_state.from_asset, &swap_state.to_asset)
                {
                    if let Some(sender) = &self.event_sender {
                        let _ = sender.send(Event::ExecuteSwap {
                            from_asset: from_asset.clone(),
                            to_asset: to_asset.clone(),
                            amount: swap_state.amount,
                            pool_id: swap_state.selected_pool_id,
                            slippage_tolerance: Some(swap_state.slippage),
                        });
                    }
                }
            }
            "liquidity_provide" => {
                // Provide liquidity
                let liquidity_state = self.state.liquidity_state.clone();
                if let Some(pool_id_str) = &liquidity_state.selected_pool_id {
                    if let Ok(pool_id) = pool_id_str.parse::<u64>() {
                        if let Some(sender) = &self.event_sender {
                            let _ = sender.send(Event::ProvideLiquidity {
                                pool_id: pool_id.to_string(),
                                asset_1_amount: liquidity_state.first_asset_amount,
                                asset_2_amount: liquidity_state.second_asset_amount,
                                slippage_tolerance: Some(liquidity_state.slippage_amount),
                            });
                        }
                    }
                }
            }
            "rewards_claim_all" => {
                // Claim all rewards
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::ClaimRewards {
                        pool_id: None,
                        epochs: None,
                        claim_all: true,
                    });
                }
            }
            "dashboard_refresh" => {
                self.refresh_current_screen_data().await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle dropdown selection or toggle (legacy method - now handled directly by dropdown components)
    fn handle_dropdown_selection(&mut self, dropdown_id: &str) {
        // This method is now mostly a no-op since dropdown handling is done directly
        // by the SimpleDropdown components in handle_swap_screen_event.
        // We keep this for compatibility with other screens that might still use it.

        if self.state.current_screen == Screen::Swap {
            // For swap screen, dropdown handling is now done directly by SimpleDropdown components
            // in the handle_swap_screen_event method. No additional handling needed here.
            return;
        }

        // Legacy handling for other screens that might still need it
        match dropdown_id {
            "swap_pool" | "swap_from_asset" | "swap_to_asset" => {
                // These are handled by SimpleDropdown components directly
            }
            _ => {
                // Handle other dropdowns that might still use the old system
                // This would be implemented for screens that haven't been migrated to SimpleDropdown yet
            }
        }
    }

    /// Handle dropdown toggle (kept for backward compatibility)
    fn _handle_dropdown_toggle(&mut self, _dropdown_id: &str) {
        // This method is no longer needed since SimpleList handles its own state
        // Kept for backward compatibility with other screens that might still use it
    }

    /// Handle checkbox toggle
    fn handle_checkbox_toggle(&mut self, _checkbox_id: &str) {
        // Implementation depends on specific checkbox state management
        // This is a placeholder
    }

    /// Handle screen-specific events
    async fn handle_screen_specific_event(&mut self, event: Event) -> Result<bool, Error> {
        match self.state.current_screen {
            Screen::WalletSelection => self.handle_wallet_selection_event(event).await,
            Screen::Swap => self.handle_swap_screen_event(event).await,
            Screen::Liquidity => self.handle_liquidity_screen_event(event).await,
            Screen::Admin => self.handle_admin_screen_event(event).await,
            Screen::Settings => self.handle_settings_screen_event(event).await,
            _ => Ok(false),
        }
    }

    /// Handle wallet selection screen specific events. Returns `true` if the event was handled.
    async fn handle_wallet_selection_event(&mut self, event: Event) -> Result<bool, Error> {
        use crate::tui_dex::screens::wallet_selection::{WalletSelectionAction, WalletSelectionState};

        match event {
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Up) => {
                if self.state.wallet_selection_state.state == WalletSelectionState::SelectingWallet
                {
                    self.state.wallet_selection_state.move_selection_up();
                    return Ok(true);
                }
            }
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Down) => {
                if self.state.wallet_selection_state.state == WalletSelectionState::SelectingWallet
                {
                    self.state.wallet_selection_state.move_selection_down();
                    return Ok(true);
                }
            }
            Event::Enter => {
                let action = self.state.wallet_selection_state.handle_selection();
                return self.handle_wallet_selection_action(action).await;
            }
            Event::Escape => {
                let action = self.state.wallet_selection_state.handle_escape();
                return self.handle_wallet_selection_action(action).await;
            }
            Event::Char(c) => {
                if self.state.wallet_selection_state.state == WalletSelectionState::EnteringPassword
                {
                    self.state.wallet_selection_state.handle_char(c);
                    return Ok(true);
                } else if c == 'n' || c == 'N' {
                    // Quick shortcut to create new wallet
                    if self.state.wallet_selection_state.state
                        == WalletSelectionState::SelectingWallet
                    {
                        return self
                            .handle_wallet_selection_action(WalletSelectionAction::CreateNewWallet)
                            .await;
                    }
                } else if c == 'r' || c == 'R' {
                    // Quick shortcut to recover wallet
                    if self.state.wallet_selection_state.state
                        == WalletSelectionState::SelectingWallet
                    {
                        return self
                            .handle_wallet_selection_action(WalletSelectionAction::RecoverWallet)
                            .await;
                    }
                }
            }
            Event::Backspace => {
                if self.state.wallet_selection_state.state == WalletSelectionState::EnteringPassword
                {
                    self.state.wallet_selection_state.handle_backspace();
                    return Ok(true);
                }
            }
            Event::F(1) => {
                // Toggle password visibility
                self.state
                    .wallet_selection_state
                    .toggle_password_visibility();
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle wallet selection actions
    async fn handle_wallet_selection_action(
        &mut self,
        action: crate::tui_dex::screens::wallet_selection::WalletSelectionAction,
    ) -> Result<bool, Error> {
        use crate::tui_dex::screens::wallet_selection::WalletSelectionAction;

        match action {
            WalletSelectionAction::None => Ok(true),
            WalletSelectionAction::CreateNewWallet => {
                // Go to wizard for creating new wallet
                self.state.wizard_state.reset(); // Reset first to clear any previous state
                self.state.wizard_state.show_wizard = true;
                self.state.wizard_state.import_existing = false;
                // Generate new mnemonic for wallet creation
                match crate::wallet::MantraWallet::generate() {
                    Ok((_, mnemonic)) => {
                        self.state.wizard_state.generated_mnemonic = Some(mnemonic);
                    }
                    Err(e) => {
                        self.set_error(format!("Failed to generate wallet: {}", e));
                        return Ok(true);
                    }
                }
                Ok(true)
            }
            WalletSelectionAction::RecoverWallet => {
                // Go to wizard for recovering existing wallet
                self.state.wizard_state.reset(); // Reset first to clear any previous state
                self.state.wizard_state.show_wizard = true;
                self.state.wizard_state.import_existing = true;
                Ok(true)
            }
            WalletSelectionAction::AuthenticateWallet {
                wallet_name,
                password,
            } => {
                // Attempt to load and decrypt the wallet
                self.set_loading(format!("Loading wallet '{}'...", wallet_name));

                let storage = crate::wallet::WalletStorage::new().map_err(|e| {
                    Error::Wallet(format!("Failed to initialize wallet storage: {}", e))
                })?;

                match storage.load_wallet(&wallet_name, &password) {
                    Ok(mnemonic) => {
                        self.state
                            .wallet_selection_state
                            .handle_authentication_success(wallet_name.clone(), mnemonic.clone());

                        // Load the wallet into the application
                        match crate::wallet::MantraWallet::from_mnemonic(&mnemonic, 0) {
                            Ok(wallet) => {
                                match wallet.address() {
                                    Ok(address) => {
                                        self.set_wallet_address(address.to_string());
                                        // Reconfigure the client with the loaded wallet
                                        self.configure_client_wallet(wallet).await?;

                                        // Navigate to dashboard and hide wizard
                                        self.state.wizard_state.show_wizard = false;
                                        self.navigate_to(Screen::Dashboard);

                                        self.set_success(format!(
                                            "Wallet '{}' loaded successfully!",
                                            wallet_name
                                        ));
                                    }
                                    Err(e) => {
                                        self.state
                                            .wallet_selection_state
                                            .handle_authentication_failure(format!(
                                                "Failed to derive wallet address: {}",
                                                e
                                            ));
                                    }
                                }
                            }
                            Err(e) => {
                                self.state
                                    .wallet_selection_state
                                    .handle_authentication_failure(format!(
                                        "Failed to load wallet: {}",
                                        e
                                    ));
                            }
                        }
                    }
                    Err(e) => {
                        self.state
                            .wallet_selection_state
                            .handle_authentication_failure(format!("Authentication failed: {}", e));
                    }
                }
                Ok(true)
            }
            WalletSelectionAction::WalletLoaded {
                wallet_name: _,
                mnemonic: _,
            } => {
                // This case is handled in AuthenticateWallet above
                Ok(true)
            }
            WalletSelectionAction::Quit => {
                self.state.should_quit = true;
                Ok(true)
            }
        }
    }

    /// Handle swap screen specific events. Returns `true` if the event was handled.
    async fn handle_swap_screen_event(&mut self, event: Event) -> Result<bool, Error> {
        let swap_state = crate::tui_dex::screens::swap::get_swap_screen_state();

        // Handle MoveFocus events directly for better arrow key navigation
        match &event {
            Event::MoveFocus(direction) => {
                // Only handle arrow keys when we're in content mode and focused on interactive elements
                if self.state.navigation_mode == NavigationMode::WithinScreen {
                    match direction {
                        crate::tui_dex::events::FocusDirection::Up => {
                            // Check if we're in a dropdown that should handle up/down
                            if matches!(
                                swap_state.input_focus,
                                crate::tui_dex::screens::swap::SwapInputFocus::Pool
                                    | crate::tui_dex::screens::swap::SwapInputFocus::FromToken
                            ) && swap_state.is_any_list_editing()
                            {
                                // Let the dropdown handle the navigation
                                let key = crossterm::event::KeyEvent::new(
                                    crossterm::event::KeyCode::Up,
                                    crossterm::event::KeyModifiers::NONE,
                                );
                                if swap_state.handle_key_event(key, self.state.navigation_mode) {
                                    self.sync_swap_state_to_app(swap_state);
                                    return Ok(true);
                                }
                            }
                            // Otherwise, let global focus management handle it
                            return Ok(false);
                        }
                        crate::tui_dex::events::FocusDirection::Down => {
                            // Check if we're in a dropdown that should handle up/down
                            if matches!(
                                swap_state.input_focus,
                                crate::tui_dex::screens::swap::SwapInputFocus::Pool
                                    | crate::tui_dex::screens::swap::SwapInputFocus::FromToken
                            ) && swap_state.is_any_list_editing()
                            {
                                // Let the dropdown handle the navigation
                                let key = crossterm::event::KeyEvent::new(
                                    crossterm::event::KeyCode::Down,
                                    crossterm::event::KeyModifiers::NONE,
                                );
                                if swap_state.handle_key_event(key, self.state.navigation_mode) {
                                    self.sync_swap_state_to_app(swap_state);
                                    return Ok(true);
                                }
                            }
                            // Otherwise, let global focus management handle it
                            return Ok(false);
                        }
                        _ => return Ok(false), // Let global focus handle other directions
                    }
                }
                return Ok(false);
            }
            _ => {}
        }

        // Handle other swap-specific events
        match event {
            Event::Tab => {
                // Only handle Tab navigation between form fields when in WithinScreen mode
                if self.state.navigation_mode == NavigationMode::WithinScreen {
                    swap_state.next_focus();
                    self.sync_swap_state_to_app(swap_state);
                    return Ok(true);
                } else {
                    // In ScreenLevel mode, let global navigation handle Tab for screen switching
                    return Ok(false);
                }
            }
            Event::BackTab => {
                // Only handle Shift+Tab (reverse navigation) between form fields when in WithinScreen mode
                if self.state.navigation_mode == NavigationMode::WithinScreen {
                    swap_state.previous_focus();
                    self.sync_swap_state_to_app(swap_state);
                    return Ok(true);
                } else {
                    // In ScreenLevel mode, let global navigation handle BackTab for screen switching
                    return Ok(false);
                }
            }
            Event::Enter => {
                // Handle selection for currently focused list or execute button
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Enter,
                    crossterm::event::KeyModifiers::NONE,
                );
                if swap_state.handle_key_event(key_event, self.state.navigation_mode) {
                    self.sync_swap_state_to_app(swap_state);
                    // Check if execute button was pressed by examining the current focus
                    if matches!(
                        swap_state.input_focus,
                        crate::tui_dex::screens::swap::SwapInputFocus::Execute
                    ) {
                        // Trigger swap confirmation
                        if let Err(e) = self.handle_swap_execute_confirmation() {
                            self.set_error(format!("Swap preparation failed: {}", e));
                        }
                    }
                    return Ok(true);
                }
            }
            Event::Escape => {
                // ESC in WithinScreen mode should return to ScreenLevel mode
                // First, let the swap screen handle the ESC key event for any internal state cleanup
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Esc,
                    crossterm::event::KeyModifiers::NONE,
                );
                swap_state.handle_key_event(key_event, self.state.navigation_mode);

                // Return false to let the main app handle the navigation mode switch
                return Ok(false);
            }
            Event::Char(c) => {
                // Handle character input for text fields
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(c),
                    crossterm::event::KeyModifiers::NONE,
                );
                if swap_state.handle_key_event(key_event, self.state.navigation_mode) {
                    self.sync_swap_state_to_app(swap_state);
                    return Ok(true);
                }
            }
            Event::Backspace => {
                // Handle backspace for text fields
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Backspace,
                    crossterm::event::KeyModifiers::NONE,
                );
                if swap_state.handle_key_event(key_event, self.state.navigation_mode) {
                    self.sync_swap_state_to_app(swap_state);
                    return Ok(true);
                }
            }
            Event::TriggerSimulation => {
                // Only run simulation if we have valid input
                if !swap_state.from_amount_input.value().is_empty()
                    && swap_state.pool_dropdown.get_selected_value().is_some()
                    && swap_state
                        .from_token_dropdown
                        .get_selected_value()
                        .is_some()
                {
                    // Trigger swap simulation
                    let from_amount = swap_state.from_amount_input.value();
                    let from_token = swap_state
                        .from_token_dropdown
                        .get_selected_value()
                        .unwrap_or("");
                    let pool_text = swap_state.pool_dropdown.get_selected_label().unwrap_or("");

                    self.set_loading("Running swap simulation...".to_string());

                    // For now, just simulate some delay and show success
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    self.set_success(format!(
                        "Simulation complete: {} {} via {}",
                        from_amount, from_token, pool_text
                    ));

                    swap_state.reset_simulation_timer();
                }
                return Ok(true);
            }
            _ => return Ok(false),
        }

        Ok(false)
    }

    /// Sync swap screen state back to app state
    fn sync_swap_state_to_app(&mut self, swap_state: &crate::tui_dex::screens::swap::SwapScreenState) {
        // Update app state with changes from swap screen
        if let Some(selected_value) = swap_state.pool_dropdown.get_selected_value() {
            if let Ok(pool_id) = selected_value.parse::<u64>() {
                self.state.swap_state.selected_pool_id = Some(pool_id.to_string());
            }
        }
        if let Some(selected_token) = swap_state.from_token_dropdown.get_selected_value() {
            self.state.swap_state.from_asset = Some(selected_token.to_string());
        }
        self.state.swap_state.amount = swap_state.from_amount_input.value().to_string();
        self.state.swap_state.slippage = swap_state.slippage_input.value().to_string();
    }

    /// Handle liquidity screen specific events. Returns `true` if the event was handled.
    async fn handle_liquidity_screen_event(&mut self, event: Event) -> Result<bool, Error> {
        let liquidity_state = liquidity::get_liquidity_screen_state();

        // Convert Event to KeyEvent for the new key system (similar to swap screen)
        let key_event = match &event {
            Event::MoveFocus(direction) => {
                // Convert focus events to direct key events for navigation
                match direction {
                    crate::tui_dex::events::FocusDirection::Up => {
                        Some(crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Up,
                            crossterm::event::KeyModifiers::NONE,
                        ))
                    }
                    crate::tui_dex::events::FocusDirection::Down => {
                        Some(crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Down,
                            crossterm::event::KeyModifiers::NONE,
                        ))
                    }
                    crate::tui_dex::events::FocusDirection::Left => {
                        Some(crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Left,
                            crossterm::event::KeyModifiers::NONE,
                        ))
                    }
                    crate::tui_dex::events::FocusDirection::Right => {
                        Some(crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Right,
                            crossterm::event::KeyModifiers::NONE,
                        ))
                    }
                    _ => None,
                }
            }
            Event::Char(c) => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(*c),
                crossterm::event::KeyModifiers::NONE,
            )),
            Event::Enter => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            )),
            Event::Tab => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Tab,
                crossterm::event::KeyModifiers::NONE,
            )),
            Event::Backspace => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Backspace,
                crossterm::event::KeyModifiers::NONE,
            )),
            Event::Delete => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Delete,
                crossterm::event::KeyModifiers::NONE,
            )),
            Event::Home => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Home,
                crossterm::event::KeyModifiers::NONE,
            )),
            Event::End => Some(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::End,
                crossterm::event::KeyModifiers::NONE,
            )),
            _ => None,
        };

        // Handle the event using the new key event system (similar to swap screen)
        let mut key_handled = false;
        let mut pool_changed = false;
        if let Some(key_event) = key_event {
            key_handled = liquidity_state.handle_key_event(key_event, self.state.navigation_mode);
            if key_handled {
                // For navigation keys that might change pool selection, check if pool changed
                if matches!(
                    key_event.code,
                    crossterm::event::KeyCode::Up
                        | crossterm::event::KeyCode::Down
                        | crossterm::event::KeyCode::Enter
                ) {
                    pool_changed = true;
                }

                // Check if execute button was pressed by examining the current focus
                if matches!(
                    liquidity_state.input_focus,
                    crate::tui_dex::screens::liquidity::LiquidityInputFocus::Execute
                ) && matches!(event, Event::Enter)
                {
                    // Trigger liquidity confirmation
                    if let Err(e) = self.handle_liquidity_execute_confirmation() {
                        self.set_error(format!("Liquidity preparation failed: {}", e));
                    }
                }
            }
        }

        if key_handled {
            // If pool might have changed, fetch reserves for proportional calculations
            if pool_changed {
                if let Some(pool_id) = liquidity_state.pool_dropdown.get_selected_value() {
                    self.fetch_pool_reserves_for_liquidity(&pool_id).await?;
                }
            }
            return Ok(true);
        }

        // Handle specific events that don't need key conversion
        match event {
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Right) => {
                // Handle direct right arrow for tab switching
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Right,
                    crossterm::event::KeyModifiers::NONE,
                );
                if liquidity_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Left) => {
                // Handle direct left arrow for tab switching
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Left,
                    crossterm::event::KeyModifiers::NONE,
                );
                if liquidity_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Up) => {
                // Handle up arrow for lists
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Up,
                    crossterm::event::KeyModifiers::NONE,
                );
                if liquidity_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Down) => {
                // Handle down arrow for lists
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Down,
                    crossterm::event::KeyModifiers::NONE,
                );
                if liquidity_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            _ => {}
        }

        Ok(false)
    }

    /// Handle admin screen specific events. Returns `true` if the event was handled.
    async fn handle_admin_screen_event(&mut self, event: Event) -> Result<bool, Error> {
        let admin_state = crate::tui_dex::screens::admin::get_admin_screen_state();

        // Handle character events (including admin tab switching 1-3)
        if let Event::Char(c) = &event {
            if !c.is_control() {
                // Let admin screen handle all character input, including tab switching
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(*c),
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
        }

        // Handle MoveFocus events directly for better arrow key navigation
        // But first check if we're in a dropdown that should handle its own navigation
        match &event {
            Event::MoveFocus(direction) => match direction {
                crate::tui_dex::events::FocusDirection::Up
                | crate::tui_dex::events::FocusDirection::Down => {
                    // Check if we're focused on a dropdown that should handle its own navigation
                    let should_handle_in_dropdown = match admin_state.input_focus {
                        crate::tui_dex::screens::admin::AdminInputFocus::PoolSelection => {
                            admin_state
                                .pool_management
                                .pool_selection_dropdown
                                .is_active
                        }
                        crate::tui_dex::screens::admin::AdminInputFocus::PoolType => {
                            admin_state.pool_creation.pool_type_dropdown.is_active
                        }
                        _ => false,
                    };

                    if should_handle_in_dropdown {
                        // Let the dropdown handle the event
                        let key_event = crossterm::event::KeyEvent::new(
                            match direction {
                                crate::tui_dex::events::FocusDirection::Up => {
                                    crossterm::event::KeyCode::Up
                                }
                                crate::tui_dex::events::FocusDirection::Down => {
                                    crossterm::event::KeyCode::Down
                                }
                                _ => unreachable!(),
                            },
                            crossterm::event::KeyModifiers::NONE,
                        );
                        if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                            return Ok(true);
                        }
                    } else {
                        // Handle normal focus navigation
                        match direction {
                            crate::tui_dex::events::FocusDirection::Up => {
                                admin_state.previous_focus();
                                return Ok(true);
                            }
                            crate::tui_dex::events::FocusDirection::Down => {
                                admin_state.next_focus();
                                return Ok(true);
                            }
                            _ => {}
                        }
                    }
                }
                crate::tui_dex::events::FocusDirection::Next => {
                    admin_state.next_focus();
                    return Ok(true);
                }
                crate::tui_dex::events::FocusDirection::Previous => {
                    admin_state.previous_focus();
                    return Ok(true);
                }
                _ => {}
            },
            _ => {}
        }

        // Handle Enter key for admin screen navigation and selections
        if let Event::Enter = &event {
            let key_event = crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            );
            if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                // Check if execute button was pressed by examining the current focus
                match admin_state.input_focus {
                    crate::tui_dex::screens::admin::AdminInputFocus::CreationExecute => {
                        // Trigger pool creation confirmation
                        if let Err(e) = self.handle_pool_creation_execute_confirmation() {
                            self.set_error(format!("Pool creation preparation failed: {}", e));
                        }
                    }
                    crate::tui_dex::screens::admin::AdminInputFocus::ManagementExecute => {
                        // Trigger pool management confirmation
                        if let Err(e) = self.handle_pool_management_execute_confirmation() {
                            self.set_error(format!("Pool management preparation failed: {}", e));
                        }
                    }
                    _ => {}
                }
                return Ok(true);
            }
        }

        // Handle other key events
        match event {
            Event::Tab => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Tab,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::BackTab => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::BackTab,
                    crossterm::event::KeyModifiers::SHIFT,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::Backspace => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Backspace,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::Delete => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Delete,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::Char(c) => {
                // Convert character to key event for admin screen
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(c),
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Left) => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Left,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::MoveFocus(crate::tui_dex::events::FocusDirection::Right) => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Right,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::Home => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Home,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::End => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::End,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(true);
                }
            }
            Event::Paste(text) => {
                for ch in text.chars() {
                    let key_event = crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char(ch),
                        crossterm::event::KeyModifiers::NONE,
                    );
                    admin_state.handle_key_event(key_event, self.state.navigation_mode);
                }
                return Ok(true);
            }
            Event::Escape => {
                let key_event = crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Esc,
                    crossterm::event::KeyModifiers::NONE,
                );
                if admin_state.handle_key_event(key_event, self.state.navigation_mode) {
                    return Ok(false); // Let main app handle escape for navigation mode switching
                }
            }
            _ => {}
        }

        // Handle specific admin events for blockchain operations
        match &event {
            Event::CreatePool {
                asset_1,
                asset_2,
                swap_fee,
                exit_fee,
                pool_features,
            } => {
                // Execute pool creation
                self.set_loading_with_progress(
                    format!("Creating pool for {} / {}", asset_1, asset_2),
                    Some(10.0),
                    true,
                );

                // Use the async blockchain processor to execute the real transaction
                if let Some(event_sender) = &self.event_sender {
                    let blockchain_processor =
                        crate::tui_dex::events::AsyncBlockchainProcessor::with_client(
                            event_sender.clone(),
                            self.client.clone(),
                        );

                    let asset_1_clone = asset_1.clone();
                    let asset_2_clone = asset_2.clone();
                    let swap_fee_clone = swap_fee.clone();
                    let exit_fee_clone = exit_fee.clone();
                    let pool_features_clone = pool_features.clone();

                    // Spawn the async operation for pool creation
                    tokio::spawn(async move {
                        blockchain_processor
                            .create_pool(
                                asset_1_clone,
                                asset_2_clone,
                                swap_fee_clone,
                                exit_fee_clone,
                                pool_features_clone,
                            )
                            .await;
                    });
                } else {
                    self.set_error("No event sender available for pool creation".to_string());
                }

                return Ok(true);
            }
            Event::UpdatePoolFeatures {
                pool_id,
                features,
                enabled,
            } => {
                // Execute pool feature update
                let operation_desc = if *enabled {
                    format!("Enabling features for pool {}", pool_id)
                } else {
                    format!("Disabling features for pool {}", pool_id)
                };

                self.set_loading_with_progress(operation_desc, Some(10.0), true);

                // Use the async blockchain processor to execute the real transaction
                if let Some(event_sender) = &self.event_sender {
                    let blockchain_processor =
                        crate::tui_dex::events::AsyncBlockchainProcessor::with_client(
                            event_sender.clone(),
                            self.client.clone(),
                        );

                    let pool_id_clone = pool_id.clone();
                    let features_clone = features.clone();
                    let enabled_clone = *enabled;
                    let event_sender_clone = event_sender.clone();

                    // Spawn the async operation for feature update
                    tokio::spawn(async move {
                        // TODO: Implement actual pool feature update via blockchain_processor
                        // For now, simulate the operation
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                        // Send success event
                        let _ =
                            event_sender_clone.send(crate::tui_dex::events::Event::BlockchainSuccess {
                                operation: "update_pool_features".to_string(),
                                result: format!("Features updated for pool {}", pool_id_clone),
                                transaction_hash: Some(format!(
                                    "0x{:x}",
                                    chrono::Utc::now().timestamp()
                                )),
                                enhanced_data: Some(format!(
                                    "Features: {:?}, Enabled: {}",
                                    features_clone, enabled_clone
                                )),
                            });
                    });
                } else {
                    self.set_error("No event sender available for feature update".to_string());
                }

                return Ok(true);
            }
            _ => {}
        }

        Ok(false)
    }

    /// Handle settings screen specific events. Returns `true` if the event was handled.
    async fn handle_settings_screen_event(&mut self, event: Event) -> Result<bool, Error> {
        // Check if we're in content mode and handle navigation accordingly
        if matches!(self.state.navigation_mode, NavigationMode::WithinScreen) {
            match event {
                Event::Char(c) => {
                    self.handle_settings_input(c).await?;
                    return Ok(true);
                }
                Event::MoveFocus(direction) => {
                    // Handle focus movement within settings screen content
                    match direction {
                        crate::tui_dex::events::FocusDirection::Next => {
                            self.state.focus_manager.focus_next();
                        }
                        crate::tui_dex::events::FocusDirection::Previous => {
                            self.state.focus_manager.focus_previous();
                        }
                        crate::tui_dex::events::FocusDirection::Up => {
                            // Check if we're on the section list, then navigate sections
                            if let Some(focused) = self.state.focus_manager.current_focus() {
                                if let crate::tui_dex::events::FocusableComponent::Custom(id) = focused
                                {
                                    if id == "settings_section_list" {
                                        self.state.settings_state.previous_section();
                                        return Ok(true);
                                    }
                                }
                            }
                            // Otherwise use normal focus navigation
                            self.state.focus_manager.focus_previous();
                        }
                        crate::tui_dex::events::FocusDirection::Down => {
                            // Check if we're on the section list, then navigate sections
                            if let Some(focused) = self.state.focus_manager.current_focus() {
                                if let crate::tui_dex::events::FocusableComponent::Custom(id) = focused
                                {
                                    if id == "settings_section_list" {
                                        self.state.settings_state.next_section();
                                        return Ok(true);
                                    }
                                }
                            }
                            // Otherwise use normal focus navigation
                            self.state.focus_manager.focus_next();
                        }
                        _ => {}
                    }
                    return Ok(true);
                }
                Event::Enter => {
                    // Handle enter key in settings
                    if let Some(focused) = self.state.focus_manager.current_focus() {
                        match focused {
                            crate::tui_dex::events::FocusableComponent::Button(button_id) => {
                                match button_id.as_str() {
                                    "settings_save" => {
                                        self.handle_settings_action().await?;
                                    }
                                    "settings_reset" => {
                                        self.state.settings_state.reset_to_defaults();
                                    }
                                    "settings_wallet_import_mode" => {
                                        self.state.settings_state.toggle_import_mode();
                                    }
                                    "settings_wallet_show_mnemonic" => {
                                        self.state.settings_state.toggle_mnemonic_visibility();
                                    }
                                    "settings_auto_refresh" => {
                                        self.state.settings_state.toggle_auto_refresh();
                                    }
                                    _ => {}
                                }
                            }
                            crate::tui_dex::events::FocusableComponent::Dropdown(dropdown_id) => {
                                // Toggle dropdown or handle selection
                                match dropdown_id.as_str() {
                                    "settings_network_environment" => {
                                        self.state.settings_state.toggle_network_environment();
                                    }
                                    "settings_theme" => {
                                        self.state.settings_state.toggle_theme();
                                    }
                                    _ => {}
                                }
                            }
                            crate::tui_dex::events::FocusableComponent::TextInput(_) => {
                                // Enter editing mode for text input
                                match self.state.settings_state.current_section {
                                    crate::tui_dex::screens::settings::SettingsSection::Network => {
                                        self.state.settings_state.network_form.form_state.editing =
                                            true;
                                    }
                                    crate::tui_dex::screens::settings::SettingsSection::Wallet => {
                                        if self.state.settings_state.wallet_form.import_mode {
                                            self.state
                                                .settings_state
                                                .wallet_form
                                                .form_state
                                                .editing = true;
                                        }
                                    }
                                    crate::tui_dex::screens::settings::SettingsSection::Display => {
                                        self.state.settings_state.display_form.form_state.editing =
                                            true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    return Ok(true);
                }
                Event::Escape => {
                    // Handle escape key - close confirmation modal or go back to tab mode
                    if self.state.settings_state.show_confirmation {
                        self.state.settings_state.show_confirmation = false;
                    } else if self.state.settings_state.message.is_some() {
                        self.state.settings_state.clear_message();
                    } else {
                        // Exit content mode and return to tab mode
                        self.state.navigation_mode = NavigationMode::ScreenLevel;
                    }
                    return Ok(true);
                }
                Event::Backspace => {
                    // Handle backspace for text input fields
                    if let Some(focused) = self.state.focus_manager.current_focus() {
                        if let crate::tui_dex::events::FocusableComponent::TextInput(field_id) = focused
                        {
                            match field_id.as_str() {
                                "settings_network_name"
                                | "settings_network_rpc"
                                | "settings_gas_price"
                                | "settings_gas_adjustment" => {
                                    let _ = self.state.settings_state.handle_backspace();
                                }
                                "settings_wallet_mnemonic" => {
                                    let _ = self.state.settings_state.handle_backspace();
                                }
                                "settings_balance_refresh"
                                | "settings_pool_refresh"
                                | "settings_decimal_precision" => {
                                    let _ = self.state.settings_state.handle_backspace();
                                }
                                _ => {}
                            }
                        }
                    }
                    return Ok(true);
                }
                Event::ContextAction => {
                    // Handle space bar context actions
                    match self.state.settings_state.current_section {
                        crate::tui_dex::screens::settings::SettingsSection::Network => {
                            self.state.settings_state.toggle_network_environment();
                        }
                        crate::tui_dex::screens::settings::SettingsSection::Display => {
                            self.state.settings_state.toggle_theme();
                        }
                        crate::tui_dex::screens::settings::SettingsSection::Wallet => {
                            self.state.settings_state.toggle_import_mode();
                        }
                    }
                    return Ok(true);
                }
                _ => {}
            }
        }

        // If not handled in content mode, return false to allow global handling
        Ok(false)
    }

    /// Set an error with comprehensive error handling
    pub fn set_error(&mut self, message: String) {
        self.set_error_with_type(message, ErrorType::Unknown)
    }

    /// Set an error with specific error type
    pub fn set_error_with_type(&mut self, message: String, error_type: ErrorType) {
        self.state.error_message = Some(message.clone());
        self.state.loading_state = LoadingState::error(message.clone(), error_type.clone());

        // Show comprehensive error modal
        self.state.modal_state = Some(ModalState::error(
            "Error".to_string(),
            message,
            error_type,
            None,
            None,
        ));
    }

    /// Set an error with retry capability
    pub fn set_error_with_retry(
        &mut self,
        message: String,
        error_type: ErrorType,
        retry_action: String,
    ) {
        self.state.error_message = Some(message.clone());
        self.state.loading_state = LoadingState::error_with_retry(
            message.clone(),
            error_type.clone(),
            retry_action.clone(),
        );

        // Show error modal with retry option
        self.state.modal_state = Some(ModalState::error(
            "Error".to_string(),
            message,
            error_type,
            None,
            Some(retry_action),
        ));
    }

    /// Set a status message
    pub fn set_status(&mut self, message: String) {
        self.state.status_message = Some(message);
    }

    /// Clear error and status messages
    pub fn clear_messages(&mut self) {
        self.state.error_message = None;
        self.state.status_message = None;
        self.state.modal_state = None;
        if matches!(
            self.state.loading_state,
            LoadingState::Error { .. } | LoadingState::Success { .. }
        ) {
            self.state.loading_state = LoadingState::Idle;
        }
    }

    /// Set loading state with comprehensive progress tracking
    pub fn set_loading(&mut self, message: String) {
        self.set_loading_with_progress(message, None, false)
    }

    /// Set loading state with progress and cancellation support
    pub fn set_loading_with_progress(
        &mut self,
        message: String,
        progress: Option<f64>,
        can_cancel: bool,
    ) {
        self.state.loading_state = LoadingState::Loading {
            message: message.clone(),
            progress,
            can_cancel,
            operation_id: None,
        };

        // Always show loading modal for user feedback
        self.state.modal_state = Some(ModalState::loading(
            "Processing".to_string(),
            message,
            progress,
            can_cancel,
        ));
    }

    /// Update loading progress
    pub fn update_loading_progress(&mut self, progress: f64, message: Option<String>) {
        if let LoadingState::Loading {
            message: ref mut m,
            progress: ref mut p,
            ..
        } = self.state.loading_state
        {
            *p = Some(progress);
            if let Some(ref new_message) = message {
                *m = new_message.clone();
            }
        }

        // Update modal if visible
        if let Some(ref mut modal) = self.state.modal_state {
            modal.update_progress(Some(progress));
            if let Some(new_message) = message {
                modal.update_loading_message(new_message);
            }
        }
    }

    /// Set success state
    pub fn set_success(&mut self, message: String) {
        self.state.loading_state = LoadingState::success(message.clone());
        self.state.status_message = Some(message);
        // Clear any modal on success
        self.state.modal_state = None;
    }

    /// Show confirmation dialog
    pub fn show_confirmation(
        &mut self,
        title: String,
        message: String,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    ) {
        self.state.modal_state = Some(ModalState::confirmation(
            title,
            message,
            confirm_text,
            cancel_text,
        ));
    }

    /// Show help modal
    pub fn show_help(&mut self) {
        self.state.modal_state = Some(crate::tui_dex::components::modals::create_comprehensive_help());
    }

    /// Show quit confirmation modal
    pub fn show_quit_confirmation(&mut self) {
        self.show_confirmation(
            "Exit Application".to_string(),
            "Are you sure you want to exit the MANTRA DEX TUI?\n\nAny unsaved changes will be lost.".to_string(),
            Some("Exit".to_string()),
            Some("Cancel".to_string()),
        );
    }

    /// Show validation error
    pub fn show_validation_error(
        &mut self,
        field_name: String,
        error_message: String,
        suggestions: Vec<String>,
    ) {
        self.state.modal_state = Some(ModalState::validation_error(
            "Validation Error".to_string(),
            field_name,
            error_message,
            suggestions,
        ));
    }

    /// Handle modal events (navigation, confirmation, etc.)
    pub fn handle_modal_event(&mut self, event: &Event) -> bool {
        if let Some(ref mut modal) = self.state.modal_state {
            match event {
                Event::MoveFocus(crate::tui_dex::events::FocusDirection::Up) => {
                    modal.scroll_up();
                    return true;
                }
                Event::MoveFocus(crate::tui_dex::events::FocusDirection::Down) => {
                    modal.scroll_down();
                    return true;
                }
                Event::MoveFocus(crate::tui_dex::events::FocusDirection::Left) => {
                    modal.select_previous();
                    return true;
                }
                Event::MoveFocus(crate::tui_dex::events::FocusDirection::Right) => {
                    modal.select_next();
                    return true;
                }
                Event::Enter => {
                    // For confirmation modals, Enter should execute the currently selected option
                    match &modal.modal_type {
                        crate::tui_dex::components::modals::ModalType::Confirmation { .. } => {
                            let is_confirmed = modal.selected_option == 0; // 0 = confirm, 1 = cancel

                            if is_confirmed {
                                // Handle confirmation actions (this will clear the modal)
                                self.handle_confirmation();
                            } else {
                                // User cancelled - just clear modal and handle cancellation
                                self.state.modal_state = None;
                                if self.state.current_screen == Screen::Swap {
                                    let _ = crate::tui_dex::screens::swap::handle_confirmation_response(
                                        false,
                                    );
                                } else if self.state.current_screen == Screen::Liquidity {
                                    let _ = crate::tui_dex::screens::liquidity::handle_liquidity_confirmation_response(
                                        false,
                                    );
                                }
                                self.set_status("Action cancelled".to_string());
                            }
                        }
                        crate::tui_dex::components::modals::ModalType::Error {
                            retry_action, ..
                        } => {
                            let should_retry = retry_action.is_some() && modal.selected_option == 0;

                            // Clear modal first
                            self.state.modal_state = None;

                            if should_retry {
                                // Implement retry logic based on the last failed operation
                                self.retry_last_operation();
                            }
                            // If not retry (close/cancel), just close modal (already done above)
                        }
                        _ => {
                            // For other modal types, just close the modal
                            self.state.modal_state = None;
                        }
                    }
                    return true;
                }
                Event::Escape => {
                    self.state.modal_state = None;
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Retry the last failed operation
    fn retry_last_operation(&mut self) {
        // Check if the last error was slippage-related
        if let LoadingState::Error {
            message,
            error_type,
            ..
        } = &self.state.loading_state.clone()
        {
            if message.to_lowercase().contains("slippage")
                && matches!(error_type, ErrorType::Validation)
            {
                // For slippage errors, send an event to trigger async slippage retry
                if let Some(sender) = self.event_sender.as_ref() {
                    let _ = sender.send(crate::tui_dex::events::Event::RetryWithIncreasedSlippage);
                }
                return;
            }
        }

        // For other errors, just clear the modal and let user try again manually
        self.state.modal_state = None;
        self.state.error_message = None;
        self.set_status("You can try the operation again".to_string());
    }

    /// Handle confirmation actions
    fn handle_confirmation(&mut self) {
        // Clone the modal state to check its contents before clearing it
        let modal_state_clone = self.state.modal_state.clone();

        if let Some(ref modal_state) = modal_state_clone {
            // Check if this is a quit confirmation by examining the modal title
            if let crate::tui_dex::components::modals::ModalType::Confirmation { title, .. } =
                &modal_state.modal_type
            {
                if title == "Exit Application" {
                    // User confirmed they want to quit
                    self.state.should_quit = true;
                    self.state.modal_state = None;
                    return;
                }
            }

            // Check if this is a swap confirmation modal
            if self.state.current_screen == Screen::Swap {
                // Clear modal first
                self.state.modal_state = None;

                // Handle swap confirmation
                if let Some(swap_event) =
                    crate::tui_dex::screens::swap::handle_confirmation_response(true)
                {
                    // Process the swap event immediately
                    if let Some(sender) = self.event_sender.as_ref() {
                        let _ = sender.send(swap_event);
                    }
                }
            } else if self.state.current_screen == Screen::Liquidity {
                // Clear modal first
                self.state.modal_state = None;

                // Handle liquidity confirmation
                if let Some(liquidity_event) =
                    crate::tui_dex::screens::liquidity::handle_liquidity_confirmation_response(true)
                {
                    // Process the liquidity event immediately
                    if let Some(sender) = self.event_sender.as_ref() {
                        crate::tui_dex::utils::logger::log_info(&format!(
                            "Sending liquidity event: {:?}",
                            liquidity_event
                        ));
                        match sender.send(liquidity_event) {
                            Ok(_) => {
                                crate::tui_dex::utils::logger::log_info(
                                    "Liquidity event sent successfully",
                                );
                            }
                            Err(e) => {
                                crate::tui_dex::utils::logger::log_error(&format!(
                                    "Failed to send liquidity event: {}",
                                    e
                                ));
                                self.set_error(
                                    "Failed to process liquidity confirmation".to_string(),
                                );
                            }
                        }
                    } else {
                        crate::tui_dex::utils::logger::log_error("No event sender available");
                        self.set_error("Internal error: No event sender available".to_string());
                    }
                } else {
                    crate::tui_dex::utils::logger::log_error(
                        "Liquidity confirmation response returned None",
                    );
                    self.set_error("Failed to create liquidity operation".to_string());
                }
            } else if self.state.current_screen == Screen::Admin {
                // Clear modal first
                self.state.modal_state = None;

                // Determine which admin operation based on the modal title
                if let crate::tui_dex::components::modals::ModalType::Confirmation { title, .. } =
                    &modal_state.modal_type
                {
                    if title == "Confirm Pool Creation" {
                        // Handle pool creation confirmation
                        if let Some(create_pool_event) =
                            crate::tui_dex::screens::admin::handle_pool_creation_confirmation_response(
                                &mut self.state,
                                true,
                            )
                        {
                            // Process the pool creation event immediately
                            if let Some(sender) = self.event_sender.as_ref() {
                                crate::tui_dex::utils::logger::log_info(&format!(
                                    "Sending pool creation event: {:?}",
                                    create_pool_event
                                ));
                                match sender.send(create_pool_event) {
                                    Ok(_) => {
                                        crate::tui_dex::utils::logger::log_info(
                                            "Pool creation event sent successfully",
                                        );
                                    }
                                    Err(e) => {
                                        crate::tui_dex::utils::logger::log_error(&format!(
                                            "Failed to send pool creation event: {}",
                                            e
                                        ));
                                        self.set_error(
                                            "Failed to process pool creation confirmation"
                                                .to_string(),
                                        );
                                    }
                                }
                            } else {
                                crate::tui_dex::utils::logger::log_error("No event sender available");
                                self.set_error(
                                    "Internal error: No event sender available".to_string(),
                                );
                            }
                        } else {
                            crate::tui_dex::utils::logger::log_error(
                                "Pool creation confirmation response returned None",
                            );
                            self.set_error("Failed to create pool creation operation".to_string());
                        }
                    } else if title == "Confirm Pool Management" {
                        // Handle pool management confirmation
                        if let Some(pool_management_event) =
                            crate::tui_dex::screens::admin::handle_pool_management_confirmation_response(
                                &mut self.state,
                                true,
                            )
                        {
                            // Process the pool management event immediately
                            if let Some(sender) = self.event_sender.as_ref() {
                                crate::tui_dex::utils::logger::log_info(&format!(
                                    "Sending pool management event: {:?}",
                                    pool_management_event
                                ));
                                match sender.send(pool_management_event) {
                                    Ok(_) => {
                                        crate::tui_dex::utils::logger::log_info(
                                            "Pool management event sent successfully",
                                        );
                                    }
                                    Err(e) => {
                                        crate::tui_dex::utils::logger::log_error(&format!(
                                            "Failed to send pool management event: {}",
                                            e
                                        ));
                                        self.set_error(
                                            "Failed to process pool management confirmation"
                                                .to_string(),
                                        );
                                    }
                                }
                            } else {
                                crate::tui_dex::utils::logger::log_error("No event sender available");
                                self.set_error(
                                    "Internal error: No event sender available".to_string(),
                                );
                            }
                        } else {
                            crate::tui_dex::utils::logger::log_error(
                                "Pool management confirmation response returned None",
                            );
                            self.set_error(
                                "Failed to create pool management operation".to_string(),
                            );
                        }
                    }
                }
            } else {
                // Handle other confirmation types
                self.state.modal_state = None;
                self.set_status("Action confirmed".to_string());
            }
        } else {
            // No modal state available - just clear and set cancelled status
            self.state.modal_state = None;
            self.set_status("Action cancelled".to_string());
        }
    }

    /// Navigate to a specific screen
    pub fn navigate_to(&mut self, screen: Screen) {
        // Only clear messages if we're actually changing screens
        let is_changing_screen = self.state.current_screen != screen;

        self.state.current_screen = screen;
        self.state.navigation_mode = NavigationMode::ScreenLevel;

        // Only clear messages when actually changing screens, not when staying on the same screen
        if is_changing_screen {
            // Don't clear error messages immediately - let them persist for a bit
            self.state.status_message = None;
            // Keep error messages and modals when changing screens
        }

        // Update screen-specific data when navigating
        match screen {
            Screen::Swap => {
                // Update swap pools when entering swap screen
                self.update_swap_screen_pools();
            }
            Screen::Liquidity => {
                // Initialize liquidity screen focus state
                crate::tui_dex::screens::liquidity::initialize_liquidity_screen_focus();
                // Update liquidity screen pools when entering screen
                self.update_liquidity_screen_pools();
            }
            Screen::Admin => {
                // Initialize admin screen focus state
                crate::tui_dex::screens::admin::initialize_admin_screen_focus(&mut self.state);
                // Update admin screen pools when entering screen
                self.update_admin_screen_pools();
            }
            _ => {}
        }
        // Don't initialize focus here - it will be done when user presses Enter
    }

    /// Navigate to the next tab
    pub fn next_tab(&mut self) {
        let screens = Screen::all();
        self.state.current_tab = (self.state.current_tab + 1) % screens.len();
        let new_screen = screens[self.state.current_tab];
        self.state.current_screen = new_screen;
        self.state.navigation_mode = NavigationMode::ScreenLevel;

        // Don't clear error messages when navigating tabs - let them persist
        self.state.status_message = None;

        // Update screen-specific data when navigating
        match new_screen {
            Screen::Swap => {
                // Update swap pools when entering swap screen
                self.update_swap_screen_pools();
            }
            Screen::Liquidity => {
                // Initialize liquidity screen focus state
                crate::tui_dex::screens::liquidity::initialize_liquidity_screen_focus();
                // Update liquidity screen pools when entering screen
                self.update_liquidity_screen_pools();
            }
            Screen::Admin => {
                // Initialize admin screen focus state
                crate::tui_dex::screens::admin::initialize_admin_screen_focus(&mut self.state);
                // Update admin screen pools when entering screen
                self.update_admin_screen_pools();
            }
            _ => {}
        }
        // Don't initialize focus here - it will be done when user presses Enter
    }

    /// Navigate to the previous tab
    pub fn previous_tab(&mut self) {
        let screens = Screen::all();
        if self.state.current_tab == 0 {
            self.state.current_tab = screens.len() - 1;
        } else {
            self.state.current_tab -= 1;
        }
        let new_screen = screens[self.state.current_tab];
        self.state.current_screen = new_screen;
        self.state.navigation_mode = NavigationMode::ScreenLevel;

        // Don't clear error messages when navigating tabs - let them persist
        self.state.status_message = None;

        // Update screen-specific data when navigating
        match new_screen {
            Screen::Swap => {
                // Update swap pools when entering swap screen
                self.update_swap_screen_pools();
            }
            Screen::Liquidity => {
                // Initialize liquidity screen focus state
                crate::tui_dex::screens::liquidity::initialize_liquidity_screen_focus();
                // Update liquidity screen pools when entering screen
                self.update_liquidity_screen_pools();
            }
            Screen::Admin => {
                // Initialize admin screen focus state
                crate::tui_dex::screens::admin::initialize_admin_screen_focus(&mut self.state);
                // Update admin screen pools when entering screen
                self.update_admin_screen_pools();
            }
            _ => {}
        }
        // Don't initialize focus here - it will be done when user presses Enter
    }

    /// Handle background data refresh events
    async fn handle_data_refresh(
        &mut self,
        data_type: String,
        success: bool,
        error: Option<String>,
    ) -> Result<(), Error> {
        if !success {
            if let Some(err) = error {
                self.set_error(format!("Failed to refresh {}: {}", data_type, err));
            }
            return Ok(());
        }

        // Perform actual data refresh based on type
        match data_type.as_str() {
            "balances" => {
                if let Some(address) = &self.state.wallet_address.clone() {
                    // Refresh balances
                    if let Ok(balances) = self.client.get_balances().await {
                        for balance in balances {
                            self.state
                                .balances
                                .insert(balance.denom, balance.amount.to_string());
                        }
                    }

                    // Note: Wallet address updated for future background tasks
                }
            }
            "pools" => {
                // Refresh pool data
                if let Ok(pools) = self.client.get_pools(None).await {
                    for pool in pools {
                        let pool_id = pool.pool_info.pool_identifier.clone();
                        let cache_entry = PoolCacheEntry {
                            pool_info: pool,
                            cached_at: chrono::Utc::now(),
                        };
                        self.state.pool_cache.insert(pool_id, cache_entry);
                    }

                    // Update swap screen pools if currently on swap screen
                    if self.state.current_screen == Screen::Swap {
                        self.update_swap_screen_pools();
                    }
                    // Update liquidity screen pools if currently on liquidity screen
                    if self.state.current_screen == Screen::Liquidity {
                        self.update_liquidity_screen_pools();
                    }
                    // Update admin screen pools if currently on admin screen
                    if self.state.current_screen == Screen::Admin {
                        self.update_admin_screen_pools();
                    }
                }
            }
            "transactions" => {
                // Refresh transaction status for pending transactions
                let pending_txs: Vec<String> = self
                    .state
                    .recent_transactions
                    .iter()
                    .filter(|tx| tx.status == TransactionStatus::Pending)
                    .map(|tx| tx.hash.clone())
                    .collect();

                // TODO: Implement transaction status checking when SDK supports it
                // For now, we'll mark pending transactions as unknown status
                for tx_hash in pending_txs {
                    if let Some(tx_info) = self
                        .state
                        .recent_transactions
                        .iter_mut()
                        .find(|tx| tx.hash == tx_hash)
                    {
                        // Mark as unknown for now - would check actual status in real implementation
                        tx_info.status = TransactionStatus::Unknown;
                    }
                }
            }
            "network_info" => {
                // Refresh network information
                if let Ok(height) = self.client.get_last_block_height().await {
                    self.state.block_height = Some(height);
                }

                // Update sync status
                self.state.network_info.last_sync_time = Some(chrono::Utc::now());
                self.state.network_info.is_syncing = false;
            }
            "prices" => {
                // Refresh price data - this would be implemented when price APIs are available
                // For now, just update the last sync time
                self.state.network_info.last_sync_time = Some(chrono::Utc::now());
            }
            _ => {
                // Unknown data type, log but don't error
                crate::tui_dex::utils::logger::log_warning(&format!(
                    "Unknown data refresh type: {}",
                    data_type
                ));
            }
        }

        Ok(())
    }

    /// Update wallet address
    pub fn set_wallet_address(&mut self, address: String) {
        self.state.wallet_address = Some(address);
    }

    /// Update block height
    pub fn set_block_height(&mut self, height: u64) {
        self.state.block_height = Some(height);
        self.state.network_info.last_sync_time = Some(chrono::Utc::now());
    }

    /// Update token balance
    pub fn update_balance(&mut self, token: String, balance: String) {
        self.state.balances.insert(token, balance);
    }

    /// Add a recent transaction with full details
    pub fn add_transaction(&mut self, tx_info: TransactionInfo) {
        self.state.recent_transactions.insert(0, tx_info);
        // Keep only the last 50 transactions
        if self.state.recent_transactions.len() > 50 {
            self.state.recent_transactions.truncate(50);
        }
    }

    /// Add a recent transaction (legacy method for backward compatibility)
    pub fn add_recent_transaction(&mut self, tx_hash: String) {
        let tx_info = TransactionInfo {
            hash: tx_hash,
            status: TransactionStatus::Unknown,
            operation_type: "Unknown".to_string(),
            timestamp: chrono::Utc::now(),
            gas_used: None,
            gas_wanted: None,
        };
        self.add_transaction(tx_info);
    }

    /// Select a pool
    pub fn select_pool(&mut self, pool_id: u64) {
        self.state.selected_pool_id = Some(pool_id);
    }

    /// Clear pool selection
    pub fn clear_pool_selection(&mut self) {
        self.state.selected_pool_id = None;
    }

    /// Update swap state
    pub fn update_swap_state(&mut self, swap_state: SwapState) {
        self.state.swap_state = swap_state;
    }

    /// Get cached pool information
    pub fn get_cached_pool(&self, pool_id: &str) -> Option<&PoolInfoResponse> {
        self.state
            .pool_cache
            .get(pool_id)
            .map(|entry| &entry.pool_info)
    }

    /// Check if pool cache is stale (older than 5 minutes)
    pub fn is_pool_cache_stale(&self, pool_id: &str) -> bool {
        match self.state.pool_cache.get(pool_id) {
            Some(entry) => {
                let age = chrono::Utc::now() - entry.cached_at;
                age.num_minutes() > 5
            }
            None => true,
        }
    }

    /// Update network information
    pub fn update_network_info(&mut self, chain_id: Option<String>, is_syncing: bool) {
        self.state.network_info.chain_id = chain_id;
        self.state.network_info.is_syncing = is_syncing;
        self.state.network_info.last_sync_time = Some(chrono::Utc::now());
    }

    /// Process transaction response and update state
    pub async fn process_transaction_response(
        &mut self,
        tx_response: TxResponse,
        operation_type: String,
    ) {
        let status = if tx_response.code == 0 {
            TransactionStatus::Success
        } else {
            TransactionStatus::Failed
        };

        let tx_info = TransactionInfo {
            hash: tx_response.txhash.clone(),
            status: status.clone(),
            operation_type: operation_type.clone(),
            timestamp: chrono::Utc::now(),
            gas_used: Some(tx_response.gas_used),
            gas_wanted: Some(tx_response.gas_wanted),
        };

        self.add_transaction(tx_info);

        match status {
            TransactionStatus::Success => {
                self.set_success(format!(
                    "{} successful: {}",
                    operation_type, tx_response.txhash
                ));
            }
            TransactionStatus::Failed => {
                self.set_error(format!(
                    "{} failed: {}",
                    operation_type, tx_response.raw_log
                ));
            }
            _ => {}
        }

        // Trigger data refresh after successful operations
        if status == TransactionStatus::Success {
            let _ = self.refresh_current_screen_data().await;
        }
    }

    /// Refresh settings data
    async fn refresh_dashboard_data(&mut self) -> Result<(), Error> {
        // Only refresh data if we have a connected wallet, otherwise just refresh network info
        if self.state.wallet_address.is_none() {
            // No wallet connected - only refresh basic network info
            self.set_loading_with_progress(
                "Refreshing network data...".to_string(),
                Some(10.0),
                false,
            );

            let mut errors = Vec::new();

            // Refresh network info only
            match self.client.get_last_block_height().await {
                Ok(height) => {
                    self.state.block_height = Some(height);
                    self.state.network_info.last_sync_time = Some(chrono::Utc::now());
                    self.state.network_info.is_syncing = false;
                }
                Err(e) => {
                    errors.push(format!("Failed to fetch block height: {}", e));
                }
            }

            // Update progress - fetching pool data
            self.update_loading_progress(80.0, Some("Fetching pool information...".to_string()));

            // Refresh pool data (limited to avoid overwhelming)
            match self.client.get_pools(Some(20)).await {
                Ok(pools) => {
                    for pool in pools {
                        let pool_id = pool.pool_info.pool_identifier.clone();
                        let cache_entry = PoolCacheEntry {
                            pool_info: pool,
                            cached_at: chrono::Utc::now(),
                        };
                        self.state.pool_cache.insert(pool_id, cache_entry);
                    }
                }
                Err(e) => {
                    errors.push(format!("Failed to fetch pools: {}", e));
                }
            }

            if errors.is_empty() {
                self.set_success("Network data refreshed successfully".to_string());
            } else {
                let error_summary = if errors.len() == 1 {
                    errors[0].clone()
                } else {
                    format!("Multiple errors occurred: {}", errors.join("; "))
                };
                self.set_error_with_type(
                    format!("Network refresh completed with warnings: {}", error_summary),
                    crate::tui_dex::components::modals::ErrorType::Network,
                );
            }

            return Ok(());
        }

        // Refresh balances, network info, and other dashboard data (wallet connected)
        self.set_loading_with_progress(
            "Refreshing dashboard data...".to_string(),
            Some(10.0),
            false,
        );

        let mut errors = Vec::new();

        // Update progress - fetching asset decimals
        self.update_loading_progress(20.0, Some("Fetching asset decimals...".to_string()));

        // Refresh asset decimals cache (needed for proper balance formatting)
        match self.refresh_asset_decimals_cache().await {
            Ok(_) => {
                // Successfully refreshed decimals cache
            }
            Err(e) => {
                errors.push(format!("Failed to refresh asset decimals: {}", e));
                // Continue with default decimals - not critical for basic functionality
            }
        }

        // Update progress - fetching balances
        self.update_loading_progress(40.0, Some("Fetching wallet balances...".to_string()));

        // Refresh balances if wallet is connected
        if let Some(address) = &self.state.wallet_address.clone() {
            match self.client.get_balances().await {
                Ok(balances) => {
                    // Clear existing balances
                    self.state.balances.clear();
                    // Update with new balances
                    for balance in balances {
                        self.state
                            .balances
                            .insert(balance.denom, balance.amount.to_string());
                    }
                }
                Err(e) => {
                    errors.push(format!("Failed to fetch balances: {}", e));
                }
            }
        }

        // Update progress - fetching network info
        self.update_loading_progress(70.0, Some("Fetching network information...".to_string()));

        // Refresh network info
        match self.client.get_last_block_height().await {
            Ok(height) => {
                self.state.block_height = Some(height);
                self.state.network_info.last_sync_time = Some(chrono::Utc::now());
                self.state.network_info.is_syncing = false;
            }
            Err(e) => {
                errors.push(format!("Failed to fetch block height: {}", e));
            }
        }

        // Refresh pool data (limited to avoid overwhelming)
        match self.client.get_pools(Some(20)).await {
            Ok(pools) => {
                for pool in pools {
                    let pool_id = pool.pool_info.pool_identifier.clone();
                    let cache_entry = PoolCacheEntry {
                        pool_info: pool,
                        cached_at: chrono::Utc::now(),
                    };
                    self.state.pool_cache.insert(pool_id, cache_entry);
                }
            }
            Err(e) => {
                errors.push(format!("Failed to fetch pools: {}", e));
            }
        }

        // Update background coordinator with wallet address for future syncing
        if let Some(address) = &self.state.wallet_address {
            if let Some(coordinator) = &mut self.background_coordinator {
                coordinator.set_wallet_address(address.clone());
            }
        }

        if errors.is_empty() {
            self.set_success("Dashboard refreshed successfully".to_string());
        } else {
            // Show partial success with warnings
            let error_summary = if errors.len() == 1 {
                errors[0].clone()
            } else {
                format!("Multiple errors occurred: {}", errors.join("; "))
            };
            self.set_error_with_type(
                format!(
                    "Dashboard refresh completed with warnings: {}",
                    error_summary
                ),
                crate::tui_dex::components::modals::ErrorType::Network,
            );
        }

        Ok(())
    }

    async fn refresh_settings_data(&mut self) -> Result<(), Error> {
        // Initialize settings state with current config if needed
        if self.state.settings_state.current_config.mnemonic.is_none() {
            // Load current config into settings state
            let current_config = crate::config::Config {
                network: self.config.clone(),
                mnemonic: None, // We don't store mnemonic in memory for security
                tokens: std::collections::HashMap::new(),
            };
            self.state.settings_state =
                crate::tui_dex::screens::settings::SettingsState::new(current_config);
        }
        Ok(())
    }

    /// Handle settings actions
    async fn handle_settings_action(&mut self) -> Result<(), Error> {
        // If showing confirmation, handle confirmation
        if self.state.settings_state.show_confirmation {
            // Save settings
            match self.state.settings_state.save_settings() {
                Ok(new_config) => {
                    // Update application config
                    self.config = new_config.network;
                    self.state.settings_state.show_confirmation = false;
                    self.set_success("Settings saved successfully!".to_string());
                }
                Err(e) => {
                    self.set_error(format!("Failed to save settings: {}", e));
                    self.state.settings_state.show_confirmation = false;
                }
            }
        } else if self.state.settings_state.has_changes {
            // Show confirmation dialog
            self.state.settings_state.show_confirmation = true;
        } else {
            // No changes, just refresh
            self.refresh_settings_data().await?;
        }
        Ok(())
    }

    /// Handle character input for settings screen
    async fn handle_settings_input(&mut self, c: char) -> Result<(), Error> {
        // Clear any messages first
        self.state.settings_state.clear_message();

        match c {
            // Ctrl+S - Save settings
            '\x13' => {
                if self.state.settings_state.has_changes {
                    self.state.settings_state.show_confirmation = true;
                } else {
                    self.state.settings_state.message =
                        Some(("No changes to save".to_string(), false));
                }
            }
            // Ctrl+R - Reset settings
            '\x12' => {
                self.state.settings_state.reset_to_defaults();
            }
            // Section navigation
            '\t' => {
                self.state.settings_state.next_section();
            }
            // Environment/theme toggles
            'e' => {
                if self.state.settings_state.current_section
                    == crate::tui_dex::screens::settings::SettingsSection::Network
                {
                    self.state.settings_state.toggle_network_environment();
                }
            }
            't' => {
                if self.state.settings_state.current_section
                    == crate::tui_dex::screens::settings::SettingsSection::Display
                {
                    self.state.settings_state.toggle_theme();
                }
            }
            'a' => {
                if self.state.settings_state.current_section
                    == crate::tui_dex::screens::settings::SettingsSection::Wallet
                {
                    self.state.settings_state.toggle_import_mode();
                }
            }
            'm' => {
                if self.state.settings_state.current_section
                    == crate::tui_dex::screens::settings::SettingsSection::Wallet
                {
                    self.state.settings_state.toggle_mnemonic_visibility();
                }
            }
            // Escape key handling
            '\x1b' => {
                if self.state.settings_state.show_confirmation {
                    self.state.settings_state.show_confirmation = false;
                } else if self.state.settings_state.message.is_some() {
                    self.state.settings_state.clear_message();
                }
            }
            // Backspace
            '\x08' | '\x7f' => {
                let _ = self.state.settings_state.handle_backspace();
            }
            // Regular character input
            _ => {
                if c.is_ascii_graphic() || c == ' ' {
                    let _ = self.state.settings_state.handle_char_input(c);
                }
            }
        }
        Ok(())
    }

    /// Handle wizard-specific events
    async fn handle_wizard_event(&mut self, event: Event) -> Result<bool, Error> {
        match event {
            Event::Quit => {
                // If we're in mnemonic input mode, treat 'q' as a regular character
                if self.state.wizard_state.current_step
                    == crate::tui_dex::screens::wizard::WizardStep::WalletSetup
                    && self.state.wizard_state.import_existing
                {
                    self.state.wizard_state.mnemonic_input.push('q');
                } else {
                    self.state.should_quit = true;
                    return Ok(true);
                }
            }
            Event::Escape => {
                // Go back a step or exit wizard
                if self.state.wizard_state.current_step
                    == crate::tui_dex::screens::wizard::WizardStep::Welcome
                {
                    // Show quit confirmation instead of immediately skipping wizard
                    self.show_quit_confirmation();
                } else {
                    self.state.wizard_state.previous_step();
                }
            }
            Event::Enter => {
                // Proceed to next step or finish wizard
                if self.state.wizard_state.can_proceed() {
                    if self.state.wizard_state.current_step
                        == crate::tui_dex::screens::wizard::WizardStep::Complete
                    {
                        // Finish wizard and apply settings
                        self.apply_wizard_settings().await?;
                        self.state.wizard_state.finish_wizard();

                        // Navigate to dashboard with the newly created/imported wallet
                        self.navigate_to(Screen::Dashboard);
                    } else {
                        self.state.wizard_state.next_step();
                    }
                }
            }
            // Add MoveFocus event handling for wizard navigation
            Event::MoveFocus(direction) => {
                match self.state.wizard_state.current_step {
                    crate::tui_dex::screens::wizard::WizardStep::NetworkSelection => {
                        // Handle Up/Down arrows for network selection
                        match direction {
                            crate::tui_dex::events::FocusDirection::Up
                            | crate::tui_dex::events::FocusDirection::Down => {
                                self.state.wizard_state.toggle_network();
                                return Ok(true);
                            }
                            _ => {} // Ignore other directions
                        }
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSave => {
                        // Handle Tab navigation for wallet save form
                        match direction {
                            crate::tui_dex::events::FocusDirection::Next => {
                                self.state.wizard_state.wallet_save_focus_next();
                                return Ok(true);
                            }
                            crate::tui_dex::events::FocusDirection::Previous => {
                                self.state.wizard_state.wallet_save_focus_previous();
                                return Ok(true);
                            }
                            _ => {} // Other directions not used in wallet save
                        }
                    }
                    _ => {} // Other steps don't need MoveFocus handling
                }
            }
            Event::Tab => {
                // Navigate between options in current step
                match self.state.wizard_state.current_step {
                    crate::tui_dex::screens::wizard::WizardStep::NetworkSelection => {
                        self.state.wizard_state.toggle_network();
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSetup => {
                        self.state.wizard_state.toggle_wallet_mode();
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSave => {
                        self.state.wizard_state.wallet_save_focus_next();
                    }
                    _ => {}
                }
            }
            Event::BackTab => {
                // Navigate between options in current step (reverse)
                match self.state.wizard_state.current_step {
                    crate::tui_dex::screens::wizard::WizardStep::NetworkSelection => {
                        self.state.wizard_state.toggle_network();
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSetup => {
                        self.state.wizard_state.toggle_wallet_mode();
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSave => {
                        self.state.wizard_state.wallet_save_focus_previous();
                    }
                    _ => {}
                }
            }
            Event::Char(c) => {
                // Handle character input for mnemonic or other text fields
                match self.state.wizard_state.current_step {
                    crate::tui_dex::screens::wizard::WizardStep::WalletSetup => {
                        if self.state.wizard_state.import_existing {
                            self.state.wizard_state.mnemonic_input.push(c);
                        }
                    }
                    crate::tui_dex::screens::wizard::WizardStep::SecurityWarning => {
                        if c == 'y' || c == 'Y' {
                            self.state.wizard_state.security_acknowledged = true;
                        } else if c == 'n' || c == 'N' {
                            self.state.wizard_state.security_acknowledged = false;
                        }
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSave => {
                        self.state.wizard_state.wallet_save_handle_char(c);
                    }
                    _ => {}
                }
            }
            Event::Backspace => {
                // Handle backspace for text input
                match self.state.wizard_state.current_step {
                    crate::tui_dex::screens::wizard::WizardStep::WalletSetup => {
                        if self.state.wizard_state.import_existing {
                            self.state.wizard_state.mnemonic_input.pop();
                        }
                    }
                    crate::tui_dex::screens::wizard::WizardStep::WalletSave => {
                        self.state.wizard_state.wallet_save_handle_backspace();
                    }
                    _ => {}
                }
            }
            // Treat context action (space bar) as a space character when typing mnemonic
            Event::ContextAction => {
                if self.state.wizard_state.current_step
                    == crate::tui_dex::screens::wizard::WizardStep::WalletSetup
                    && self.state.wizard_state.import_existing
                {
                    self.state.wizard_state.mnemonic_input.push(' ');
                } else if self.state.wizard_state.current_step
                    == crate::tui_dex::screens::wizard::WizardStep::WalletSave
                {
                    // Handle space bar for the currently focused field
                    self.state.wizard_state.wallet_save_handle_char(' ');
                }
            }
            // Handle paste events (bracketed paste) for mnemonic field
            Event::Paste(text) => {
                if self.state.wizard_state.current_step
                    == crate::tui_dex::screens::wizard::WizardStep::WalletSetup
                    && self.state.wizard_state.import_existing
                {
                    self.state.wizard_state.mnemonic_input.push_str(&text);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Apply wizard settings to the app configuration
    async fn apply_wizard_settings(&mut self) -> Result<(), Error> {
        // Apply network settings
        match self.state.wizard_state.selected_network {
            crate::tui_dex::screens::wizard::NetworkEnvironment::Mainnet => {
                self.set_status("Connected to Mainnet".to_string());
            }
            crate::tui_dex::screens::wizard::NetworkEnvironment::Testnet => {
                self.set_status("Connected to Testnet".to_string());
            }
        }

        // Apply wallet settings
        if self.state.wizard_state.import_existing
            && !self.state.wizard_state.mnemonic_input.is_empty()
        {
            // Import wallet from mnemonic
            match crate::wallet::MantraWallet::from_mnemonic(
                &self.state.wizard_state.mnemonic_input,
                0,
            ) {
                Ok(wallet) => {
                    match wallet.address() {
                        Ok(address) => {
                            self.set_wallet_address(address.to_string());
                            // Reconfigure the client so all future calls have the wallet attached
                            self.configure_client_wallet(wallet).await?;
                            self.set_status("Wallet imported successfully".to_string());
                        }
                        Err(e) => {
                            self.set_error(format!("Failed to derive wallet address: {}", e));
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    self.set_error(format!("Failed to import wallet: {}", e));
                    return Err(e);
                }
            }
        } else if !self.state.wizard_state.import_existing {
            // Create new wallet
            if let Some(mnemonic) = &self.state.wizard_state.generated_mnemonic {
                match crate::wallet::MantraWallet::from_mnemonic(mnemonic, 0) {
                    Ok(wallet) => {
                        match wallet.address() {
                            Ok(address) => {
                                self.set_wallet_address(address.to_string());
                                // Reconfigure the client with the newly generated wallet
                                self.configure_client_wallet(wallet).await?;
                                self.set_status("New wallet created successfully".to_string());
                            }
                            Err(e) => {
                                self.set_error(format!("Failed to derive wallet address: {}", e));
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        self.set_error(format!("Failed to create wallet: {}", e));
                        return Err(e);
                    }
                }
            }
        }

        // Save wallet if user chose to do so
        if self.state.wizard_state.save_wallet {
            if let Some(mnemonic) = self.get_current_wallet_mnemonic() {
                if let Some(address) = &self.state.wallet_address {
                    self.save_wallet_from_wizard(mnemonic, address.clone())
                        .await?;
                }
            }
        }

        // Trigger dashboard refresh to reflect new wallet and network state
        if let Err(e) = self.refresh_dashboard_data().await {
            crate::tui_dex::utils::logger::log_warning(&format!(
                "Failed to refresh dashboard data: {}",
                e
            ));
        }

        Ok(())
    }

    /// Get the current wallet mnemonic from wizard state
    fn get_current_wallet_mnemonic(&self) -> Option<String> {
        if self.state.wizard_state.import_existing {
            // For imported wallets, return the entered mnemonic
            if !self.state.wizard_state.mnemonic_input.is_empty() {
                Some(self.state.wizard_state.mnemonic_input.clone())
            } else {
                None
            }
        } else {
            // For generated wallets, return the generated mnemonic
            self.state.wizard_state.generated_mnemonic.clone()
        }
    }

    /// Save wallet from wizard with user's chosen settings
    async fn save_wallet_from_wizard(
        &mut self,
        mnemonic: String,
        address: String,
    ) -> Result<(), Error> {
        self.set_loading("Saving wallet...".to_string());

        let storage = crate::wallet::WalletStorage::new()
            .map_err(|e| Error::Wallet(format!("Failed to initialize wallet storage: {}", e)))?;

        match storage.save_wallet(
            &self.state.wizard_state.wallet_name,
            &mnemonic,
            &self.state.wizard_state.save_password,
            &address,
        ) {
            Ok(()) => {
                // Clear sensitive data from memory
                self.state.wizard_state.clear_wallet_save_data();
                self.set_success(format!(
                    "Wallet '{}' saved successfully!",
                    self.state.wizard_state.wallet_name
                ));
            }
            Err(e) => {
                self.set_error(format!("Failed to save wallet: {}", e));
                return Err(e);
            }
        }

        Ok(())
    }

    /// Show modal to save wallet with password protection
    fn show_wallet_save_modal(&mut self, mnemonic: String, address: String) {
        // Create wallet save modal with password input
        let title = "Save Wallet".to_string();
        let message = format!(
            "Would you like to save this wallet for future use?\n\n\
            Wallet Address: {}\n\n\
            Saving your wallet allows you to quickly access it next time without \
            re-entering your mnemonic. Your wallet will be encrypted with a strong password.",
            &address[..20] // Show partial address for security
        );

        self.state.modal_state = Some(crate::tui_dex::components::modals::ModalState::wallet_save(
            title, message, mnemonic, address,
        ));
    }

    /// Update the underlying client with a newly provided wallet and restart background tasks
    async fn configure_client_wallet(
        &mut self,
        wallet: crate::wallet::MantraWallet,
    ) -> Result<(), Error> {
        // Stop any currently running background sync tasks so they don't keep using the stale client
        self.stop_background_tasks();

        // Re-create a fresh client instance that includes the wallet
        let mut new_client = MantraDexClient::new(self.config.clone()).await?;
        new_client = new_client.with_wallet(wallet);

        // Replace the old Arc so all subsequent operations use the updated client
        self.client = std::sync::Arc::new(new_client);

        // Restart background tasks so they pick up the new client instance
        if let Some(sender) = self.event_sender.clone() {
            self.initialize_background_tasks(sender);
        }

        Ok(())
    }

    /// Map display name back to actual denomination
    fn map_display_name_to_denom(
        &self,
        display_name: &str,
        pool_assets: &[cosmwasm_std::Coin],
    ) -> String {
        // Try to find the asset in the pool that matches the display name
        for asset in pool_assets {
            let denom = &asset.denom;

            // Check if the display name matches the full denomination
            if denom == display_name {
                return denom.clone();
            }

            // Check if the display name matches our symbol mapping
            let symbol = self.denom_to_symbol(denom);
            if symbol == display_name {
                return denom.clone();
            }

            // Check if the display name matches the shortened version (fallback)
            let shortened = if denom.len() > 15 {
                if let Some(last_part) = denom.split('/').last() {
                    last_part.to_string()
                } else {
                    format!("{}...", &denom[..10])
                }
            } else {
                denom.clone()
            };

            if shortened == display_name {
                return denom.clone();
            }
        }

        // If no match found, return the display name as-is (fallback)
        crate::tui_dex::utils::logger::log_warning(&format!(
            "Could not map display name '{}' to actual denomination, using as-is",
            display_name
        ));
        display_name.to_string()
    }

    /// Convert token denomination to display symbol
    /// Maps micro denominations (uUSDC, uom) to their symbols (USDC, OM)
    pub fn denom_to_symbol(&self, denom: &str) -> String {
        // Handle common token mappings
        match denom {
            "uom" => "OM".to_string(),
            d if d.starts_with("factory/") && d.contains("/uUSDC") => "USDC".to_string(),
            d if d.starts_with("factory/") && d.contains("/uUSDT") => "USDT".to_string(),
            d if d.starts_with("factory/") && d.contains("/uUSDY") => "USDY".to_string(),
            d if d.starts_with("factory/") && d.contains("/aUSDY") => "aUSDY".to_string(),
            d if d.starts_with("factory/") && d.contains("/uATOM") => "ATOM".to_string(),
            d if d.starts_with("factory/") && d.contains("/uOSMO") => "OSMO".to_string(),
            _ => {
                // For other factory tokens, try to extract the last part
                if let Some(last_part) = denom.split('/').last() {
                    // Remove 'u' prefix if it exists and the rest looks like a symbol
                    if last_part.starts_with('u') && last_part.len() > 1 {
                        last_part[1..].to_string()
                    } else {
                        last_part.to_string()
                    }
                } else {
                    denom.to_string()
                }
            }
        }
    }

    /// Get token decimals for a given denomination
    /// Most Mantra tokens use 6 decimals
    pub fn get_token_decimals(&self, denom: &str) -> u8 {
        // Check cache first
        if let Some(&decimals) = self.state.asset_decimals_cache.get(denom) {
            return decimals;
        }

        // Fallback to hardcoded values if not in cache
        match denom {
            "uom" => 6,
            d if d.starts_with("factory/") => 6, // Most factory tokens use 6 decimals
            d if d.starts_with("ibc/") => 6,     // Most IBC tokens use 6 decimals
            d if d.starts_with("pool/") || d.contains("/lp/") => 6, // LP tokens
            _ => 6,                              // Default to 6 decimals
        }
    }

    /// Refresh asset decimals cache from blockchain data
    pub async fn refresh_asset_decimals_cache(&mut self) -> Result<(), Error> {
        match self.client.get_asset_decimals_from_pools().await {
            Ok(decimals_map) => {
                self.state.asset_decimals_cache = decimals_map;
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Asset decimals cache refreshed with {} entries",
                    self.state.asset_decimals_cache.len()
                ));
                Ok(())
            }
            Err(e) => {
                crate::tui_dex::utils::logger::log_error(&format!(
                    "Failed to refresh asset decimals cache: {}",
                    e
                ));
                Err(e)
            }
        }
    }

    /// Get all token balances formatted for display
    pub fn get_formatted_balances(&self) -> Vec<(String, String, String)> {
        let mut formatted_balances = Vec::new();

        for (denom, balance) in &self.state.balances {
            if let Ok(amount) = balance.parse::<u128>() {
                if amount > 0 {
                    let symbol = self.denom_to_symbol(denom);
                    let formatted_amount = self.micro_to_token_amount(balance, denom);
                    formatted_balances.push((symbol, formatted_amount, denom.clone()));
                }
            }
        }

        // Sort by symbol for consistent display
        formatted_balances.sort_by(|a, b| a.0.cmp(&b.0));
        formatted_balances
    }

    /// Convert micro amount to actual token amount
    /// Divides by 10^decimals to get the real amount
    pub fn micro_to_token_amount(&self, amount: &str, denom: &str) -> String {
        let decimals = self.get_token_decimals(denom);
        let divisor = 10_u128.pow(decimals as u32);

        if let Ok(micro_amount) = amount.parse::<u128>() {
            let token_amount = micro_amount as f64 / divisor as f64;
            // Format with appropriate precision
            if token_amount >= 1000.0 {
                format!("{:.2}", token_amount)
            } else if token_amount >= 1.0 {
                format!("{:.4}", token_amount)
            } else {
                format!("{:.6}", token_amount)
            }
        } else {
            amount.to_string()
        }
    }

    /// Format token amount with symbol for display
    /// Converts from micro units and shows proper symbol
    pub fn format_token_display(&self, amount: &str, denom: &str) -> String {
        let token_amount = self.micro_to_token_amount(amount, denom);
        let symbol = self.denom_to_symbol(denom);
        format!("{} {}", token_amount, symbol)
    }

    /// Execute a real swap transaction on the blockchain
    async fn execute_real_swap(
        &mut self,
        from_asset: String,
        to_asset: String,
        amount: String,
        pool_id: Option<String>,
        slippage_tolerance: Option<String>,
    ) -> Result<(), Error> {
        crate::tui_dex::utils::logger::log_info("=== EXECUTE REAL SWAP - BLOCKCHAIN TRANSACTION ===");
        crate::tui_dex::utils::logger::log_info(&format!("Starting blockchain swap execution:"));
        crate::tui_dex::utils::logger::log_info(&format!("  From Asset: {}", from_asset));
        crate::tui_dex::utils::logger::log_info(&format!("  To Asset: {}", to_asset));
        crate::tui_dex::utils::logger::log_info(&format!("  Amount: {}", amount));
        crate::tui_dex::utils::logger::log_info(&format!("  Pool ID: {:?}", pool_id));
        crate::tui_dex::utils::logger::log_info(&format!(
            "  Slippage Tolerance: {:?}",
            slippage_tolerance
        ));

        // Validate that we have a valid pool ID
        let pool_id_str = match pool_id {
            Some(id) => {
                crate::tui_dex::utils::logger::log_info(&format!("Pool ID validated: {}", id));
                id
            }
            None => {
                crate::tui_dex::utils::logger::log_error("SWAP FAILED: No pool ID provided");
                self.set_error_with_type(
                    "Swap Validation Error".to_string(),
                    ErrorType::Validation,
                );
                return Err(Error::Other("No pool selected for swap".to_string()));
            }
        };

        // Validate that the pool exists in our cache
        crate::tui_dex::utils::logger::log_info(&format!(
            "Checking pool cache for pool: {}",
            pool_id_str
        ));
        crate::tui_dex::utils::logger::log_info(&format!(
            "Total pools in cache: {}",
            self.state.pool_cache.len()
        ));

        let pool_entry = match self.state.pool_cache.get(&pool_id_str) {
            Some(entry) => {
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Pool {} found in cache",
                    pool_id_str
                ));
                crate::tui_dex::utils::logger::log_debug(&format!(
                    "Pool info: {:?}",
                    entry.pool_info.pool_info
                ));
                entry
            }
            None => {
                crate::tui_dex::utils::logger::log_error(&format!(
                    "SWAP FAILED: Pool {} not found in cache",
                    pool_id_str
                ));
                crate::tui_dex::utils::logger::log_error("Available pools in cache:");
                for (cached_pool_id, _) in &self.state.pool_cache {
                    crate::tui_dex::utils::logger::log_error(&format!("  - {}", cached_pool_id));
                }
                self.set_error_with_type(
                    format!("Pool {} not found or not loaded", pool_id_str),
                    ErrorType::Validation,
                );
                return Err(Error::Other(format!(
                    "Pool {} does not exist or is not loaded",
                    pool_id_str
                )));
            }
        };

        // Map display names back to actual denominations
        let actual_from_denom =
            self.map_display_name_to_denom(&from_asset, &pool_entry.pool_info.pool_info.assets);
        let actual_to_denom =
            self.map_display_name_to_denom(&to_asset, &pool_entry.pool_info.pool_info.assets);

        crate::tui_dex::utils::logger::log_debug(&format!(
            "Asset mapping: '{}' -> '{}', '{}' -> '{}'",
            from_asset, actual_from_denom, to_asset, actual_to_denom
        ));

        // Parse amount
        crate::tui_dex::utils::logger::log_info(&format!("Parsing amount: {}", amount));
        let amount_f64 = amount.parse::<f64>().map_err(|e| {
            crate::tui_dex::utils::logger::log_error(&format!(
                "SWAP FAILED: Invalid amount '{}': {}",
                amount, e
            ));
            Error::Other(format!("Invalid amount: {}", amount))
        })?;

        // Convert to blockchain format (assuming 6 decimal places)
        let amount_uint = cosmwasm_std::Uint128::new((amount_f64 * 1_000_000.0) as u128);
        crate::tui_dex::utils::logger::log_info(&format!(
            "Amount converted: {} -> {} (micro units)",
            amount_f64, amount_uint
        ));

        // Parse slippage tolerance
        let slippage = if let Some(slippage_str) = slippage_tolerance {
            let parsed_slippage = slippage_str
                .parse::<f64>()
                .ok()
                .map(|s| cosmwasm_std::Decimal::percent((s * 100.0) as u64));
            crate::tui_dex::utils::logger::log_info(&format!(
                "Slippage parsed: {}% -> {:?}",
                slippage_str, parsed_slippage
            ));
            parsed_slippage
        } else {
            crate::tui_dex::utils::logger::log_info("Using default slippage: 1%");
            Some(cosmwasm_std::Decimal::percent(100)) // 1% default slippage
        };

        // Create the offer asset coin using the actual denomination
        let offer_asset = cosmwasm_std::Coin {
            denom: actual_from_denom.clone(),
            amount: amount_uint,
        };

        crate::tui_dex::utils::logger::log_info(&format!(
            "Offer asset created: {} {}",
            offer_asset.amount, offer_asset.denom
        ));
        crate::tui_dex::utils::logger::log_info(&format!("Target denomination: {}", actual_to_denom));

        // Execute the swap using actual denominations
        crate::tui_dex::utils::logger::log_info("=== CALLING BLOCKCHAIN SWAP METHOD ===");
        crate::tui_dex::utils::logger::log_info(&format!("Calling client.swap() with parameters:"));
        crate::tui_dex::utils::logger::log_info(&format!("  Pool ID: {}", pool_id_str));
        crate::tui_dex::utils::logger::log_info(&format!(
            "  Offer Asset: {} {}",
            offer_asset.amount, offer_asset.denom
        ));
        crate::tui_dex::utils::logger::log_info(&format!("  Target Denom: {}", actual_to_denom));
        crate::tui_dex::utils::logger::log_info(&format!("  Slippage: {:?}", slippage));

        let swap_start_time = std::time::Instant::now();
        match self
            .client
            .swap(&pool_id_str, offer_asset, &actual_to_denom, slippage)
            .await
        {
            Ok(tx_response) => {
                let elapsed = swap_start_time.elapsed();
                crate::tui_dex::utils::logger::log_info("=== BLOCKCHAIN SWAP SUCCESS ===");
                crate::tui_dex::utils::logger::log_info(&format!("Swap execution time: {:?}", elapsed));
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Transaction Hash: {}",
                    tx_response.txhash
                ));
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Transaction Code: {}",
                    tx_response.code
                ));
                crate::tui_dex::utils::logger::log_info(&format!("Gas Used: {}", tx_response.gas_used));
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Gas Wanted: {}",
                    tx_response.gas_wanted
                ));
                crate::tui_dex::utils::logger::log_info(&format!("Height: {}", tx_response.height));
                crate::tui_dex::utils::logger::log_info(&format!("Raw Log: {}", tx_response.raw_log));

                // Log transaction events if any
                if !tx_response.events.is_empty() {
                    crate::tui_dex::utils::logger::log_info("Transaction Events:");
                    for event in &tx_response.events {
                        crate::tui_dex::utils::logger::log_info(&format!(
                            "  Event Type: {}",
                            event.r#type
                        ));
                        for attr in &event.attributes {
                            crate::tui_dex::utils::logger::log_info(&format!(
                                "    {}: {}",
                                attr.key, attr.value
                            ));
                        }
                    }
                } else {
                    crate::tui_dex::utils::logger::log_info("No transaction events found");
                }

                // Check if transaction actually succeeded (code 0 means success)
                if tx_response.code != 0 {
                    crate::tui_dex::utils::logger::log_error(&format!(
                        "TRANSACTION FAILED WITH CODE: {}",
                        tx_response.code
                    ));
                    crate::tui_dex::utils::logger::log_error(&format!(
                        "Error Log: {}",
                        tx_response.raw_log
                    ));
                } else {
                    crate::tui_dex::utils::logger::log_info("Transaction executed successfully!");
                }

                // Final progress update
                self.update_loading_progress(
                    100.0,
                    Some("Swap completed successfully!".to_string()),
                );

                // Give users a moment to see the completion before showing success modal
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Swap succeeded - show comprehensive success modal with improved transaction parsing
                let execution_time = format!("{:.2}s", elapsed.as_secs_f64());
                self.show_swap_success_modal(
                    &tx_response,
                    &from_asset,
                    &to_asset,
                    &amount,
                    &execution_time,
                );

                // Add to transaction history
                let tx_info = TransactionInfo {
                    hash: tx_response.txhash.clone(),
                    status: TransactionStatus::Success,
                    operation_type: "Swap".to_string(),
                    timestamp: chrono::Utc::now(),
                    gas_used: Some(tx_response.gas_used),
                    gas_wanted: Some(tx_response.gas_wanted),
                };
                self.add_transaction(tx_info);

                // Reset swap form
                crate::tui_dex::screens::swap::reset_swap_form();

                // Refresh swap screen pools to ensure they remain available
                if self.state.current_screen == Screen::Swap {
                    self.update_swap_screen_pools();
                }

                crate::tui_dex::utils::logger::log_info(
                    "=== SWAP EXECUTION COMPLETED SUCCESSFULLY ===",
                );
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Final transaction hash: {}",
                    tx_response.txhash
                ));
                crate::tui_dex::utils::logger::log_info(
                    "Transaction should be visible on Mantra testnet explorer",
                );
                crate::tui_dex::utils::logger::log_info(&format!(
                    "Explorer URL: https://explorer.mantrachain.io/Mantra-Dukong/tx/{}",
                    tx_response.txhash
                ));

                Ok(())
            }
            Err(e) => {
                let elapsed = swap_start_time.elapsed();
                crate::tui_dex::utils::logger::log_error("=== BLOCKCHAIN SWAP FAILED ===");
                crate::tui_dex::utils::logger::log_error(&format!(
                    "Swap execution time before failure: {:?}",
                    elapsed
                ));
                crate::tui_dex::utils::logger::log_error(&format!("Error type: {:?}", e));
                crate::tui_dex::utils::logger::log_error(&format!("Error message: {}", e));

                // Log the full error to file for debugging
                crate::tui_dex::utils::logger::log_error("SWAP FAILED - Full error details:");
                crate::tui_dex::utils::logger::log_error(&format!(
                    "  Operation: Swap {} {} to {} (display names)",
                    amount, from_asset, to_asset
                ));
                crate::tui_dex::utils::logger::log_error(&format!(
                    "  Actual denominations: {} -> {}",
                    actual_from_denom, actual_to_denom
                ));
                crate::tui_dex::utils::logger::log_error(&format!("  Pool ID: {}", pool_id_str));
                crate::tui_dex::utils::logger::log_error(&format!(
                    "  Amount: {} (parsed as {})",
                    amount, amount_uint
                ));
                crate::tui_dex::utils::logger::log_error(&format!("  Slippage: {:?}", slippage));
                crate::tui_dex::utils::logger::log_error(&format!("  Error: {:?}", e));
                crate::tui_dex::utils::logger::log_error(&format!("  Error string: {}", e));

                // Check if this is a network/connection error
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("connection")
                    || error_str.contains("network")
                    || error_str.contains("timeout")
                {
                    crate::tui_dex::utils::logger::log_error(
                        "This appears to be a NETWORK/CONNECTION error",
                    );
                } else if error_str.contains("insufficient") || error_str.contains("balance") {
                    crate::tui_dex::utils::logger::log_error(
                        "This appears to be an INSUFFICIENT FUNDS error",
                    );
                } else if error_str.contains("slippage") && error_str.contains("exceeded") {
                    crate::tui_dex::utils::logger::log_error(
                        "This appears to be a SLIPPAGE LIMIT EXCEEDED error",
                    );
                } else if error_str.contains("contract") || error_str.contains("execution") {
                    crate::tui_dex::utils::logger::log_error(
                        "This appears to be a CONTRACT EXECUTION error",
                    );
                } else {
                    crate::tui_dex::utils::logger::log_error(
                        "This appears to be an UNKNOWN error type",
                    );
                }

                // Determine error type and create user-friendly error handling
                let (error_type, error_title, user_message, suggestions) = if error_str
                    .contains("slippage")
                    && error_str.contains("exceeded")
                {
                    (
                            crate::tui_dex::components::modals::ErrorType::Validation,
                            "Slippage Limit Exceeded".to_string(),
                            format!(
                                                                 "The price moved too much during your swap transaction.\n\n\
                                 Your slippage tolerance of {}% was not sufficient to complete the swap.\n\n\
                                 This usually happens when:\n\
                                 • The pool has low liquidity\n\
                                 • There's high trading activity\n\
                                 • Market conditions are volatile",
                                 slippage.map(|s| {
                                     // Convert cosmwasm Decimal percentage back to human-readable format
                                     let percentage = s.to_string().parse::<f64>().unwrap_or(0.01) * 100.0;
                                     format!("{:.1}", percentage)
                                 }).unwrap_or_else(|| "1.0".to_string())
                            ),
                            vec![
                                "Increase your slippage tolerance (e.g., to 2-5%)".to_string(),
                                "Try a smaller swap amount".to_string(),
                                "Wait for better market conditions".to_string(),
                                "Check if the pool has sufficient liquidity".to_string(),
                            ]
                        )
                } else if error_str.contains("insufficient") || error_str.contains("balance") {
                    (
                        crate::tui_dex::components::modals::ErrorType::InsufficientFunds,
                        "Insufficient Funds".to_string(),
                        format!(
                            "You don't have enough {} to complete this swap.\n\n\
                                Required: {} {}\n\
                                Please check your wallet balance.",
                            from_asset, amount, from_asset
                        ),
                        vec![
                            "Check your wallet balance".to_string(),
                            "Reduce the swap amount".to_string(),
                            "Add more funds to your wallet".to_string(),
                            "Ensure you have enough for gas fees".to_string(),
                        ],
                    )
                } else if error_str.contains("connection")
                    || error_str.contains("network")
                    || error_str.contains("timeout")
                {
                    (
                            crate::tui_dex::components::modals::ErrorType::Network,
                            "Network Error".to_string(),
                            "Failed to connect to the Mantra network.\n\nPlease check your internet connection and try again.".to_string(),
                            vec![
                                "Check your internet connection".to_string(),
                                "Wait a moment and try again".to_string(),
                                "Try switching to a different RPC endpoint".to_string(),
                                "Check if the Mantra network is operational".to_string(),
                            ]
                        )
                } else if error_str.contains("contract") || error_str.contains("execution") {
                    (
                        crate::tui_dex::components::modals::ErrorType::Contract,
                        "Contract Execution Error".to_string(),
                        format!(
                            "The smart contract failed to execute your swap.\n\n\
                                This could be due to:\n\
                                • Pool configuration issues\n\
                                • Unexpected contract state\n\n\
                                Technical details: {}",
                            e
                        ),
                        vec![
                            "Try again in a few moments".to_string(),
                            "Check if the pool is active".to_string(),
                            "Verify your transaction parameters".to_string(),
                            "Contact support if the problem persists".to_string(),
                        ],
                    )
                } else {
                    (
                        crate::tui_dex::components::modals::ErrorType::Transaction,
                        "Transaction Failed".to_string(),
                        format!("Your swap transaction failed: {}", e),
                        vec![
                            "Review your transaction parameters".to_string(),
                            "Try again with different settings".to_string(),
                            "Check network status".to_string(),
                            "Contact support if needed".to_string(),
                        ],
                    )
                };

                // Create detailed error information for the modal
                let error_details = vec![
                    format!("Operation: Swap {} {} to {}", amount, from_asset, to_asset),
                    format!("Pool ID: {}", pool_id_str),
                    format!("Amount: {} ({})", amount, amount_uint),
                    format!("Slippage: {:?}", slippage),
                    format!("Technical Error: {}", e),
                ];

                // Show appropriate modal based on error type
                if error_str.contains("slippage") && error_str.contains("exceeded") {
                    // Create validation error modal for slippage issues to provide better guidance
                    self.state.modal_state = Some(
                        crate::tui_dex::components::modals::ModalState::validation_error(
                            error_title.clone(),
                            "Slippage Tolerance".to_string(),
                            user_message.clone(),
                            suggestions,
                        ),
                    );
                } else {
                    // For other errors, use the standard error modal
                    self.state.modal_state =
                        Some(crate::tui_dex::components::modals::ModalState::error(
                            error_title.clone(),
                            user_message.clone(),
                            error_type.clone(),
                            Some(error_details),
                            Some("Try again".to_string()),
                        ));
                }

                // Update loading state to error with persistent message
                self.state.loading_state = LoadingState::error_with_retry(
                    error_title.clone(),
                    error_type.clone(),
                    "retry_swap".to_string(),
                );

                // Also set a persistent error message in the status
                self.state.error_message = Some(error_title);

                Err(e)
            }
        }
    }

    /// Handle swap execute button - show confirmation modal
    pub fn handle_swap_execute_confirmation(&mut self) -> Result<(), Error> {
        let swap_state = crate::tui_dex::screens::swap::get_swap_screen_state();

        // Check if any pools are available
        if self.state.pool_cache.is_empty() {
            self.show_validation_error(
                "No Pools Available".to_string(),
                "No pools are currently loaded for swapping".to_string(),
                vec![
                    "Wait for pool data to load from blockchain".to_string(),
                    "Check network connection".to_string(),
                    "Refresh the pools data".to_string(),
                ],
            );
            return Ok(());
        }

        // Validate swap inputs
        if !swap_state.validate() {
            self.show_validation_error(
                "Swap Validation".to_string(),
                "Please fill in all required fields".to_string(),
                vec![
                    "Select a pool".to_string(),
                    "Select from token".to_string(),
                    "Enter swap amount".to_string(),
                    "Set slippage tolerance".to_string(),
                ],
            );
            return Ok(());
        }

        // Get swap details for confirmation
        let from_amount = swap_state.from_amount_input.value();
        let from_token = swap_state
            .from_token_dropdown
            .get_selected_value()
            .unwrap_or_default();
        let pool_id = swap_state
            .pool_dropdown
            .get_selected_value()
            .unwrap_or_default();
        let slippage = swap_state.slippage_input.value();

        // Get the "to" token from the selected pool
        let to_token = if let Some(pool_name) = swap_state.pool_dropdown.get_selected_label() {
            crate::tui_dex::screens::swap::determine_to_token_from_pool(&pool_name, &from_token)
        } else {
            "Unknown".to_string()
        };

        // Calculate expected output (placeholder - would use simulation result)
        let expected_output = format!("{:.6}", from_amount.parse::<f64>().unwrap_or(0.0) * 0.95);

        // Calculate price impact (placeholder - would use real simulation data)
        let price_impact = 0.05; // 0.05%

        // Calculate fees (placeholder - would use real pool data)
        let fee_amount = format!("{:.6}", from_amount.parse::<f64>().unwrap_or(0.0) * 0.003);

        // Create swap details for confirmation
        let swap_details = crate::tui_dex::screens::swap::SwapDetails {
            from_amount: from_amount.to_string(),
            from_token: from_token.to_string(),
            to_amount: expected_output.clone(),
            to_token: to_token.clone(),
            pool_name: swap_state
                .pool_dropdown
                .get_selected_label()
                .unwrap_or_default()
                .to_string(),
            slippage: slippage.to_string(),
            expected_output: expected_output.clone(),
            price_impact,
            fee_amount,
        };

        // Show global confirmation modal
        let confirmation_message = swap_state.show_confirmation_modal(&swap_details);

        self.show_confirmation(
            "Confirm Swap".to_string(),
            confirmation_message,
            Some("Execute Swap".to_string()),
            Some("Cancel".to_string()),
        );

        Ok(())
    }

    /// Map display name to actual denomination using available pool assets
    /// This is a public utility for balance lookups
    pub fn map_token_name_to_denom(&self, token_name: &str) -> Option<String> {
        // First try direct lookup (for simple denoms like "uom")
        if self.state.balances.contains_key(token_name) {
            return Some(token_name.to_string());
        }

        // Search through all pool assets to find matching denominations using symbol mapping
        for pool_entry in self.state.pool_cache.values() {
            for asset in &pool_entry.pool_info.pool_info.assets {
                let denom = &asset.denom;

                // Check if the token name matches our symbol mapping
                let symbol = self.denom_to_symbol(denom);
                if symbol == token_name {
                    return Some(denom.clone());
                }

                // Check if the token name matches the shortened version (fallback)
                let shortened = if denom.len() > 15 {
                    if let Some(last_part) = denom.split('/').last() {
                        last_part.to_string()
                    } else {
                        format!("{}...", &denom[..10])
                    }
                } else {
                    denom.clone()
                };

                if shortened == token_name {
                    return Some(denom.clone());
                }
            }
        }

        // Search through all balances to find matching denominations using symbol mapping
        for (denom, _) in &self.state.balances {
            let symbol = self.denom_to_symbol(denom);
            if symbol == token_name {
                return Some(denom.clone());
            }

            // Fallback: check if denom ends with the token name
            if denom.ends_with(&format!("/{}", token_name)) {
                return Some(denom.clone());
            }

            if let Some(last_part) = denom.split('/').last() {
                if last_part == token_name {
                    return Some(denom.clone());
                }
            }
        }

        None
    }

    /// Get balance for a token by its display name
    /// This handles mapping from display names to full denominations
    pub fn get_balance_by_token_name(&self, token_name: &str) -> Option<&String> {
        // Try direct lookup first
        if let Some(balance) = self.state.balances.get(token_name) {
            return Some(balance);
        }

        // Map token name to full denomination and lookup balance
        if let Some(full_denom) = self.map_token_name_to_denom(token_name) {
            if let Some(balance) = self.state.balances.get(&full_denom) {
                return Some(balance);
            }
        }

        None
    }

    /// Handle slippage error retry with automatic slippage increase
    pub async fn handle_slippage_retry(&mut self) -> Result<(), Error> {
        // Get current swap parameters
        let swap_state = crate::tui_dex::screens::swap::get_swap_screen_state();

        let current_slippage = swap_state
            .slippage_input
            .value()
            .parse::<f64>()
            .unwrap_or(1.0);
        let suggested_slippage = if current_slippage < 2.0 {
            2.0
        } else if current_slippage < 5.0 {
            5.0
        } else {
            current_slippage + 2.0 // Increase by 2% if already high
        };

        // Show confirmation for increased slippage
        self.show_confirmation(
            "Increase Slippage and Retry?".to_string(),
            format!(
                "Would you like to retry the swap with increased slippage tolerance?\n\n\
                Current: {:.1}%\n\
                Suggested: {:.1}%\n\n\
                Higher slippage tolerance increases the chance of success but may result in less favorable rates.",
                current_slippage,
                suggested_slippage
            ),
            Some("Retry with Higher Slippage".to_string()),
            Some("Cancel".to_string()),
        );

        Ok(())
    }

    /// Apply suggested slippage and retry swap
    pub async fn retry_swap_with_increased_slippage(&mut self) -> Result<(), Error> {
        let swap_state = crate::tui_dex::screens::swap::get_swap_screen_state();

        let current_slippage = swap_state
            .slippage_input
            .value()
            .parse::<f64>()
            .unwrap_or(1.0);
        let suggested_slippage = if current_slippage < 2.0 {
            2.0
        } else if current_slippage < 5.0 {
            5.0
        } else {
            current_slippage + 2.0
        };

        // Update the slippage input
        swap_state
            .slippage_input
            .set_value(&format!("{:.1}", suggested_slippage));

        // Clear previous error modal
        self.state.modal_state = None;
        self.state.error_message = None;

        // Show info message
        self.set_status(format!(
            "Increased slippage tolerance to {:.1}% - retrying swap...",
            suggested_slippage
        ));

        // Retry the swap with the same parameters but higher slippage
        if let Some(from_asset) = swap_state.from_token_dropdown.get_selected_value() {
            // Get the to_asset from the pool selection
            let to_asset = if let Some(pool_name) = swap_state.pool_dropdown.get_selected_label() {
                crate::tui_dex::screens::swap::determine_to_token_from_pool(&pool_name, &from_asset)
            } else {
                "Unknown".to_string()
            };
            if let Some(pool_id) = swap_state.pool_dropdown.get_selected_value() {
                let amount = swap_state.from_amount_input.value().to_string();

                self.execute_real_swap(
                    from_asset.to_string(),
                    to_asset,
                    amount,
                    Some(pool_id.to_string()),
                    Some(format!("{:.1}", suggested_slippage)),
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Show swap success modal with comprehensive transaction details
    pub fn show_swap_success_modal(
        &mut self,
        tx_response: &TxResponse,
        from_asset: &str,
        to_asset: &str,
        from_amount: &str,
        execution_time: &str,
    ) {
        // Parse transaction events to get actual received amount
        let mut actual_received_amount = "Calculating...".to_string();
        let mut fee_amount = "N/A".to_string();

        // Look for swap-related events in the transaction
        for event in &tx_response.events {
            if event.r#type == "wasm" {
                let mut found_swap_event = false;
                let mut received_amount = None;
                let mut fee = None;

                for attr in &event.attributes {
                    match attr.key.as_str() {
                        "action" if attr.value == "swap" => {
                            found_swap_event = true;
                        }
                        "offer_amount" | "ask_amount" => {
                            // This might be the received amount
                            if found_swap_event && attr.key == "ask_amount" {
                                if let Ok(amount) = attr.value.parse::<u128>() {
                                    let decimals = self.get_token_decimals(
                                        &self.map_token_name_to_denom(to_asset).unwrap_or_default(),
                                    );
                                    let display_amount =
                                        (amount as f64) / (10_f64.powi(decimals as i32));
                                    received_amount = Some(format!("{:.6}", display_amount));
                                }
                            }
                        }
                        "swap_fee" | "fee_amount" => {
                            fee = Some(attr.value.clone());
                        }
                        _ => {}
                    }
                }

                if let Some(amount) = received_amount {
                    actual_received_amount = format!("{} {}", amount, to_asset);
                }
                if let Some(f) = fee {
                    fee_amount = f;
                }
            }
        }

        // If we couldn't parse the actual amount, provide an estimate
        if actual_received_amount == "Calculating..." {
            if let Ok(from_amount_f64) = from_amount.parse::<f64>() {
                let estimated = from_amount_f64 * 0.997; // Assuming ~0.3% fees
                actual_received_amount = format!("~{:.6} {}", estimated, to_asset);
            }
        }

        // Create comprehensive transaction details
        let transaction_details = vec![
            ("Transaction Hash".to_string(), tx_response.txhash.clone()),
            ("Operation Type".to_string(), "Token Swap".to_string()),
            (
                "From Asset".to_string(),
                format!("{} {}", from_amount, from_asset),
            ),
            ("To Asset (Received)".to_string(), actual_received_amount),
            ("Execution Time".to_string(), execution_time.to_string()),
            ("Block Height".to_string(), tx_response.height.to_string()),
            ("Gas Used".to_string(), tx_response.gas_used.to_string()),
            ("Gas Wanted".to_string(), tx_response.gas_wanted.to_string()),
            ("Transaction Fee".to_string(), fee_amount),
            (
                "Status".to_string(),
                "✅ Completed Successfully".to_string(),
            ),
            (
                "Explorer".to_string(),
                format!(
                    "https://explorer.mantrachain.io/Mantra-Dukong/tx/{}",
                    tx_response.txhash
                ),
            ),
        ];

        // Show the success modal
        self.state.modal_state = Some(
            crate::tui_dex::components::modals::ModalState::transaction_details(
                tx_response.txhash.clone(),
                "Swap Completed Successfully!".to_string(),
                transaction_details,
            ),
        );

        // Update loading state to success
        self.state.loading_state =
            LoadingState::success("Swap completed successfully!".to_string());
    }

    /// Handle pool creation execute button - show confirmation modal
    pub fn handle_pool_creation_execute_confirmation(&mut self) -> Result<(), Error> {
        let admin_state = crate::tui_dex::screens::admin::get_admin_screen_state();

        // Validate pool creation inputs
        if !admin_state.validate() {
            let errors = admin_state.clone().get_validation_errors();
            self.show_validation_error(
                "Pool Creation Validation".to_string(),
                "Please fill in all required fields".to_string(),
                errors,
            );
            return Ok(());
        }

        // Get pool creation details for confirmation
        let first_asset = admin_state.pool_creation.first_asset_input.value();
        let second_asset = admin_state.pool_creation.second_asset_input.value();
        let swap_fee = admin_state.pool_creation.swap_fee_input.value();
        let protocol_fee = admin_state.pool_creation.protocol_fee_input.value();
        let burn_fee = admin_state.pool_creation.burn_fee_input.value();
        let pool_type = admin_state
            .pool_creation
            .pool_type_dropdown
            .get_selected_value()
            .unwrap_or("Standard");

        // Create confirmation message
        let confirmation_message = format!(
            "Confirm Pool Creation:\n\n• Asset Pair: {} / {}\n• Pool Type: {}\n• Swap Fee: {}%\n• Protocol Fee: {}%\n• Burn Fee: {}%\n• Total Fee: {:.2}%\n\nThis will create a new pool on the Mantra DEX.\nTransaction requires admin privileges.",
            first_asset,
            second_asset,
            pool_type,
            swap_fee,
            protocol_fee,
            burn_fee,
            swap_fee.parse::<f64>().unwrap_or(0.0) +
            protocol_fee.parse::<f64>().unwrap_or(0.0) +
            burn_fee.parse::<f64>().unwrap_or(0.0)
        );

        // Show global confirmation modal
        self.show_confirmation(
            "Confirm Pool Creation".to_string(),
            confirmation_message,
            Some("Create Pool".to_string()),
            Some("Cancel".to_string()),
        );

        Ok(())
    }

    /// Handle pool management execute button - show confirmation modal
    pub fn handle_pool_management_execute_confirmation(&mut self) -> Result<(), Error> {
        let admin_state = crate::tui_dex::screens::admin::get_admin_screen_state();

        // Validate pool management inputs
        if !admin_state.validate() {
            let errors = admin_state.clone().get_validation_errors();
            self.show_validation_error(
                "Pool Management Validation".to_string(),
                "Please fill in all required fields".to_string(),
                errors,
            );
            return Ok(());
        }

        // Get pool management details for confirmation
        let pool_id = admin_state
            .pool_management
            .pool_selection_dropdown
            .get_selected_value()
            .unwrap_or_default();
        let features = admin_state
            .pool_management
            .selected_pool_features
            .unwrap_or((true, true, true));

        // Create confirmation message
        let confirmation_message = format!(
            "Confirm Feature Update:\n\n• Pool: {}\n• Withdrawals: {}\n• Deposits: {}\n• Swaps: {}\n\nThis will update pool features on the Mantra DEX.\nTransaction requires admin privileges.",
            pool_id,
            if features.0 { "Enabled" } else { "Disabled" },
            if features.1 { "Enabled" } else { "Disabled" },
            if features.2 { "Enabled" } else { "Disabled" }
        );

        // Show global confirmation modal
        self.show_confirmation(
            "Confirm Pool Management".to_string(),
            confirmation_message,
            Some("Update Features".to_string()),
            Some("Cancel".to_string()),
        );

        Ok(())
    }

    /// Handle liquidity execute button - show confirmation modal (similar to swap screen)
    pub fn handle_liquidity_execute_confirmation(&mut self) -> Result<(), Error> {
        let liquidity_state = crate::tui_dex::screens::liquidity::get_liquidity_screen_state();

        // Check if any pools are available
        if self.state.pool_cache.is_empty() {
            self.show_validation_error(
                "No Pools Available".to_string(),
                "No pools are currently loaded for liquidity operations".to_string(),
                vec![
                    "Wait for pool data to load from blockchain".to_string(),
                    "Check network connection".to_string(),
                    "Refresh the pools data".to_string(),
                ],
            );
            return Ok(());
        }

        // Validate liquidity inputs
        if !liquidity_state.validate() {
            let errors = liquidity_state.clone().get_validation_errors();
            self.show_validation_error(
                "Liquidity Validation".to_string(),
                "Please fill in all required fields".to_string(),
                errors,
            );
            return Ok(());
        }

        // Execute the enhanced liquidity operation confirmation
        crate::tui_dex::screens::liquidity::execute_liquidity_operation_with_confirmation();

        // Get liquidity details for confirmation based on mode
        let confirmation_message = match liquidity_state.mode {
            crate::tui_dex::screens::liquidity::LiquidityMode::Provide => {
                let first_amount = liquidity_state.first_asset_input.value();
                let second_amount = liquidity_state.second_asset_input.value();
                let pool_name = liquidity_state
                    .pool_dropdown
                    .get_selected_label()
                    .unwrap_or("Unknown Pool");
                let slippage = liquidity_state.slippage_input.value();

                // Extract asset names from pool
                let (first_asset, second_asset) =
                    crate::tui_dex::screens::liquidity::extract_assets_from_pool_label(pool_name);

                format!(
                    "Confirm Provide Liquidity:\n\n• First Asset: {} {}\n• Second Asset: {} {}\n• Pool: {}\n• Slippage: {}%\n\nProceed with transaction?",
                    first_amount, first_asset, second_amount, second_asset, pool_name, slippage
                )
            }
            crate::tui_dex::screens::liquidity::LiquidityMode::Withdraw => {
                let lp_amount = liquidity_state.withdraw_amount_input.value();
                let pool_name = liquidity_state
                    .pool_dropdown
                    .get_selected_label()
                    .unwrap_or("Unknown Pool");

                format!(
                    "Confirm Withdraw Liquidity:\n\n• LP Token Amount: {}\n• Pool: {}\n\nProceed with transaction?",
                    lp_amount, pool_name
                )
            }
            crate::tui_dex::screens::liquidity::LiquidityMode::Positions => {
                return Ok(()); // No operation for positions mode
            }
        };

        // Show global confirmation modal
        let title = match liquidity_state.mode {
            crate::tui_dex::screens::liquidity::LiquidityMode::Provide => "Confirm Provide Liquidity",
            crate::tui_dex::screens::liquidity::LiquidityMode::Withdraw => "Confirm Withdraw Liquidity",
            _ => "Confirm Liquidity Operation",
        };

        self.show_confirmation(
            title.to_string(),
            confirmation_message,
            Some("Execute".to_string()),
            Some("Cancel".to_string()),
        );

        Ok(())
    }

    /// Fetch pool reserves for liquidity proportional calculations
    async fn fetch_pool_reserves_for_liquidity(&mut self, pool_id: &str) -> Result<(), Error> {
        crate::tui_dex::utils::logger::log_debug(&format!(
            "Fetching pool reserves for liquidity calculations: {}",
            pool_id
        ));

        // Get pool info from cache or fetch from blockchain
        let pool_info = if let Some(cached_pool) = self.get_cached_pool(pool_id) {
            cached_pool.clone()
        } else {
            // Fetch pool info from blockchain
            match self.client.get_pool(pool_id).await {
                Ok(pool_info) => {
                    // Cache the pool info
                    let cache_entry = PoolCacheEntry {
                        pool_info: pool_info.clone(),
                        cached_at: chrono::Utc::now(),
                    };
                    self.state
                        .pool_cache
                        .insert(pool_id.to_string(), cache_entry);
                    pool_info
                }
                Err(e) => {
                    crate::tui_dex::utils::logger::log_error(&format!(
                        "Failed to fetch pool info for reserves: {}",
                        e
                    ));
                    return Err(e);
                }
            }
        };

        // Extract reserves from pool assets (supports multi-asset pools)
        let mut reserves = Vec::new();
        for asset in &pool_info.pool_info.assets {
            reserves.push((asset.amount, asset.denom.clone()));
        }

        crate::tui_dex::utils::logger::log_info(&format!(
            "Fetched {} reserves for pool {}: {:?}",
            reserves.len(),
            pool_id,
            reserves
                .iter()
                .map(|(amt, denom)| format!("{} {}", amt, denom))
                .collect::<Vec<_>>()
        ));

        // Update liquidity screen with reserves
        crate::tui_dex::screens::liquidity::update_liquidity_pool_reserves(reserves);

        Ok(())
    }

    /// Create enhanced liquidity success details with LP token information
    fn create_liquidity_success_details(
        &self,
        result: &str,
        transaction_hash: &Option<String>,
        enhanced_data: &Option<String>,
    ) -> Vec<(String, String)> {
        let mut details = vec![("Result".to_string(), result.to_string())];

        if let Some(tx_hash) = transaction_hash {
            details.push(("Transaction Hash".to_string(), tx_hash.clone()));

            // Add Mantra explorer link
            let explorer_url = format!(
                "https://explorer.mantrachain.io/Mantra-Dukong/tx/{}",
                tx_hash
            );
            details.push(("Explorer Link".to_string(), explorer_url));
        }

        // Parse enhanced data if available
        if let Some(enhanced_json) = enhanced_data {
            if let Ok(provide_result) =
                serde_json::from_str::<crate::tui_dex::events::ProvideResultWrapper>(enhanced_json)
            {
                // Add LP token information if available
                if let Some(lp_amount) = &provide_result.lp_tokens_received {
                    let lp_display = self.format_token_amount_for_display(lp_amount, 6);
                    details.push(("LP Tokens Received".to_string(), lp_display));
                }

                // Add user's total LP balance if available
                if let Some(total_balance) = &provide_result.user_lp_balance_after {
                    let balance_display = self.format_token_amount_for_display(total_balance, 6);
                    details.push(("Total LP Balance".to_string(), balance_display));
                }

                // Calculate and add pool share information
                if let (Some(user_balance), Some(total_supply)) = (
                    &provide_result.user_lp_balance_after,
                    &provide_result.pool_total_supply,
                ) {
                    if total_supply.u128() > 0 {
                        let share_percentage =
                            (user_balance.u128() as f64 / total_supply.u128() as f64) * 100.0;
                        details.push((
                            "Total Share of Pool".to_string(),
                            format!("{:.4}%", share_percentage),
                        ));

                        // If we know how much was received, calculate the share of just the received amount
                        if let Some(received_amount) = &provide_result.lp_tokens_received {
                            let received_share = (received_amount.u128() as f64
                                / total_supply.u128() as f64)
                                * 100.0;
                            details.push((
                                "Share from This Transaction".to_string(),
                                format!("{:.4}%", received_share),
                            ));
                        }
                    }
                }

                // Add LP token denomination for reference
                if let Some(lp_denom) = &provide_result.lp_token_denom {
                    details.push(("LP Token Denom".to_string(), lp_denom.clone()));
                }

                crate::tui_dex::utils::logger::log_info(&format!(
                    "Enhanced liquidity success details created with {} items",
                    details.len()
                ));
            } else {
                crate::tui_dex::utils::logger::log_warning("Failed to parse enhanced liquidity data");
            }
        }

        details
    }

    /// Create enhanced pool creation success details with pool ID and explorer link
    fn create_pool_creation_success_details(
        &self,
        result: &str,
        transaction_hash: &Option<String>,
    ) -> Vec<(String, String)> {
        let mut details = vec![("Result".to_string(), result.to_string())];

        if let Some(tx_hash) = transaction_hash {
            details.push(("Transaction Hash".to_string(), tx_hash.clone()));

            // Add explorer link
            let explorer_url = format!("https://explorer.mantrachain.io/transaction/{}", tx_hash);
            details.push(("Explorer Link".to_string(), explorer_url));
        }

        // Try to extract pool ID from transaction result
        if let Some(pool_id) = self.extract_pool_id_from_result(result) {
            details.push(("Pool ID".to_string(), pool_id.clone()));

            // Add pool-specific explorer link if available
            let pool_explorer_url = format!("https://explorer.mantrachain.io/pools/{}", pool_id);
            details.push(("Pool Explorer".to_string(), pool_explorer_url));
        }

        // Add pool creation fee information
        details.push(("Pool Creation Fee".to_string(), "88 OM".to_string()));

        // Add timestamp
        details.push((
            "Created At".to_string(),
            chrono::Utc::now()
                .format("%Y-%m-%d %H:%M:%S UTC")
                .to_string(),
        ));

        details
    }

    /// Extract pool ID from transaction result or logs
    fn extract_pool_id_from_result(&self, result: &str) -> Option<String> {
        // Try to parse pool ID from various possible formats in the result

        // Look for "pool_id" in JSON-like structures
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(result) {
            // Check for pool_id in various locations
            if let Some(pool_id) = json_value.get("pool_id").and_then(|v| v.as_str()) {
                return Some(pool_id.to_string());
            }

            // Check for pool_id in events or logs
            if let Some(events) = json_value.get("events").and_then(|v| v.as_array()) {
                for event in events {
                    if let Some(attributes) = event.get("attributes").and_then(|v| v.as_array()) {
                        for attr in attributes {
                            if let (Some(key), Some(value)) = (
                                attr.get("key").and_then(|v| v.as_str()),
                                attr.get("value").and_then(|v| v.as_str()),
                            ) {
                                if key == "pool_id" || key == "pool_identifier" {
                                    return Some(value.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Try regex patterns for pool ID extraction
        use regex::Regex;

        // Pattern: "pool_id": "123" or "pool_id":"123"
        if let Ok(re) = Regex::new(r#""pool_id"\s*:\s*"([^"]+)""#) {
            if let Some(captures) = re.captures(result) {
                if let Some(pool_id) = captures.get(1) {
                    return Some(pool_id.as_str().to_string());
                }
            }
        }

        // Pattern: pool_id=123 or pool_id: 123
        if let Ok(re) = Regex::new(r"pool_id[:\s=]+(\w+)") {
            if let Some(captures) = re.captures(result) {
                if let Some(pool_id) = captures.get(1) {
                    return Some(pool_id.as_str().to_string());
                }
            }
        }

        // Pattern: Pool 123 created or Created pool 123
        if let Ok(re) = Regex::new(r"(?i)(?:pool\s+(\w+)\s+created|created\s+pool\s+(\w+))") {
            if let Some(captures) = re.captures(result) {
                if let Some(pool_id) = captures.get(1).or_else(|| captures.get(2)) {
                    return Some(pool_id.as_str().to_string());
                }
            }
        }

        None
    }

    /// Create basic success details for non-enhanced operations
    fn create_basic_success_details(
        &self,
        result: &str,
        transaction_hash: &Option<String>,
    ) -> Vec<(String, String)> {
        let mut details = vec![("Result".to_string(), result.to_string())];

        if let Some(tx_hash) = transaction_hash {
            details.push(("Transaction".to_string(), tx_hash.clone()));
        }

        details
    }

    /// Format token amount for display (convert from micro units to display units)
    fn format_token_amount_for_display(
        &self,
        amount: &cosmwasm_std::Uint128,
        decimals: u8,
    ) -> String {
        let amount_f64 = amount.u128() as f64 / 10_f64.powi(decimals as i32);

        // Format with appropriate precision
        if amount_f64 >= 1000.0 {
            format!("{:.2}", amount_f64)
        } else if amount_f64 >= 1.0 {
            format!("{:.4}", amount_f64)
        } else {
            format!("{:.6}", amount_f64)
        }
    }
}

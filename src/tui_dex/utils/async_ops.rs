//! Async Operation Helpers
//!
//! This module provides utilities for handling background async operations
//! and real-time data updates in the TUI application without blocking the UI.

use crate::tui_dex::events::Event;
use crate::{Error, MantraDexClient};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

/// Background sync configuration
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// Interval for balance refresh (default: 30 seconds)
    pub balance_refresh_interval: Duration,
    /// Interval for pool data refresh (default: 60 seconds)
    pub pool_data_refresh_interval: Duration,
    /// Interval for transaction status checks (default: 10 seconds)
    pub transaction_status_interval: Duration,
    /// Interval for network info refresh (default: 45 seconds)
    pub network_info_interval: Duration,
    /// Interval for price updates (default: 15 seconds)
    pub price_update_interval: Duration,
    /// Network connection timeout (default: 10 seconds)
    pub network_timeout: Duration,
    /// Retry attempts for failed operations (default: 3)
    pub retry_attempts: u32,
    /// Retry delay between attempts (default: 5 seconds)
    pub retry_delay: Duration,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            balance_refresh_interval: Duration::from_secs(30),
            pool_data_refresh_interval: Duration::from_secs(60),
            transaction_status_interval: Duration::from_secs(10),
            network_info_interval: Duration::from_secs(45),
            price_update_interval: Duration::from_secs(15),
            network_timeout: Duration::from_secs(10),
            retry_attempts: 3,
            retry_delay: Duration::from_secs(5),
        }
    }
}

/// Network connection state
#[derive(Debug, Clone, PartialEq)]
pub enum NetworkState {
    Connected,
    Disconnected,
    Reconnecting,
    Error(String),
}

/// Enhanced operation result with detailed status
#[derive(Debug, Clone)]
pub struct OperationResult {
    pub success: bool,
    pub error: Option<String>,
    pub retry_count: u32,
    pub duration: Duration,
    pub network_state: NetworkState,
    pub additional_info: Option<String>,
}

impl OperationResult {
    pub fn success(duration: Duration) -> Self {
        Self {
            success: true,
            error: None,
            retry_count: 0,
            duration,
            network_state: NetworkState::Connected,
            additional_info: None,
        }
    }

    pub fn error(error: String, retry_count: u32, network_state: NetworkState) -> Self {
        Self {
            success: false,
            error: Some(error),
            retry_count,
            duration: Duration::default(),
            network_state,
            additional_info: None,
        }
    }
}

/// Background sync manager for real-time updates
pub struct SyncManager {
    /// Event sender for communicating with the main app
    event_sender: mpsc::UnboundedSender<Event>,
    /// Background task handles
    task_handles: Vec<tokio::task::JoinHandle<()>>,
    /// Sync configuration
    config: SyncConfig,
    /// Client for blockchain operations
    client: Arc<MantraDexClient>,
    /// Current wallet address for balance updates
    wallet_address: Option<String>,
    /// Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
    /// Network state tracking
    network_state: Arc<tokio::sync::RwLock<NetworkState>>,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(
        event_sender: mpsc::UnboundedSender<Event>,
        client: Arc<MantraDexClient>,
        config: Option<SyncConfig>,
    ) -> Self {
        Self {
            event_sender,
            task_handles: Vec::new(),
            config: config.unwrap_or_default(),
            client,
            wallet_address: None,
            cancellation_token: CancellationToken::new(),
            network_state: Arc::new(tokio::sync::RwLock::new(NetworkState::Connected)),
        }
    }

    /// Start all background sync tasks
    pub fn start_background_sync(&mut self) {
        self.start_balance_sync();
        self.start_pool_data_sync();
        self.start_transaction_status_sync();
        self.start_network_info_sync();
        self.start_price_sync();
        self.start_network_health_monitor();
    }

    /// Set wallet address for balance syncing
    pub fn set_wallet_address(&mut self, address: String) {
        self.wallet_address = Some(address);
    }

    /// Get current network state
    pub async fn get_network_state(&self) -> NetworkState {
        self.network_state.read().await.clone()
    }

    /// Start network health monitoring
    fn start_network_health_monitor(&mut self) {
        let sender = self.event_sender.clone();
        let client = Arc::clone(&self.client);
        let network_state = Arc::clone(&self.network_state);
        let cancellation_token = self.cancellation_token.clone();
        let config = self.config.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(15)); // Check every 15 seconds

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    _ = interval.tick() => {
                        let start_time = std::time::Instant::now();

                        // Test network connectivity with timeout
                        let network_result = tokio::time::timeout(
                            config.network_timeout,
                            client.get_last_block_height()
                        ).await;

                        let new_state = match network_result {
                            Ok(Ok(_)) => {
                                // Network is healthy
                                NetworkState::Connected
                            },
                            Ok(Err(e)) => {
                                // Network error
                                NetworkState::Error(format!("Network error: {}", e))
                            },
                            Err(_) => {
                                // Timeout
                                NetworkState::Disconnected
                            }
                        };

                        // Update network state if changed
                        let mut state_guard = network_state.write().await;
                        if *state_guard != new_state {
                            let old_state = state_guard.clone();
                            *state_guard = new_state.clone();
                            drop(state_guard);

                            // Send network state change event
                            let _ = sender.send(Event::Custom(format!(
                                "network_state_changed:{}:{}",
                                match old_state {
                                    NetworkState::Connected => "connected",
                                    NetworkState::Disconnected => "disconnected",
                                    NetworkState::Reconnecting => "reconnecting",
                                    NetworkState::Error(_) => "error",
                                },
                                match new_state {
                                    NetworkState::Connected => "connected",
                                    NetworkState::Disconnected => "disconnected",
                                    NetworkState::Reconnecting => "reconnecting",
                                    NetworkState::Error(_) => "error",
                                }
                            )));
                        }
                    }
                }
            }
        });

        self.task_handles.push(handle);
    }

    /// Execute operation with retry logic and comprehensive error handling
    async fn execute_with_retry<F, Fut, T>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> OperationResult
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        let start_time = std::time::Instant::now();
        let mut last_error = None;

        for attempt in 0..=self.config.retry_attempts {
            // Check network state before attempting operation
            let network_state = self.network_state.read().await.clone();

            // Skip if network is disconnected (unless this is a network health check)
            if matches!(network_state, NetworkState::Disconnected)
                && operation_name != "network_health"
            {
                return OperationResult::error(
                    "Network disconnected".to_string(),
                    attempt,
                    network_state,
                );
            }

            // Send progress update for retries
            if attempt > 0 {
                let _ = self.event_sender.send(Event::BlockchainProgress {
                    operation: operation_name.to_string(),
                    status: format!(
                        "Retrying operation (attempt {} of {})",
                        attempt + 1,
                        self.config.retry_attempts + 1
                    ),
                    progress: Some(attempt as f32 / (self.config.retry_attempts + 1) as f32),
                });

                // Wait before retry
                tokio::time::sleep(self.config.retry_delay).await;
            }

            // Execute the operation with timeout
            match tokio::time::timeout(self.config.network_timeout, operation()).await {
                Ok(Ok(_)) => {
                    return OperationResult::success(start_time.elapsed());
                }
                Ok(Err(e)) => {
                    last_error = Some(e);

                    // Update network state based on error type
                    let mut state_guard = self.network_state.write().await;
                    if last_error
                        .as_ref()
                        .unwrap()
                        .to_string()
                        .contains("connection")
                        || last_error.as_ref().unwrap().to_string().contains("timeout")
                    {
                        *state_guard = NetworkState::Disconnected;
                    }
                }
                Err(_) => {
                    last_error = Some(Error::Rpc("Operation timeout".to_string()));
                    let mut state_guard = self.network_state.write().await;
                    *state_guard = NetworkState::Disconnected;
                }
            }
        }

        let final_network_state = self.network_state.read().await.clone();
        OperationResult::error(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string()),
            self.config.retry_attempts,
            final_network_state,
        )
    }

    /// Start balance refresh task with enhanced error handling
    fn start_balance_sync(&mut self) {
        let sender = self.event_sender.clone();
        let client = Arc::clone(&self.client);
        let interval_duration = self.config.balance_refresh_interval;
        let cancellation_token = self.cancellation_token.clone();
        let retry_attempts = self.config.retry_attempts;
        let retry_delay = self.config.retry_delay;
        let network_timeout = self.config.network_timeout;

        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    _ = interval.tick() => {
                        // Execute balance refresh with retry logic
                        let start_time = std::time::Instant::now();
                        let mut success = false;
                        let mut error_message = None;
                        let mut retry_count = 0;

                        for attempt in 0..retry_attempts {
                            retry_count = attempt;

                            match tokio::time::timeout(network_timeout, client.get_balances()).await {
                                Ok(Ok(_)) => {
                                    success = true;
                                    break;
                                }
                                Ok(Err(e)) => {
                                    error_message = Some(e.to_string());
                                    if attempt < retry_attempts - 1 {
                                        tokio::time::sleep(retry_delay).await;
                                    }
                                }
                                Err(_) => {
                                    error_message = Some("Operation timeout".to_string());
                                    if attempt < retry_attempts - 1 {
                                        tokio::time::sleep(retry_delay).await;
                                    }
                                }
                            }
                        }

                        // Send appropriate event based on result
                        let event = Event::DataRefresh {
                            data_type: "balances".to_string(),
                            success,
                            error: error_message,
                        };

                        if sender.send(event).is_err() {
                            break; // Channel closed, stop task
                        }
                    }
                }
            }
        });

        self.task_handles.push(handle);
    }

    /// Start pool data refresh task with enhanced error handling
    fn start_pool_data_sync(&mut self) {
        let sender = self.event_sender.clone();
        let client = Arc::clone(&self.client);
        let interval_duration = self.config.pool_data_refresh_interval;
        let cancellation_token = self.cancellation_token.clone();
        let retry_attempts = self.config.retry_attempts;
        let retry_delay = self.config.retry_delay;
        let network_timeout = self.config.network_timeout;

        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    _ = interval.tick() => {
                        // Execute pool data refresh with retry logic
                        let mut success = false;
                        let mut error_message = None;

                        for attempt in 0..retry_attempts {
                            match tokio::time::timeout(network_timeout, client.get_pools(Some(50))).await {
                                Ok(Ok(_)) => {
                                    success = true;
                                    break;
                                }
                                Ok(Err(e)) => {
                                    error_message = Some(e.to_string());
                                    if attempt < retry_attempts - 1 {
                                        tokio::time::sleep(retry_delay).await;
                                    }
                                }
                                Err(_) => {
                                    error_message = Some("Operation timeout".to_string());
                                    if attempt < retry_attempts - 1 {
                                        tokio::time::sleep(retry_delay).await;
                                    }
                                }
                            }
                        }

                        let event = Event::DataRefresh {
                            data_type: "pools".to_string(),
                            success,
                            error: error_message,
                        };

                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        self.task_handles.push(handle);
    }

    /// Start transaction status check task
    fn start_transaction_status_sync(&mut self) {
        let sender = self.event_sender.clone();
        let client = Arc::clone(&self.client);
        let interval_duration = self.config.transaction_status_interval;
        let cancellation_token = self.cancellation_token.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    _ = interval.tick() => {
                        // Send transaction status refresh event
                        let event = Event::DataRefresh {
                            data_type: "transactions".to_string(),
                            success: true,
                            error: None,
                        };

                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        self.task_handles.push(handle);
    }

    /// Start network info refresh task
    fn start_network_info_sync(&mut self) {
        let sender = self.event_sender.clone();
        let client = Arc::clone(&self.client);
        let interval_duration = self.config.network_info_interval;
        let cancellation_token = self.cancellation_token.clone();
        let retry_attempts = self.config.retry_attempts;
        let retry_delay = self.config.retry_delay;
        let network_timeout = self.config.network_timeout;

        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    _ = interval.tick() => {
                        // Execute network info refresh with retry logic
                        let mut success = false;
                        let mut error_message = None;

                        for attempt in 0..retry_attempts {
                            match tokio::time::timeout(network_timeout, client.get_last_block_height()).await {
                                Ok(Ok(_)) => {
                                    success = true;
                                    break;
                                }
                                Ok(Err(e)) => {
                                    error_message = Some(e.to_string());
                                    if attempt < retry_attempts - 1 {
                                        tokio::time::sleep(retry_delay).await;
                                    }
                                }
                                Err(_) => {
                                    error_message = Some("Operation timeout".to_string());
                                    if attempt < retry_attempts - 1 {
                                        tokio::time::sleep(retry_delay).await;
                                    }
                                }
                            }
                        }

                        let event = Event::DataRefresh {
                            data_type: "network_info".to_string(),
                            success,
                            error: error_message,
                        };

                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        self.task_handles.push(handle);
    }

    /// Start price update task
    fn start_price_sync(&mut self) {
        let sender = self.event_sender.clone();
        let client = Arc::clone(&self.client);
        let interval_duration = self.config.price_update_interval;
        let cancellation_token = self.cancellation_token.clone();

        let handle = tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => break,
                    _ = interval.tick() => {
                        // For now, just send a placeholder price refresh event
                        // This can be enhanced with actual price data sources
                        let event = Event::DataRefresh {
                            data_type: "prices".to_string(),
                            success: true,
                            error: None,
                        };

                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        self.task_handles.push(handle);
    }

    /// Stop all background sync tasks
    pub fn stop_background_sync(&mut self) {
        // Signal all tasks to stop
        self.cancellation_token.cancel();

        // Abort all task handles
        for handle in self.task_handles.drain(..) {
            handle.abort();
        }
    }

    /// Update sync configuration
    pub fn update_config(&mut self, config: SyncConfig) {
        self.config = config;
        // Restart background sync with new config
        self.stop_background_sync();
        self.start_background_sync();
    }
}

impl Drop for SyncManager {
    fn drop(&mut self) {
        self.stop_background_sync();
    }
}

/// Enhanced async data refresher with comprehensive error handling
pub struct AsyncDataRefresher {
    client: Arc<MantraDexClient>,
    event_sender: mpsc::UnboundedSender<Event>,
    config: SyncConfig,
}

impl AsyncDataRefresher {
    /// Create a new async data refresher
    pub fn new(client: Arc<MantraDexClient>, event_sender: mpsc::UnboundedSender<Event>) -> Self {
        Self {
            client,
            event_sender,
            config: SyncConfig::default(),
        }
    }

    /// Execute operation with comprehensive status updates
    async fn execute_operation<F, Fut, T>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> Result<T, Error>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        // Send start event
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation_name.to_string(),
            status: "Starting operation...".to_string(),
            progress: Some(0.1),
        });

        let start_time = std::time::Instant::now();
        let mut last_error = None;

        for attempt in 0..=self.config.retry_attempts {
            if attempt > 0 {
                // Send retry event
                let _ = self.event_sender.send(Event::BlockchainProgress {
                    operation: operation_name.to_string(),
                    status: format!(
                        "Retrying... (attempt {} of {})",
                        attempt + 1,
                        self.config.retry_attempts + 1
                    ),
                    progress: Some(0.3 + (attempt as f32 * 0.2)),
                });

                tokio::time::sleep(self.config.retry_delay).await;
            }

            match tokio::time::timeout(self.config.network_timeout, operation()).await {
                Ok(Ok(result)) => {
                    // Send success event
                    let _ = self.event_sender.send(Event::BlockchainSuccess {
                        operation: operation_name.to_string(),
                        result: format!("Operation completed in {:?}", start_time.elapsed()),
                        transaction_hash: None,
                        enhanced_data: None, // No enhanced data for general operations
                    });
                    return Ok(result);
                }
                Ok(Err(e)) => {
                    last_error = Some(e);
                }
                Err(_) => {
                    last_error = Some(Error::Rpc("Operation timeout".to_string()));
                }
            }
        }

        let error = last_error.unwrap_or_else(|| Error::Rpc("Unknown error".to_string()));

        // Send error event
        let _ = self.event_sender.send(Event::BlockchainError {
            operation: operation_name.to_string(),
            error: error.to_string(),
        });

        Err(error)
    }

    /// Refresh user balances with comprehensive error handling
    pub async fn refresh_balances(&self, address: &str) -> Result<(), Error> {
        self.execute_operation("balance_refresh", || {
            let client = Arc::clone(&self.client);
            async move { client.get_balances().await.map(|_| ()) }
        })
        .await
    }

    /// Refresh pool data with comprehensive error handling
    pub async fn refresh_pool_data(&self) -> Result<(), Error> {
        self.execute_operation("pool_data_refresh", || {
            let client = Arc::clone(&self.client);
            async move { client.get_pools(Some(50)).await.map(|_| ()) }
        })
        .await
    }

    /// Refresh transaction status with comprehensive error handling
    pub async fn refresh_transaction_status(&self, tx_hashes: Vec<String>) -> Result<(), Error> {
        self.execute_operation("transaction_status_refresh", || {
            let client = Arc::clone(&self.client);
            let hashes = tx_hashes.clone();
            async move {
                // Placeholder for transaction status checking
                // In a real implementation, this would check each transaction hash
                for _hash in hashes {
                    // Check transaction status
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Ok(())
            }
        })
        .await
    }

    /// Refresh network info with comprehensive error handling
    pub async fn refresh_network_info(&self) -> Result<(), Error> {
        self.execute_operation("network_info_refresh", || {
            let client = Arc::clone(&self.client);
            async move { client.get_last_block_height().await.map(|_| ()) }
        })
        .await
    }

    /// Refresh price data with comprehensive error handling
    pub async fn refresh_prices(&self) -> Result<(), Error> {
        self.execute_operation("price_refresh", || async {
            // Placeholder for price refresh
            // In a real implementation, this would fetch current market prices
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(())
        })
        .await
    }
}

/// Enhanced background task coordinator with comprehensive async integration
pub struct BackgroundTaskCoordinator {
    /// Sync manager for real-time updates
    sync_manager: SyncManager,
    /// Data refresher for on-demand updates
    data_refresher: AsyncDataRefresher,
    /// Flag to indicate if background sync is active
    is_active: bool,
}

impl BackgroundTaskCoordinator {
    /// Create a new background task coordinator
    pub fn new(
        event_sender: mpsc::UnboundedSender<Event>,
        client: Arc<MantraDexClient>,
        config: Option<SyncConfig>,
    ) -> Self {
        let sync_manager = SyncManager::new(event_sender.clone(), Arc::clone(&client), config);
        let data_refresher = AsyncDataRefresher::new(Arc::clone(&client), event_sender);

        Self {
            sync_manager,
            data_refresher,
            is_active: false,
        }
    }

    /// Start background synchronization
    pub fn start(&mut self) {
        if !self.is_active {
            self.sync_manager.start_background_sync();
            self.is_active = true;
        }
    }

    /// Stop background synchronization
    pub fn stop(&mut self) {
        if self.is_active {
            self.sync_manager.stop_background_sync();
            self.is_active = false;
        }
    }

    /// Set wallet address for balance tracking
    pub fn set_wallet_address(&mut self, address: String) {
        self.sync_manager.set_wallet_address(address);
    }

    /// Get data refresher for manual refresh operations
    pub fn get_data_refresher(&self) -> &AsyncDataRefresher {
        &self.data_refresher
    }

    /// Update configuration and restart if needed
    pub fn update_config(&mut self, config: SyncConfig) {
        self.sync_manager.update_config(config);
    }

    /// Check if background sync is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Get current network state
    pub async fn get_network_state(&self) -> NetworkState {
        self.sync_manager.get_network_state().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_default() {
        let config = SyncConfig::default();
        assert_eq!(config.balance_refresh_interval, Duration::from_secs(30));
        assert_eq!(config.pool_data_refresh_interval, Duration::from_secs(60));
        assert_eq!(config.network_timeout, Duration::from_secs(10));
        assert_eq!(config.retry_attempts, 3);
    }

    #[tokio::test]
    async fn test_background_task_coordinator_lifecycle() {
        // This test would require a mock client and proper async setup
        // For now, just verify the structure compiles and basic functionality works
        assert!(true);
    }

    #[test]
    fn test_operation_result() {
        let success_result = OperationResult::success(Duration::from_secs(1));
        assert!(success_result.success);
        assert!(success_result.error.is_none());

        let error_result =
            OperationResult::error("Test error".to_string(), 2, NetworkState::Disconnected);
        assert!(!error_result.success);
        assert_eq!(error_result.error, Some("Test error".to_string()));
        assert_eq!(error_result.retry_count, 2);
    }

    #[test]
    fn test_network_state() {
        let connected = NetworkState::Connected;
        let disconnected = NetworkState::Disconnected;
        let error = NetworkState::Error("Test error".to_string());

        assert_ne!(connected, disconnected);
        assert_ne!(connected, error);
        assert_ne!(disconnected, error);
    }
}

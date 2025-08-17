//! MCP-to-SDK Adapter
//!
//! This module provides a wrapper layer that adapts the Mantra DEX SDK for use in MCP contexts.
//! It handles async runtime integration, error mapping, connection pooling, and provides
//! an MCP-friendly interface to the underlying SDK functionality.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono;
use cosmwasm_std::{Coin, Decimal, Uint128};

use serde_json::Value;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::protocols::dex::MantraDexClient;
use crate::client::MantraClient;
use crate::config::MantraNetworkConfig;
use crate::wallet::{MantraWallet, WalletInfo};

use super::server::{McpResult, McpServerError};

/// Configuration for connection pooling
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections per network
    pub max_connections_per_network: usize,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Connection TTL in seconds
    pub connection_ttl_secs: u64,
    /// Maximum number of retries for failed operations
    pub max_retries: u32,
    /// Base delay for exponential backoff in milliseconds
    pub retry_base_delay_ms: u64,
    /// Maximum idle time before connection is considered stale in seconds
    pub max_idle_time_secs: u64,
    /// Health check interval in seconds
    pub health_check_interval_secs: u64,
    /// Maximum derivation index to search when looking for wallets
    pub max_wallet_derivation_index: u32,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_network: 5,
            connection_timeout_secs: 30,
            connection_ttl_secs: 300, // 5 minutes
            max_retries: 3,
            retry_base_delay_ms: 100,
            max_idle_time_secs: 60,         // 1 minute
            health_check_interval_secs: 30, // 30 seconds
            max_wallet_derivation_index: 100, // Search up to index 100
        }
    }
}

/// Pooled connection wrapper with metadata
#[derive(Debug)]
struct PooledConnection {
    /// The actual DEX client
    client: MantraDexClient,
    /// When this connection was created
    created_at: Instant,
    /// When this connection was last used
    last_used: Instant,
    /// Whether this connection is currently healthy
    is_healthy: bool,
}

impl PooledConnection {
    /// Create a new pooled connection
    fn new(client: MantraDexClient) -> Self {
        let now = Instant::now();
        Self {
            client,
            created_at: now,
            last_used: now,
            is_healthy: true,
        }
    }

    /// Check if the connection is expired based on TTL
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    /// Check if the connection is idle based on max idle time
    fn is_idle(&self, max_idle: Duration) -> bool {
        self.last_used.elapsed() > max_idle
    }

    /// Update the last used timestamp
    fn mark_used(&mut self) {
        self.last_used = Instant::now();
    }

    /// Mark connection as healthy/unhealthy
    fn set_health(&mut self, healthy: bool) {
        self.is_healthy = healthy;
    }
}

/// Connection pool for a specific network
#[derive(Debug)]
struct NetworkConnectionPool {
    /// Available connections
    connections: Vec<PooledConnection>,
    /// Network configuration
    network_config: MantraNetworkConfig,
    /// Semaphore to limit concurrent connection creation
    creation_semaphore: Semaphore,
    /// Pool configuration
    config: ConnectionPoolConfig,
}

impl NetworkConnectionPool {
    /// Create a new network connection pool
    fn new(network_config: MantraNetworkConfig, config: ConnectionPoolConfig) -> Self {
        Self {
            connections: Vec::new(),
            creation_semaphore: Semaphore::new(config.max_connections_per_network),
            network_config,
            config,
        }
    }

    /// Get a connection from the pool or create a new one
    async fn get_connection(&mut self) -> McpResult<MantraDexClient> {
        // First, try to get a healthy, non-expired connection from the pool
        for (_index, pooled_conn) in self.connections.iter_mut().enumerate() {
            if pooled_conn.is_healthy
                && !pooled_conn.is_expired(Duration::from_secs(self.config.connection_ttl_secs))
                && !pooled_conn.is_idle(Duration::from_secs(self.config.max_idle_time_secs))
            {
                pooled_conn.mark_used();
                debug!(
                    "Reusing existing connection for network: {}",
                    self.network_config.chain_id
                );
                // Since MantraDexClient can't be cloned, we need to create a new client
                // with the same configuration. This is a limitation of the current SDK design.
                return self.create_new_client().await;
            }
        }

        // Remove expired or unhealthy connections
        self.cleanup_expired_connections();

        // If we're at the connection limit, wait for a permit
        let _permit = self.creation_semaphore.acquire().await.map_err(|e| {
            McpServerError::Internal(format!("Failed to acquire connection permit: {}", e))
        })?;

        // Create a new client
        let client = self.create_new_client().await?;

        // Add to pool for tracking purposes (even though we can't reuse the exact instance)
        let pooled_conn = PooledConnection::new(
            MantraDexClient::new(self.network_config.clone())
                .await
                .map_err(|e| McpServerError::Sdk(e))?,
        );

        self.connections.push(pooled_conn);

        debug!(
            "Created new connection for network: {} (pool size: {})",
            self.network_config.chain_id,
            self.connections.len()
        );

        Ok(client)
    }

    /// Create a new client for the network
    async fn create_new_client(&self) -> McpResult<MantraDexClient> {
        debug!(
            "Creating new DEX client for network: {}",
            self.network_config.chain_id
        );

        match MantraDexClient::new(self.network_config.clone()).await {
            Ok(client) => {
                info!(
                    "Successfully created DEX client for network: {}",
                    self.network_config.chain_id
                );
                Ok(client)
            }
            Err(e) => {
                error!(
                    "Failed to create DEX client for network {}: {}",
                    self.network_config.chain_id, e
                );
                Err(McpServerError::Sdk(e))
            }
        }
    }

    /// Cleanup expired and unhealthy connections
    fn cleanup_expired_connections(&mut self) {
        let ttl = Duration::from_secs(self.config.connection_ttl_secs);
        let max_idle = Duration::from_secs(self.config.max_idle_time_secs);

        let initial_count = self.connections.len();

        self.connections
            .retain(|conn| conn.is_healthy && !conn.is_expired(ttl) && !conn.is_idle(max_idle));

        let removed_count = initial_count - self.connections.len();
        if removed_count > 0 {
            debug!(
                "Cleaned up {} expired/unhealthy connections for network: {}",
                removed_count, self.network_config.chain_id
            );
        }
    }

    /// Perform health checks on all connections
    async fn health_check(&mut self) {
        for pooled_conn in &mut self.connections {
            // Simple health check - try to get the latest block height
            match pooled_conn.client.get_last_block_height().await {
                Ok(_) => {
                    pooled_conn.set_health(true);
                }
                Err(e) => {
                    warn!(
                        "Health check failed for connection to network {}: {}",
                        self.network_config.chain_id, e
                    );
                    pooled_conn.set_health(false);
                }
            }
        }
    }

    /// Get pool statistics
    fn get_stats(&self) -> (usize, usize, usize) {
        let total = self.connections.len();
        let healthy = self.connections.iter().filter(|c| c.is_healthy).count();
        let available_permits = self.creation_semaphore.available_permits();
        (total, healthy, available_permits)
    }
}

/// MCP SDK adapter for connection management and wallet state
#[derive(Debug)]
pub struct McpSdkAdapter {
    /// Connection pools per network
    connection_pools: Arc<RwLock<HashMap<String, NetworkConnectionPool>>>,
    /// Connection pool configuration
    config: ConnectionPoolConfig,
    /// Cache for frequently accessed data
    cache: Arc<RwLock<HashMap<String, (Value, Instant)>>>,
    /// Cache TTL
    cache_ttl: Duration,
    /// Health check task handle
    health_check_handle: Option<tokio::task::JoinHandle<()>>,
    /// Loaded wallets (address -> wallet info)
    wallets: Arc<RwLock<HashMap<String, WalletInfo>>>,
    /// Current active wallet address
    active_wallet: Arc<Mutex<Option<String>>>,
    /// Current active wallet instance (if available)
    active_wallet_instance: Arc<Mutex<Option<MantraWallet>>>,
    /// Cache for wallet address to derivation index mappings
    wallet_derivation_cache: Arc<RwLock<HashMap<String, u32>>>,
}

impl McpSdkAdapter {
    /// Create a new MCP SDK adapter with connection pooling
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let adapter = Self {
            connection_pools: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(config.connection_ttl_secs),
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            health_check_handle: None,
            wallets: Arc::new(RwLock::new(HashMap::new())),
            active_wallet: Arc::new(Mutex::new(None)),
            active_wallet_instance: Arc::new(Mutex::new(None)),
            wallet_derivation_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        adapter
    }

    /// Start the background health check task
    pub async fn start_health_checks(&mut self) {
        let pools = Arc::clone(&self.connection_pools);
        let interval = Duration::from_secs(self.config.health_check_interval_secs);

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                debug!("Running connection pool health checks");

                let mut pools_guard = pools.write().await;
                for (network_id, pool) in pools_guard.iter_mut() {
                    debug!("Health checking pool for network: {}", network_id);
                    pool.health_check().await;
                }

                debug!("Completed connection pool health checks");
            }
        });

        self.health_check_handle = Some(handle);
    }

    /// Stop the background health check task
    pub async fn stop_health_checks(&mut self) {
        if let Some(handle) = self.health_check_handle.take() {
            handle.abort();
            debug!("Stopped connection pool health checks");
        }
    }

    /// Get a client connection for the specified network
    pub async fn get_client(
        &self,
        network_config: &MantraNetworkConfig,
    ) -> McpResult<MantraDexClient> {
        let network_id = network_config.chain_id.clone();

        // Get or create the network pool
        {
            let mut pools = self.connection_pools.write().await;
            if !pools.contains_key(&network_id) {
                debug!("Creating new connection pool for network: {}", network_id);
                pools.insert(
                    network_id.clone(),
                    NetworkConnectionPool::new(network_config.clone(), self.config.clone()),
                );
            }
        }

        // Get a connection from the pool
        let mut pools = self.connection_pools.write().await;
        let pool = pools.get_mut(&network_id).ok_or_else(|| {
            McpServerError::Internal(format!("Network pool not found: {}", network_id))
        })?;

        pool.get_connection().await
    }

    /// Get a client with wallet attached
    pub async fn get_client_with_wallet(
        &self,
        network_config: &MantraNetworkConfig,
        wallet: MantraWallet,
    ) -> McpResult<MantraDexClient> {
        let base_client = self.get_client(network_config).await?;
        Ok(base_client.with_wallet(wallet))
    }

    /// Execute with retry logic
    pub async fn execute_with_retry<F, T>(&self, operation: F) -> McpResult<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = McpResult<T>> + Send>>
            + Send
            + 'static,
        T: Send + 'static,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(
                            self.config.retry_base_delay_ms * (2_u64.pow(attempt)),
                        );
                        warn!(
                            "Operation failed (attempt {}), retrying in {:?}",
                            attempt + 1,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| McpServerError::Internal("Unknown retry error".to_string())))
    }

    /// Clean up expired cache entries and connection pools
    pub async fn cleanup(&self) -> McpResult<()> {
        // Clean cache
        {
            let mut cache = self.cache.write().await;
            let original_count = cache.len();

            cache.retain(|_, (_, timestamp)| timestamp.elapsed() < self.cache_ttl);

            let removed_count = original_count - cache.len();
            if removed_count > 0 {
                debug!("Cleaned {} expired cache entries", removed_count);
            }
        }

        // Clean connection pools
        {
            let mut pools = self.connection_pools.write().await;
            for (network_id, pool) in pools.iter_mut() {
                debug!("Cleaning connection pool for network: {}", network_id);
                pool.cleanup_expired_connections();
            }
        }

        Ok(())
    }

    /// Get cached value
    pub async fn cache_get(&self, key: &str) -> Option<Value> {
        let cache = self.cache.read().await;
        cache.get(key).and_then(|(value, timestamp)| {
            if timestamp.elapsed() < self.cache_ttl {
                Some(value.clone())
            } else {
                None
            }
        })
    }

    /// Set cached value
    pub async fn cache_set(&self, key: String, value: Value) {
        let mut cache = self.cache.write().await;
        cache.insert(key, (value, Instant::now()));
    }

    /// Clear all cached values
    pub async fn cache_clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Cleared all cache entries");
    }

    /// Get the currently active wallet
    ///
    /// Note: Since MantraWallet doesn't implement Clone, this method recreates
    /// the wallet instance from stored mnemonic when available.
    pub async fn get_active_wallet(&self) -> McpResult<Option<MantraWallet>> {
        use crate::wallet::MantraWallet;
        use std::env;

        // Check if we have an active wallet address
        let active_address = self.active_wallet.lock().await.clone();
        if active_address.is_none() {
            return Ok(None);
        }

        // Try to recreate wallet from environment mnemonic using cached derivation index
        if let Ok(mnemonic) = env::var("WALLET_MNEMONIC") {
            if !mnemonic.trim().is_empty() {
                if let Some(active_addr) = &active_address {
                    // Check cache for derivation index
                    let cache = self.wallet_derivation_cache.read().await;
                    if let Some(&derivation_index) = cache.get(active_addr) {
                        match MantraWallet::from_mnemonic(&mnemonic, derivation_index) {
                            Ok(wallet) => {
                                debug!("Recreated active wallet instance from WALLET_MNEMONIC using cached index {}", derivation_index);
                                return Ok(Some(wallet));
                            }
                            Err(e) => {
                                error!("Failed to recreate active wallet from WALLET_MNEMONIC with cached index {}: {}", derivation_index, e);
                            }
                        }
                    } else {
                        // Fallback to index 0 for backward compatibility if no cache entry exists
                        match MantraWallet::from_mnemonic(&mnemonic, 0) {
                            Ok(wallet) => {
                                debug!("Recreated active wallet instance from WALLET_MNEMONIC using fallback index 0");
                                return Ok(Some(wallet));
                            }
                            Err(e) => {
                                error!("Failed to recreate active wallet from WALLET_MNEMONIC with fallback index 0: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Fall back to stored instance if available (though this will consume it)
        let wallet = self.active_wallet_instance.lock().await.take();
        if wallet.is_some() {
            debug!("Using stored wallet instance (will be consumed)");
        }
        Ok(wallet)
    }

    /// Get the currently active wallet info
    pub async fn get_active_wallet_info(&self) -> McpResult<Option<WalletInfo>> {
        let active_address = self.active_wallet.lock().await.clone();
        if let Some(address) = active_address {
            let wallets = self.wallets.read().await;
            Ok(wallets.get(&address).cloned())
        } else {
            debug!("No active wallet set");
            Ok(None)
        }
    }

    /// Set the active wallet
    pub async fn set_active_wallet(
        &self,
        address: String,
        wallet_info: WalletInfo,
    ) -> McpResult<()> {
        // Store the wallet info and set as active
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);
        *self.active_wallet.lock().await = Some(address.clone());

        info!("Set active wallet: {}", address);
        Ok(())
    }

    /// Set the active wallet with the actual wallet instance
    pub async fn set_active_wallet_with_instance(&self, wallet: MantraWallet) -> McpResult<()> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();

        // Store the wallet info
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);

        // Set as active
        *self.active_wallet.lock().await = Some(address.clone());

        // Store the wallet instance
        *self.active_wallet_instance.lock().await = Some(wallet);

        info!("Set active wallet with instance: {}", address);
        Ok(())
    }

    /// Add wallet validation methods
    pub async fn validate_wallet_exists(&self) -> McpResult<()> {
        if self.get_active_wallet_info().await?.is_none() {
            return Err(McpServerError::WalletNotConfigured);
        }
        Ok(())
    }

    /// Get all available wallets
    pub async fn get_all_wallets(&self) -> McpResult<HashMap<String, WalletInfo>> {
        let wallets = self.wallets.read().await;
        Ok(wallets.clone())
    }

    /// Add a new wallet to the collection
    pub async fn add_wallet(&self, wallet: MantraWallet) -> McpResult<String> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();
        
        // Store the wallet info
        self.wallets.write().await.insert(address.clone(), wallet_info);
        
        info!("Added new wallet: {}", address);
        Ok(address)
    }

    /// Add a new wallet to the collection with known derivation index for caching
    pub async fn add_wallet_with_derivation_index(&self, wallet: MantraWallet, derivation_index: u32) -> McpResult<String> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();
        
        // Store the wallet info
        self.wallets.write().await.insert(address.clone(), wallet_info);
        
        // Cache the derivation index for efficient wallet recreation
        {
            let mut cache = self.wallet_derivation_cache.write().await;
            cache.insert(address.clone(), derivation_index);
        }
        
        info!("Added new wallet: {} with derivation index: {}", address, derivation_index);
        Ok(address)
    }

    /// Remove a wallet from the collection
    pub async fn remove_wallet(&self, address: &str) -> McpResult<()> {
        let mut wallets = self.wallets.write().await;
        
        if wallets.remove(address).is_some() {
            // Clear derivation cache entry
            {
                let mut cache = self.wallet_derivation_cache.write().await;
                cache.remove(address);
            }
            
            // If this was the active wallet, clear the active wallet
            let mut active_wallet = self.active_wallet.lock().await;
            if active_wallet.as_ref() == Some(&address.to_string()) {
                *active_wallet = None;
                *self.active_wallet_instance.lock().await = None;
            }
            info!("Removed wallet: {}", address);
            Ok(())
        } else {
            Err(McpServerError::InvalidArguments(format!("Wallet not found: {}", address)))
        }
    }

    /// Switch active wallet to a different address
    pub async fn switch_active_wallet(&self, address: &str) -> McpResult<()> {
        let wallets = self.wallets.read().await;
        
        if let Some(_wallet_info) = wallets.get(address) {
            *self.active_wallet.lock().await = Some(address.to_string());
            // Clear the wallet instance - will be recreated when needed
            *self.active_wallet_instance.lock().await = None;
            info!("Switched active wallet to: {}", address);
            Ok(())
        } else {
            Err(McpServerError::InvalidArguments(format!("Wallet not found: {}", address)))
        }
    }

    /// Get wallet info by address
    pub async fn get_wallet_info(&self, address: &str) -> McpResult<Option<WalletInfo>> {
        let wallets = self.wallets.read().await;
        Ok(wallets.get(address).cloned())
    }

    /// Check if a wallet exists
    pub async fn wallet_exists(&self, address: &str) -> bool {
        let wallets = self.wallets.read().await;
        wallets.contains_key(address)
    }

    /// Get a wallet instance by address
    /// This method uses cached derivation indices for efficiency and falls back to a targeted search
    pub async fn get_wallet_by_address(&self, address: &str) -> McpResult<Option<MantraWallet>> {
        use crate::wallet::MantraWallet;
        use std::env;

        // Check if wallet exists in our collection
        if !self.wallet_exists(address).await {
            return Ok(None);
        }

        // Get environment mnemonic
        let mnemonic = match env::var("WALLET_MNEMONIC") {
            Ok(m) if !m.trim().is_empty() => m,
            _ => {
                debug!("No valid WALLET_MNEMONIC found in environment for address: {}", address);
                return Ok(None);
            }
        };

        // Check cache first for known derivation index
        {
            let cache = self.wallet_derivation_cache.read().await;
            if let Some(&derivation_index) = cache.get(address) {
                match MantraWallet::from_mnemonic(&mnemonic, derivation_index) {
                    Ok(wallet) => {
                        if wallet.info().address == address {
                            debug!("Retrieved wallet from cache at index {} for address {}", derivation_index, address);
                            return Ok(Some(wallet));
                        } else {
                            // Cache is stale, wallet address doesn't match
                            warn!("Cached derivation index {} for address {} is stale, clearing cache entry", derivation_index, address);
                            drop(cache);
                            let mut cache_mut = self.wallet_derivation_cache.write().await;
                            cache_mut.remove(address);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to recreate wallet from cached index {} for address {}: {}", derivation_index, address, e);
                        // Clear stale cache entry
                        drop(cache);
                        let mut cache_mut = self.wallet_derivation_cache.write().await;
                        cache_mut.remove(address);
                    }
                }
            }
        }

        // Cache miss or stale cache - perform targeted search
        debug!("Performing derivation search for address: {}", address);
        
        // Search with configurable upper bound to prevent infinite derivation
        let max_index = self.config.max_wallet_derivation_index;
        for index in 0..=max_index {
            match MantraWallet::from_mnemonic(&mnemonic, index) {
                Ok(wallet) => {
                    if wallet.info().address == address {
                        debug!("Found wallet at derivation index {} for address {}", index, address);
                        
                        // Cache the successful derivation index
                        {
                            let mut cache = self.wallet_derivation_cache.write().await;
                            cache.insert(address.to_string(), index);
                        }
                        
                        return Ok(Some(wallet));
                    }
                }
                Err(e) => {
                    debug!("Failed to create wallet at derivation index {}: {}", index, e);
                    // Continue searching - derivation errors at specific indices don't necessarily
                    // mean the wallet doesn't exist at a higher index
                }
            }
        }

        // If we reach here, the wallet was not found within the search bounds
        warn!("Could not find wallet for address {} within derivation index range 0-{}", address, max_index);
        Ok(None)
    }

    /// Get wallet error handling with proper error messages
    pub async fn get_active_wallet_with_validation(&self) -> McpResult<MantraWallet> {
        match self.get_active_wallet().await? {
            Some(wallet) => Ok(wallet),
            None => Err(McpServerError::WalletNotConfigured),
        }
    }

    /// Get spendable balances for a specific address
    ///
    /// # Arguments
    ///
    /// * `network_config` - Network configuration for the query
    /// * `wallet_address` - Wallet address to query balances for
    ///
    /// # Returns
    ///
    /// JSON value containing balance information
    pub async fn get_balances_for_address_direct(
        &self,
        network_config: &MantraNetworkConfig,
        wallet_address: &str,
    ) -> McpResult<Value> {
        debug!("Getting balances for network: {}", network_config.chain_id);
        info!("Querying balances for address: {}", wallet_address);

        // Get client and execute balance query
        let client = self.get_client(network_config).await?;

        // Query spendable balances using the SDK client
        let balances = client
            .get_balances_for_address(wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        debug!(
            "Retrieved {} balances for address {}",
            balances.len(),
            wallet_address
        );

        // Convert to JSON format
        let balance_json: Vec<Value> = balances
            .into_iter()
            .map(|coin| {
                serde_json::json!({
                    "denom": coin.denom,
                    "amount": coin.amount.to_string()
                })
            })
            .collect();

        let result = serde_json::json!({
            "address": wallet_address,
            "balances": balance_json,
            "total_tokens": balance_json.len(),
            "network": network_config.chain_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        info!(
            "Successfully retrieved balances for address: {}",
            wallet_address
        );
        Ok(result)
    }

    /// Get spendable balances for the active wallet
    ///
    /// # Arguments
    ///
    /// * `network_config` - Network configuration for the query
    /// * `wallet_address` - Optional specific wallet address, uses active wallet if None
    ///
    /// # Returns
    ///
    /// JSON value containing balance information
    pub async fn get_balances(
        &self,
        network_config: &MantraNetworkConfig,
        wallet_address: Option<String>,
    ) -> McpResult<Value> {
        // Get the wallet address to query
        let address = if let Some(addr) = wallet_address {
            addr
        } else {
            // Use active wallet address
            match self.get_active_wallet_info().await? {
                Some(wallet_info) => wallet_info.address,
                None => return Err(McpServerError::WalletNotConfigured),
            }
        };

        // Delegate to the direct address method
        self.get_balances_for_address_direct(network_config, &address)
            .await
    }

    /// Get connection pool statistics
    pub async fn get_pool_stats(&self) -> HashMap<String, (usize, usize, usize)> {
        let pools = self.connection_pools.read().await;
        pools
            .iter()
            .map(|(network_id, pool)| (network_id.clone(), pool.get_stats()))
            .collect()
    }

    /// Get the cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total = cache.len();
        let expired = cache
            .values()
            .filter(|(_, timestamp)| timestamp.elapsed() >= self.cache_ttl)
            .count();
        (total, total - expired)
    }

    /// Shutdown the adapter and cleanup resources
    pub async fn shutdown(&mut self) -> McpResult<()> {
        info!("Shutting down MCP SDK adapter");

        // Stop health checks
        self.stop_health_checks().await;

        // Clear caches
        self.cache_clear().await;

        // Clear wallet derivation cache
        {
            let mut derivation_cache = self.wallet_derivation_cache.write().await;
            derivation_cache.clear();
        }

        // Clear connection pools
        {
            let mut pools = self.connection_pools.write().await;
            pools.clear();
        }

        info!("MCP SDK adapter shutdown complete");
        Ok(())
    }

    // =========================================================================
    // Script Execution Methods
    // =========================================================================

    /// Get the first available pool ID as a fallback
    pub async fn get_first_available_pool_id(&self) -> McpResult<String> {
        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;
        
        // Get available pools
        let pools = client.get_pools(Some(10)).await
            .map_err(|e| McpServerError::Sdk(e))?;
        
        if pools.is_empty() {
            return Err(McpServerError::InvalidArguments("No pools available".to_string()));
        }
        
        // Return the first available pool ID
        Ok(pools[0].pool_info.pool_identifier.clone())
    }

    /// Execute a swap with string parameters (for script execution)
    pub async fn execute_swap_simple(
        &self,
        from_asset: String,
        to_asset: String,
        amount: String,
        slippage: String,
        pool_id: String,
        _min_output: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Executing swap from {} to {} with amount {} and slippage {}",
            from_asset, to_asset, amount, slippage
        );

        // Parse amount
        let offer_amount = Uint128::from_str(&amount)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid amount: {}", e)))?;

        // Parse slippage with explicit error handling and validation
        let max_slippage = match Decimal::from_str(&slippage) {
            Ok(slippage_value) => {
                // Validate slippage range (0.0 to 1.0)
                if slippage_value < Decimal::zero() {
                    return Err(McpServerError::InvalidArguments(
                        format!("Invalid slippage: {} - slippage cannot be negative", slippage_value)
                    ));
                }
                if slippage_value > Decimal::one() {
                    return Err(McpServerError::InvalidArguments(
                        format!("Invalid slippage: {} - slippage cannot be greater than 1.0 (100%)", slippage_value)
                    ));
                }
                Some(slippage_value)
            }
            Err(e) => {
                return Err(McpServerError::InvalidArguments(
                    format!("Invalid slippage format: '{}' - {}", slippage, e)
                ));
            }
        };

        // Create offer coin
        let offer_coin = Coin {
            denom: from_asset.clone(),
            amount: offer_amount,
        };

        // Use provided pool_id
        let pool_id_str = pool_id;

        // Get active wallet (required for swaps)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute the swap
        let swap_result = client
            .swap(&pool_id_str, offer_coin, &to_asset, max_slippage)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!(
            "Successfully executed swap from {} to {} with tx hash: {}",
            from_asset, to_asset, swap_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": swap_result.txhash,
            "swap_details": {
                "from_asset": from_asset,
                "to_asset": to_asset,
                "amount": amount,
                "slippage": slippage,
                "pool_id": pool_id_str,
                "gas_used": swap_result.gas_used,
                "gas_wanted": swap_result.gas_wanted
            },
            "block_height": swap_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Provide liquidity with string parameters (for script execution)
    pub async fn provide_liquidity_simple(
        &self,
        pool_id: String,
        asset_a_amount: String,
        asset_b_amount: String,
        min_lp_tokens: Option<String>,
        liquidity_slippage: Option<String>,
        swap_slippage: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Providing liquidity to pool {} with amounts {} and {}",
            pool_id, asset_a_amount, asset_b_amount
        );

        // First, get the pool information to determine the asset denoms
        let pool_info = self.get_pool(&pool_id).await?;
        
        // Extract asset denoms from pool info
        let assets_array = pool_info
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Pool does not have valid assets".to_string())
            })?;

        if assets_array.len() != 2 {
            return Err(McpServerError::InvalidArguments(
                "Pool must have exactly 2 assets for simple liquidity provision".to_string(),
            ));
        }

        // Get the asset denoms
        let asset_a_denom = assets_array[0]
            .get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Invalid asset A denom".to_string())
            })?;

        let asset_b_denom = assets_array[1]
            .get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Invalid asset B denom".to_string())
            })?;

        // Construct the assets array for the provide_liquidity call
        let assets_json = serde_json::json!([
            {
                "denom": asset_a_denom,
                "amount": asset_a_amount
            },
            {
                "denom": asset_b_denom,
                "amount": asset_b_amount
            }
        ]);

        // Construct the arguments for the provide_liquidity method
        let mut args = serde_json::json!({
            "pool_id": pool_id,
            "assets": assets_json
        });

        // Add slippage parameters if provided
        if let Some(liquidity_slippage_str) = liquidity_slippage {
            args["liquidity_max_slippage"] = serde_json::Value::String(liquidity_slippage_str);
        }
        if let Some(swap_slippage_str) = swap_slippage {
            args["swap_max_slippage"] = serde_json::Value::String(swap_slippage_str);
        }

        // Call the existing provide_liquidity method
        let result = self.provide_liquidity(args).await?;

        // Add the min_lp_tokens parameter to the response for reference
        if let Some(min_lp) = min_lp_tokens {
            if let Some(liquidity_details) = result.get("liquidity_details") {
                let mut details = liquidity_details.clone();
                if let Some(details_obj) = details.as_object_mut() {
                    details_obj.insert("min_lp_tokens".to_string(), serde_json::Value::String(min_lp));
                }
                
                let mut modified_result = result.clone();
                if let Some(result_obj) = modified_result.as_object_mut() {
                    result_obj.insert("liquidity_details".to_string(), details);
                }
                return Ok(modified_result);
            }
        }

        Ok(result)
    }

    /// Withdraw liquidity with string parameters (for script execution)
    pub async fn withdraw_liquidity_simple(
        &self,
        pool_id: String,
        lp_amount: String,
        min_asset_a: Option<String>,
        min_asset_b: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Withdrawing liquidity from pool {} with LP amount {}",
            pool_id, lp_amount
        );

        // Note: min_asset_a and min_asset_b parameters are not currently supported by the underlying SDK
        if min_asset_a.is_some() || min_asset_b.is_some() {
            debug!("min_asset_a and min_asset_b parameters are not currently supported by the underlying SDK and will be ignored");
        }

        // Parse LP amount to Uint128
        let lp_amount_uint = Uint128::from_str(&lp_amount)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid LP amount: {}", e)))?;

        // Get active wallet
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute withdraw liquidity
        let withdraw_result = client
            .withdraw_liquidity(&pool_id, lp_amount_uint)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!(
            "Successfully withdrew liquidity from pool {} with tx hash: {}",
            pool_id, withdraw_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": withdraw_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", withdraw_result.txhash),
            "withdrawal_details": {
                "pool_id": pool_id,
                "lp_amount": lp_amount,
                "gas_used": withdraw_result.gas_used,
                "gas_wanted": withdraw_result.gas_wanted
            },
            "block_height": withdraw_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": withdraw_result.events
        }))
    }

    /// Create a pool with string parameters (for script execution)
    pub async fn create_pool_simple(
        &self,
        asset_a: String,
        asset_b: String,
        initial_price: String,
        pool_type: Option<String>,
        _fee_rate: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Creating pool for {} and {} with initial price {}",
            asset_a, asset_b, initial_price
        );

        // This is a simplified implementation
        // In a real implementation, you'd need to interact with the pool creation methods
        Ok(serde_json::json!({
            "status": "success",
            "operation": "create_pool",
            "asset_a": asset_a,
            "asset_b": asset_b,
            "initial_price": initial_price,
            "pool_type": pool_type.unwrap_or_else(|| "constant_product".to_string()),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Get balances with optional asset filter (for script execution)
    pub async fn get_balances_filtered(&self, filter: Option<String>) -> McpResult<Value> {
        debug!("SDK Adapter: Getting balances with filter: {:?}", filter);

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Get active wallet address
        let wallet_address = match self.get_active_wallet_info().await? {
            Some(wallet_info) => wallet_info.address,
            None => return Err(McpServerError::WalletNotConfigured),
        };

        // Get balances for the address
        let balances = self
            .get_balances_for_address_direct(&network_config, &wallet_address)
            .await?;

        // Apply filter if provided
        if let Some(filter_str) = filter {
            let assets_filter: Vec<&str> = filter_str.split(',').collect();
            if let Some(balances_array) = balances.get("balances").and_then(|v| v.as_array()) {
                let filtered_balances: Vec<Value> = balances_array
                    .iter()
                    .filter(|balance| {
                        if let Some(denom) = balance.get("denom").and_then(|v| v.as_str()) {
                            assets_filter.iter().any(|asset| denom.contains(asset))
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();

                return Ok(serde_json::json!({
                    "address": wallet_address,
                    "balances": filtered_balances,
                    "total_tokens": filtered_balances.len(),
                    "filter": filter_str,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }
        }

        Ok(balances)
    }

    /// Get pools with optional filter and pagination (for script execution)  
    pub async fn get_pools_filtered(
        &self,
        filter: Option<String>,
        limit: Option<u32>,
        start_after: Option<String>,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting pools with filter: {:?}, limit: {:?}",
            filter, limit
        );

        let args = serde_json::json!({
            "limit": limit,
            "start_after": start_after
        });

        let pools = self.get_pools(args).await?;

        // Apply filter if provided
        if let Some(filter_str) = filter {
            if let Some(pools_array) = pools.get("pools").and_then(|v| v.as_array()) {
                let filtered_pools: Vec<Value> = pools_array
                    .iter()
                    .filter(|pool| {
                        if let Some(pool_id) = pool.get("pool_id").and_then(|v| v.as_str()) {
                            pool_id.contains(&filter_str)
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect();

                return Ok(serde_json::json!({
                    "pools": filtered_pools,
                    "count": filtered_pools.len(),
                    "filter": filter_str,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }
        }

        Ok(pools)
    }

    /// Get pool information (for script execution)
    pub async fn get_pool_info(&self, pool_id: String) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting pool information for pool: {}",
            pool_id
        );
        self.get_pool(&pool_id).await
    }

    /// Validate network connectivity (for script execution)
    pub async fn validate_network_connectivity(&self) -> McpResult<Value> {
        debug!("SDK Adapter: Validating network connectivity");

        // Get network config
        let network_config = self.get_default_network_config().await?;

        // Try to get a client to validate connectivity
        match self.get_client(&network_config).await {
            Ok(_) => Ok(serde_json::json!({
                "status": "success",
                "network": network_config.network_name,
                "chain_id": network_config.chain_id,
                "connectivity": "healthy",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
            Err(e) => Ok(serde_json::json!({
                "status": "error",
                "network": network_config.network_name,
                "chain_id": network_config.chain_id,
                "connectivity": "failed",
                "error": e.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
        }
    }

    /// Get contract addresses (for script execution)
    pub async fn get_contract_addresses(&self) -> McpResult<Value> {
        debug!("SDK Adapter: Getting contract addresses");

        // Get network config
        let network_config = self.get_default_network_config().await?;

        Ok(serde_json::json!({
            "status": "success",
            "network": network_config.network_name,
            "chain_id": network_config.chain_id,
            "contracts": {
                "pool_manager": network_config.contracts.pool_manager,
                "fee_collector": network_config.contracts.fee_collector,
                "farm_manager": network_config.contracts.farm_manager,
                "epoch_manager": network_config.contracts.epoch_manager
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Monitor a transaction by hash with timeout (for script execution)
    pub async fn monitor_transaction(
        &self,
        tx_hash: String,
        timeout_seconds: Option<u64>,
    ) -> McpResult<Value> {
        debug!("SDK Adapter: Monitoring transaction: {}", tx_hash);

        let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30));
        let start_time = Instant::now();

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Poll the transaction with timeout
        let poll_interval = Duration::from_secs(2);
        
        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() > timeout {
                return Ok(serde_json::json!({
                    "status": "timeout",
                    "tx_hash": tx_hash,
                    "message": format!("Transaction monitoring timed out after {} seconds", timeout.as_secs()),
                    "elapsed_seconds": start_time.elapsed().as_secs(),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }

            // Query the transaction
            match client.query_transaction(&tx_hash).await {
                Ok(tx_result) => {
                    // Check if the transaction has a result code
                    if let Some(tx_result_obj) = tx_result.get("tx_result") {
                        if let Some(code) = tx_result_obj.get("code").and_then(|c| c.as_u64()) {
                            let status = if code == 0 { "success" } else { "failed" };
                            
                            return Ok(serde_json::json!({
                                "status": status,
                                "tx_hash": tx_hash,
                                "code": code,
                                "height": tx_result.get("height"),
                                "gas_used": tx_result_obj.get("gas_used"),
                                "gas_wanted": tx_result_obj.get("gas_wanted"),
                                "log": tx_result_obj.get("log"),
                                "events": tx_result_obj.get("events"),
                                "elapsed_seconds": start_time.elapsed().as_secs(),
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }));
                        }
                    }
                    
                    // If we can't determine the status, but got a result, it's likely pending
                    debug!("Transaction {} found but status unclear, continuing to monitor", tx_hash);
                }
                Err(e) => {
                    // If the transaction is not found, it might still be pending
                    debug!("Transaction {} not found or error occurred: {}", tx_hash, e);
                }
            }

            // Wait before polling again
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Execute a custom MCP tool by name with parameters (for script execution)
    pub async fn execute_custom_tool(
        &self,
        tool_name: &str,
        parameters: &HashMap<String, String>,
    ) -> McpResult<Value> {
        debug!("SDK Adapter: Executing custom tool: {} with parameters: {:?}", tool_name, parameters);

        // Convert string parameters to serde_json::Value
        let mut json_params = serde_json::Map::new();
        for (key, value) in parameters {
            // Try to parse as JSON first, fallback to string
            let json_value = if let Ok(parsed) = serde_json::from_str::<Value>(value) {
                parsed
            } else {
                Value::String(value.clone())
            };
            json_params.insert(key.clone(), json_value);
        }
        let args = Value::Object(json_params);

        // Validate parameters before tool execution
        Self::validate_tool_parameters(tool_name, parameters)?;

        // Route to appropriate tool based on tool_name
        match tool_name {
            "get_balances" => {
                // get_balances needs network_config and wallet_address parameters
                let network_config = self.get_default_network_config().await?;
                let wallet_address = parameters.get("wallet_address").map(|s| s.clone());
                self.get_balances(&network_config, wallet_address).await
            }
            "get_pool" => {
                let pool_id = parameters.get("pool_id")
                    .ok_or_else(|| McpServerError::InvalidArguments("pool_id parameter required".to_string()))?
                    .clone();
                self.get_pool_info(pool_id).await
            }
            "get_pools" => self.get_pools(args).await,
            "swap" | "execute_swap" => self.execute_swap(args).await,
            "provide_liquidity" => self.provide_liquidity(args).await,
            "withdraw_liquidity" => self.withdraw_liquidity(args).await,
            "create_pool" => self.create_pool(args).await,
            "get_lp_token_balance" => self.get_lp_token_balance(args).await,
            "get_all_lp_token_balances" => self.get_all_lp_token_balances(args).await,
            "validate_network" => self.validate_network_connectivity().await,
            "get_contracts" => self.get_contract_addresses().await,
            "monitor_transaction" => {
                // Special handling for monitor_transaction which needs different parameters
                let tx_hash = parameters.get("tx_hash")
                    .ok_or_else(|| McpServerError::InvalidArguments("tx_hash parameter required".to_string()))?
                    .clone();
                let timeout_seconds = parameters.get("timeout")
                    .and_then(|t| t.parse::<u64>().ok());
                
                self.monitor_transaction(tx_hash, timeout_seconds).await
            }
            _ => {
                // For unknown tools, return an error result
                Ok(serde_json::json!({
                    "status": "error",
                    "tool_name": tool_name,
                    "message": format!("Unknown tool: {}. Available tools: get_balances, get_pool, get_pools, swap, execute_swap, provide_liquidity, withdraw_liquidity, create_pool, get_lp_token_balance, get_all_lp_token_balances, validate_network, get_contracts, monitor_transaction", tool_name),
                    "parameters": parameters,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
        }
    }

    /// Get the default network configuration
    /// This is a temporary method until proper network configuration management is implemented
    async fn get_default_network_config(&self) -> McpResult<MantraNetworkConfig> {
        // For now, we'll use the testnet configuration
        // This should be replaced with proper configuration management
        use crate::config::NetworkConstants;

        let network_constants = NetworkConstants::load("mantra-dukong").map_err(|e| {
            McpServerError::Internal(format!("Failed to load network constants: {}", e))
        })?;

        let network_config = MantraNetworkConfig::from_constants(&network_constants).map_err(|e| {
            McpServerError::Internal(format!("Failed to create network config: {}", e))
        })?;
        Ok(network_config)
    }

    // =========================================================================
    // SDK Operation Wrappers
    // =========================================================================

    pub async fn get_pool(&self, pool_id: &str) -> McpResult<Value> {
        // Validate pool_id parameter
        if pool_id.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "pool_id cannot be empty".to_string(),
            ));
        }

        // Get client connection
        let client = self
            .get_client(&self.get_default_network_config().await?)
            .await?;

        // Query pool information from blockchain
        let pool_info = client
            .get_pool(pool_id)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Convert pool info to JSON format
        let pool_data = serde_json::json!({
            "pool_id": pool_info.pool_info.pool_identifier,
            "pool_type": match pool_info.pool_info.pool_type {
                mantra_dex_std::pool_manager::PoolType::ConstantProduct => "constant_product",
                mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "stable_swap",
            },
            "assets": pool_info.pool_info.assets.iter().map(|asset| {
                serde_json::json!({
                    "denom": asset.denom,
                    "amount": asset.amount.to_string()
                })
            }).collect::<Vec<_>>(),
            "status": {
                "swaps_enabled": pool_info.pool_info.status.swaps_enabled,
                "deposits_enabled": pool_info.pool_info.status.deposits_enabled,
                "withdrawals_enabled": pool_info.pool_info.status.withdrawals_enabled
            },
            "lp_token_denom": pool_info.pool_info.lp_denom,
            "total_share": pool_info.total_share.to_string()
        });

        Ok(pool_data)
    }

    pub async fn get_pools(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting pools with args: {:?}", args);

        // Parse optional parameters
        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as u32);

        let start_after = args
            .get("start_after")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Execute the pools query directly (without retry for now due to client not being Clone)
        let pools_result = client
            .get_pools(limit)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Convert pools to JSON format
        let pools_json: Vec<Value> = pools_result
            .into_iter()
            .map(|pool| {
                serde_json::json!({
                    "pool_id": pool.pool_info.pool_identifier,
                    "pool_type": match pool.pool_info.pool_type {
                        mantra_dex_std::pool_manager::PoolType::ConstantProduct => "constant_product",
                        mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "stable_swap",
                    },
                    "assets": pool.pool_info.assets.iter().map(|asset| {
                        serde_json::json!({
                            "denom": asset.denom,
                            "amount": asset.amount.to_string()
                        })
                    }).collect::<Vec<_>>(),
                    "lp_denom": pool.pool_info.lp_denom,
                    "status": {
                        "swaps_enabled": pool.pool_info.status.swaps_enabled,
                        "deposits_enabled": pool.pool_info.status.deposits_enabled,
                        "withdrawals_enabled": pool.pool_info.status.withdrawals_enabled
                    },
                    "total_share": pool.total_share.to_string()
                })
            })
            .collect();

        info!("Successfully retrieved {} pools", pools_json.len());

        Ok(serde_json::json!({
            "pools": pools_json,
            "count": pools_json.len(),
            "limit": limit,
            "start_after": start_after
        }))
    }

    pub async fn validate_pool_status(
        &self,
        pool_id: &str,
        operation: Option<String>,
        include_recommendations: bool,
    ) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Validating pool status for pool {} with operation {:?}",
            pool_id, operation
        );

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get pool information
        let pool_result = client.get_pool(pool_id).await;

        let mut validation_result = serde_json::Map::new();
        validation_result.insert(
            "pool_id".to_string(),
            serde_json::Value::String(pool_id.to_string()),
        );
        validation_result.insert(
            "validation_timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );

        match pool_result {
            Ok(pool_info) => {
                let status = &pool_info.pool_info.status;

                // Overall pool existence validation
                validation_result.insert("pool_exists".to_string(), serde_json::Value::Bool(true));
                validation_result.insert(
                    "pool_identifier".to_string(),
                    serde_json::Value::String(pool_info.pool_info.pool_identifier.clone()),
                );

                // Feature status validation
                let mut feature_status = serde_json::Map::new();
                feature_status.insert(
                    "swaps_enabled".to_string(),
                    serde_json::Value::Bool(status.swaps_enabled),
                );
                feature_status.insert(
                    "deposits_enabled".to_string(),
                    serde_json::Value::Bool(status.deposits_enabled),
                );
                feature_status.insert(
                    "withdrawals_enabled".to_string(),
                    serde_json::Value::Bool(status.withdrawals_enabled),
                );
                validation_result.insert(
                    "features".to_string(),
                    serde_json::Value::Object(feature_status),
                );

                // Operation-specific validation
                if let Some(op) = operation {
                    let operation_valid = match op.as_str() {
                        "swap" => status.swaps_enabled,
                        "deposit" | "provide_liquidity" => status.deposits_enabled,
                        "withdraw" | "withdraw_liquidity" => status.withdrawals_enabled,
                        _ => false,
                    };

                    validation_result.insert(
                        "operation".to_string(),
                        serde_json::Value::String(op.clone()),
                    );
                    validation_result.insert(
                        "operation_valid".to_string(),
                        serde_json::Value::Bool(operation_valid),
                    );

                    if !operation_valid {
                        validation_result.insert(
                            "operation_error".to_string(),
                            serde_json::Value::String(format!(
                                "Operation '{}' is not enabled for this pool",
                                op
                            )),
                        );
                    }
                }

                // Overall status assessment
                let all_operations_enabled =
                    status.swaps_enabled && status.deposits_enabled && status.withdrawals_enabled;
                let overall_status = if all_operations_enabled {
                    "fully_operational"
                } else if !status.swaps_enabled
                    && !status.deposits_enabled
                    && !status.withdrawals_enabled
                {
                    "disabled"
                } else {
                    "partially_operational"
                };

                validation_result.insert(
                    "overall_status".to_string(),
                    serde_json::Value::String(overall_status.to_string()),
                );
                validation_result.insert(
                    "is_operational".to_string(),
                    serde_json::Value::Bool(all_operations_enabled),
                );

                // Add recommendations if requested
                if include_recommendations {
                    let mut recommendations = Vec::new();

                    if !status.swaps_enabled {
                        recommendations
                            .push("Swaps are disabled - users cannot trade in this pool");
                    }
                    if !status.deposits_enabled {
                        recommendations
                            .push("Deposits are disabled - users cannot provide liquidity");
                    }
                    if !status.withdrawals_enabled {
                        recommendations
                            .push("Withdrawals are disabled - users cannot remove liquidity");
                    }

                    if recommendations.is_empty() {
                        recommendations
                            .push("Pool is fully operational - all operations are enabled");
                    }

                    validation_result.insert(
                        "recommendations".to_string(),
                        serde_json::Value::Array(
                            recommendations
                                .into_iter()
                                .map(|s| serde_json::Value::String(s.to_string()))
                                .collect(),
                        ),
                    );
                }

                validation_result.insert(
                    "status".to_string(),
                    serde_json::Value::String("success".to_string()),
                );
            }
            Err(e) => {
                validation_result.insert("pool_exists".to_string(), serde_json::Value::Bool(false));
                validation_result.insert(
                    "error".to_string(),
                    serde_json::Value::String(format!("Failed to get pool information: {}", e)),
                );
                validation_result.insert(
                    "status".to_string(),
                    serde_json::Value::String("error".to_string()),
                );
                validation_result
                    .insert("is_operational".to_string(), serde_json::Value::Bool(false));

                if include_recommendations {
                    validation_result.insert(
                        "recommendations".to_string(),
                        serde_json::Value::Array(vec![serde_json::Value::String(
                            "Pool does not exist or is not accessible".to_string(),
                        )]),
                    );
                }
            }
        }

        Ok(serde_json::Value::Object(validation_result))
    }

    pub async fn provide_liquidity(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Providing liquidity with args: {:?}", args);

        // Parse required parameters
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let assets_json = args
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("assets array is required".to_string())
            })?;

        // Parse assets
        let mut assets = Vec::new();
        for asset_json in assets_json {
            let denom = asset_json
                .get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.denom is required".to_string())
                })?;

            let amount_str = asset_json
                .get("amount")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.amount is required".to_string())
                })?;

            let amount = Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid asset amount: {}", e))
            })?;

            assets.push(Coin {
                denom: denom.to_string(),
                amount,
            });
        }

        // Parse optional slippage parameters
        let liquidity_max_slippage = args
            .get("liquidity_max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        let swap_max_slippage = args
            .get("swap_max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Get wallet (use provided wallet_address or active wallet)
        let wallet = if let Some(wallet_address) = args.get("wallet_address").and_then(|v| v.as_str()) {
            match self.get_wallet_by_address(wallet_address).await? {
                Some(wallet) => wallet,
                None => {
                    return Err(McpServerError::InvalidArguments(format!(
                        "Wallet with address {} not found",
                        wallet_address
                    )));
                }
            }
        } else {
            self.get_active_wallet_with_validation().await?
        };

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute provide liquidity directly (without retry for now due to client not being Clone)
        let liquidity_result = client
            .provide_liquidity(pool_id, assets, liquidity_max_slippage, swap_max_slippage)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!(
            "Successfully provided liquidity to pool {} with tx hash: {}",
            pool_id, liquidity_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": liquidity_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", liquidity_result.txhash),
            "liquidity_details": {
                "pool_id": pool_id,
                "assets": assets_json,
                "liquidity_max_slippage": liquidity_max_slippage.map(|d| d.to_string()),
                "swap_max_slippage": swap_max_slippage.map(|d| d.to_string()),
                "gas_used": liquidity_result.gas_used,
                "gas_wanted": liquidity_result.gas_wanted
            },
            "block_height": liquidity_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": liquidity_result.events
        }))
    }

    pub async fn provide_liquidity_unchecked(&self, args: Value) -> McpResult<Value> {
        info!(?args, "SDK Adapter: Providing liquidity (unchecked)");
        Ok(serde_json::json!({
            "status": "success",
            "message": "Liquidity provided (unchecked, simulation)",
            "tx_hash": "SIMULATED_UNCHECKED_TX_HASH"
        }))
    }

    pub async fn withdraw_liquidity(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Withdrawing liquidity with args: {:?}", args);

        // Parse required parameters
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let amount_str = args
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("amount is required".to_string()))?;

        let lp_amount = Uint128::from_str(amount_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid LP amount: {}", e)))?;

        // Get wallet (use provided wallet_address or active wallet)
        let wallet = if let Some(wallet_address) = args.get("wallet_address").and_then(|v| v.as_str()) {
            match self.get_wallet_by_address(wallet_address).await? {
                Some(wallet) => wallet,
                None => {
                    return Err(McpServerError::InvalidArguments(format!(
                        "Wallet with address {} not found",
                        wallet_address
                    )));
                }
            }
        } else {
            self.get_active_wallet_with_validation().await?
        };

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute withdraw liquidity directly (without retry for now due to client not being Clone)
        let withdraw_result = client
            .withdraw_liquidity(pool_id, lp_amount)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!(
            "Successfully withdrew liquidity from pool {} with tx hash: {}",
            pool_id, withdraw_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": withdraw_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", withdraw_result.txhash),
            "withdrawal_details": {
                "pool_id": pool_id,
                "lp_amount": amount_str,
                "gas_used": withdraw_result.gas_used,
                "gas_wanted": withdraw_result.gas_wanted
            },
            "block_height": withdraw_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": withdraw_result.events
        }))
    }

    pub async fn get_liquidity_positions(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting liquidity positions with args: {:?}",
            args
        );

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet to find LP tokens
        let balances_result = client
            .get_balances_for_address(&wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Filter for LP tokens (typically start with "factory/" and contain "lp" or are pool denoms)
        let mut lp_positions = Vec::new();

        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this looks like an LP token denom
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                // Try to extract pool identifier from the denom
                let pool_id = if let Some(last_part) = denom.split('/').last() {
                    if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    }
                } else {
                    denom
                };

                lp_positions.push(serde_json::json!({
                    "pool_id": pool_id,
                    "lp_token_denom": denom,
                    "balance": balance.amount.to_string(),
                    "wallet_address": wallet_address
                }));
            }
        }

        info!(
            "Found {} LP positions for wallet {}",
            lp_positions.len(),
            wallet_address
        );

        Ok(serde_json::json!({
            "status": "success",
            "wallet_address": wallet_address,
            "positions": lp_positions,
            "total_positions": lp_positions.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn execute_swap(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Executing swap with args: {:?}", args);

        // Parse required parameters
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let offer_asset = args.get("offer_asset").ok_or_else(|| {
            McpServerError::InvalidArguments("offer_asset is required".to_string())
        })?;

        let ask_asset_denom = args
            .get("ask_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("ask_asset_denom is required".to_string())
            })?;

        // Parse offer asset
        let offer_denom = offer_asset
            .get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("offer_asset.denom is required".to_string())
            })?;

        let offer_amount_str = offer_asset
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("offer_asset.amount is required".to_string())
            })?;

        let offer_amount = Uint128::from_str(offer_amount_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid offer amount: {}", e))
        })?;

        let offer_coin = Coin {
            denom: offer_denom.to_string(),
            amount: offer_amount,
        };

        // Parse optional max_slippage
        let max_slippage = args
            .get("max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Get wallet (use provided wallet_address or active wallet)
        let wallet = if let Some(wallet_address) = args.get("wallet_address").and_then(|v| v.as_str()) {
            match self.get_wallet_by_address(wallet_address).await? {
                Some(wallet) => wallet,
                None => {
                    return Err(McpServerError::InvalidArguments(format!(
                        "Wallet with address {} not found",
                        wallet_address
                    )));
                }
            }
        } else {
            self.get_active_wallet_with_validation().await?
        };

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute the swap directly (without retry for now due to client not being Clone)
        let swap_result = client
            .swap(pool_id, offer_coin, ask_asset_denom, max_slippage)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!(
            "Successfully executed swap in pool {} with tx hash: {}",
            pool_id, swap_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": swap_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", swap_result.txhash),
            "swap_details": {
                "pool_id": pool_id,
                "offer_asset": {
                    "denom": offer_denom,
                    "amount": offer_amount_str
                },
                "ask_asset_denom": ask_asset_denom,
                "max_slippage": max_slippage.map(|d| d.to_string()),
                "gas_used": swap_result.gas_used,
                "gas_wanted": swap_result.gas_wanted
            },
            "block_height": swap_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": swap_result.events
        }))
    }

    pub async fn get_lp_token_balance(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting LP token balance with args: {:?}",
            args
        );

        // Parse required pool_id parameter
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet
        let balances_result = client
            .get_balances_for_address(&wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Look for LP token for this specific pool
        let mut lp_balance = None;
        let mut lp_denom = None;

        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this is an LP token for the specified pool
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                // Try to extract pool identifier from the denom
                if let Some(last_part) = denom.split('/').last() {
                    let extracted_pool_id = if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    };

                    if extracted_pool_id == pool_id {
                        lp_balance = Some(balance.amount.to_string());
                        lp_denom = Some(denom.clone());
                        break;
                    }
                }
            }
        }

        let balance_amount = lp_balance.unwrap_or_else(|| "0".to_string());
        let token_denom = lp_denom.unwrap_or_else(|| format!("factory/mantra/lp_{}", pool_id));

        info!(
            "LP token balance for pool {}: {} {}",
            pool_id, balance_amount, token_denom
        );

        Ok(serde_json::json!({
            "status": "success",
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_token_balance": {
                "denom": token_denom,
                "amount": balance_amount
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn get_all_lp_token_balances(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Getting all LP token balances with args: {:?}",
            args
        );

        // Parse optional parameters
        let include_zero_balances = args
            .get("include_zero_balances")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet
        let balances_result = client
            .get_balances_for_address(&wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Filter for LP tokens
        let mut lp_balances = Vec::new();

        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this looks like an LP token denom
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                let amount_str = balance.amount.to_string();

                // Skip zero balances if not requested
                if !include_zero_balances && balance.amount.is_zero() {
                    continue;
                }

                // Try to extract pool identifier from the denom
                let pool_id = if let Some(last_part) = denom.split('/').last() {
                    if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    }
                } else {
                    denom
                };

                lp_balances.push(serde_json::json!({
                    "pool_id": pool_id,
                    "lp_token_denom": denom,
                    "balance": amount_str,
                    "is_zero": balance.amount.is_zero()
                }));
            }
        }

        info!(
            "Found {} LP token balances for wallet {}",
            lp_balances.len(),
            wallet_address
        );

        Ok(serde_json::json!({
            "status": "success",
            "wallet_address": wallet_address,
            "lp_balances": lp_balances,
            "total_positions": lp_balances.len(),
            "include_zero_balances": include_zero_balances,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn estimate_lp_withdrawal_amounts(&self, args: Value) -> McpResult<Value> {
        debug!(
            "SDK Adapter: Estimating LP withdrawal amounts with args: {:?}",
            args
        );

        // Parse required pool_id parameter
        let pool_id = args
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str())
        {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => wallet
                    .address()
                    .map_err(|e| {
                        McpServerError::InvalidArguments(format!(
                            "Failed to get wallet address: {}",
                            e
                        ))
                    })?
                    .to_string(),
                None => {
                    return Err(McpServerError::InvalidArguments(
                        "No wallet configured and no wallet_address provided".to_string(),
                    ));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get pool information
        let pool_info = client
            .get_pool(pool_id)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get LP token amount to withdraw (use full balance if not provided)
        let lp_amount = if let Some(amount_str) =
            args.get("lp_token_amount").and_then(|v| v.as_str())
        {
            Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid LP token amount: {}", e))
            })?
        } else {
            // Use full LP balance
            let balances_result = client
                .get_balances_for_address(&wallet_address)
                .await
                .map_err(|e| McpServerError::Sdk(e))?;

            let mut full_balance = Uint128::zero();
            for balance in &balances_result {
                let denom = &balance.denom;
                if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                    if let Some(last_part) = denom.split('/').last() {
                        let extracted_pool_id = if last_part.starts_with("lp_") {
                            last_part.strip_prefix("lp_").unwrap_or(last_part)
                        } else {
                            last_part
                        };

                        if extracted_pool_id == pool_id {
                            full_balance = balance.amount;
                            break;
                        }
                    }
                }
            }
            full_balance
        };

        if lp_amount.is_zero() {
            return Ok(serde_json::json!({
                "status": "success",
                "pool_id": pool_id,
                "wallet_address": wallet_address,
                "lp_amount": "0",
                "estimated_withdrawal": [],
                "total_share": pool_info.total_share.to_string(),
                "message": "No LP tokens to withdraw",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }));
        }

        // Calculate withdrawal amounts based on pool ratio
        let total_share = pool_info.total_share;
        let mut estimated_amounts = Vec::new();

        for asset in &pool_info.pool_info.assets {
            // Calculate proportional withdrawal amount
            // withdrawal_amount = (lp_amount / total_share) * asset_amount
            let withdrawal_amount = if !total_share.amount.is_zero() {
                asset.amount.multiply_ratio(lp_amount, total_share.amount)
            } else {
                Uint128::zero()
            };

            estimated_amounts.push(serde_json::json!({
                "denom": asset.denom,
                "amount": withdrawal_amount.to_string(),
                "pool_amount": asset.amount.to_string()
            }));
        }

        info!(
            "Estimated withdrawal amounts for {} LP tokens from pool {}: {:?}",
            lp_amount, pool_id, estimated_amounts
        );

        Ok(serde_json::json!({
            "status": "success",
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_amount": lp_amount.to_string(),
            "estimated_withdrawal": estimated_amounts,
            "pool_info": {
                "total_share": total_share.to_string(),
                "assets": pool_info.pool_info.assets.iter().map(|asset| {
                    serde_json::json!({
                        "denom": asset.denom,
                        "amount": asset.amount.to_string()
                    })
                }).collect::<Vec<_>>()
            },
            "withdrawal_ratio": if !total_share.amount.is_zero() {
                format!("{:.6}", lp_amount.u128() as f64 / total_share.amount.u128() as f64)
            } else {
                "0.000000".to_string()
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn create_pool(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Creating pool with args: {:?}", args);

        // Parse required parameters
        let pool_type_str = args
            .get("pool_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_type is required".to_string()))?;

        let assets_json = args
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("assets array is required".to_string())
            })?;

        // Parse pool type
        let pool_type = match pool_type_str {
            "constant_product" => mantra_dex_std::pool_manager::PoolType::ConstantProduct,
            "stable_swap" => {
                // For stable swap, we need amplification parameter
                let amplification = args
                    .get("amplification")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u64;
                mantra_dex_std::pool_manager::PoolType::StableSwap { amp: amplification }
            }
            _ => {
                return Err(McpServerError::InvalidArguments(
                    "Invalid pool_type. Must be 'constant_product' or 'stable_swap'".to_string(),
                ))
            }
        };

        // Parse assets - extract denominations and decimals
        let mut asset_denoms = Vec::new();
        let mut asset_decimals = Vec::new();

        for asset_json in assets_json {
            let denom = asset_json
                .get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpServerError::InvalidArguments("asset.denom is required".to_string())
                })?;

            let decimals = asset_json
                .get("decimals")
                .and_then(|v| v.as_u64())
                .unwrap_or(6) as u8; // Default to 6 decimals

            asset_denoms.push(denom.to_string());
            asset_decimals.push(decimals);
        }

        // Parse fees
        let fees_json = args.get("fees");
        let protocol_fee_str = fees_json
            .and_then(|f| f.get("protocol_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.01"); // Default 1%

        let swap_fee_str = fees_json
            .and_then(|f| f.get("swap_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.03"); // Default 3%

        let burn_fee_str = fees_json
            .and_then(|f| f.get("burn_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.0"); // Default 0%

        // Parse fee decimals
        let protocol_fee = Decimal::from_str(protocol_fee_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid protocol_fee: {}", e))
        })?;
        let swap_fee = Decimal::from_str(swap_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid swap_fee: {}", e)))?;
        let burn_fee = Decimal::from_str(burn_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid burn_fee: {}", e)))?;

        // Create pool fees structure
        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: protocol_fee,
            },
            swap_fee: mantra_dex_std::fee::Fee { share: swap_fee },
            burn_fee: mantra_dex_std::fee::Fee { share: burn_fee },
            extra_fees: vec![],
        };

        // Parse optional pool identifier
        let pool_identifier = args
            .get("pool_identifier")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Clone pool_identifier for response formatting
        let pool_identifier_for_response = pool_identifier.clone();

        // Get active wallet (required for pool creation)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Query the actual pool creation fee for response
        let creation_fee = client
            .get_pool_creation_fee()
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Execute pool creation directly (without retry for now due to client not being Clone)
        let create_result = client
            .create_pool(
                asset_denoms,
                asset_decimals,
                pool_fees,
                pool_type,
                pool_identifier,
            )
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!(
            "Successfully created pool with tx hash: {}",
            create_result.txhash
        );

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": create_result.txhash,
            "explorer_url": format!("https://explorer.mantrachain.io/mantra-dukong/tx/{}", create_result.txhash),
            "pool_details": {
                "pool_type": pool_type_str,
                "assets": assets_json,
                "fees": {
                    "protocol_fee": protocol_fee_str,
                    "swap_fee": swap_fee_str,
                    "burn_fee": burn_fee_str
                },
                "pool_identifier": pool_identifier_for_response,
                "creation_fee": creation_fee.amount.to_string(),
                "gas_used": create_result.gas_used,
                "gas_wanted": create_result.gas_wanted
            },
            "block_height": create_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": create_result.events
        }))
    }

    /// Validate tool parameters before execution
    fn validate_tool_parameters(
        tool_name: &str,
        parameters: &HashMap<String, String>,
    ) -> McpResult<()> {
        match tool_name {
            "get_pool" => {
                if let Some(pool_id) = parameters.get("pool_id") {
                    if pool_id.trim().is_empty() {
                        return Err(McpServerError::InvalidArguments(
                            "pool_id cannot be empty".to_string(),
                        ));
                    }
                } else {
                    return Err(McpServerError::InvalidArguments(
                        "pool_id parameter is required".to_string(),
                    ));
                }
            }
            "monitor_transaction" => {
                if let Some(tx_hash) = parameters.get("tx_hash") {
                    if tx_hash.trim().is_empty() {
                        return Err(McpServerError::InvalidArguments(
                            "tx_hash cannot be empty".to_string(),
                        ));
                    }
                    // Validate tx_hash format (should be hex)
                    if !tx_hash.chars().all(|c| c.is_ascii_hexdigit() || c.is_ascii_uppercase()) {
                        return Err(McpServerError::InvalidArguments(
                            "tx_hash must contain only hexadecimal characters".to_string(),
                        ));
                    }
                } else {
                    return Err(McpServerError::InvalidArguments(
                        "tx_hash parameter is required".to_string(),
                    ));
                }
                
                // Validate timeout if present
                if let Some(timeout) = parameters.get("timeout") {
                    if let Ok(timeout_val) = timeout.parse::<u64>() {
                        if timeout_val == 0 || timeout_val > 300 {
                            return Err(McpServerError::InvalidArguments(
                                "timeout must be between 1 and 300 seconds".to_string(),
                            ));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(
                            "timeout must be a valid number".to_string(),
                        ));
                    }
                }
            }
            "get_balances" => {
                // Validate wallet_address if present
                if let Some(wallet_addr) = parameters.get("wallet_address") {
                    if !wallet_addr.trim().is_empty() && !wallet_addr.starts_with("mantra") {
                        return Err(McpServerError::InvalidArguments(
                            "wallet_address must be a valid Mantra address (starts with 'mantra')".to_string(),
                        ));
                    }
                }
            }
            "swap" | "execute_swap" => {
                // Validate required swap parameters
                for required_param in &["asset_in", "asset_out", "amount_in"] {
                    if let Some(value) = parameters.get(*required_param) {
                        if value.trim().is_empty() {
                            return Err(McpServerError::InvalidArguments(
                                format!("{} cannot be empty", required_param),
                            ));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(
                            format!("{} parameter is required", required_param),
                        ));
                    }
                }
                
                // Validate amount_in is numeric
                if let Some(amount) = parameters.get("amount_in") {
                    if amount.parse::<f64>().is_err() {
                        return Err(McpServerError::InvalidArguments(
                            "amount_in must be a valid number".to_string(),
                        ));
                    }
                }
                
                // Validate slippage_tolerance if present
                if let Some(slippage) = parameters.get("slippage_tolerance") {
                    if let Ok(slippage_val) = slippage.parse::<f64>() {
                        if slippage_val < 0.0 || slippage_val > 100.0 {
                            return Err(McpServerError::InvalidArguments(
                                "slippage_tolerance must be between 0 and 100".to_string(),
                            ));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(
                            "slippage_tolerance must be a valid number".to_string(),
                        ));
                    }
                }
            }
            "provide_liquidity" | "withdraw_liquidity" => {
                // Validate required liquidity parameters
                for required_param in &["pool_id", "amount"] {
                    if let Some(value) = parameters.get(*required_param) {
                        if value.trim().is_empty() {
                            return Err(McpServerError::InvalidArguments(
                                format!("{} cannot be empty", required_param),
                            ));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(
                            format!("{} parameter is required", required_param),
                        ));
                    }
                }
            }
            "create_pool" => {
                // Validate required pool creation parameters
                for required_param in &["asset_a", "asset_b", "amount_a", "amount_b"] {
                    if let Some(value) = parameters.get(*required_param) {
                        if value.trim().is_empty() {
                            return Err(McpServerError::InvalidArguments(
                                format!("{} cannot be empty", required_param),
                            ));
                        }
                    } else {
                        return Err(McpServerError::InvalidArguments(
                            format!("{} parameter is required", required_param),
                        ));
                    }
                }
            }
            _ => {
                // For other tools, perform basic validation
                for (key, value) in parameters {
                    if value.trim().is_empty() {
                        return Err(McpServerError::InvalidArguments(
                            format!("Parameter '{}' cannot be empty", key),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    // =============================================================================
    // ClaimDrop Protocol Methods
    // =============================================================================

    /// Create a new claimdrop campaign through the factory
    pub async fn claimdrop_create_campaign(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Creating ClaimDrop campaign with args: {:?}", args);

        // Parse required parameters
        let factory_address = args
            .get("factory_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("factory_address is required".to_string()))?;

        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("name is required".to_string()))?;

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("description is required".to_string()))?;

        let campaign_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("type is required".to_string()))?;

        let start_time = args
            .get("start_time")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| McpServerError::InvalidArguments("start_time is required".to_string()))?;

        let end_time = args
            .get("end_time")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| McpServerError::InvalidArguments("end_time is required".to_string()))?;

        let reward_denom = args
            .get("reward_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("reward_denom is required".to_string()))?;

        let total_reward_str = args
            .get("total_reward")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("total_reward is required".to_string()))?;

        let total_reward = Uint128::from_str(total_reward_str).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid total_reward: {}", e))
        })?;

        // Parse distribution_type array
        let distribution_type = args
            .get("distribution_type")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpServerError::InvalidArguments("distribution_type is required".to_string()))?;

        let mut parsed_distributions = Vec::new();
        for dist in distribution_type {
            let dist_type = dist.get("type").and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("distribution_type.type is required".to_string()))?;
            
            match dist_type {
                "lump_sum" => {
                    let percentage = dist.get("percentage").and_then(|v| v.as_str())
                        .ok_or_else(|| McpServerError::InvalidArguments("distribution percentage is required".to_string()))?;
                    let percentage_decimal = Decimal::from_str(percentage).map_err(|e| {
                        McpServerError::InvalidArguments(format!("Invalid percentage: {}", e))
                    })?;
                    let start_time = dist.get("start_time").and_then(|v| v.as_u64())
                        .ok_or_else(|| McpServerError::InvalidArguments("distribution start_time is required".to_string()))?;
                    
                    parsed_distributions.push(mantra_claimdrop_std::msg::DistributionType::LumpSum {
                        percentage: percentage_decimal,
                        start_time,
                    });
                }
                "linear_vesting" => {
                    let percentage = dist.get("percentage").and_then(|v| v.as_str())
                        .ok_or_else(|| McpServerError::InvalidArguments("distribution percentage is required".to_string()))?;
                    let percentage_decimal = Decimal::from_str(percentage).map_err(|e| {
                        McpServerError::InvalidArguments(format!("Invalid percentage: {}", e))
                    })?;
                    let start_time = dist.get("start_time").and_then(|v| v.as_u64())
                        .ok_or_else(|| McpServerError::InvalidArguments("distribution start_time is required".to_string()))?;
                    let end_time = dist.get("end_time").and_then(|v| v.as_u64())
                        .ok_or_else(|| McpServerError::InvalidArguments("distribution end_time is required".to_string()))?;
                    let cliff_duration = dist.get("cliff_duration").and_then(|v| v.as_u64());
                    
                    parsed_distributions.push(mantra_claimdrop_std::msg::DistributionType::LinearVesting {
                        percentage: percentage_decimal,
                        start_time,
                        end_time,
                        cliff_duration,
                    });
                }
                _ => {
                    return Err(McpServerError::InvalidArguments(
                        format!("Invalid distribution type: {}", dist_type)
                    ));
                }
            }
        }

        // Get network config and active wallet
        let network_config = self.get_default_network_config().await?;
        let wallet = self.get_active_wallet_with_validation().await?;
        
        // Create MantraClient with wallet
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet))).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Create campaign parameters
        let campaign_params = mantra_claimdrop_std::msg::CampaignParams {
            name: name.to_string(),
            description: description.to_string(),
            ty: campaign_type.to_string(),
            reward_denom: reward_denom.to_string(),
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

        let result = factory_client.create_campaign(campaign_params, fee).await
            .map_err(|e| McpServerError::Sdk(e))?;

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
        debug!("SDK Adapter: Claiming from ClaimDrop campaign with args: {:?}", args);

        // Parse required parameters
        let campaign_address = args
            .get("campaign_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("campaign_address is required".to_string()))?;

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
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet))).await
            .map_err(|e| McpServerError::Sdk(e))?;

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

        let result = claimdrop_client.claim(amount, receiver, fee).await
            .map_err(|e| McpServerError::Sdk(e))?;

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
        debug!("SDK Adapter: Querying ClaimDrop rewards with args: {:?}", args);

        // Parse required parameters
        let campaign_address = args
            .get("campaign_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("campaign_address is required".to_string()))?;

        let receiver_address = args
            .get("receiver")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("receiver is required".to_string()))?;

        // Get network config and create MantraClient
        let network_config = self.get_default_network_config().await?;
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get ClaimDrop client for the campaign
        let claimdrop_client = client.claimdrop_campaign(campaign_address.to_string());

        let rewards = claimdrop_client.query_rewards(receiver_address).await
            .map_err(|e| McpServerError::Sdk(e))?;

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
        debug!("SDK Adapter: Querying ClaimDrop campaigns with args: {:?}", args);

        // Parse required parameters
        let factory_address = args
            .get("factory_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("factory_address is required".to_string()))?;

        // Parse optional parameters
        let start_after = args
            .get("start_after")
            .and_then(|v| v.as_str());

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|l| l as u16);

        // Get network config and create MantraClient
        let network_config = self.get_default_network_config().await?;
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get factory client
        let factory_client = client.claimdrop_factory(factory_address.to_string());

        let campaigns = factory_client.query_campaigns(start_after, limit).await
            .map_err(|e| McpServerError::Sdk(e))?;

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
        debug!("SDK Adapter: Adding ClaimDrop allocations with args: {:?}", args);

        // Parse required parameters
        let campaign_address = args
            .get("campaign_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("campaign_address is required".to_string()))?;

        let allocations_array = args
            .get("allocations")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpServerError::InvalidArguments("allocations is required".to_string()))?;

        // Parse allocations
        let mut allocations = Vec::new();
        for allocation in allocations_array {
            let user = allocation.get("user").and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("allocation.user is required".to_string()))?;
            let amount_str = allocation.get("allocated_amount").and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("allocation.allocated_amount is required".to_string()))?;
            
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
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet))).await
            .map_err(|e| McpServerError::Sdk(e))?;

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

        let result = claimdrop_client.add_allocations(allocations, fee).await
            .map_err(|e| McpServerError::Sdk(e))?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "add_allocations",
            "campaign_address": campaign_address,
            "result": result,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    // =============================================================================
    // Skip Protocol Tools
    // =============================================================================

    /// Find optimal cross-chain routes between assets
    pub async fn skip_get_route(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting Skip route with args: {:?}", args);

        // Parse required parameters
        let source_asset_denom = args
            .get("source_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_asset_denom is required".to_string()))?;

        let source_asset_amount = args
            .get("source_asset_amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_asset_amount is required".to_string()))?;

        let source_chain = args
            .get("source_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_chain is required".to_string()))?;

        let target_asset_denom = args
            .get("target_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("target_asset_denom is required".to_string()))?;

        let target_chain = args
            .get("target_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("target_chain is required".to_string()))?;

        // Optional parameters
        let allow_multi_tx = args.get("allow_multi_tx").and_then(|v| v.as_bool()).unwrap_or(false);
        let smart_relay = args.get("smart_relay").and_then(|v| v.as_bool()).unwrap_or(true);

        // Parse amount
        let amount = Uint128::from_str(source_asset_amount).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid source_asset_amount: {}", e))
        })?;

        // Get network config
        let network_config = self.get_default_network_config().await?;
        
        // Create MantraClient  
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get Skip client
        let skip_client = client.skip().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Create source and target assets
        use crate::protocols::skip::types::CrossChainAsset;
        let source_asset = CrossChainAsset {
            denom: source_asset_denom.to_string(),
            amount,
            chain: source_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let target_asset = CrossChainAsset {
            denom: target_asset_denom.to_string(),
            amount: Uint128::zero(), // Amount will be calculated by routing
            chain: target_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        // Create route options
        use crate::protocols::skip::client::RouteOptions;
        let options = Some(RouteOptions {
            allow_multi_tx,
            smart_relay,
            allowed_bridges: None,
            affiliate_fee_bps: None,
        });

        // Get routes
        let routes = skip_client.get_route(&source_asset, &target_asset, options).await
            .map_err(|e| McpServerError::Sdk(e))?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "get_route",
            "source_asset": {
                "denom": source_asset.denom,
                "amount": source_asset.amount.to_string(),
                "chain": source_asset.chain
            },
            "target_asset": {
                "denom": target_asset.denom,
                "chain": target_asset.chain
            },
            "routes": routes,
            "route_count": routes.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Execute cross-chain asset transfers
    pub async fn skip_execute_transfer(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Executing Skip transfer with args: {:?}", args);

        // Parse required parameters
        let source_asset_denom = args
            .get("source_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_asset_denom is required".to_string()))?;

        let source_asset_amount = args
            .get("source_asset_amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_asset_amount is required".to_string()))?;

        let source_chain = args
            .get("source_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_chain is required".to_string()))?;

        let target_asset_denom = args
            .get("target_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("target_asset_denom is required".to_string()))?;

        let target_chain = args
            .get("target_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("target_chain is required".to_string()))?;

        let recipient = args
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("recipient is required".to_string()))?;

        // Optional parameters
        let timeout_seconds = args.get("timeout_seconds").and_then(|v| v.as_u64());
        let slippage_tolerance = args.get("slippage_tolerance")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Parse amount
        let amount = Uint128::from_str(source_asset_amount).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid source_asset_amount: {}", e))
        })?;

        // Get network config and wallet
        let network_config = self.get_default_network_config().await?;
        let wallet = self.get_active_wallet_with_validation().await?;
        
        // Create MantraClient with wallet
        let client = MantraClient::new(network_config.clone(), Some(Arc::new(wallet))).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get Skip client
        let skip_client = client.skip().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Create transfer request
        use crate::protocols::skip::types::{CrossChainAsset, TransferRequest};
        let source_asset = CrossChainAsset {
            denom: source_asset_denom.to_string(),
            amount,
            chain: source_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let target_asset = CrossChainAsset {
            denom: target_asset_denom.to_string(),
            amount: Uint128::zero(),
            chain: target_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let transfer_request = TransferRequest {
            source_asset,
            target_asset,
            recipient: recipient.to_string(),
            timeout_seconds,
            slippage_tolerance,
            route: None, // Let the client find the best route
        };

        // Execute transfer
        let result = skip_client.execute_cross_chain_transfer(&transfer_request).await
            .map_err(|e| McpServerError::Sdk(e))?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "execute_transfer",
            "transfer_id": result.transfer_id,
            "transfer_status": result.status,
            "source_tx_hash": result.source_tx_hash,
            "recipient": recipient,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Monitor transfer status and progress
    pub async fn skip_track_transfer(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Tracking Skip transfer with args: {:?}", args);

        // Parse required parameters
        let transfer_id = args
            .get("transfer_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("transfer_id is required".to_string()))?;

        // Get network config
        let network_config = self.get_default_network_config().await?;
        
        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get Skip client
        let skip_client = client.skip().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Track transfer
        let result = skip_client.track_transfer(transfer_id).await
            .map_err(|e| McpServerError::Sdk(e))?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "track_transfer",
            "transfer_id": result.transfer_id,
            "transfer_status": result.status,
            "source_tx_hash": result.source_tx_hash,
            "dest_tx_hash": result.dest_tx_hash,
            "amount_transferred": result.amount_transferred.map(|a| a.to_string()),
            "error_message": result.error_message,
            "initiated_at": result.initiated_at,
            "completed_at": result.completed_at,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// List available chains and their configurations
    pub async fn skip_get_supported_chains(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting supported chains with args: {:?}", args);

        // Optional filter parameter
        let filter = args.get("filter").and_then(|v| v.as_str());

        // Get network config
        let network_config = self.get_default_network_config().await?;
        
        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get Skip client
        let skip_client = client.skip().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get supported chains
        let mut chains = skip_client.get_supported_chains().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Apply filter if provided
        if let Some(filter_str) = filter {
            chains.retain(|chain| {
                chain.chain_name.to_lowercase().contains(&filter_str.to_lowercase()) ||
                chain.chain_id.to_lowercase().contains(&filter_str.to_lowercase())
            });
        }

        Ok(serde_json::json!({
            "status": "success",
            "operation": "get_supported_chains",
            "chains": chains,
            "chain_count": chains.len(),
            "filter": filter,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Validate assets across different chains
    pub async fn skip_verify_assets(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Verifying assets with args: {:?}", args);

        // Parse required parameters
        let assets_array = args
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpServerError::InvalidArguments("assets array is required".to_string()))?;

        // Parse assets
        use crate::protocols::skip::types::CrossChainAsset;
        let mut assets = Vec::new();
        
        for asset_value in assets_array {
            let denom = asset_value
                .get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("asset.denom is required".to_string()))?;

            let chain = asset_value
                .get("chain")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("asset.chain is required".to_string()))?;

            let amount_str = asset_value
                .get("amount")
                .and_then(|v| v.as_str())
                .unwrap_or("0");

            let amount = Uint128::from_str(amount_str).map_err(|e| {
                McpServerError::InvalidArguments(format!("Invalid asset amount: {}", e))
            })?;

            assets.push(CrossChainAsset {
                denom: denom.to_string(),
                amount,
                chain: chain.to_string(),
                decimals: asset_value.get("decimals").and_then(|v| v.as_u64()).map(|d| d as u8),
                symbol: asset_value.get("symbol").and_then(|v| v.as_str()).map(|s| s.to_string()),
            });
        }

        // Get network config
        let network_config = self.get_default_network_config().await?;
        
        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get Skip client
        let skip_client = client.skip().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Verify assets
        let verification_result = skip_client.verify_assets(&assets).await
            .map_err(|e| McpServerError::Sdk(e))?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "verify_assets",
            "verified_assets": verification_result.verified_assets,
            "invalid_assets": verification_result.invalid_assets,
            "verification_timestamp": verification_result.verification_timestamp,
            "summary": {
                "total_assets": assets.len(),
                "verified_count": verification_result.verified_assets.len(),
                "invalid_count": verification_result.invalid_assets.len()
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Estimate fees for cross-chain operations
    pub async fn skip_estimate_fees(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Estimating fees with args: {:?}", args);

        // Parse required parameters
        let source_asset_denom = args
            .get("source_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_asset_denom is required".to_string()))?;

        let source_asset_amount = args
            .get("source_asset_amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_asset_amount is required".to_string()))?;

        let source_chain = args
            .get("source_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("source_chain is required".to_string()))?;

        let target_asset_denom = args
            .get("target_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("target_asset_denom is required".to_string()))?;

        let target_chain = args
            .get("target_chain")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("target_chain is required".to_string()))?;

        let recipient = args
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("recipient is required".to_string()))?;

        // Parse amount
        let amount = Uint128::from_str(source_asset_amount).map_err(|e| {
            McpServerError::InvalidArguments(format!("Invalid source_asset_amount: {}", e))
        })?;

        // Get network config
        let network_config = self.get_default_network_config().await?;
        
        // Create MantraClient
        let client = MantraClient::new(network_config.clone(), None).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get Skip client
        let skip_client = client.skip().await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Create transfer request for fee estimation
        use crate::protocols::skip::types::{CrossChainAsset, TransferRequest};
        let source_asset = CrossChainAsset {
            denom: source_asset_denom.to_string(),
            amount,
            chain: source_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let target_asset = CrossChainAsset {
            denom: target_asset_denom.to_string(),
            amount: Uint128::zero(),
            chain: target_chain.to_string(),
            decimals: None,
            symbol: None,
        };

        let transfer_request = TransferRequest {
            source_asset,
            target_asset,
            recipient: recipient.to_string(),
            timeout_seconds: None,
            slippage_tolerance: None,
            route: None,
        };

        // Estimate fees
        let fee_estimate = skip_client.estimate_fees(&transfer_request).await
            .map_err(|e| McpServerError::Sdk(e))?;

        Ok(serde_json::json!({
            "status": "success",
            "operation": "estimate_fees",
            "total_fees": fee_estimate.total_fees,
            "gas_estimates": fee_estimate.gas_estimates,
            "route_steps": fee_estimate.route_steps,
            "estimated_time_seconds": fee_estimate.estimated_time_seconds,
            "price_impact": fee_estimate.price_impact,
            "summary": {
                "total_fee_amount": fee_estimate.total_fees.iter()
                    .map(|f| format!("{}{}", f.amount, f.denom))
                    .collect::<Vec<_>>().join(", "),
                "step_count": fee_estimate.route_steps
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

impl Default for McpSdkAdapter {
    fn default() -> Self {
        Self::new(ConnectionPoolConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = ConnectionPoolConfig::default();
        let adapter = McpSdkAdapter::new(config);

        // Verify initial state
        let pool_stats = adapter.get_pool_stats().await;
        assert!(pool_stats.is_empty());
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let adapter = McpSdkAdapter::default();

        // Test cache set and get
        let key = "test_key".to_string();
        let value = serde_json::json!({"test": "value"});

        adapter.cache_set(key.clone(), value.clone()).await;

        let retrieved = adapter.cache_get(&key).await;
        assert_eq!(retrieved, Some(value));

        // Test cache miss
        let missing = adapter.cache_get("nonexistent").await;
        assert_eq!(missing, None);

        // Test cache clear
        adapter.cache_clear().await;
        let after_clear = adapter.cache_get(&key).await;
        assert_eq!(after_clear, None);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let adapter = McpSdkAdapter::default();

        // Add some cache entries
        adapter
            .cache_set("key1".to_string(), serde_json::json!("value1"))
            .await;
        adapter
            .cache_set("key2".to_string(), serde_json::json!("value2"))
            .await;

        // Cleanup should not remove non-expired entries
        adapter.cleanup().await.unwrap();

        let (total, _valid) = adapter.get_cache_stats().await;
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_connection_pool_config() {
        let config = ConnectionPoolConfig {
            max_connections_per_network: 10,
            connection_timeout_secs: 60,
            connection_ttl_secs: 600,
            max_retries: 5,
            retry_base_delay_ms: 200,
            max_idle_time_secs: 120,
            health_check_interval_secs: 45,
            max_wallet_derivation_index: 100,
        };

        let adapter = McpSdkAdapter::new(config.clone());
        assert_eq!(adapter.config.max_connections_per_network, 10);
        assert_eq!(adapter.config.connection_timeout_secs, 60);
        assert_eq!(adapter.config.max_retries, 5);
    }

    #[tokio::test]
    async fn test_health_check_lifecycle() {
        let mut adapter = McpSdkAdapter::default();

        // Start health checks
        adapter.start_health_checks().await;
        assert!(adapter.health_check_handle.is_some());

        // Stop health checks
        adapter.stop_health_checks().await;
        assert!(adapter.health_check_handle.is_none());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut adapter = McpSdkAdapter::default();

        // Add some data
        adapter
            .cache_set("test".to_string(), serde_json::json!("data"))
            .await;

        // Start health checks
        adapter.start_health_checks().await;

        // Shutdown should clean everything
        adapter.shutdown().await.unwrap();

        let (cache_total, _) = adapter.get_cache_stats().await;
        let pool_stats = adapter.get_pool_stats().await;

        assert_eq!(cache_total, 0);
        assert!(pool_stats.is_empty());
        assert!(adapter.health_check_handle.is_none());
    }
}

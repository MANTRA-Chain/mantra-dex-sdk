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

use serde::Serialize;
use serde_json::Value;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::client::MantraClient;
use crate::config::MantraNetworkConfig;
use crate::protocols::dex::MantraDexClient;
use crate::protocols::evm::client::EvmClient;
use crate::protocols::evm::contracts::Erc20 as SdkErc20;
use crate::wallet::{MantraWallet, MultiVMWallet, WalletInfo};
use alloy_primitives::{Address, U256};

use super::erc20_registry::{Erc20Registry, Erc20TokenInfo, TokenSource};

use super::server::{McpResult, McpServerError};

// Module declarations - methods are added to McpSdkAdapter via impl blocks
mod claimdrop;
mod dex;
#[cfg(feature = "evm")]
mod evm;
mod network;
mod skip;
mod wallet;

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
            max_idle_time_secs: 60,           // 1 minute
            health_check_interval_secs: 30,   // 30 seconds
            max_wallet_derivation_index: 100, // Search up to index 100
        }
    }
}

// =============================================================================
// EVM Transaction Constants
// =============================================================================

/// Initial gas limit used as placeholder during gas estimation.
///
/// This value is used in `build_sign_and_broadcast_transaction()` when
/// building the transaction for gas estimation. The RPC node simulates
/// the transaction with this limit to calculate the actual gas needed.
///
/// **Important:** This is NOT the final gas limit - it's just a placeholder.
/// The actual gas limit is determined by the estimation result plus a buffer
/// (GAS_BUFFER_SIMPLE_PERCENT or GAS_BUFFER_COMPLEX_PERCENT).
///
/// **Why 300,000:**
/// - PrimarySale `invest()`: ~150k gas
/// - PrimarySale `settle()`: ~220k base gas
/// - Provides 2x safety margin for investment operations
/// - Still only 1% of Ethereum block gas limit (30M)
///
/// If operations consistently fail with "gas required exceeds allowance",
/// this constant may need to be increased further.
#[cfg(feature = "evm")]
pub(crate) const GAS_ESTIMATE_INITIAL: u64 = 300_000;

/// Gas buffer percentage for simple operations (transfers, approvals, simple admin calls).
/// Adds 20% to estimated gas to account for block state changes.
#[cfg(feature = "evm")]
pub(crate) const GAS_BUFFER_SIMPLE_PERCENT: u64 = 20;

/// Gas buffer percentage for complex operations (settlement, multi-step transactions).
/// Adds 30% to estimated gas to account for higher variability.
#[cfg(feature = "evm")]
pub(crate) const GAS_BUFFER_COMPLEX_PERCENT: u64 = 30;

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
pub(crate) struct NetworkConnectionPool {
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
        for pooled_conn in self.connections.iter_mut() {
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
                .map_err(McpServerError::Sdk)?,
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
    pub(crate) connection_pools: Arc<RwLock<HashMap<String, NetworkConnectionPool>>>,
    /// Connection pool configuration
    pub(crate) config: ConnectionPoolConfig,
    /// Cache for frequently accessed data
    pub(crate) cache: Arc<RwLock<HashMap<String, (Value, Instant)>>>,
    /// Cache TTL
    pub(crate) cache_ttl: Duration,
    /// Health check task handle
    pub(crate) health_check_handle: Option<tokio::task::JoinHandle<()>>,
    /// Loaded wallets (address -> wallet info)
    pub(crate) wallets: Arc<RwLock<HashMap<String, WalletInfo>>>,
    /// Current active wallet address
    pub(crate) active_wallet: Arc<Mutex<Option<String>>>,
    /// Current active wallet instance (if available)
    pub(crate) active_wallet_instance: Arc<Mutex<Option<MantraWallet>>>,
    /// Cache for wallet address to derivation index mappings
    pub(crate) wallet_derivation_cache: Arc<RwLock<HashMap<String, u32>>>,
    /// ERC-20 metadata registry and cache
    pub(crate) erc20_registry: Arc<RwLock<Erc20Registry>>,
}

impl McpSdkAdapter {
    /// Create a new MCP SDK adapter with connection pooling
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let registry = match Erc20Registry::load_default() {
            Ok(registry) => registry,
            Err(err) => {
                warn!("Failed to load ERC-20 registry: {}", err);
                Erc20Registry::default()
            }
        };

        Self {
            connection_pools: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(config.connection_ttl_secs),
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            health_check_handle: None,
            wallets: Arc::new(RwLock::new(HashMap::new())),
            active_wallet: Arc::new(Mutex::new(None)),
            active_wallet_instance: Arc::new(Mutex::new(None)),
            wallet_derivation_cache: Arc::new(RwLock::new(HashMap::new())),
            erc20_registry: Arc::new(RwLock::new(registry)),
        }
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

    pub(crate) fn erc20_registry(&self) -> Arc<RwLock<Erc20Registry>> {
        Arc::clone(&self.erc20_registry)
    }

    pub async fn list_registry_tokens(&self, chain_id: u64) -> Vec<Erc20TokenInfo> {
        let registry_arc = self.erc20_registry();
        let registry = registry_arc.read().await;
        registry.list_for_chain(chain_id)
    }

    pub async fn get_registry_token(
        &self,
        chain_id: u64,
        address: Address,
    ) -> Option<Erc20TokenInfo> {
        let registry_arc = self.erc20_registry();
        let registry = registry_arc.read().await;
        registry.get(chain_id, &address).cloned()
    }

    pub async fn add_custom_token(&self, info: Erc20TokenInfo) -> McpResult<()> {
        let registry = self.erc20_registry();
        let mut guard = registry.write().await;
        guard
            .upsert_custom(info)
            .map_err(|e| McpServerError::Internal(e.to_string()))
    }

    pub async fn remove_custom_token(&self, chain_id: u64, address: Address) -> McpResult<bool> {
        let registry = self.erc20_registry();
        let mut guard = registry.write().await;
        guard
            .remove_custom(chain_id, &address)
            .map_err(|e| McpServerError::Internal(e.to_string()))
    }

    /// Check if an address is a precompile address
    #[cfg(feature = "evm")]
    pub(crate) fn is_precompile_address(addr: Address) -> bool {
        // Precompiles are 0x0000...0001 through 0x0000...00ff
        let addr_bytes = addr.as_slice();

        // First 19 bytes must be zero
        let zeros = addr_bytes[0..19].iter().all(|&b| b == 0);

        // Last byte must be 0x01-0xff (not 0x00)
        let last_byte = addr_bytes[19];

        // Note: last_byte is u8, so <= PRECOMPILE_ADDRESS_MAX (0xff) is always true
        zeros && last_byte > 0
    }

    pub(crate) async fn ensure_token_metadata(
        &self,
        evm_client: &EvmClient,
        chain_id: u64,
        token_address: Address,
    ) -> McpResult<Erc20TokenInfo> {
        {
            let registry_arc = self.erc20_registry();
            let registry = registry_arc.read().await;
            if let Some(info) = registry.get(chain_id, &token_address) {
                if !info.needs_refresh(registry.ttl()) {
                    return Ok(info.clone());
                }
            }
        }

        let registry = self.erc20_registry();
        let guard = registry.write().await;
        let existing = guard.get(chain_id, &token_address).cloned();
        drop(guard);

        // Check if contract exists (has code)
        let code = evm_client
            .get_code(
                crate::protocols::evm::types::EthAddress(token_address),
                None,
            )
            .await
            .map_err(McpServerError::Sdk)?;

        if code.is_empty() && !Self::is_precompile_address(token_address) {
            return Err(McpServerError::InvalidArguments(format!(
                "No contract code found at address {:#x}. Please verify the address is correct.",
                token_address
            )));
        }

        // For precompiles, log but continue
        if Self::is_precompile_address(token_address) {
            debug!(
                "Address {:#x} is a precompile (no bytecode expected)",
                token_address
            );
        }

        let erc20 = SdkErc20::new(evm_client.clone(), token_address);

        // Try to fetch symbol, use address as fallback for precompiles that don't implement it
        let symbol = match erc20.symbol().await {
            Ok(value) => value,
            Err(err) => {
                debug!("Failed to fetch token symbol (likely precompile): {}", err);
                // Use checksummed address as fallback symbol
                format!("{:#x}", token_address)
            }
        };

        let name = match erc20.name().await {
            Ok(value) => Some(value),
            Err(err) => {
                debug!("Failed to fetch token name: {}", err);
                None
            }
        };

        let decimals = match erc20.decimals().await {
            Ok(value) => value,
            Err(err) => {
                debug!("Failed to fetch token decimals, using default 18: {}", err);
                18
            }
        };

        let source = existing
            .as_ref()
            .map(|info| info.source.clone())
            .unwrap_or(TokenSource::Discovered);

        let info = Erc20TokenInfo {
            address: token_address,
            symbol,
            name,
            decimals,
            chain_id,
            last_refreshed: Some(Instant::now()),
            source,
        };

        let registry = self.erc20_registry();
        let mut guard = registry.write().await;
        guard.upsert_runtime(info.clone());
        Ok(info)
    }

    pub(crate) async fn get_evm_client(&self) -> McpResult<(EvmClient, u64)> {
        let network_config = self.get_default_network_config().await?;

        let evm_rpc_url = network_config.evm_rpc_url.as_ref().ok_or_else(|| {
            McpServerError::InvalidArguments("EVM RPC URL not configured".to_string())
        })?;
        let chain_id = network_config.evm_chain_id.ok_or_else(|| {
            McpServerError::InvalidArguments("EVM chain ID not configured".to_string())
        })?;

        let client = EvmClient::new(evm_rpc_url, chain_id)
            .await
            .map_err(McpServerError::Sdk)?;

        Ok((client, chain_id))
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

    pub async fn get_pool_stats(&self) -> HashMap<String, (usize, usize, usize)> {
        let pools = self.connection_pools.read().await;
        pools
            .iter()
            .map(|(network_id, pool)| (network_id.clone(), pool.get_stats()))
            .collect()
    }

    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total = cache.len();
        let valid = cache
            .values()
            .filter(|(_, timestamp)| timestamp.elapsed() < self.cache_ttl)
            .count();
        (total, valid)
    }

    pub async fn shutdown(&mut self) -> McpResult<()> {
        info!("Shutting down MCP SDK Adapter...");

        // Stop health checks
        self.stop_health_checks().await;

        // Clear cache
        self.cache_clear().await;

        // Clear connection pools
        {
            let mut pools = self.connection_pools.write().await;
            pools.clear();
            debug!("Cleared all connection pools");
        }

        info!("MCP SDK Adapter shutdown complete");
        Ok(())
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

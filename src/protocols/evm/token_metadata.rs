/// Token metadata caching for EVM tokens
///
/// Provides caching for ERC-20 token metadata (decimals, symbol, name) to avoid
/// repeated RPC calls for the same token addresses.
use crate::protocols::evm::client::EvmClient;
use crate::protocols::evm::contracts::erc20::Erc20;
use alloy_primitives::Address;
use std::collections::HashMap;
use std::sync::RwLock;
use tracing::{debug, warn};

/// Cache for token metadata
///
/// Thread-safe cache using RwLock for concurrent access. ERC-20 token metadata
/// is immutable on-chain, so we cache indefinitely without TTL.
pub struct TokenMetadataCache {
    /// Cached token decimals (Address -> u8)
    decimals: RwLock<HashMap<Address, u8>>,
}

impl TokenMetadataCache {
    /// Create a new empty token metadata cache
    pub fn new() -> Self {
        Self {
            decimals: RwLock::new(HashMap::new()),
        }
    }

    /// Get token decimals with caching
    ///
    /// Queries the ERC-20 `decimals()` method on first access and caches the result.
    /// Falls back to 18 decimals (ERC-20 standard) if the contract doesn't implement
    /// the decimals() method or if the call fails.
    ///
    /// # Arguments
    /// * `address` - Token contract address
    /// * `client` - EVM client for making contract calls
    ///
    /// # Returns
    /// Token decimals (typically 6, 8, or 18 for standard tokens)
    pub async fn get_decimals(&self, address: Address, client: &EvmClient) -> u8 {
        // Fast path: check cache with read lock
        {
            let cache = self.decimals.read().unwrap();
            if let Some(&decimals) = cache.get(&address) {
                debug!("Token decimals cache hit for {}: {}", address, decimals);
                return decimals;
            }
        }

        // Slow path: query contract and update cache
        debug!(
            "Token decimals cache miss for {}, querying contract",
            address
        );

        let erc20 = Erc20::new(client.clone(), address);
        let decimals = match erc20.decimals().await {
            Ok(decimals) => {
                debug!(
                    "Successfully queried decimals for {}: {}",
                    address, decimals
                );
                decimals
            }
            Err(e) => {
                warn!(
                    "Failed to query decimals for {} ({}), falling back to 18",
                    address, e
                );
                // Fallback to 18 decimals (ERC-20 standard default)
                18
            }
        };

        // Update cache with write lock
        {
            let mut cache = self.decimals.write().unwrap();
            cache.insert(address, decimals);
        }

        decimals
    }

    /// Clear all cached metadata (useful for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        let mut cache = self.decimals.write().unwrap();
        cache.clear();
    }

    /// Get cache size (useful for monitoring)
    pub fn cache_size(&self) -> usize {
        let cache = self.decimals.read().unwrap();
        cache.len()
    }
}

impl Default for TokenMetadataCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = TokenMetadataCache::new();
        assert_eq!(cache.cache_size(), 0);
    }

    #[test]
    fn test_cache_size() {
        let cache = TokenMetadataCache::new();

        // Manually insert test data
        {
            let mut decimals_cache = cache.decimals.write().unwrap();
            let test_addr = Address::ZERO;
            decimals_cache.insert(test_addr, 6);
        }

        assert_eq!(cache.cache_size(), 1);

        cache.clear();
        assert_eq!(cache.cache_size(), 0);
    }
}

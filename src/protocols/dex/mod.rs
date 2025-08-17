/// DEX Protocol Module
/// Handles all DEX-related operations including pools, swaps, liquidity, and farming

pub mod client;
pub mod types;

pub use client::MantraDexClient;

use crate::error::Error;
use crate::protocols::Protocol;
use async_trait::async_trait;
use cosmrs::rpc::HttpClient;
use serde_json::{json, Value};
use std::sync::Arc;

/// DEX Protocol implementation
#[derive(Clone)]
pub struct DexProtocol {
    initialized: bool,
}

impl DexProtocol {
    /// Create a new DEX protocol instance
    pub fn new() -> Self {
        Self {
            initialized: false,
        }
    }
}

#[async_trait]
impl Protocol for DexProtocol {
    fn name(&self) -> &'static str {
        "dex"
    }

    fn version(&self) -> &'static str {
        "3.0.0"
    }

    async fn is_available(&self, _rpc_client: &HttpClient) -> Result<bool, Error> {
        // Check if DEX contracts are deployed on the network
        Ok(self.initialized)
    }

    fn get_config(&self) -> Result<Value, Error> {
        Ok(json!({
            "name": self.name(),
            "version": self.version(),
            "initialized": self.initialized,
        }))
    }

    async fn initialize(&mut self, rpc_client: Arc<HttpClient>) -> Result<(), Error> {
        // Initialize the DEX client with the RPC connection
        // This would typically load contract addresses from config
        self.initialized = true;
        Ok(())
    }
}

impl Default for DexProtocol {
    fn default() -> Self {
        Self::new()
    }
}
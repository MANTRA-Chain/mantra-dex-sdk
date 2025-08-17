/// Skip Protocol Module
/// Handles cross-chain routing and swaps via Skip Protocol
pub mod client;
pub mod types;

// Re-export Skip client
pub use client::SkipClient;

// Re-export Skip types for convenience
pub use types::{
    AssetPair, BridgeInfo, ChainAsset, CrossChainAsset, CrossChainRoute, RouteStep, RouteStepType,
    SimulateSmartSwapExactAssetInResponse, SimulateSwapExactAssetInResponse,
    SimulateSwapExactAssetOutResponse, SkipAction, SkipAffiliate, SkipAsset,
    SkipEntryPointExecuteMsg, SkipEntryPointQueryMsg, SkipIbcInfo, SkipRoute, SkipSwap,
    SkipSwapExactAssetIn, SkipSwapExactAssetOut, SkipSwapOperation, SupportedChain,
    TransferRequest, TransferResult, TransferStatus,
};

use crate::error::Error;
use crate::protocols::Protocol;
use async_trait::async_trait;
use cosmrs::rpc::HttpClient;
use serde_json::{json, Value};
use std::sync::Arc;

/// Skip Protocol implementation
#[derive(Clone)]
pub struct SkipProtocol {
    initialized: bool,
    contract_address: Option<String>,
}

impl SkipProtocol {
    /// Create a new Skip protocol instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            contract_address: None,
        }
    }

    /// Get the Skip contract address
    pub fn contract_address(&self) -> Option<&str> {
        self.contract_address.as_deref()
    }

    /// Set the Skip contract address
    pub fn set_contract_address(&mut self, address: String) {
        self.contract_address = Some(address);
    }
}

#[async_trait]
impl Protocol for SkipProtocol {
    fn name(&self) -> &'static str {
        "skip"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    async fn is_available(&self, _rpc_client: &HttpClient) -> Result<bool, Error> {
        // Check if Skip adapter contract is deployed on the network
        Ok(self.initialized && self.contract_address.is_some())
    }

    fn get_config(&self) -> Result<Value, Error> {
        Ok(json!({
            "name": self.name(),
            "version": self.version(),
            "initialized": self.initialized,
            "contract_address": self.contract_address,
        }))
    }

    async fn initialize(&mut self, _rpc_client: Arc<HttpClient>) -> Result<(), Error> {
        // Initialize the Skip protocol
        // This would typically load the Skip adapter contract address from config
        self.initialized = true;
        Ok(())
    }
}

impl Default for SkipProtocol {
    fn default() -> Self {
        Self::new()
    }
}

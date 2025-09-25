/// EVM Protocol Support for MANTRA SDK
///
/// This module provides first-class support for Ethereum Virtual Machine (EVM)
/// compatible blockchains, enabling interaction with smart contracts, token operations,
/// and cross-chain functionality through the MANTRA SDK.
///
/// # Features
///
/// - Read-only contract calls via `eth_call`
/// - Transaction submission with EIP-1559 fee market
/// - Event log filtering and querying
/// - ERC-20 and ERC-721 token helpers
/// - ABI loading and encoding/decoding
/// - Wallet address derivation from Cosmos keys
///
/// # Example
///
/// ```rust,no_run
/// use mantra_sdk::{MantraClient, MantraClientBuilder};
/// use alloy_primitives::{address, U256};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = MantraClientBuilder::new().build_auto().await?;
/// let evm = client.evm().await?;
///
/// // ERC-20 balance check
/// let usdc = evm.erc20(address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
/// let balance: U256 = usdc.balance_of(evm.eth_address()?).await?;
/// println!("USDC Balance: {}", balance);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "evm")]
pub mod abi;
#[cfg(feature = "evm")]
pub mod client;
#[cfg(feature = "evm")]
pub mod contracts;
#[cfg(feature = "evm")]
pub mod custom;
#[cfg(feature = "evm")]
pub mod erc20;
#[cfg(feature = "evm")]
pub mod erc721;
#[cfg(feature = "evm")]
pub mod types;

#[cfg(feature = "evm")]
use crate::error::Error;
#[cfg(feature = "evm")]
use crate::protocols::Protocol;
#[cfg(feature = "evm")]
use async_trait::async_trait;
#[cfg(feature = "evm")]
use cosmrs::rpc::HttpClient;
#[cfg(feature = "evm")]
use serde_json::{json, Value};
#[cfg(feature = "evm")]
use std::sync::Arc;

/// EVM Protocol implementation
///
/// Provides metadata and lifecycle management for EVM protocol support.
/// The actual EVM operations are handled by the `EvmClient`.
#[cfg(feature = "evm")]
#[derive(Clone)]
pub struct EvmProtocol {
    /// Whether the protocol has been initialized
    initialized: bool,
    /// RPC HTTP endpoint URL
    rpc_http: Option<String>,
    /// Chain ID for EIP-155 transactions
    chain_id: Option<u64>,
}

#[cfg(feature = "evm")]
impl EvmProtocol {
    /// Create a new EVM protocol instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            rpc_http: None,
            chain_id: None,
        }
    }

    /// Set the RPC HTTP endpoint
    pub fn set_rpc_http(&mut self, url: String) {
        self.rpc_http = Some(url);
    }

    /// Set the chain ID
    pub fn set_chain_id(&mut self, chain_id: u64) {
        self.chain_id = Some(chain_id);
    }

    /// Get the RPC HTTP endpoint
    pub fn rpc_http(&self) -> Option<&str> {
        self.rpc_http.as_deref()
    }

    /// Get the chain ID
    pub fn chain_id(&self) -> Option<u64> {
        self.chain_id
    }
}

#[cfg(feature = "evm")]
#[async_trait]
impl Protocol for EvmProtocol {
    fn name(&self) -> &'static str {
        "evm"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    async fn is_available(&self, _rpc: &HttpClient) -> Result<bool, Error> {
        Ok(self.initialized && self.rpc_http.is_some() && self.chain_id.is_some())
    }

    fn get_config(&self) -> Result<Value, Error> {
        Ok(json!({
            "name": self.name(),
            "version": self.version(),
            "rpc_http": self.rpc_http,
            "chain_id": self.chain_id,
            "initialized": self.initialized
        }))
    }

    async fn initialize(&mut self, _rpc: Arc<HttpClient>) -> Result<(), Error> {
        // Basic initialization - RPC connectivity could be tested here
        // but we defer actual provider creation to EvmClient
        self.initialized = true;
        Ok(())
    }
}

#[cfg(feature = "evm")]
impl Default for EvmProtocol {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "evm"))]
/// Stub implementation when EVM feature is not enabled
pub struct EvmProtocol;

#[cfg(not(feature = "evm"))]
impl EvmProtocol {
    /// Create a new EVM protocol instance (stub)
    pub fn new() -> Self {
        Self
    }
}

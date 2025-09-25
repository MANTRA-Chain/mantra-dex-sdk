/// Protocol modules for the Mantra SDK
/// Each protocol represents a different contract or feature set on the MANTRA blockchain
pub mod claimdrop;
pub mod dex;
#[cfg(feature = "evm")]
pub mod evm;
pub mod skip;

use crate::error::Error;
use async_trait::async_trait;
use cosmrs::rpc::HttpClient;
use serde_json::Value;
use std::sync::Arc;

/// Common trait for all protocol implementations
#[async_trait]
pub trait Protocol: Send + Sync {
    /// Get the protocol name
    fn name(&self) -> &'static str;

    /// Get the protocol version
    fn version(&self) -> &'static str;

    /// Check if the protocol is available on the current network
    async fn is_available(&self, rpc_client: &HttpClient) -> Result<bool, Error>;

    /// Get protocol-specific configuration
    fn get_config(&self) -> Result<Value, Error>;

    /// Initialize the protocol (e.g., load contract addresses)
    async fn initialize(&mut self, rpc_client: Arc<HttpClient>) -> Result<(), Error>;
}

/// Protocol registry for managing multiple protocols
pub struct ProtocolRegistry {
    protocols: Vec<Arc<dyn Protocol>>,
}

impl ProtocolRegistry {
    /// Create a new protocol registry
    pub fn new() -> Self {
        Self {
            protocols: Vec::new(),
        }
    }

    /// Register a new protocol
    pub fn register(&mut self, protocol: Arc<dyn Protocol>) {
        self.protocols.push(protocol);
    }

    /// Get a protocol by name
    pub fn get(&self, name: &str) -> Option<&dyn Protocol> {
        self.protocols
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
    }

    /// Get a protocol by name with error context
    pub fn get_protocol(&self, name: &str) -> Result<&dyn Protocol, Error> {
        self.protocols
            .iter()
            .find(|p| p.name() == name)
            .map(|p| p.as_ref())
            .ok_or_else(|| {
                let available_protocols = self.list();
                Error::Config(format!(
                    "Protocol '{}' not found. Available protocols: [{}]",
                    name,
                    available_protocols.join(", ")
                ))
            })
    }

    /// List all registered protocols
    pub fn list(&self) -> Vec<&str> {
        self.protocols.iter().map(|p| p.name()).collect()
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// ClaimDrop Protocol Module
/// Handles claimdrop campaigns, allocations, and rewards distribution

pub mod client;
pub mod factory;
pub mod types;

pub use client::ClaimdropClient;
pub use factory::ClaimdropFactoryClient;
pub use types::*;

use crate::error::Error;
use crate::protocols::Protocol;
use async_trait::async_trait;
use cosmrs::rpc::HttpClient;
use serde_json::{json, Value};
use std::sync::Arc;

/// ClaimDrop Protocol implementation
#[derive(Clone)]
pub struct ClaimdropProtocol {
    initialized: bool,
    factory_address: Option<String>,
    campaigns: Vec<String>,
}

impl ClaimdropProtocol {
    /// Create a new ClaimDrop protocol instance
    pub fn new() -> Self {
        Self {
            initialized: false,
            factory_address: None,
            campaigns: Vec::new(),
        }
    }

    /// Get the factory contract address
    pub fn factory_address(&self) -> Option<&str> {
        self.factory_address.as_deref()
    }

    /// Set the factory contract address
    pub fn set_factory_address(&mut self, address: String) {
        self.factory_address = Some(address);
    }

    /// Get list of known campaign addresses
    pub fn campaigns(&self) -> &[String] {
        &self.campaigns
    }

    /// Add a campaign address
    pub fn add_campaign(&mut self, address: String) {
        if !self.campaigns.contains(&address) {
            self.campaigns.push(address);
        }
    }
}

#[async_trait]
impl Protocol for ClaimdropProtocol {
    fn name(&self) -> &'static str {
        "claimdrop"
    }

    fn version(&self) -> &'static str {
        "2.0.0"
    }

    async fn is_available(&self, _rpc_client: &HttpClient) -> Result<bool, Error> {
        // Check if ClaimDrop factory contract is deployed on the network
        Ok(self.initialized && self.factory_address.is_some())
    }

    fn get_config(&self) -> Result<Value, Error> {
        Ok(json!({
            "name": self.name(),
            "version": self.version(),
            "initialized": self.initialized,
            "factory_address": self.factory_address,
            "campaigns": self.campaigns,
        }))
    }

    async fn initialize(&mut self, _rpc_client: Arc<HttpClient>) -> Result<(), Error> {
        // Initialize the ClaimDrop protocol
        // This would typically load the factory contract address from config
        self.initialized = true;
        Ok(())
    }
}

impl Default for ClaimdropProtocol {
    fn default() -> Self {
        Self::new()
    }
}
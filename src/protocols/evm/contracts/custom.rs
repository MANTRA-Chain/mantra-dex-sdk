/// Custom contract helpers
///
/// Provides utilities for interacting with custom smart contracts
/// that don't follow standard interfaces like ERC-20 or ERC-721.
use crate::error::Error;
use crate::protocols::evm::abi::AbiHelper;
use crate::protocols::evm::client::EvmClient;
use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::SolCall;
use serde_json::Value;

/// Custom contract helper
pub struct CustomContract {
    client: EvmClient,
    address: Address,
    abi: Option<AbiHelper>,
}

impl CustomContract {
    /// Create a new custom contract helper
    pub fn new(client: EvmClient, address: Address) -> Self {
        Self {
            client,
            address,
            abi: None,
        }
    }

    /// Create a new custom contract helper with ABI
    pub fn with_abi(client: EvmClient, address: Address, abi_json: &str) -> Result<Self, Error> {
        let mut abi_registry = AbiHelper::new();
        abi_registry.load_from_json(abi_json, "contract".to_string())?;
        let abi = Some(abi_registry);
        Ok(Self {
            client,
            address,
            abi,
        })
    }

    /// Load ABI from JSON string
    pub fn load_abi(&mut self, abi_json: &str) -> Result<(), Error> {
        let mut abi_registry = AbiHelper::new();
        abi_registry.load_from_json(abi_json, "contract".to_string())?;
        self.abi = Some(abi_registry);
        Ok(())
    }

    /// Call a read-only contract method by name with parameters
    pub async fn call_method(
        &self,
        _method_name: &str,
        _params: Vec<Value>,
    ) -> Result<Value, Error> {
        // TODO: Implement ABI-based method calling
        Err(Error::NotImplemented(
            "ABI-based method calling not yet implemented".to_string(),
        ))
    }

    /// Send a transaction to a contract method by name with parameters
    pub async fn send_method(
        &self,
        _method_name: &str,
        _params: Vec<Value>,
    ) -> Result<B256, Error> {
        // TODO: Implement ABI-based method sending
        Err(Error::NotImplemented(
            "ABI-based method sending not yet implemented".to_string(),
        ))
    }

    /// Call a contract method with raw encoded data
    pub async fn call_raw(&self, data: Vec<u8>) -> Result<Vec<u8>, Error> {
        self.client.call_raw(self.address, data).await
    }

    /// Send a raw transaction to the contract
    pub async fn send_raw(
        &self,
        data: Vec<u8>,
        value: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<B256, Error> {
        self.client
            .send_raw_transaction_data(self.address, data, value, wallet)
            .await
    }

    /// Get the contract address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Check if ABI is loaded
    pub fn has_abi(&self) -> bool {
        self.abi.is_some()
    }

    /// Get available method names (if ABI is loaded)
    pub fn method_names(&self) -> Option<Vec<String>> {
        // TODO: Implement method name extraction from ABI
        self.abi.as_ref().map(|_abi| vec![])
    }
}

#[cfg(feature = "evm")]
use crate::error::Error;
#[cfg(feature = "evm")]
use crate::protocols::evm::abi::AbiRegistry;
#[cfg(feature = "evm")]
use crate::protocols::evm::client::EvmClient;
#[cfg(feature = "evm")]
use crate::protocols::evm::types::EthAddress;
/// Generic Contract Helper for Custom ABI Interactions
///
/// Provides a flexible interface for interacting with contracts that don't
/// follow standard ERC patterns, using loaded ABIs for encoding/decoding.

#[cfg(feature = "evm")]
use alloy_primitives::{Address, U256};
#[cfg(feature = "evm")]
use serde_json::Value;
#[cfg(feature = "evm")]
use std::sync::Arc;

/// Generic contract helper for custom ABI interactions
#[cfg(feature = "evm")]
#[derive(Clone)]
pub struct CustomContract {
    /// EVM client for blockchain interaction
    client: EvmClient,
    /// Contract address
    address: EthAddress,
    /// ABI registry key for this contract
    abi_key: String,
    /// Reference to ABI registry
    abi_registry: Arc<AbiRegistry>,
}

#[cfg(feature = "evm")]
impl CustomContract {
    /// Create a new custom contract helper
    pub fn new(
        client: EvmClient,
        address: EthAddress,
        abi_key: String,
        abi_registry: Arc<AbiRegistry>,
    ) -> Self {
        Self {
            client,
            address,
            abi_key,
            abi_registry,
        }
    }

    /// Get the contract address
    pub fn address(&self) -> &EthAddress {
        &self.address
    }

    /// Get the ABI key
    pub fn abi_key(&self) -> &str {
        &self.abi_key
    }

    /// Call a read-only function by name with JSON parameters
    pub async fn call_function(
        &self,
        function_name: &str,
        params: Vec<Value>,
    ) -> Result<Vec<u8>, Error> {
        let function = self
            .abi_registry
            .get_function(&self.abi_key, function_name)?;
        let data = crate::protocols::evm::abi::AbiEncoder::encode_function_call(function, &params)?;

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        self.client.call(request).await
    }

    /// Decode function result by name
    pub fn decode_function_result(
        &self,
        function_name: &str,
        data: &[u8],
    ) -> Result<Vec<Value>, Error> {
        let function = self
            .abi_registry
            .get_function(&self.abi_key, function_name)?;
        crate::protocols::evm::abi::AbiEncoder::decode_function_result(function, data)
    }

    /// Call and decode a function in one step
    pub async fn call_and_decode(
        &self,
        function_name: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Value>, Error> {
        let result_data = self.call_function(function_name, params).await?;
        self.decode_function_result(function_name, &result_data)
    }

    /// Estimate gas for a function call
    pub async fn estimate_function_gas(
        &self,
        function_name: &str,
        params: Vec<Value>,
    ) -> Result<u64, Error> {
        let function = self
            .abi_registry
            .get_function(&self.abi_key, function_name)?;
        let data = crate::protocols::evm::abi::AbiEncoder::encode_function_call(function, &params)?;

        let tx_request =
            crate::protocols::evm::types::EvmTransactionRequest::new(self.client.chain_id())
                .to(self.address.clone())
                .value(U256::ZERO)
                .data(data);

        self.client.estimate_gas(tx_request).await
    }

    /// Get event topics for filtering
    pub fn get_event_topics(
        &self,
        event_name: &str,
        params: Option<&[Option<Value>]>,
    ) -> Result<Vec<Option<alloy_primitives::B256>>, Error> {
        let event = self.abi_registry.get_event(&self.abi_key, event_name)?;
        crate::protocols::evm::abi::AbiEncoder::encode_event_topics(event, params)
    }

    /// Check if a function exists in the ABI
    pub fn has_function(&self, function_name: &str) -> bool {
        self.abi_registry
            .get_function(&self.abi_key, function_name)
            .is_ok()
    }

    /// Check if an event exists in the ABI
    pub fn has_event(&self, event_name: &str) -> bool {
        self.abi_registry
            .get_event(&self.abi_key, event_name)
            .is_ok()
    }

    /// List all available functions
    pub fn list_functions(&self) -> Vec<String> {
        if let Some(abi) = self.abi_registry.get(&self.abi_key) {
            abi.functions().map(|f| f.name.clone()).collect()
        } else {
            vec![]
        }
    }

    /// List all available events
    pub fn list_events(&self) -> Vec<String> {
        if let Some(abi) = self.abi_registry.get(&self.abi_key) {
            abi.events().map(|e| e.name.clone()).collect()
        } else {
            vec![]
        }
    }
}

#[cfg(not(feature = "evm"))]
/// Stub custom contract implementation when EVM feature is not enabled
pub struct CustomContract;

#[cfg(not(feature = "evm"))]
impl CustomContract {
    pub fn new(_client: (), _address: (), _abi_key: String, _registry: ()) -> Self {
        Self
    }
    pub async fn call_function(
        &self,
        _function_name: &str,
        _params: (),
    ) -> Result<(), crate::error::Error> {
        Err(crate::error::Error::Config(
            "EVM feature not enabled".to_string(),
        ))
    }
}

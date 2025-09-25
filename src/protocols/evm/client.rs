#[cfg(feature = "evm")]
use crate::error::Error;
#[cfg(feature = "evm")]
use crate::protocols::evm::types::{
    EthAddress, EventFilter, EvmCallRequest, EvmError, EvmTransactionRequest,
};
#[cfg(feature = "evm")]
use crate::wallet::MantraWallet;
#[cfg(feature = "evm")]
use alloy_primitives::{Address, Bytes, B256, U256};
/// EVM Client for MANTRA SDK
///
/// Provides high-level interface for interacting with EVM-compatible blockchains,
/// including contract calls, transaction submission, event querying, and gas estimation.

#[cfg(feature = "evm")]
use alloy_provider::{Provider, ProviderBuilder};
#[cfg(feature = "evm")]
use alloy_rpc_types_eth::{BlockNumberOrTag, Filter, Log, TransactionRequest};
#[cfg(feature = "evm")]
use alloy_sol_types::SolCall;
#[cfg(feature = "evm")]
use alloy_transport_http::{Client, Http};
#[cfg(feature = "evm")]
use std::sync::Arc;

/// EVM Client for blockchain interactions
#[cfg(feature = "evm")]
#[derive(Clone)]
pub struct EvmClient {
    /// Alloy provider for RPC communication
    provider: alloy_provider::RootProvider<Http<Client>>,
    /// Chain ID for transaction signing
    chain_id: u64,
}

#[cfg(feature = "evm")]
impl EvmClient {
    /// Create a new EVM client with the given RPC endpoint and chain ID
    pub async fn new(rpc_url: &str, chain_id: u64) -> Result<Self, Error> {
        let url = reqwest::Url::parse(rpc_url)
            .map_err(|e| Error::Config(format!("Invalid RPC URL: {}", e)))?;
        let provider = ProviderBuilder::new().on_http(url);

        Ok(Self { provider, chain_id })
    }

    /// Execute a read-only contract call
    pub async fn call(&self, request: EvmCallRequest) -> Result<Vec<u8>, Error> {
        let tx_request = TransactionRequest {
            to: Some(alloy_primitives::TxKind::Call(request.to.0)),
            input: request.data.into(),
            ..Default::default()
        };

        let block = request
            .block
            .as_ref()
            .and_then(|b| b.parse::<BlockNumberOrTag>().ok())
            .unwrap_or(BlockNumberOrTag::Latest);

        let result = self
            .provider
            .call(&tx_request)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(result.to_vec())
    }

    /// Estimate gas for a transaction
    pub async fn estimate_gas(&self, request: EvmTransactionRequest) -> Result<u64, Error> {
        let tx_request = TransactionRequest {
            to: request
                .to
                .map(|addr| alloy_primitives::TxKind::Call(addr.0)),
            value: Some(request.value),
            gas: request.gas_limit,
            max_fee_per_gas: request.max_fee_per_gas.map(|f| f.to::<u128>()),
            max_priority_fee_per_gas: request.max_priority_fee_per_gas.map(|f| f.to::<u128>()),
            input: request.data.into(),
            ..Default::default()
        };

        let gas = self
            .provider
            .estimate_gas(&tx_request)
            .await
            .map_err(|e| EvmError::GasEstimationError(e.to_string()))?;

        Ok(gas)
    }

    /// Get the current block number
    pub async fn get_block_number(&self) -> Result<u64, Error> {
        let block_number = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(block_number)
    }

    /// Get the current gas price (legacy)
    pub async fn get_gas_price(&self) -> Result<U256, Error> {
        let gas_price = self
            .provider
            .get_gas_price()
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(U256::from(gas_price))
    }

    /// Get EIP-1559 fee data
    pub async fn get_fee_data(&self) -> Result<(U256, U256), Error> {
        // Get fee history for the last block
        let fee_history = self
            .provider
            .get_fee_history(1, BlockNumberOrTag::Latest, &[50.0])
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        if let (Some(base_fee), Some(reward)) = (
            fee_history.base_fee_per_gas.last(),
            fee_history
                .reward
                .as_ref()
                .and_then(|r| r.last())
                .and_then(|r| r.first()),
        ) {
            Ok((U256::from(*base_fee), U256::from(*reward)))
        } else {
            // Fallback to gas price
            let gas_price = self.get_gas_price().await?;
            Ok((gas_price, gas_price / U256::from(10))) // Rough estimate
        }
    }

    /// Query event logs
    pub async fn get_logs(&self, filter: EventFilter) -> Result<Vec<Log>, Error> {
        let alloy_filter = Filter {
            block_option: alloy_rpc_types_eth::FilterBlockOption::Range {
                from_block: filter.from_block.as_ref().and_then(|b| b.parse().ok()),
                to_block: filter.to_block.as_ref().and_then(|b| b.parse().ok()),
            },
            address: alloy_rpc_types_eth::FilterSet::from(
                filter
                    .addresses
                    .into_iter()
                    .map(|addr| addr.0)
                    .collect::<Vec<_>>(),
            ),
            topics: Default::default(), // TODO: Implement proper topic filtering
        };

        let logs = self
            .provider
            .get_logs(&alloy_filter)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(logs)
    }

    /// Get the balance of an address
    pub async fn get_balance(
        &self,
        address: EthAddress,
        block: Option<String>,
    ) -> Result<U256, Error> {
        let block_tag = block
            .as_ref()
            .and_then(|b| b.parse::<BlockNumberOrTag>().ok())
            .unwrap_or(BlockNumberOrTag::Latest);

        let balance = self
            .provider
            .get_balance(address.0)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(balance)
    }

    /// Get transaction receipt by hash
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<alloy_rpc_types_eth::TransactionReceipt>, Error> {
        let receipt = self
            .provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(receipt)
    }

    /// Get transaction by hash
    pub async fn get_transaction(
        &self,
        tx_hash: B256,
    ) -> Result<Option<alloy_rpc_types_eth::Transaction>, Error> {
        let tx = self
            .provider
            .get_transaction_by_hash(tx_hash)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(tx)
    }

    /// Get code at address
    pub async fn get_code(
        &self,
        address: EthAddress,
        block: Option<String>,
    ) -> Result<Bytes, Error> {
        let block_tag = block
            .as_ref()
            .and_then(|b| b.parse::<BlockNumberOrTag>().ok())
            .unwrap_or(BlockNumberOrTag::Latest);

        let code = self
            .provider
            .get_code_at(address.0)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(code)
    }

    /// Get storage value at address and slot
    pub async fn get_storage_at(
        &self,
        address: EthAddress,
        slot: U256,
        block: Option<String>,
    ) -> Result<U256, Error> {
        let block_tag = block
            .as_ref()
            .and_then(|b| b.parse::<BlockNumberOrTag>().ok())
            .unwrap_or(BlockNumberOrTag::Latest);

        let storage = self
            .provider
            .get_storage_at(address.0, slot)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(storage)
    }

    /// Get the chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Submit a signed transaction
    ///
    /// Note: This is a placeholder. Actual implementation would require
    /// transaction signing and submission logic integrated with the wallet.
    pub async fn send_raw_transaction(&self, _signed_tx: Vec<u8>) -> Result<B256, Error> {
        // TODO: Implement transaction submission
        // This would decode the signed transaction and submit via eth_sendRawTransaction
        Err(Error::Other(
            "Transaction submission not yet implemented".to_string(),
        ))
    }

    /// Call a contract method (read-only)
    pub async fn call_contract<T: SolCall>(
        &self,
        contract_address: Address,
        call: T,
    ) -> Result<T::Return, Error> {
        let data = call.abi_encode();
        let request = EvmCallRequest {
            to: EthAddress(contract_address),
            data: data.into(),
            block: None,
        };
        let result = self.call(request).await?;
        let decoded = T::abi_decode_returns(&result, false)
            .map_err(|e| Error::Evm(format!("Failed to decode contract call result: {}", e)))?;
        Ok(decoded)
    }

    /// Send a contract transaction
    pub async fn send_contract_call<T: SolCall>(
        &self,
        _contract_address: Address,
        _call: T,
    ) -> Result<(), Error> {
        // TODO: Implement transaction sending
        Err(Error::NotImplemented(
            "Transaction sending not yet implemented".to_string(),
        ))
    }

    /// Call raw contract data
    pub async fn call_raw(&self, address: Address, data: Vec<u8>) -> Result<Vec<u8>, Error> {
        let request = EvmCallRequest {
            to: EthAddress(address),
            data: data.into(),
            block: None,
        };
        self.call(request).await
    }

    /// Send raw transaction data
    pub async fn send_raw_transaction_data(
        &self,
        _address: Address,
        _data: Vec<u8>,
        _value: U256,
    ) -> Result<B256, Error> {
        // TODO: Implement raw transaction sending
        Err(Error::NotImplemented(
            "Raw transaction sending not yet implemented".to_string(),
        ))
    }

    /// Create an ERC-20 helper for the given contract address
    pub fn erc20(&self, address: Address) -> crate::protocols::evm::contracts::Erc20 {
        crate::protocols::evm::contracts::Erc20::new(self.clone(), address)
    }

    /// Create an ERC-721 helper for the given contract address
    pub fn erc721(&self, address: Address) -> crate::protocols::evm::contracts::Erc721 {
        crate::protocols::evm::contracts::Erc721::new(self.clone(), address)
    }
}

#[cfg(not(feature = "evm"))]
/// Stub client when EVM feature is not enabled
pub struct EvmClient;

#[cfg(not(feature = "evm"))]
impl EvmClient {
    pub async fn new(_rpc_url: &str, _chain_id: u64) -> Result<Self, crate::error::Error> {
        Err(crate::error::Error::Config(
            "EVM feature not enabled".to_string(),
        ))
    }
}

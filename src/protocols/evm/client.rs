#[cfg(feature = "evm")]
use crate::error::Error;
#[cfg(feature = "evm")]
use crate::protocols::evm::tx::{Eip1559Transaction, SignedEip1559Transaction};
#[cfg(feature = "evm")]
use crate::protocols::evm::types::{
    Eip1559FeeSuggestion, EthAddress, EventFilter, EvmCallRequest, EvmError, EvmTransactionRequest,
};
#[cfg(feature = "evm")]
use alloy_primitives::{Address, Bytes, B256, U256};
/// EVM Client for MANTRA SDK
///
/// Provides high-level interface for interacting with EVM-compatible blockchains,
/// including contract calls, transaction submission, event querying, and gas estimation.

#[cfg(feature = "evm")]
use alloy_provider::{PendingTransactionBuilder, Provider, ProviderBuilder};
#[cfg(feature = "evm")]
use alloy_rpc_types_eth::{BlockId, BlockNumberOrTag, Filter, Log, TransactionRequest};
#[cfg(feature = "evm")]
use alloy_sol_types::SolCall;
#[cfg(feature = "evm")]
use alloy_transport_http::{Client, Http};
#[cfg(feature = "evm")]
use std::time::Duration;

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
        self.estimate_gas_with_options(request, None, None).await
    }

    /// Estimate gas with optional sender override and block context.
    pub async fn estimate_gas_with_options(
        &self,
        request: EvmTransactionRequest,
        from: Option<EthAddress>,
        block: Option<BlockNumberOrTag>,
    ) -> Result<u64, Error> {
        let rpc_request = request.to_rpc_request(from);
        let mut call = self.provider.estimate_gas(&rpc_request);
        if let Some(block) = block {
            call = call.block(BlockId::Number(block));
        }

        call.await
            .map_err(|e| EvmError::GasEstimationError(e.to_string()).into())
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

    /// Provide EIP-1559 fee suggestions using provider heuristics.
    pub async fn fee_suggestion(&self) -> Result<Eip1559FeeSuggestion, Error> {
        let estimation = self
            .provider
            .estimate_eip1559_fees(None)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        let max_fee = U256::from(estimation.max_fee_per_gas);
        let max_priority = U256::from(estimation.max_priority_fee_per_gas);
        let base_component = max_fee.saturating_sub(max_priority);
        let base_fee = if base_component.is_zero() {
            U256::ZERO
        } else {
            base_component / U256::from(2u64)
        };

        Ok(Eip1559FeeSuggestion {
            base_fee_per_gas: base_fee,
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas: max_priority,
        })
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

    /// Fetch the transaction count (nonce) for an address at a given block.
    pub async fn get_transaction_count(
        &self,
        address: EthAddress,
        block: Option<BlockNumberOrTag>,
    ) -> Result<u64, Error> {
        let mut call = self.provider.get_transaction_count(address.0);
        if let Some(block) = block {
            call = call.block_id(BlockId::Number(block));
        }

        call.await
            .map_err(|e| EvmError::RpcError(e.to_string()).into())
    }

    /// Fetch the pending nonce (including mempool transactions).
    pub async fn get_pending_nonce(&self, address: EthAddress) -> Result<u64, Error> {
        self.get_transaction_count(address, Some(BlockNumberOrTag::Pending))
            .await
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

    /// Broadcast a signed EIP-1559 transaction.
    pub async fn send_raw_transaction(
        &self,
        signed_tx: &SignedEip1559Transaction,
    ) -> Result<B256, Error> {
        let raw = signed_tx.raw().clone();
        let pending = self
            .provider
            .send_raw_transaction(raw.as_ref())
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(*pending.tx_hash())
    }

    /// Broadcast a signed transaction and wait for the receipt.
    pub async fn send_raw_transaction_with_confirmations(
        &self,
        signed_tx: &SignedEip1559Transaction,
        confirmations: u64,
        timeout: Option<Duration>,
    ) -> Result<(B256, alloy_rpc_types_eth::TransactionReceipt), Error> {
        let tx_hash = self.send_raw_transaction(signed_tx).await?;
        let receipt = self
            .wait_for_receipt(tx_hash, confirmations, timeout)
            .await?;
        Ok((tx_hash, receipt))
    }

    /// Wait for a transaction to be mined with optional timeout.
    pub async fn wait_for_receipt(
        &self,
        tx_hash: B256,
        confirmations: u64,
        timeout: Option<Duration>,
    ) -> Result<alloy_rpc_types_eth::TransactionReceipt, Error> {
        let mut builder = PendingTransactionBuilder::new(&self.provider, tx_hash)
            .with_required_confirmations(confirmations);

        if let Some(timeout) = timeout {
            builder = builder.with_timeout(Some(timeout));
        }

        builder
            .get_receipt()
            .await
            .map_err(|e| Error::Other(format!("Pending transaction error: {e}")))
    }

    /// Simulate an EIP-1559 transaction via eth_call.
    pub async fn simulate_eip1559(
        &self,
        from: EthAddress,
        tx: &Eip1559Transaction,
        block: Option<BlockNumberOrTag>,
    ) -> Result<Bytes, Error> {
        let request =
            <EvmTransactionRequest as From<&Eip1559Transaction>>::from(tx).from(from.clone());
        let rpc_request = request.to_rpc_request(Some(from));
        let mut call = self.provider.call(&rpc_request);
        if let Some(block) = block {
            call = call.block(BlockId::Number(block));
        }
        call.await
            .map_err(|e| EvmError::RpcError(e.to_string()).into())
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

    /// Submit already-signed transaction bytes to the network.
    pub async fn send_raw_transaction_bytes(&self, signed_tx: Vec<u8>) -> Result<B256, Error> {
        let pending = self
            .provider
            .send_raw_transaction(&signed_tx)
            .await
            .map_err(|e| EvmError::RpcError(e.to_string()))?;

        Ok(*pending.tx_hash())
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

#[cfg(feature = "evm")]
use crate::error::Error;
#[cfg(feature = "evm")]
use crate::protocols::evm::client::EvmClient;
#[cfg(feature = "evm")]
use crate::protocols::evm::types::EthAddress;
/// ERC-20 Token Contract Helper
///
/// Provides high-level methods for interacting with ERC-20 compatible tokens,
/// including balance queries, transfers, and approvals.

#[cfg(feature = "evm")]
use alloy_primitives::{Address, U256};
#[cfg(feature = "evm")]
use alloy_sol_types::SolValue;

/// ERC-20 token contract helper
#[cfg(feature = "evm")]
#[derive(Clone)]
pub struct Erc20 {
    /// EVM client for blockchain interaction
    client: EvmClient,
    /// Contract address
    address: EthAddress,
}

#[cfg(feature = "evm")]
impl Erc20 {
    /// Create a new ERC-20 helper for the given contract address
    pub fn new(client: EvmClient, address: EthAddress) -> Self {
        Self { client, address }
    }

    /// Get the contract address
    pub fn address(&self) -> &EthAddress {
        &self.address
    }

    /// Query the balance of an address
    pub async fn balance_of(&self, owner: EthAddress) -> Result<U256, Error> {
        let data = Self::encode_balance_of(owner.0);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_uint256(&result)
    }

    /// Query the total supply of the token
    pub async fn total_supply(&self) -> Result<U256, Error> {
        let data = Self::encode_total_supply();

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_uint256(&result)
    }

    /// Query token decimals
    pub async fn decimals(&self) -> Result<u8, Error> {
        let data = Self::encode_decimals();

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        if result.len() >= 32 {
            Ok(result[31]) // Last byte contains decimals
        } else {
            Err(Error::Config("Invalid decimals response".to_string()))
        }
    }

    /// Query token symbol
    pub async fn symbol(&self) -> Result<String, Error> {
        let data = Self::encode_symbol();

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_string(&result)
    }

    /// Query token name
    pub async fn name(&self) -> Result<String, Error> {
        let data = Self::encode_name();

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_string(&result)
    }

    /// Query allowance between owner and spender
    pub async fn allowance(&self, owner: EthAddress, spender: EthAddress) -> Result<U256, Error> {
        let data = Self::encode_allowance(owner.0, spender.0);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_uint256(&result)
    }

    /// Create a transfer call data (for use in transactions)
    pub fn transfer(&self, to: EthAddress, amount: U256) -> Vec<u8> {
        Self::encode_transfer(to.0, amount)
    }

    /// Create an approve call data (for use in transactions)
    pub fn approve(&self, spender: EthAddress, amount: U256) -> Vec<u8> {
        Self::encode_approve(spender.0, amount)
    }

    /// Create a transferFrom call data (for use in transactions)
    pub fn transfer_from(&self, from: EthAddress, to: EthAddress, amount: U256) -> Vec<u8> {
        Self::encode_transfer_from(from.0, to.0, amount)
    }

    /// Estimate gas for a transfer
    pub async fn estimate_transfer_gas(
        &self,
        _from: EthAddress,
        to: EthAddress,
        amount: U256,
    ) -> Result<u64, Error> {
        let data = self.transfer(to, amount);

        let tx_request =
            crate::protocols::evm::types::EvmTransactionRequest::new(self.client.chain_id())
                .to(self.address.clone())
                .value(U256::ZERO)
                .data(data);

        self.client.estimate_gas(tx_request).await
    }

    // Encoding helpers
    fn encode_balance_of(owner: Address) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"balanceOf(address)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&owner.abi_encode());
        data
    }

    fn encode_total_supply() -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"totalSupply()");
        signature[..4].to_vec()
    }

    fn encode_decimals() -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"decimals()");
        signature[..4].to_vec()
    }

    fn encode_symbol() -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"symbol()");
        signature[..4].to_vec()
    }

    fn encode_name() -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"name()");
        signature[..4].to_vec()
    }

    fn encode_allowance(owner: Address, spender: Address) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"allowance(address,address)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&owner.abi_encode());
        data.extend_from_slice(&spender.abi_encode());
        data
    }

    fn encode_transfer(to: Address, amount: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"transfer(address,uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&to.abi_encode());
        data.extend_from_slice(&amount.abi_encode());
        data
    }

    fn encode_approve(spender: Address, amount: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"approve(address,uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&spender.abi_encode());
        data.extend_from_slice(&amount.abi_encode());
        data
    }

    fn encode_transfer_from(from: Address, to: Address, amount: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"transferFrom(address,address,uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&from.abi_encode());
        data.extend_from_slice(&to.abi_encode());
        data.extend_from_slice(&amount.abi_encode());
        data
    }

    // Decoding helpers
    fn decode_uint256(data: &[u8]) -> Result<U256, Error> {
        if data.len() < 32 {
            return Err(Error::Config("Insufficient data for uint256".to_string()));
        }
        Ok(U256::from_be_slice(&data[0..32]))
    }

    fn decode_string(data: &[u8]) -> Result<String, Error> {
        // Simplified string decoding - in production, this would handle dynamic types properly
        if data.len() >= 64 {
            let length = u32::from_be_bytes(data[32..36].try_into().unwrap()) as usize;
            if data.len() >= 64 + length {
                let string_bytes = &data[64..64 + length];
                String::from_utf8(string_bytes.to_vec())
                    .map_err(|e| Error::Config(format!("Invalid string encoding: {}", e)))
            } else {
                Err(Error::Config("Invalid string response length".to_string()))
            }
        } else {
            Err(Error::Config("Invalid string response".to_string()))
        }
    }
}

#[cfg(not(feature = "evm"))]
/// Stub ERC-20 implementation when EVM feature is not enabled
pub struct Erc20;

#[cfg(not(feature = "evm"))]
impl Erc20 {
    pub fn new(_client: (), _address: ()) -> Self {
        Self
    }
    pub async fn balance_of(&self, _owner: ()) -> Result<(), crate::error::Error> {
        Err(crate::error::Error::Config(
            "EVM feature not enabled".to_string(),
        ))
    }
}

#[cfg(feature = "evm")]
use crate::error::Error;
#[cfg(feature = "evm")]
use crate::protocols::evm::client::EvmClient;
#[cfg(feature = "evm")]
use crate::protocols::evm::types::EthAddress;
/// ERC-721 NFT Contract Helper
///
/// Provides high-level methods for interacting with ERC-721 compatible NFTs,
/// including ownership queries, transfers, and metadata.
#[cfg(feature = "evm")]
use alloy_primitives::{Address, U256};
#[cfg(feature = "evm")]
use alloy_sol_types::SolValue;

/// ERC-721 NFT contract helper
#[cfg(feature = "evm")]
#[derive(Clone)]
pub struct Erc721 {
    /// EVM client for blockchain interaction
    client: EvmClient,
    /// Contract address
    address: EthAddress,
}

#[cfg(feature = "evm")]
impl Erc721 {
    /// Create a new ERC-721 helper for the given contract address
    pub fn new(client: EvmClient, address: EthAddress) -> Self {
        Self { client, address }
    }

    /// Get the contract address
    pub fn address(&self) -> &EthAddress {
        &self.address
    }

    /// Query the owner of a token
    pub async fn owner_of(&self, token_id: U256) -> Result<EthAddress, Error> {
        let data = Self::encode_owner_of(token_id);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        let address = Self::decode_address(&result)?;
        Ok(EthAddress(address))
    }

    /// Query the balance of an address (number of tokens owned)
    pub async fn balance_of(&self, owner: EthAddress) -> Result<U256, Error> {
        let data = Self::encode_balance_of(owner.0);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_uint256(&result)
    }

    /// Query token name
    pub async fn name(&self) -> Result<String, Error> {
        let data = Self::encode_name();

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_string(&result)
    }

    /// Query token symbol
    pub async fn symbol(&self) -> Result<String, Error> {
        let data = Self::encode_symbol();

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_string(&result)
    }

    /// Query token URI for metadata
    pub async fn token_uri(&self, token_id: U256) -> Result<String, Error> {
        let data = Self::encode_token_uri(token_id);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_string(&result)
    }

    /// Query approved address for a token
    pub async fn get_approved(&self, token_id: U256) -> Result<EthAddress, Error> {
        let data = Self::encode_get_approved(token_id);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        let address = Self::decode_address(&result)?;
        Ok(EthAddress(address))
    }

    /// Query if an operator is approved for all tokens of an owner
    pub async fn is_approved_for_all(
        &self,
        owner: EthAddress,
        operator: EthAddress,
    ) -> Result<bool, Error> {
        let data = Self::encode_is_approved_for_all(owner.0, operator.0);

        let request = crate::protocols::evm::types::EvmCallRequest::new(self.address.clone(), data);

        let result = self.client.call(request).await?;
        Self::decode_bool(&result)
    }

    /// Create a transferFrom call data (for use in transactions)
    pub fn transfer_from(&self, from: EthAddress, to: EthAddress, token_id: U256) -> Vec<u8> {
        Self::encode_transfer_from(from.0, to.0, token_id)
    }

    /// Create a safeTransferFrom call data (for use in transactions)
    pub fn safe_transfer_from(&self, from: EthAddress, to: EthAddress, token_id: U256) -> Vec<u8> {
        Self::encode_safe_transfer_from(from.0, to.0, token_id)
    }

    /// Create an approve call data (for use in transactions)
    pub fn approve(&self, approved: EthAddress, token_id: U256) -> Vec<u8> {
        Self::encode_approve(approved.0, token_id)
    }

    /// Create a setApprovalForAll call data (for use in transactions)
    pub fn set_approval_for_all(&self, operator: EthAddress, approved: bool) -> Vec<u8> {
        Self::encode_set_approval_for_all(operator.0, approved)
    }

    /// Estimate gas for a transfer
    pub async fn estimate_transfer_gas(
        &self,
        from: EthAddress,
        to: EthAddress,
        token_id: U256,
    ) -> Result<u64, Error> {
        let data = self.transfer_from(from, to, token_id);

        let tx_request =
            crate::protocols::evm::types::EvmTransactionRequest::new(self.client.chain_id())
                .to(self.address.clone())
                .value(U256::ZERO)
                .data(data);

        self.client.estimate_gas(tx_request).await
    }

    // Encoding helpers
    fn encode_owner_of(token_id: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"ownerOf(uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&token_id.abi_encode());
        data
    }

    fn encode_balance_of(owner: Address) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"balanceOf(address)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&owner.abi_encode());
        data
    }

    fn encode_name() -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"name()");
        signature[..4].to_vec()
    }

    fn encode_symbol() -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"symbol()");
        signature[..4].to_vec()
    }

    fn encode_token_uri(token_id: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"tokenURI(uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&token_id.abi_encode());
        data
    }

    fn encode_get_approved(token_id: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"getApproved(uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&token_id.abi_encode());
        data
    }

    fn encode_is_approved_for_all(owner: Address, operator: Address) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"isApprovedForAll(address,address)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&owner.abi_encode());
        data.extend_from_slice(&operator.abi_encode());
        data
    }

    fn encode_transfer_from(from: Address, to: Address, token_id: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"transferFrom(address,address,uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&from.abi_encode());
        data.extend_from_slice(&to.abi_encode());
        data.extend_from_slice(&token_id.abi_encode());
        data
    }

    fn encode_safe_transfer_from(from: Address, to: Address, token_id: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"safeTransferFrom(address,address,uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&from.abi_encode());
        data.extend_from_slice(&to.abi_encode());
        data.extend_from_slice(&token_id.abi_encode());
        data
    }

    fn encode_approve(approved: Address, token_id: U256) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"approve(address,uint256)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&approved.abi_encode());
        data.extend_from_slice(&token_id.abi_encode());
        data
    }

    fn encode_set_approval_for_all(operator: Address, approved: bool) -> Vec<u8> {
        let signature = alloy_primitives::keccak256(b"setApprovalForAll(address,bool)");
        let selector = &signature[..4];
        let mut data = selector.to_vec();
        data.extend_from_slice(&operator.abi_encode());
        data.extend_from_slice(&(approved as u8).to_be_bytes());
        data
    }

    // Decoding helpers
    fn decode_uint256(data: &[u8]) -> Result<U256, Error> {
        if data.len() < 32 {
            return Err(Error::Config("Insufficient data for uint256".to_string()));
        }
        Ok(U256::from_be_slice(&data[0..32]))
    }

    fn decode_address(data: &[u8]) -> Result<Address, Error> {
        if data.len() < 32 {
            return Err(Error::Config("Insufficient data for address".to_string()));
        }
        let mut addr_bytes = [0u8; 20];
        addr_bytes.copy_from_slice(&data[12..32]);
        Ok(Address::from(addr_bytes))
    }

    fn decode_bool(data: &[u8]) -> Result<bool, Error> {
        if data.len() < 32 {
            return Err(Error::Config("Insufficient data for bool".to_string()));
        }
        Ok(data[31] != 0)
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
/// Stub ERC-721 implementation when EVM feature is not enabled
pub struct Erc721;

#[cfg(not(feature = "evm"))]
impl Erc721 {
    pub fn new(_client: (), _address: ()) -> Self {
        Self
    }
    pub async fn owner_of(&self, _token_id: ()) -> Result<(), crate::error::Error> {
        Err(crate::error::Error::Config(
            "EVM feature not enabled".to_string(),
        ))
    }
}

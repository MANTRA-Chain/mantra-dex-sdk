#[cfg(feature = "evm")]
use crate::error::Error;
/// EVM-specific types and utilities for MANTRA SDK
///
/// This module provides type definitions and utilities for working with
/// Ethereum Virtual Machine (EVM) compatible blockchains, including
/// address handling, transaction types, and error definitions.
#[cfg(feature = "evm")]
use alloy_eips::eip2930::AccessList;
#[cfg(feature = "evm")]
use alloy_primitives::{Address, Bytes, ChainId, B256, U256};
#[cfg(feature = "evm")]
use alloy_rpc_types_eth::transaction::{
    TransactionInput, TransactionRequest as RpcTransactionRequest,
};
#[cfg(feature = "evm")]
use alloy_sol_types::SolCall;
#[cfg(feature = "evm")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "evm")]
use std::convert::TryInto;
#[cfg(feature = "evm")]
use std::str::FromStr;

#[cfg(feature = "evm")]
use super::tx::Eip1559Transaction;

/// Ethereum address wrapper with EIP-55 checksum validation
#[cfg(feature = "evm")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EthAddress(pub Address);

#[cfg(feature = "evm")]
impl EthAddress {
    /// Create a new Ethereum address from a string with checksum validation
    pub fn from_str(s: &str) -> Result<Self, Error> {
        let addr = Address::from_str(s)
            .map_err(|e| Error::Config(format!("Invalid Ethereum address: {}", e)))?;

        // Validate EIP-55 checksum if the address contains uppercase letters
        if s.starts_with("0x") && s[2..].chars().any(|c| c.is_uppercase()) {
            utils::validate_eip55_checksum(s)?;
        }

        Ok(Self(addr))
    }

    /// Create from raw bytes (no checksum validation)
    pub fn from_slice(bytes: &[u8; 20]) -> Self {
        Self(Address::from(bytes))
    }

    /// Get the underlying alloy Address
    pub fn inner(&self) -> &Address {
        &self.0
    }

    /// Convert to checksummed hex string (EIP-55)
    pub fn to_checksummed_string(&self) -> String {
        utils::to_eip55_checksum(self.0)
    }
}

#[cfg(feature = "evm")]
impl std::fmt::Display for EthAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_checksummed_string())
    }
}

#[cfg(feature = "evm")]
impl From<Address> for EthAddress {
    fn from(addr: Address) -> Self {
        Self(addr)
    }
}

#[cfg(feature = "evm")]
impl From<EthAddress> for Address {
    fn from(addr: EthAddress) -> Self {
        addr.0
    }
}

/// EVM transaction request for read-only calls
#[cfg(feature = "evm")]
#[derive(Debug, Clone)]
pub struct EvmCallRequest {
    /// Target contract address
    pub to: EthAddress,
    /// Call data (encoded function call)
    pub data: Vec<u8>,
    /// Block number or tag (latest, pending, etc.)
    pub block: Option<String>,
}

#[cfg(feature = "evm")]
impl EvmCallRequest {
    /// Create a new call request
    pub fn new(to: EthAddress, data: Vec<u8>) -> Self {
        Self {
            to,
            data,
            block: None,
        }
    }

    /// Set the block parameter
    pub fn at_block(mut self, block: String) -> Self {
        self.block = Some(block);
        self
    }
}

/// EVM transaction request for state-changing operations
#[cfg(feature = "evm")]
#[derive(Debug, Clone)]
pub struct EvmTransactionRequest {
    /// Target address (contract or EOA)
    pub to: Option<EthAddress>,
    /// Transaction value in wei
    pub value: U256,
    /// Gas limit
    pub gas_limit: Option<u64>,
    /// Maximum fee per gas (EIP-1559)
    pub max_fee_per_gas: Option<U256>,
    /// Maximum priority fee per gas (EIP-1559)
    pub max_priority_fee_per_gas: Option<U256>,
    /// Transaction data
    pub data: Vec<u8>,
    /// Chain ID for EIP-155 replay protection
    pub chain_id: ChainId,
    /// Explicit nonce to use
    pub nonce: Option<u64>,
    /// Optional access list
    pub access_list: AccessList,
    /// Optional sender address
    pub from: Option<EthAddress>,
}

#[cfg(feature = "evm")]
impl EvmTransactionRequest {
    /// Create a new transaction request
    pub fn new(chain_id: u64) -> Self {
        Self {
            to: None,
            value: U256::ZERO,
            gas_limit: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            data: Vec::new(),
            chain_id,
            nonce: None,
            access_list: AccessList::default(),
            from: None,
        }
    }

    /// Set the target address
    pub fn to(mut self, to: EthAddress) -> Self {
        self.to = Some(to);
        self
    }

    /// Set the transaction value
    pub fn value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Set the gas limit
    pub fn gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = Some(gas_limit);
        self
    }

    /// Set EIP-1559 fees
    pub fn eip1559_fees(mut self, max_fee: U256, priority_fee: U256) -> Self {
        self.max_fee_per_gas = Some(max_fee);
        self.max_priority_fee_per_gas = Some(priority_fee);
        self
    }

    /// Set transaction data
    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Set the nonce for the transaction
    pub fn nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Attach an access list
    pub fn access_list(mut self, list: AccessList) -> Self {
        self.access_list = list;
        self
    }

    /// Set the sender address
    pub fn from(mut self, from: EthAddress) -> Self {
        self.from = Some(from);
        self
    }

    /// Convert to an RPC transaction request (optional override for sender).
    pub fn to_rpc_request(&self, override_from: Option<EthAddress>) -> RpcTransactionRequest {
        let mut request = RpcTransactionRequest::default();

        if let Some(from) = override_from.or_else(|| self.from.clone()) {
            request = request.from(from.0);
        }

        if let Some(to) = self.to.clone() {
            request = request.to(to.0);
        }

        request = request.value(self.value);

        if let Some(gas) = self.gas_limit {
            request = request.gas_limit(gas);
        }

        if let Some(max_fee) = self.max_fee_per_gas {
            if let Ok(fee) = u128::try_from(max_fee) {
                request = request.max_fee_per_gas(fee);
            }
        }

        if let Some(priority) = self.max_priority_fee_per_gas {
            if let Ok(tip) = u128::try_from(priority) {
                request = request.max_priority_fee_per_gas(tip);
            }
        }

        if let Some(nonce) = self.nonce {
            request = request.nonce(nonce);
        }

        request.chain_id = Some(self.chain_id);

        if !self.access_list.0.is_empty() {
            request = request.access_list(self.access_list.clone());
        }

        if !self.data.is_empty() {
            request.input = TransactionInput::from(self.data.clone());
        }

        request
    }

    /// Consume and produce an EIP-1559 transaction ready for signing
    pub fn into_eip1559(self) -> Result<Eip1559Transaction, Error> {
        let nonce = self
            .nonce
            .ok_or_else(|| Error::Config("Transaction nonce must be provided".to_string()))?;
        let gas_limit = self
            .gas_limit
            .ok_or_else(|| Error::Config("Gas limit must be provided".to_string()))?;
        let max_fee = self
            .max_fee_per_gas
            .ok_or_else(|| Error::Config("max_fee_per_gas must be provided".to_string()))?;
        let max_priority = self.max_priority_fee_per_gas.ok_or_else(|| {
            Error::Config("max_priority_fee_per_gas must be provided".to_string())
        })?;

        let max_fee: u128 = max_fee
            .try_into()
            .map_err(|_| Error::Config("max_fee_per_gas exceeds u128 range".to_string()))?;
        let max_priority: u128 = max_priority.try_into().map_err(|_| {
            Error::Config("max_priority_fee_per_gas exceeds u128 range".to_string())
        })?;

        let mut tx = Eip1559Transaction::new(self.chain_id, nonce)
            .gas_limit(gas_limit)
            .max_fee_per_gas(max_fee)
            .max_priority_fee_per_gas(max_priority)
            .value(self.value)
            .data(Bytes::from(self.data))
            .access_list(self.access_list);

        if let Some(to) = self.to {
            tx = tx.to(Some(to.0));
        } else {
            tx = tx.to(None);
        }

        Ok(tx)
    }
}

#[cfg(feature = "evm")]
impl From<&Eip1559Transaction> for EvmTransactionRequest {
    fn from(tx: &Eip1559Transaction) -> Self {
        let mut request = EvmTransactionRequest::new(tx.chain_id)
            .value(tx.value)
            .gas_limit(tx.gas_limit)
            .eip1559_fees(
                U256::from(tx.max_fee_per_gas),
                U256::from(tx.max_priority_fee_per_gas),
            )
            .data(tx.data.clone().to_vec());

        request.nonce = Some(tx.nonce);
        request.access_list = tx.access_list.clone();

        if let Some(to) = tx.to {
            request = request.to(EthAddress(to));
        }

        request
    }
}

/// Event log filter for querying blockchain events
#[cfg(feature = "evm")]
#[derive(Debug, Clone)]
pub struct EventFilter {
    /// Contract addresses to filter by (empty for all)
    pub addresses: Vec<EthAddress>,
    /// Event topics to filter by
    pub topics: Vec<Option<B256>>,
    /// Starting block number
    pub from_block: Option<String>,
    /// Ending block number
    pub to_block: Option<String>,
}

#[cfg(feature = "evm")]
impl EventFilter {
    /// Create a new event filter
    pub fn new() -> Self {
        Self {
            addresses: Vec::new(),
            topics: Vec::new(),
            from_block: None,
            to_block: None,
        }
    }

    /// Add contract addresses to filter
    pub fn addresses(mut self, addresses: Vec<EthAddress>) -> Self {
        self.addresses = addresses;
        self
    }

    /// Add event topics to filter
    pub fn topics(mut self, topics: Vec<Option<B256>>) -> Self {
        self.topics = topics;
        self
    }

    /// Set block range
    pub fn block_range(mut self, from: Option<String>, to: Option<String>) -> Self {
        self.from_block = from;
        self.to_block = to;
        self
    }
}

#[cfg(feature = "evm")]
impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// EVM-specific errors
#[cfg(feature = "evm")]
#[derive(Debug, thiserror::Error)]
pub enum EvmError {
    #[error("Invalid Ethereum address: {0}")]
    InvalidAddress(String),

    #[error("ABI encoding/decoding error: {0}")]
    AbiError(String),

    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Transaction failed: {0}")]
    TransactionError(String),

    #[error("Gas estimation failed: {0}")]
    GasEstimationError(String),

    #[error("Insufficient funds for transaction")]
    InsufficientFunds,

    #[error("Transaction reverted: {0}")]
    TransactionReverted(String),
}

#[cfg(feature = "evm")]
impl From<EvmError> for Error {
    fn from(err: EvmError) -> Self {
        Error::Other(format!("EVM error: {}", err))
    }
}

#[cfg(feature = "evm")]
#[derive(Debug, Clone)]
pub struct Eip1559FeeSuggestion {
    pub base_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
}

/// Utility functions for EVM operations
#[cfg(feature = "evm")]
pub mod utils {
    use super::*;

    use tiny_keccak::{Hasher, Keccak};

    /// Derive Ethereum address from a secp256k1 public key
    ///
    /// Takes the uncompressed public key (65 bytes, 0x04 prefix),
    /// removes the prefix, computes Keccak-256 hash, and takes the last 20 bytes.
    pub fn eth_address_from_pubkey_uncompressed(pubkey: &[u8]) -> Result<EthAddress, Error> {
        if pubkey.len() != 65 || pubkey[0] != 0x04 {
            return Err(Error::Config(
                "Invalid uncompressed public key format".to_string(),
            ));
        }

        let mut hasher = Keccak::v256();
        hasher.update(&pubkey[1..]); // Skip the 0x04 prefix
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);

        let mut address_bytes = [0u8; 20];
        address_bytes.copy_from_slice(&hash[12..32]);
        Ok(EthAddress::from_slice(&address_bytes))
    }

    /// Convert address to EIP-55 checksummed format
    pub fn to_eip55_checksum(address: Address) -> String {
        let addr_hex = format!("{:x}", address);

        // Keccak-256 hash of the lowercase hex address
        let mut hasher = Keccak::v256();
        hasher.update(addr_hex.as_bytes());
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);

        let mut checksummed = String::from("0x");
        for (i, ch) in addr_hex.chars().enumerate() {
            // If it's a letter and the corresponding hash byte is >= 8, uppercase it
            if ch.is_alphabetic() {
                let hash_byte = hash[i / 2];
                let nibble = if i % 2 == 0 {
                    hash_byte >> 4
                } else {
                    hash_byte & 0x0f
                };
                if nibble >= 8 {
                    checksummed.push(ch.to_ascii_uppercase());
                } else {
                    checksummed.push(ch);
                }
            } else {
                checksummed.push(ch);
            }
        }

        checksummed
    }

    /// Validate EIP-55 checksum for an Ethereum address
    pub fn validate_eip55_checksum(address: &str) -> Result<(), Error> {
        if !address.starts_with("0x") || address.len() != 42 {
            return Err(Error::Config("Invalid Ethereum address format".to_string()));
        }

        // Parse the address to get the raw Address
        let addr = Address::from_str(address)
            .map_err(|e| Error::Config(format!("Invalid Ethereum address: {}", e)))?;

        // Generate the correct checksum
        let correct_checksum = to_eip55_checksum(addr);

        // Compare with the provided address
        if address != correct_checksum {
            return Err(Error::Config(format!(
                "Invalid EIP-55 checksum. Expected: {}, got: {}",
                correct_checksum, address
            )));
        }

        Ok(())
    }

    /// Convert wei to ether with exact precision (returns string representation)
    ///
    /// This function converts wei to ether without precision loss by using
    /// string representation. For display or financial calculations requiring
    /// exact precision, this is the recommended method.
    ///
    /// # Example
    /// ```ignore
    /// let wei = U256::from(1234567890123456789u64);
    /// let ether = wei_to_ether_string(wei);
    /// assert_eq!(ether, "1.234567890123456789");
    /// ```
    pub fn wei_to_ether_string(wei: U256) -> String {
        let wei_multiplier = U256::from(10u64).pow(U256::from(18u64));
        let ether_int = wei / wei_multiplier;
        let remainder_wei = wei % wei_multiplier;

        if remainder_wei.is_zero() {
            // No fractional part
            ether_int.to_string()
        } else {
            // Format with fractional part, padding with zeros
            format!("{}.{:018}", ether_int, remainder_wei)
        }
    }

    /// Convert wei to ether (f64 - may lose precision for large values)
    ///
    /// **Warning**: This function uses f64 which only has 53 bits of precision.
    /// For values requiring exact precision, use `wei_to_ether_string` instead.
    ///
    /// This is provided for backwards compatibility and simple display purposes only.
    #[deprecated(note = "Use wei_to_ether_string for exact precision")]
    pub fn wei_to_ether(wei: U256) -> f64 {
        // Use string conversion to avoid precision issues with large numbers
        match wei_to_ether_string(wei).parse::<f64>() {
            Ok(value) => value,
            Err(_) => {
                // Fallback for very large numbers that exceed f64 range
                let wei_multiplier = U256::from(10u64).pow(U256::from(18u64));
                let ether_int = wei / wei_multiplier;
                ether_int.to_string().parse::<f64>().unwrap_or(f64::MAX)
            }
        }
    }

    /// Convert ether to wei using proper decimal arithmetic
    ///
    /// This function safely converts ether amounts to wei without precision loss
    /// by treating the input as a decimal string and performing integer arithmetic.
    pub fn ether_to_wei(ether: &str) -> Result<U256, Error> {
        // Parse the ether string as a decimal
        let ether_str = ether.trim();

        // Find the decimal point
        if let Some(dot_pos) = ether_str.find('.') {
            let integer_part = &ether_str[..dot_pos];
            let decimal_part = &ether_str[dot_pos + 1..];

            // Limit decimal places to 18 (wei precision)
            let decimal_part = if decimal_part.len() > 18 {
                &decimal_part[..18]
            } else {
                decimal_part
            };

            // Parse integer and decimal parts
            let int_value: U256 = if integer_part.is_empty() || integer_part == "0" {
                U256::ZERO
            } else {
                U256::from_str(integer_part).map_err(|_| {
                    Error::Config(format!(
                        "Invalid integer part in ether amount: {}",
                        integer_part
                    ))
                })?
            };

            let dec_value: U256 = if decimal_part.is_empty() {
                U256::ZERO
            } else {
                U256::from_str(decimal_part).map_err(|_| {
                    Error::Config(format!(
                        "Invalid decimal part in ether amount: {}",
                        decimal_part
                    ))
                })?
            };

            // Calculate wei: integer_part * 10^18 + decimal_part * 10^(18 - decimal_digits)
            let wei_multiplier = U256::from(10u64).pow(U256::from(18u64));
            let decimal_multiplier =
                U256::from(10u64).pow(U256::from(18u64 - decimal_part.len() as u64));

            let wei_from_int = int_value * wei_multiplier;
            let wei_from_dec = dec_value * decimal_multiplier;

            Ok(wei_from_int + wei_from_dec)
        } else {
            // No decimal point, treat as integer
            let int_value = U256::from_str(ether_str)
                .map_err(|_| Error::Config(format!("Invalid ether amount: {}", ether_str)))?;
            let wei_multiplier = U256::from(10u64).pow(U256::from(18u64));
            Ok(int_value * wei_multiplier)
        }
    }
}

#[cfg(not(feature = "evm"))]
/// Stub types when EVM feature is not enabled
pub mod types {
    use crate::error::Error;

    #[derive(Debug)]
    pub struct EthAddress;

    impl EthAddress {
        pub fn from_str(_s: &str) -> Result<Self, Error> {
            Err(Error::Config("EVM feature not enabled".to_string()))
        }
    }
}

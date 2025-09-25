#[cfg(feature = "evm")]
use crate::error::Error;
/// EVM-specific types and utilities for MANTRA SDK
///
/// This module provides type definitions and utilities for working with
/// Ethereum Virtual Machine (EVM) compatible blockchains, including
/// address handling, transaction types, and error definitions.

#[cfg(feature = "evm")]
use alloy_primitives::{Address, B256, U256};
#[cfg(feature = "evm")]
use alloy_sol_types::SolCall;
#[cfg(feature = "evm")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "evm")]
use std::str::FromStr;

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
        // TODO: Add EIP-55 checksum validation
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

    /// Convert to checksummed hex string
    pub fn to_checksummed_string(&self) -> String {
        // TODO: Implement EIP-55 checksum
        format!("{:?}", self.0)
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
    pub chain_id: u64,
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

/// Utility functions for EVM operations
#[cfg(feature = "evm")]
pub mod utils {
    use super::*;
    use k256::ecdsa::SigningKey;
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

    /// Validate EIP-55 checksum for an Ethereum address
    pub fn validate_eip55_checksum(address: &str) -> Result<(), Error> {
        // TODO: Implement EIP-55 checksum validation
        // For now, just check basic format
        if !address.starts_with("0x") || address.len() != 42 {
            return Err(Error::Config("Invalid Ethereum address format".to_string()));
        }
        Ok(())
    }

    /// Convert wei to ether (for display purposes)
    ///
    /// This function converts wei to ether with proper decimal handling.
    /// Note: For display purposes, f64 provides sufficient precision for most use cases.
    /// For financial calculations requiring exact precision, use string-based representations.
    pub fn wei_to_ether(wei: U256) -> f64 {
        // Use U256 division for better precision
        let wei_multiplier = U256::from(10u64).pow(U256::from(18u64));
        let ether_wei = wei / wei_multiplier;
        let remainder_wei = wei % wei_multiplier;

        // Convert to f64 with decimal precision
        let ether_int = ether_wei.to_string().parse::<f64>().unwrap_or(0.0);
        let remainder = remainder_wei.to_string().parse::<f64>().unwrap_or(0.0);
        let remainder_divisor = 1_000_000_000_000_000_000.0; // 10^18

        ether_int + (remainder / remainder_divisor)
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

use cosmrs::rpc::endpoint::broadcast::tx_sync::Response as SyncTxResponse;
use thiserror::Error;

/// SDK Error type for MANTRA DEX SDK v3.0.0
///
/// This enum represents all possible errors that can occur when using the SDK.
/// v3.0.0 introduces new error types for enhanced fee validation and pool status checking.
#[derive(Error, Debug)]
pub enum Error {
    /// Error when interacting with CosmRS
    #[error("CosmRS error: {0}")]
    CosmRs(#[from] cosmrs::Error),

    /// RPC client error - occurs when communication with the blockchain fails
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Transaction broadcast error with response - occurs when transaction submission fails
    #[error("Transaction broadcast error: {0:?}")]
    TxBroadcast(SyncTxResponse),

    /// Transaction simulation error - occurs when transaction simulation fails
    #[error("Transaction simulation error: {0}")]
    TxSimulation(String),

    /// Wallet error - occurs when wallet operations fail or wallet is not configured
    #[error("Wallet error: {0}")]
    Wallet(String),

    /// Configuration error - occurs when network or contract configuration is invalid
    #[error("Configuration error: {0}")]
    Config(String),

    /// Contract interaction error - occurs when smart contract calls fail
    #[error("Contract error: {0}")]
    Contract(String),

    /// Serialization/Deserialization error - occurs when JSON parsing fails
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error - occurs when file system operations fail
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Fee validation error - **v3.0.0 New**: occurs when pool fees exceed 20% total limit
    ///
    /// This error is thrown when:
    /// - Total pool fees (protocol_fee + swap_fee + burn_fee + extra_fees) exceed 20%
    /// - Individual fees are negative
    /// - Fee structure validation fails
    #[error("Fee validation error: {0}")]
    FeeValidation(String),

    /// Other errors - generic error type for miscellaneous failures
    #[error("{0}")]
    Other(String),

    /// Transaction error - occurs when transaction execution fails
    #[error("Transaction error: {0}")]
    Tx(String),

    /// Network error - occurs when network connectivity issues arise
    #[error("Network error: {0}")]
    Network(String),

    /// Timeout error - occurs when operations exceed their timeout limit
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// Not implemented error - occurs when a feature is not yet implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Wallet not set error - occurs when trying to execute a transaction without a wallet
    #[error("Wallet not set - cannot execute transaction without a wallet")]
    WalletNotSet,

    /// Skip protocol error - occurs when Skip API or cross-chain operations fail
    #[error("Skip protocol error: {0}")]
    Skip(String),

    /// EVM protocol error - occurs when EVM blockchain operations fail
    #[error("EVM error: {0}")]
    Evm(String),
}

/// EVM Contract interfaces and helpers
///
/// This module provides type-safe interfaces for interacting with smart contracts
/// on EVM-compatible blockchains using the Alloy sol! macro.
///
/// # Available Contracts
///
/// - **ERC-20**: Standard fungible token interface (transfer, approve, mint, etc.)
/// - **ERC-721**: Standard NFT interface
/// - **PrimarySale**: MANTRA RWA token sale contract (v2.0)
/// - **Allowlist**: KYC/AML compliance management for PrimarySale
/// - **Custom**: Generic custom contract interactions

#[cfg(feature = "evm")]
pub mod allowlist;
#[cfg(feature = "evm")]
pub mod custom;
#[cfg(feature = "evm")]
pub mod erc20;
#[cfg(feature = "evm")]
pub mod erc721;
#[cfg(feature = "evm")]
pub mod primary_sale;

// Re-export commonly used types
#[cfg(feature = "evm")]
pub use allowlist::IAllowlist;
#[cfg(feature = "evm")]
pub use custom::CustomContract;
#[cfg(feature = "evm")]
pub use erc20::{Erc20, IERC20};
#[cfg(feature = "evm")]
pub use erc721::{Erc721, IERC721};
#[cfg(feature = "evm")]
pub use primary_sale::{IPrimarySale, PrimarySale};

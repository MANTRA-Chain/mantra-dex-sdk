#[cfg(feature = "evm")]
pub mod custom;
/// Contract helpers for common EVM contract patterns
///
/// This module provides high-level helpers for interacting with
/// standard contract interfaces like ERC-20, ERC-721, and custom contracts.

#[cfg(feature = "evm")]
pub mod erc20;
#[cfg(feature = "evm")]
pub mod erc721;

#[cfg(feature = "evm")]
pub use custom::CustomContract;
#[cfg(feature = "evm")]
pub use erc20::Erc20;
#[cfg(feature = "evm")]
pub use erc721::Erc721;

pub mod client;
pub mod config;
pub mod error;
pub mod protocols;
pub mod wallet;

// DEX TUI module - optional via "tui-dex" feature
#[cfg(feature = "tui-dex")]
pub mod tui_dex;

// MCP module - optional via "mcp" feature
#[cfg(feature = "mcp")]
pub mod mcp;
// Re-export mantra-dex-std for user convenience
pub use mantra_dex_std;

// Main client exports
pub use client::{ConfigurationChanges, MantraClient, MantraClientBuilder};
pub use config::{MantraNetworkConfig, NetworkConstants};
pub use error::Error;
pub use wallet::MantraWallet;

// Protocol exports
pub use protocols::{Protocol, ProtocolRegistry};

// DEX protocol exports
pub use protocols::dex::{DexProtocol, MantraDexClient};

// Skip protocol exports
pub use protocols::skip::{
    SimulateSmartSwapExactAssetInResponse, SimulateSwapExactAssetInResponse,
    SimulateSwapExactAssetOutResponse, SkipAction, SkipAffiliate, SkipAsset, SkipIbcInfo,
    SkipProtocol, SkipRoute, SkipSwap, SkipSwapExactAssetIn, SkipSwapExactAssetOut,
    SkipSwapOperation,
};

// ClaimDrop protocol exports
pub use protocols::claimdrop::{
    AggregatedRewards, Allocation, AllocationsResponse, BlacklistAction, CampaignAction,
    CampaignInfo, CampaignParams, CampaignStats, CampaignsResponse, ClaimParams, ClaimdropClient,
    ClaimdropFactoryClient, ClaimdropOperationResult, ClaimdropProtocol, UserRewards,
    UserRewardsResponse,
};

// Re-export DEX TUI entry point when feature is enabled
#[cfg(feature = "tui-dex")]
pub use tui_dex::run_tui;

// Re-export MCP server types when feature is enabled
#[cfg(feature = "mcp")]
pub use mcp::{
    create_http_server, create_mcp_server, create_stdio_server, ConnectionPoolConfig,
    MantraDexMcpServer, McpResult, McpSdkAdapter, McpServerConfig, McpServerError, MCP_SERVER_NAME,
    MCP_SERVER_VERSION,
};

// Re-export common types from mantra-dex-std
pub use cosmwasm_std::{Coin, Decimal, Uint128};
pub use mantra_dex_std::{
    fee::PoolFee,
    pool_manager::{FeatureToggle, PoolInfo, PoolType, SwapOperation},
};

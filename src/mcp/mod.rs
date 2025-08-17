//! Mantra DEX SDK Model Context Protocol (MCP) Server
//!
//! This module provides a complete MCP server implementation for the Mantra DEX SDK,
//! enabling AI agents and other MCP clients to interact with the Mantra blockchain DEX
//! through a standardized protocol.
//!
//! ## Features
//!
//! - **Wallet Management**: Create, import, and manage HD wallets
//! - **Pool Operations**: Query and manage liquidity pools
//! - **Trading Operations**: Execute swaps and multi-hop operations
//! - **Liquidity Operations**: Provide and withdraw liquidity
//! - **Rewards Management**: Query and claim rewards
//! - **Network Operations**: Switch between networks and query status
//!
//! ## Architecture
//!
//! The MCP server uses a modular architecture with the following components:
//!
//! - **Server Core**: Main MCP server implementation with transport support
//! - **Tools**: MCP tools for executing DEX operations
//! - **Resources**: MCP resources for accessing DEX data
//! - **State Management**: Centralized state management with caching
//! - **Error Handling**: Comprehensive error mapping and handling
//!
//! ## Usage
//!
//! ```rust
//! use mantra_dex_sdk::mcp::{McpServerConfig, create_stdio_server};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let config = McpServerConfig::default();
//!     let _server = create_stdio_server(config).await?;
//!
//!     // Server is now initialized and ready for MCP communication
//!     println!("MCP Server initialized successfully");
//!     Ok(())
//! }
//! ```

// Core server implementation
pub mod server;

// SDK integration layer
pub mod sdk_adapter;

// MCP client wrapper
pub mod client_wrapper;

// Re-export main types for easy access
pub use server::{
    create_http_server, create_mcp_server, create_stdio_server, MantraDexMcpServer, McpResult,
    McpServerConfig, McpServerError, McpServerStateData,
};

// Re-export SDK adapter types
pub use sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};

// Re-export client wrapper types
pub use client_wrapper::McpClientWrapper;

// TODO: Add these modules as they are implemented in subsequent tasks
// pub mod tools;
// pub mod resources;
// pub mod config;
// pub mod error;
// pub mod utils;

/// MCP server version
pub const MCP_SERVER_VERSION: &str = "0.1.0";

/// Default MCP server name
pub const MCP_SERVER_NAME: &str = "Mantra DEX SDK MCP Server";

/// Supported MCP protocol versions
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26"];

pub mod logging;

pub use logging::{
    configure_tracing_subscriber, get_default_log_filter, get_mcp_specific_filter, setup_logging,
    LogFormat, LogLevel, LoggingConfig, LoggingMetrics, McpLogger, RequestSpan,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test that main types are properly exported
        let config = McpServerConfig::default();
        assert_eq!(config.name, MCP_SERVER_NAME);
        assert_eq!(config.version, MCP_SERVER_VERSION);
    }
}

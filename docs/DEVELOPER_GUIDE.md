# MANTRA SDK Developer Guide

This guide provides comprehensive information for developers working with or contributing to the MANTRA SDK. It covers architecture, development patterns, testing strategies, and extension mechanisms.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Design Principles](#design-principles)
- [Adding New Protocols](#adding-new-protocols)
- [Extending MCP Tools](#extending-mcp-tools)
- [Testing Strategies](#testing-strategies)
- [Code Organization](#code-organization)
- [Contributing Guidelines](#contributing-guidelines)
- [Performance Considerations](#performance-considerations)
- [Security Guidelines](#security-guidelines)

## Architecture Overview

### High-Level Architecture

The MANTRA SDK follows a modular, protocol-agnostic architecture that enables easy extension and maintenance:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Application Layer                        │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   TUI Client    │  │   MCP Server    │  │  Direct Usage   │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                 │
┌─────────────────────────────────────────────────────────────────┐
│                         Core SDK Layer                         │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                   MantraClient                              │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │  │
│  │  │   Wallet    │  │   Config    │  │  Protocol Registry  │  │  │
│  │  │ Management  │  │ Management  │  │                     │  │  │
│  │  └─────────────┘  └─────────────┘  └─────────────────────┘  │  │
│  └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                 │
┌─────────────────────────────────────────────────────────────────┐
│                      Protocol Layer                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │  DEX Protocol   │  │ClaimDrop Protocol│  │ Skip Protocol   │  │
│  │   28 MCP tools  │  │   5 MCP tools   │  │   6 MCP tools   │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                 │
┌─────────────────────────────────────────────────────────────────┐
│                    Blockchain Layer                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ MANTRA Chain    │  │ Cosmos Hub      │  │  Other Chains   │  │
│  │ (cosmrs)        │  │ (IBC/Skip)      │  │  (via Skip)     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. MantraClient (`src/client.rs`)

The central orchestrator that provides unified access to all protocols:

- **Protocol Management**: Manages protocol instances and their lifecycle
- **Configuration Integration**: Unifies legacy and modern configuration systems
- **Connection Pooling**: Optimizes RPC connections across protocols
- **Error Handling**: Provides consistent error handling across the SDK

#### 2. Protocol System (`src/protocols/`)

Each protocol implements the common `Protocol` trait:

```rust
#[async_trait]
pub trait Protocol: Send + Sync {
    fn name(&self) -> &'static str;
    fn version(&self) -> &'static str;
    async fn is_available(&self, rpc_client: &HttpClient) -> Result<bool, Error>;
    fn get_config(&self) -> Result<Value, Error>;
    async fn initialize(&mut self, rpc_client: Arc<HttpClient>) -> Result<(), Error>;
}
```

**Current Protocols:**
- **DEX Protocol** (`protocols/dex/`): Core trading, liquidity, and farming
- **ClaimDrop Protocol** (`protocols/claimdrop/`): Airdrop campaign management
- **Skip Protocol** (`protocols/skip/`): Cross-chain routing and bridging

#### 3. Configuration System (`src/config/`)

Modular configuration architecture supporting:
- **Contract Management** (`config/contracts.rs`): Contract addresses per network
- **Protocol Settings** (`config/protocols.rs`): Protocol-specific parameters
- **Environment Config** (`config/env.rs`): Environment variables and defaults

#### 4. MCP Integration (`src/mcp/`)

Model Context Protocol implementation for AI integration:
- **Server** (`mcp/server.rs`): JSON-RPC 2.0 compliant server
- **Adapter** (`mcp/sdk_adapter.rs`): Protocol-agnostic tool routing
- **Transport Support**: Both STDIO and HTTP transports

## Design Principles

### 1. Modularity

Each protocol is self-contained and can be developed independently:

```rust
// Protocol independence
let dex_client = client.dex();     // DEX-specific operations
let skip_client = client.skip();   // Skip-specific operations
```

### 2. Async-First

All I/O operations use async/await for optimal performance:

```rust
// All blockchain operations are async
let pools = client.dex().get_pools(None, None).await?;
let balances = client.dex().get_balances(&address).await?;
```

### 3. Error Transparency

Comprehensive error handling with specific error types:

```rust
match result {
    Err(Error::InsufficientBalance { required, available }) => {
        // Handle specific error case
    }
    Err(Error::SlippageExceeded { expected, actual }) => {
        // Handle slippage error
    }
    _ => {}
}
```

### 4. Configuration Flexibility

Multiple configuration approaches supported:

```rust
// Modern configuration
let config = ConfigurationManager::load_from_files("config/")?;
let client = MantraClient::new_with_config(config).await?;

// Legacy configuration
let network_config = MantraNetworkConfig::testnet();
let client = MantraClientBuilder::new()
    .with_network_config(network_config)
    .build().await?;
```

## Adding New Protocols

### Step 1: Define Protocol Structure

Create a new protocol module in `src/protocols/`:

```rust
// src/protocols/my_protocol/mod.rs
pub mod client;
pub mod types;

pub use client::MyProtocolClient;

use crate::error::Error;
use crate::protocols::Protocol;
use async_trait::async_trait;
use cosmrs::rpc::HttpClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct MyProtocol {
    initialized: bool,
    config: MyProtocolConfig,
}

impl MyProtocol {
    pub fn new(config: MyProtocolConfig) -> Self {
        Self {
            initialized: false,
            config,
        }
    }
}

#[async_trait]
impl Protocol for MyProtocol {
    fn name(&self) -> &'static str {
        "my_protocol"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    async fn is_available(&self, rpc_client: &HttpClient) -> Result<bool, Error> {
        // Check if protocol contracts are available
        Ok(self.initialized)
    }

    fn get_config(&self) -> Result<serde_json::Value, Error> {
        Ok(serde_json::json!({
            "name": self.name(),
            "version": self.version(),
            "initialized": self.initialized,
        }))
    }

    async fn initialize(&mut self, rpc_client: Arc<HttpClient>) -> Result<(), Error> {
        // Initialize protocol (load contracts, verify connectivity, etc.)
        self.initialized = true;
        Ok(())
    }
}
```

### Step 2: Implement Client Logic

```rust
// src/protocols/my_protocol/client.rs
use crate::error::Error;
use crate::wallet::MantraWallet;
use cosmrs::rpc::HttpClient;
use std::sync::Arc;

pub struct MyProtocolClient {
    rpc_client: Arc<HttpClient>,
    contract_address: String,
}

impl MyProtocolClient {
    pub fn new(rpc_client: Arc<HttpClient>, contract_address: String) -> Self {
        Self {
            rpc_client,
            contract_address,
        }
    }

    pub async fn my_operation(&self, param: String) -> Result<MyOperationResult, Error> {
        // Implement protocol-specific operations
        todo!()
    }

    pub async fn query_data(&self) -> Result<Vec<MyData>, Error> {
        // Implement queries
        todo!()
    }
}
```

### Step 3: Define Types

```rust
// src/protocols/my_protocol/types.rs
use cosmwasm_std::Uint128;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyData {
    pub id: u64,
    pub value: String,
    pub amount: Uint128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyOperationResult {
    pub txhash: String,
    pub data: Option<MyData>,
}
```

### Step 4: Integrate with MantraClient

Update `src/client.rs`:

```rust
// Add to MantraClient struct
my_protocol: Option<MyProtocol>,

// Add accessor method
impl MantraClient {
    pub fn my_protocol(&self) -> MyProtocolClient {
        MyProtocolClient::new(
            self.rpc_client.clone(),
            self.config_manager.get_contract_address(
                &self.network_config.chain_id,
                &ContractType::MyProtocol
            ).unwrap_or_default()
        )
    }
}
```

### Step 5: Update Exports

Update `src/lib.rs`:

```rust
// Add to protocol exports
pub use protocols::my_protocol::{MyProtocol, MyProtocolClient, MyData, MyOperationResult};
```

## Extending MCP Tools

### Step 1: Define Tool Schema

Add to `src/mcp/sdk_adapter.rs`:

```rust
fn create_my_tool() -> Tool {
    Tool {
        name: "my_tool".to_string(),
        description: Some("Description of what this tool does".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "param1": {
                    "type": "string",
                    "description": "Description of param1"
                },
                "param2": {
                    "type": "number",
                    "description": "Optional numeric parameter",
                    "minimum": 0
                }
            },
            "required": ["param1"]
        }),
    }
}
```

### Step 2: Implement Tool Handler

```rust
async fn handle_my_tool(&self, arguments: &Value) -> Result<Vec<TextContent>, McpError> {
    // Extract and validate parameters
    let param1 = arguments["param1"].as_str()
        .ok_or_else(|| McpError::InvalidParams("param1 is required".to_string()))?;
    
    let param2 = arguments["param2"].as_f64().unwrap_or(0.0);
    
    // Validate parameters
    if param1.is_empty() {
        return Err(McpError::InvalidParams("param1 cannot be empty".to_string()));
    }
    
    // Use SDK client for actual operation
    let result = self.client.my_protocol().my_operation(param1.to_string()).await
        .map_err(|e| McpError::InternalError(format!("Operation failed: {}", e)))?;
    
    // Format response
    let response = json!({
        "success": true,
        "result": result,
        "tool": "my_tool",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    Ok(vec![TextContent::text(serde_json::to_string_pretty(&response)?)])
}
```

### Step 3: Register Tool

```rust
// In list_tools() method
tools.push(self.create_my_tool());

// In call_tool() method
"my_tool" => self.handle_my_tool(arguments).await,
```

### Step 4: Add Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_my_tool() {
        let adapter = create_test_adapter().await;
        
        let args = json!({
            "param1": "test_value",
            "param2": 42
        });
        
        let result = adapter.handle_my_tool(&args).await;
        assert!(result.is_ok());
    }
}
```

## Testing Strategies

### Unit Testing

Test individual components in isolation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_protocol_initialization() {
        let mut protocol = MyProtocol::new(default_config());
        let rpc_client = create_mock_rpc_client();
        
        let result = protocol.initialize(rpc_client).await;
        assert!(result.is_ok());
        assert!(protocol.is_available(&mock_client).await.unwrap());
    }
}
```

### Integration Testing

Test protocol interactions:

```rust
// tests/integration/my_protocol.rs
use mantra_sdk::*;

#[tokio::test]
async fn test_my_protocol_workflow() {
    let client = create_test_client().await;
    
    // Test basic operations
    let data = client.my_protocol().query_data().await.unwrap();
    assert!(!data.is_empty());
    
    // Test complex workflow
    let result = client.my_protocol().my_operation("test".to_string()).await.unwrap();
    assert!(!result.txhash.is_empty());
}
```

### MCP Tool Testing

Test MCP integration:

```rust
#[tokio::test]
async fn test_mcp_my_tool() {
    let adapter = create_test_adapter().await;
    
    let args = json!({"param1": "test"});
    let result = adapter.handle_my_tool(&args).await.unwrap();
    
    assert!(!result.is_empty());
    let response: Value = serde_json::from_str(&result[0].text).unwrap();
    assert_eq!(response["success"], true);
}
```

### Property-Based Testing

Use property-based testing for complex logic:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_amount_calculations(amount in 1u128..1_000_000_000u128) {
        let result = calculate_fees(Uint128::from(amount));
        prop_assert!(result.unwrap() <= Uint128::from(amount));
    }
}
```

## Code Organization

### Directory Structure

```
src/
├── lib.rs                 # Public API exports
├── client.rs              # Main MantraClient
├── error.rs               # Error types
├── config/                # Configuration system
│   ├── mod.rs
│   ├── contracts.rs       # Contract management
│   ├── protocols.rs       # Protocol settings
│   └── env.rs            # Environment config
├── protocols/             # Protocol implementations
│   ├── mod.rs
│   ├── dex/              # DEX protocol
│   ├── claimdrop/        # ClaimDrop protocol
│   └── skip/             # Skip protocol
├── wallet/                # Wallet management
│   ├── mod.rs
│   └── storage.rs
├── mcp/                   # MCP server
│   ├── mod.rs
│   ├── server.rs         # MCP server implementation
│   └── sdk_adapter.rs    # Protocol adapter
└── tui_dex/              # TUI implementation
```

### Naming Conventions

- **Types**: PascalCase (`MantraClient`, `PoolInfo`)
- **Functions**: snake_case (`get_pools`, `execute_swap`)
- **Constants**: SCREAMING_SNAKE_CASE (`MAX_SLIPPAGE`, `DEFAULT_TIMEOUT`)
- **Modules**: snake_case (`claimdrop`, `sdk_adapter`)

### Import Organization

```rust
// Standard library imports
use std::collections::HashMap;
use std::sync::Arc;

// External crate imports
use anyhow::Result;
use async_trait::async_trait;
use cosmwasm_std::{Coin, Uint128};
use serde::{Deserialize, Serialize};
use tokio;

// Internal imports (grouped by module)
use crate::config::{ConfigurationManager, ContractType};
use crate::error::Error;
use crate::protocols::{Protocol, ProtocolRegistry};
use crate::wallet::MantraWallet;
```

## Contributing Guidelines

### Development Workflow

1. **Fork and Clone**
   ```bash
   git clone https://github.com/your-fork/mantra-dex-sdk.git
   cd mantra-dex-sdk
   ```

2. **Create Feature Branch**
   ```bash
   git checkout -b feature/my-new-feature
   ```

3. **Development Setup**
   ```bash
   # Install dependencies
   cargo build
   
   # Run tests
   cargo test
   
   # Check formatting
   cargo fmt --check
   
   # Run clippy
   cargo clippy -- -D warnings
   ```

4. **Make Changes**
   - Follow existing patterns and conventions
   - Add comprehensive tests
   - Update documentation
   - Ensure all tests pass

5. **Commit and Push**
   ```bash
   git add .
   git commit -m "feat: add new protocol support"
   git push origin feature/my-new-feature
   ```

6. **Create Pull Request**
   - Provide clear description
   - Reference related issues
   - Ensure CI passes

### Code Standards

#### Error Handling

Use specific error types:

```rust
// Good
match client.execute_swap(params).await {
    Err(Error::InsufficientBalance { required, available }) => {
        // Handle specific case
    }
    Err(e) => return Err(e),
    Ok(result) => result,
}

// Avoid generic errors
match client.execute_swap(params).await {
    Err(e) => panic!("Something went wrong: {}", e), // Bad
    Ok(result) => result,
}
```

#### Async Patterns

Use proper async patterns:

```rust
// Good - concurrent execution
let (pools, balances) = tokio::try_join!(
    client.get_pools(None, None),
    client.get_balances(&address)
)?;

// Avoid sequential when concurrent is possible
let pools = client.get_pools(None, None).await?; // Could be concurrent
let balances = client.get_balances(&address).await?;
```

#### Documentation

Document all public APIs:

```rust
/// Executes a token swap with slippage protection
/// 
/// # Arguments
/// 
/// * `offer_asset` - The asset to swap from
/// * `ask_asset_denom` - The denomination to swap to
/// * `slippage_tolerance` - Maximum acceptable slippage (optional)
/// * `wallet` - Wallet to sign the transaction
/// 
/// # Returns
/// 
/// Returns the transaction hash and swap details
/// 
/// # Errors
/// 
/// * `Error::InsufficientBalance` - If wallet lacks required tokens
/// * `Error::SlippageExceeded` - If actual slippage exceeds tolerance
/// * `Error::PoolNotFound` - If no pool exists for the asset pair
/// 
/// # Example
/// 
/// ```
/// let result = client.execute_swap(
///     Coin { denom: "uom".to_string(), amount: Uint128::from(1000000u128) },
///     "uusdc".to_string(),
///     Some(Decimal::percent(1)),
///     &wallet
/// ).await?;
/// ```
pub async fn execute_swap(
    &self,
    offer_asset: Coin,
    ask_asset_denom: String,
    slippage_tolerance: Option<Decimal>,
    wallet: &MantraWallet,
) -> Result<SwapResult, Error> {
    // Implementation
}
```

### Testing Requirements

#### Test Coverage

- **Unit Tests**: All public methods must have unit tests
- **Integration Tests**: Protocol workflows must have integration tests
- **MCP Tests**: All MCP tools must have tests
- **Error Tests**: Error conditions must be tested

#### Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    mod unit_tests {
        use super::*;
        
        #[test]
        fn test_individual_function() {
            // Test single function in isolation
        }
    }
    
    mod integration_tests {
        use super::*;
        
        #[tokio::test]
        async fn test_workflow() {
            // Test complete workflows
        }
    }
    
    mod error_tests {
        use super::*;
        
        #[tokio::test]
        async fn test_error_conditions() {
            // Test error handling
        }
    }
}
```

## Performance Considerations

### Connection Management

Use connection pooling for optimal performance:

```rust
// Good - reuse connections
let client = MantraClientBuilder::new()
    .with_connection_pool_config(ConnectionPoolConfig {
        max_connections_per_network: 10,
        connection_timeout_secs: 30,
        ..Default::default()
    })
    .build().await?;

// Avoid creating new clients repeatedly
for operation in operations {
    let result = client.execute_operation(operation).await?; // Reuses connection
}
```

### Batch Operations

Implement batch operations where possible:

```rust
// Good - batch multiple operations
let results = client.batch_execute(operations).await?;

// Avoid individual calls when batch is available
for operation in operations {
    let result = client.execute_operation(operation).await?; // Less efficient
}
```

### Caching Strategies

Implement appropriate caching:

```rust
// Cache expensive operations
#[cached(time = 300)] // 5 minutes
async fn get_pools_cached(&self) -> Result<Vec<PoolInfo>, Error> {
    self.get_pools_uncached().await
}
```

## Security Guidelines

### Private Key Handling

Never expose private keys:

```rust
// Good - keys stay in memory
let wallet = MantraWallet::from_mnemonic(mnemonic, prefix, index)?;
let signature = wallet.sign_tx(tx)?; // Key never leaves wallet

// Never log or return private keys
log::debug!("Wallet address: {}", wallet.address()); // OK
log::debug!("Private key: {:?}", wallet.private_key()); // NEVER DO THIS
```

### Input Validation

Validate all inputs:

```rust
pub async fn execute_swap(
    &self,
    offer_asset: Coin,
    ask_asset_denom: String,
    slippage_tolerance: Option<Decimal>,
    wallet: &MantraWallet,
) -> Result<SwapResult, Error> {
    // Validate inputs
    if offer_asset.amount.is_zero() {
        return Err(Error::InvalidAmount("Amount cannot be zero".to_string()));
    }
    
    if ask_asset_denom.is_empty() {
        return Err(Error::InvalidDenom("Asset denomination cannot be empty".to_string()));
    }
    
    if let Some(slippage) = slippage_tolerance {
        if slippage > Decimal::percent(50) {
            return Err(Error::InvalidSlippage("Slippage cannot exceed 50%".to_string()));
        }
    }
    
    // Proceed with validated inputs
    self.execute_swap_internal(offer_asset, ask_asset_denom, slippage_tolerance, wallet).await
}
```

### Environment Variables

Handle sensitive configuration securely:

```rust
// Good - secure environment handling
fn load_mnemonic() -> Option<String> {
    std::env::var("WALLET_MNEMONIC").ok()
        .filter(|m| !m.is_empty())
        .map(|m| {
            // Clear from environment after reading
            std::env::remove_var("WALLET_MNEMONIC");
            m
        })
}

// Never hardcode secrets
const WALLET_MNEMONIC: &str = "abandon abandon..."; // NEVER DO THIS
```

This developer guide provides the foundation for extending and contributing to the MANTRA SDK. For specific implementation details, refer to the [API Documentation](api/README.md) and examine existing protocol implementations as reference patterns.
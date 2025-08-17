# MANTRA SDK API Documentation

A comprehensive Rust SDK for interacting with the MANTRA blockchain ecosystem, supporting DEX, ClaimDrop, Skip Protocol, and more. This SDK provides both programmatic access through a Rust library and AI-agent access through an MCP (Model Context Protocol) server.

## Table of Contents

- [Getting Started](#getting-started)
- [Installation](#installation)
- [Quick Setup](#quick-setup)
- [Core Client API](#core-client-api)
- [DEX Protocol API](#dex-protocol-api)
- [ClaimDrop Protocol API](#claimdrop-protocol-api)
- [Skip Protocol API](#skip-protocol-api)
- [Wallet Management](#wallet-management)
- [Configuration Management](#configuration-management)
- [MCP Server Setup](#mcp-server-setup)
- [Error Handling](#error-handling)
- [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

Add the MANTRA SDK to your `Cargo.toml`:

```toml
[dependencies]
mantra-sdk = "0.1.0"

# Optional features
# mantra-sdk = { version = "0.1.0", features = ["mcp", "tui-dex"] }
```

### Available Features

- **Default**: Core SDK functionality only
- **`mcp`**: Enables MCP server for AI integration (39 tools across all protocols)
- **`tui-dex`**: Enables Terminal User Interface

### Quick Setup

```rust
use mantra_sdk::{MantraClient, MantraClientBuilder, MantraNetworkConfig};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client for testnet
    let network_config = MantraNetworkConfig::testnet();
    let client = MantraClientBuilder::new()
        .with_network_config(network_config)
        .build()
        .await?;

    // Get all pools
    let pools = client.dex().get_pools(None, None).await?;
    println!("Found {} pools", pools.len());

    Ok(())
}
```

## Core Client API

### MantraClient

The main entry point for all MANTRA blockchain interactions.

#### Creation

```rust
use mantra_sdk::{MantraClient, MantraClientBuilder, MantraNetworkConfig};

// Using builder pattern (recommended)
let client = MantraClientBuilder::new()
    .with_network_config(MantraNetworkConfig::testnet())
    .with_timeout(Duration::from_secs(30))
    .build()
    .await?;

// Direct creation with configuration manager
use mantra_sdk::config::ConfigurationManager;
let config_manager = ConfigurationManager::load_from_files("config/")?;
let client = MantraClient::new_with_config(config_manager).await?;
```

#### Network Configuration

```rust
use mantra_sdk::MantraNetworkConfig;

// Predefined networks
let testnet = MantraNetworkConfig::testnet();
let mainnet = MantraNetworkConfig::mainnet();
let dukong = MantraNetworkConfig::mantra_dukong();

// Custom network
let custom = MantraNetworkConfig {
    chain_id: "custom-1".to_string(),
    rpc_url: "https://rpc.custom.network".to_string(),
    bech32_prefix: "mantra".to_string(),
    fee_denom: "uom".to_string(),
    gas_price: 0.01,
    gas_adjustment: 1.5,
};
```

#### Protocol Access

```rust
// Access protocol-specific clients
let dex_client = client.dex();          // DEX operations
let claimdrop_client = client.claimdrop(); // ClaimDrop operations  
let skip_client = client.skip();        // Skip cross-chain operations
```

## DEX Protocol API

The DEX protocol provides trading, liquidity management, and farming capabilities.

### Pool Operations

```rust
// Get all pools
let pools = client.dex().get_pools(None, None).await?;

// Get specific pool
let pool_id = 1u64;
let pool = client.dex().get_pool(pool_id).await?;

// Get pool by asset pair
let pool = client.dex().get_pool_by_assets("uom", "uusdc").await?;
```

### Trading Operations

```rust
use cosmwasm_std::{Coin, Uint128};
use mantra_sdk::MantraWallet;

// Load wallet
let wallet = MantraWallet::from_mnemonic(
    "your twelve word mnemonic here...",
    "mantra", // bech32 prefix
    0         // account index
)?;

// Execute swap
let offer_asset = Coin {
    denom: "uom".to_string(),
    amount: Uint128::from(1_000_000u128), // 1 OM
};

let result = client.dex().execute_swap(
    offer_asset,
    "uusdc".to_string(), // ask asset
    Some(Decimal::percent(1)), // 1% slippage tolerance
    &wallet
).await?;

println!("Swap executed: {}", result.txhash);
```

### Liquidity Operations

```rust
// Provide liquidity
let asset_a = Coin {
    denom: "uom".to_string(),
    amount: Uint128::from(1_000_000u128),
};
let asset_b = Coin {
    denom: "uusdc".to_string(), 
    amount: Uint128::from(1_000_000u128),
};

let result = client.dex().provide_liquidity(
    asset_a,
    asset_b,
    Some(Decimal::percent(5)), // 5% slippage tolerance
    &wallet
).await?;

// Withdraw liquidity
let lp_token = Coin {
    denom: "factory/mantra1.../pool1".to_string(),
    amount: Uint128::from(500_000u128),
};

let result = client.dex().withdraw_liquidity(
    lp_token,
    &wallet
).await?;
```

### Balance Queries

```rust
// Get all balances for an address
let balances = client.dex().get_balances("mantra1...").await?;

// Get LP token balances
let lp_balances = client.dex().get_all_lp_token_balances("mantra1...").await?;

// Get specific LP balance
let pool_id = 1u64;
let lp_balance = client.dex().get_lp_token_balance("mantra1...", pool_id).await?;
```

### Farming Operations

```rust
// Get all farms
let farms = client.dex().get_farms().await?;

// Stake in farm
let lp_amount = Uint128::from(1_000_000u128);
let result = client.dex().enter_farm(pool_id, lp_amount, &wallet).await?;

// Claim rewards
let result = client.dex().claim_rewards(pool_id, &wallet).await?;

// Exit farm
let result = client.dex().exit_farm(pool_id, lp_amount, &wallet).await?;
```

## ClaimDrop Protocol API

The ClaimDrop protocol manages airdrop campaigns and token distributions.

### Campaign Management

```rust
use mantra_sdk::protocols::claimdrop::{CampaignParams, Allocation};

// Create campaign (admin only)
let campaign_params = CampaignParams {
    name: "Test Airdrop".to_string(),
    description: "A test airdrop campaign".to_string(),
    reward_asset: Coin {
        denom: "uom".to_string(),
        amount: Uint128::from(1_000_000_000u128),
    },
    start_time: None, // Starts immediately
    end_time: Some(1735689600), // Unix timestamp
};

let allocations = vec![
    Allocation {
        address: "mantra1...".to_string(),
        amount: Uint128::from(1_000_000u128),
    },
];

let result = client.claimdrop().create_campaign(
    campaign_params,
    allocations,
    &admin_wallet
).await?;
```

### Campaign Queries

```rust
// Get all campaigns
let campaigns = client.claimdrop().get_campaigns(None, None).await?;

// Get specific campaign
let campaign_id = 1u64;
let campaign = client.claimdrop().get_campaign(campaign_id).await?;

// Get user rewards
let user_rewards = client.claimdrop().get_user_rewards(
    campaign_id,
    "mantra1..."
).await?;

// Check if user has claimed
let has_claimed = client.claimdrop().has_user_claimed(
    campaign_id,
    "mantra1..."
).await?;
```

### Claiming Rewards

```rust
// Claim rewards from campaign
let result = client.claimdrop().claim_rewards(
    campaign_id,
    &wallet
).await?;

// Batch claim from multiple campaigns
let campaign_ids = vec![1, 2, 3];
let result = client.claimdrop().batch_claim_rewards(
    campaign_ids,
    &wallet
).await?;
```

### Factory Operations

```rust
// Deploy new claimdrop contract
let init_msg = json!({
    "admin": "mantra1...",
    "fee_collector": "mantra1...",
});

let result = client.claimdrop().deploy_claimdrop_contract(
    init_msg,
    &admin_wallet
).await?;

// Get factory info
let factory_info = client.claimdrop().get_factory_info().await?;
```

## Skip Protocol API

The Skip protocol enables cross-chain routing and asset bridging.

### Cross-Chain Routing

```rust
use mantra_sdk::protocols::skip::{SkipAsset, SkipRoute};

// Get available assets
let assets = client.skip().get_assets().await?;

// Find route between chains
let source_asset = SkipAsset {
    denom: "uom".to_string(),
    chain_id: "mantra-1".to_string(),
};

let dest_asset = SkipAsset {
    denom: "uosmo".to_string(), 
    chain_id: "osmosis-1".to_string(),
};

let routes = client.skip().get_route(
    source_asset,
    dest_asset,
    Uint128::from(1_000_000u128)
).await?;
```

### Cross-Chain Swaps

```rust
// Execute cross-chain swap
let swap_params = SkipSwapExactAssetIn {
    source_asset: SkipAsset {
        denom: "uom".to_string(),
        chain_id: "mantra-1".to_string(),
    },
    dest_asset: SkipAsset {
        denom: "uosmo".to_string(),
        chain_id: "osmosis-1".to_string(), 
    },
    amount_in: Uint128::from(1_000_000u128),
    amount_out_min: Uint128::from(950_000u128), // 5% slippage
    receiver: "osmo1...".to_string(),
};

let result = client.skip().execute_swap(swap_params, &wallet).await?;
```

### Route Simulation

```rust
// Simulate swap to get expected output
let simulation = client.skip().simulate_swap_exact_asset_in(
    source_asset,
    dest_asset, 
    Uint128::from(1_000_000u128)
).await?;

println!("Expected output: {}", simulation.amount_out);
println!("Price impact: {}%", simulation.price_impact);
```

## Wallet Management

### Creating Wallets

```rust
use mantra_sdk::MantraWallet;

// Generate new wallet
let wallet = MantraWallet::generate("mantra", 0)?;
println!("Address: {}", wallet.address());
println!("Mnemonic: {}", wallet.mnemonic());

// From existing mnemonic
let wallet = MantraWallet::from_mnemonic(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    "mantra",
    0
)?;

// From private key
let wallet = MantraWallet::from_private_key(
    &private_key_bytes,
    "mantra"
)?;
```

### Wallet Operations

```rust
// Sign transaction
let unsigned_tx = /* create transaction */;
let signed_tx = wallet.sign_tx(unsigned_tx)?;

// Get wallet info
let info = wallet.get_wallet_info();
println!("Address: {}", info.address);
println!("Public key: {}", hex::encode(&info.public_key));

// Derive additional accounts
let wallet_account_1 = MantraWallet::from_mnemonic(
    wallet.mnemonic(),
    "mantra", 
    1  // account index 1
)?;
```

### Encrypted Storage

```rust
// Save wallet encrypted
let password = "secure_password";
wallet.save_encrypted("./wallets/my_wallet.json", password)?;

// Load encrypted wallet
let loaded_wallet = MantraWallet::load_encrypted(
    "./wallets/my_wallet.json",
    password
)?;
```

## Configuration Management

### Modern Configuration System

```rust
use mantra_sdk::config::{ConfigurationManager, ProtocolId, ContractType};

// Load configuration from files
let config = ConfigurationManager::load_from_files("./config")?;

// Access contract addresses
let pool_manager = config.get_contract_address(
    &"mantra-1".to_string(),
    &ContractType::PoolManager
)?;

// Access protocol configuration  
let dex_config = config.get_protocol_config(&ProtocolId::Dex)?;

// Environment-based configuration
let env_config = config.get_environment_config();
```

### Legacy Configuration Support

```rust
use mantra_sdk::{MantraNetworkConfig, ContractAddresses};

// Legacy network config
let config = MantraNetworkConfig {
    chain_id: "mantra-1".to_string(),
    rpc_url: "https://rpc.mantrachain.io".to_string(),
    bech32_prefix: "mantra".to_string(),
    fee_denom: "uom".to_string(),
    gas_price: 0.01,
    gas_adjustment: 1.5,
};

// Legacy contract addresses
let contracts = ContractAddresses {
    pool_manager: "mantra1...".to_string(),
    farm_manager: Some("mantra1...".to_string()),
    fee_collector: Some("mantra1...".to_string()),
    epoch_manager: Some("mantra1...".to_string()),
    skip_entry_point: Some("mantra1...".to_string()),
    skip_ibc_hooks_adapter: Some("mantra1...".to_string()),
    skip_mantra_dex_adapter: Some("mantra1...".to_string()),
};
```

## MCP Server Setup

The SDK includes an MCP (Model Context Protocol) server that provides AI agents with 39 tools across all protocols.

### Building and Running

```bash
# Build MCP server
cargo build --features mcp --bin mcp-server

# Run with stdio transport (for AI integration)
cargo run --features mcp --bin mcp-server -- --transport stdio --network testnet

# Run with HTTP transport (for debugging)
cargo run --features mcp --bin mcp-server -- --transport http --port 8080
```

### Available MCP Tools

#### DEX Tools (28 tools)
- Wallet management: `get_active_wallet`, `list_wallets`, `switch_wallet`, `add_wallet_from_mnemonic`, `remove_wallet`
- Balance queries: `get_balances`, `get_lp_token_balance`, `get_all_lp_token_balances`
- Pool operations: `get_pools`, `get_pool`, `create_pool`
- Trading: `execute_swap`, `simulate_swap`
- Liquidity: `provide_liquidity`, `withdraw_liquidity`, `estimate_lp_withdrawal_amounts`
- Farming: `get_farms`, `enter_farm`, `exit_farm`, `claim_rewards`
- Network: `validate_network_connectivity`, `get_contract_addresses`

#### ClaimDrop Tools (5 tools)
- Campaign management: `create_claimdrop_campaign`, `get_claimdrop_campaigns`
- Claiming: `claim_claimdrop_rewards`, `get_user_claimdrop_rewards`
- Factory operations: `deploy_claimdrop_contract`

#### Skip Tools (6 tools)
- Asset queries: `get_skip_assets`, `get_skip_chains`
- Routing: `get_skip_route`, `simulate_skip_swap`
- Cross-chain operations: `execute_skip_swap`, `get_skip_transaction_status`

### MCP Configuration

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "/path/to/target/release/mcp-server",
      "args": ["--transport", "stdio", "--network", "testnet"],
      "env": {
        "RUST_LOG": "info",
        "WALLET_MNEMONIC": "your mnemonic for automation"
      }
    }
  }
}
```

### MCP Resources

The server provides 3 read-only resources:

- `network://status` - Network health and connectivity
- `network://config` - Current network configuration  
- `contracts://addresses` - Smart contract addresses

## Error Handling

### Error Types

```rust
use mantra_sdk::Error;

match result {
    Ok(value) => println!("Success: {:?}", value),
    Err(Error::NetworkError(msg)) => eprintln!("Network error: {}", msg),
    Err(Error::ParseError(msg)) => eprintln!("Parse error: {}", msg),
    Err(Error::WalletError(msg)) => eprintln!("Wallet error: {}", msg),
    Err(Error::ContractError(msg)) => eprintln!("Contract error: {}", msg),
    Err(Error::ConfigError(msg)) => eprintln!("Config error: {}", msg),
    Err(Error::SkipError(msg)) => eprintln!("Skip protocol error: {}", msg),
    Err(Error::ClaimdropError(msg)) => eprintln!("ClaimDrop error: {}", msg),
    Err(Error::InsufficientBalance { required, available }) => {
        eprintln!("Insufficient balance: need {}, have {}", required, available);
    },
    Err(Error::SlippageExceeded { expected, actual }) => {
        eprintln!("Slippage exceeded: expected {}, got {}", expected, actual);
    },
    Err(Error::PoolNotFound { pool_id }) => {
        eprintln!("Pool {} not found", pool_id);
    },
}
```

### Best Practices

```rust
use anyhow::Result;

// Use Result types consistently
async fn perform_swap() -> Result<String> {
    let client = MantraClientBuilder::new()
        .with_network_config(MantraNetworkConfig::testnet())
        .build()
        .await?;

    let result = client.dex().execute_swap(
        offer_asset,
        "uusdc".to_string(),
        Some(Decimal::percent(1)),
        &wallet
    ).await?;

    Ok(result.txhash)
}

// Handle specific errors
async fn safe_swap() -> Result<String> {
    match perform_swap().await {
        Ok(txhash) => Ok(txhash),
        Err(e) if e.to_string().contains("insufficient") => {
            Err(anyhow::anyhow!("Not enough tokens for swap"))
        },
        Err(e) => Err(e),
    }
}
```

## Troubleshooting

### Common Issues

#### Connection Issues
```rust
// Test network connectivity
let client = MantraClientBuilder::new()
    .with_network_config(MantraNetworkConfig::testnet())
    .with_timeout(Duration::from_secs(10))
    .build()
    .await?;

let status = client.validate_network_connectivity().await?;
println!("Network status: {:?}", status);
```

#### Wallet Issues
```rust
// Validate wallet setup
let wallet = MantraWallet::from_mnemonic(mnemonic, "mantra", 0)?;
let balances = client.dex().get_balances(&wallet.address()).await?;

if balances.is_empty() {
    println!("Wallet has no balances - fund it first");
}
```

#### Contract Issues
```rust
// Check contract addresses
let addresses = client.dex().get_contract_addresses().await?;
println!("Pool manager: {}", addresses.pool_manager);

// Verify contracts are deployed
let pools = client.dex().get_pools(None, None).await?;
println!("Found {} pools", pools.len());
```

### Environment Variables

```bash
# Network configuration
export MANTRA_NETWORK=testnet
export MANTRA_RPC_URL=https://rpc.testnet.mantrachain.io

# Wallet automation (development only)
export WALLET_MNEMONIC="your mnemonic here"

# Logging
export RUST_LOG=debug
export MCP_DEBUG=true
```

### Common Error Solutions

| Error | Solution |
|-------|----------|
| "Connection refused" | Check RPC URL and network connectivity |
| "Invalid mnemonic" | Verify mnemonic has 12 or 24 words |
| "Insufficient balance" | Fund wallet or reduce transaction amount |
| "Pool not found" | Check pool exists and pool ID is correct |
| "Slippage exceeded" | Increase slippage tolerance |
| "Contract error" | Check contract is deployed and accessible |

### Debug Logging

```rust
use env_logger;

// Enable debug logging
env_logger::init();

// In your application
log::debug!("Executing swap with params: {:?}", swap_params);
log::info!("Swap completed successfully: {}", result.txhash);
```

### Testing Configuration

```bash
# Run with test configuration
MANTRA_NETWORK=testnet \
WALLET_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about" \
cargo run --features mcp --bin mcp-server -- --transport stdio --debug
```

For more detailed examples and advanced usage patterns, see the [examples directory](../../examples/) and [Developer Guide](../DEVELOPER_GUIDE.md).
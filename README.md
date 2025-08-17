# MANTRA SDK

A comprehensive Rust SDK for interacting with the MANTRA blockchain ecosystem, supporting multiple protocols including DEX, ClaimDrop, Skip, and more. Features wallet management, protocol-specific operations, and optional MCP server and TUI interfaces.

## Features

- **Multi-Protocol Support**: Modular architecture supporting DEX, ClaimDrop, Skip, and future protocols
- **Complete DEX Operations**: Swap execution, liquidity provision/withdrawal, pool management
- **ClaimDrop Management**: Campaign creation, reward claims, allocation management
- **Cross-Chain Operations**: Skip protocol integration for cross-chain swaps and routing
- **HD Wallet Management**: BIP32/BIP39 compatible wallet generation and import
- **Multi-Network Support**: Configurable testnet/mainnet connectivity
- **MCP Server Integration**: Model Context Protocol server with protocol-prefixed tools for AI agents
- **Terminal UI**: Interactive command-line interface for DEX operations

## Architecture

### Core SDK (`src/`)

```
src/
├── client.rs          # Generic MANTRA client with protocol adapters
├── config.rs          # Network configuration and constants management  
├── wallet/            # HD wallet operations and key management
├── error.rs           # Centralized error types and handling
├── protocols/         # Protocol implementations
│   ├── dex/           # DEX protocol (pools, swaps, liquidity)
│   │   ├── client.rs  # DEX client implementation
│   │   └── types.rs   # DEX-specific types
│   ├── claimdrop/     # ClaimDrop protocol
│   │   ├── client.rs  # Campaign operations
│   │   ├── factory.rs # Factory operations
│   │   └── types.rs   # ClaimDrop types
│   └── skip/          # Skip protocol for cross-chain
│       ├── client.rs  # Skip adapter client
│       └── types.rs   # Skip-specific types
└── lib.rs             # Module exports and feature-gated re-exports
```

### Feature-Gated Modules

#### MCP Server (`--features mcp`)
```
src/mcp/
├── server.rs          # Core MCP server with JSON-RPC 2.0 support
├── sdk_adapter.rs     # Adapter layer between MCP and SDK
└── client_wrapper.rs  # MCP client wrapper functionality
```

#### DEX Terminal UI (`--features tui-dex`)
```
src/tui_dex/
├── app.rs             # Central application state management (2680 lines)
├── events.rs          # Event handling and async operations (929 lines)
├── screens/           # Individual screen implementations
└── components/        # Reusable UI components
```

## Quick Start

### Installation

```bash
git clone <repository-url>
cd mantra-dex-sdk
cargo build --release
```

### Basic SDK Usage

```rust
use mantra_sdk::{MantraClient, MantraClientBuilder, MantraNetworkConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client with builder pattern
    let client = MantraClientBuilder::new()
        .with_network(MantraNetworkConfig::testnet())
        .build()
        .await?;
    
    // List available protocols
    let protocols = client.list_protocols();
    println!("Available protocols: {:?}", protocols);
    
    // Use DEX protocol
    let dex_client = client.dex()?;
    let pools = dex_client.get_pools(None, None).await?;
    println!("Available DEX pools: {}", pools.len());
    
    // Use ClaimDrop protocol
    let factory_address = "mantra1...";
    let claimdrop_factory = client.claimdrop_factory(factory_address.to_string());
    let campaigns = claimdrop_factory.query_campaigns(None, None).await?;
    println!("ClaimDrop campaigns: {:?}", campaigns);
    
    Ok(())
}
```

## Build Targets

### SDK Library (Default)
```bash
cargo build              # Build SDK library only
cargo test               # Run SDK tests
```

### MCP Server
```bash
cargo check --features mcp                    # Compilation validation
cargo build --features mcp                    # Build MCP server
cargo run --bin mcp-server --features mcp     # Run MCP server
```

The MCP server provides protocol-prefixed tools for AI agents:

**Network Tools:**
- `network_get_contract_addresses` - Get contract addresses for the current network
- `network_validate_connectivity` - Validate network connectivity

**Wallet Tools:**
- `wallet_get_balances` - Get wallet balances
- `wallet_list` - List available wallets
- `wallet_switch` - Switch active wallet
- `wallet_get_active` - Get active wallet info
- `wallet_add_from_mnemonic` - Add wallet from mnemonic
- `wallet_remove` - Remove a wallet

**DEX Tools:**
- `dex_get_pools` - Query available pools
- `dex_execute_swap` - Execute a token swap
- `dex_provide_liquidity` - Provide liquidity to a pool
- `dex_withdraw_liquidity` - Withdraw liquidity from a pool
- `dex_create_pool` - Create a new pool
- `dex_get_lp_token_balance` - Get LP token balance
- `dex_get_all_lp_token_balances` - Get all LP token balances
- `dex_estimate_lp_withdrawal_amounts` - Estimate withdrawal amounts

**ClaimDrop Tools:**
- `claimdrop_create_campaign` - Create a new claimdrop campaign
- `claimdrop_claim` - Claim rewards from a campaign
- `claimdrop_query_rewards` - Query user rewards
- `claimdrop_query_campaigns` - Query all campaigns
- `claimdrop_add_allocations` - Add allocations to a campaign

**Skip Protocol Tools:**
- `skip_route_assets` - Find optimal cross-chain route
- `skip_simulate_swap` - Simulate cross-chain swap

### DEX Terminal UI
```bash
cargo run --bin mantra-dex-tui --features tui-dex  # Primary DEX TUI entry point
cargo run --bin tui --features tui-dex             # Alternative DEX TUI entry point
```

## Core Modules

### Main Client (`src/client.rs`)
The generic MANTRA client providing access to all protocols:
- **Protocol Management**: Dynamic protocol loading and configuration
- **Unified Interface**: Access all protocols through a single client
- **Connection Pooling**: Efficient RPC connection management

### DEX Protocol (`src/protocols/dex/`)
Complete DEX functionality:
- **Pool Operations**: Query pools, create pools (admin), manage pool features
- **Trading**: Execute swaps, simulate trades, monitor transactions  
- **Liquidity Management**: Provide/withdraw liquidity, manage LP tokens
- **Wallet Integration**: Balance queries, transaction signing
- **Analytics**: Generate trading reports, calculate impermanent loss

### ClaimDrop Protocol (`src/protocols/claimdrop/`)
Campaign-based reward distribution:
- **Campaign Management**: Create campaigns through factory, close campaigns
- **Reward Operations**: Claim rewards, query allocations
- **Admin Functions**: Add/remove allocations, manage blacklists
- **Aggregation**: Query rewards across all campaigns

### Skip Protocol (`src/protocols/skip/`)
Cross-chain operations:
- **Route Discovery**: Find optimal paths for cross-chain transfers
- **Swap Simulation**: Simulate cross-chain swaps before execution
- **IBC Integration**: Handle Inter-Blockchain Communication

```rust
// Example: Execute a DEX swap
let dex_client = client.dex()?;
let swap_result = dex_client.execute_swap(
    "1",                    // pool_id
    ("uom", "1000000"),     // offer_asset (denom, amount)
    "uusdy",                // ask_asset_denom
    Some("0.05")            // max_slippage (5%)
).await?;
```

### Wallet (`src/wallet.rs`)
HD wallet functionality with secure key management:
- **Generation**: Create new wallets with mnemonic phrases
- **Import**: Import existing wallets from mnemonic
- **Key Derivation**: BIP32/BIP39 compliant key derivation
- **Security**: Encrypted storage, private key protection

```rust
// Example: Generate new wallet
let wallet = client.generate_wallet(0, true, Some("my-wallet")).await?;
println!("Address: {}", wallet.address);
```

### Configuration (`src/config.rs`)
Network and runtime configuration:
- **Multi-Network**: Testnet/mainnet support with chain_id migration
- **Endpoints**: Configurable RPC, LCD, and gRPC endpoints
- **Validation**: Bech32 address validation for Cosmos addresses

```rust
// Example: Custom network configuration
let config = Config {
    chain_id: "mantra-dukong".to_string(),
    rpc_endpoint: "https://rpc.testnet.mantra.com".to_string(),
    lcd_endpoint: "https://api.testnet.mantra.com".to_string(),
    grpc_endpoint: "https://grpc.testnet.mantra.com".to_string(),
    ..Config::default()
};
```

## Key Dependencies

### Core SDK
- **`mantra-dex-std`**: DEX standard library and types
- **`cosmrs`**: Cosmos SDK integration with RPC, BIP32, CosmWasm support
- **`tokio`**: Async runtime for concurrent operations

### MCP Server (Optional)
- **`rust-mcp-sdk`**: MCP server implementation
- **`rust-mcp-schema`**: MCP schema definitions
- **`axum`, `hyper`**: HTTP server infrastructure

### Terminal UI (Optional)
- **`ratatui`**: Modern terminal UI framework
- **`crossterm`**: Cross-platform terminal control
- **`tui-input`**: Advanced text input handling

## Development Workflow

### Adding New SDK Features
1. Implement core functionality in `src/client.rs`
2. Add error types to `src/error.rs`
3. Update configuration in `src/config.rs` if needed
4. Add comprehensive unit tests
5. Update `src/lib.rs` exports

### Testing Strategy
- **SDK Core**: Full test coverage for business logic
- **MCP Server**: Integration tests for protocol compliance
- **TUI**: Manual testing only (no automated UI tests)

```bash
cargo test                           # Run all SDK tests
cargo test --features mcp          # Test MCP functionality  
cargo test wallet_operations       # Test specific modules
```

## Environment Configuration

```bash
# Network settings
export MANTRA_NETWORK=testnet
export MANTRA_RPC_ENDPOINT=https://rpc.testnet.mantra.com
export MANTRA_LCD_ENDPOINT=https://api.testnet.mantra.com

# Development settings  
export RUST_LOG=debug
export MCP_SERVER_DEBUG=true
```

## Security Best Practices

- **Private Keys**: Never exposed in responses, encrypted storage
- **Validation**: All transaction parameters validated before execution
- **Slippage Protection**: Configurable slippage limits for trades
- **Address Verification**: Proper bech32 address validation

## Contributing

1. Fork and clone the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following existing code conventions
4. Add tests for new functionality
5. Ensure all tests pass (`cargo test`)
6. Submit a pull request

### Code Conventions
- Follow existing patterns in `src/client.rs` for new operations
- Add proper error handling with descriptive error types
- Use existing libraries and utilities where possible
- Maintain comprehensive test coverage for business logic

## License

MIT License - see [LICENSE](LICENSE) file for details.

---

Built for the MANTRA ecosystem 🚀
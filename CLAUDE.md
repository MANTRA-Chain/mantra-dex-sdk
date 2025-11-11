# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MANTRA SDK - A modular Rust SDK for the MANTRA blockchain ecosystem supporting multiple protocols (DEX, ClaimDrop, Skip) with optional MCP server and TUI interfaces.

## Common Development Commands

### Build and Test
```bash
# Core SDK
cargo build                                # Build SDK library only
cargo test                                 # Run all tests
cargo test --test integration_test        # Run integration tests only
cargo test wallet_operations              # Test specific module

# With features
cargo build --features mcp                # Build with MCP server
cargo build --features tui-dex            # Build with DEX TUI
cargo test --features mcp                 # Test MCP functionality

# Makefile commands
make build                                 # Build with all features
make test                                  # Run all tests
make test-unit                            # Run unit tests only
make lint                                  # Run clippy linter
make format                                # Format code
make quick-test                           # format-check + lint + test
make clean                                 # Clean build artifacts
```

### Run Components
```bash
# MCP Server (Model Context Protocol)
cargo check --features mcp                # Check compilation (preferred)
cargo run --bin mcp-server --features mcp # Run MCP server

# TUI (Terminal User Interface)
cargo run --bin mantra-dex-tui --features tui-dex  # Primary TUI
cargo run --bin tui --features tui-dex             # Alternative TUI
make dev-tui                                        # Development mode
```

### Development Workflow
```bash
make setup                                 # Setup dev environment
make dev                                   # Start dev environment with monitoring
make dev-watch                            # Auto-reload on changes
make docker-build                         # Build Docker images
make k8s-deploy                          # Deploy to Kubernetes
```

## Code Search and Analysis

For semantic code search and analysis, refer to **~/workspace/AST_GREP_INSTRUCTIONS.md** which provides guidance on using AST-grep for:
- Precise semantic code search using AST patterns
- Finding specific code structures (functions, hooks, classes)
- Language-aware pattern matching across the codebase
- Structural code analysis without regex complexity

Example usage for this repository:
```bash
# Find all Protocol trait implementations
ast-grep run -p 'impl Protocol for $TYPE { $$$BODY }' --lang rust

# Find all DEX client method calls
ast-grep run -p 'client.dex()?' --lang rust

# Find wallet operations
ast-grep run -p '$CLIENT.wallet.$METHOD($$$ARGS)' --lang rust
```

## Architecture

### Modular Protocol System (`src/`)
```
src/
├── client.rs                   # Generic MantraClient with protocol adapters
├── config/                     # Configuration management
│   ├── contracts.rs           # Contract addresses by network
│   ├── env.rs                 # Environment configuration
│   └── protocols.rs           # Protocol-specific configs
├── protocols/                  # Protocol implementations
│   ├── dex/                   # DEX protocol
│   │   ├── client.rs          # MantraDexClient implementation
│   │   └── types.rs           # DEX-specific types
│   ├── claimdrop/             # ClaimDrop protocol
│   │   ├── client.rs          # Campaign operations
│   │   ├── factory.rs         # Factory pattern implementation
│   │   └── types.rs           # ClaimDrop types
│   └── skip/                  # Skip protocol (cross-chain)
│       ├── client.rs          # SkipAdapter implementation
│       └── types.rs           # Skip-specific types
├── wallet/                     # Wallet management
│   ├── mod.rs                 # HD wallet operations
│   └── storage.rs             # Encrypted wallet storage
└── mcp/ (feature-gated)       # MCP server
    ├── server.rs              # JSON-RPC 2.0 server
    └── sdk_adapter.rs         # Protocol adapter layer
```

### Key Architecture Patterns

**Protocol Registry Pattern**: All protocols implement the `Protocol` trait and register with `ProtocolRegistry` in `MantraClient`. Access protocols via:
```rust
client.dex()?           // Get DEX protocol
client.claimdrop_factory(address)  // Get ClaimDrop factory
client.skip()?          // Get Skip protocol
```

**Configuration System**: Unified `ConfigurationManager` loads from:
- `config/network.toml` - Network endpoints
- `config/contracts.toml` - Contract addresses
- Environment variables override TOML configs

**MCP Tool Naming**: All MCP tools are prefixed by protocol:
- `network_*` - Network operations
- `wallet_*` - Wallet management
- `dex_*` - DEX operations
- `claimdrop_*` - ClaimDrop operations
- `skip_*` - Cross-chain operations

## Protocol-Specific Development

### Adding New Protocol
1. Create `src/protocols/<name>/` directory
2. Implement `Protocol` trait in `client.rs`
3. Define types in `types.rs`
4. Register in `ProtocolRegistry` (src/client.rs:80-100)
5. Add contract addresses to `config/contracts.toml`
6. Update MCP adapter if needed (src/mcp/sdk_adapter.rs)

### DEX Protocol (`src/protocols/dex/`)
- Pool management, swaps, liquidity operations
- Uses `mantra-dex-std` for contract types
- Slippage protection built into all swap operations

### ClaimDrop Protocol (`src/protocols/claimdrop/`)
- Campaign creation through factory pattern
- Reward claiming and allocation management
- Uses `mantra-claimdrop-std` for contract types

### Skip Protocol (`src/protocols/skip/`)
- Cross-chain routing and transfers
- IBC packet handling
- External Skip API integration

## Testing Strategy

### What to Test
- Protocol client implementations
- Wallet operations and key derivation
- Configuration loading and validation
- MCP protocol compliance
- Integration tests in `tests/integration/`

### What NOT to Test
- TUI components (manual testing only)
- UI rendering or visual elements
- Generated bindings from `*-std` crates

### Running Specific Tests
```bash
cargo test client_test                    # Test client operations
cargo test --test integration_test       # Integration tests
cargo test --features mcp mcp_           # MCP-specific tests
cargo test --test migration_validation_test  # Migration validation
cargo test --test fee_validation_test    # Fee validation tests
cargo test --test slippage_validation_test  # Slippage tests
```

## Configuration Files

- `config/network.toml` - RPC/LCD/gRPC endpoints
- `config/contracts.toml` - Contract addresses per network
- `config/test.toml` - Test configuration
- `config/mcp.toml` - MCP server configuration
- `Makefile` - Development automation (40+ targets)
- `docker-compose.yml` - Local development stack
- `k8s/` - Kubernetes deployment configurations

## Feature Flags

- `default`: No features, core SDK only
- `mcp`: Enable MCP server with all dependencies
- `tui-dex`: Enable DEX Terminal UI
- `performance`: Performance monitoring (in Makefile)
- `security`: Security features (in Makefile)
- `resilience`: Resilience features (in Makefile)
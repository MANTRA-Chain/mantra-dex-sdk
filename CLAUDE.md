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

### PrimarySale Protocol (`src/protocols/evm/contracts/primary_sale.rs`)
- RWA token sale contract integration for atomic settlement
- Status flow: Pending → Active → Ended/Failed → Settled/Cancelled
- Uses ERC-20 mantraUSD for investor contributions
- Commission-based settlement with configurable basis points
- Integrated with Allowlist contract for KYC/AML compliance

**MCP Tools (13 total):**

*Query Operations:*
- `primary_sale_get_sale_info` - Get comprehensive sale status (start/end times, soft cap, contributions, investor count)
- `primary_sale_get_investor_info` - Query investor allocation and contribution details
- `primary_sale_get_all_investors` - Paginated list of all investors with pagination support

*Investor Operations:*
- `primary_sale_invest` - Contribute mantraUSD to an active sale (with allowance validation)
- `primary_sale_claim_refund` - Claim refund when sale fails or is cancelled

*Admin Operations:*
- `primary_sale_activate` - Transition sale from Pending to Active (requires current_time >= START, admin role)
- `primary_sale_end_sale` - Transition to Ended or Failed based on soft cap achievement (callable by anyone after END time)
- `primary_sale_settle_and_distribute` - Atomic settlement: distribute RWA tokens to all investors (settlement role required, 30% gas buffer)
- `primary_sale_cancel` - Cancel sale from Pending or Active state (admin role, enables refunds)
- `primary_sale_pause` - Pause contract (admin role, blocks invest and refund operations)
- `primary_sale_unpause` - Unpause contract (admin role, re-enables operations)
- `primary_sale_emergency_withdraw` - Recover stuck ERC-20 tokens (admin role, only when Cancelled)

*Public Operations:*
- `primary_sale_top_up_refunds` - Anyone can fund the refund pool (requires allowance)

**Key Implementation Details:**
- Pre-settlement validation: checks investor count ≤ max_loop, verifies multisig and asset owner balances/allowances
- Commission calculation: `(total_contributed * commission_bps) / 10000`
- Gas buffers: 20% for simple operations, 30% for complex settlement operation
- Allowance checks: mandatory before invest and top_up_refunds operations
- Error handling: surfaces contract-specific errors with helpful messages

**Transaction Patterns:**
- All state-changing operations use `build_sign_and_broadcast_transaction()` helper
- Uses EIP-1559 transaction format with dynamic fee suggestion
- Proper gas estimation with configurable buffers
- Signature generation via MultiVM wallet with Ethereum derivation

**Security Considerations:**

*Access Control:*
- **Admin Operations (DEFAULT_ADMIN_ROLE):**
  - `primary_sale_activate` - Can only activate when `current_time >= START`
  - `primary_sale_cancel` - Only from Pending or Active status
  - `primary_sale_pause` / `primary_sale_unpause` - Emergency circuit breaker
  - `primary_sale_emergency_withdraw` - Only when sale is Cancelled

- **Settlement Operations (SETTLEMENT_ROLE):**
  - `primary_sale_settle_and_distribute` - Requires SETTLEMENT_ROLE
  - Pre-settlement validation checks investor count, balances, and allowances
  - Uses 30% gas buffer due to complexity (multiple transfers in one tx)

- **Public Operations (No Role Required):**
  - `primary_sale_invest` - Must pass Allowlist validation (KYC/AML)
  - `primary_sale_top_up_refunds` - Anyone can fund refund pool
  - `primary_sale_claim_refund` - Only when sale Failed or Cancelled
  - `primary_sale_end_sale` - Callable by anyone after END timestamp

*Allowlist Integration:*
- All `invest()` calls validate investor address via Allowlist contract
- Allowlist enforces KYC/AML compliance
- Investors must be pre-approved before investing
- Check allowlist status before attempting investment

*Pausability:*
- When paused: `invest()` and `claimRefund()` operations blocked
- Admin operations still functional when paused
- Use for emergency situations (detected vulnerability, regulatory hold)
- Unpause only after issue resolved and validated

*Commission Validation:*
- Commission basis points (bps) set at deployment
- Calculated as: `(total_contributed * commission_bps) / 10000`
- No on-chain validation of reasonable commission rates
- Recommend max 20% (2000 bps) for user protection

*Status Flow Validation:*
```
Pending ──activate()──> Active ──endSale()──> Ended ──settle()──> Settled
   │                       │                     │
   └─────cancel()──────────┘                     │
                                                  │
                          Failed <──endSale()─────┘
                             │                 (soft cap not met)
                             └─refund pool topped up
                                └─investors claim refunds
```

*Emergency Procedures:*
1. **Pause Contract**: Use `primary_sale_pause` if vulnerability detected
2. **Cancel Sale**: Use `primary_sale_cancel` to enable refunds
3. **Top Up Refunds**: If contract balance insufficient, use `primary_sale_top_up_refunds`
4. **Emergency Withdraw**: After cancellation, use `primary_sale_emergency_withdraw` to recover stuck tokens

*Gas Estimation Buffers:*
- Simple operations (invest, claim, admin): 20% gas buffer
- Complex operations (settlement): 30% gas buffer
- Settlement distributes to all investors in one transaction
- Max investors per settlement: 500 (max_loop parameter)

*Best Practices:*
- Always check sale status before operations
- Verify Allowlist approval before investing
- Monitor gas prices for settlement (expensive operation)
- Test admin operations on testnet first
- Use multi-sig wallet for admin operations
- Validate commission rates before deployment
- Ensure asset owner approves contract before settlement

## EVM Transaction Analysis Tools

The SDK includes tools for decoding and analyzing EVM transaction history with human-readable narratives.

### Overview

The transaction analysis system consists of two main components:
- **TransactionDecoder** - Decodes transaction input data by matching function selectors to known contract ABIs
- **NarrativeGenerator** - Converts decoded transaction data into human-readable narratives

### Supported Contract Types

- **ERC-20**: Standard token operations (transfer, approve, transferFrom)
- **PrimarySale v2.0**: All primary sale contract functions including investment, settlement, refunds, and admin operations

### MCP Tool: `evm_analyze_transaction_history`

Analyzes one or more EVM transactions and generates human-readable narratives.

**Parameters:**
- `transaction_hashes` - Array of transaction hashes to analyze (e.g., `["0xabc...", "0xdef..."]`)

**Returns:**
- Sequential narrative describing what happened in the transactions
- Detailed transaction information including decoded function calls and parameters
- Transaction status (success/failure)

**Example Usage:**

```json
{
  "tool": "evm_analyze_transaction_history",
  "arguments": {
    "transaction_hashes": [
      "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
    ]
  }
}
```

**Output Example:**
```
First. you approved 0x2222...2222 to spend 1000.0 tokens from contract at 0x3333...3333 [tx: 0x1234...cdef]
```

### Use Cases

- **Transaction Debugging**: Understand what a transaction actually did by viewing decoded function calls
- **Audit Trails**: Generate human-readable transaction histories for compliance and auditing
- **User Activity Monitoring**: Track user interactions with smart contracts
- **Failed Transaction Analysis**: Understand why transactions failed by examining decoded input data

### Implementation Details

**Function Selector Matching:**
- Uses the first 4 bytes of transaction input data as the function selector
- Matches against known contract ABIs (ERC-20, PrimarySale)
- Gracefully handles unknown functions with generic narratives

**Narrative Features:**
- Address abbreviation (0x1234...5678) for readability
- Active wallet substitution ("you" instead of address)
- Sequential connectors (First, Then, Finally) for multi-transaction narratives
- Amount formatting with proper decimal handling
- Transaction hash linking for reference

**Source Code References:**
- `src/protocols/evm/transaction_decoder.rs` - Transaction decoding logic and function selector registry
- `src/protocols/evm/narrative_generator.rs` - Human-readable narrative generation
- `src/mcp/sdk_adapter/evm.rs:2700-2800` - MCP tool implementation

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
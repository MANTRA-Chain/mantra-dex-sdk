# PrimarySale EVM Protocol Integration

This document describes the PrimarySale contract integration as the first supported EVM protocol in the MANTRA SDK.

## Overview

The PrimarySale protocol provides functionality for managing primary sales of RWA (Real World Asset) tokens on the MANTRA EVM chain. This integration allows developers to interact with PrimarySale contracts programmatically through a clean, type-safe Rust interface.

## Architecture

The integration follows the established pattern for EVM contract support in the SDK:

```
src/protocols/evm/
├── contracts/
│   ├── primary_sale.rs    # PrimarySale contract bindings and helpers
│   ├── erc20.rs            # ERC-20 helpers
│   ├── erc721.rs           # ERC-721 helpers
│   └── custom.rs           # Custom contract support
├── client.rs               # EVM client with provider
└── types.rs                # Common EVM types
```

## Features

### Contract Interface

The `PrimarySale` helper provides comprehensive access to:

#### View Functions
- `status()` - Get current sale status (Pending, Active, Ended, Failed, Settled, Cancelled)
- `investor_count()` - Total number of investors
- `tokens_for(investor)` - Calculate token allocation for an investor
- `get_investor(index)` - Get investor address by index
- `is_sale_active()` - Check if sale is currently active
- `get_remaining_sale_time()` - Time remaining in sale window
- `total_contributed()` - Total mantraUSD contributed
- `contributed(investor)` - Individual investor contribution
- `refunds_pool()` - Available refund pool amount
- `refunded(investor)` - Check if investor claimed refund

#### Configuration Functions
- `mantra_usd()` - MantraUSD token address
- `allowlist()` - Allowlist contract address
- `start()` / `end()` - Sale time window
- `soft_cap()` - Minimum funding threshold
- `commission_bps()` - Commission rate in basis points
- `min_step()` - Minimum investment step

#### State-Changing Functions
- `activate()` - Activate the sale (admin)
- `invest(amount)` - Invest mantraUSD
- `end_sale()` - End the sale
- `settle_and_distribute()` - Distribute tokens (settlement role)
- `claim_refund()` - Claim refund if eligible
- `cancel()` - Cancel the sale (admin)
- `pause()` / `unpause()` - Pause/unpause contract

#### Convenience Methods
- `get_investors(start, limit)` - Paginated investor list
- `get_sale_info()` - Comprehensive sale information summary

## Usage

### Basic Example

```rust
use mantra_sdk::protocols::evm::client::EvmClient;
use alloy_primitives::{address, Address, U256};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create EVM client
    let evm_client = EvmClient::new(
        "https://evm.dukong.mantrachain.io",
        5887  // MANTRA EVM chain ID
    ).await?;

    // Create PrimarySale helper
    let primary_sale_address = address!("0x...");
    let primary_sale = evm_client.primary_sale(primary_sale_address);

    // Query sale information
    let sale_info = primary_sale.get_sale_info().await?;
    println!("Sale Status: {}", sale_info.status);
    println!("Total Raised: {}", sale_info.total_contributed);
    println!("Investors: {}", sale_info.investor_count);

    // Check investor allocation
    let investor = address!("0x...");
    let tokens = primary_sale.tokens_for(investor).await?;
    println!("Token allocation: {}", tokens);

    Ok(())
}
```

### Querying Multiple Sales

```rust
// Monitor multiple primary sales
let sales = vec![
    evm_client.primary_sale(address!("0x...")),
    evm_client.primary_sale(address!("0x...")),
];

for sale in sales {
    let info = sale.get_sale_info().await?;
    println!("Sale {} - Status: {}, Raised: {}",
        sale.address(),
        info.status,
        info.total_contributed
    );
}
```

### Investor Analytics

```rust
// Get all investors and their contributions
let sale = evm_client.primary_sale(address!("0x..."));
let investor_count: usize = sale.investor_count().await?.try_into()?;

let mut total_tokens = U256::ZERO;
for i in 0..investor_count {
    let investor = sale.get_investor(U256::from(i)).await?;
    let tokens = sale.tokens_for(investor).await?;
    let contributed = sale.contributed(investor).await?;

    println!("Investor {}: {} tokens, {} contributed",
        investor, tokens, contributed);
    total_tokens += tokens;
}

println!("Total tokens to distribute: {}", total_tokens);
```

## Configuration

### Contract Addresses

Contract addresses are configured in `config/evm_contracts.toml`:

```toml
[primary_sale]
testnet = "0x..."
mainnet = "0x..."
```

### Network Configuration

EVM RPC endpoints are configured in `config/network.toml`:

```toml
[mantra-dukong]
evm_rpc_url = "https://evm.dukong.mantrachain.io"
evm_chain_id = 5887
```

## Status Enumeration

```rust
pub enum Status {
    Pending = 0,     // Sale created but not activated
    Active = 1,      // Sale is active and accepting investments
    Ended = 2,       // Sale ended, awaiting settlement
    Failed = 3,      // Sale failed to meet soft cap
    Settled = 4,     // Sale settled, tokens distributed
    Cancelled = 5,   // Sale cancelled by admin
}
```

## Implementation Details

### Contract Bindings

The integration uses Alloy's `sol!` macro to generate type-safe contract bindings:

```rust
sol! {
    interface IPrimarySale {
        enum Status { Pending, Active, Ended, Failed, Settled, Cancelled }

        function status() external view returns (Status);
        function invest(uint256 amount) external;
        // ... more functions
    }
}
```

### Type Safety

All contract interactions are fully type-safe:
- Addresses use `alloy_primitives::Address`
- Amounts use `alloy_primitives::U256`
- Enums are properly represented
- ABI encoding/decoding is automatic

### Error Handling

All contract calls return `Result<T, Error>`:
- RPC errors are properly propagated
- Contract reverts are captured
- ABI decode errors are handled

## Future Enhancements

### Planned Features

1. **MCP Protocol Tools** - Add Model Context Protocol tools for PrimarySale operations
2. **Transaction Support** - Implement transaction signing and submission for state-changing operations
3. **Event Monitoring** - Add event log filtering and parsing for PrimarySale events
4. **Multi-contract Support** - Factory pattern for managing multiple PrimarySale instances
5. **Analytics Tools** - Built-in analytics and reporting functions

### Extension Points

The modular design allows for easy extension:
- Add new contract types in `src/protocols/evm/contracts/`
- Extend `EvmClient` with additional helper methods
- Add protocol-specific configuration loaders

## Related Contracts

The PrimarySale contract interacts with:
- **MantraUSD** (ERC-20) - Payment token
- **Allowlist** - KYC/AML compliance
- **RwaToken** (ERC-20) - Asset tokens being sold

These can be accessed using the existing `erc20()` helper:

```rust
let mantra_usd_addr = primary_sale.mantra_usd().await?;
let mantra_usd = evm_client.erc20(mantra_usd_addr);
let balance = mantra_usd.balance_of(investor).await?;
```

## References

- [PrimarySale Contract Source](../../../mantra-primary-sale/packages/evm/contracts/PrimarySale.sol)
- [Alloy Documentation](https://alloy.rs)
- [MANTRA Chain EVM](https://docs.mantrachain.io/evm)

## Support

For issues or questions:
- Create an issue in the repository
- Consult the inline documentation in `src/protocols/evm/contracts/primary_sale.rs`
- Review the example code in this document
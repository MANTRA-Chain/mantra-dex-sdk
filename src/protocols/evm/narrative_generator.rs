/// Narrative generator for EVM transactions
///
/// Converts decoded transaction data into human-readable narratives using
/// template-based generation. Supports multiple contract types and formats
/// addresses and amounts appropriately.
///
/// # Example
///
/// ```rust,no_run
/// use mantra_sdk::protocols::evm::narrative_generator::NarrativeGenerator;
/// use mantra_sdk::protocols::evm::transaction_decoder::{TransactionDecoder, DecodedCall};
/// use alloy_primitives::{Address, B256};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let generator = NarrativeGenerator::new(None);
/// let decoder = TransactionDecoder::new();
///
/// // Decode and generate narrative for a transaction
/// let tx_input = vec![/* transaction input data */];
/// let from_address = Address::ZERO;
/// let to_address = Some(Address::ZERO);
/// let tx_hash = B256::ZERO;
///
/// let decoded = decoder.decode(&tx_input, to_address)?;
/// let narrative = generator.generate_narrative(&decoded, from_address, to_address, tx_hash, true);
///
/// println!("{}", narrative);
/// # Ok(())
/// # }
/// ```
use crate::protocols::evm::transaction_decoder::{ContractType, DecodedCall};
use alloy_primitives::{Address, B256};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Configuration for narrative generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeConfig {
    /// Whether to show full addresses or abbreviated (0x1234...5678)
    pub show_full_addresses: bool,
    /// Active wallet address (to distinguish "you" from others)
    pub active_wallet: Option<Address>,
}

impl Default for NarrativeConfig {
    fn default() -> Self {
        Self {
            show_full_addresses: false,
            active_wallet: None,
        }
    }
}

/// Narrative generator for transactions
///
/// Generates human-readable narratives from decoded transaction data using
/// templates and contextual information.
pub struct NarrativeGenerator {
    /// Configuration for narrative generation
    config: NarrativeConfig,
    /// Optional EVM client for querying token metadata (decimals)
    evm_client: Option<crate::protocols::evm::client::EvmClient>,
}

impl NarrativeGenerator {
    /// Create a new narrative generator
    ///
    /// # Arguments
    /// * `active_wallet` - Optional wallet address to use "you" instead of address
    pub fn new(active_wallet: Option<Address>) -> Self {
        Self {
            config: NarrativeConfig {
                show_full_addresses: false,
                active_wallet,
            },
            evm_client: None,
        }
    }

    /// Create a new narrative generator with EVM client for token metadata queries
    ///
    /// # Arguments
    /// * `active_wallet` - Optional wallet address to use "you" instead of address
    /// * `evm_client` - EVM client for querying token decimals
    pub fn new_with_client(
        active_wallet: Option<Address>,
        evm_client: crate::protocols::evm::client::EvmClient,
    ) -> Self {
        Self {
            config: NarrativeConfig {
                show_full_addresses: false,
                active_wallet,
            },
            evm_client: Some(evm_client),
        }
    }

    /// Generate a narrative for a single transaction
    ///
    /// # Arguments
    /// * `decoded` - Decoded transaction call data
    /// * `from` - Transaction sender address
    /// * `to` - Transaction recipient/contract address
    /// * `tx_hash` - Transaction hash
    /// * `success` - Whether the transaction succeeded
    ///
    /// # Returns
    /// * Human-readable narrative string
    pub async fn generate_narrative(
        &self,
        decoded: &DecodedCall,
        from: Address,
        to: Option<Address>,
        tx_hash: B256,
        success: bool,
    ) -> String {
        let from_str = self.format_address(from);
        let to_str = to.map(|a| self.format_address(a));

        let status_suffix = if !success {
            " (transaction failed)"
        } else {
            ""
        };

        // Generate narrative based on contract type and function
        let narrative = match decoded.contract_type {
            ContractType::ERC20 => {
                self.generate_erc20_narrative(decoded, &from_str, &to_str, to)
                    .await
            }
            ContractType::PrimarySale => {
                self.generate_primary_sale_narrative(decoded, &from_str, &to_str, to)
                    .await
            }
            ContractType::Allowlist => {
                self.generate_allowlist_narrative(decoded, &from_str, &to_str)
            }
            ContractType::ERC721 => self.generate_erc721_narrative(decoded, &from_str, &to_str),
            ContractType::ContractCreation => {
                self.generate_contract_creation_narrative(decoded, &from_str)
            }
            ContractType::Unknown => self.generate_unknown_narrative(decoded, &from_str, &to_str),
        };

        format!(
            "{}{} [tx: {}]",
            narrative,
            status_suffix,
            self.format_hash(tx_hash)
        )
    }

    /// Generate sequential narrative from multiple transactions
    ///
    /// Uses connectors like "First", "Then", "Finally" for readability.
    ///
    /// # Arguments
    /// * `narratives` - Vector of individual transaction narratives
    ///
    /// # Returns
    /// * Combined narrative with sequential connectors
    pub fn generate_sequential_narrative(&self, narratives: Vec<String>) -> String {
        if narratives.is_empty() {
            return "No transactions found.".to_string();
        }

        if narratives.len() == 1 {
            return narratives[0].clone();
        }

        let mut result = String::new();
        for (i, narrative) in narratives.iter().enumerate() {
            let connector = match i {
                0 => "First",
                i if i == narratives.len() - 1 => "Finally",
                _ => "Then",
            };

            result.push_str(&format!("{}. {}\n", connector, narrative));
        }

        result
    }

    // Address and amount formatting
    // =========================================================================

    /// Format address for display
    fn format_address(&self, addr: Address) -> String {
        if let Some(active) = self.config.active_wallet {
            if addr == active {
                return "you".to_string();
            }
        }

        if self.config.show_full_addresses {
            format!("{:?}", addr)
        } else {
            self.abbreviate_address(addr)
        }
    }

    /// Abbreviate address to 0x1234...5678 format
    fn abbreviate_address(&self, addr: Address) -> String {
        let full = format!("{:?}", addr);
        if full.len() > 10 {
            format!("{}...{}", &full[..6], &full[full.len() - 4..])
        } else {
            full
        }
    }

    /// Parse and format an address string (e.g., "0x1234..." to "0x1234...5678")
    fn parse_and_format_address(&self, addr_str: &str) -> String {
        match Address::from_str(addr_str) {
            Ok(addr) => self.format_address(addr),
            Err(_) => addr_str.to_string(), // Fallback to raw string if parsing fails
        }
    }

    /// Format transaction hash
    fn format_hash(&self, hash: B256) -> String {
        let full = format!("{:?}", hash);
        if full.len() > 10 {
            format!("{}...{}", &full[..6], &full[full.len() - 4..])
        } else {
            full
        }
    }

    /// Format token amount with proper decimals
    ///
    /// Note: This is a simplified version. In production, you'd query
    /// the token contract for actual decimals.
    fn format_amount(&self, amount: &str, decimals: u8) -> String {
        use alloy_primitives::U256;
        use std::str::FromStr;

        // Parse U256 from string (supports full 256-bit range)
        let amount_u256 = match U256::from_str(amount) {
            Ok(val) => val,
            Err(_) => {
                // Fallback: return raw string if parsing fails
                return amount.to_string();
            }
        };

        // Calculate divisor: 10^decimals as U256
        let divisor = U256::from(10).pow(U256::from(decimals));

        // Perform division and modulo using U256 arithmetic
        let whole = amount_u256 / divisor;
        let fraction = amount_u256 % divisor;

        if fraction.is_zero() {
            format!("{}", whole)
        } else {
            // Convert fraction to string with leading zeros
            let frac_str = format!("{:0width$}", fraction, width = decimals as usize);
            let frac_str = frac_str.trim_end_matches('0');
            format!("{}.{}", whole, frac_str)
        }
    }

    // Contract-specific narrative generators
    // =========================================================================

    /// Generate narrative for ERC-20 transactions
    async fn generate_erc20_narrative(
        &self,
        decoded: &DecodedCall,
        from: &str,
        to_str: &Option<String>,
        to_address: Option<Address>,
    ) -> String {
        // Query token decimals if we have an EVM client and contract address
        let decimals = if let (Some(client), Some(addr)) = (&self.evm_client, to_address) {
            client
                .token_metadata_cache()
                .get_decimals(addr, client)
                .await
        } else {
            // Fallback to 18 decimals (ERC-20 standard default) if no client available
            18
        };
        match decoded.function_name.as_str() {
            "transfer" => {
                let recipient = decoded
                    .parameters
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                let formatted_recipient = self.parse_and_format_address(recipient);
                let formatted_amount = self.format_amount(amount, decimals);
                let to_contract = to_str.as_deref().unwrap_or("unknown contract");

                format!(
                    "{} transferred {} tokens to {} via contract at {}",
                    from, formatted_amount, formatted_recipient, to_contract
                )
            }
            "approve" => {
                let spender = decoded
                    .parameters
                    .get("spender")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                let formatted_spender = self.parse_and_format_address(spender);
                let formatted_amount = self.format_amount(amount, decimals);
                let to_contract = to_str.as_deref().unwrap_or("unknown contract");

                format!(
                    "{} approved {} to spend {} tokens from contract at {}",
                    from, formatted_spender, formatted_amount, to_contract
                )
            }
            "transferFrom" => {
                let from_param = decoded
                    .parameters
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let to_param = decoded
                    .parameters
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                let formatted_from = self.parse_and_format_address(from_param);
                let formatted_to = self.parse_and_format_address(to_param);
                let formatted_amount = self.format_amount(amount, decimals);
                let to_contract = to_str.as_deref().unwrap_or("unknown contract");

                format!(
                    "{} transferred {} tokens from {} to {} via contract at {}",
                    from, formatted_amount, formatted_from, formatted_to, to_contract
                )
            }
            "mint" => {
                let recipient = decoded
                    .parameters
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                let formatted_recipient = self.parse_and_format_address(recipient);
                let formatted_amount = self.format_amount(amount, decimals);
                let to_contract = to_str.as_deref().unwrap_or("unknown contract");

                format!(
                    "{} minted {} tokens to {} via contract at {}",
                    from, formatted_amount, formatted_recipient, to_contract
                )
            }
            _ => format!("{} called unknown ERC-20 function", from),
        }
    }

    /// Generate narrative for PrimarySale transactions
    async fn generate_primary_sale_narrative(
        &self,
        decoded: &DecodedCall,
        from: &str,
        to: &Option<String>,
        _to_address: Option<Address>,
    ) -> String {
        let contract = to.as_deref().unwrap_or("unknown contract");

        match decoded.function_name.as_str() {
            "invest" => {
                let token = decoded
                    .parameters
                    .get("token")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown token");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                // Query decimals for the token being invested (e.g., mantraUSD)
                let decimals = if let (Some(client), Ok(token_addr)) =
                    (&self.evm_client, Address::from_str(token))
                {
                    client
                        .token_metadata_cache()
                        .get_decimals(token_addr, client)
                        .await
                } else {
                    6 // Fallback to 6 decimals (typical for stablecoins like mantraUSD)
                };

                let formatted_amount = self.format_amount(amount, decimals);

                format!(
                    "{} invested {} tokens ({}) in primary sale at {}",
                    from, formatted_amount, token, contract
                )
            }
            "activate" => {
                format!("{} activated primary sale at {}", from, contract)
            }
            "endSale" => {
                format!("{} ended primary sale at {}", from, contract)
            }
            "initializeSettlement" => {
                let asset_token = decoded
                    .parameters
                    .get("assetToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let asset_owner = decoded
                    .parameters
                    .get("assetOwner")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                let formatted_token = self.parse_and_format_address(asset_token);
                let formatted_owner = self.parse_and_format_address(asset_owner);

                format!(
                    "{} initialized settlement with asset token {} from owner {} at {}",
                    from, formatted_token, formatted_owner, contract
                )
            }
            "settleBatch" => {
                let batch_size = decoded
                    .parameters
                    .get("batchSize")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");
                let restricted_wallets = decoded
                    .parameters
                    .get("restrictedWallets")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                if restricted_wallets == 0 {
                    format!(
                        "{} settled batch of {} investors at {}",
                        from, batch_size, contract
                    )
                } else {
                    format!(
                        "{} settled batch of {} investors (excluded {} restricted wallets) at {}",
                        from, batch_size, restricted_wallets, contract
                    )
                }
            }
            "finalizeSettlement" => {
                format!("{} finalized settlement at {}", from, contract)
            }
            "settleAndDistribute" => {
                let asset_token = decoded
                    .parameters
                    .get("assetToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let max_loop = decoded
                    .parameters
                    .get("maxLoop")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                format!(
                    "{} settled and distributed {} tokens to {} investors at {}",
                    from, asset_token, max_loop, contract
                )
            }
            "claimRefund" => {
                format!("{} claimed refund from primary sale at {}", from, contract)
            }
            "cancel" => {
                format!("{} cancelled primary sale at {}", from, contract)
            }
            "pause" => {
                format!("{} paused primary sale at {}", from, contract)
            }
            "unpause" => {
                format!("{} unpaused primary sale at {}", from, contract)
            }
            "topUpRefunds" => {
                let token = decoded
                    .parameters
                    .get("token")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown token");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                // Query decimals for the token being used for refunds
                let decimals = if let (Some(client), Ok(token_addr)) =
                    (&self.evm_client, Address::from_str(token))
                {
                    client
                        .token_metadata_cache()
                        .get_decimals(token_addr, client)
                        .await
                } else {
                    6 // Fallback to 6 decimals (typical for stablecoins)
                };

                let formatted_amount = self.format_amount(amount, decimals);

                format!(
                    "{} topped up refund pool with {} tokens ({}) at {}",
                    from, formatted_amount, token, contract
                )
            }
            "emergencyWithdraw" => {
                let token = decoded
                    .parameters
                    .get("token")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown token");
                let recipient = decoded
                    .parameters
                    .get("recipient")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let amount = decoded
                    .parameters
                    .get("amount")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                let formatted_amount = self.format_amount(amount, 6);

                format!(
                    "{} emergency withdrew {} tokens ({}) to {} from {}",
                    from, formatted_amount, token, recipient, contract
                )
            }
            _ => format!(
                "{} called unknown PrimarySale function at {}",
                from, contract
            ),
        }
    }

    /// Generate narrative for Allowlist transactions
    fn generate_allowlist_narrative(
        &self,
        decoded: &DecodedCall,
        from: &str,
        to: &Option<String>,
    ) -> String {
        let contract = to.as_deref().unwrap_or("unknown contract");

        match decoded.function_name.as_str() {
            "setAllowedBatch" => {
                let total_count = decoded
                    .parameters
                    .get("total_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let added_count = decoded
                    .parameters
                    .get("added_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let removed_count = decoded
                    .parameters
                    .get("removed_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                if removed_count == 0 {
                    format!(
                        "{} updated allowlist for {} addresses (added: {}) at {}",
                        from, total_count, added_count, contract
                    )
                } else if added_count == 0 {
                    format!(
                        "{} updated allowlist for {} addresses (removed: {}) at {}",
                        from, total_count, removed_count, contract
                    )
                } else {
                    format!(
                        "{} updated allowlist for {} addresses (added: {}, removed: {}) at {}",
                        from, total_count, added_count, removed_count, contract
                    )
                }
            }
            _ => format!("{} called unknown Allowlist function at {}", from, contract),
        }
    }

    /// Generate narrative for ERC-721 transactions
    fn generate_erc721_narrative(
        &self,
        decoded: &DecodedCall,
        from: &str,
        to: &Option<String>,
    ) -> String {
        let contract = to.as_deref().unwrap_or("unknown contract");
        format!(
            "{} called ERC-721 function '{}' at {}",
            from, decoded.function_name, contract
        )
    }

    /// Generate narrative for contract creation (deployment) transactions
    ///
    /// This is handled early in the transaction processing pipeline and already
    /// includes the deployed contract address. This method serves as a fallback
    /// or for direct NarrativeGenerator usage without the MCP adapter.
    pub fn generate_contract_creation_narrative(
        &self,
        decoded: &DecodedCall,
        from: &str,
    ) -> String {
        // Try to extract contract address from decoded parameters if available
        if let Some(contract_addr) = decoded.parameters.get("contract_address") {
            format!("{} deployed contract at {}", from, contract_addr)
        } else {
            // Fallback when address is not in parameters
            format!("{} deployed contract", from)
        }
    }

    /// Generate narrative for unknown contract transactions
    fn generate_unknown_narrative(
        &self,
        decoded: &DecodedCall,
        from: &str,
        to: &Option<String>,
    ) -> String {
        let contract = to.as_deref().unwrap_or("unknown contract");

        if decoded.function_name == "unknown" {
            format!(
                "{} called unknown function {} at {}",
                from, decoded.selector, contract
            )
        } else if to.is_none() {
            format!("{} deployed contract", from)
        } else {
            format!(
                "{} called function '{}' at {}",
                from, decoded.function_name, contract
            )
        }
    }
}

impl Default for NarrativeGenerator {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::evm::transaction_decoder::TransactionDecoder;
    use alloy_primitives::{address, U256};
    use alloy_sol_types::SolCall;

    #[test]
    fn test_format_address_abbreviation() {
        let generator = NarrativeGenerator::new(None);
        let addr = address!("4444444444444444444444444444444444444444");

        let formatted = generator.abbreviate_address(addr);
        assert!(formatted.starts_with("0x4444"));
        assert!(formatted.ends_with("4444"));
        assert!(formatted.contains("..."));
    }

    #[test]
    fn test_format_address_active_wallet() {
        let active = address!("5555555555555555555555555555555555555555");
        let generator = NarrativeGenerator::new(Some(active));

        let formatted = generator.format_address(active);
        assert_eq!(formatted, "you");
    }

    #[test]
    fn test_format_amount() {
        let generator = NarrativeGenerator::new(None);

        // 1000000 with 6 decimals = 1.0
        assert_eq!(generator.format_amount("1000000", 6), "1");

        // 1500000 with 6 decimals = 1.5
        assert_eq!(generator.format_amount("1500000", 6), "1.5");

        // 1234567 with 6 decimals = 1.234567
        assert_eq!(generator.format_amount("1234567", 6), "1.234567");
    }

    #[tokio::test]
    async fn test_erc20_transfer_narrative() {
        use crate::protocols::evm::contracts::erc20::IERC20;

        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new(None);

        let from = address!("1111111111111111111111111111111111111111");
        let to = address!("2222222222222222222222222222222222222222");
        let recipient = address!("3333333333333333333333333333333333333333");
        let amount = U256::from(1000000u64); // 1.0 with 6 decimals

        let call = IERC20::transferCall {
            to: recipient,
            amount,
        };
        let input = call.abi_encode();

        let decoded = decoder.decode(&input, Some(to)).unwrap();
        let narrative = generator
            .generate_narrative(&decoded, from, Some(to), B256::ZERO, true)
            .await;

        assert!(narrative.contains("transferred"));
        assert!(narrative.contains("tokens"));
        assert!(narrative.contains(&generator.abbreviate_address(recipient)));
    }

    #[test]
    fn test_sequential_narrative() {
        let generator = NarrativeGenerator::new(None);

        let narratives = vec![
            "Transaction 1".to_string(),
            "Transaction 2".to_string(),
            "Transaction 3".to_string(),
        ];

        let result = generator.generate_sequential_narrative(narratives);

        assert!(result.contains("First. Transaction 1"));
        assert!(result.contains("Then. Transaction 2"));
        assert!(result.contains("Finally. Transaction 3"));
    }

    #[test]
    fn test_empty_sequential_narrative() {
        let generator = NarrativeGenerator::new(None);
        let result = generator.generate_sequential_narrative(vec![]);

        assert_eq!(result, "No transactions found.");
    }

    #[tokio::test]
    async fn test_erc20_mint_narrative() {
        use crate::protocols::evm::contracts::erc20::IERC20;

        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new(None);

        let from = address!("1111111111111111111111111111111111111111");
        let to_contract = address!("2222222222222222222222222222222222222222");
        let recipient = address!("3333333333333333333333333333333333333333");
        let amount = U256::from(1000000u64); // 1.0 with 6 decimals

        let call = IERC20::mintCall {
            to: recipient,
            amount,
        };
        let input = call.abi_encode();

        let decoded = decoder.decode(&input, Some(to_contract)).unwrap();
        let narrative = generator
            .generate_narrative(&decoded, from, Some(to_contract), B256::ZERO, true)
            .await;

        assert!(narrative.contains("minted"));
        assert!(narrative.contains("tokens"));
        assert!(narrative.contains(&generator.abbreviate_address(recipient)));
        assert!(narrative.contains("via contract"));
    }

    #[tokio::test]
    async fn test_allowlist_narrative() {
        use crate::protocols::evm::contracts::allowlist::IAllowlist;

        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new(None);

        let from = address!("1111111111111111111111111111111111111111");
        let contract = address!("2222222222222222222222222222222222222222");

        // Test with 3 addresses: 2 added, 1 removed
        let addrs = vec![
            address!("3333333333333333333333333333333333333333"),
            address!("4444444444444444444444444444444444444444"),
            address!("5555555555555555555555555555555555555555"),
        ];
        let flags = vec![true, true, false];

        let call = IAllowlist::setAllowedBatchCall { addrs, flags };
        let input = call.abi_encode();

        let decoded = decoder.decode(&input, Some(contract)).unwrap();
        let narrative = generator
            .generate_narrative(&decoded, from, Some(contract), B256::ZERO, true)
            .await;

        assert!(narrative.contains("updated allowlist"));
        assert!(narrative.contains("3 addresses"));
        assert!(narrative.contains("added: 2"));
        assert!(narrative.contains("removed: 1"));
    }

    #[tokio::test]
    async fn test_settlement_narratives() {
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;

        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new(None);

        let from = address!("1111111111111111111111111111111111111111");
        let contract = address!("2222222222222222222222222222222222222222");

        // Test initializeSettlement
        let asset_token = address!("3333333333333333333333333333333333333333");
        let asset_owner = address!("4444444444444444444444444444444444444444");

        let init_call = IPrimarySale::initializeSettlementCall {
            assetToken: asset_token,
            assetOwner: asset_owner,
        };
        let init_input = init_call.abi_encode();

        let init_decoded = decoder.decode(&init_input, Some(contract)).unwrap();
        let init_narrative = generator
            .generate_narrative(&init_decoded, from, Some(contract), B256::ZERO, true)
            .await;

        assert!(init_narrative.contains("initialized settlement"));
        assert!(init_narrative.contains("asset token"));
        assert!(init_narrative.contains("from owner"));

        // Test settleBatch
        let batch_size = U256::from(250u64);
        let restricted_wallets = vec![
            address!("5555555555555555555555555555555555555555"),
            address!("6666666666666666666666666666666666666666"),
        ];

        let batch_call = IPrimarySale::settleBatchCall {
            batchSize: batch_size,
            restrictedWallets: restricted_wallets,
        };
        let batch_input = batch_call.abi_encode();

        let batch_decoded = decoder.decode(&batch_input, Some(contract)).unwrap();
        let batch_narrative = generator
            .generate_narrative(&batch_decoded, from, Some(contract), B256::ZERO, true)
            .await;

        assert!(batch_narrative.contains("settled batch"));
        assert!(batch_narrative.contains("250 investors"));
        assert!(batch_narrative.contains("excluded 2 restricted wallets"));

        // Test finalizeSettlement
        let finalize_call = IPrimarySale::finalizeSettlementCall {};
        let finalize_input = finalize_call.abi_encode();

        let finalize_decoded = decoder.decode(&finalize_input, Some(contract)).unwrap();
        let finalize_narrative = generator
            .generate_narrative(&finalize_decoded, from, Some(contract), B256::ZERO, true)
            .await;

        assert!(finalize_narrative.contains("finalized settlement"));
    }

    #[tokio::test]
    async fn test_settlement_batch_no_restrictions() {
        use crate::protocols::evm::contracts::primary_sale::IPrimarySale;

        let decoder = TransactionDecoder::new();
        let generator = NarrativeGenerator::new(None);

        let from = address!("1111111111111111111111111111111111111111");
        let contract = address!("2222222222222222222222222222222222222222");

        // Test settleBatch with no restricted wallets
        let batch_size = U256::from(100u64);
        let restricted_wallets = vec![];

        let batch_call = IPrimarySale::settleBatchCall {
            batchSize: batch_size,
            restrictedWallets: restricted_wallets,
        };
        let batch_input = batch_call.abi_encode();

        let batch_decoded = decoder.decode(&batch_input, Some(contract)).unwrap();
        let batch_narrative = generator
            .generate_narrative(&batch_decoded, from, Some(contract), B256::ZERO, true)
            .await;

        assert!(batch_narrative.contains("settled batch of 100 investors"));
        assert!(!batch_narrative.contains("excluded")); // Should not mention restrictions
    }
}

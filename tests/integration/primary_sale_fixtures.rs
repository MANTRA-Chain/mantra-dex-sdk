//! Test fixtures for PrimarySale integration tests
//!
//! This module provides mock data, constants, and helper functions for testing
//! PrimarySale contract operations without requiring testnet access.

use alloy_primitives::{Address, U256};
use mantra_sdk::protocols::evm::contracts::primary_sale::MAX_SETTLEMENT_INVESTORS;
use serde_json::{json, Value};
use std::str::FromStr;

// Mock contract addresses for testing
pub const MOCK_CONTRACT_ADDR: &str = "0x1111111111111111111111111111111111111111";
pub const MOCK_INVESTOR_ADDR: &str = "0x2222222222222222222222222222222222222222";
pub const MOCK_MANTRA_USD_ADDR: &str = "0x3333333333333333333333333333333333333333";
pub const MOCK_USDC_ADDR: &str = "0x3333333333333333333333333333333333333334";
pub const MOCK_USDT_ADDR: &str = "0x3333333333333333333333333333333333333335";
#[allow(dead_code)]
pub const MOCK_ASSET_TOKEN_ADDR: &str = "0x4444444444444444444444444444444444444444";
pub const MOCK_ALLOWLIST_ADDR: &str = "0x5555555555555555555555555555555555555555";
pub const MOCK_MULTISIG_ADDR: &str = "0x6666666666666666666666666666666666666666";
pub const MOCK_ISSUER_ADDR: &str = "0x7777777777777777777777777777777777777777";

/// Mock sale information with different states (v2.0)
pub struct MockSaleInfo {
    pub status: u8,
    pub status_name: &'static str,
    pub name: &'static str, // NEW v2.0
    pub hard_cap: U256,     // NEW v2.0
    pub is_active: bool,
    pub start_time: u64,
    pub end_time: u64,
    pub soft_cap: U256,
    pub total_contributed_normalized: U256, // RENAMED in v2.0
    pub remaining_capacity: U256,           // NEW v2.0
    pub accepted_tokens: Vec<&'static str>, // NEW v2.0
    pub investor_count: U256,
    pub commission_bps: u16,
}

impl MockSaleInfo {
    /// Create a sale in Active state with 50% soft cap progress
    pub fn active_sale() -> Self {
        let soft_cap = U256::from(100_000_000_000_000_000_000u128);
        let total = U256::from(50_000_000_000_000_000_000u128);
        let hard_cap = U256::from(200_000_000_000_000_000_000u128);
        Self {
            status: 1,
            status_name: "Active",
            name: "Active Test Sale",
            hard_cap,
            is_active: true,
            start_time: timestamps::past_start(),
            end_time: timestamps::future_end(),
            soft_cap,
            total_contributed_normalized: total,
            remaining_capacity: hard_cap - total,
            accepted_tokens: vec![MOCK_MANTRA_USD_ADDR, MOCK_USDC_ADDR, MOCK_USDT_ADDR],
            investor_count: U256::from(10),
            commission_bps: 50,
        }
    }

    /// Create a sale in Pending state
    pub fn pending_sale() -> Self {
        let hard_cap = U256::from(200_000_000_000_000_000_000u128);
        Self {
            status: 0,
            status_name: "Pending",
            name: "Pending Test Sale",
            hard_cap,
            is_active: false,
            start_time: timestamps::future_start(),
            end_time: timestamps::future_end(),
            soft_cap: U256::from(100_000_000_000_000_000_000u128),
            total_contributed_normalized: U256::ZERO,
            remaining_capacity: hard_cap,
            accepted_tokens: vec![MOCK_MANTRA_USD_ADDR, MOCK_USDC_ADDR, MOCK_USDT_ADDR],
            investor_count: U256::ZERO,
            commission_bps: 50,
        }
    }

    /// Create a sale in Ended state (soft cap met)
    pub fn ended_sale() -> Self {
        let total = U256::from(150_000_000_000_000_000_000u128);
        let hard_cap = U256::from(200_000_000_000_000_000_000u128);
        Self {
            status: 2,
            status_name: "Ended",
            name: "Ended Test Sale",
            hard_cap,
            is_active: false,
            start_time: relative_timestamp(-60),
            end_time: timestamps::past_end(),
            soft_cap: U256::from(100_000_000_000_000_000_000u128),
            total_contributed_normalized: total,
            remaining_capacity: hard_cap - total,
            accepted_tokens: vec![MOCK_MANTRA_USD_ADDR, MOCK_USDC_ADDR, MOCK_USDT_ADDR],
            investor_count: U256::from(42),
            commission_bps: 50,
        }
    }

    /// Create a sale in Failed state (soft cap not met)
    pub fn failed_sale() -> Self {
        let total = U256::from(50_000_000_000_000_000_000u128);
        let hard_cap = U256::from(200_000_000_000_000_000_000u128);
        Self {
            status: 3,
            status_name: "Failed",
            name: "Failed Test Sale",
            hard_cap,
            is_active: false,
            start_time: relative_timestamp(-60),
            end_time: timestamps::past_end(),
            soft_cap: U256::from(100_000_000_000_000_000_000u128),
            total_contributed_normalized: total,
            remaining_capacity: hard_cap - total,
            accepted_tokens: vec![MOCK_MANTRA_USD_ADDR, MOCK_USDC_ADDR, MOCK_USDT_ADDR],
            investor_count: U256::from(5),
            commission_bps: 50,
        }
    }

    /// Create a sale in Settled state
    pub fn settled_sale() -> Self {
        let total = U256::from(200_000_000_000_000_000_000u128);
        let hard_cap = U256::from(200_000_000_000_000_000_000u128);
        Self {
            status: 4,
            status_name: "Settled",
            name: "Settled Test Sale",
            hard_cap,
            is_active: false,
            start_time: relative_timestamp(-90),
            end_time: relative_timestamp(-30),
            soft_cap: U256::from(100_000_000_000_000_000_000u128),
            total_contributed_normalized: total,
            remaining_capacity: U256::ZERO,
            accepted_tokens: vec![MOCK_MANTRA_USD_ADDR, MOCK_USDC_ADDR, MOCK_USDT_ADDR],
            investor_count: U256::from(100),
            commission_bps: 50,
        }
    }

    /// Create a sale in Cancelled state
    pub fn cancelled_sale() -> Self {
        let total = U256::from(25_000_000_000_000_000_000u128);
        let hard_cap = U256::from(200_000_000_000_000_000_000u128);
        Self {
            status: 5,
            status_name: "Cancelled",
            name: "Cancelled Test Sale",
            hard_cap,
            is_active: false,
            start_time: relative_timestamp(-60),
            end_time: timestamps::future_end(),
            soft_cap: U256::from(100_000_000_000_000_000_000u128),
            total_contributed_normalized: total,
            remaining_capacity: hard_cap - total,
            accepted_tokens: vec![MOCK_MANTRA_USD_ADDR, MOCK_USDC_ADDR, MOCK_USDT_ADDR],
            investor_count: U256::from(3),
            commission_bps: 50,
        }
    }

    /// Convert to JSON response structure (v2.0 format)
    pub fn to_json_response(&self, contract_address: &str) -> Value {
        let current_time = now_timestamp();
        let remaining_time = if current_time < self.end_time {
            self.end_time - current_time
        } else {
            0
        };

        json!({
            "status": "success",
            "operation": "primary_sale_get_sale_info",
            "contract_address": contract_address,
            "sale": {
                "name": self.name,
                "hard_cap": self.hard_cap.to_string(),
                "status": self.status_name,
                "status_code": self.status,
                "is_active": self.is_active,
                "start_time": self.start_time,
                "end_time": self.end_time,
                "remaining_time_seconds": remaining_time,
                "soft_cap": self.soft_cap.to_string(),
                "total_contributed_normalized": self.total_contributed_normalized.to_string(),
                "remaining_capacity": self.remaining_capacity.to_string(),
                "accepted_tokens": self.accepted_tokens,
                "investor_count": self.investor_count.to_string(),
                "commission_bps": self.commission_bps
            },
            "contracts": {
                "allowlist": MOCK_ALLOWLIST_ADDR,
                "multisig": MOCK_MULTISIG_ADDR,
                "issuer": MOCK_ISSUER_ADDR
            },
            "notes": {
                "multi_token": "v2.0 supports multiple payment tokens",
                "normalized_amounts": "All amounts normalized to 18 decimals"
            },
            "timestamp": "2024-07-03T00:00:00Z"
        })
    }
}

/// Mock investor information (v2.0)
pub struct MockInvestorInfo {
    pub address: String,
    pub contribution_normalized: U256, // RENAMED in v2.0
    pub contributions_by_token: Vec<(String, U256)>, // NEW v2.0
    pub tokens_allocated: U256,
    pub is_kyc_approved: bool,         // NEW v2.0
    pub has_received_settlement: bool, // NEW v2.0 (renamed from has_claimed_refund)
}

impl MockInvestorInfo {
    /// Create investor with contribution (v2.0)
    pub fn with_contribution(amount: u128) -> Self {
        let normalized_amount = U256::from(amount * 1_000_000_000_000_000_000); // 18 decimals
        Self {
            address: MOCK_INVESTOR_ADDR.to_string(),
            contribution_normalized: normalized_amount,
            contributions_by_token: vec![
                (
                    MOCK_MANTRA_USD_ADDR.to_string(),
                    normalized_amount / U256::from(2),
                ),
                (
                    MOCK_USDC_ADDR.to_string(),
                    normalized_amount / U256::from(2),
                ),
            ],
            tokens_allocated: U256::from(amount),
            is_kyc_approved: true,
            has_received_settlement: false,
        }
    }

    /// Create investor who has received settlement
    pub fn with_settlement_received() -> Self {
        let amount = U256::from(1000_000_000_000_000_000_000u128);
        Self {
            address: MOCK_INVESTOR_ADDR.to_string(),
            contribution_normalized: amount,
            contributions_by_token: vec![(MOCK_MANTRA_USD_ADDR.to_string(), amount)],
            tokens_allocated: U256::from(1000_000_000_000_000_000_000u128),
            is_kyc_approved: true,
            has_received_settlement: true,
        }
    }

    /// Create investor without KYC approval
    pub fn without_kyc() -> Self {
        let amount = U256::from(500_000_000_000_000_000_000u128);
        Self {
            address: MOCK_INVESTOR_ADDR.to_string(),
            contribution_normalized: amount,
            contributions_by_token: vec![(MOCK_USDC_ADDR.to_string(), amount)],
            tokens_allocated: U256::ZERO,
            is_kyc_approved: false,
            has_received_settlement: false,
        }
    }

    /// Convert to JSON response structure (v2.0 format)
    pub fn to_json_response(&self, contract_address: &str) -> Value {
        let mut contributions_map = serde_json::Map::new();
        for (token, amount) in &self.contributions_by_token {
            contributions_map.insert(token.clone(), json!(amount.to_string()));
        }

        json!({
            "status": "success",
            "operation": "primary_sale_get_investor_info",
            "contract_address": contract_address,
            "investor": {
                "address": &self.address,
                "contribution_normalized": self.contribution_normalized.to_string(),
                "contributions_by_token": contributions_map,
                "tokens_allocated": self.tokens_allocated.to_string(),
                "is_kyc_approved": self.is_kyc_approved,
                "has_received_settlement": self.has_received_settlement
            },
            "notes": {
                "normalized": "contribution_normalized is always 18 decimals",
                "by_token": "contributions_by_token shows raw amounts in each token's native decimals"
            },
            "timestamp": "2024-07-03T00:00:00Z"
        })
    }
}

/// Helper function to create mock transaction response
pub fn mock_transaction_response(operation: &str, tx_hash: &str) -> Value {
    json!({
        "status": "success",
        "operation": operation,
        "transaction_hash": tx_hash,
        "contract_address": MOCK_CONTRACT_ADDR,
        "timestamp": "2024-07-03T00:00:00Z"
    })
}

/// Helper function to create mock settlement response (DEPRECATED - v1.0 only)
#[allow(dead_code)]
pub fn mock_settlement_response(
    total_contributed: &str,
    commission_amount: &str,
    issuer_amount: &str,
    investors_processed: u64,
) -> Value {
    json!({
        "status": "success",
        "operation": "primary_sale_settle_and_distribute",
        "transaction_hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "total_contributed": total_contributed,
        "commission_amount": commission_amount,
        "issuer_amount": issuer_amount,
        "investors_processed": investors_processed.to_string(),
        "max_loop": MAX_SETTLEMENT_INVESTORS,
        "timestamp": "2024-07-03T00:00:00Z"
    })
}

/// Mock settlement progress (v2.0)
pub struct MockSettlementProgress {
    pub processed_investors: U256,
    pub total_investors: U256,
    pub is_initialized: bool,
    pub is_complete: bool,
}

impl MockSettlementProgress {
    /// Settlement not yet started
    pub fn not_started(total: u64) -> Self {
        Self {
            processed_investors: U256::ZERO,
            total_investors: U256::from(total),
            is_initialized: false,
            is_complete: false,
        }
    }

    /// Settlement in progress
    pub fn in_progress(processed: u64, total: u64) -> Self {
        Self {
            processed_investors: U256::from(processed),
            total_investors: U256::from(total),
            is_initialized: true,
            is_complete: false,
        }
    }

    /// Settlement complete
    pub fn complete(total: u64) -> Self {
        Self {
            processed_investors: U256::from(total),
            total_investors: U256::from(total),
            is_initialized: true,
            is_complete: true,
        }
    }

    /// Convert to JSON response
    pub fn to_json_response(&self) -> Value {
        let progress_percentage = if !self.total_investors.is_zero() {
            let pct = (self.processed_investors.saturating_mul(U256::from(10000))
                / self.total_investors)
                .to::<u64>() as f64
                / 100.0;
            format!("{:.2}", pct)
        } else {
            "0.00".to_string()
        };

        json!({
            "status": "success",
            "operation": "primary_sale_get_settlement_progress",
            "processed_investors": self.processed_investors.to_string(),
            "total_investors": self.total_investors.to_string(),
            "is_initialized": self.is_initialized,
            "is_complete": self.is_complete,
            "progress_percentage": progress_percentage,
            "timestamp": "2024-07-03T00:00:00Z"
        })
    }
}

/// Helper to create initialize settlement response (v2.0)
pub fn mock_initialize_settlement_response(total_investors: u64) -> Value {
    json!({
        "status": "success",
        "operation": "primary_sale_initialize_settlement",
        "transaction_hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "contract_address": MOCK_CONTRACT_ADDR,
        "asset_token": MOCK_ASSET_TOKEN_ADDR,
        "asset_owner": MOCK_ISSUER_ADDR,
        "total_investors": total_investors.to_string(),
        "timestamp": "2024-07-03T00:00:00Z"
    })
}

/// Helper to create settle batch response (v2.0)
pub fn mock_settle_batch_response(
    batch_size: u64,
    processed: u64,
    total: u64,
    is_complete: bool,
) -> Value {
    let progress_percentage = if total > 0 {
        format!("{:.2}", (processed as f64 / total as f64) * 100.0)
    } else {
        "0.00".to_string()
    };

    json!({
        "status": "success",
        "operation": "primary_sale_settle_batch",
        "transaction_hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "contract_address": MOCK_CONTRACT_ADDR,
        "batch_size": batch_size,
        "restricted_wallets_count": 0,
        "processed": processed.to_string(),
        "total": total.to_string(),
        "progress_percentage": progress_percentage,
        "is_complete": is_complete,
        "timestamp": "2024-07-03T00:00:00Z"
    })
}

/// Helper to create finalize settlement response (v2.0)
pub fn mock_finalize_settlement_response() -> Value {
    json!({
        "status": "success",
        "operation": "primary_sale_finalize_settlement",
        "transaction_hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
        "contract_address": MOCK_CONTRACT_ADDR,
        "is_complete": true,
        "timestamp": "2024-07-03T00:00:00Z"
    })
}

/// Helper function to create mock pagination response
pub fn mock_investors_list(start: usize, limit: usize, total: usize) -> Value {
    let investors: Vec<String> = (start..std::cmp::min(start + limit, total))
        .map(|i| format!("0x{:040x}", i + 1))
        .collect();

    json!({
        "status": "success",
        "operation": "primary_sale_get_all_investors",
        "contract_address": MOCK_CONTRACT_ADDR,
        "pagination": {
            "start": start,
            "limit": limit,
            "count": investors.len()
        },
        "investors": investors,
        "timestamp": "2024-07-03T00:00:00Z"
    })
}

/// Status code to string mapping
pub fn status_code_to_str(code: u8) -> &'static str {
    match code {
        0 => "Pending",
        1 => "Active",
        2 => "Ended",
        3 => "Failed",
        4 => "Settled",
        5 => "Cancelled",
        _ => "Unknown",
    }
}

/// Validate that a string is a valid Ethereum address
pub fn is_valid_eth_address(addr: &str) -> bool {
    if !addr.starts_with("0x") {
        return false;
    }
    if addr.len() != 42 {
        return false;
    }
    Address::from_str(addr).is_ok()
}

/// Calculate commission amount from total and basis points
///
/// # Arguments
/// * `total` - Total amount in wei
/// * `bps` - Basis points (1 bps = 0.01%, valid range: 0-10000)
///
/// # Returns
/// Ok(commission) if bps valid, Err if bps > 10000
///
/// # Examples
/// ```
/// // 1% commission on 100 tokens
/// let commission = calculate_commission(U256::from(100), 100).unwrap();
/// assert_eq!(commission, U256::from(1));
///
/// // Invalid: 150% commission
/// let result = calculate_commission(U256::from(100), 15000);
/// assert!(result.is_err());
/// ```
pub fn calculate_commission(total: U256, bps: u16) -> Result<U256, String> {
    // Validate basis points (0-10000 = 0-100%)
    if bps > 10000 {
        return Err(format!(
            "Invalid commission basis points: {} (maximum 10000 = 100%).\n\
            \n\
            Basis points represent percentage * 100.\n\
            Examples:\n\
            - 50 bps = 0.5%\n\
            - 100 bps = 1%\n\
            - 10000 bps = 100%\n\
            \n\
            Your value: {} bps = {}%\n\
            Maximum: 10000 bps = 100%",
            bps,
            bps,
            (bps as f64) / 100.0
        ));
    }

    // Calculate: (total * bps) / 10000
    // U256 arithmetic can't overflow but integer division truncates
    Ok((total * U256::from(bps)) / U256::from(10000))
}

// =============================================================================
// Timestamp Helpers for Relative Time Tests
// =============================================================================

/// Get current Unix timestamp
pub fn now_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

/// Create timestamp relative to now
///
/// # Arguments
/// * `days_offset` - Number of days to offset (positive = future, negative = past)
///
/// # Examples
/// ```
/// let yesterday = relative_timestamp(-1);  // 24 hours ago
/// let tomorrow = relative_timestamp(1);    // 24 hours from now
/// ```
pub fn relative_timestamp(days_offset: i64) -> u64 {
    let now = now_timestamp();
    if days_offset >= 0 {
        now + (days_offset as u64 * 86400)
    } else {
        now.saturating_sub((-days_offset) as u64 * 86400)
    }
}

/// Common test timestamps
pub mod timestamps {
    use super::*;

    /// Sale started 30 days ago
    pub fn past_start() -> u64 {
        relative_timestamp(-30)
    }

    /// Sale ends 30 days from now
    pub fn future_end() -> u64 {
        relative_timestamp(30)
    }

    /// Sale ended 7 days ago
    pub fn past_end() -> u64 {
        relative_timestamp(-7)
    }

    /// Sale starts in 7 days
    pub fn future_start() -> u64 {
        relative_timestamp(7)
    }

    /// Sale starts in 1 hour
    pub fn imminent_start() -> u64 {
        now_timestamp() + 3600
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_validation() {
        assert!(is_valid_eth_address(MOCK_CONTRACT_ADDR));
        assert!(is_valid_eth_address(MOCK_INVESTOR_ADDR));
        assert!(!is_valid_eth_address("not_an_address"));
        assert!(!is_valid_eth_address("0x123")); // Too short
    }

    #[test]
    fn test_status_mapping() {
        assert_eq!(status_code_to_str(0), "Pending");
        assert_eq!(status_code_to_str(1), "Active");
        assert_eq!(status_code_to_str(2), "Ended");
        assert_eq!(status_code_to_str(3), "Failed");
        assert_eq!(status_code_to_str(4), "Settled");
        assert_eq!(status_code_to_str(5), "Cancelled");
        assert_eq!(status_code_to_str(99), "Unknown");
    }

    #[test]
    fn test_commission_calculation() {
        // Test 0.5% commission (50 bps) on 100 tokens
        let total = U256::from(100_000_000_000_000_000_000u128); // 100 tokens with 18 decimals
        let commission = calculate_commission(total, 50).unwrap();
        let expected = U256::from(500_000_000_000_000_000u128); // 0.5 tokens
        assert_eq!(commission, expected);

        // Test 1% commission (100 bps)
        let commission = calculate_commission(total, 100).unwrap();
        let expected = U256::from(1_000_000_000_000_000_000u128); // 1 token
        assert_eq!(commission, expected);

        // Test validation: invalid bps > 10000
        let result = calculate_commission(total, 10001);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maximum 10000"));

        // Test validation: max valid bps (10000 = 100%)
        let commission = calculate_commission(total, 10000).unwrap();
        let expected = total; // 100% commission
        assert_eq!(commission, expected);
    }

    #[test]
    fn test_mock_sale_info_states() {
        let active = MockSaleInfo::active_sale();
        assert_eq!(active.status, 1);
        assert!(active.is_active);
        assert!(active.total_contributed_normalized > U256::ZERO);
        assert!(!active.accepted_tokens.is_empty());
        assert!(active.remaining_capacity > U256::ZERO);

        let pending = MockSaleInfo::pending_sale();
        assert_eq!(pending.status, 0);
        assert!(!pending.is_active);
        assert_eq!(pending.total_contributed_normalized, U256::ZERO);
        assert_eq!(pending.remaining_capacity, pending.hard_cap);

        let failed = MockSaleInfo::failed_sale();
        assert_eq!(failed.status, 3);
        assert!(failed.total_contributed_normalized < failed.soft_cap);
    }
}

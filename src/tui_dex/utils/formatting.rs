//! Text and Number Formatting Utilities for TUI
//!
//! This module provides utility functions for formatting addresses,
//! amounts, and other data for display in the TUI.

/// Format a long address for display (show first 8 and last 8 characters)
pub fn format_address(address: &str) -> String {
    if address.len() <= 16 {
        address.to_string()
    } else {
        format!("{}...{}", &address[..8], &address[address.len() - 8..])
    }
}

/// Format an amount with appropriate decimal places
pub fn format_amount(amount: &str, decimals: usize) -> String {
    match amount.parse::<f64>() {
        Ok(num) => format!("{:.prec$}", num, prec = decimals),
        Err(_) => amount.to_string(),
    }
}

/// Format a percentage with 2 decimal places
pub fn format_percentage(value: f64) -> String {
    format!("{:.2}%", value)
}

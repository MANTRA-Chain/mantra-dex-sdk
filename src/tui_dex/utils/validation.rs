//! Validation utilities for TUI forms
//!
//! This module provides validation functions for common input types
//! in the MANTRA DEX TUI application.

/// Validate a wallet address
pub fn validate_address(address: &str) -> Result<(), String> {
    if address.starts_with("mantra") && address.len() >= 40 {
        Ok(())
    } else {
        Err("Invalid address format (should start with 'mantra')".to_string())
    }
}

/// Validate a numeric amount
pub fn validate_amount(amount: &str) -> Result<f64, String> {
    match amount.parse::<f64>() {
        Ok(value) if value >= 0.0 => Ok(value),
        Ok(_) => Err("Amount must be positive".to_string()),
        Err(_) => Err("Invalid number format".to_string()),
    }
}

/// Validate a pool ID
pub fn validate_pool_id(pool_id: &str) -> Result<u64, String> {
    match pool_id.parse::<u64>() {
        Ok(id) => Ok(id),
        Err(_) => Err("Pool ID must be a valid number".to_string()),
    }
}

/// Validate an email address (basic validation)
pub fn validate_email(email: &str) -> Result<(), String> {
    if email.contains('@') && email.contains('.') {
        Ok(())
    } else {
        Err("Invalid email format".to_string())
    }
}

/// Validate password strength
pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() >= 8 {
        Ok(())
    } else {
        Err("Password must be at least 8 characters".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address() {
        assert!(validate_address("mantra1234567890123456789012345678901234567890").is_ok());
        assert!(validate_address("cosmos123").is_err());
        assert!(validate_address("short").is_err());
    }

    #[test]
    fn test_validate_amount() {
        assert!(validate_amount("100.5").is_ok());
        assert!(validate_amount("0").is_ok());
        assert!(validate_amount("-10").is_err());
        assert!(validate_amount("not_a_number").is_err());
    }

    #[test]
    fn test_validate_pool_id() {
        assert!(validate_pool_id("123").is_ok());
        assert!(validate_pool_id("0").is_ok());
        assert!(validate_pool_id("abc").is_err());
    }
}

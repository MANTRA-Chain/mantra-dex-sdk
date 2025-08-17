mod utils;

#[cfg(feature = "mcp")]
use mantra_sdk::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};

/// Test slippage validation in execute_swap_simple function
#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_slippage_validation_invalid_format() {
    println!("Testing slippage validation with invalid format...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test non-numeric string
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "invalid_slippage".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with invalid slippage format");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Invalid slippage format"), "Should contain format error message");
    assert!(error_msg.contains("invalid_slippage"), "Should contain the invalid input");
    
    println!("  ✓ Non-numeric slippage correctly rejected");

    // Test empty string
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with empty slippage");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Invalid slippage format"), "Should contain format error message");
    
    println!("  ✓ Empty slippage correctly rejected");

    // Test special characters
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "0.05%".to_string(), // percentage symbol not allowed
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with percentage symbol");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Invalid slippage format"), "Should contain format error message");
    
    println!("  ✓ Slippage with special characters correctly rejected");
}

#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_slippage_validation_negative_values() {
    println!("Testing slippage validation with negative values...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test negative decimal
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "-0.05".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with negative slippage");
    let error_msg = format!("{:?}", result.unwrap_err());
    // Negative values are caught as format errors by Decimal::from_str
    assert!(error_msg.contains("Invalid slippage format"), "Should contain format error message");
    assert!(error_msg.contains("-0.05"), "Should contain the negative value");
    
    println!("  ✓ Negative decimal slippage correctly rejected");

    // Test negative integer
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "-1".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with negative integer slippage");
    let error_msg = format!("{:?}", result.unwrap_err());
    // Negative values are caught as format errors by Decimal::from_str
    assert!(error_msg.contains("Invalid slippage format"), "Should contain format error message");
    
    println!("  ✓ Negative integer slippage correctly rejected");

    // Test very small negative value
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "-0.000001".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with very small negative slippage");
    let error_msg = format!("{:?}", result.unwrap_err());
    // Negative values are caught as format errors by Decimal::from_str
    assert!(error_msg.contains("Invalid slippage format"), "Should contain format error message");
    
    println!("  ✓ Very small negative slippage correctly rejected");
}

#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_slippage_validation_values_greater_than_one() {
    println!("Testing slippage validation with values greater than 1.0...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test exactly greater than 1.0
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "1.1".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with slippage > 1.0");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("cannot be greater than 1.0"), "Should contain greater than 1.0 error message");
    assert!(error_msg.contains("1.1"), "Should contain the invalid value");
    
    println!("  ✓ Slippage > 1.0 correctly rejected");

    // Test much larger value
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "5.0".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with much larger slippage");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("cannot be greater than 1.0"), "Should contain greater than 1.0 error message");
    
    println!("  ✓ Much larger slippage correctly rejected");

    // Test integer greater than 1
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "2".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with integer > 1");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("cannot be greater than 1.0"), "Should contain greater than 1.0 error message");
    
    println!("  ✓ Integer > 1 slippage correctly rejected");

    // Test percentage-like value (common user mistake)
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "5".to_string(), // User might think this is 5%
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with percentage-like value");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("cannot be greater than 1.0"), "Should contain greater than 1.0 error message");
    
    println!("  ✓ Percentage-like value correctly rejected");
}

#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_slippage_validation_valid_values() {
    println!("Testing slippage validation with valid values...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Note: These tests will fail at wallet validation since we don't have a configured wallet,
    // but the slippage parsing should succeed before that point
    let test_cases = vec![
        ("0.0", "zero slippage"),
        ("0.001", "very small slippage"),
        ("0.05", "5% slippage"),
        ("0.1", "10% slippage"),
        ("0.5", "50% slippage"),
        ("1.0", "100% slippage"),
        ("0.999999", "almost 100% slippage"),
    ];
    
    for (slippage_value, description) in test_cases {
        println!("  Testing {} ({})", description, slippage_value);
        
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "1000000".to_string(),
            slippage_value.to_string(),
            "test_pool".to_string(),
            None,
        ).await;
        
        // The function should fail later (at wallet validation), not at slippage parsing
        match result {
            Err(err) => {
                let error_msg = format!("{:?}", err);
                // Should NOT contain slippage-related errors
                assert!(!error_msg.contains("Invalid slippage"), 
                    "Should not fail on slippage parsing for valid value: {}", slippage_value);
                assert!(!error_msg.contains("cannot be greater than 1.0"), 
                    "Should not fail on >1.0 slippage for valid value: {}", slippage_value);
                
                // Should fail on wallet or other issues instead
                assert!(error_msg.contains("WalletNotConfigured") || 
                       error_msg.contains("Network") || 
                       error_msg.contains("Contract"),
                    "Should fail on wallet/network/contract issues, not slippage for: {}", slippage_value);
                
                println!("    ✓ {} parsed successfully (failed later as expected)", description);
            }
            Ok(_) => {
                // This would be unexpected in test environment, but slippage parsing worked
                println!("    ✓ {} parsed successfully (unexpectedly succeeded)", description);
            }
        }
    }
    
    println!("  ✓ All valid slippage values parsed correctly");
}

#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_slippage_validation_edge_cases() {
    println!("Testing slippage validation edge cases...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test exactly 1.0 (should be valid)
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "1.0".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    match result {
        Err(err) => {
            let error_msg = format!("{:?}", err);
            assert!(!error_msg.contains("Invalid slippage"), "1.0 should be valid slippage");
            assert!(!error_msg.contains("cannot be greater than 1.0"), "1.0 should not be > 1.0");
            println!("    ✓ Exactly 1.0 slippage accepted (failed later as expected)");
        }
        Ok(_) => {
            println!("    ✓ Exactly 1.0 slippage accepted (unexpectedly succeeded)");
        }
    }

    // Test exactly 0.0 (should be valid)
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "0.0".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    match result {
        Err(err) => {
            let error_msg = format!("{:?}", err);
            assert!(!error_msg.contains("Invalid slippage"), "0.0 should be valid slippage");
            println!("    ✓ Exactly 0.0 slippage accepted (failed later as expected)");
        }
        Ok(_) => {
            println!("    ✓ Exactly 0.0 slippage accepted (unexpectedly succeeded)");
        }
    }

    // Test very precise decimal
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "0.123456789".to_string(),
        "test_pool".to_string(),
        None,
    ).await;
    
    match result {
        Err(err) => {
            let error_msg = format!("{:?}", err);
            assert!(!error_msg.contains("Invalid slippage"), "Precise decimal should be valid slippage");
            println!("    ✓ Precise decimal slippage accepted (failed later as expected)");
        }
        Ok(_) => {
            println!("    ✓ Precise decimal slippage accepted (unexpectedly succeeded)");
        }
    }

    // Test scientific notation (should fail as invalid format)
    let result = adapter.execute_swap_simple(
        "uom".to_string(),
        "uusdc".to_string(),
        "1000000".to_string(),
        "1e-2".to_string(), // Scientific notation for 0.01
        "test_pool".to_string(),
        None,
    ).await;
    
    assert!(result.is_err(), "Should fail with scientific notation");
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("Invalid slippage format"), "Should reject scientific notation");
    
    println!("    ✓ Scientific notation correctly rejected");

    println!("  ✓ All edge cases handled correctly");
}

#[cfg(feature = "mcp")]
#[tokio::test]
async fn test_slippage_comprehensive_error_messages() {
    println!("Testing comprehensive slippage error messages...");
    
    let adapter = McpSdkAdapter::new(ConnectionPoolConfig::default());
    
    // Test that error messages are descriptive and helpful
    let test_cases = vec![
        ("abc", "non-numeric string"),
        ("-0.5", "negative value"),
        ("2.0", "value > 1.0"),
        ("", "empty string"),
        ("0.05%", "percentage symbol"),
    ];
    
    for (invalid_input, description) in test_cases {
        println!("  Testing error message for {} ({})", description, invalid_input);
        
        let result = adapter.execute_swap_simple(
            "uom".to_string(),
            "uusdc".to_string(),
            "1000000".to_string(),
            invalid_input.to_string(),
            "test_pool".to_string(),
            None,
        ).await;
        
        assert!(result.is_err(), "Should fail for invalid input: {}", invalid_input);
        let error_msg = format!("{:?}", result.unwrap_err());
        
        
        // Check that error message contains the invalid input for traceability
        if !invalid_input.is_empty() {
            // For decimal values like "2.0", Decimal may format them as "2" in error messages
            let input_variations = if invalid_input == "2.0" {
                vec!["2.0", "2"]
            } else {
                vec![invalid_input]
            };
            
            let contains_input = input_variations.iter().any(|input| error_msg.contains(input));
            assert!(contains_input, 
                "Error message should contain one of {:?} for debugging - actual: {}", input_variations, error_msg);
        }
        
        // Check that error message is descriptive
        assert!(error_msg.contains("Invalid") || error_msg.contains("cannot"), 
            "Error message should be descriptive for input: {}", invalid_input);
        
        println!("    ✓ Error message for {} is descriptive and contains input", description);
    }
    
    println!("  ✓ All error messages are comprehensive and helpful");
}
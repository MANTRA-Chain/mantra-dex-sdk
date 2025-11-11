//! PrimarySale tests entry point
//!
//! This file serves as the test entry point for PrimarySale integration tests.
//! The actual test modules are defined in tests/integration/primary_sale_test.rs

#[path = "integration/primary_sale_fixtures.rs"]
mod primary_sale_fixtures;

#[cfg(feature = "evm")]
#[cfg(feature = "mcp")]
#[path = "integration/primary_sale_test.rs"]
mod primary_sale_test;

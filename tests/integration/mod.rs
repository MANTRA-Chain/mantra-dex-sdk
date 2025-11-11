/// Integration test module for Phase 6
///
/// This module provides integration tests for all protocols and cross-protocol interactions

pub mod basic_validation;
pub mod claimdrop;
#[cfg(feature = "evm")]
pub mod evm;
#[cfg(feature = "evm")]
pub mod primary_sale_test;
#[cfg(feature = "evm")]
pub mod primary_sale_fixtures;
pub mod skip;
pub mod cross_protocol;
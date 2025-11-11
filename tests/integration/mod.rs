/// Integration test module for Phase 6
/// 
/// This module provides integration tests for all protocols and cross-protocol interactions

pub mod basic_validation;
pub mod claimdrop;
#[cfg(feature = "evm")]
pub mod evm;
pub mod skip;
pub mod cross_protocol;
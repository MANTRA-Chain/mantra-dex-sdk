/// Transaction decoder for EVM transactions
///
/// This module provides functionality to decode transaction input data by matching
/// 4-byte function selectors to known contract functions. It supports ERC-20,
/// PrimarySale, and other common contract types.
///
/// # Function Selector Matching
///
/// Function selectors are the first 4 bytes of the keccak256 hash of the function
/// signature. For example:
/// - `transfer(address,uint256)` -> `0xa9059cbb`
/// - `approve(address,uint256)` -> `0x095ea7b3`
///
/// # Example
///
/// ```rust,no_run
/// use mantra_sdk::protocols::evm::transaction_decoder::TransactionDecoder;
/// use alloy_primitives::hex;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let decoder = TransactionDecoder::new();
///
/// // Decode ERC-20 transfer
/// let input = hex::decode("a9059cbb000000000000000000000000742d35Cc6634C0532925a3b844Bc9e7595f0000000000000000000000000000000000000000000000000000000000000000064")?;
/// let decoded = decoder.decode(&input, None)?;
///
/// println!("Function: {}", decoded.function_name);
/// println!("Contract: {:?}", decoded.contract_type);
/// # Ok(())
/// # }
/// ```
use crate::error::Error;
use crate::protocols::evm::contracts::erc20::IERC20;
use crate::protocols::evm::contracts::primary_sale::IPrimarySale;
use alloy_primitives::{Address, FixedBytes};
use alloy_sol_types::SolCall;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of contract that the transaction interacts with
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractType {
    /// ERC-20 token contract
    ERC20,
    /// PrimarySale contract
    PrimarySale,
    /// ERC-721 NFT contract
    ERC721,
    /// Unknown or custom contract
    Unknown,
}

/// Decoded transaction call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedCall {
    /// Name of the function being called
    pub function_name: String,
    /// Type of contract
    pub contract_type: ContractType,
    /// Function selector (first 4 bytes)
    pub selector: String,
    /// Decoded parameters as JSON
    pub parameters: serde_json::Value,
    /// Raw input data
    #[serde(skip)]
    pub raw_input: Vec<u8>,
}

/// Transaction decoder with function selector registry
///
/// The decoder maintains a registry of known function selectors and their
/// corresponding decoding logic. It supports multiple contract types and
/// gracefully handles unknown functions.
pub struct TransactionDecoder {
    /// Map of function selector to decoder function
    decoders: HashMap<FixedBytes<4>, DecoderFn>,
}

/// Type alias for decoder functions
type DecoderFn = fn(&[u8]) -> Result<DecodedCall, Error>;

impl TransactionDecoder {
    /// Create a new transaction decoder with default registry
    ///
    /// Registers all known contract functions from:
    /// - ERC-20 (transfer, approve, transferFrom)
    /// - PrimarySale (invest, activate, settle, etc.)
    pub fn new() -> Self {
        let mut decoder = Self {
            decoders: HashMap::new(),
        };

        // Register ERC-20 decoders
        decoder.register_erc20_decoders();

        // Register PrimarySale decoders
        decoder.register_primary_sale_decoders();

        decoder
    }

    /// Register ERC-20 function decoders
    fn register_erc20_decoders(&mut self) {
        // transfer(address to, uint256 amount)
        self.register(
            FixedBytes::from(IERC20::transferCall::SELECTOR),
            Self::decode_erc20_transfer,
        );

        // approve(address spender, uint256 amount)
        self.register(
            FixedBytes::from(IERC20::approveCall::SELECTOR),
            Self::decode_erc20_approve,
        );

        // transferFrom(address from, address to, uint256 amount)
        self.register(
            FixedBytes::from(IERC20::transferFromCall::SELECTOR),
            Self::decode_erc20_transfer_from,
        );
    }

    /// Register PrimarySale function decoders
    fn register_primary_sale_decoders(&mut self) {
        // invest(address token, uint256 amount)
        self.register(
            FixedBytes::from(IPrimarySale::investCall::SELECTOR),
            Self::decode_primary_sale_invest,
        );

        // activate()
        self.register(
            FixedBytes::from(IPrimarySale::activateCall::SELECTOR),
            Self::decode_primary_sale_activate,
        );

        // endSale()
        self.register(
            FixedBytes::from(IPrimarySale::endSaleCall::SELECTOR),
            Self::decode_primary_sale_end_sale,
        );

        // initializeSettlement(address assetToken, address assetOwner)
        self.register(
            FixedBytes::from(IPrimarySale::initializeSettlementCall::SELECTOR),
            Self::decode_primary_sale_initialize_settlement,
        );

        // settleBatch(uint256 batchSize, address[] calldata restrictedWallets)
        self.register(
            FixedBytes::from(IPrimarySale::settleBatchCall::SELECTOR),
            Self::decode_primary_sale_settle_batch,
        );

        // finalizeSettlement()
        self.register(
            FixedBytes::from(IPrimarySale::finalizeSettlementCall::SELECTOR),
            Self::decode_primary_sale_finalize_settlement,
        );

        // claimRefund()
        self.register(
            FixedBytes::from(IPrimarySale::claimRefundCall::SELECTOR),
            Self::decode_primary_sale_claim_refund,
        );

        // cancel()
        self.register(
            FixedBytes::from(IPrimarySale::cancelCall::SELECTOR),
            Self::decode_primary_sale_cancel,
        );

        // pause()
        self.register(
            FixedBytes::from(IPrimarySale::pauseCall::SELECTOR),
            Self::decode_primary_sale_pause,
        );

        // unpause()
        self.register(
            FixedBytes::from(IPrimarySale::unpauseCall::SELECTOR),
            Self::decode_primary_sale_unpause,
        );

        // topUpRefunds(address token, uint256 amount)
        self.register(
            FixedBytes::from(IPrimarySale::topUpRefundsCall::SELECTOR),
            Self::decode_primary_sale_top_up_refunds,
        );

        // emergencyWithdrawERC20(address token, address recipient, uint256 amount)
        self.register(
            FixedBytes::from(IPrimarySale::emergencyWithdrawERC20Call::SELECTOR),
            Self::decode_primary_sale_emergency_withdraw,
        );
    }

    /// Register a decoder function for a specific selector
    fn register(&mut self, selector: FixedBytes<4>, decoder: DecoderFn) {
        self.decoders.insert(selector, decoder);
    }

    /// Decode transaction input data
    ///
    /// # Arguments
    /// * `input` - Transaction input data (including function selector)
    /// * `to_address` - Optional contract address for additional context
    ///
    /// # Returns
    /// * `Ok(DecodedCall)` - Successfully decoded transaction
    /// * `Err(Error)` - Input too short or decoding failed
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use mantra_sdk::protocols::evm::transaction_decoder::TransactionDecoder;
    /// # use alloy_primitives::hex;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let decoder = TransactionDecoder::new();
    /// let input = hex::decode("a9059cbb...")?;
    /// let decoded = decoder.decode(&input, None)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn decode(&self, input: &[u8], _to_address: Option<Address>) -> Result<DecodedCall, Error> {
        if input.len() < 4 {
            return Err(Error::Other(
                "Input data too short for function selector".to_string(),
            ));
        }

        // Extract function selector (first 4 bytes)
        let selector = FixedBytes::<4>::from_slice(&input[0..4]);

        // Look up decoder function
        if let Some(decoder_fn) = self.decoders.get(&selector) {
            decoder_fn(input)
        } else {
            // Unknown function selector
            Ok(DecodedCall {
                function_name: "unknown".to_string(),
                contract_type: ContractType::Unknown,
                selector: format!("0x{}", hex::encode(selector)),
                parameters: serde_json::json!({}),
                raw_input: input.to_vec(),
            })
        }
    }

    // ERC-20 Decoders
    // =========================================================================

    fn decode_erc20_transfer(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IERC20::transferCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode transfer: {}", e)))?;

        Ok(DecodedCall {
            function_name: "transfer".to_string(),
            contract_type: ContractType::ERC20,
            selector: format!("0x{}", hex::encode(IERC20::transferCall::SELECTOR)),
            parameters: serde_json::json!({
                "to": format!("{:?}", call.to),
                "amount": call.amount.to_string(),
            }),
            raw_input: input.to_vec(),
        })
    }

    fn decode_erc20_approve(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IERC20::approveCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode approve: {}", e)))?;

        Ok(DecodedCall {
            function_name: "approve".to_string(),
            contract_type: ContractType::ERC20,
            selector: format!("0x{}", hex::encode(IERC20::approveCall::SELECTOR)),
            parameters: serde_json::json!({
                "spender": format!("{:?}", call.spender),
                "amount": call.amount.to_string(),
            }),
            raw_input: input.to_vec(),
        })
    }

    fn decode_erc20_transfer_from(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IERC20::transferFromCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode transferFrom: {}", e)))?;

        Ok(DecodedCall {
            function_name: "transferFrom".to_string(),
            contract_type: ContractType::ERC20,
            selector: format!("0x{}", hex::encode(IERC20::transferFromCall::SELECTOR)),
            parameters: serde_json::json!({
                "from": format!("{:?}", call.from),
                "to": format!("{:?}", call.to),
                "amount": call.amount.to_string(),
            }),
            raw_input: input.to_vec(),
        })
    }

    // PrimarySale Decoders
    // =========================================================================

    fn decode_primary_sale_invest(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IPrimarySale::investCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode invest: {}", e)))?;

        Ok(DecodedCall {
            function_name: "invest".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::investCall::SELECTOR)),
            parameters: serde_json::json!({
                "token": format!("{:?}", call.token),
                "amount": call.amount.to_string(),
            }),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_activate(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::activateCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode activate: {}", e)))?;

        Ok(DecodedCall {
            function_name: "activate".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::activateCall::SELECTOR)),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_end_sale(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::endSaleCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode endSale: {}", e)))?;

        Ok(DecodedCall {
            function_name: "endSale".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::endSaleCall::SELECTOR)),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_initialize_settlement(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IPrimarySale::initializeSettlementCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode initializeSettlement: {}", e)))?;

        Ok(DecodedCall {
            function_name: "initializeSettlement".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!(
                "0x{}",
                hex::encode(IPrimarySale::initializeSettlementCall::SELECTOR)
            ),
            parameters: serde_json::json!({
                "assetToken": format!("{:?}", call.assetToken),
                "assetOwner": format!("{:?}", call.assetOwner),
            }),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_settle_batch(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IPrimarySale::settleBatchCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode settleBatch: {}", e)))?;

        Ok(DecodedCall {
            function_name: "settleBatch".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::settleBatchCall::SELECTOR)),
            parameters: serde_json::json!({
                "batchSize": call.batchSize.to_string(),
                "restrictedWallets": call.restrictedWallets.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
            }),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_finalize_settlement(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::finalizeSettlementCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode finalizeSettlement: {}", e)))?;

        Ok(DecodedCall {
            function_name: "finalizeSettlement".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!(
                "0x{}",
                hex::encode(IPrimarySale::finalizeSettlementCall::SELECTOR)
            ),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_claim_refund(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::claimRefundCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode claimRefund: {}", e)))?;

        Ok(DecodedCall {
            function_name: "claimRefund".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::claimRefundCall::SELECTOR)),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_cancel(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::cancelCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode cancel: {}", e)))?;

        Ok(DecodedCall {
            function_name: "cancel".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::cancelCall::SELECTOR)),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_pause(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::pauseCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode pause: {}", e)))?;

        Ok(DecodedCall {
            function_name: "pause".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::pauseCall::SELECTOR)),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_unpause(input: &[u8]) -> Result<DecodedCall, Error> {
        let _call = IPrimarySale::unpauseCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode unpause: {}", e)))?;

        Ok(DecodedCall {
            function_name: "unpause".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!("0x{}", hex::encode(IPrimarySale::unpauseCall::SELECTOR)),
            parameters: serde_json::json!({}),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_top_up_refunds(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IPrimarySale::topUpRefundsCall::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode topUpRefunds: {}", e)))?;

        Ok(DecodedCall {
            function_name: "topUpRefunds".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!(
                "0x{}",
                hex::encode(IPrimarySale::topUpRefundsCall::SELECTOR)
            ),
            parameters: serde_json::json!({
                "token": format!("{:?}", call.token),
                "amount": call.amount.to_string(),
            }),
            raw_input: input.to_vec(),
        })
    }

    fn decode_primary_sale_emergency_withdraw(input: &[u8]) -> Result<DecodedCall, Error> {
        let call = IPrimarySale::emergencyWithdrawERC20Call::abi_decode(input, true)
            .map_err(|e| Error::Other(format!("Failed to decode emergencyWithdrawERC20: {}", e)))?;

        Ok(DecodedCall {
            function_name: "emergencyWithdrawERC20".to_string(),
            contract_type: ContractType::PrimarySale,
            selector: format!(
                "0x{}",
                hex::encode(IPrimarySale::emergencyWithdrawERC20Call::SELECTOR)
            ),
            parameters: serde_json::json!({
                "token": format!("{:?}", call.token),
                "recipient": format!("{:?}", call.recipient),
                "amount": call.amount.to_string(),
            }),
            raw_input: input.to_vec(),
        })
    }
}

impl Default for TransactionDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{address, U256};

    #[test]
    fn test_decode_erc20_transfer() {
        let decoder = TransactionDecoder::new();

        // transfer(0x1111111111111111111111111111111111111111, 100)
        let input = hex::decode("a9059cbb00000000000000000000000011111111111111111111111111111111111111110000000000000000000000000000000000000000000000000000000000000064").unwrap();

        let decoded = decoder.decode(&input, None).unwrap();

        assert_eq!(decoded.function_name, "transfer");
        assert_eq!(decoded.contract_type, ContractType::ERC20);
        assert!(decoded.parameters.get("to").is_some());
        assert!(decoded.parameters.get("amount").is_some());
    }

    #[test]
    fn test_decode_erc20_approve() {
        let decoder = TransactionDecoder::new();

        // approve(0x2222222222222222222222222222222222222222, 1000)
        let spender = address!("2222222222222222222222222222222222222222");
        let amount = U256::from(1000u64);

        let call = IERC20::approveCall { spender, amount };
        let input = call.abi_encode();

        let decoded = decoder.decode(&input, None).unwrap();

        assert_eq!(decoded.function_name, "approve");
        assert_eq!(decoded.contract_type, ContractType::ERC20);
        assert!(decoded.parameters.get("spender").is_some());
        assert!(decoded.parameters.get("amount").is_some());
    }

    #[test]
    fn test_decode_unknown_function() {
        let decoder = TransactionDecoder::new();

        // Unknown function selector
        let input = hex::decode("deadbeef00000000000000000000000000000000").unwrap();

        let decoded = decoder.decode(&input, None).unwrap();

        assert_eq!(decoded.function_name, "unknown");
        assert_eq!(decoded.contract_type, ContractType::Unknown);
        assert_eq!(decoded.selector, "0xdeadbeef");
    }

    #[test]
    fn test_decode_too_short() {
        let decoder = TransactionDecoder::new();

        // Input too short
        let input = vec![0x01, 0x02];

        let result = decoder.decode(&input, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_primary_sale_invest() {
        let decoder = TransactionDecoder::new();

        // invest(0x3333333333333333333333333333333333333333, 5000000000000000000000)
        let token = address!("3333333333333333333333333333333333333333");
        let amount = U256::from(5000000000000000000000u128);

        let call = IPrimarySale::investCall { token, amount };
        let input = call.abi_encode();

        let decoded = decoder.decode(&input, None).unwrap();

        assert_eq!(decoded.function_name, "invest");
        assert_eq!(decoded.contract_type, ContractType::PrimarySale);
        assert!(decoded.parameters.get("token").is_some());
        assert!(decoded.parameters.get("amount").is_some());
    }
}

/// PrimarySale contract helpers
///
/// Provides high-level methods for interacting with the MANTRA PrimarySale contract.
///
/// # Example
///
/// ```rust,no_run
/// use mantra_sdk::protocols::evm::client::EvmClient;
/// use alloy_primitives::{address, Address, U256};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create an EVM client
/// let evm_client = EvmClient::new("https://evm.dukong.mantrachain.io", 5887).await?;
///
/// // Create a PrimarySale helper for a specific contract
/// let primary_sale_address = address!("0x0000000000000000000000000000000000000000");
/// let primary_sale = evm_client.primary_sale(primary_sale_address);
///
/// // Query sale information
/// let sale_info = primary_sale.get_sale_info().await?;
/// println!("Sale Status: {}", sale_info.status);
/// println!("Total Contributed: {}", sale_info.total_contributed);
/// println!("Investor Count: {}", sale_info.investor_count);
///
/// // Query investor allocation
/// let investor = address!("0x1234567890123456789012345678901234567890");
/// let tokens = primary_sale.tokens_for(investor).await?;
/// println!("Tokens allocated: {}", tokens);
///
/// // Check contribution
/// let contributed = primary_sale.contributed(investor).await?;
/// println!("Amount contributed: {}", contributed);
///
/// # Ok(())
/// # }
/// ```
use crate::error::Error;
use crate::protocols::evm::client::EvmClient;
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;

/// Maximum number of investors that can be processed in a single settlement transaction.
///
/// This limit prevents gas exhaustion during the distribution loop in `settleAndDistribute()`.
/// The contract enforces this limit to ensure the transaction can complete within the block gas limit.
///
/// **Important:**
/// - Settlement distributes RWA tokens to all investors in one atomic transaction
/// - Processing >500 investors could exceed the block gas limit (~30M on Ethereum)
/// - For sales with more investors, call `settleAndDistribute()` multiple times
/// - Each call processes the next batch of uninvested investors
pub const MAX_SETTLEMENT_INVESTORS: u64 = 500;

/// Recommended batch size for settlement to leave gas buffer.
///
/// While the contract allows up to 500 investors per transaction, using 250 provides:
/// - Safety margin for network congestion
/// - Buffer for complex token transfer logic
/// - Faster transaction confirmation
pub const RECOMMENDED_SETTLEMENT_BATCH: u64 = 250;

sol! {
    #[derive(Debug)]
    interface IPrimarySale {
        // Enums
        enum Status {
            Pending,
            Active,
            Ended,
            Failed,
            Settled,
            Cancelled
        }

        /// Reasons an investor might be refunded during settlement
        enum RefundReason {
            KYC_DELISTED,      // Investor's KYC approval was revoked; refund goes to investor
            RESTRICTED_WALLET  // Investor flagged as restricted/sanctioned; refund goes to MULTISIG
        }

        // Events
        /// Invested event now includes token parameter for multi-token support
        event Invested(
            address indexed investor,
            address indexed token,
            uint256 amount
        );
        event SaleEnded(uint256 indexed total, bool indexed softCapMet);
        /// SettlementComplete signature changed in v2.0
        event SettlementComplete(
            uint256 indexed total,
            uint256 indexed commission,
            uint256 indexed investors,
            address assetToken
        );
        /// RefundsFunded now includes token parameter
        event RefundsFunded(address indexed token, uint256 indexed amount);
        event RefundClaimed(address indexed investor, uint256 indexed amount);
        event SaleActivated();
        event SaleCancelled();

        // New events in v2.0
        event ContractPaused(address indexed actor, uint256 indexed timestamp);
        event ContractUnpaused(address indexed actor, uint256 indexed timestamp);
        event HardCapReached(uint256 totalRaised, address finalInvestor);
        event EmergencyWithdrawal(
            address indexed token,
            address indexed recipient,
            uint256 amount
        );
        event BatchSettled(
            uint256 indexed batchNumber,
            uint256 indexed processedCount,
            uint256 indexed totalInvestors,
            address assetToken
        );
        event InvestorRefundedDuringSettlement(
            address indexed investor,
            uint256 normalizedAmount,
            uint256 tokenAmount,
            RefundReason indexed reason
        );
        event InvestorDistributed(
            address indexed investor,
            uint256 indexed tokenAmount,
            uint256 indexed contributionNormalized
        );
        event InvestorAlreadySettled(
            address indexed investor,
            uint256 indexed position
        );
        event InvalidRestrictedAddress(
            address indexed invalidAddress,
            uint256 indexed arrayIndex
        );
        event SettlementInitiated(
            address indexed assetToken,
            address indexed assetOwner,
            uint256 indexed totalInvestors,
            uint256 totalTokensRequired
        );
        event SettlementFinalized(
            uint256 indexed totalDistributed,
            uint256 indexed totalRefunded,
            uint256 indexed investorsSettled,
            address assetToken
        );

        // View functions
        function status() external view returns (Status);
        function investorCount() external view returns (uint256);
        function tokensFor(address investor) external view returns (uint256);
        function getInvestor(uint256 index) external view returns (address);
        function isSaleActive() external view returns (bool);
        function getRemainingSaleTime() external view returns (uint64);
        function getTotalTokens() external view returns (uint256);
        function getCommissionAmount() external view returns (uint256);
        function refunded(address investor) external view returns (bool);

        // New view functions for multi-token support (v2.0)
        function getRemainingCapacity() external view returns (uint256);
        function getAcceptedTokens() external view returns (address[] memory);
        function getContributionByToken(address investor, address token) external view returns (uint256);
        function getTotalContributedByToken(address token) external view returns (uint256);
        function getContributedNormalized(address investor) external view returns (uint256);
        function getTotalContributedNormalized() external view returns (uint256);
        function getTokenDecimals(address token) external view returns (uint8);
        function getInvestors() external view returns (address[] memory);
        function getInvestorDistribution(address investor) external view returns (
            uint256 contribution,
            uint256 tokensToReceive,
            bool isKYCApproved,
            bool hasSettled
        );
        function getInvestorDistributionBatch(uint256 startIndex, uint256 endIndex) external view returns (
            address[] memory addresses,
            uint256[] memory contributions,
            uint256[] memory tokens,
            bool[] memory kycStatuses,
            bool[] memory settlementStatuses
        );
        function getSettlementProgress() external view returns (
            uint256 processedInvestors,
            uint256 totalInvestors,
            bool isInitialized,
            bool isComplete
        );

        // Multi-token state mappings (v2.0)
        function contributedByToken(address investor, address token) external view returns (uint256);
        function contributedNormalized(address investor) external view returns (uint256);
        function totalContributedNormalized() external view returns (uint256);
        function totalContributedByToken(address token) external view returns (uint256);
        function refundsPoolByToken(address token) external view returns (uint256);
        function hasReceivedSettlement(address investor) external view returns (bool);
        function isAcceptedToken(address token) external view returns (bool);
        function tokenDecimals(address token) external view returns (uint8);
        function acceptedTokens(uint256 index) external view returns (address);

        // Immutable configuration
        function NAME() external view returns (string memory);
        function HARD_CAP() external view returns (uint256);
        function ALLOWLIST() external view returns (address);
        function MULTISIG() external view returns (address);
        function ISSUER() external view returns (address);
        function MANTRA() external view returns (address);
        function START() external view returns (uint64);
        function END() external view returns (uint64);
        function SOFT_CAP() external view returns (uint256);
        function COMMISSION_BPS() external view returns (uint16);
        function MIN_STEP() external view returns (uint256);

        // State-changing functions
        function activate() external;
        /// invest now requires token parameter (v2.0)
        function invest(address token, uint256 amount) external;
        function endSale() external;

        // New 3-step settlement process (v2.0)
        function initializeSettlement(address assetToken, address assetOwner) external;
        function settleBatch(uint256 batchSize, address[] calldata restrictedWallets) external;
        function finalizeSettlement() external;

        /// topUpRefunds now requires token parameter (v2.0)
        function topUpRefunds(address token, uint256 amount) external;
        function claimRefund() external;
        function cancel() external;
        function pause() external;
        function unpause() external;
        function emergencyWithdrawERC20(address token, address recipient, uint256 amount) external;
    }
}

/// PrimarySale contract helper
pub struct PrimarySale {
    client: EvmClient,
    address: Address,
}

impl PrimarySale {
    /// Create a new PrimarySale helper for the given contract address
    pub fn new(client: EvmClient, address: Address) -> Self {
        Self { client, address }
    }

    /// Get contract address
    pub fn address(&self) -> Address {
        self.address
    }

    // ========== View Functions ==========

    /// Get current sale status
    pub async fn status(&self) -> Result<u8, Error> {
        let call = IPrimarySale::statusCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0 as u8)
    }

    /// Get total number of investors
    pub async fn investor_count(&self) -> Result<U256, Error> {
        let call = IPrimarySale::investorCountCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Calculate tokens for a specific investor
    pub async fn tokens_for(&self, investor: Address) -> Result<U256, Error> {
        let call = IPrimarySale::tokensForCall { investor };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get investor address by index
    pub async fn get_investor(&self, index: U256) -> Result<Address, Error> {
        let call = IPrimarySale::getInvestorCall { index };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Check if sale is currently active
    pub async fn is_sale_active(&self) -> Result<bool, Error> {
        let call = IPrimarySale::isSaleActiveCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get remaining time in the sale window
    pub async fn get_remaining_sale_time(&self) -> Result<u64, Error> {
        let call = IPrimarySale::getRemainingSaleTimeCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get total tokens to be distributed
    pub async fn get_total_tokens(&self) -> Result<U256, Error> {
        let call = IPrimarySale::getTotalTokensCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get commission amount
    pub async fn get_commission_amount(&self) -> Result<U256, Error> {
        let call = IPrimarySale::getCommissionAmountCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get contribution amount for an investor in a specific token (raw decimals)
    ///
    /// # Multi-Token Support (v2.0)
    /// Returns the raw amount in the token's native decimals (e.g., 1000e6 for USDC).
    /// For normalized amounts (18 decimals), use `contributed_normalized()`.
    ///
    /// # Arguments
    /// * `investor` - Investor address
    /// * `token` - Payment token address
    ///
    /// # Returns
    /// Raw token amount in token's native decimals (0-18)
    pub async fn contributed_by_token(
        &self,
        investor: Address,
        token: Address,
    ) -> Result<U256, Error> {
        let call = IPrimarySale::contributedByTokenCall { investor, token };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get total contributed amount for a specific token (raw decimals)
    ///
    /// # Multi-Token Support (v2.0)
    /// Returns the total raw amount across all investors for this token.
    /// For normalized totals (18 decimals), use `total_contributed_normalized()`.
    ///
    /// # Arguments
    /// * `token` - Payment token address
    ///
    /// # Returns
    /// Total raw amount in token's native decimals
    pub async fn total_contributed_by_token(&self, token: Address) -> Result<U256, Error> {
        let call = IPrimarySale::getTotalContributedByTokenCall { token };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get normalized contribution for an investor (always 18 decimals)
    ///
    /// # Multi-Token Support (v2.0)
    /// Returns the total contribution normalized to 18 decimals for fair comparison.
    /// This is the sum across all tokens the investor contributed.
    ///
    /// # Example
    /// - Investor contributed 1000 USDC (6 decimals) + 500 DAI (18 decimals)
    /// - Returns: 1500e18 (1000e18 + 500e18 normalized)
    ///
    /// # Arguments
    /// * `investor` - Investor address
    ///
    /// # Returns
    /// Normalized contribution amount (18 decimals)
    pub async fn contributed_normalized(&self, investor: Address) -> Result<U256, Error> {
        let call = IPrimarySale::getContributedNormalizedCall { investor };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get total normalized contributions across all investors (always 18 decimals)
    ///
    /// # Multi-Token Support (v2.0)
    /// Returns the total normalized amount across all tokens and all investors.
    /// Used for soft cap validation and hard cap enforcement.
    ///
    /// # Returns
    /// Total normalized contribution amount (18 decimals)
    pub async fn total_contributed_normalized(&self) -> Result<U256, Error> {
        let call = IPrimarySale::getTotalContributedNormalizedCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get refund pool amount for a specific token
    ///
    /// # Multi-Token Support (v2.0)
    /// Each accepted token has its own refund pool.
    ///
    /// # Arguments
    /// * `token` - Payment token address
    ///
    /// # Returns
    /// Refund pool balance in token's native decimals
    pub async fn refunds_pool_by_token(&self, token: Address) -> Result<U256, Error> {
        let call = IPrimarySale::refundsPoolByTokenCall { token };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get list of accepted payment tokens
    ///
    /// # Multi-Token Support (v2.0)
    /// Returns all tokens that can be used for investment.
    /// Maximum 10 tokens (MAX_ACCEPTED_TOKENS).
    ///
    /// # Returns
    /// Array of accepted token addresses
    pub async fn get_accepted_tokens(&self) -> Result<Vec<Address>, Error> {
        let call = IPrimarySale::getAcceptedTokensCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get decimals for a specific token
    ///
    /// # Multi-Token Support (v2.0)
    /// Returns the number of decimals for the given token (0-18).
    ///
    /// # Arguments
    /// * `token` - Token address
    ///
    /// # Returns
    /// Number of decimals (0-18)
    pub async fn get_token_decimals(&self, token: Address) -> Result<u8, Error> {
        let call = IPrimarySale::getTokenDecimalsCall { token };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Check if a token is accepted for investment
    ///
    /// # Multi-Token Support (v2.0)
    /// O(1) lookup to validate if a token is in the accepted list.
    ///
    /// # Arguments
    /// * `token` - Token address to check
    ///
    /// # Returns
    /// true if token is accepted, false otherwise
    pub async fn is_accepted_token(&self, token: Address) -> Result<bool, Error> {
        let call = IPrimarySale::isAcceptedTokenCall { token };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Check if investor has claimed refund
    pub async fn refunded(&self, investor: Address) -> Result<bool, Error> {
        let call = IPrimarySale::refundedCall { investor };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    // ========== Immutable Configuration ==========

    /// Get human-readable sale name
    ///
    /// # New in v2.0
    /// Returns the NAME immutable field (1-100 bytes).
    ///
    /// # Returns
    /// Sale name string
    pub async fn name(&self) -> Result<String, Error> {
        let call = IPrimarySale::NAMECall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get hard cap (maximum funding limit)
    ///
    /// # New in v2.0
    /// Returns the HARD_CAP immutable field in normalized 18 decimals.
    /// A value of 0 means unlimited (no hard cap).
    ///
    /// # Returns
    /// Hard cap amount (18 decimals), or 0 for unlimited
    pub async fn hard_cap(&self) -> Result<U256, Error> {
        let call = IPrimarySale::HARD_CAPCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get remaining investment capacity
    ///
    /// # New in v2.0
    /// Calculates remaining investable amount before hitting hard cap.
    /// Returns type(uint256).max if no hard cap (unlimited).
    ///
    /// # Returns
    /// Remaining capacity (18 decimals) or max uint256 if unlimited
    pub async fn get_remaining_capacity(&self) -> Result<U256, Error> {
        let call = IPrimarySale::getRemainingCapacityCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get allowlist contract address
    pub async fn allowlist(&self) -> Result<Address, Error> {
        let call = IPrimarySale::ALLOWLISTCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get multisig wallet address
    pub async fn multisig(&self) -> Result<Address, Error> {
        let call = IPrimarySale::MULTISIGCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get issuer address
    pub async fn issuer(&self) -> Result<Address, Error> {
        let call = IPrimarySale::ISSUERCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get MANTRA address for commission
    pub async fn mantra(&self) -> Result<Address, Error> {
        let call = IPrimarySale::MANTRACall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get sale start timestamp
    pub async fn start(&self) -> Result<u64, Error> {
        let call = IPrimarySale::STARTCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get sale end timestamp
    pub async fn end(&self) -> Result<u64, Error> {
        let call = IPrimarySale::ENDCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get soft cap (minimum funding threshold)
    pub async fn soft_cap(&self) -> Result<U256, Error> {
        let call = IPrimarySale::SOFT_CAPCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get commission in basis points
    pub async fn commission_bps(&self) -> Result<u16, Error> {
        let call = IPrimarySale::COMMISSION_BPSCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get minimum investment step size
    pub async fn min_step(&self) -> Result<U256, Error> {
        let call = IPrimarySale::MIN_STEPCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    // ========== Access Control Functions ==========

    /// Check if an address has admin role
    ///
    /// # Changed in v2.0
    /// Now checks ADMIN_ROLE = keccak256("ADMIN_ROLE") as primary role.
    /// Also checks DEFAULT_ADMIN_ROLE (bytes32(0)) for backward compatibility.
    ///
    /// This role can:
    /// - activate, cancel, pause, unpause sale
    /// - perform emergency operations
    /// - execute all settlement operations (initializeSettlement, settleBatch, finalizeSettlement)
    ///
    /// # Breaking Change
    /// SETTLEMENT_ROLE has been removed in v2.0. All settlement operations now require ADMIN_ROLE.
    pub async fn has_admin_role(&self, account: Address) -> Result<bool, Error> {
        // ADMIN_ROLE = keccak256("ADMIN_ROLE")
        let admin_role = alloy_primitives::keccak256(b"ADMIN_ROLE");

        sol! {
            function hasRole(bytes32 role, address account) external view returns (bool);
        }

        // Check ADMIN_ROLE first (v2.0 standard)
        let has_role_call = hasRoleCall { role: admin_role, account };
        let has_admin = self.client.call_contract(self.address, has_role_call).await?;

        if has_admin._0 {
            return Ok(true);
        }

        // Fallback to DEFAULT_ADMIN_ROLE for backward compatibility
        let default_admin_role = alloy_primitives::B256::ZERO;
        let has_role_call = hasRoleCall { role: default_admin_role, account };
        let result = self.client.call_contract(self.address, has_role_call).await?;
        Ok(result._0)
    }

    /// Get the ADMIN_ROLE constant
    ///
    /// # New in v2.0
    /// Returns the ADMIN_ROLE constant = keccak256("ADMIN_ROLE").
    ///
    /// # Returns
    /// ADMIN_ROLE bytes32 constant
    pub fn get_admin_role() -> alloy_primitives::B256 {
        alloy_primitives::keccak256(b"ADMIN_ROLE")
    }

    // ========== Batch Query Methods (v2.0) ==========

    /// Get distribution details for a single investor
    ///
    /// # New in v2.0
    /// Returns comprehensive distribution information including KYC status and settlement status.
    ///
    /// # Arguments
    /// * `investor` - Investor address
    ///
    /// # Returns
    /// Tuple of (contribution, tokensToReceive, isKYCApproved, hasSettled)
    pub async fn get_investor_distribution(
        &self,
        investor: Address,
    ) -> Result<(U256, U256, bool, bool), Error> {
        let call = IPrimarySale::getInvestorDistributionCall { investor };
        let result = self.client.call_contract(self.address, call).await?;
        Ok((result.contribution, result.tokensToReceive, result.isKYCApproved, result.hasSettled))
    }

    /// Get distribution details for multiple investors (batch query)
    ///
    /// # New in v2.0
    /// Efficient batch query to reduce RPC calls. Recommended batch size: 100-250.
    ///
    /// # Arguments
    /// * `start_index` - Start index in investors array (inclusive)
    /// * `end_index` - End index in investors array (exclusive)
    ///
    /// # Returns
    /// Tuple of (addresses, contributions, tokens, kycStatuses, settlementStatuses)
    ///
    /// # Gas Considerations
    /// Large batches (500+) may hit gas limits on some RPC nodes.
    /// Recommended to use batches of 100-250 for best performance.
    pub async fn get_investor_distribution_batch(
        &self,
        start_index: U256,
        end_index: U256,
    ) -> Result<(Vec<Address>, Vec<U256>, Vec<U256>, Vec<bool>, Vec<bool>), Error> {
        let call = IPrimarySale::getInvestorDistributionBatchCall {
            startIndex: start_index,
            endIndex: end_index,
        };
        let result = self.client.call_contract(self.address, call).await?;
        Ok((
            result.addresses,
            result.contributions,
            result.tokens,
            result.kycStatuses,
            result.settlementStatuses,
        ))
    }

    /// Get complete list of all investors
    ///
    /// # New in v2.0
    /// Returns the full investors array. For large sales, consider using pagination
    /// via `get_investors()` or batch queries via `get_investor_distribution_batch()`.
    ///
    /// # Returns
    /// Array of all investor addresses
    pub async fn get_investors_list(&self) -> Result<Vec<Address>, Error> {
        let call = IPrimarySale::getInvestorsCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get settlement progress
    ///
    /// # New in v2.0
    /// Returns the current state of the batch settlement process.
    ///
    /// # Returns
    /// Tuple of (processedInvestors, totalInvestors, isInitialized, isComplete)
    ///
    /// # Usage
    /// Check this between `settleBatch()` calls to track progress:
    /// - processedInvestors: Number processed so far (includes skipped)
    /// - totalInvestors: Total number of investors
    /// - isInitialized: Has `initializeSettlement()` been called
    /// - isComplete: All investors processed, ready for `finalizeSettlement()`
    pub async fn get_settlement_progress(&self) -> Result<(U256, U256, bool, bool), Error> {
        let call = IPrimarySale::getSettlementProgressCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok((
            result.processedInvestors,
            result.totalInvestors,
            result.isInitialized,
            result.isComplete,
        ))
    }

    /// Check if investor has received settlement
    ///
    /// # New in v2.0
    /// Used for idempotency checking during batch settlement.
    ///
    /// # Arguments
    /// * `investor` - Investor address
    ///
    /// # Returns
    /// true if investor has been settled, false otherwise
    pub async fn has_received_settlement(&self, investor: Address) -> Result<bool, Error> {
        let call = IPrimarySale::hasReceivedSettlementCall { investor };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    // ========== State-Changing Functions ==========
    // Note: These methods will return NotImplemented errors until transaction
    // sending is fully implemented in the EvmClient

    /// Activate the sale (admin only)
    pub async fn activate(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::activateCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Invest payment tokens in the sale
    ///
    /// # Changed in v2.0 - Breaking Change
    /// Now requires `token` parameter to specify which accepted token to invest with.
    ///
    /// # Multi-Token Support
    /// - Token must be in the accepted tokens list (check via `get_accepted_tokens()`)
    /// - Amount must be in token's native decimals (e.g., 1000e6 for USDC)
    /// - Contract normalizes to 18 decimals internally for fair comparison
    /// - Amount must be multiple of MIN_STEP (after normalization)
    ///
    /// # Arguments
    /// * `token` - Payment token address (must be accepted)
    /// * `amount` - Amount to invest in token's native decimals
    /// * `wallet` - Wallet to sign transaction
    ///
    /// # Gas Buffer
    /// Uses 20% gas buffer (simple operation)
    ///
    /// # Requirements
    /// - Sale must be Active
    /// - Current time must be within sale window
    /// - Investor must be allowlisted (KYC/AML approved)
    /// - Hard cap not exceeded (if set)
    /// - Investor must approve token allowance first
    pub async fn invest(
        &self,
        token: Address,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::investCall { token, amount };
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(20))
            .await
    }

    /// End the sale (callable by anyone after end time)
    pub async fn end_sale(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::endSaleCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    // ========== 3-Step Settlement Process (v2.0) ==========

    /// Initialize settlement (Step 1 of 3)
    ///
    /// # New in v2.0
    /// Replaces the old `settleAndDistribute()` with a 3-step batch process.
    ///
    /// # What This Does
    /// - Validates all preconditions (balances, allowances)
    /// - Pulls ALL asset tokens from asset owner upfront (atomic guarantee)
    /// - Initializes settlement state for batch processing
    ///
    /// # Arguments
    /// * `asset_token` - ERC-20 asset token to distribute to investors
    /// * `asset_owner` - Address owning the asset tokens
    /// * `wallet` - Admin wallet to sign transaction
    ///
    /// # Gas Buffer
    /// Uses 30% gas buffer (complex operation with validation and token pull)
    ///
    /// # Requirements
    /// - Admin role required
    /// - Sale status must be Ended (not Failed)
    /// - Soft cap must be met
    /// - At least one investor
    /// - Asset token decimals 0-18
    /// - Asset owner has sufficient tokens and allowance
    /// - Multisig has sufficient balances and allowances for all payment tokens
    /// - Can only be called once (idempotent)
    ///
    /// # After This
    /// Call `settleBatch()` repeatedly until all investors processed.
    pub async fn initialize_settlement(
        &self,
        asset_token: Address,
        asset_owner: Address,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::initializeSettlementCall {
            assetToken: asset_token,
            assetOwner: asset_owner,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(30))
            .await
    }

    /// Settle a batch of investors (Step 2 of 3)
    ///
    /// # New in v2.0
    /// Processes investors in batches of 1-100. Can be called multiple times.
    ///
    /// # What This Does
    /// For each investor in batch:
    /// - Check if restricted (sanctioned) → refund to MULTISIG, return tokens to ISSUER
    /// - Check if KYC de-listed → refund to investor, return tokens to ISSUER
    /// - Otherwise → distribute asset tokens to investor
    /// - Mark as settled (idempotent - safe to retry)
    ///
    /// # Arguments
    /// * `batch_size` - Number of investors to process (1-100 recommended: 100-250)
    /// * `restricted_wallets` - Optional array of sanctioned addresses (max 50)
    /// * `wallet` - Admin wallet to sign transaction
    ///
    /// # Gas Buffer
    /// Uses 30% gas buffer (very complex operation with multiple transfers)
    ///
    /// # Gas Estimation
    /// - 100 investors, all distributed: ~3-4M gas
    /// - 100 investors, 10 refunded: ~4-5M gas (refunds cost more)
    ///
    /// # Idempotency
    /// Safe to call multiple times. Skips already-settled investors and emits
    /// `InvestorAlreadySettled` event for tracking.
    ///
    /// # Requirements
    /// - Admin role required
    /// - `initializeSettlement()` must be called first
    /// - Not already complete
    /// - batch_size 1-100
    /// - restricted_wallets length ≤ 50
    ///
    /// # Progress Tracking
    /// Use `get_settlement_progress()` to check:
    /// - How many investors processed
    /// - Whether all are complete (ready for finalization)
    ///
    /// # After This
    /// When all investors processed, call `finalizeSettlement()`.
    pub async fn settle_batch(
        &self,
        batch_size: U256,
        restricted_wallets: Vec<Address>,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        // Validate parameters before transaction submission
        validate_settlement_batch_params(batch_size, &restricted_wallets)?;

        let call = IPrimarySale::settleBatchCall {
            batchSize: batch_size,
            restrictedWallets: restricted_wallets,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(30))
            .await
    }

    /// Finalize settlement (Step 3 of 3)
    ///
    /// # New in v2.0
    /// Completes the settlement process after all investors processed.
    ///
    /// # What This Does
    /// For each accepted payment token:
    /// - Calculate net proceeds (total - refunds)
    /// - Transfer commission to MANTRA
    /// - Transfer remaining proceeds to ISSUER
    /// - Return any remaining asset tokens to ISSUER (from refunds)
    /// - Mark settlement as complete
    /// - Change sale status to Settled
    ///
    /// # Arguments
    /// * `wallet` - Admin wallet to sign transaction
    ///
    /// # Gas Buffer
    /// Uses 30% gas buffer (multiple token transfers across payment tokens)
    ///
    /// # Requirements
    /// - Admin role required
    /// - `initializeSettlement()` called
    /// - All investors processed via `settleBatch()`
    /// - Not already finalized (idempotent)
    ///
    /// # After This
    /// Settlement is complete. Sale status = Settled.
    pub async fn finalize_settlement(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::finalizeSettlementCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(30))
            .await
    }

    /// Check if settlement can be finalized
    ///
    /// # Helper Method
    /// Convenience method to check if all preconditions for finalization are met.
    ///
    /// # Returns
    /// true if ready to finalize, false otherwise
    pub async fn can_finalize_settlement(&self) -> Result<bool, Error> {
        let (processed, total, is_initialized, is_complete) =
            self.get_settlement_progress().await?;

        Ok(is_initialized && !is_complete && processed == total)
    }

    /// Top up refunds pool for a specific token
    ///
    /// # Changed in v2.0 - Breaking Change
    /// Now requires `token` parameter to specify which token's refund pool to fund.
    ///
    /// # Multi-Token Support
    /// Each accepted token has its own refund pool. Must specify which token to fund.
    ///
    /// # Permissionless
    /// Anyone can call this to help fund refunds (no access control).
    /// Typical callers: MULTISIG or ADMIN returning investor funds after sale failure.
    ///
    /// # Arguments
    /// * `token` - Payment token address (must be accepted)
    /// * `amount` - Amount to add to refund pool (token's native decimals)
    /// * `wallet` - Wallet to sign transaction
    ///
    /// # Gas Buffer
    /// Uses 20% gas buffer (simple operation)
    ///
    /// # Requirements
    /// - Sale status must be Failed or Cancelled
    /// - Token must be in accepted tokens list
    /// - Caller must approve token allowance first
    pub async fn top_up_refunds(
        &self,
        token: Address,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::topUpRefundsCall { token, amount };
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(20))
            .await
    }

    /// Claim refund for failed or cancelled sale
    ///
    /// # Changed in v2.0 - Behavior Change
    /// Now automatically claims refunds for ALL accepted tokens the investor contributed.
    /// Contract loops through all accepted tokens and refunds each one.
    ///
    /// # Multi-Token Support
    /// - Refunds all tokens in one transaction
    /// - Contract verifies sufficient refund pool for each token
    /// - Emits single RefundClaimed event with total amount
    ///
    /// # Arguments
    /// * `wallet` - Investor wallet to sign transaction
    ///
    /// # Gas Buffer
    /// Uses 20% gas buffer
    ///
    /// # Requirements
    /// - Sale status must be Failed or Cancelled
    /// - Investor must have contributions (contributedNormalized > 0)
    /// - Not already refunded
    /// - Sufficient refund pool for each token contributed
    pub async fn claim_refund(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::claimRefundCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(20))
            .await
    }

    /// Cancel the sale (admin only)
    pub async fn cancel(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::cancelCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Pause the contract (admin only)
    pub async fn pause(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::pauseCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Unpause the contract (admin only)
    pub async fn unpause(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::unpauseCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Emergency withdraw ERC-20 tokens (admin only, only when Cancelled)
    pub async fn emergency_withdraw_erc20(
        &self,
        token: Address,
        recipient: Address,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::emergencyWithdrawERC20Call {
            token,
            recipient,
            amount,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, Some(20))
            .await
    }

    // ========== Convenience Methods ==========

    /// Get all investors (paginated)
    ///
    /// # Arguments
    /// * `start` - Starting index (0-based)
    /// * `limit` - Maximum number of investors to return
    ///
    /// # Returns
    /// Returns a vector of investor addresses. If `start` is beyond the total count,
    /// returns an empty vector. If an error occurs during retrieval, stops at that
    /// point and returns investors collected so far.
    ///
    /// # Errors
    /// - Returns error if investor count cannot be retrieved
    /// - Returns error if investor count overflows usize
    /// - Returns error if start index is invalid
    pub async fn get_investors(&self, start: usize, limit: usize) -> Result<Vec<Address>, Error> {
        let total_count = self.investor_count().await?;
        let total: usize = total_count
            .try_into()
            .map_err(|_| Error::Other(format!(
                "Investor count overflow: {} exceeds maximum addressable size",
                total_count
            )))?;

        // Validate start index
        if start > total {
            return Err(Error::Other(format!(
                "Start index {} exceeds total investor count {}",
                start, total
            )));
        }

        let end = std::cmp::min(start + limit, total);
        let expected_count = end - start;
        let mut investors = Vec::with_capacity(expected_count);

        for i in start..end {
            match self.get_investor(U256::from(i)).await {
                Ok(investor) => investors.push(investor),
                Err(e) => {
                    // Return partial results with context about what went wrong
                    return Err(Error::Other(format!(
                        "Failed to retrieve investor at index {}/{}: {}. Retrieved {} of {} expected investors.",
                        i, total, e, investors.len(), expected_count
                    )));
                }
            }
        }

        Ok(investors)
    }

    /// Get sale info summary
    ///
    /// # Updated for v2.0
    /// Now includes new fields: name, hard_cap, accepted_tokens, remaining_capacity.
    /// Returns normalized total contribution (not per-token breakdown).
    pub async fn get_sale_info(&self) -> Result<SaleInfo, Error> {
        Ok(SaleInfo {
            status: self.status().await?,
            name: self.name().await?,
            start: self.start().await?,
            end: self.end().await?,
            soft_cap: self.soft_cap().await?,
            hard_cap: self.hard_cap().await?,
            total_contributed_normalized: self.total_contributed_normalized().await?,
            investor_count: self.investor_count().await?,
            is_active: self.is_sale_active().await?,
            remaining_time: self.get_remaining_sale_time().await?,
            remaining_capacity: self.get_remaining_capacity().await?,
            commission_bps: self.commission_bps().await?,
            accepted_tokens: self.get_accepted_tokens().await?,
        })
    }

    /// Get investor information
    ///
    /// # New in v2.0
    /// Convenience method that aggregates investor data from multiple contract calls.
    ///
    /// # Arguments
    /// * `investor` - Investor address
    ///
    /// # Returns
    /// InvestorInfo struct with contribution, allocation, and status details
    pub async fn get_investor_info(&self, investor: Address) -> Result<InvestorInfo, Error> {
        let (contribution, tokens_allocated, is_kyc_approved, has_settled) =
            self.get_investor_distribution(investor).await?;

        // Get per-token contributions
        let accepted_tokens = self.get_accepted_tokens().await?;
        let mut contributions_by_token = std::collections::HashMap::new();
        for token in accepted_tokens {
            let amount = self.contributed_by_token(investor, token).await?;
            if amount > U256::ZERO {
                contributions_by_token.insert(token, amount);
            }
        }

        Ok(InvestorInfo {
            address: investor,
            contribution_normalized: contribution,
            contributions_by_token,
            tokens_allocated,
            is_kyc_approved,
            has_received_settlement: has_settled,
        })
    }

    /// Get settlement progress info
    ///
    /// # New in v2.0
    /// Convenience method that wraps settlement progress query.
    ///
    /// # Returns
    /// SettlementProgress struct with detailed progress information
    pub async fn get_settlement_progress_info(&self) -> Result<SettlementProgress, Error> {
        let (processed, total, is_initialized, is_complete) =
            self.get_settlement_progress().await?;

        // If not initialized, return early with empty state
        if !is_initialized {
            return Ok(SettlementProgress {
                processed_investors: U256::ZERO,
                total_investors: total,
                is_initialized: false,
                is_complete: false,
            });
        }

        Ok(SettlementProgress {
            processed_investors: processed,
            total_investors: total,
            is_initialized,
            is_complete,
        })
    }
}

/// Sale information summary
///
/// # Changed in v2.0
/// - Added: name, hard_cap, accepted_tokens, remaining_capacity
/// - Changed: total_contributed → total_contributed_normalized
#[derive(Debug, Clone)]
pub struct SaleInfo {
    pub status: u8,
    pub name: String,
    pub start: u64,
    pub end: u64,
    pub soft_cap: U256,
    pub hard_cap: U256,
    pub total_contributed_normalized: U256,
    pub investor_count: U256,
    pub is_active: bool,
    pub remaining_time: u64,
    pub remaining_capacity: U256,
    pub commission_bps: u16,
    pub accepted_tokens: Vec<Address>,
}

/// Investor information
///
/// # New in v2.0
/// Aggregates investor contribution and allocation details.
#[derive(Debug, Clone)]
pub struct InvestorInfo {
    pub address: Address,
    pub contribution_normalized: U256,
    pub contributions_by_token: std::collections::HashMap<Address, U256>,
    pub tokens_allocated: U256,
    pub is_kyc_approved: bool,
    pub has_received_settlement: bool,
}

/// Settlement progress
///
/// # New in v2.0
/// Tracks the state of the 3-step batch settlement process.
///
/// # Available Fields
/// - `processed_investors`: Number of investors processed so far
/// - `total_investors`: Total number of investors to process
/// - `is_initialized`: Whether settlement has been initialized
/// - `is_complete`: Whether all investors have been processed
///
/// # Limitations
/// This struct provides basic progress tracking. For complete settlement state
/// (asset token address, total distributed tokens, total refunded amounts),
/// query the contract directly using `get_settlement_state()` once available.
#[derive(Debug, Clone)]
pub struct SettlementProgress {
    pub processed_investors: U256,
    pub total_investors: U256,
    pub is_initialized: bool,
    pub is_complete: bool,
}

/// Validate settlement batch parameters before transaction submission
///
/// # Arguments
/// * `batch_size` - Number of investors to process
/// * `restricted_wallets` - List of addresses to flag as restricted
///
/// # Returns
/// Ok(()) if valid, Err with actionable message if invalid
///
/// # Validation Rules
/// - Batch size must be in range 1..=100
/// - Restricted wallets array must have length ≤ 50
///
/// # Why Client-Side Validation?
/// The contract enforces these limits on-chain, but client-side validation:
/// - Prevents wasted gas on guaranteed-to-fail transactions
/// - Provides instant feedback with clear error messages
/// - Guides users to fix invalid inputs before submission
fn validate_settlement_batch_params(
    batch_size: U256,
    restricted_wallets: &[Address],
) -> Result<(), Error> {
    // Validate batch size (1-100)
    let batch_size_u64: u64 = batch_size
        .try_into()
        .map_err(|_| Error::Other(
            format!("Batch size {} exceeds maximum u64", batch_size)
        ))?;

    if batch_size_u64 == 0 {
        return Err(Error::Other(
            "Batch size must be at least 1 investor".to_string()
        ));
    }

    if batch_size_u64 > 100 {
        return Err(Error::Other(
            format!(
                "Batch size {} exceeds maximum 100 investors.\n\
                \n\
                Contract enforces max 100 investors per batch to prevent gas exhaustion.\n\
                Current batch size: {}\n\
                Maximum allowed: 100\n\
                \n\
                To process {} investors, call settleBatch() {} times with batch_size=100",
                batch_size_u64,
                batch_size_u64,
                batch_size_u64,
                (batch_size_u64 + 99) / 100  // Ceiling division
            )
        ));
    }

    // Validate restricted wallets array (max 50)
    if restricted_wallets.len() > 50 {
        return Err(Error::Other(
            format!(
                "Too many restricted wallets: {} (maximum 50).\n\
                \n\
                Contract enforces max 50 restricted wallets per batch.\n\
                Current count: {}\n\
                Maximum allowed: 50\n\
                \n\
                Split your restricted wallets list across multiple settleBatch() calls.",
                restricted_wallets.len(),
                restricted_wallets.len()
            )
        ));
    }

    Ok(())
}

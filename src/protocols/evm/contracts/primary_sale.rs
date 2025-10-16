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

        // Events
        event Invested(
            address indexed investor,
            uint256 mantraUsdAmount,
            bytes32 screeningHash
        );
        event SaleEnded(uint256 indexed total, bool indexed softCapMet);
        event SettlementComplete(
            address indexed assetToken,
            address indexed assetOwner,
            uint256 indexed tokensDistributed,
            uint256 commission,
            uint256 proceeds
        );
        event RefundsFunded(uint256 indexed amount);
        event RefundClaimed(address indexed investor, uint256 indexed amount);
        event SaleActivated();
        event SaleCancelled();

        // View functions
        function status() external view returns (Status);
        function investorCount() external view returns (uint256);
        function tokensFor(address investor) external view returns (uint256);
        function getInvestor(uint256 index) external view returns (address);
        function isSaleActive() external view returns (bool);
        function getRemainingSaleTime() external view returns (uint64);
        function getTotalTokens() external view returns (uint256);
        function getCommissionAmount() external view returns (uint256);
        function contributed(address investor) external view returns (uint256);
        function totalContributed() external view returns (uint256);
        function refundsPool() external view returns (uint256);
        function refunded(address investor) external view returns (bool);

        // Immutable configuration
        function MANTRA_USD() external view returns (address);
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
        function invest(uint256 amount) external;
        function endSale() external;
        function settleAndDistribute(address assetToken, address assetOwner, uint256 maxLoop) external;
        function topUpRefunds(uint256 amount) external;
        function claimRefund() external;
        function cancel() external;
        function pause() external;
        function unpause() external;
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

    /// Get contribution amount for an investor
    pub async fn contributed(&self, investor: Address) -> Result<U256, Error> {
        let call = IPrimarySale::contributedCall { investor };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get total amount contributed to the sale
    pub async fn total_contributed(&self) -> Result<U256, Error> {
        let call = IPrimarySale::totalContributedCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get available refund pool amount
    pub async fn refunds_pool(&self) -> Result<U256, Error> {
        let call = IPrimarySale::refundsPoolCall {};
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

    /// Get mantraUSD token address
    pub async fn mantra_usd(&self) -> Result<Address, Error> {
        let call = IPrimarySale::MANTRA_USDCall {};
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
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Invest mantraUSD in the sale
    pub async fn invest(
        &self,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::investCall { amount };
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// End the sale (callable by anyone after end time)
    pub async fn end_sale(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::endSaleCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Settle and distribute tokens (settlement role only)
    pub async fn settle_and_distribute(
        &self,
        asset_token: Address,
        asset_owner: Address,
        max_loop: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::settleAndDistributeCall {
            assetToken: asset_token,
            assetOwner: asset_owner,
            maxLoop: max_loop,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Top up refunds pool
    pub async fn top_up_refunds(
        &self,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::topUpRefundsCall { amount };
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Claim refund (if eligible)
    pub async fn claim_refund(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::claimRefundCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Cancel the sale (admin only)
    pub async fn cancel(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::cancelCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Pause the contract (admin only)
    pub async fn pause(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::pauseCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Unpause the contract (admin only)
    pub async fn unpause(
        &self,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IPrimarySale::unpauseCall {};
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    // ========== Convenience Methods ==========

    /// Get all investors (paginated)
    pub async fn get_investors(&self, start: usize, limit: usize) -> Result<Vec<Address>, Error> {
        let total_count = self.investor_count().await?;
        let total = total_count
            .try_into()
            .map_err(|_| Error::Other("Investor count overflow".to_string()))?;

        let end = std::cmp::min(start + limit, total);
        let mut investors = Vec::new();

        for i in start..end {
            match self.get_investor(U256::from(i)).await {
                Ok(investor) => investors.push(investor),
                Err(_) => break, // Stop on first error (likely out of bounds)
            }
        }

        Ok(investors)
    }

    /// Get sale info summary
    pub async fn get_sale_info(&self) -> Result<SaleInfo, Error> {
        Ok(SaleInfo {
            status: self.status().await?,
            start: self.start().await?,
            end: self.end().await?,
            soft_cap: self.soft_cap().await?,
            total_contributed: self.total_contributed().await?,
            investor_count: self.investor_count().await?,
            is_active: self.is_sale_active().await?,
            remaining_time: self.get_remaining_sale_time().await?,
            commission_bps: self.commission_bps().await?,
        })
    }
}

/// Sale information summary
#[derive(Debug, Clone)]
pub struct SaleInfo {
    pub status: u8,
    pub start: u64,
    pub end: u64,
    pub soft_cap: U256,
    pub total_contributed: U256,
    pub investor_count: U256,
    pub is_active: bool,
    pub remaining_time: u64,
    pub commission_bps: u16,
}

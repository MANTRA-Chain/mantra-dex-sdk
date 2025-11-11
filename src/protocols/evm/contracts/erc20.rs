/// ERC-20 token contract helpers
///
/// Provides high-level methods for interacting with ERC-20 tokens.
use crate::error::Error;
use crate::protocols::evm::client::EvmClient;
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;

sol! {
    #[derive(Debug)]
    interface IERC20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);

        event Transfer(address indexed from, address indexed to, uint256 value);
        event Approval(address indexed owner, address indexed spender, uint256 value);
    }
}

/// ERC-20 token helper
pub struct Erc20 {
    client: EvmClient,
    address: Address,
}

impl Erc20 {
    /// Create a new ERC-20 helper for the given contract address
    pub fn new(client: EvmClient, address: Address) -> Self {
        Self { client, address }
    }

    /// Get token name
    pub async fn name(&self) -> Result<String, Error> {
        let call = IERC20::nameCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get token symbol
    pub async fn symbol(&self) -> Result<String, Error> {
        let call = IERC20::symbolCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get token decimals
    pub async fn decimals(&self) -> Result<u8, Error> {
        let call = IERC20::decimalsCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get total supply
    pub async fn total_supply(&self) -> Result<U256, Error> {
        let call = IERC20::totalSupplyCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get balance of an address
    pub async fn balance_of(&self, account: Address) -> Result<U256, Error> {
        let call = IERC20::balanceOfCall { account };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get the contract address
    pub fn address(&self) -> Address {
        self.address
    }

    /// Encode transfer call data
    pub fn encode_transfer(&self, to: Address, amount: U256) -> Vec<u8> {
        use alloy_sol_types::SolCall;
        let call = IERC20::transferCall { to, amount };
        call.abi_encode()
    }

    /// Transfer tokens
    pub async fn transfer(
        &self,
        to: Address,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC20::transferCall { to, amount };
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Get allowance
    pub async fn allowance(&self, owner: Address, spender: Address) -> Result<U256, Error> {
        let call = IERC20::allowanceCall { owner, spender };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Encode approve call data
    pub fn encode_approve(&self, spender: Address, amount: U256) -> Vec<u8> {
        use alloy_sol_types::SolCall;
        let call = IERC20::approveCall { spender, amount };
        call.abi_encode()
    }

    /// Approve spending
    pub async fn approve(
        &self,
        spender: Address,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC20::approveCall { spender, amount };
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }

    /// Transfer from (requires allowance)
    pub async fn transfer_from(
        &self,
        from: Address,
        to: Address,
        amount: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC20::transferFromCall { from, to, amount };
        self.client
            .send_contract_call(self.address, call, wallet, None)
            .await
    }
}

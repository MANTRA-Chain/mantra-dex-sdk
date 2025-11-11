/// ERC-721 NFT contract helpers
///
/// Provides high-level methods for interacting with ERC-721 NFTs.
use crate::error::Error;
use crate::protocols::evm::client::EvmClient;
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;

sol! {
    #[derive(Debug)]
    interface IERC721 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function tokenURI(uint256 tokenId) external view returns (string);
        function balanceOf(address owner) external view returns (uint256);
        function ownerOf(uint256 tokenId) external view returns (address);
        function getApproved(uint256 tokenId) external view returns (address);
        function isApprovedForAll(address owner, address operator) external view returns (bool);
        function transferFrom(address from, address to, uint256 tokenId) external;
        function safeTransferFrom(address from, address to, uint256 tokenId) external;
        function safeTransferFrom(address from, address to, uint256 tokenId, bytes calldata data) external;
        function approve(address to, uint256 tokenId) external;
        function setApprovalForAll(address operator, bool approved) external;

        event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
        event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId);
        event ApprovalForAll(address indexed owner, address indexed operator, bool approved);
    }
}

/// ERC-721 NFT helper
pub struct Erc721 {
    client: EvmClient,
    address: Address,
}

impl Erc721 {
    /// Create a new ERC-721 helper for the given contract address
    pub fn new(client: EvmClient, address: Address) -> Self {
        Self { client, address }
    }

    /// Get collection name
    pub async fn name(&self) -> Result<String, Error> {
        let call = IERC721::nameCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get collection symbol
    pub async fn symbol(&self) -> Result<String, Error> {
        let call = IERC721::symbolCall {};
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get token URI
    pub async fn token_uri(&self, token_id: U256) -> Result<String, Error> {
        let call = IERC721::tokenURICall { tokenId: token_id };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get balance of an address
    pub async fn balance_of(&self, owner: Address) -> Result<U256, Error> {
        let call = IERC721::balanceOfCall { owner };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get owner of a token
    pub async fn owner_of(&self, token_id: U256) -> Result<Address, Error> {
        let call = IERC721::ownerOfCall { tokenId: token_id };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Get approved address for a token
    pub async fn get_approved(&self, token_id: U256) -> Result<Address, Error> {
        let call = IERC721::getApprovedCall { tokenId: token_id };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Check if operator is approved for all tokens of owner
    pub async fn is_approved_for_all(
        &self,
        owner: Address,
        operator: Address,
    ) -> Result<bool, Error> {
        let call = IERC721::isApprovedForAllCall { owner, operator };
        let result = self.client.call_contract(self.address, call).await?;
        Ok(result._0)
    }

    /// Transfer token from one address to another
    pub async fn transfer_from(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC721::transferFromCall {
            from,
            to,
            tokenId: token_id,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Safe transfer token from one address to another
    pub async fn safe_transfer_from(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC721::safeTransferFrom_0Call {
            from,
            to,
            tokenId: token_id,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Safe transfer token with additional data
    pub async fn safe_transfer_from_with_data(
        &self,
        from: Address,
        to: Address,
        token_id: U256,
        data: Vec<u8>,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC721::safeTransferFrom_1Call {
            from,
            to,
            tokenId: token_id,
            data: data.into(),
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Approve an address to transfer a specific token
    pub async fn approve(
        &self,
        to: Address,
        token_id: U256,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC721::approveCall {
            to,
            tokenId: token_id,
        };
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }

    /// Set approval for all tokens
    pub async fn set_approval_for_all(
        &self,
        operator: Address,
        approved: bool,
        wallet: &crate::wallet::MultiVMWallet,
    ) -> Result<alloy_primitives::B256, Error> {
        let call = IERC721::setApprovalForAllCall { operator, approved };
        self.client
            .send_contract_call(self.address, call, wallet, None, None)
            .await
    }
}

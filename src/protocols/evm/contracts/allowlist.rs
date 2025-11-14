/// Allowlist contract helpers
///
/// Provides high-level methods for interacting with the MANTRA Allowlist contract,
/// which manages KYC/AML compliance for PrimarySale investors.
///
/// # Example
///
/// ```rust,no_run
/// use mantra_sdk::protocols::evm::contracts::allowlist::IAllowlist;
/// use alloy_sol_types::SolCall;
/// use alloy_primitives::address;
///
/// # fn example() {
/// // Encode setAllowedBatch call
/// let addresses = vec![address!("0x1111111111111111111111111111111111111111")];
/// let flags = vec![true];
/// let call = IAllowlist::setAllowedBatchCall {
///     addrs: addresses,
///     flags: flags,
/// };
/// let encoded = call.abi_encode();
/// # }
/// ```
use alloy_sol_types::sol;

sol! {
    #[derive(Debug)]
    interface IAllowlist {
        /// Set KYC/AML approval status for multiple addresses in batch
        ///
        /// # Arguments
        /// * `addrs` - Array of investor addresses to update
        /// * `flags` - Array of boolean flags (true = approved, false = removed)
        ///
        /// # Requirements
        /// * Arrays must have equal length
        /// * Caller must have ALLOWLIST_ADMIN_ROLE
        function setAllowedBatch(address[] calldata addrs, bool[] calldata flags) external;

        /// Check if an address is KYC approved
        function isAllowed(address addr) external view returns (bool);

        /// Get all allowed addresses (paginated)
        function getAllowedAddresses(uint256 start, uint256 count) external view returns (address[] memory);

        /// Get total count of allowed addresses
        function getAllowedCount() external view returns (uint256);

        // Events
        event AllowedSet(address indexed addr, bool indexed allowed);
        event BatchAllowedSet(uint256 indexed count, uint256 indexed timestamp);
    }
}

//! Multi-VM Wallet for Cosmos and EVM Chains
//!
//! This wallet supports both Cosmos-based blockchains and EVM (Ethereum Virtual Machine)
//! compatible chains using a single BIP-39 mnemonic phrase with different derivation paths.
//!
//! # Security Considerations
//!
//! ## Mnemonic Storage
//!
//! **⚠️ CRITICAL SECURITY NOTICE**: This wallet stores the mnemonic phrase in memory
//! during its lifetime. While we use the `secrecy` crate to protect the mnemonic with:
//! - Automatic zeroization when the wallet is dropped
//! - Redacted debug output
//! - Memory protection against casual inspection
//!
//! **This approach still has security risks**:
//!
//! ### Attack Surface
//! 1. **Memory Dumps**: Process memory dumps (crash dumps, debugging) may expose the mnemonic
//! 2. **Swap Files**: OS may swap memory to disk, persisting the secret temporarily
//! 3. **Debugging Tools**: Debuggers and profilers can read process memory
//! 4. **Side-Channel Attacks**: Timing attacks or memory access patterns could leak information
//! 5. **Long-Lived Processes**: The longer the process runs, the higher the exposure risk
//!
//! ### Why We Store the Mnemonic
//!
//! We store the mnemonic instead of just derived keys because:
//! 1. **Multi-VM Support**: We need to derive different keys for Cosmos (m/44'/118'/0'/0/N)
//!    and EVM (m/44'/60'/0'/0/N) from the same seed
//! 2. **Key Recreation**: Keys must be recreated on-demand for signing operations
//! 3. **Account Index Flexibility**: Users can derive keys for multiple accounts
//!
//! ### Mitigation Strategies
//!
//! Users of this wallet should:
//!
//! 1. **Minimize Lifetime**: Create wallet instances only when needed, drop immediately after use
//!    ```rust,no_run
//!    # use mantra_sdk::wallet::MultiVMWallet;
//!    # fn example() -> Result<(), mantra_sdk::error::Error> {
//!    {
//!        let wallet = MultiVMWallet::from_mnemonic(
//!            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
//!            0
//!        )?;
//!        let _signature = wallet.sign_cosmos_tx(todo!())?;
//!        // wallet dropped here, mnemonic zeroized
//!    }
//!    # Ok(())
//!    # }
//!    ```
//!
//! 2. **Avoid Long-Running Processes**: Don't keep wallets in memory for extended periods
//!
//! 3. **Secure Environments**: Run in secure environments with:
//!    - Encrypted memory (e.g., Intel SGX, ARM TrustZone)
//!    - Disabled core dumps: `ulimit -c 0`
//!    - Encrypted swap: `swapon -e`
//!    - Memory locking: `mlock()` for critical processes
//!
//! 4. **Hardware Security Modules**: For production systems, consider:
//!    - Hardware wallets (Ledger, Trezor)
//!    - HSM integration
//!    - Key Management Services (KMS)
//!
//! 5. **Principle of Least Privilege**: Run processes with minimal permissions
//!
//! ### Alternative Approaches
//!
//! For higher security requirements, consider:
//!
//! 1. **External Signers**: Delegate signing to hardware wallets or remote HSMs
//! 2. **Derived Key Storage**: Store only derived keys, not the mnemonic (limits flexibility)
//! 3. **Encrypted Key Files**: Store encrypted mnemonics on disk, decrypt only for operations
//! 4. **Multi-Signature Schemes**: Distribute trust across multiple keys/devices
//!
//! ### Compliance
//!
//! If your application handles:
//! - Large amounts of funds
//! - Production financial operations
//! - Regulatory compliance requirements (GDPR, PCI-DSS, SOC2)
//!
//! **You MUST**:
//! - Conduct a professional security audit
//! - Implement additional security layers
//! - Consider hardware security solutions
//! - Follow your organization's security policies
//!
//! ## Threat Model
//!
//! This wallet protects against:
//! - ✅ Casual memory inspection
//! - ✅ Accidental logging/debug output of secrets
//! - ✅ Secrets persisting after wallet drop
//!
//! This wallet does NOT protect against:
//! - ❌ Malicious code running in the same process
//! - ❌ OS-level memory inspection (debuggers, dumps)
//! - ❌ Physical memory attacks (cold boot, DMA)
//! - ❌ Side-channel attacks (timing, power analysis)

// Allow deprecated Signature for compatibility with alloy-consensus ecosystem
#![allow(deprecated)]

use crate::error::Error;
use bip32::{DerivationPath, Seed, XPrv};
use bip39::Mnemonic;
use cosmrs::crypto::secp256k1::{Signature, SigningKey as CosmosSigningKey};
use cosmrs::{tx::SignDoc, AccountId};
use secrecy::{ExposeSecret, Secret};
use std::str::FromStr;

/// HD Path for Cosmos chains (BIP-44)
const COSMOS_HD_PATH: &str = "m/44'/118'/0'/0/";

/// HD Path for Ethereum chains (BIP-44)
const ETHEREUM_HD_PATH: &str = "m/44'/60'/0'/0/";

/// MultiVM wallet that supports both Cosmos and EVM chains
/// Note: We store the mnemonic to recreate keys as needed since CosmosSigningKey
/// doesn't implement Clone or Debug. The mnemonic is protected with Secret to prevent
/// accidental exposure and is automatically zeroized when dropped.
pub struct MultiVMWallet {
    /// The mnemonic phrase (protected in memory, automatically zeroized on drop)
    mnemonic: Secret<String>,
    /// Account prefix for Cosmos addresses
    account_prefix: String,
    /// Account index used for derivation
    account_index: u32,
}

impl MultiVMWallet {
    /// Create a new MultiVM wallet from mnemonic
    pub fn from_mnemonic(mnemonic: &str, account_index: u32) -> Result<Self, Error> {
        // Validate the mnemonic
        let _ = Mnemonic::from_str(mnemonic)
            .map_err(|e| Error::Wallet(format!("Invalid mnemonic: {}", e)))?;

        Ok(Self {
            mnemonic: Secret::new(mnemonic.to_string()),
            account_prefix: "mantra".to_string(),
            account_index,
        })
    }

    /// Get the Cosmos signing key (recreated on demand)
    fn get_cosmos_signing_key(&self) -> Result<CosmosSigningKey, Error> {
        let mnemonic = Mnemonic::from_str(self.mnemonic.expose_secret())
            .map_err(|e| Error::Wallet(format!("Invalid stored mnemonic: {}", e)))?;

        let seed = mnemonic.to_seed("");
        let seed = Seed::new(seed);

        let cosmos_path = format!("{}{}", COSMOS_HD_PATH, self.account_index);
        let cosmos_path = DerivationPath::from_str(&cosmos_path)
            .map_err(|e| Error::Wallet(format!("Invalid Cosmos derivation path: {}", e)))?;

        let cosmos_derived_key = XPrv::derive_from_path(seed.as_bytes(), &cosmos_path)
            .map_err(|e| Error::Wallet(format!("Cosmos key derivation error: {}", e)))?;

        let cosmos_key_bytes = cosmos_derived_key.to_bytes();
        CosmosSigningKey::from_slice(&cosmos_key_bytes)
            .map_err(|e| Error::Wallet(format!("Failed to create Cosmos signing key: {}", e)))
    }

    /// Get the EVM signing key (recreated on demand)
    #[cfg(feature = "evm")]
    fn get_evm_signing_key(&self) -> Result<k256::ecdsa::SigningKey, Error> {
        let mnemonic = Mnemonic::from_str(self.mnemonic.expose_secret())
            .map_err(|e| Error::Wallet(format!("Invalid stored mnemonic: {}", e)))?;

        let seed = mnemonic.to_seed("");
        let seed = Seed::new(seed);

        let evm_path = format!("{}{}", ETHEREUM_HD_PATH, self.account_index);
        let evm_path = DerivationPath::from_str(&evm_path)
            .map_err(|e| Error::Wallet(format!("Invalid Ethereum derivation path: {}", e)))?;

        let evm_derived_key = XPrv::derive_from_path(seed.as_bytes(), &evm_path)
            .map_err(|e| Error::Wallet(format!("EVM key derivation error: {}", e)))?;

        let evm_key_bytes = evm_derived_key.to_bytes();
        k256::ecdsa::SigningKey::from_slice(&evm_key_bytes)
            .map_err(|e| Error::Wallet(format!("Failed to create EVM signing key: {}", e)))
    }

    /// Get the Cosmos address
    pub fn cosmos_address(&self) -> Result<AccountId, Error> {
        let signing_key = self.get_cosmos_signing_key()?;
        signing_key
            .public_key()
            .account_id(&self.account_prefix)
            .map_err(|e| Error::Wallet(format!("Failed to get Cosmos account ID: {}", e)))
    }

    /// Get the EVM address (Ethereum-compatible)
    #[cfg(feature = "evm")]
    pub fn evm_address(&self) -> Result<alloy_primitives::Address, Error> {
        use tiny_keccak::{Hasher, Keccak};

        let evm_signing_key = self.get_evm_signing_key()?;

        // Get the verifying key (public key) from the EVM signing key
        let verifying_key = evm_signing_key.verifying_key();

        // Encode as uncompressed point
        let point = verifying_key.to_encoded_point(false); // false = uncompressed
        let pubkey_bytes = point.as_bytes();

        // The uncompressed key should be 65 bytes (0x04 prefix + 64 bytes)
        if pubkey_bytes.len() != 65 || pubkey_bytes[0] != 0x04 {
            return Err(Error::Wallet(
                "Invalid public key format for Ethereum address derivation".to_string(),
            ));
        }

        // Compute Keccak-256 hash of the public key (excluding the 0x04 prefix)
        let mut hasher = Keccak::v256();
        hasher.update(&pubkey_bytes[1..65]); // Skip the 0x04 prefix
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);

        // Take the last 20 bytes as the Ethereum address
        let address = alloy_primitives::Address::from_slice(&hash[12..]);
        Ok(address)
    }

    /// Get the account index
    pub fn account_index(&self) -> u32 {
        self.account_index
    }

    /// Sign a Cosmos transaction
    pub fn sign_cosmos_tx(&self, sign_doc: SignDoc) -> Result<Signature, Error> {
        let signing_key = self.get_cosmos_signing_key()?;
        let sign_doc_bytes = sign_doc
            .into_bytes()
            .map_err(|e| Error::Wallet(format!("Failed to convert sign doc to bytes: {}", e)))?;
        signing_key
            .sign(&sign_doc_bytes)
            .map_err(|e| Error::Wallet(format!("Cosmos signing error: {}", e)))
    }

    /// Sign an Ethereum transaction with recoverable signature
    #[cfg(feature = "evm")]
    pub fn sign_ethereum_tx(
        &self,
        tx_hash: &[u8; 32],
    ) -> Result<(k256::ecdsa::Signature, k256::ecdsa::RecoveryId), Error> {
        let evm_signing_key = self.get_evm_signing_key()?;
        let (sig, recid) = evm_signing_key
            .sign_prehash_recoverable(tx_hash)
            .map_err(|e| Error::Wallet(format!("Failed to sign transaction: {}", e)))?;
        Ok((sig, recid))
    }

    /// Convert k256 signature to alloy Signature format
    ///
    /// This helper converts the ECDSA signature from k256 format to the alloy-primitives
    /// Signature format, handling the parity encoding correctly.
    #[cfg(feature = "evm")]
    pub fn to_alloy_signature(
        sig: &k256::ecdsa::Signature,
        recid: k256::ecdsa::RecoveryId,
    ) -> alloy_primitives::Signature {
        use alloy_primitives::Signature;

        Signature::from((*sig, recid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multivm_wallet_creation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let wallet = MultiVMWallet::from_mnemonic(mnemonic, 0).unwrap();

        // Test Cosmos address
        let cosmos_addr = wallet.cosmos_address().unwrap();
        assert!(!cosmos_addr.to_string().is_empty());

        // Test EVM address
        #[cfg(feature = "evm")]
        {
            let evm_addr = wallet.evm_address().unwrap();
            assert!(!evm_addr.to_string().is_empty());
            // The addresses should be different due to different derivation paths
            assert_ne!(cosmos_addr.to_string(), evm_addr.to_string());
        }
    }
}

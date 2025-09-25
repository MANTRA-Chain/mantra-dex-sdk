use crate::error::Error;
use bip32::{DerivationPath, XPrv, Seed};
use bip39::Mnemonic;
use cosmrs::crypto::secp256k1::{SigningKey as CosmosSigningKey, Signature};
use cosmrs::{AccountId, tx::SignDoc};
use std::str::FromStr;

/// HD Path for Cosmos chains (BIP-44)
const COSMOS_HD_PATH: &str = "m/44'/118'/0'/0/";

/// HD Path for Ethereum chains (BIP-44)
const ETHEREUM_HD_PATH: &str = "m/44'/60'/0'/0/";

/// MultiVM wallet that supports both Cosmos and EVM chains
/// Note: We store the mnemonic to recreate keys as needed since CosmosSigningKey
/// doesn't implement Clone or Debug
pub struct MultiVMWallet {
    /// The mnemonic phrase (stored for key recreation)
    mnemonic: String,
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
            mnemonic: mnemonic.to_string(),
            account_prefix: "mantra".to_string(),
            account_index,
        })
    }

    /// Get the Cosmos signing key (recreated on demand)
    fn get_cosmos_signing_key(&self) -> Result<CosmosSigningKey, Error> {
        let mnemonic = Mnemonic::from_str(&self.mnemonic)
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
        let mnemonic = Mnemonic::from_str(&self.mnemonic)
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
        use k256::elliptic_curve::sec1::ToEncodedPoint;
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

    /// Sign an Ethereum transaction
    #[cfg(feature = "evm")]
    pub fn sign_ethereum_tx(&self, tx_hash: &[u8; 32]) -> Result<k256::ecdsa::Signature, Error> {
        use k256::ecdsa::signature::Signer;

        let evm_signing_key = self.get_evm_signing_key()?;
        Ok(evm_signing_key.sign(tx_hash))
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
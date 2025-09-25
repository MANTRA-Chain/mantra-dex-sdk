use bip32::DerivationPath;
use bip39::Mnemonic;
use cosmrs::{
    crypto::secp256k1::{Signature, SigningKey},
    crypto::PublicKey,
    tx::{BodyBuilder, Fee, Raw, SignDoc, SignerInfo},
    AccountId, Coin as CosmosCoin, Denom,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[cfg(feature = "evm")]
use tiny_keccak::{Hasher, Keccak};

use crate::error::Error;

// Storage module for wallet persistence
pub mod storage;
pub use storage::*;

// MultiVM wallet for Cosmos and EVM support
pub mod multivm;
pub use multivm::MultiVMWallet;

/// HD Path prefix for Cosmos chains (BIP-44)
const HD_PATH_PREFIX: &str = "m/44'/118'/0'/0/";

/// Mantra wallet for managing key and signing transactions
pub struct MantraWallet {
    /// The signing account
    signing_account: cosmrs::crypto::secp256k1::SigningKey,
    /// The account prefix (mantra)
    account_prefix: String,
}

// Note: MantraWallet intentionally does not implement Clone for security reasons
// The signing key should not be easily duplicated

impl std::fmt::Debug for MantraWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MantraWallet")
            .field("account_prefix", &self.account_prefix)
            .field(
                "public_key",
                &hex::encode(self.signing_account.public_key().to_bytes()),
            )
            .finish()
    }
}

/// Wallet info that can be serialized safely
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfo {
    /// The wallet address
    pub address: String,
    /// The public key as hex
    pub public_key: String,
}

impl MantraWallet {
    /// Create a new wallet from a mnemonic
    pub fn from_mnemonic(mnemonic: &str, account_index: u32) -> Result<Self, Error> {
        let mnemonic = Mnemonic::from_str(mnemonic)
            .map_err(|e| Error::Wallet(format!("Invalid mnemonic: {}", e)))?;

        let seed = mnemonic.to_seed("");
        let seed = bip32::Seed::new(seed);

        let path = format!("{}{}", HD_PATH_PREFIX, account_index);
        let path = DerivationPath::from_str(&path)
            .map_err(|e| Error::Wallet(format!("Invalid derivation path: {}", e)))?;

        let derived_key = bip32::XPrv::derive_from_path(seed.as_bytes(), &path)
            .map_err(|e| Error::Wallet(format!("Key derivation error: {}", e)))?;

        let derived_key_bytes = derived_key.to_bytes();
        let signing_account = SigningKey::from_slice(&derived_key_bytes)
            .map_err(|e| Error::Wallet(format!("Failed to create signing account: {}", e)))?;

        Ok(Self {
            signing_account,
            account_prefix: "mantra".to_string(),
        })
    }

    /// Generate a new random wallet
    pub fn generate() -> Result<(Self, String), Error> {
        use rand::{thread_rng, RngCore};

        // Generate 16 bytes (128 bits) of entropy for a 12-word mnemonic
        let mut entropy = [0u8; 16];
        thread_rng().fill_bytes(&mut entropy);

        let mnemonic = Mnemonic::from_entropy(&entropy)
            .map_err(|e| Error::Wallet(format!("Failed to generate mnemonic: {}", e)))?;

        let phrase = mnemonic.to_string();
        let wallet = Self::from_mnemonic(&phrase, 0)?;

        Ok((wallet, phrase))
    }

    /// Get the wallet's address
    pub fn address(&self) -> Result<AccountId, Error> {
        self.signing_account
            .public_key()
            .account_id(&self.account_prefix)
            .map_err(|e| Error::Wallet(format!("Failed to get account ID: {}", e)))
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        self.signing_account.public_key()
    }

    /// Get access to the signing key
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_account
    }

    /// Sign a transaction doc
    pub fn sign_doc(&self, sign_doc: SignDoc) -> Result<Signature, Error> {
        let sign_doc_bytes = sign_doc
            .into_bytes()
            .map_err(|e| Error::Wallet(format!("Failed to convert sign doc to bytes: {}", e)))?;
        let signature = self
            .signing_account
            .sign(&sign_doc_bytes)
            .map_err(|e| Error::Wallet(format!("Signing error: {}", e)))?;
        Ok(signature)
    }

    /// Prepare and sign a transaction with body and auth info
    pub fn sign_tx(
        &self,
        account_number: u64,
        sequence: u64,
        chain_id: &str,
        fee: Fee,
        msgs: Vec<cosmrs::Any>,
        timeout_height: Option<u32>,
        memo: Option<String>,
    ) -> Result<Raw, Error> {
        // Create body builder and add messages
        let mut body_builder = BodyBuilder::new();
        body_builder.msgs(msgs);

        // Add memo if provided
        if let Some(memo_text) = memo {
            body_builder.memo(memo_text);
        }

        // Add timeout height if provided
        if let Some(height) = timeout_height {
            body_builder.timeout_height(height);
        }

        let tx_body = body_builder.finish();

        // Create signer info with sequence number
        let signer_info = SignerInfo::single_direct(Some(self.public_key()), sequence);

        // Create auth info with fee and signer info
        let auth_info = signer_info.auth_info(fee);

        // Create sign doc
        let chain_id = cosmrs::tendermint::chain::Id::from_str(chain_id)
            .map_err(|e| Error::Wallet(format!("Invalid chain ID: {}", e)))?;

        let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, account_number)
            .map_err(|e| Error::Wallet(format!("Failed to create sign doc: {}", e)))?;

        // Sign the transaction
        sign_doc
            .sign(&self.signing_account)
            .map_err(|e| Error::Wallet(format!("Failed to sign transaction: {}", e)))
    }

    /// Get wallet info
    pub fn info(&self) -> WalletInfo {
        WalletInfo {
            address: self.address().unwrap().to_string(),
            public_key: self.signing_account.public_key().to_string(),
        }
    }

    /// Create a fee object for transactions
    pub fn create_fee(&self, amount: u64, gas_limit: u64, denom: &str) -> Result<Fee, Error> {
        let denom =
            Denom::from_str(denom).map_err(|e| Error::Wallet(format!("Invalid denom: {}", e)))?;

        let coin = CosmosCoin {
            amount: amount.into(),
            denom,
        };

        Ok(Fee::from_amount_and_gas(coin, gas_limit))
    }

    /// Create a default fee using the native token
    pub fn create_default_fee(&self, gas_limit: u64) -> Result<Fee, Error> {
        let gas_price = self.compute_gas_price()?;
        let amount = (gas_limit as f64 * gas_price) as u64;

        // Load network constants
        let constants = crate::config::NetworkConstants::default_dukong()
            .map_err(|e| Error::Config(format!("Failed to load network constants: {}", e)))?;

        self.create_fee(amount, gas_limit, &constants.native_denom)
    }

    /// Calculate gas price with adjustment
    fn compute_gas_price(&self) -> Result<f64, Error> {
        // Load network constants
        let constants = crate::config::NetworkConstants::default_dukong()
            .map_err(|e| Error::Config(format!("Failed to load network constants: {}", e)))?;

        Ok(constants.default_gas_price * constants.default_gas_adjustment)
    }

    /// Derive Ethereum address from the wallet's public key
    ///
    /// Uses the same secp256k1 key as Cosmos but derives the Ethereum address
    /// by taking the Keccak-256 hash of the public key (without 0x04 prefix)
    /// and taking the last 20 bytes.
    #[cfg(feature = "evm")]
    pub fn ethereum_address(&self) -> Result<alloy_primitives::Address, Error> {
        use k256::ecdsa::{SigningKey as K256SigningKey, VerifyingKey};
        use k256::elliptic_curve::sec1::ToEncodedPoint;

        // The signing_account is a secp256k1 key wrapper - we need to extract the raw key
        // We'll recreate it from the same mnemonic/derivation for now
        // In production, we'd want to expose the raw key bytes from cosmrs

        // Get the compressed public key first to verify we have the right key
        let compressed_pubkey = self.signing_account.public_key();
        let compressed_bytes = compressed_pubkey.to_bytes();

        // Create a k256 verifying key from the compressed public key
        // The compressed key is 33 bytes (0x02/0x03 prefix + 32 bytes)
        let verifying_key = VerifyingKey::from_sec1_bytes(&compressed_bytes)
            .map_err(|e| Error::Wallet(format!("Failed to create verifying key: {}", e)))?;

        // Encode as uncompressed point
        let point = verifying_key.to_encoded_point(false); // false = uncompressed
        let pubkey_bytes = point.as_bytes();

        // The uncompressed key should be 65 bytes (0x04 prefix + 64 bytes)
        if pubkey_bytes.len() != 65 || pubkey_bytes[0] != 0x04 {
            return Err(Error::Wallet(
                "Invalid public key format for Ethereum address derivation".to_string(),
            ));
        }

        let pubkey_without_prefix = &pubkey_bytes[1..]; // Skip the 0x04 prefix

        // Compute Keccak-256 hash
        let mut hasher = Keccak::v256();
        hasher.update(pubkey_without_prefix);
        let mut hash = [0u8; 32];
        hasher.finalize(&mut hash);

        // Take the last 20 bytes
        let mut address_bytes = [0u8; 20];
        address_bytes.copy_from_slice(&hash[12..32]);

        Ok(alloy_primitives::Address::from(address_bytes))
    }

    /// Sign an Ethereum transaction (EIP-155 compatible)
    ///
    /// Note: This is a placeholder. Full EIP-155 signing implementation
    /// would require additional parameters and proper transaction serialization.
    #[cfg(feature = "evm")]
    pub fn sign_ethereum_transaction(
        &self,
        _tx_hash: &[u8; 32],
    ) -> Result<alloy_primitives::Signature, Error> {
        // TODO: Implement proper EIP-155 transaction signing
        // This would involve:
        // 1. Serializing the transaction according to EIP-155
        // 2. Computing the Keccak-256 hash
        // 3. Signing with the secp256k1 key
        // 4. Adjusting recovery id for Ethereum format

        Err(Error::Wallet(
            "Ethereum transaction signing not yet implemented".to_string(),
        ))
    }
}

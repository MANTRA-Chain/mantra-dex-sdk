//! Wallet management methods

use super::*;

impl McpSdkAdapter {
    pub async fn get_active_wallet(&self) -> McpResult<Option<MantraWallet>> {
        use crate::wallet::MantraWallet;
        use std::env;

        // Check if we have an active wallet address
        let active_address = self.active_wallet.lock().await.clone();
        if active_address.is_none() {
            return Ok(None);
        }

        // Try to recreate wallet from environment mnemonic using cached derivation index
        if let Ok(mnemonic) = env::var("WALLET_MNEMONIC") {
            if !mnemonic.trim().is_empty() {
                if let Some(active_addr) = &active_address {
                    // Check cache for derivation index
                    let cache = self.wallet_derivation_cache.read().await;
                    if let Some(&derivation_index) = cache.get(active_addr) {
                        match MantraWallet::from_mnemonic(&mnemonic, derivation_index) {
                            Ok(wallet) => {
                                debug!("Recreated active wallet instance from WALLET_MNEMONIC using cached index {}", derivation_index);
                                return Ok(Some(wallet));
                            }
                            Err(e) => {
                                error!("Failed to recreate active wallet from WALLET_MNEMONIC with cached index {}: {}", derivation_index, e);
                            }
                        }
                    } else {
                        // Fallback to index 0 for backward compatibility if no cache entry exists
                        match MantraWallet::from_mnemonic(&mnemonic, 0) {
                            Ok(wallet) => {
                                debug!("Recreated active wallet instance from WALLET_MNEMONIC using fallback index 0");
                                return Ok(Some(wallet));
                            }
                            Err(e) => {
                                error!("Failed to recreate active wallet from WALLET_MNEMONIC with fallback index 0: {}", e);
                            }
                        }
                    }
                }
            }
        }

        // Fall back to stored instance if available (though this will consume it)
        let wallet = self.active_wallet_instance.lock().await.take();
        if wallet.is_some() {
            debug!("Using stored wallet instance (will be consumed)");
        }
        Ok(wallet)
    }

    /// Get the currently active wallet info
    pub async fn get_active_wallet_info(&self) -> McpResult<Option<WalletInfo>> {
        let active_address = self.active_wallet.lock().await.clone();
        if let Some(address) = active_address {
            let wallets = self.wallets.read().await;
            Ok(wallets.get(&address).cloned())
        } else {
            debug!("No active wallet set");
            Ok(None)
        }
    }

    /// Set the active wallet
    pub async fn set_active_wallet(
        &self,
        address: String,
        wallet_info: WalletInfo,
    ) -> McpResult<()> {
        // Store the wallet info and set as active
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);
        *self.active_wallet.lock().await = Some(address.clone());

        info!("Set active wallet: {}", address);
        Ok(())
    }

    /// Set the active wallet with the actual wallet instance
    pub async fn set_active_wallet_with_instance(&self, wallet: MantraWallet) -> McpResult<()> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();

        // Store the wallet info
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);

        // Set as active
        *self.active_wallet.lock().await = Some(address.clone());

        // Store the wallet instance
        *self.active_wallet_instance.lock().await = Some(wallet);

        info!("Set active wallet with instance: {}", address);
        Ok(())
    }

    /// Add wallet validation methods
    pub async fn validate_wallet_exists(&self) -> McpResult<()> {
        if self.get_active_wallet_info().await?.is_none() {
            return Err(McpServerError::WalletNotConfigured);
        }
        Ok(())
    }

    /// Get all available wallets
    pub async fn get_all_wallets(&self) -> McpResult<HashMap<String, WalletInfo>> {
        let wallets = self.wallets.read().await;
        Ok(wallets.clone())
    }

    /// Add a new wallet to the collection
    pub async fn add_wallet(&self, wallet: MantraWallet) -> McpResult<String> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();

        // Store the wallet info
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);

        info!("Added new wallet: {}", address);
        Ok(address)
    }

    /// Add a new wallet to the collection with known derivation index for caching
    pub async fn add_wallet_with_derivation_index(
        &self,
        wallet: MantraWallet,
        derivation_index: u32,
    ) -> McpResult<String> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();

        // Store the wallet info
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);

        // Cache the derivation index for efficient wallet recreation
        {
            let mut cache = self.wallet_derivation_cache.write().await;
            cache.insert(address.clone(), derivation_index);
        }

        info!(
            "Added new wallet: {} with derivation index: {}",
            address, derivation_index
        );
        Ok(address)
    }

    /// Remove a wallet from the collection
    pub async fn remove_wallet(&self, address: &str) -> McpResult<()> {
        let mut wallets = self.wallets.write().await;

        if wallets.remove(address).is_some() {
            // Clear derivation cache entry
            {
                let mut cache = self.wallet_derivation_cache.write().await;
                cache.remove(address);
            }

            // If this was the active wallet, clear the active wallet
            let mut active_wallet = self.active_wallet.lock().await;
            if active_wallet.as_ref() == Some(&address.to_string()) {
                *active_wallet = None;
                *self.active_wallet_instance.lock().await = None;
            }
            info!("Removed wallet: {}", address);
            Ok(())
        } else {
            Err(McpServerError::InvalidArguments(format!(
                "Wallet not found: {}",
                address
            )))
        }
    }

    /// Switch active wallet to a different address
    pub async fn switch_active_wallet(&self, address: &str) -> McpResult<()> {
        let wallets = self.wallets.read().await;

        if let Some(_wallet_info) = wallets.get(address) {
            *self.active_wallet.lock().await = Some(address.to_string());
            // Clear the wallet instance - will be recreated when needed
            *self.active_wallet_instance.lock().await = None;
            info!("Switched active wallet to: {}", address);
            Ok(())
        } else {
            Err(McpServerError::InvalidArguments(format!(
                "Wallet not found: {}",
                address
            )))
        }
    }

    /// Get wallet info by address
    pub async fn get_wallet_info(&self, address: &str) -> McpResult<Option<WalletInfo>> {
        let wallets = self.wallets.read().await;
        Ok(wallets.get(address).cloned())
    }

    /// Check if a wallet exists
    pub async fn wallet_exists(&self, address: &str) -> bool {
        let wallets = self.wallets.read().await;
        wallets.contains_key(address)
    }

    /// Get a MultiVM wallet instance by address (for EVM operations)
    /// This creates a wallet with both Cosmos and EVM keys derived from the same mnemonic
    pub async fn get_multivm_wallet_by_address(
        &self,
        address: &str,
    ) -> McpResult<Option<MultiVMWallet>> {
        use std::env;

        // Check if wallet exists in our collection
        if !self.wallet_exists(address).await {
            return Ok(None);
        }

        // Get environment mnemonic
        let mnemonic = match env::var("WALLET_MNEMONIC") {
            Ok(m) if !m.trim().is_empty() => m,
            _ => {
                error!("WALLET_MNEMONIC environment variable is not set or empty");
                return Err(McpServerError::InvalidArguments(
                    "Cannot access wallet: mnemonic not configured".to_string(),
                ));
            }
        };

        // Check cache first for known derivation index
        {
            let cache = self.wallet_derivation_cache.read().await;
            if let Some(&derivation_index) = cache.get(address) {
                match MultiVMWallet::from_mnemonic(&mnemonic, derivation_index) {
                    Ok(wallet) => {
                        // Verify the Cosmos address matches
                        if let Ok(cosmos_addr) = wallet.cosmos_address() {
                            if cosmos_addr.to_string() == address {
                                debug!(
                                    "Retrieved MultiVM wallet from cache at index {} for address {}",
                                    derivation_index, address
                                );
                                return Ok(Some(wallet));
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to create MultiVM wallet for cached index {}: {}",
                            derivation_index, e
                        );
                    }
                }
            }
        }

        // Search for the correct derivation index
        debug!(
            "Performing derivation search for MultiVM wallet with address: {}",
            address
        );
        let max_index = self.config.max_wallet_derivation_index;
        for index in 0..=max_index {
            match MultiVMWallet::from_mnemonic(&mnemonic, index) {
                Ok(wallet) => {
                    if let Ok(cosmos_addr) = wallet.cosmos_address() {
                        if cosmos_addr.to_string() == address {
                            debug!(
                                "Found MultiVM wallet at derivation index {} for address {}",
                                index, address
                            );

                            // Cache the successful derivation index
                            {
                                let mut cache = self.wallet_derivation_cache.write().await;
                                cache.insert(address.to_string(), index);
                            }

                            return Ok(Some(wallet));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to create MultiVM wallet at index {}: {}", index, e);
                }
            }
        }

        warn!(
            "Could not find MultiVM wallet derivation for address {} within {} indices",
            address, max_index
        );
        Ok(None)
    }

    /// Get a wallet instance by address
    /// This method uses cached derivation indices for efficiency and falls back to a targeted search
    pub async fn get_wallet_by_address(&self, address: &str) -> McpResult<Option<MantraWallet>> {
        use crate::wallet::MantraWallet;
        use std::env;

        // Check if wallet exists in our collection
        if !self.wallet_exists(address).await {
            return Ok(None);
        }

        // Get environment mnemonic
        let mnemonic = match env::var("WALLET_MNEMONIC") {
            Ok(m) if !m.trim().is_empty() => m,
            _ => {
                debug!(
                    "No valid WALLET_MNEMONIC found in environment for address: {}",
                    address
                );
                return Ok(None);
            }
        };

        // Check cache first for known derivation index
        {
            let cache = self.wallet_derivation_cache.read().await;
            if let Some(&derivation_index) = cache.get(address) {
                match MantraWallet::from_mnemonic(&mnemonic, derivation_index) {
                    Ok(wallet) => {
                        if wallet.info().address == address {
                            debug!(
                                "Retrieved wallet from cache at index {} for address {}",
                                derivation_index, address
                            );
                            return Ok(Some(wallet));
                        } else {
                            // Cache is stale, wallet address doesn't match
                            warn!("Cached derivation index {} for address {} is stale, clearing cache entry", derivation_index, address);
                            drop(cache);
                            let mut cache_mut = self.wallet_derivation_cache.write().await;
                            cache_mut.remove(address);
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to recreate wallet from cached index {} for address {}: {}",
                            derivation_index, address, e
                        );
                        // Clear stale cache entry
                        drop(cache);
                        let mut cache_mut = self.wallet_derivation_cache.write().await;
                        cache_mut.remove(address);
                    }
                }
            }
        }

        // Cache miss or stale cache - perform targeted search
        debug!("Performing derivation search for address: {}", address);

        // Search with configurable upper bound to prevent infinite derivation
        let max_index = self.config.max_wallet_derivation_index;
        for index in 0..=max_index {
            match MantraWallet::from_mnemonic(&mnemonic, index) {
                Ok(wallet) => {
                    if wallet.info().address == address {
                        debug!(
                            "Found wallet at derivation index {} for address {}",
                            index, address
                        );

                        // Cache the successful derivation index
                        {
                            let mut cache = self.wallet_derivation_cache.write().await;
                            cache.insert(address.to_string(), index);
                        }

                        return Ok(Some(wallet));
                    }
                }
                Err(e) => {
                    debug!(
                        "Failed to create wallet at derivation index {}: {}",
                        index, e
                    );
                    // Continue searching - derivation errors at specific indices don't necessarily
                    // mean the wallet doesn't exist at a higher index
                }
            }
        }

        // If we reach here, the wallet was not found within the search bounds
        warn!(
            "Could not find wallet for address {} within derivation index range 0-{}",
            address, max_index
        );
        Ok(None)
    }

    /// Get wallet error handling with proper error messages
    pub async fn get_active_wallet_with_validation(&self) -> McpResult<MantraWallet> {
        match self.get_active_wallet().await? {
            Some(wallet) => Ok(wallet),
            None => Err(McpServerError::WalletNotConfigured),
        }
    }

    /// Get the EVM address for a wallet
    #[cfg(feature = "evm")]
    pub async fn get_wallet_evm_address(
        &self,
        wallet_address: Option<String>,
    ) -> McpResult<(String, String)> {
        // Get the wallet address to use
        let address = if let Some(addr) = wallet_address {
            addr
        } else {
            // Use active wallet
            match self.get_active_wallet_info().await? {
                Some(wallet_info) => wallet_info.address,
                None => return Err(McpServerError::WalletNotConfigured),
            }
        };

        // Get the MultiVM wallet which has both Cosmos and EVM keys
        let wallet = match self.get_multivm_wallet_by_address(&address).await? {
            Some(w) => w,
            None => {
                return Err(McpServerError::InvalidArguments(format!(
                    "Wallet not found: {}",
                    address
                )))
            }
        };

        // Get the EVM address using the proper Ethereum HD path
        let evm_address = wallet
            .evm_address()
            .map_err(McpServerError::Sdk)?
            .to_string();

        Ok((address, evm_address))
    }

    /// Get spendable balances for a specific address
    ///
    /// # Arguments
    ///
    /// * `network_config` - Network configuration for the query
    /// * `wallet_address` - Wallet address to query balances for
    ///
    /// # Returns
    ///
    /// JSON value containing balance information
    pub async fn get_balances_for_address_direct(
        &self,
        network_config: &MantraNetworkConfig,
        wallet_address: &str,
    ) -> McpResult<Value> {
        debug!("Getting balances for network: {}", network_config.chain_id);
        info!("Querying balances for address: {}", wallet_address);

        // Get client and execute balance query
        let client = self.get_client(network_config).await?;

        // Query spendable balances using the SDK client
        let balances = client
            .get_balances_for_address(wallet_address)
            .await
            .map_err(McpServerError::Sdk)?;

        debug!(
            "Retrieved {} balances for address {}",
            balances.len(),
            wallet_address
        );

        // Convert to JSON format
        let balance_json: Vec<Value> = balances
            .into_iter()
            .map(|coin| {
                serde_json::json!({
                    "denom": coin.denom,
                    "amount": coin.amount.to_string()
                })
            })
            .collect();

        let result = serde_json::json!({
            "address": wallet_address,
            "balances": balance_json,
            "total_tokens": balance_json.len(),
            "network": network_config.chain_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        info!(
            "Successfully retrieved balances for address: {}",
            wallet_address
        );
        Ok(result)
    }

    /// Get spendable balances for the active wallet
    ///
    /// # Arguments
    ///
    /// * `network_config` - Network configuration for the query
    /// * `wallet_address` - Optional specific wallet address, uses active wallet if None
    ///
    /// # Returns
    ///
    /// JSON value containing balance information
    pub async fn get_balances(
        &self,
        network_config: &MantraNetworkConfig,
        wallet_address: Option<String>,
    ) -> McpResult<Value> {
        // Get the wallet address to query
        let address = if let Some(addr) = wallet_address {
            addr
        } else {
            // Use active wallet address
            match self.get_active_wallet_info().await? {
                Some(wallet_info) => wallet_info.address,
                None => return Err(McpServerError::WalletNotConfigured),
            }
        };

        // Delegate to the direct address method
        self.get_balances_for_address_direct(network_config, &address)
            .await
    }
}

use crate::error::Error;
use crate::protocols::evm::types::EthAddress;
use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Duration, Instant};
use toml::Value;

const DEFAULT_TTL_SECS: u64 = 600;
const DEFAULT_REGISTRY_FILE: &str = "erc20_tokens.toml";
const DEFAULT_CUSTOM_FILE: &str = "erc20_tokens.local.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenKey {
    pub chain_id: u64,
    pub address: Address,
}

impl TokenKey {
    pub fn new(chain_id: u64, address: Address) -> Self {
        Self { chain_id, address }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    BuiltIn,
    Custom,
    Discovered,
}

#[derive(Debug, Clone)]
pub struct Erc20TokenInfo {
    pub address: Address,
    pub symbol: String,
    pub name: Option<String>,
    pub decimals: u8,
    pub chain_id: u64,
    pub last_refreshed: Option<Instant>,
    pub source: TokenSource,
}

impl Erc20TokenInfo {
    pub fn key(&self) -> TokenKey {
        TokenKey::new(self.chain_id, self.address)
    }

    pub fn checksummed_address(&self) -> String {
        format!("{:#x}", self.address)
    }

    pub fn needs_refresh(&self, ttl: Duration) -> bool {
        match self.last_refreshed {
            Some(last) => last.elapsed() >= ttl,
            None => true,
        }
    }
}

#[derive(Debug, Default)]
pub struct Erc20Registry {
    tokens: HashMap<TokenKey, Erc20TokenInfo>,
    network_index: HashMap<u64, Vec<Address>>,
    custom_tokens: HashSet<TokenKey>,
    custom_path: Option<PathBuf>,
    ttl: Duration,
}

#[derive(Debug, Deserialize)]
struct TokenEntry {
    address: String,
    symbol: String,
    #[serde(default)]
    name: Option<String>,
    decimals: u8,
    #[serde(default)]
    chain_id: Option<u64>,
    #[serde(default)]
    network: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct CustomSection {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    tokens: Vec<TokenEntry>,
}

impl Erc20Registry {
    pub fn load_default() -> Result<Self, Error> {
        let config_dir = Self::resolve_config_dir();
        Self::load_from_dir(config_dir)
    }

    fn resolve_config_dir() -> PathBuf {
        env::var("MANTRA_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config"))
    }

    fn load_from_dir(config_dir: PathBuf) -> Result<Self, Error> {
        let registry_path = config_dir.join(DEFAULT_REGISTRY_FILE);
        let ttl = Self::load_ttl();

        let mut registry = Self {
            tokens: HashMap::new(),
            network_index: HashMap::new(),
            custom_tokens: HashSet::new(),
            custom_path: None,
            ttl,
        };

        if registry_path.exists() {
            let raw = fs::read_to_string(&registry_path).map_err(|e| {
                Error::Config(format!(
                    "Failed to read ERC-20 registry at {}: {}",
                    registry_path.display(),
                    e
                ))
            })?;

            let value: Value = toml::from_str(&raw).map_err(|e| {
                Error::Config(format!(
                    "Failed to parse ERC-20 registry {}: {}",
                    registry_path.display(),
                    e
                ))
            })?;

            let mut network_chain_ids = HashMap::new();
            let mut custom_config = CustomSection::default();

            if let Some(table) = value.as_table() {
                for (key, entry) in table {
                    if key == "custom" {
                        custom_config = match CustomSection::deserialize(entry.clone()) {
                            Ok(cfg) => cfg,
                            Err(e) => {
                                return Err(Error::Config(format!(
                                    "Invalid custom token section: {}",
                                    e
                                )))
                            }
                        };
                        continue;
                    }

                    if let Some(network_table) = entry.as_table() {
                        let chain_id = network_table
                            .get("chain_id")
                            .and_then(|v| v.as_integer())
                            .ok_or_else(|| {
                                Error::Config(format!(
                                    "Network '{}' missing chain_id in registry",
                                    key
                                ))
                            })? as u64;
                        network_chain_ids.insert(key.clone(), chain_id);

                        if let Some(tokens_value) = network_table.get("tokens") {
                            if let Some(tokens_array) = tokens_value.as_array() {
                                for token_value in tokens_array {
                                    let token_entry = TokenEntry::deserialize(token_value.clone())
                                        .map_err(|e| {
                                            Error::Config(format!(
                                                "Invalid token entry for network '{}': {}",
                                                key, e
                                            ))
                                        })?;
                                    registry.insert_entry(
                                        &token_entry,
                                        chain_id,
                                        Some(key.as_str()),
                                        TokenSource::BuiltIn,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }

            // Inline custom tokens in base file
            for entry in custom_config.tokens {
                let chain = entry
                    .chain_id
                    .or_else(|| {
                        entry
                            .network
                            .as_ref()
                            .and_then(|name| network_chain_ids.get(name).copied())
                    })
                    .ok_or_else(|| {
                        Error::Config("Custom token entry missing chain_id or network".to_string())
                    })?;
                registry.insert_entry(
                    &entry,
                    chain,
                    entry.network.as_deref(),
                    TokenSource::Custom,
                )?;
            }

            // Load additional custom tokens from file if specified
            let mut custom_path = custom_config
                .path
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_CUSTOM_FILE));
            if !custom_path.is_absolute() {
                custom_path = config_dir.join(custom_path);
            }
            registry.custom_path = Some(custom_path.clone());
            if custom_path.exists() {
                registry.load_custom_file(&custom_path, &network_chain_ids)?;
            }
        }

        Ok(registry)
    }

    fn load_custom_file(
        &mut self,
        path: &Path,
        networks: &HashMap<String, u64>,
    ) -> Result<(), Error> {
        let raw = fs::read_to_string(path).map_err(|e| {
            Error::Config(format!(
                "Failed to read custom ERC-20 registry {}: {}",
                path.display(),
                e
            ))
        })?;
        let value: Value = toml::from_str(&raw).map_err(|e| {
            Error::Config(format!(
                "Failed to parse custom ERC-20 registry {}: {}",
                path.display(),
                e
            ))
        })?;

        if let Some(table) = value.as_table() {
            if let Some(tokens_value) = table.get("tokens") {
                if let Some(tokens_array) = tokens_value.as_array() {
                    for token_value in tokens_array {
                        let token_entry =
                            TokenEntry::deserialize(token_value.clone()).map_err(|e| {
                                Error::Config(format!("Invalid custom token entry: {}", e))
                            })?;
                        let chain = token_entry
                            .chain_id
                            .or_else(|| {
                                token_entry
                                    .network
                                    .as_ref()
                                    .and_then(|name| networks.get(name).copied())
                            })
                            .ok_or_else(|| {
                                Error::Config(
                                    "Custom token entry missing chain_id or network".to_string(),
                                )
                            })?;
                        self.insert_entry(
                            &token_entry,
                            chain,
                            token_entry.network.as_deref(),
                            TokenSource::Custom,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    fn insert_entry(
        &mut self,
        entry: &TokenEntry,
        chain_id: u64,
        network: Option<&str>,
        source: TokenSource,
    ) -> Result<(), Error> {
        let info = entry.to_info(chain_id, source.clone())?;
        let key = info.key();
        self.index_insert(chain_id, info.address);
        if source == TokenSource::Custom {
            self.custom_tokens.insert(key);
        }

        if let Some(network) = network {
            tracing::debug!(
                address = %info.checksummed_address(),
                symbol = %info.symbol,
                chain_id,
                network,
                "Loaded ERC-20 token from registry"
            );
        }

        self.tokens.insert(key, info);
        Ok(())
    }

    fn load_ttl() -> Duration {
        env::var("TOKEN_METADATA_CACHE_TTL")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_TTL_SECS))
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn get(&self, chain_id: u64, address: &Address) -> Option<&Erc20TokenInfo> {
        self.tokens.get(&TokenKey::new(chain_id, *address))
    }

    pub fn get_mut(&mut self, chain_id: u64, address: &Address) -> Option<&mut Erc20TokenInfo> {
        self.tokens.get_mut(&TokenKey::new(chain_id, *address))
    }

    pub fn list_for_chain(&self, chain_id: u64) -> Vec<Erc20TokenInfo> {
        self.network_index
            .get(&chain_id)
            .into_iter()
            .flatten()
            .filter_map(|addr| self.get(chain_id, addr).cloned())
            .collect()
    }

    pub fn upsert_custom(&mut self, mut info: Erc20TokenInfo) -> Result<(), Error> {
        info.source = TokenSource::Custom;
        let key = info.key();
        self.index_insert(info.chain_id, info.address);
        self.custom_tokens.insert(key);
        self.tokens.insert(key, info);
        self.persist_custom_tokens()
    }

    pub fn remove_custom(&mut self, chain_id: u64, address: &Address) -> Result<bool, Error> {
        let key = TokenKey::new(chain_id, *address);
        if self.custom_tokens.remove(&key) {
            self.tokens.remove(&key);
            self.remove_from_index(chain_id, address);
            self.persist_custom_tokens()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn mark_refreshed(&mut self, chain_id: u64, address: &Address) {
        if let Some(info) = self.get_mut(chain_id, address) {
            info.last_refreshed = Some(Instant::now());
        }
    }

    pub fn is_stale(&self, chain_id: u64, address: &Address) -> bool {
        self.get(chain_id, address)
            .map(|info| info.needs_refresh(self.ttl))
            .unwrap_or(true)
    }

    fn persist_custom_tokens(&self) -> Result<(), Error> {
        let Some(path) = &self.custom_path else {
            return Ok(());
        };

        let mut tokens: Vec<&Erc20TokenInfo> = self
            .custom_tokens
            .iter()
            .filter_map(|key| self.tokens.get(key))
            .collect();
        tokens.sort_by(|a, b| a.symbol.cmp(&b.symbol));

        let serialized_tokens: Vec<Value> = tokens
            .iter()
            .map(|info| {
                let entry = TokenEntrySerialize::from(*info);
                toml::Value::try_from(entry).expect("serialize token entry")
            })
            .collect();

        let mut table = toml::map::Map::new();
        table.insert("tokens".to_string(), Value::Array(serialized_tokens));
        let toml_string = Value::Table(table).to_string();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                Error::Config(format!(
                    "Failed to create directory for custom ERC-20 registry {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        fs::write(path, toml_string).map_err(|e| {
            Error::Config(format!(
                "Failed to persist custom ERC-20 registry {}: {}",
                path.display(),
                e
            ))
        })
    }
}

impl Erc20Registry {
    fn index_insert(&mut self, chain_id: u64, address: Address) {
        let entry = self.network_index.entry(chain_id).or_default();
        if !entry.contains(&address) {
            entry.push(address);
        }
    }

    fn remove_from_index(&mut self, chain_id: u64, address: &Address) {
        if let Some(entries) = self.network_index.get_mut(&chain_id) {
            entries.retain(|addr| addr != address);
        }
    }

    pub fn upsert_runtime(&mut self, info: Erc20TokenInfo) {
        let key = info.key();
        self.index_insert(info.chain_id, info.address);
        self.tokens.insert(key, info);
    }
}

#[derive(serde::Serialize)]
struct TokenEntrySerialize<'a> {
    address: String,
    symbol: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a String>,
    decimals: u8,
    chain_id: u64,
}

impl<'a> From<&'a Erc20TokenInfo> for TokenEntrySerialize<'a> {
    fn from(info: &'a Erc20TokenInfo) -> Self {
        Self {
            address: info.checksummed_address(),
            symbol: &info.symbol,
            name: info.name.as_ref(),
            decimals: info.decimals,
            chain_id: info.chain_id,
        }
    }
}

impl TokenEntry {
    fn to_info(&self, chain_id: u64, source: TokenSource) -> Result<Erc20TokenInfo, Error> {
        let address = Address::from_str(&self.address).map_err(|e| {
            Error::Config(format!("Invalid ERC-20 address '{}': {}", self.address, e))
        })?;
        Ok(Erc20TokenInfo {
            address,
            symbol: self.symbol.clone(),
            name: self.name.clone(),
            decimals: self.decimals,
            chain_id,
            last_refreshed: None,
            source,
        })
    }
}

impl Erc20Registry {
    pub fn to_eth_address(&self, key: &TokenKey) -> EthAddress {
        EthAddress(key.address)
    }
}

use config::{Config as ConfigLoader, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use crate::error::Error;

/// Environment variable prefixes for different configuration sections
const ENV_PREFIX: &str = "MANTRA";
const ENV_NETWORK_PREFIX: &str = "MANTRA_NETWORK";
const ENV_CONTRACT_PREFIX: &str = "MANTRA_CONTRACT";
const ENV_PROTOCOL_PREFIX: &str = "MANTRA_PROTOCOL";
const ENV_MCP_PREFIX: &str = "MANTRA_MCP";
const ENV_LOG_PREFIX: &str = "MANTRA_LOG";

/// Network configuration loaded from environment/files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEnvConfig {
    /// Network name (e.g., "mantra-dukong")
    pub name: Option<String>,
    /// Chain ID for transactions
    pub chain_id: Option<String>,
    /// RPC endpoint URL
    pub rpc_url: Option<String>,
    /// Alternative RPC endpoints for fallback
    pub rpc_fallback_urls: Vec<String>,
    /// Gas price in native token
    pub gas_price: Option<f64>,
    /// Gas adjustment multiplier
    pub gas_adjustment: Option<f64>,
    /// Native token denomination
    pub native_denom: Option<String>,
    /// Request timeout for RPC calls (seconds)
    pub rpc_timeout_secs: Option<u64>,
    /// Connection pool size for RPC clients
    pub rpc_pool_size: Option<u32>,
    /// EVM RPC endpoint URL (optional)
    #[cfg(feature = "evm")]
    pub evm_rpc_url: Option<String>,
    /// EVM chain ID (optional)
    #[cfg(feature = "evm")]
    pub evm_chain_id: Option<u64>,
}

impl Default for NetworkEnvConfig {
    fn default() -> Self {
        Self {
            name: None,
            chain_id: None,
            rpc_url: None,
            rpc_fallback_urls: Vec::new(),
            gas_price: None,
            gas_adjustment: None,
            native_denom: None,
            rpc_timeout_secs: None,
            rpc_pool_size: None,
            #[cfg(feature = "evm")]
            evm_rpc_url: None,
            #[cfg(feature = "evm")]
            evm_chain_id: None,
        }
    }
}

/// MCP server configuration from environment/files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpEnvConfig {
    /// Server name
    pub name: Option<String>,
    /// Server version
    pub version: Option<String>,
    /// Enable debug mode
    pub debug: Option<bool>,
    /// Maximum concurrent operations
    pub max_concurrent_ops: Option<u32>,
    /// Request timeout in seconds
    pub request_timeout_secs: Option<u64>,
    /// Cache TTL in seconds
    pub cache_ttl_secs: Option<u64>,
    /// Transport type (stdio, http)
    pub transport_type: Option<String>,
    /// HTTP host for HTTP transport
    pub http_host: Option<String>,
    /// HTTP port for HTTP transport
    pub http_port: Option<u16>,
    /// Enable MCP server on startup
    pub enabled: Option<bool>,
}

impl Default for McpEnvConfig {
    fn default() -> Self {
        Self {
            name: None,
            version: None,
            debug: None,
            max_concurrent_ops: None,
            request_timeout_secs: None,
            cache_ttl_secs: None,
            transport_type: None,
            http_host: None,
            http_port: None,
            enabled: None,
        }
    }
}

/// Logging configuration from environment/files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingEnvConfig {
    /// Log level (error, warn, info, debug, trace)
    pub level: Option<String>,
    /// Log format (compact, pretty, json)
    pub format: Option<String>,
    /// Enable colored output
    pub enable_colors: Option<bool>,
    /// Log file path (optional)
    pub file_path: Option<String>,
    /// Maximum log file size in MB
    pub max_file_size_mb: Option<u64>,
    /// Number of log files to retain
    pub max_files: Option<u32>,
    /// Enable structured logging
    pub structured: Option<bool>,
}

impl Default for LoggingEnvConfig {
    fn default() -> Self {
        Self {
            level: None,
            format: None,
            enable_colors: None,
            file_path: None,
            max_file_size_mb: None,
            max_files: None,
            structured: None,
        }
    }
}

/// Complete environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Network configuration
    pub network: NetworkEnvConfig,
    /// MCP server configuration
    pub mcp: McpEnvConfig,
    /// Logging configuration
    pub logging: LoggingEnvConfig,
    /// Custom environment variables
    pub custom: HashMap<String, String>,
    /// Configuration file paths that were loaded
    pub loaded_files: Vec<String>,
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            network: NetworkEnvConfig::default(),
            mcp: McpEnvConfig::default(),
            logging: LoggingEnvConfig::default(),
            custom: HashMap::new(),
            loaded_files: Vec::new(),
        }
    }
}

impl EnvironmentConfig {
    /// Load configuration from environment variables and files
    pub fn load() -> Result<Self, Error> {
        let mut env_config = Self::default();

        // Load from configuration files first
        env_config.load_from_files()?;

        // Override with environment variables
        env_config.load_from_env()?;

        // Validate the configuration
        env_config.validate()?;

        Ok(env_config)
    }

    /// Load configuration from files
    fn load_from_files(&mut self) -> Result<(), Error> {
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        // Configuration file names to try (in order of preference)
        let config_files = vec![
            "mantra.toml",
            "mantra.json",
            "config.toml",
            "config.json",
            "network.toml",
        ];

        // Paths to search for configuration files
        let search_paths = vec![
            config_dir.clone(),
            "config".to_string(),
            "../config".to_string(),
            "../../config".to_string(),
            ".".to_string(),
        ];

        for search_path in &search_paths {
            for config_file in &config_files {
                let file_path = Path::new(search_path).join(config_file);
                if file_path.exists() {
                    self.load_config_file(&file_path)?;
                    self.loaded_files
                        .push(file_path.to_string_lossy().to_string());
                }
            }
        }

        Ok(())
    }

    /// Load a specific configuration file
    fn load_config_file(&mut self, file_path: &Path) -> Result<(), Error> {
        let file_format = match file_path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") => FileFormat::Toml,
            Some("json") => FileFormat::Json,
            _ => FileFormat::Toml, // Default to TOML
        };

        let settings = ConfigLoader::builder()
            .add_source(File::from(file_path).format(file_format))
            .build()
            .map_err(|e| Error::Config(format!("Failed to load config file: {}", e)))?;

        // Load network configuration
        if let Ok(network_config) = settings.get::<NetworkEnvConfig>("network") {
            self.merge_network_config(network_config);
        }

        // Load MCP configuration
        if let Ok(mcp_config) = settings.get::<McpEnvConfig>("mcp") {
            self.merge_mcp_config(mcp_config);
        }

        // Load logging configuration
        if let Ok(logging_config) = settings.get::<LoggingEnvConfig>("logging") {
            self.merge_logging_config(logging_config);
        }

        // Load custom configuration
        if let Ok(custom_map) = settings.get::<HashMap<String, String>>("custom") {
            for (key, value) in custom_map {
                self.custom.insert(key, value);
            }
        }

        Ok(())
    }

    /// Load configuration from environment variables
    fn load_from_env(&mut self) -> Result<(), Error> {
        // Load network configuration from environment
        self.load_network_env()?;

        // Load MCP configuration from environment
        self.load_mcp_env()?;

        // Load logging configuration from environment
        self.load_logging_env()?;

        // Load custom environment variables
        self.load_custom_env()?;

        Ok(())
    }

    /// Load network configuration from environment variables
    fn load_network_env(&mut self) -> Result<(), Error> {
        if let Ok(name) = env::var(format!("{}_NAME", ENV_NETWORK_PREFIX)) {
            self.network.name = Some(name);
        }

        if let Ok(chain_id) = env::var(format!("{}_CHAIN_ID", ENV_NETWORK_PREFIX)) {
            self.network.chain_id = Some(chain_id);
        }

        if let Ok(rpc_url) = env::var(format!("{}_RPC_URL", ENV_NETWORK_PREFIX)) {
            self.network.rpc_url = Some(rpc_url);
        }

        if let Ok(fallback_urls) = env::var(format!("{}_RPC_FALLBACK_URLS", ENV_NETWORK_PREFIX)) {
            self.network.rpc_fallback_urls = fallback_urls
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(gas_price_str) = env::var(format!("{}_GAS_PRICE", ENV_NETWORK_PREFIX)) {
            if let Ok(gas_price) = gas_price_str.parse::<f64>() {
                self.network.gas_price = Some(gas_price);
            }
        }

        if let Ok(gas_adjustment_str) = env::var(format!("{}_GAS_ADJUSTMENT", ENV_NETWORK_PREFIX)) {
            if let Ok(gas_adjustment) = gas_adjustment_str.parse::<f64>() {
                self.network.gas_adjustment = Some(gas_adjustment);
            }
        }

        if let Ok(native_denom) = env::var(format!("{}_NATIVE_DENOM", ENV_NETWORK_PREFIX)) {
            self.network.native_denom = Some(native_denom);
        }

        if let Ok(timeout_str) = env::var(format!("{}_RPC_TIMEOUT_SECS", ENV_NETWORK_PREFIX)) {
            if let Ok(timeout) = timeout_str.parse::<u64>() {
                self.network.rpc_timeout_secs = Some(timeout);
            }
        }

        if let Ok(pool_size_str) = env::var(format!("{}_RPC_POOL_SIZE", ENV_NETWORK_PREFIX)) {
            if let Ok(pool_size) = pool_size_str.parse::<u32>() {
                self.network.rpc_pool_size = Some(pool_size);
            }
        }

        // Load EVM configuration from environment variables
        #[cfg(feature = "evm")]
        {
            if let Ok(evm_rpc_url) = env::var(format!("{}_EVM_RPC_URL", ENV_NETWORK_PREFIX)) {
                self.network.evm_rpc_url = Some(evm_rpc_url);
            }

            if let Ok(evm_chain_id_str) = env::var(format!("{}_EVM_CHAIN_ID", ENV_NETWORK_PREFIX)) {
                if let Ok(evm_chain_id) = evm_chain_id_str.parse::<u64>() {
                    self.network.evm_chain_id = Some(evm_chain_id);
                }
            }
        }

        Ok(())
    }

    /// Load MCP configuration from environment variables
    fn load_mcp_env(&mut self) -> Result<(), Error> {
        if let Ok(name) = env::var(format!("{}_NAME", ENV_MCP_PREFIX)) {
            self.mcp.name = Some(name);
        }

        if let Ok(version) = env::var(format!("{}_VERSION", ENV_MCP_PREFIX)) {
            self.mcp.version = Some(version);
        }

        if let Ok(debug_str) = env::var(format!("{}_DEBUG", ENV_MCP_PREFIX)) {
            if let Ok(debug) = debug_str.parse::<bool>() {
                self.mcp.debug = Some(debug);
            }
        }

        if let Ok(max_ops_str) = env::var(format!("{}_MAX_CONCURRENT_OPS", ENV_MCP_PREFIX)) {
            if let Ok(max_ops) = max_ops_str.parse::<u32>() {
                self.mcp.max_concurrent_ops = Some(max_ops);
            }
        }

        if let Ok(timeout_str) = env::var(format!("{}_REQUEST_TIMEOUT_SECS", ENV_MCP_PREFIX)) {
            if let Ok(timeout) = timeout_str.parse::<u64>() {
                self.mcp.request_timeout_secs = Some(timeout);
            }
        }

        if let Ok(cache_ttl_str) = env::var(format!("{}_CACHE_TTL_SECS", ENV_MCP_PREFIX)) {
            if let Ok(cache_ttl) = cache_ttl_str.parse::<u64>() {
                self.mcp.cache_ttl_secs = Some(cache_ttl);
            }
        }

        if let Ok(transport_type) = env::var(format!("{}_TRANSPORT_TYPE", ENV_MCP_PREFIX)) {
            self.mcp.transport_type = Some(transport_type);
        }

        if let Ok(http_host) = env::var(format!("{}_HTTP_HOST", ENV_MCP_PREFIX)) {
            self.mcp.http_host = Some(http_host);
        }

        if let Ok(http_port_str) = env::var(format!("{}_HTTP_PORT", ENV_MCP_PREFIX)) {
            if let Ok(http_port) = http_port_str.parse::<u16>() {
                self.mcp.http_port = Some(http_port);
            }
        }

        if let Ok(enabled_str) = env::var(format!("{}_ENABLED", ENV_MCP_PREFIX)) {
            if let Ok(enabled) = enabled_str.parse::<bool>() {
                self.mcp.enabled = Some(enabled);
            }
        }

        Ok(())
    }

    /// Load logging configuration from environment variables
    fn load_logging_env(&mut self) -> Result<(), Error> {
        if let Ok(level) = env::var(format!("{}_LEVEL", ENV_LOG_PREFIX)) {
            self.logging.level = Some(level);
        }

        if let Ok(format) = env::var(format!("{}_FORMAT", ENV_LOG_PREFIX)) {
            self.logging.format = Some(format);
        }

        if let Ok(colors_str) = env::var(format!("{}_ENABLE_COLORS", ENV_LOG_PREFIX)) {
            if let Ok(colors) = colors_str.parse::<bool>() {
                self.logging.enable_colors = Some(colors);
            }
        }

        if let Ok(file_path) = env::var(format!("{}_FILE_PATH", ENV_LOG_PREFIX)) {
            self.logging.file_path = Some(file_path);
        }

        if let Ok(max_size_str) = env::var(format!("{}_MAX_FILE_SIZE_MB", ENV_LOG_PREFIX)) {
            if let Ok(max_size) = max_size_str.parse::<u64>() {
                self.logging.max_file_size_mb = Some(max_size);
            }
        }

        if let Ok(max_files_str) = env::var(format!("{}_MAX_FILES", ENV_LOG_PREFIX)) {
            if let Ok(max_files) = max_files_str.parse::<u32>() {
                self.logging.max_files = Some(max_files);
            }
        }

        if let Ok(structured_str) = env::var(format!("{}_STRUCTURED", ENV_LOG_PREFIX)) {
            if let Ok(structured) = structured_str.parse::<bool>() {
                self.logging.structured = Some(structured);
            }
        }

        Ok(())
    }

    /// Load custom environment variables with MANTRA prefix
    fn load_custom_env(&mut self) -> Result<(), Error> {
        for (key, value) in env::vars() {
            if key.starts_with(ENV_PREFIX)
                && !key.starts_with(ENV_NETWORK_PREFIX)
                && !key.starts_with(ENV_CONTRACT_PREFIX)
                && !key.starts_with(ENV_PROTOCOL_PREFIX)
                && !key.starts_with(ENV_MCP_PREFIX)
                && !key.starts_with(ENV_LOG_PREFIX)
            {
                self.custom.insert(key, value);
            }
        }

        Ok(())
    }

    /// Merge network configuration (file config is overridden by env)
    fn merge_network_config(&mut self, file_config: NetworkEnvConfig) {
        if self.network.name.is_none() {
            self.network.name = file_config.name;
        }
        if self.network.chain_id.is_none() {
            self.network.chain_id = file_config.chain_id;
        }
        if self.network.rpc_url.is_none() {
            self.network.rpc_url = file_config.rpc_url;
        }
        if self.network.rpc_fallback_urls.is_empty() {
            self.network.rpc_fallback_urls = file_config.rpc_fallback_urls;
        }
        if self.network.gas_price.is_none() {
            self.network.gas_price = file_config.gas_price;
        }
        if self.network.gas_adjustment.is_none() {
            self.network.gas_adjustment = file_config.gas_adjustment;
        }
        if self.network.native_denom.is_none() {
            self.network.native_denom = file_config.native_denom;
        }
        if self.network.rpc_timeout_secs.is_none() {
            self.network.rpc_timeout_secs = file_config.rpc_timeout_secs;
        }
        if self.network.rpc_pool_size.is_none() {
            self.network.rpc_pool_size = file_config.rpc_pool_size;
        }
    }

    /// Merge MCP configuration (file config is overridden by env)
    fn merge_mcp_config(&mut self, file_config: McpEnvConfig) {
        if self.mcp.name.is_none() {
            self.mcp.name = file_config.name;
        }
        if self.mcp.version.is_none() {
            self.mcp.version = file_config.version;
        }
        if self.mcp.debug.is_none() {
            self.mcp.debug = file_config.debug;
        }
        if self.mcp.max_concurrent_ops.is_none() {
            self.mcp.max_concurrent_ops = file_config.max_concurrent_ops;
        }
        if self.mcp.request_timeout_secs.is_none() {
            self.mcp.request_timeout_secs = file_config.request_timeout_secs;
        }
        if self.mcp.cache_ttl_secs.is_none() {
            self.mcp.cache_ttl_secs = file_config.cache_ttl_secs;
        }
        if self.mcp.transport_type.is_none() {
            self.mcp.transport_type = file_config.transport_type;
        }
        if self.mcp.http_host.is_none() {
            self.mcp.http_host = file_config.http_host;
        }
        if self.mcp.http_port.is_none() {
            self.mcp.http_port = file_config.http_port;
        }
        if self.mcp.enabled.is_none() {
            self.mcp.enabled = file_config.enabled;
        }
    }

    /// Merge logging configuration (file config is overridden by env)
    fn merge_logging_config(&mut self, file_config: LoggingEnvConfig) {
        if self.logging.level.is_none() {
            self.logging.level = file_config.level;
        }
        if self.logging.format.is_none() {
            self.logging.format = file_config.format;
        }
        if self.logging.enable_colors.is_none() {
            self.logging.enable_colors = file_config.enable_colors;
        }
        if self.logging.file_path.is_none() {
            self.logging.file_path = file_config.file_path;
        }
        if self.logging.max_file_size_mb.is_none() {
            self.logging.max_file_size_mb = file_config.max_file_size_mb;
        }
        if self.logging.max_files.is_none() {
            self.logging.max_files = file_config.max_files;
        }
        if self.logging.structured.is_none() {
            self.logging.structured = file_config.structured;
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Error> {
        // Validate network configuration
        if let Some(ref gas_price) = self.network.gas_price {
            if *gas_price <= 0.0 {
                return Err(Error::Config("Gas price must be positive".to_string()));
            }
        }

        if let Some(ref gas_adjustment) = self.network.gas_adjustment {
            if *gas_adjustment <= 0.0 {
                return Err(Error::Config("Gas adjustment must be positive".to_string()));
            }
        }

        if let Some(ref rpc_timeout) = self.network.rpc_timeout_secs {
            if *rpc_timeout == 0 {
                return Err(Error::Config(
                    "RPC timeout must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(ref pool_size) = self.network.rpc_pool_size {
            if *pool_size == 0 {
                return Err(Error::Config(
                    "RPC pool size must be greater than 0".to_string(),
                ));
            }
        }

        // Validate RPC URLs
        if let Some(ref rpc_url) = self.network.rpc_url {
            if !rpc_url.starts_with("http://") && !rpc_url.starts_with("https://") {
                return Err(Error::Config(
                    "RPC URL must start with http:// or https://".to_string(),
                ));
            }
        }

        for fallback_url in &self.network.rpc_fallback_urls {
            if !fallback_url.starts_with("http://") && !fallback_url.starts_with("https://") {
                return Err(Error::Config(
                    "Fallback RPC URL must start with http:// or https://".to_string(),
                ));
            }
        }

        // Validate MCP configuration
        if let Some(ref max_ops) = self.mcp.max_concurrent_ops {
            if *max_ops == 0 {
                return Err(Error::Config(
                    "Max concurrent operations must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(ref request_timeout) = self.mcp.request_timeout_secs {
            if *request_timeout == 0 {
                return Err(Error::Config(
                    "MCP request timeout must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(ref cache_ttl) = self.mcp.cache_ttl_secs {
            if *cache_ttl == 0 {
                return Err(Error::Config(
                    "MCP cache TTL must be greater than 0".to_string(),
                ));
            }
        }

        if let Some(ref transport_type) = self.mcp.transport_type {
            if transport_type != "stdio" && transport_type != "http" {
                return Err(Error::Config(
                    "MCP transport type must be 'stdio' or 'http'".to_string(),
                ));
            }
        }

        // Validate logging configuration
        if let Some(ref level) = self.logging.level {
            let valid_levels = ["error", "warn", "info", "debug", "trace"];
            if !valid_levels.contains(&level.as_str()) {
                return Err(Error::Config(format!(
                    "Invalid log level '{}'. Must be one of: {:?}",
                    level, valid_levels
                )));
            }
        }

        if let Some(ref format) = self.logging.format {
            let valid_formats = ["compact", "pretty", "json"];
            if !valid_formats.contains(&format.as_str()) {
                return Err(Error::Config(format!(
                    "Invalid log format '{}'. Must be one of: {:?}",
                    format, valid_formats
                )));
            }
        }

        if let Some(ref max_files) = self.logging.max_files {
            if *max_files == 0 {
                return Err(Error::Config(
                    "Max log files must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Generate default configuration file
    pub fn generate_default_config() -> Self {
        let mut config = Self::default();

        // Set default network configuration
        config.network.name = Some("mantra-dukong".to_string());
        config.network.chain_id = Some("mantra-dukong-1".to_string());
        config.network.rpc_url = Some("https://rpc.dukong.mantrachain.io:443".to_string());
        config.network.gas_price = Some(0.01);
        config.network.gas_adjustment = Some(1.5);
        config.network.native_denom = Some("uom".to_string());
        config.network.rpc_timeout_secs = Some(30);
        config.network.rpc_pool_size = Some(5);

        // Set default MCP configuration
        config.mcp.name = Some("Mantra DEX MCP Server".to_string());
        config.mcp.version = Some("0.1.0".to_string());
        config.mcp.debug = Some(false);
        config.mcp.max_concurrent_ops = Some(10);
        config.mcp.request_timeout_secs = Some(30);
        config.mcp.cache_ttl_secs = Some(300);
        config.mcp.transport_type = Some("stdio".to_string());
        config.mcp.http_host = Some("127.0.0.1".to_string());
        config.mcp.http_port = Some(8080);
        config.mcp.enabled = Some(true);

        // Set default logging configuration
        config.logging.level = Some("info".to_string());
        config.logging.format = Some("compact".to_string());
        config.logging.enable_colors = Some(true);
        config.logging.max_file_size_mb = Some(10);
        config.logging.max_files = Some(5);
        config.logging.structured = Some(false);

        config
    }

    /// Save configuration to file
    pub fn save_to_file(&self, file_path: &Path) -> Result<(), Error> {
        // Create directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;

        fs::write(file_path, content)?;
        Ok(())
    }

    /// Get a custom environment variable value
    pub fn get_custom(&self, key: &str) -> Option<&String> {
        self.custom.get(key)
    }

    /// Set a custom environment variable value
    pub fn set_custom(&mut self, key: String, value: String) {
        self.custom.insert(key, value);
    }

    /// Get network name with fallback
    pub fn get_network_name(&self) -> String {
        self.network
            .name
            .clone()
            .unwrap_or_else(|| "mantra-dukong".to_string())
    }

    /// Get chain ID with fallback
    pub fn get_chain_id(&self) -> String {
        self.network
            .chain_id
            .clone()
            .unwrap_or_else(|| "mantra-dukong-1".to_string())
    }

    /// Get RPC URL with fallback
    pub fn get_rpc_url(&self) -> String {
        self.network
            .rpc_url
            .clone()
            .unwrap_or_else(|| "https://rpc.dukong.mantrachain.io:443".to_string())
    }

    /// Get gas price with fallback
    pub fn get_gas_price(&self) -> f64 {
        self.network.gas_price.unwrap_or(0.01)
    }

    /// Get gas adjustment with fallback
    pub fn get_gas_adjustment(&self) -> f64 {
        self.network.gas_adjustment.unwrap_or(1.5)
    }

    /// Get native denom with fallback
    pub fn get_native_denom(&self) -> String {
        self.network
            .native_denom
            .clone()
            .unwrap_or_else(|| "uom".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_environment_config_defaults() {
        let config = EnvironmentConfig::generate_default_config();

        assert_eq!(config.get_network_name(), "mantra-dukong");
        assert_eq!(config.get_chain_id(), "mantra-dukong-1");
        assert_eq!(config.get_gas_price(), 0.01);
        assert_eq!(config.get_native_denom(), "uom");
    }

    #[test]
    fn test_environment_config_validation() {
        let mut config = EnvironmentConfig::generate_default_config();

        // Valid configuration should pass
        assert!(config.validate().is_ok());

        // Invalid gas price should fail
        config.network.gas_price = Some(-1.0);
        assert!(config.validate().is_err());

        // Fix gas price and test invalid RPC URL
        config.network.gas_price = Some(0.01);
        config.network.rpc_url = Some("invalid-url".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_variable_loading() {
        // Set test environment variables
        env::set_var("MANTRA_NETWORK_NAME", "test-network");
        env::set_var("MANTRA_NETWORK_CHAIN_ID", "test-1");
        env::set_var("MANTRA_MCP_DEBUG", "true");
        env::set_var("MANTRA_LOG_LEVEL", "debug");

        let mut config = EnvironmentConfig::default();
        config.load_from_env().unwrap();

        assert_eq!(config.network.name, Some("test-network".to_string()));
        assert_eq!(config.network.chain_id, Some("test-1".to_string()));
        assert_eq!(config.mcp.debug, Some(true));
        assert_eq!(config.logging.level, Some("debug".to_string()));

        // Clean up
        env::remove_var("MANTRA_NETWORK_NAME");
        env::remove_var("MANTRA_NETWORK_CHAIN_ID");
        env::remove_var("MANTRA_MCP_DEBUG");
        env::remove_var("MANTRA_LOG_LEVEL");
    }

    #[test]
    fn test_config_merging() {
        let mut config = EnvironmentConfig::default();

        // File config
        let file_config = NetworkEnvConfig {
            name: Some("file-network".to_string()),
            chain_id: Some("file-1".to_string()),
            rpc_url: Some("https://file-rpc.example.com".to_string()),
            ..Default::default()
        };

        config.merge_network_config(file_config);

        // Environment override
        config.network.name = Some("env-network".to_string());

        assert_eq!(config.network.name, Some("env-network".to_string())); // Env overrides
        assert_eq!(config.network.chain_id, Some("file-1".to_string())); // File value used
        assert_eq!(
            config.network.rpc_url,
            Some("https://file-rpc.example.com".to_string())
        ); // File value used
    }
}

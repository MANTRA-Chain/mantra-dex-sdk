use config::{Config as ConfigLoader, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::time::Duration;

use crate::error::Error;

/// Protocol identifier enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolId {
    /// DEX protocol for trading and liquidity
    Dex,
    /// ClaimDrop protocol for reward distribution
    ClaimDrop,
    /// EVM protocol for Ethereum Virtual Machine compatibility
    #[cfg(feature = "evm")]
    Evm,
    /// Skip protocol for cross-chain operations
    Skip,
}

impl std::fmt::Display for ProtocolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolId::Dex => write!(f, "dex"),
            ProtocolId::ClaimDrop => write!(f, "claimdrop"),
            #[cfg(feature = "evm")]
            ProtocolId::Evm => write!(f, "evm"),
            ProtocolId::Skip => write!(f, "skip"),
        }
    }
}

impl std::str::FromStr for ProtocolId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dex" => Ok(ProtocolId::Dex),
            "claimdrop" => Ok(ProtocolId::ClaimDrop),
            #[cfg(feature = "evm")]
            "evm" => Ok(ProtocolId::Evm),
            "skip" => Ok(ProtocolId::Skip),
            _ => Err(Error::Config(format!("Unknown protocol: {}", s))),
        }
    }
}

/// Fee configuration for protocol operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConfig {
    /// Default transaction fee in native token (uom)
    pub default_tx_fee: u64,
    /// Gas price multiplier for fee calculation
    pub gas_price_multiplier: f64,
    /// Maximum allowed fee for transactions
    pub max_fee: u64,
    /// Protocol-specific fee percentage (basis points, 10000 = 100%)
    pub protocol_fee_bps: u16,
    /// Minimum fee amount regardless of calculation
    pub min_fee: u64,
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            default_tx_fee: 5000, // 0.005 OM
            gas_price_multiplier: 1.5,
            max_fee: 100000,      // 0.1 OM
            protocol_fee_bps: 30, // 0.3%
            min_fee: 1000,        // 0.001 OM
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute
    pub requests_per_minute: u32,
    /// Maximum concurrent operations
    pub max_concurrent_ops: u32,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Burst capacity for sudden request spikes
    pub burst_capacity: u32,
    /// Cool-down period after hitting limits (seconds)
    pub cooldown_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            max_concurrent_ops: 10,
            request_timeout_secs: 30,
            burst_capacity: 20,
            cooldown_secs: 60,
        }
    }
}

/// Health monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Health check interval in seconds
    pub check_interval_secs: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Health check timeout in seconds
    pub health_check_timeout_secs: u64,
    /// Whether to enable automatic recovery attempts
    pub auto_recovery: bool,
    /// Recovery attempt interval in seconds
    pub recovery_interval_secs: u64,
    /// Maximum number of recovery attempts
    pub max_recovery_attempts: u32,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 30,
            failure_threshold: 3,
            health_check_timeout_secs: 10,
            auto_recovery: true,
            recovery_interval_secs: 60,
            max_recovery_attempts: 5,
        }
    }
}

/// Protocol-specific parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolParameters {
    /// Maximum slippage tolerance (basis points)
    pub max_slippage_bps: u16,
    /// Default slippage tolerance (basis points)
    pub default_slippage_bps: u16,
    /// Minimum trade amount in native token
    pub min_trade_amount: u64,
    /// Maximum trade amount in native token
    pub max_trade_amount: u64,
    /// Protocol-specific settings as key-value pairs
    pub custom_params: HashMap<String, String>,
}

impl Default for ProtocolParameters {
    fn default() -> Self {
        Self {
            max_slippage_bps: 1000,       // 10%
            default_slippage_bps: 50,     // 0.5%
            min_trade_amount: 1000,       // 0.001 OM
            max_trade_amount: 1000000000, // 1000 OM
            custom_params: HashMap::new(),
        }
    }
}

/// Complete protocol configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Protocol identifier
    pub protocol_id: ProtocolId,
    /// Whether the protocol is enabled
    pub enabled: bool,
    /// Protocol version
    pub version: String,
    /// Protocol-specific parameters
    pub parameters: ProtocolParameters,
    /// Fee configuration
    pub fees: FeeConfig,
    /// Rate limiting configuration
    pub rate_limits: RateLimitConfig,
    /// Health monitoring configuration
    pub health: HealthConfig,
    /// Priority level (higher number = higher priority)
    pub priority: u32,
    /// Network-specific overrides
    pub network_overrides: HashMap<String, ProtocolConfigOverride>,
}

/// Network-specific protocol configuration overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfigOverride {
    /// Override enabled status
    pub enabled: Option<bool>,
    /// Override parameters
    pub parameters: Option<ProtocolParameters>,
    /// Override fee configuration
    pub fees: Option<FeeConfig>,
    /// Override rate limits
    pub rate_limits: Option<RateLimitConfig>,
    /// Override health configuration
    pub health: Option<HealthConfig>,
}

impl ProtocolConfig {
    /// Create a new protocol configuration with defaults
    pub fn new(protocol_id: ProtocolId, enabled: bool) -> Self {
        Self {
            protocol_id,
            enabled,
            version: "1.0.0".to_string(),
            parameters: ProtocolParameters::default(),
            fees: FeeConfig::default(),
            rate_limits: RateLimitConfig::default(),
            health: HealthConfig::default(),
            priority: 100,
            network_overrides: HashMap::new(),
        }
    }

    /// Apply network-specific overrides to this configuration
    pub fn apply_network_overrides(&mut self, network: &str) {
        if let Some(override_config) = self.network_overrides.get(network) {
            if let Some(enabled) = override_config.enabled {
                self.enabled = enabled;
            }
            if let Some(ref parameters) = override_config.parameters {
                self.parameters = parameters.clone();
            }
            if let Some(ref fees) = override_config.fees {
                self.fees = fees.clone();
            }
            if let Some(ref rate_limits) = override_config.rate_limits {
                self.rate_limits = rate_limits.clone();
            }
            if let Some(ref health) = override_config.health {
                self.health = health.clone();
            }
        }
    }

    /// Get effective configuration for a specific network
    pub fn for_network(&self, network: &str) -> Self {
        let mut config = self.clone();
        config.apply_network_overrides(network);
        config
    }

    /// Validate protocol configuration
    pub fn validate(&self) -> Result<(), Error> {
        // Validate slippage settings
        if self.parameters.default_slippage_bps > self.parameters.max_slippage_bps {
            return Err(Error::Config(
                "Default slippage cannot exceed maximum slippage".to_string(),
            ));
        }

        // Validate trade amounts
        if self.parameters.min_trade_amount >= self.parameters.max_trade_amount {
            return Err(Error::Config(
                "Minimum trade amount must be less than maximum".to_string(),
            ));
        }

        // Validate fee configuration
        if self.fees.min_fee > self.fees.max_fee {
            return Err(Error::Config(
                "Minimum fee cannot exceed maximum fee".to_string(),
            ));
        }

        // Validate protocol fee (should not exceed 100%)
        if self.fees.protocol_fee_bps > 10000 {
            return Err(Error::Config(
                "Protocol fee cannot exceed 100% (10000 basis points)".to_string(),
            ));
        }

        // Validate rate limits
        if self.rate_limits.max_concurrent_ops == 0 {
            return Err(Error::Config(
                "Maximum concurrent operations must be greater than 0".to_string(),
            ));
        }

        // Validate health configuration
        if self.health.failure_threshold == 0 {
            return Err(Error::Config(
                "Health failure threshold must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculate transaction fee based on configuration
    pub fn calculate_fee(&self, gas_used: u64) -> u64 {
        let calculated_fee = (gas_used as f64 * self.fees.gas_price_multiplier) as u64;
        let fee_with_default = calculated_fee.max(self.fees.default_tx_fee);
        let capped_fee = fee_with_default.min(self.fees.max_fee);
        capped_fee.max(self.fees.min_fee)
    }

    /// Get request timeout as Duration
    pub fn get_request_timeout(&self) -> Duration {
        Duration::from_secs(self.rate_limits.request_timeout_secs)
    }

    /// Get health check interval as Duration
    pub fn get_health_check_interval(&self) -> Duration {
        Duration::from_secs(self.health.check_interval_secs)
    }
}

/// Registry for managing protocol configurations
#[derive(Debug, Clone)]
pub struct ProtocolRegistry {
    /// Map of protocol ID to configuration
    protocols: HashMap<ProtocolId, ProtocolConfig>,
    /// Current active network for applying overrides
    active_network: Option<String>,
}

impl ProtocolRegistry {
    /// Create a new empty protocol registry
    pub fn new() -> Self {
        Self {
            protocols: HashMap::new(),
            active_network: None,
        }
    }

    /// Load protocol configurations from config files or environment
    pub fn load() -> Result<Self, Error> {
        let mut registry = Self::new();

        // Try to load from configuration files
        if let Err(_) = registry.load_from_config() {
            // If config loading fails, use defaults
            registry.load_defaults()?;
        }

        Ok(registry)
    }

    /// Load protocol configurations from config files
    fn load_from_config(&mut self) -> Result<(), Error> {
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let config_paths = vec![
            format!("{}/protocols", config_dir),
            "config/protocols".to_string(),
            "../config/protocols".to_string(),
            "../../config/protocols".to_string(),
        ];

        for config_path in &config_paths {
            if let Ok(settings) = ConfigLoader::builder()
                .add_source(File::with_name(config_path))
                .build()
            {
                self.load_protocols_from_settings(&settings)?;
                return Ok(());
            }
        }

        Err(Error::Config(
            "No protocol configuration file found".to_string(),
        ))
    }

    /// Load protocols from configuration settings
    fn load_protocols_from_settings(&mut self, settings: &ConfigLoader) -> Result<(), Error> {
        let protocols = [ProtocolId::Dex, ProtocolId::ClaimDrop, ProtocolId::Skip];

        for protocol in &protocols {
            let protocol_str = protocol.to_string();

            // Check if protocol section exists
            if let Ok(enabled) = settings.get::<bool>(&format!("{}.enabled", protocol_str)) {
                let mut config = ProtocolConfig::new(protocol.clone(), enabled);

                // Load basic settings
                if let Ok(version) = settings.get::<String>(&format!("{}.version", protocol_str)) {
                    config.version = version;
                }

                if let Ok(priority) = settings.get::<u32>(&format!("{}.priority", protocol_str)) {
                    config.priority = priority;
                }

                // Load parameters
                self.load_protocol_parameters(&mut config, settings, &protocol_str)?;

                // Load fee configuration
                self.load_fee_config(&mut config, settings, &protocol_str)?;

                // Load rate limits
                self.load_rate_limits(&mut config, settings, &protocol_str)?;

                // Load health configuration
                self.load_health_config(&mut config, settings, &protocol_str)?;

                // Validate and add to registry
                config.validate()?;
                self.protocols.insert(protocol.clone(), config);
            }
        }

        Ok(())
    }

    /// Load protocol parameters from settings
    fn load_protocol_parameters(
        &self,
        config: &mut ProtocolConfig,
        settings: &ConfigLoader,
        protocol: &str,
    ) -> Result<(), Error> {
        let params_prefix = format!("{}.parameters", protocol);

        if let Ok(max_slippage) =
            settings.get::<u16>(&format!("{}.max_slippage_bps", params_prefix))
        {
            config.parameters.max_slippage_bps = max_slippage;
        }

        if let Ok(default_slippage) =
            settings.get::<u16>(&format!("{}.default_slippage_bps", params_prefix))
        {
            config.parameters.default_slippage_bps = default_slippage;
        }

        if let Ok(min_amount) = settings.get::<u64>(&format!("{}.min_trade_amount", params_prefix))
        {
            config.parameters.min_trade_amount = min_amount;
        }

        if let Ok(max_amount) = settings.get::<u64>(&format!("{}.max_trade_amount", params_prefix))
        {
            config.parameters.max_trade_amount = max_amount;
        }

        Ok(())
    }

    /// Load fee configuration from settings
    fn load_fee_config(
        &self,
        config: &mut ProtocolConfig,
        settings: &ConfigLoader,
        protocol: &str,
    ) -> Result<(), Error> {
        let fees_prefix = format!("{}.fees", protocol);

        if let Ok(default_fee) = settings.get::<u64>(&format!("{}.default_tx_fee", fees_prefix)) {
            config.fees.default_tx_fee = default_fee;
        }

        if let Ok(multiplier) =
            settings.get::<f64>(&format!("{}.gas_price_multiplier", fees_prefix))
        {
            config.fees.gas_price_multiplier = multiplier;
        }

        if let Ok(max_fee) = settings.get::<u64>(&format!("{}.max_fee", fees_prefix)) {
            config.fees.max_fee = max_fee;
        }

        if let Ok(protocol_fee) = settings.get::<u16>(&format!("{}.protocol_fee_bps", fees_prefix))
        {
            config.fees.protocol_fee_bps = protocol_fee;
        }

        if let Ok(min_fee) = settings.get::<u64>(&format!("{}.min_fee", fees_prefix)) {
            config.fees.min_fee = min_fee;
        }

        Ok(())
    }

    /// Load rate limit configuration from settings
    fn load_rate_limits(
        &self,
        config: &mut ProtocolConfig,
        settings: &ConfigLoader,
        protocol: &str,
    ) -> Result<(), Error> {
        let limits_prefix = format!("{}.rate_limits", protocol);

        if let Ok(rpm) = settings.get::<u32>(&format!("{}.requests_per_minute", limits_prefix)) {
            config.rate_limits.requests_per_minute = rpm;
        }

        if let Ok(max_ops) = settings.get::<u32>(&format!("{}.max_concurrent_ops", limits_prefix)) {
            config.rate_limits.max_concurrent_ops = max_ops;
        }

        if let Ok(timeout) = settings.get::<u64>(&format!("{}.request_timeout_secs", limits_prefix))
        {
            config.rate_limits.request_timeout_secs = timeout;
        }

        if let Ok(burst) = settings.get::<u32>(&format!("{}.burst_capacity", limits_prefix)) {
            config.rate_limits.burst_capacity = burst;
        }

        if let Ok(cooldown) = settings.get::<u64>(&format!("{}.cooldown_secs", limits_prefix)) {
            config.rate_limits.cooldown_secs = cooldown;
        }

        Ok(())
    }

    /// Load health configuration from settings
    fn load_health_config(
        &self,
        config: &mut ProtocolConfig,
        settings: &ConfigLoader,
        protocol: &str,
    ) -> Result<(), Error> {
        let health_prefix = format!("{}.health", protocol);

        if let Ok(interval) = settings.get::<u64>(&format!("{}.check_interval_secs", health_prefix))
        {
            config.health.check_interval_secs = interval;
        }

        if let Ok(threshold) = settings.get::<u32>(&format!("{}.failure_threshold", health_prefix))
        {
            config.health.failure_threshold = threshold;
        }

        if let Ok(timeout) =
            settings.get::<u64>(&format!("{}.health_check_timeout_secs", health_prefix))
        {
            config.health.health_check_timeout_secs = timeout;
        }

        if let Ok(auto_recovery) = settings.get::<bool>(&format!("{}.auto_recovery", health_prefix))
        {
            config.health.auto_recovery = auto_recovery;
        }

        if let Ok(recovery_interval) =
            settings.get::<u64>(&format!("{}.recovery_interval_secs", health_prefix))
        {
            config.health.recovery_interval_secs = recovery_interval;
        }

        if let Ok(max_attempts) =
            settings.get::<u32>(&format!("{}.max_recovery_attempts", health_prefix))
        {
            config.health.max_recovery_attempts = max_attempts;
        }

        Ok(())
    }

    /// Load default protocol configurations
    fn load_defaults(&mut self) -> Result<(), Error> {
        // DEX Protocol - Core protocol, always enabled
        let mut dex_config = ProtocolConfig::new(ProtocolId::Dex, true);
        dex_config.priority = 1000; // Highest priority
        dex_config.parameters.max_slippage_bps = 1000; // 10%
        dex_config.parameters.default_slippage_bps = 50; // 0.5%
        self.protocols.insert(ProtocolId::Dex, dex_config);

        // ClaimDrop Protocol - Optional, enabled by default
        let mut claimdrop_config = ProtocolConfig::new(ProtocolId::ClaimDrop, true);
        claimdrop_config.priority = 500;
        claimdrop_config.rate_limits.requests_per_minute = 30; // Lower limits
        self.protocols
            .insert(ProtocolId::ClaimDrop, claimdrop_config);

        // Skip Protocol - Cross-chain, enabled by default
        let mut skip_config = ProtocolConfig::new(ProtocolId::Skip, true);
        skip_config.priority = 750;
        skip_config.rate_limits.request_timeout_secs = 60; // Longer timeout for cross-chain
        skip_config.health.check_interval_secs = 60; // More frequent health checks
        self.protocols.insert(ProtocolId::Skip, skip_config);

        // EVM Protocol - Ethereum Virtual Machine compatibility, enabled by default when feature is available
        #[cfg(feature = "evm")]
        {
            let mut evm_config = ProtocolConfig::new(ProtocolId::Evm, true);
            evm_config.priority = 500;
            evm_config.rate_limits.requests_per_minute = 50; // Moderate rate limiting for EVM calls
            evm_config.rate_limits.request_timeout_secs = 30; // Standard timeout
            evm_config.health.check_interval_secs = 30; // Health checks for RPC connectivity
            self.protocols.insert(ProtocolId::Evm, evm_config);
        }

        Ok(())
    }

    /// Set active network
    pub fn set_active_network(&mut self, network: &str) {
        self.active_network = Some(network.to_string());
    }

    /// Get protocol configuration
    pub fn get_protocol(&self, protocol_id: &ProtocolId) -> Option<ProtocolConfig> {
        self.protocols
            .get(protocol_id)
            .map(|config| match &self.active_network {
                Some(network) => config.for_network(network),
                None => config.clone(),
            })
    }

    /// Check if protocol is enabled
    pub fn is_protocol_enabled(&self, protocol_id: &ProtocolId) -> bool {
        self.get_protocol(protocol_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Get all enabled protocols, sorted by priority
    pub fn get_enabled_protocols(&self) -> Vec<(ProtocolId, ProtocolConfig)> {
        let mut enabled: Vec<_> = self
            .protocols
            .iter()
            .filter_map(|(id, config)| {
                let effective_config = match &self.active_network {
                    Some(network) => config.for_network(network),
                    None => config.clone(),
                };
                if effective_config.enabled {
                    Some((id.clone(), effective_config))
                } else {
                    None
                }
            })
            .collect();

        // Sort by priority (descending)
        enabled.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));
        enabled
    }

    /// Update protocol configuration
    pub fn update_protocol(
        &mut self,
        protocol_id: ProtocolId,
        config: ProtocolConfig,
    ) -> Result<(), Error> {
        config.validate()?;
        self.protocols.insert(protocol_id, config);
        Ok(())
    }

    /// Enable or disable a protocol
    pub fn set_protocol_enabled(
        &mut self,
        protocol_id: &ProtocolId,
        enabled: bool,
    ) -> Result<(), Error> {
        if let Some(config) = self.protocols.get_mut(protocol_id) {
            config.enabled = enabled;
            Ok(())
        } else {
            Err(Error::Config(format!(
                "Protocol {:?} not found",
                protocol_id
            )))
        }
    }

    /// Validate all protocol configurations
    pub fn validate_all(&self) -> Result<(), Error> {
        for (protocol_id, config) in &self.protocols {
            config.validate().map_err(|e| {
                Error::Config(format!(
                    "Protocol {:?} validation failed: {}",
                    protocol_id, e
                ))
            })?;
        }
        Ok(())
    }
}

impl Default for ProtocolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_config_validation() {
        let mut config = ProtocolConfig::new(ProtocolId::Dex, true);

        // Valid configuration should pass
        assert!(config.validate().is_ok());

        // Invalid slippage should fail
        config.parameters.default_slippage_bps = 2000;
        config.parameters.max_slippage_bps = 1000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_fee_calculation() {
        let config = ProtocolConfig::new(ProtocolId::Dex, true);

        let fee = config.calculate_fee(100000);
        assert!(fee >= config.fees.min_fee);
        assert!(fee <= config.fees.max_fee);
    }

    #[test]
    fn test_protocol_registry() {
        let mut registry = ProtocolRegistry::new();
        registry.load_defaults().unwrap();

        assert!(registry.is_protocol_enabled(&ProtocolId::Dex));
        assert!(registry.get_protocol(&ProtocolId::Dex).is_some());

        let enabled_protocols = registry.get_enabled_protocols();
        assert!(!enabled_protocols.is_empty());
    }

    #[test]
    fn test_network_overrides() {
        let mut config = ProtocolConfig::new(ProtocolId::Dex, true);

        let override_config = ProtocolConfigOverride {
            enabled: Some(false),
            parameters: None,
            fees: None,
            rate_limits: None,
            health: None,
        };

        config
            .network_overrides
            .insert("testnet".to_string(), override_config);

        let testnet_config = config.for_network("testnet");
        assert!(!testnet_config.enabled);

        let mainnet_config = config.for_network("mainnet");
        assert!(mainnet_config.enabled);
    }
}

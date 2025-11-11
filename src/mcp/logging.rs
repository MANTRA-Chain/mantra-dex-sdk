//! Comprehensive Logging and Tracing Infrastructure for MCP Server
//!
//! This module provides a robust logging and tracing system specifically designed for the
//! Mantra DEX MCP Server. It includes:
//!
//! - Configurable logging levels and formats
//! - Structured logging with context
//! - Performance monitoring and metrics
//! - Multiple output destinations (stdout, stderr, file)
//! - Request/response tracing
//! - Error tracking and categorization
//! - Environment-based configuration

use std::fs::OpenOptions;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{Level, Span};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

use crate::mcp::server::McpServerError;

/// Logging configuration for the MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Logging level (TRACE, DEBUG, INFO, WARN, ERROR)
    pub level: LogLevel,
    /// Output format (JSON, Compact, Pretty)
    pub format: LogFormat,
    /// Whether to enable colored output
    pub enable_colors: bool,
    /// Whether to include timestamps
    pub include_timestamps: bool,
    /// Whether to include thread IDs
    pub include_thread_ids: bool,
    /// Whether to include span information
    pub include_spans: bool,
    /// Whether to include file/line information
    pub include_file_line: bool,
    /// Target for log output (stdout, stderr, file)
    pub output_target: LogTarget,
    /// File path for file output
    pub log_file_path: Option<PathBuf>,
    /// Maximum log file size in MB
    pub max_file_size_mb: u64,
    /// Number of log files to rotate
    pub max_log_files: u32,
    /// Whether to enable request/response tracing
    pub enable_request_tracing: bool,
    /// Whether to enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Custom environment filter
    pub custom_filter: Option<String>,
    /// Whether to enable metrics collection
    pub enable_metrics: bool,
    /// Log sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            format: LogFormat::Compact,
            enable_colors: true,
            include_timestamps: true,
            include_thread_ids: false,
            include_spans: true,
            include_file_line: false,
            output_target: LogTarget::Stderr,
            log_file_path: None,
            max_file_size_mb: 100,
            max_log_files: 5,
            enable_request_tracing: true,
            enable_performance_monitoring: true,
            custom_filter: None,
            enable_metrics: true,
            sampling_rate: 1.0,
        }
    }
}

impl LoggingConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.sampling_rate < 0.0 || self.sampling_rate > 1.0 {
            return Err("Sampling rate must be between 0.0 and 1.0".to_string());
        }

        if self.max_file_size_mb == 0 {
            return Err("Max file size must be greater than 0".to_string());
        }

        if self.max_log_files == 0 {
            return Err("Max log files must be greater than 0".to_string());
        }

        if let Some(path) = &self.log_file_path {
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    return Err(format!("Log file directory does not exist: {:?}", parent));
                }
            }
        }

        Ok(())
    }
}

/// Supported logging levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => Level::TRACE,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Info => Level::INFO,
            LogLevel::Warn => Level::WARN,
            LogLevel::Error => Level::ERROR,
        }
    }
}

/// Supported log output formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    /// Structured JSON format
    Json,
    /// Compact human-readable format
    Compact,
    /// Pretty human-readable format with indentation
    Pretty,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(LogFormat::Json),
            "compact" => Ok(LogFormat::Compact),
            "pretty" => Ok(LogFormat::Pretty),
            _ => Err(format!("Invalid log format: {}", s)),
        }
    }
}

/// Supported log output targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogTarget {
    /// Standard output
    Stdout,
    /// Standard error
    Stderr,
    /// File output
    File,
    /// Both stdout and file
    Both,
}

impl std::str::FromStr for LogTarget {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stdout" => Ok(LogTarget::Stdout),
            "stderr" => Ok(LogTarget::Stderr),
            "file" => Ok(LogTarget::File),
            "both" => Ok(LogTarget::Both),
            _ => Err(format!("Invalid log target: {}", s)),
        }
    }
}

/// Logging metrics and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingMetrics {
    /// Total log messages processed
    pub total_messages: u64,
    /// Messages by level
    pub messages_by_level: std::collections::HashMap<String, u64>,
    /// Error count
    pub error_count: u64,
    /// Warning count
    pub warning_count: u64,
    /// Request count
    pub request_count: u64,
    /// Response count
    pub response_count: u64,
    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Logging start time (stored as seconds since UNIX epoch)
    #[serde(with = "unix_timestamp")]
    pub start_time: SystemTime,
    /// Dropped messages due to sampling
    pub dropped_messages: u64,
}

impl Default for LoggingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl LoggingMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self {
            total_messages: 0,
            messages_by_level: std::collections::HashMap::new(),
            error_count: 0,
            warning_count: 0,
            request_count: 0,
            response_count: 0,
            avg_processing_time_ms: 0.0,
            start_time: SystemTime::now(),
            dropped_messages: 0,
        }
    }

    /// Record a log message
    pub fn record_message(&mut self, level: &str) {
        self.total_messages += 1;
        *self.messages_by_level.entry(level.to_string()).or_insert(0) += 1;

        match level.to_lowercase().as_str() {
            "error" => self.error_count += 1,
            "warn" => self.warning_count += 1,
            _ => {}
        }
    }

    /// Record a request
    pub fn record_request(&mut self) {
        self.request_count += 1;
    }

    /// Record a response
    pub fn record_response(&mut self) {
        self.response_count += 1;
    }

    /// Record a dropped message
    pub fn record_dropped(&mut self) {
        self.dropped_messages += 1;
    }

    /// Get uptime duration
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed().unwrap_or_default()
    }

    /// Convert metrics to JSON
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "total_messages": self.total_messages,
            "messages_by_level": self.messages_by_level,
            "error_count": self.error_count,
            "warning_count": self.warning_count,
            "request_count": self.request_count,
            "response_count": self.response_count,
            "avg_processing_time_ms": self.avg_processing_time_ms,
            "uptime_secs": self.uptime().as_secs(),
            "dropped_messages": self.dropped_messages
        })
    }
}

/// MCP-specific logger with sampling and metrics
pub struct McpLogger {
    /// Configuration
    config: LoggingConfig,
    /// Metrics collection
    metrics: Arc<RwLock<LoggingMetrics>>,
    /// Whether the logger is initialized
    initialized: bool,
}

impl McpLogger {
    /// Create a new MCP logger
    pub fn new(config: LoggingConfig) -> Result<Self, String> {
        config.validate()?;

        Ok(Self {
            config,
            metrics: Arc::new(RwLock::new(LoggingMetrics::new())),
            initialized: false,
        })
    }

    /// Initialize the logger
    pub fn initialize(&mut self) -> Result<(), String> {
        if !self.initialized {
            setup_logging(&self.config)?;
            self.initialized = true;
        }
        Ok(())
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> LoggingMetrics {
        self.metrics.read().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        *metrics = LoggingMetrics::new();
    }

    /// Log a message with context
    pub async fn log_with_context(&self, level: LogLevel, message: &str, context: Option<Value>) {
        if !self.should_log() {
            let mut metrics = self.metrics.write().await;
            metrics.record_dropped();
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.record_message(&format!("{:?}", level).to_lowercase());
        drop(metrics);

        // Use the tracing macros directly based on level
        match level {
            LogLevel::Trace => {
                if let Some(ctx) = context {
                    tracing::trace!(context = ?ctx, "{}", message);
                } else {
                    tracing::trace!("{}", message);
                }
            }
            LogLevel::Debug => {
                if let Some(ctx) = context {
                    tracing::debug!(context = ?ctx, "{}", message);
                } else {
                    tracing::debug!("{}", message);
                }
            }
            LogLevel::Info => {
                if let Some(ctx) = context {
                    tracing::info!(context = ?ctx, "{}", message);
                } else {
                    tracing::info!("{}", message);
                }
            }
            LogLevel::Warn => {
                if let Some(ctx) = context {
                    tracing::warn!(context = ?ctx, "{}", message);
                } else {
                    tracing::warn!("{}", message);
                }
            }
            LogLevel::Error => {
                if let Some(ctx) = context {
                    tracing::error!(context = ?ctx, "{}", message);
                } else {
                    tracing::error!("{}", message);
                }
            }
        }
    }

    /// Log a request and return a request span for tracking
    pub async fn log_request(&self, method: &str, request: &Value) -> RequestSpan {
        let mut metrics = self.metrics.write().await;
        metrics.record_request();
        drop(metrics);

        let span = tracing::info_span!("mcp_request", method = method);
        let start_time = Instant::now();

        // Log the request
        tracing::info!(
            parent: &span,
            request = ?request,
            "MCP request received: {}",
            method
        );

        RequestSpan {
            span,
            start_time,
            method: method.to_string(),
        }
    }

    /// Log a response with timing information
    pub async fn log_response(
        &self,
        request_span: RequestSpan,
        response: &Value,
        error: Option<&McpServerError>,
    ) {
        let elapsed = request_span.start_time.elapsed();
        let mut metrics = self.metrics.write().await;
        metrics.record_response();
        drop(metrics);

        let _enter = request_span.span.enter();

        if let Some(err) = error {
            tracing::error!(
                method = request_span.method,
                response = ?response,
                error = ?err,
                duration_ms = elapsed.as_millis(),
                "MCP request failed: {}",
                request_span.method
            );
        } else {
            tracing::info!(
                method = request_span.method,
                response = ?response,
                duration_ms = elapsed.as_millis(),
                "MCP request completed: {}",
                request_span.method
            );
        }
    }

    /// Log an error with detailed context
    pub async fn log_error(&self, error: &McpServerError, context: Option<Value>, operation: &str) {
        let mut metrics = self.metrics.write().await;
        metrics.record_message("error");
        drop(metrics);

        if let Some(ctx) = context {
            tracing::error!(
                error = ?error,
                context = ?ctx,
                operation = operation,
                "MCP server error in operation: {}",
                operation
            );
        } else {
            tracing::error!(
                error = ?error,
                operation = operation,
                "MCP server error in operation: {}",
                operation
            );
        }
    }

    /// Check if a message should be logged based on sampling rate
    fn should_log(&self) -> bool {
        if self.config.sampling_rate >= 1.0 {
            return true;
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen::<f64>() < self.config.sampling_rate
    }
}

/// Request span for tracking request timing
#[derive(Debug)]
pub struct RequestSpan {
    span: Span,
    start_time: Instant,
    method: String,
}

/// Set up logging based on configuration
pub fn setup_logging(config: &LoggingConfig) -> Result<(), String> {
    configure_tracing_subscriber(config)
}

/// Configure the tracing subscriber based on configuration
pub fn configure_tracing_subscriber(config: &LoggingConfig) -> Result<(), String> {
    let env_filter = create_env_filter(config)?;

    // Configure the subscriber based on output target and format
    match config.output_target {
        LogTarget::Stdout => {
            configure_stdout_subscriber(config, env_filter)?;
        }
        LogTarget::Stderr => {
            configure_stderr_subscriber(config, env_filter)?;
        }
        LogTarget::File => {
            configure_file_subscriber(config, env_filter)?;
        }
        LogTarget::Both => {
            // For now, use stderr as the primary target
            // File output would require more complex setup with tee functionality
            configure_stderr_subscriber(config, env_filter)?;
        }
    }

    Ok(())
}

/// Configure stdout subscriber
fn configure_stdout_subscriber(
    config: &LoggingConfig,
    env_filter: EnvFilter,
) -> Result<(), String> {
    match config.format {
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(std::io::stdout)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line),
                )
                .init();
        }
        LogFormat::Compact => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .compact()
                        .with_writer(std::io::stdout)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line)
                        .with_ansi(config.enable_colors),
                )
                .init();
        }
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .pretty()
                        .with_writer(std::io::stdout)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line)
                        .with_ansi(config.enable_colors),
                )
                .init();
        }
    }
    Ok(())
}

/// Configure stderr subscriber
fn configure_stderr_subscriber(
    config: &LoggingConfig,
    env_filter: EnvFilter,
) -> Result<(), String> {
    match config.format {
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(std::io::stderr)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line),
                )
                .init();
        }
        LogFormat::Compact => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .compact()
                        .with_writer(std::io::stderr)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line)
                        .with_ansi(config.enable_colors),
                )
                .init();
        }
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .pretty()
                        .with_writer(std::io::stderr)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line)
                        .with_ansi(config.enable_colors),
                )
                .init();
        }
    }
    Ok(())
}

/// Configure file subscriber
fn configure_file_subscriber(config: &LoggingConfig, env_filter: EnvFilter) -> Result<(), String> {
    let log_file = config
        .log_file_path
        .as_ref()
        .ok_or("File output requires log_file_path to be set")?;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .map_err(|e| format!("Failed to open log file: {}", e))?;

    match config.format {
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .with_writer(file)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line),
                )
                .init();
        }
        LogFormat::Compact => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .compact()
                        .with_writer(file)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line)
                        .with_ansi(false), // Disable colors for file output
                )
                .init();
        }
        LogFormat::Pretty => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    fmt::layer()
                        .pretty()
                        .with_writer(file)
                        .with_span_events(if config.include_spans {
                            FmtSpan::CLOSE
                        } else {
                            FmtSpan::NONE
                        })
                        .with_thread_ids(config.include_thread_ids)
                        .with_file(config.include_file_line)
                        .with_line_number(config.include_file_line)
                        .with_ansi(false), // Disable colors for file output
                )
                .init();
        }
    }
    Ok(())
}

/// Create environment filter based on configuration
fn create_env_filter(config: &LoggingConfig) -> Result<EnvFilter, String> {
    let filter_str = if let Some(ref custom) = config.custom_filter {
        custom.clone()
    } else {
        get_mcp_specific_filter(config.level)
    };

    EnvFilter::try_new(&filter_str)
        .map_err(|e| format!("Failed to create environment filter: {}", e))
}

/// Get default log filter for MCP server
pub fn get_default_log_filter() -> String {
    "mantra_dex_sdk::mcp=info,cosmrs=warn,reqwest=warn".to_string()
}

/// Get MCP-specific log filter based on level
pub fn get_mcp_specific_filter(level: LogLevel) -> String {
    let level_str = match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    };

    format!(
        "mantra_dex_sdk::mcp={},cosmrs=warn,reqwest=warn,tokio=warn,hyper=warn",
        level_str
    )
}

/// Custom serde module for serializing/deserializing SystemTime as unix timestamp
mod unix_timestamp {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, LogLevel::Info);
        assert_eq!(config.format, LogFormat::Compact);
        assert!(config.enable_colors);
        assert!(config.include_timestamps);
    }

    #[test]
    fn test_logging_config_validation() {
        let mut config = LoggingConfig::default();
        assert!(config.validate().is_ok());

        config.sampling_rate = 1.5;
        assert!(config.validate().is_err());

        config.sampling_rate = 0.5;
        config.max_file_size_mb = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_log_level_parsing() {
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert!("invalid".parse::<LogLevel>().is_err());
    }

    #[test]
    fn test_log_format_parsing() {
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("COMPACT".parse::<LogFormat>().unwrap(), LogFormat::Compact);
        assert!("invalid".parse::<LogFormat>().is_err());
    }

    #[test]
    fn test_log_target_parsing() {
        assert_eq!("stdout".parse::<LogTarget>().unwrap(), LogTarget::Stdout);
        assert_eq!("FILE".parse::<LogTarget>().unwrap(), LogTarget::File);
        assert!("invalid".parse::<LogTarget>().is_err());
    }

    #[test]
    fn test_logging_metrics() {
        let mut metrics = LoggingMetrics::new();
        assert_eq!(metrics.total_messages, 0);

        metrics.record_message("info");
        assert_eq!(metrics.total_messages, 1);
        assert_eq!(metrics.messages_by_level["info"], 1);

        metrics.record_message("error");
        assert_eq!(metrics.error_count, 1);

        let json = metrics.to_json();
        assert!(json.is_object());
    }

    #[test]
    fn test_mcp_specific_filter() {
        let filter = get_mcp_specific_filter(LogLevel::Debug);
        assert!(filter.contains("mantra_dex_sdk::mcp=debug"));
        assert!(filter.contains("cosmrs=warn"));
    }

    #[tokio::test]
    async fn test_mcp_logger_creation() {
        let config = LoggingConfig::default();
        let logger = McpLogger::new(config).unwrap();
        assert!(!logger.initialized);
    }

    #[tokio::test]
    async fn test_mcp_logger_metrics() {
        let config = LoggingConfig::default();
        let logger = McpLogger::new(config).unwrap();

        let metrics = logger.get_metrics().await;
        assert_eq!(metrics.total_messages, 0);
    }
}

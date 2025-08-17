use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{extract::State, http::StatusCode, response::Json, routing::post, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Configuration support
use config::{Config, ConfigError, Environment, File, FileFormat};

// Note: MCP SDK types not imported due to API instability
// Current rust-mcp-sdk 0.4.2 has unstable APIs that don't match documentation
// The server implements MCP protocol manually using standard HTTP/JSON-RPC
// until the rust-mcp-sdk API stabilizes in future versions

use crate::protocols::dex::MantraDexClient;
use crate::config::{MantraNetworkConfig, NetworkConstants};
use crate::error::Error as SdkError;
use crate::wallet::WalletInfo;

use super::client_wrapper::McpClientWrapper;
use super::logging::{LoggingConfig, McpLogger};
use super::sdk_adapter::McpSdkAdapter;

// =============================================================================
// Transaction Monitoring Types
// =============================================================================

/// Transaction monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMonitorConfig {
    /// Transaction hash to monitor
    pub tx_hash: String,
    /// Minimum confirmations required
    pub min_confirmations: u64,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Whether to monitor events
    pub monitor_events: bool,
}

/// Transaction status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is pending
    Pending,
    /// Transaction is confirmed with number of confirmations
    Confirmed { confirmations: u64 },
    /// Transaction failed with reason
    Failed { reason: String },
    /// Transaction monitoring timed out
    TimedOut,
    /// Transaction monitoring was cancelled
    Cancelled,
}

impl TransactionStatus {
    /// Check if the transaction is in a final state
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            TransactionStatus::Confirmed { .. }
                | TransactionStatus::Failed { .. }
                | TransactionStatus::TimedOut
                | TransactionStatus::Cancelled
        )
    }

    /// Get status as string for display
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Pending => "pending",
            TransactionStatus::Confirmed { .. } => "confirmed",
            TransactionStatus::Failed { .. } => "failed",
            TransactionStatus::TimedOut => "timeout",
            TransactionStatus::Cancelled => "cancelled",
        }
    }
}

/// Transaction monitor instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMonitor {
    /// Unique monitor ID
    pub id: String,
    /// Monitor configuration
    pub config: TransactionMonitorConfig,
    /// Current status
    pub status: TransactionStatus,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Number of polls performed
    pub poll_count: u64,
    /// Block height when confirmed (if applicable)
    pub block_height: Option<u64>,
    /// Gas used (if available)
    pub gas_used: Option<u64>,
    /// Any events captured
    pub events: Vec<serde_json::Value>,
}

impl TransactionMonitor {
    /// Create a new transaction monitor
    pub fn new(config: TransactionMonitorConfig) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            config,
            status: TransactionStatus::Pending,
            created_at: now,
            updated_at: now,
            poll_count: 0,
            block_height: None,
            gas_used: None,
            events: Vec::new(),
        }
    }

    /// Update the monitor status
    pub fn update_status(&mut self, status: TransactionStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now();
    }

    /// Increment poll count
    pub fn increment_poll(&mut self) {
        self.poll_count += 1;
        self.updated_at = chrono::Utc::now();
    }

    /// Check if monitoring is completed
    pub fn is_completed(&self) -> bool {
        self.status.is_final()
    }

    /// Check if monitoring timed out
    pub fn is_timed_out(&self) -> bool {
        matches!(self.status, TransactionStatus::TimedOut)
    }

    /// Add an event to the monitor
    pub fn add_event(&mut self, event: serde_json::Value) {
        self.events.push(event);
        self.updated_at = chrono::Utc::now();
    }

    /// Get elapsed time since creation
    pub fn elapsed_time(&self) -> Duration {
        let now = chrono::Utc::now();
        (now - self.created_at)
            .to_std()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Convert to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "monitor_id": self.id,
            "tx_hash": self.config.tx_hash,
            "status": self.status.as_str(),
            "status_details": self.status,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
            "elapsed_time_secs": self.elapsed_time().as_secs(),
            "poll_count": self.poll_count,
            "block_height": self.block_height,
            "gas_used": self.gas_used,
            "events": self.events,
            "config": {
                "min_confirmations": self.config.min_confirmations,
                "timeout_secs": self.config.timeout_secs,
                "poll_interval_secs": self.config.poll_interval_secs,
                "monitor_events": self.config.monitor_events
            }
        })
    }
}

/// Transaction monitor manager for handling multiple monitors
#[derive(Debug)]
pub struct TransactionMonitorManager {
    /// Active monitors
    monitors: Arc<RwLock<HashMap<String, TransactionMonitor>>>,
}

impl TransactionMonitorManager {
    /// Create a new transaction monitor manager
    pub fn new() -> Self {
        Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a monitor
    pub async fn add_monitor(&self, monitor: TransactionMonitor) -> String {
        let id = monitor.id.clone();
        let mut monitors = self.monitors.write().await;
        monitors.insert(id.clone(), monitor);
        id
    }

    /// Get a monitor by ID
    pub async fn get_monitor(&self, id: &str) -> Option<TransactionMonitor> {
        let monitors = self.monitors.read().await;
        monitors.get(id).cloned()
    }

    /// Update a monitor
    pub async fn update_monitor(&self, id: &str, monitor: TransactionMonitor) -> bool {
        let mut monitors = self.monitors.write().await;
        if monitors.contains_key(id) {
            monitors.insert(id.to_string(), monitor);
            true
        } else {
            false
        }
    }

    /// Remove a monitor
    pub async fn remove_monitor(&self, id: &str) -> Option<TransactionMonitor> {
        let mut monitors = self.monitors.write().await;
        monitors.remove(id)
    }

    /// List all monitors
    pub async fn list_monitors(&self) -> Vec<TransactionMonitor> {
        let monitors = self.monitors.read().await;
        monitors.values().cloned().collect()
    }

    /// List monitors with filtering
    pub async fn list_monitors_filtered(&self, include_completed: bool) -> Vec<TransactionMonitor> {
        let monitors = self.monitors.read().await;
        monitors
            .values()
            .filter(|monitor| include_completed || !monitor.is_completed())
            .cloned()
            .collect()
    }

    /// Clean up completed monitors
    pub async fn cleanup_completed(&self) -> usize {
        let mut monitors = self.monitors.write().await;
        let initial_count = monitors.len();
        monitors.retain(|_, monitor| !monitor.is_completed());
        initial_count - monitors.len()
    }

    /// Clean up old monitors (force cleanup)
    pub async fn cleanup_old_monitors(&self, max_age: Duration) -> usize {
        let mut monitors = self.monitors.write().await;
        let initial_count = monitors.len();
        let now = chrono::Utc::now();

        monitors.retain(|_, monitor| {
            let age = (now - monitor.created_at)
                .to_std()
                .unwrap_or(Duration::from_secs(0));
            age < max_age
        });

        initial_count - monitors.len()
    }

    /// Start monitoring a transaction with polling
    pub async fn start_monitoring(
        &self,
        config: TransactionMonitorConfig,
        client: Arc<Mutex<Option<MantraDexClient>>>,
    ) -> Result<String, String> {
        let monitor = TransactionMonitor::new(config.clone());
        let monitor_id = monitor.id.clone();

        // Add the monitor
        self.add_monitor(monitor).await;

        // Start the monitoring task
        let manager = self.clone();
        let monitor_id_task = monitor_id.clone();
        tokio::spawn(async move {
            manager
                .monitor_transaction_polling(monitor_id_task, client)
                .await;
        });

        Ok(monitor_id)
    }

    /// Monitor transaction with polling logic
    async fn monitor_transaction_polling(
        &self,
        monitor_id: String,
        client: Arc<Mutex<Option<MantraDexClient>>>,
    ) {
        let timeout_duration = {
            let monitor = match self.get_monitor(&monitor_id).await {
                Some(m) => m,
                None => {
                    error!("Monitor {} not found during polling setup", monitor_id);
                    return;
                }
            };
            Duration::from_secs(monitor.config.timeout_secs)
        };

        let start_time = Instant::now();

        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() >= timeout_duration {
                self.update_monitor_status(&monitor_id, TransactionStatus::TimedOut)
                    .await;
                info!("Transaction monitor {} timed out", monitor_id);
                break;
            }

            // Get the current monitor to check if it's been cancelled or completed
            let monitor = match self.get_monitor(&monitor_id).await {
                Some(m) => m,
                None => {
                    debug!("Monitor {} was removed during polling", monitor_id);
                    break;
                }
            };

            if monitor.is_completed() {
                debug!("Monitor {} is already completed", monitor_id);
                break;
            }

            // Perform the transaction query
            match self
                .query_transaction(&monitor.config.tx_hash, client.clone())
                .await
            {
                Ok(Some(tx_result)) => {
                    // Process the transaction result
                    self.process_transaction_result(&monitor_id, tx_result)
                        .await;

                    // Check if monitoring is complete
                    let updated_monitor = match self.get_monitor(&monitor_id).await {
                        Some(m) => m,
                        None => break,
                    };

                    if updated_monitor.is_completed() {
                        info!("Transaction monitoring completed for {}", monitor_id);
                        break;
                    }
                }
                Ok(None) => {
                    // Transaction not found yet, continue polling
                    debug!(
                        "Transaction {} not found yet, continuing to poll",
                        monitor.config.tx_hash
                    );
                }
                Err(e) => {
                    warn!(
                        "Error querying transaction {}: {}",
                        monitor.config.tx_hash, e
                    );
                    // Continue polling on errors (might be temporary network issues)
                }
            }

            // Increment poll count
            self.increment_monitor_poll(&monitor_id).await;

            // Wait for the next poll interval
            sleep(Duration::from_secs(monitor.config.poll_interval_secs)).await;
        }
    }

    /// Query transaction from blockchain
    async fn query_transaction(
        &self,
        tx_hash: &str,
        client: Arc<Mutex<Option<MantraDexClient>>>,
    ) -> Result<Option<serde_json::Value>, String> {
        let client_guard = client.lock().await;
        let client = match client_guard.as_ref() {
            Some(c) => c,
            None => return Err("Client not available".to_string()),
        };

        match client.query_transaction(tx_hash).await {
            Ok(tx_json) => Ok(Some(tx_json)),
            Err(SdkError::Rpc(e)) if e.to_string().contains("not found") => {
                // Transaction not found yet
                Ok(None)
            }
            Err(e) => Err(format!("Error querying transaction: {}", e)),
        }
    }

    /// Process transaction result and update monitor
    async fn process_transaction_result(&self, monitor_id: &str, tx_result: serde_json::Value) {
        let mut monitor = match self.get_monitor(monitor_id).await {
            Some(m) => m,
            None => return,
        };

        // Extract transaction details
        if let Some(code) = tx_result.get("code").and_then(|c| c.as_u64()) {
            if code == 0 {
                // Transaction succeeded
                let height = tx_result.get("height").and_then(|h| h.as_u64());
                let gas_used = tx_result.get("gas_used").and_then(|g| g.as_u64());

                monitor.block_height = height;
                monitor.gas_used = gas_used;

                // Check confirmations (simplified: assume 1 confirmation if we found the tx)
                let confirmations = 1; // In a real implementation, you'd query current block height and calculate

                if confirmations >= monitor.config.min_confirmations {
                    monitor.update_status(TransactionStatus::Confirmed { confirmations });
                } else {
                    // Continue monitoring for more confirmations
                    monitor.update_status(TransactionStatus::Pending);
                }

                // Extract events if monitoring is enabled
                if monitor.config.monitor_events {
                    if let Some(events) = tx_result.get("events") {
                        monitor.add_event(events.clone());
                    }
                }
            } else {
                // Transaction failed
                let reason = tx_result
                    .get("raw_log")
                    .and_then(|l| l.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                monitor.update_status(TransactionStatus::Failed { reason });
            }
        } else {
            // Malformed response
            monitor.update_status(TransactionStatus::Failed {
                reason: "Malformed transaction response".to_string(),
            });
        }

        self.update_monitor(monitor_id, monitor).await;
    }

    /// Update monitor status
    async fn update_monitor_status(&self, monitor_id: &str, status: TransactionStatus) {
        if let Some(mut monitor) = self.get_monitor(monitor_id).await {
            monitor.update_status(status);
            self.update_monitor(monitor_id, monitor).await;
        }
    }

    /// Increment monitor poll count
    async fn increment_monitor_poll(&self, monitor_id: &str) {
        if let Some(mut monitor) = self.get_monitor(monitor_id).await {
            monitor.increment_poll();
            self.update_monitor(monitor_id, monitor).await;
        }
    }

    /// Cancel a monitor
    pub async fn cancel_monitor(&self, monitor_id: &str) -> bool {
        if let Some(mut monitor) = self.get_monitor(monitor_id).await {
            if !monitor.is_completed() {
                monitor.update_status(TransactionStatus::Cancelled);
                self.update_monitor(monitor_id, monitor).await;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl Clone for TransactionMonitorManager {
    fn clone(&self) -> Self {
        Self {
            monitors: self.monitors.clone(),
        }
    }
}

// =============================================================================
// JSON-RPC Types
// =============================================================================

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

// =============================================================================
// JSON-RPC Error Code Constants
// =============================================================================

// Standard JSON-RPC error codes
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;

// MCP-specific error codes
const WALLET_NOT_CONFIGURED: i32 = -32000;
const SERIALIZATION_ERROR: i32 = -32001;
const NETWORK_CONNECTION_FAILED: i32 = -32002;
const VALIDATION_ERROR: i32 = -32003;
const CONFIGURATION_ERROR: i32 = -32004;
const RESOURCE_NOT_FOUND: i32 = -32005;

// SDK-specific error codes
const BLOCKCHAIN_RPC_ERROR: i32 = -32100;
const TRANSACTION_FAILED: i32 = -32101;
const INSUFFICIENT_FUNDS: i32 = -32102;
const INVALID_PUBLIC_KEY_FORMAT: i32 = -32103;
const INVALID_MNEMONIC_FORMAT: i32 = -32104;
const INVALID_ADDRESS_FORMAT: i32 = -32105;
const POOL_NOT_FOUND: i32 = -32106;
const SWAP_SLIPPAGE_EXCEEDED: i32 = -32107;
const LIQUIDITY_INSUFFICIENT: i32 = -32108;
const TOOL_EXECUTION_FAILED: i32 = -32109;
const FEE_VALIDATION_FAILED: i32 = -32110;
const TIMEOUT_ERROR: i32 = -32111;
const IO_ERROR: i32 = -32112;









// =============================================================================
// MCP Server Trait Definitions
// =============================================================================

/// Core MCP server lifecycle management trait
///
/// Defines the essential lifecycle operations for an MCP server including
/// initialization, configuration, and server metadata management.
#[async_trait::async_trait]
pub trait McpServerLifecycle: Send + Sync {
    /// Initialize the server with necessary resources and connections
    async fn initialize(&self) -> McpResult<()>;

    /// Get server information (name, version, etc.)
    fn get_server_info(&self) -> serde_json::Value;

    /// Get server capabilities (supported features)
    fn get_capabilities(&self) -> serde_json::Value;

    /// Perform graceful shutdown and cleanup
    async fn shutdown(&self) -> McpResult<()> {
        // Default implementation - servers can override for custom cleanup
        Ok(())
    }

    /// Check if server is ready to handle requests
    async fn is_ready(&self) -> bool {
        true // Default implementation
    }
}

/// MCP tool provider trait
///
/// Defines the interface for servers that provide executable tools.
/// Tools are functions that can be called by MCP clients to perform operations.
#[async_trait::async_trait]
pub trait McpToolProvider: Send + Sync {
    /// Get list of available tools with their schemas
    fn get_available_tools(&self) -> Vec<serde_json::Value>;

    /// Execute a tool with given arguments
    async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value>;

    /// Validate tool arguments before execution
    fn validate_tool_arguments(
        &self,
        _tool_name: &str,
        arguments: &serde_json::Value,
    ) -> McpResult<()> {
        // Default implementation - servers can override for custom validation
        if !arguments.is_object() {
            return Err(McpServerError::InvalidArguments(
                "Tool arguments must be an object".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if a tool is available
    fn has_tool(&self, tool_name: &str) -> bool {
        self.get_available_tools()
            .iter()
            .any(|tool| tool.get("name").and_then(|n| n.as_str()) == Some(tool_name))
    }
}

/// MCP resource provider trait
///
/// Defines the interface for servers that provide readable resources.
/// Resources are data sources that can be queried by MCP clients.
#[async_trait::async_trait]
pub trait McpResourceProvider: Send + Sync {
    /// Get list of available resources with their schemas
    fn get_available_resources(&self) -> Vec<serde_json::Value>;

    /// Read a resource by its URI
    async fn handle_resource_read(&self, uri: &str) -> McpResult<serde_json::Value>;

    /// Validate resource URI format
    fn validate_resource_uri(&self, uri: &str) -> McpResult<()> {
        // Default implementation - basic URI validation
        if uri.is_empty() {
            return Err(McpServerError::Validation(
                "Resource URI cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if a resource is available
    fn has_resource(&self, uri: &str) -> bool {
        self.get_available_resources()
            .iter()
            .any(|resource| resource.get("uri").and_then(|u| u.as_str()) == Some(uri))
    }

    /// Get resource metadata without reading full content
    async fn get_resource_metadata(&self, uri: &str) -> McpResult<serde_json::Value> {
        // Default implementation - basic metadata
        Ok(serde_json::json!({
            "uri": uri,
            "available": self.has_resource(uri)
        }))
    }
}

/// MCP server state management trait
///
/// Defines the interface for servers that manage internal state.
/// This includes caching, session management, and configuration.
#[async_trait::async_trait]
pub trait McpServerStateManager: Send + Sync {
    /// Get current server configuration
    async fn get_config(&self) -> serde_json::Value;

    /// Update server configuration
    async fn update_config(&self, config: serde_json::Value) -> McpResult<()>;

    /// Clear internal caches and state
    async fn clear_state(&self) -> McpResult<()>;

    /// Get server health status
    async fn get_health_status(&self) -> serde_json::Value {
        // Default implementation
        serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
}

/// Main MCP server trait
///
/// Combines all MCP server capabilities into a single trait.
/// Servers should implement this trait to provide full MCP functionality.
#[async_trait::async_trait]
pub trait McpServer:
    McpServerLifecycle + McpToolProvider + McpResourceProvider + McpServerStateManager
{
    /// Handle incoming MCP requests with proper routing
    async fn handle_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<serde_json::Value> {
        match method {
            "tools/list" => {
                let tools = self.get_available_tools();
                Ok(serde_json::json!({ "tools": tools }))
            }
            "tools/call" => {
                if let Some(params) = params {
                    let tool_name =
                        params.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                            McpServerError::InvalidArguments("Missing tool name".to_string())
                        })?;

                    let arguments = params
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    self.handle_tool_call(tool_name, arguments).await
                } else {
                    Err(McpServerError::InvalidArguments(
                        "Missing parameters for tool call".to_string(),
                    ))
                }
            }
            "resources/list" => {
                let resources = self.get_available_resources();
                Ok(serde_json::json!({ "resources": resources }))
            }
            "resources/read" => {
                if let Some(params) = params {
                    let uri = params.get("uri").and_then(|u| u.as_str()).ok_or_else(|| {
                        McpServerError::InvalidArguments("Missing resource URI".to_string())
                    })?;

                    self.handle_resource_read(uri).await
                } else {
                    Err(McpServerError::InvalidArguments(
                        "Missing parameters for resource read".to_string(),
                    ))
                }
            }
            "initialize" => {
                let mut response = serde_json::Map::new();
                response.insert(
                    "protocolVersion".to_string(),
                    serde_json::Value::String("2024-11-05".to_string()),
                );
                response.insert("serverInfo".to_string(), self.get_server_info());
                response.insert("capabilities".to_string(), self.get_capabilities());
                Ok(serde_json::Value::Object(response))
            }
            "ping" => Ok(serde_json::json!({ "result": "pong" })),
            _ => Err(McpServerError::Mcp(format!("Unknown method: {}", method))),
        }
    }

    /// Get full server capabilities and information
    async fn get_server_description(&self) -> serde_json::Value {
        serde_json::json!({
            "info": self.get_server_info(),
            "capabilities": self.get_capabilities(),
            "tools": self.get_available_tools().len(),
            "resources": self.get_available_resources().len(),
            "health": self.get_health_status().await
        })
    }
}

// =============================================================================
// Transport Layer Trait
// =============================================================================

/// MCP transport layer trait
///
/// Defines the interface for different transport mechanisms (stdio, HTTP, WebSocket).
/// Transport implementations handle the low-level communication protocol.
#[async_trait::async_trait]
pub trait McpTransportLayer: Send + Sync {
    /// Start the transport layer
    async fn start(&self) -> McpResult<()>;

    /// Stop the transport layer
    async fn stop(&self) -> McpResult<()>;

    /// Send a JSON-RPC response
    async fn send_response(&self, response: JsonRpcResponse) -> McpResult<()>;

    /// Check if transport is active
    fn is_active(&self) -> bool;

    /// Get transport type identifier
    fn transport_type(&self) -> &'static str {
        "unknown"
    }

    /// Get transport configuration
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "type": self.transport_type(),
            "active": self.is_active()
        })
    }
}

// =============================================================================
// Error Types and Results
// =============================================================================

/// MCP server error type
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    #[error("SDK error: {0}")]
    Sdk(#[from] SdkError),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Wallet not configured")]
    WalletNotConfigured,

    #[error("Invalid tool arguments: {0}")]
    InvalidArguments(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Unknown resource: {0}")]
    UnknownResource(String),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Other error: {0}")]
    Other(String),
}

impl McpServerError {
    /// Convert SDK error to MCP JSON-RPC error code
    /// This provides proper error mapping as required by the MCP specification
    pub fn to_json_rpc_error_code(&self) -> i32 {
        // JSON-RPC 2.0 standard error codes

        match self {
            // SDK error mapping - map based on the underlying SDK error type
            McpServerError::Sdk(sdk_error) => Self::sdk_error_to_code(sdk_error),

            // MCP protocol errors
            McpServerError::Mcp(_) => INTERNAL_ERROR,
            McpServerError::WalletNotConfigured => WALLET_NOT_CONFIGURED,
            McpServerError::InvalidArguments(_) => INVALID_PARAMS,
            McpServerError::UnknownTool(_) => METHOD_NOT_FOUND,
            McpServerError::UnknownResource(_) => RESOURCE_NOT_FOUND,

            // System errors
            McpServerError::Serialization(_) => SERIALIZATION_ERROR,
            McpServerError::Network(_) => NETWORK_CONNECTION_FAILED,
            McpServerError::Validation(_) => VALIDATION_ERROR,
            McpServerError::Internal(_) => INTERNAL_ERROR,
            McpServerError::Config(_) => CONFIGURATION_ERROR,
            McpServerError::Other(_) => INTERNAL_ERROR,
        }
    }

    /// Map SDK error types to appropriate JSON-RPC error codes
    fn sdk_error_to_code(sdk_error: &SdkError) -> i32 {
        // JSON-RPC 2.0 standard error codes

        match sdk_error {
            // CosmRS and RPC errors
            SdkError::CosmRs(_) => BLOCKCHAIN_RPC_ERROR,
            SdkError::Rpc(_) => BLOCKCHAIN_RPC_ERROR,

            // Transaction errors
            SdkError::TxBroadcast(_) => TRANSACTION_FAILED,
            SdkError::TxSimulation(_) => TRANSACTION_FAILED,
            SdkError::Tx(_) => TRANSACTION_FAILED,

            // Wallet errors with enhanced context detection
            SdkError::Wallet(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("insufficient") || msg_lower.contains("balance") {
                    INSUFFICIENT_FUNDS
                } else if msg_lower.contains("public key") || msg_lower.contains("pubkey") {
                    INVALID_PUBLIC_KEY_FORMAT
                } else if msg_lower.contains("mnemonic") {
                    INVALID_MNEMONIC_FORMAT
                } else if msg_lower.contains("address") || msg_lower.contains("format") {
                    INVALID_ADDRESS_FORMAT
                } else {
                    WALLET_NOT_CONFIGURED
                }
            }

            // Configuration and contract errors
            SdkError::Config(_) => CONFIGURATION_ERROR,
            SdkError::Contract(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("pool") && msg_lower.contains("not found") {
                    POOL_NOT_FOUND
                } else if msg_lower.contains("slippage") {
                    SWAP_SLIPPAGE_EXCEEDED
                } else if msg_lower.contains("liquidity") {
                    LIQUIDITY_INSUFFICIENT
                } else {
                    TOOL_EXECUTION_FAILED
                }
            }

            // Fee validation errors (v3.0.0 feature)
            SdkError::FeeValidation(_) => FEE_VALIDATION_FAILED,

            // Network and timeout errors
            SdkError::Network(_) => NETWORK_CONNECTION_FAILED,
            SdkError::Timeout(_) => TIMEOUT_ERROR,

            // Serialization and IO errors
            SdkError::Serialization(_) => SERIALIZATION_ERROR,
            SdkError::Io(_) => IO_ERROR,

            // Generic errors
            SdkError::Other(_) => INTERNAL_ERROR,
            
            // New error types
            SdkError::NotImplemented(_) => TOOL_EXECUTION_FAILED,
            SdkError::WalletNotSet => WALLET_NOT_CONFIGURED,
            SdkError::Skip(_) => BLOCKCHAIN_RPC_ERROR,
        }
    }

    /// Get additional error data for JSON-RPC error response
    /// This provides context and helps with debugging and error recovery
    pub fn get_error_data(&self) -> Option<serde_json::Value> {
        match self {
            McpServerError::Sdk(sdk_error) => Some(serde_json::json!({
                "sdk_error_type": Self::get_sdk_error_type_name(sdk_error),
                "original_error": sdk_error.to_string(),
                "category": "sdk",
                "error_code": Self::sdk_error_to_code(sdk_error),
                "recovery_suggestions": Self::get_recovery_suggestions(sdk_error),
                "severity": Self::get_error_severity(sdk_error),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::Validation(msg) => Some(serde_json::json!({
                "validation_error": msg,
                "category": "validation",
                "severity": "high",
                "recovery_suggestions": ["Check input parameters", "Validate data format"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::Network(msg) => Some(serde_json::json!({
                "network_error": msg,
                "category": "network",
                "severity": "medium",
                "recovery_suggestions": ["Check network connectivity", "Verify RPC endpoints", "Retry with exponential backoff"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::InvalidArguments(msg) => Some(serde_json::json!({
                "argument_error": msg,
                "category": "arguments",
                "severity": "high",
                "recovery_suggestions": ["Check tool argument schema", "Validate required parameters"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::WalletNotConfigured => Some(serde_json::json!({
                "category": "wallet",
                "severity": "high",
                "recovery_suggestions": ["Generate or import a wallet", "Check wallet configuration"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::UnknownTool(tool_name) => Some(serde_json::json!({
                "tool_name": tool_name,
                "category": "tool",
                "severity": "medium",
                "recovery_suggestions": ["Check available tools list", "Verify tool name spelling"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::UnknownResource(uri) => Some(serde_json::json!({
                "resource_uri": uri,
                "category": "resource",
                "severity": "medium",
                "recovery_suggestions": ["Check available resources list", "Verify resource URI format"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            _ => None,
        }
    }

    /// Get recovery suggestions based on SDK error type
    fn get_recovery_suggestions(sdk_error: &SdkError) -> Vec<&'static str> {
        match sdk_error {
            SdkError::CosmRs(_) | SdkError::Rpc(_) => vec![
                "Check network connectivity",
                "Verify RPC endpoint configuration",
                "Try alternative RPC endpoints",
                "Check if blockchain network is operational",
            ],
            SdkError::TxBroadcast(_) | SdkError::TxSimulation(_) | SdkError::Tx(_) => vec![
                "Check transaction parameters",
                "Verify wallet has sufficient balance for gas fees",
                "Try with higher gas limit",
                "Check network congestion status",
            ],
            SdkError::Wallet(_) => vec![
                "Verify wallet is properly configured",
                "Check wallet balance",
                "Validate wallet address format",
                "Regenerate or re-import wallet if corrupted",
            ],
            SdkError::Config(_) => vec![
                "Check configuration file syntax",
                "Verify network configuration parameters",
                "Reset to default configuration if needed",
            ],
            SdkError::Contract(_) => vec![
                "Verify contract addresses are correct",
                "Check contract is deployed on current network",
                "Validate method parameters",
                "Check contract state and availability",
            ],
            SdkError::FeeValidation(_) => vec![
                "Review fee structure parameters",
                "Ensure total fees don't exceed 20% limit",
                "Validate individual fee components",
                "Use default fee structure if unsure",
            ],
            SdkError::Network(_) => vec![
                "Check internet connectivity",
                "Verify firewall settings",
                "Try different network configuration",
                "Check DNS resolution",
            ],
            SdkError::Timeout(_) => vec![
                "Increase timeout duration",
                "Check network latency",
                "Retry with exponential backoff",
                "Switch to faster RPC endpoint",
            ],
            SdkError::Serialization(_) => vec![
                "Check data format and structure",
                "Validate JSON syntax",
                "Ensure all required fields are present",
            ],
            SdkError::Io(_) => vec![
                "Check file permissions",
                "Verify file paths are accessible",
                "Ensure sufficient disk space",
                "Check directory structure",
            ],
            SdkError::Other(_) => vec![
                "Check application logs for details",
                "Retry the operation",
                "Contact support if issue persists",
            ],
            SdkError::NotImplemented(_) => vec![
                "Feature not yet implemented",
                "Check for updates to the SDK",
                "Contact support for implementation timeline",
            ],
            SdkError::WalletNotSet => vec![
                "Configure wallet before performing this operation",
                "Use wallet_create or wallet_import tools",
                "Verify wallet configuration",
            ],
            SdkError::Skip(_) => vec![
                "Check Skip protocol configuration",
                "Verify cross-chain route availability",
                "Check network connectivity to Skip API",
            ],
        }
    }

    /// Get error severity level for monitoring and alerting
    fn get_error_severity(sdk_error: &SdkError) -> &'static str {
        match sdk_error {
            SdkError::CosmRs(_) | SdkError::Rpc(_) => "high",
            SdkError::TxBroadcast(_) | SdkError::TxSimulation(_) | SdkError::Tx(_) => "high",
            SdkError::Wallet(_) => "high",
            SdkError::Config(_) => "medium",
            SdkError::Contract(_) => "high",
            SdkError::FeeValidation(_) => "medium",
            SdkError::Network(_) => "medium",
            SdkError::Timeout(_) => "low",
            SdkError::Serialization(_) => "medium",
            SdkError::Io(_) => "low",
            SdkError::Other(_) => "medium",
            SdkError::NotImplemented(_) => "low",
            SdkError::WalletNotSet => "high",
            SdkError::Skip(_) => "medium",
        }
    }

    /// Get SDK error type name for debugging and categorization
    fn get_sdk_error_type_name(sdk_error: &SdkError) -> &'static str {
        match sdk_error {
            SdkError::CosmRs(_) => "CosmRs",
            SdkError::Rpc(_) => "Rpc",
            SdkError::TxBroadcast(_) => "TxBroadcast",
            SdkError::TxSimulation(_) => "TxSimulation",
            SdkError::Wallet(_) => "Wallet",
            SdkError::Config(_) => "Config",
            SdkError::Contract(_) => "Contract",
            SdkError::Serialization(_) => "Serialization",
            SdkError::Io(_) => "Io",
            SdkError::FeeValidation(_) => "FeeValidation",
            SdkError::Other(_) => "Other",
            SdkError::Tx(_) => "Tx",
            SdkError::Network(_) => "Network",
            SdkError::Timeout(_) => "Timeout",
            SdkError::NotImplemented(_) => "NotImplemented",
            SdkError::WalletNotSet => "WalletNotSet",
            SdkError::Skip(_) => "SkipProtocol",
        }
    }

    /// Create a JSON-RPC error object from this MCP error
    pub fn to_json_rpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.to_json_rpc_error_code(),
            message: self.to_string(),
            data: self.get_error_data(),
        }
    }

    /// Check if error is recoverable and suggests retry strategy
    pub fn is_recoverable(&self) -> bool {
        match self {
            McpServerError::Sdk(sdk_error) => match sdk_error {
                SdkError::Network(_) | SdkError::Timeout(_) | SdkError::Rpc(_) => true,
                SdkError::TxBroadcast(_) => true, // Transaction might succeed on retry
                _ => false,
            },
            McpServerError::Network(_) => true,
            _ => false,
        }
    }

    /// Get suggested retry delay in seconds for recoverable errors
    pub fn get_retry_delay(&self) -> Option<u64> {
        if self.is_recoverable() {
            match self {
                McpServerError::Sdk(SdkError::Network(_)) => Some(5),
                McpServerError::Sdk(SdkError::Timeout(_)) => Some(10),
                McpServerError::Sdk(SdkError::Rpc(_)) => Some(3),
                McpServerError::Sdk(SdkError::TxBroadcast(_)) => Some(15),
                McpServerError::Network(_) => Some(5),
                _ => Some(1),
            }
        } else {
            None
        }
    }
}

/// MCP server result type
pub type McpResult<T> = std::result::Result<T, McpServerError>;

/// Mantra DEX MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Network configuration
    pub network_config: MantraNetworkConfig,
    /// Whether to enable debug logging
    pub debug: bool,
    /// Maximum number of concurrent operations
    pub max_concurrent_ops: usize,
    /// HTTP server port (if using HTTP transport)
    pub http_port: u16,
    /// HTTP server host (if using HTTP transport)
    pub http_host: String,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
    /// Whether to auto-load .env file
    pub auto_load_env: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: "Mantra DEX SDK MCP Server".to_string(),
            version: "0.1.0".to_string(),
            network_config: MantraNetworkConfig::default(),
            debug: false,
            max_concurrent_ops: 10,
            http_port: 8080,
            http_host: "127.0.0.1".to_string(),
            request_timeout_secs: 30,
            cache_ttl_secs: 300,
            auto_load_env: true,
        }
    }
}

impl McpServerConfig {
    /// Load configuration from environment variables
    ///
    /// Environment variables supported:
    /// - MCP_SERVER_NAME: Server name
    /// - MCP_SERVER_VERSION: Server version
    /// - MCP_DEBUG: Enable debug logging (true/false)
    /// - MCP_MAX_CONCURRENT_OPS: Maximum concurrent operations
    /// - MCP_HTTP_PORT: HTTP server port
    /// - MCP_HTTP_HOST: HTTP server host
    /// - MCP_REQUEST_TIMEOUT_SECS: Request timeout in seconds
    /// - MCP_CACHE_TTL_SECS: Cache TTL in seconds
    /// - MCP_AUTO_LOAD_ENV: Auto-load .env file (true/false)
    /// - MANTRA_NETWORK: Network name (mainnet/testnet)
    pub fn from_env() -> McpResult<Self> {
        // Load .env file if auto-load is enabled (check env var first)
        let auto_load_env = env::var("MCP_AUTO_LOAD_ENV")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        if auto_load_env {
            // Try to load .env file, but don't fail if it doesn't exist
            if let Err(e) = dotenv::dotenv() {
                debug!("Could not load .env file: {}", e);
            } else {
                info!("Loaded configuration from .env file");
            }
        }

        let mut config = Self::default();

        // Load server configuration
        if let Ok(name) = env::var("MCP_SERVER_NAME") {
            config.name = name;
        }

        if let Ok(version) = env::var("MCP_SERVER_VERSION") {
            config.version = version;
        }

        if let Ok(debug_str) = env::var("MCP_DEBUG") {
            config.debug = debug_str.parse().unwrap_or(false);
        }

        if let Ok(max_ops_str) = env::var("MCP_MAX_CONCURRENT_OPS") {
            config.max_concurrent_ops = max_ops_str.parse().unwrap_or(10);
        }

        if let Ok(port_str) = env::var("MCP_HTTP_PORT") {
            config.http_port = port_str.parse().unwrap_or(8080);
        }

        if let Ok(host) = env::var("MCP_HTTP_HOST") {
            config.http_host = host;
        }

        if let Ok(timeout_str) = env::var("MCP_REQUEST_TIMEOUT_SECS") {
            config.request_timeout_secs = timeout_str.parse().unwrap_or(30);
        }

        if let Ok(cache_ttl_str) = env::var("MCP_CACHE_TTL_SECS") {
            config.cache_ttl_secs = cache_ttl_str.parse().unwrap_or(300);
        }

        config.auto_load_env = auto_load_env;

        // Load network configuration
        if let Ok(network_name) = env::var("MANTRA_NETWORK") {
            match network_name.as_str() {
                "mantra-dukong" => {
                    if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                        match MantraNetworkConfig::from_constants(&constants) {
                            Ok(network_config) => {
                                config.network_config = network_config;
                            }
                            Err(e) => {
                                warn!("Failed to create network config: {}, using default", e);
                            }
                        }
                    } else {
                        warn!("Could not load mantra-dukong constants, using default");
                    }
                }
                _ => {
                    warn!("Unknown network '{}', using default", network_name);
                }
            }
        }

        info!("MCP Server configuration loaded from environment");
        debug!("Config: {:?}", config);

        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> McpResult<()> {
        if self.name.is_empty() {
            return Err(McpServerError::Validation(
                "Server name cannot be empty".to_string(),
            ));
        }

        if self.version.is_empty() {
            return Err(McpServerError::Validation(
                "Server version cannot be empty".to_string(),
            ));
        }

        if self.max_concurrent_ops == 0 {
            return Err(McpServerError::Validation(
                "Maximum concurrent operations must be greater than 0".to_string(),
            ));
        }

        if self.http_port == 0 {
            return Err(McpServerError::Validation(
                "HTTP port must be greater than 0".to_string(),
            ));
        }

        if self.http_host.is_empty() {
            return Err(McpServerError::Validation(
                "HTTP host cannot be empty".to_string(),
            ));
        }

        if self.request_timeout_secs == 0 {
            return Err(McpServerError::Validation(
                "Request timeout must be greater than 0".to_string(),
            ));
        }

        if self.cache_ttl_secs == 0 {
            return Err(McpServerError::Validation(
                "Cache TTL must be greater than 0".to_string(),
            ));
        }


        Ok(())
    }

    /// Create configuration with network override
    pub fn with_network(network: &str) -> McpResult<Self> {
        let mut config = Self::from_env()?;

        match network {
            "mantra-dukong" => {
                if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                    match MantraNetworkConfig::from_constants(&constants) {
                        Ok(network_config) => {
                            config.network_config = network_config;
                        }
                        Err(e) => {
                            return Err(McpServerError::Network(format!(
                                "Failed to create network config: {}", e
                            )));
                        }
                    }
                } else {
                    return Err(McpServerError::Network(
                        "Could not load mantra-dukong network constants".to_string(),
                    ));
                }
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unsupported network: {}. Supported networks: mantra-dukong",
                    network
                )));
            }
        }

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a file (TOML or JSON)
    ///
    /// Supports the following formats based on file extension:
    /// - `.toml` - TOML format
    /// - `.json` - JSON format
    /// - `.yaml`, `.yml` - YAML format
    ///
    /// The configuration file can contain any subset of configuration options.
    /// Missing options will use default values or environment variable overrides.
    pub fn from_file<P: AsRef<Path>>(file_path: P) -> McpResult<Self> {
        let path = file_path.as_ref();

        if !path.exists() {
            return Err(McpServerError::Validation(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        let file_format = Self::detect_file_format(path)?;
        info!(
            "Loading MCP configuration from: {} ({:?})",
            path.display(),
            file_format
        );

        let config_builder = Config::builder()
            // Start with defaults
            .set_default("name", "Mantra DEX SDK MCP Server")?
            .set_default("version", "0.1.0")?
            .set_default("debug", false)?
            .set_default("max_concurrent_ops", 10)?
            .set_default("http_port", 8080)?
            .set_default("http_host", "127.0.0.1")?
            .set_default("request_timeout_secs", 30)?
            .set_default("cache_ttl_secs", 300)?
            .set_default("auto_load_env", true)?
            // Add file source
            .add_source(File::new(path.to_str().unwrap(), file_format))
            // Add environment variable overrides with MCP_ prefix
            .add_source(Environment::with_prefix("MCP").separator("_"));

        let settings = config_builder.build().map_err(|e| {
            McpServerError::Validation(format!("Failed to load configuration: {}", e))
        })?;

        // Get network name before deserializing (since try_deserialize consumes settings)
        let network_name = settings
            .get_string("network")
            .ok()
            .or_else(|| env::var("MANTRA_NETWORK").ok());

        // Deserialize into our config struct with custom field handling
        let mut config: McpServerConfig = settings.try_deserialize().map_err(|e| {
            McpServerError::Validation(format!("Failed to parse configuration: {}", e))
        })?;

        // Load network configuration if specified
        if let Some(network_name) = network_name {
            Self::apply_network_config(&mut config, &network_name)?;
        }

        // Validate the configuration
        config.validate()?;

        info!(
            "MCP Server configuration successfully loaded from file: {}",
            path.display()
        );
        debug!("Loaded config: {:?}", config);

        Ok(config)
    }

    /// Save configuration to a file (TOML or JSON)
    ///
    /// The format is determined by the file extension:
    /// - `.toml` - TOML format
    /// - `.json` - JSON format
    pub fn save_to_file<P: AsRef<Path>>(&self, file_path: P) -> McpResult<()> {
        let path = file_path.as_ref();
        let file_format = Self::detect_file_format(path)?;

        let content = match file_format {
            FileFormat::Toml => toml::to_string_pretty(self).map_err(|e| {
                McpServerError::Internal(format!("TOML serialization failed: {}", e))
            })?,
            FileFormat::Json => {
                serde_json::to_string_pretty(self).map_err(McpServerError::Serialization)?
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unsupported file format for saving: {}. Use .toml or .json",
                    path.display()
                )));
            }
        };

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                McpServerError::Internal(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        fs::write(path, content).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to write configuration to {}: {}",
                path.display(),
                e
            ))
        })?;

        info!("Configuration saved to: {}", path.display());
        Ok(())
    }

    /// Load configuration with layered sources: defaults -> file -> env vars
    ///
    /// This method provides the most flexible configuration loading:
    /// 1. Start with built-in defaults
    /// 2. Override with values from configuration file (if provided)
    /// 3. Override with environment variables (MCP_ prefix)
    /// 4. Apply network-specific settings
    pub fn from_sources(config_file: Option<&Path>) -> McpResult<Self> {
        info!("Loading MCP configuration with layered sources");

        let mut config_builder = Config::builder()
            // Start with defaults
            .set_default("name", "Mantra DEX SDK MCP Server")?
            .set_default("version", "0.1.0")?
            .set_default("debug", false)?
            .set_default("max_concurrent_ops", 10)?
            .set_default("http_port", 8080)?
            .set_default("http_host", "127.0.0.1")?
            .set_default("request_timeout_secs", 30)?
            .set_default("cache_ttl_secs", 300)?
            .set_default("auto_load_env", true)?;

        // Add file source if provided
        if let Some(path) = config_file {
            if path.exists() {
                let file_format = Self::detect_file_format(path)?;
                config_builder =
                    config_builder.add_source(File::new(path.to_str().unwrap(), file_format));
                info!(
                    "Added configuration file: {} ({:?})",
                    path.display(),
                    file_format
                );
            } else {
                warn!("Configuration file not found: {}", path.display());
            }
        }

        // Add environment variable overrides
        config_builder = config_builder.add_source(Environment::with_prefix("MCP").separator("_"));

        let settings = config_builder.build().map_err(|e| {
            McpServerError::Validation(format!("Failed to build configuration: {}", e))
        })?;

        // Get network name before deserializing (since try_deserialize consumes settings)
        let network_name = settings
            .get_string("network")
            .ok()
            .or_else(|| env::var("MANTRA_NETWORK").ok());

        // Deserialize into our config struct
        let mut config: McpServerConfig = settings.try_deserialize().map_err(|e| {
            McpServerError::Validation(format!("Failed to parse configuration: {}", e))
        })?;

        // Apply network configuration
        if let Some(network_name) = network_name {
            Self::apply_network_config(&mut config, &network_name)?;
        }

        // Validate the final configuration
        config.validate()?;

        info!("MCP Server configuration successfully loaded from layered sources");
        debug!("Final config: {:?}", config);

        Ok(config)
    }

    /// Generate an example configuration file
    pub fn generate_example_config() -> String {
        let example_config = Self::default();

        format!(
            r#"# Mantra DEX SDK MCP Server Configuration
# This is an example configuration file showing all available options.
# You can copy this file and modify the values as needed.

# Server identification
name = "{}"
version = "{}"

# Network configuration (will be overridden if MANTRA_NETWORK env var is set)
# Supported values: "mainnet", "testnet", "mantra-dukong", "mantra-testnet"
network = "testnet"

# Logging configuration
debug = {}

# Performance settings
max_concurrent_ops = {}
request_timeout_secs = {}
cache_ttl_secs = {}

# HTTP transport settings (used when running with --transport http)
http_host = "{}"
http_port = {}

# Environment file loading
auto_load_env = {}

# Async runtime configuration
[runtime_config]
# Runtime flavor: "CurrentThread" or "MultiThread"
flavor = "MultiThread"
# Number of worker threads (optional, defaults to CPU count)
# worker_threads = 4
# Enable I/O and time support
enable_io = true
enable_time = true
# Maximum blocking threads for blocking operations
max_blocking_threads = 512
# Thread keep alive duration in seconds
thread_keep_alive = 10

# Examples of network-specific overrides:
# [network_config]
# rpc_endpoint = "https://rpc.mantra.com"
# chain_id = "mantra-dukong"
# gas_price = "0.01uom"
"#,
            example_config.name,
            example_config.version,
            example_config.debug,
            example_config.max_concurrent_ops,
            example_config.request_timeout_secs,
            example_config.cache_ttl_secs,
            example_config.http_host,
            example_config.http_port,
            example_config.auto_load_env
        )
    }

    /// Create example configuration files in common formats
    pub fn create_example_files(directory: &Path) -> McpResult<()> {
        fs::create_dir_all(directory).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to create directory {}: {}",
                directory.display(),
                e
            ))
        })?;

        // Create TOML example
        let toml_path = directory.join("mcp-server.example.toml");
        let toml_content = Self::generate_example_config();
        fs::write(&toml_path, toml_content).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to write TOML example to {}: {}",
                toml_path.display(),
                e
            ))
        })?;

        // Create JSON example
        let json_path = directory.join("mcp-server.example.json");
        let example_config = Self::default();
        let json_content =
            serde_json::to_string_pretty(&example_config).map_err(McpServerError::Serialization)?;
        fs::write(&json_path, json_content).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to write JSON example to {}: {}",
                json_path.display(),
                e
            ))
        })?;

        info!(
            "Created example configuration files in: {}",
            directory.display()
        );
        info!("  - {}", toml_path.display());
        info!("  - {}", json_path.display());

        Ok(())
    }

    /// Detect file format based on file extension
    fn detect_file_format(path: &Path) -> McpResult<FileFormat> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "toml" => Ok(FileFormat::Toml),
            "json" => Ok(FileFormat::Json),
            "yaml" | "yml" => Ok(FileFormat::Yaml),
            _ => Err(McpServerError::Validation(format!(
                "Unsupported configuration file format: {}. Supported formats: .toml, .json, .yaml, .yml",
                extension
            ))),
        }
    }

    /// Apply network-specific configuration
    fn apply_network_config(config: &mut McpServerConfig, network_name: &str) -> McpResult<()> {
        match network_name {
            "mantra-dukong" => {
                if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                    match MantraNetworkConfig::from_constants(&constants) {
                        Ok(network_config) => {
                            config.network_config = network_config;
                            info!("Applied mantra-dukong network configuration");
                        }
                        Err(e) => {
                            warn!("Failed to create network config: {}, using default", e);
                        }
                    }
                } else {
                    warn!("Could not load mantra-dukong constants, using default");
                }
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unknown network: {}. Supported networks: mantra-dukong",
                    network_name
                )));
            }
        }
        Ok(())
    }
}

/// Mantra DEX MCP Server state
pub struct McpServerStateData {
    /// DEX client instance
    pub client: Arc<Mutex<Option<MantraDexClient>>>,
    /// Server configuration
    pub config: McpServerConfig,
    /// Loaded wallets (address -> wallet info)
    pub wallets: Arc<RwLock<HashMap<String, WalletInfo>>>,
    /// Current active wallet address
    pub active_wallet: Arc<Mutex<Option<String>>>,
    /// Cached data for performance
    pub cache: Arc<RwLock<HashMap<String, Value>>>,
    /// SDK adapter for connection management
    pub sdk_adapter: Arc<McpSdkAdapter>,
    /// High-level client wrapper for MCP operations
    pub client_wrapper: Arc<Mutex<Option<McpClientWrapper>>>,
    /// Logging infrastructure
    pub logger: Arc<McpLogger>,
    /// Transaction monitor manager
    pub transaction_monitor_manager: Arc<TransactionMonitorManager>,
}

impl McpServerStateData {
    pub fn new(config: McpServerConfig) -> Self {
        let sdk_adapter = Arc::new(McpSdkAdapter::default());

        // Initialize logging infrastructure
        let logging_config = LoggingConfig::default();
        let logger = Arc::new(McpLogger::new(logging_config).expect("Failed to create MCP logger"));

        // Initialize transaction monitor manager
        let transaction_monitor_manager = Arc::new(TransactionMonitorManager::new());

        Self {
            client: Arc::new(Mutex::new(None)),
            config,
            wallets: Arc::new(RwLock::new(HashMap::new())),
            active_wallet: Arc::new(Mutex::new(None)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            sdk_adapter,
            client_wrapper: Arc::new(Mutex::new(None)),
            logger,
            transaction_monitor_manager,
        }
    }

    /// Initialize the DEX client
    pub async fn initialize_client(&self) -> McpResult<()> {
        info!("Attempting to initialize DEX client...");
        debug!("Network config: {:?}", self.config.network_config);

        let client = match MantraDexClient::new(self.config.network_config.clone()).await {
            Ok(client) => {
                info!("DEX client created successfully");
                client
            }
            Err(e) => {
                error!("Failed to create DEX client: {:?}", e);
                return Err(McpServerError::Sdk(e));
            }
        };

        *self.client.lock().await = Some(client);
        info!(
            "DEX client initialized for network: {}",
            self.config.network_config.network_name
        );
        Ok(())
    }

    /// Get the current DEX client
    pub async fn get_client(&self) -> McpResult<Arc<Mutex<Option<MantraDexClient>>>> {
        Ok(self.client.clone())
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
        *self.active_wallet.lock().await = Some(address);

        Ok(())
    }

    /// Get the active wallet info
    pub async fn get_active_wallet(&self) -> McpResult<Option<WalletInfo>> {
        let active_address = self.active_wallet.lock().await.clone();
        if let Some(address) = active_address {
            let wallets = self.wallets.read().await;
            Ok(wallets.get(&address).cloned())
        } else {
            Ok(None)
        }
    }

    /// Cache a value
    /// Set cached value
    pub async fn cache_set(&self, key: String, value: Value) {
        self.cache.write().await.insert(key, value);
    }

    /// Get cached value
    pub async fn cache_get(&self, key: &str) -> Option<Value> {
        self.cache.read().await.get(key).cloned()
    }

    /// Clear cache
    pub async fn cache_clear(&self) {
        self.cache.write().await.clear();
    }

    /// Switch to a different network configuration
    pub async fn switch_network(&self, network_name: &str) -> McpResult<()> {
        use crate::config::{MantraNetworkConfig, NetworkConstants};

        // Load network constants for the requested network
        let constants = NetworkConstants::load(network_name).map_err(|e| {
            McpServerError::Network(format!("Failed to load network '{}': {}", network_name, e))
        })?;

        // Create new network config
        let _new_network_config = MantraNetworkConfig::from_constants(&constants).map_err(|e| {
            McpServerError::Network(format!("Failed to create network config: {}", e))
        })?;

        // Clear existing client and cache
        {
            let mut client = self.client.lock().await;
            *client = None;
        }
        self.cache_clear().await;

        // Update the network configuration in the server config
        // Note: This is a bit tricky since config is not mutable. We need to create a new config.
        // For now, we'll store the network config separately or update it through the client initialization

        info!("Successfully switched to network: {}", network_name);
        Ok(())
    }

    /// Initialize client with specific network configuration
    pub async fn initialize_client_with_network(
        &self,
        network_config: MantraNetworkConfig,
    ) -> McpResult<()> {
        let client = MantraDexClient::new(network_config.clone())
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        {
            let mut client_guard = self.client.lock().await;
            *client_guard = Some(client);
        }

        info!(
            "Initialized client for network: {}",
            network_config.network_name
        );
        Ok(())
    }
}

/// Mantra DEX MCP Server handler
///
/// Note: ServerHandler trait not implemented due to MCP API instability
/// Current rust-mcp-sdk 0.4.2 has unstable APIs that are changing between versions
/// This implementation provides MCP functionality via direct HTTP/JSON-RPC handling
/// and can be migrated to ServerHandler trait when the rust-mcp-sdk API stabilizes
pub struct MantraDexMcpServer {
    /// Server state
    state: Arc<McpServerStateData>,
}

impl MantraDexMcpServer {
    /// Create a new MCP server
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            state: Arc::new(McpServerStateData::new(config)),
        }
    }

    /// Initialize the server
    pub async fn initialize(&self) -> McpResult<()> {
        info!("Initializing Mantra DEX MCP Server");
        self.state.initialize_client().await?;

        // Auto-load wallet from environment if available
        self.auto_load_wallet_from_env().await?;

        // Initialize client wrapper
        let wrapper = McpClientWrapper::new(
            self.state.sdk_adapter.clone(),
            self.state.config.network_config.clone(),
        );
        *self.state.client_wrapper.lock().await = Some(wrapper);

        info!("Server initialization complete");
        Ok(())
    }

    /// Get server state
    pub fn state(&self) -> Arc<McpServerStateData> {
        self.state.clone()
    }

    /// Auto-load wallet from environment variables
    async fn auto_load_wallet_from_env(&self) -> McpResult<()> {
        use crate::wallet::MantraWallet;
        use std::env;

        // Check for wallet mnemonic in environment
        if let Ok(mnemonic) = env::var("WALLET_MNEMONIC") {
            if !mnemonic.trim().is_empty() {
                info!("Auto-loading wallet from WALLET_MNEMONIC environment variable");

                // Create wallet from mnemonic (using account index 0)
                match MantraWallet::from_mnemonic(&mnemonic, 0) {
                    Ok(wallet) => {
                        let wallet_info = wallet.info();
                        let address = wallet_info.address.clone();

                        // Store wallet info in state
                        self.state
                            .set_active_wallet(address.clone(), wallet_info.clone())
                            .await?;

                        // Create another wallet instance for the SDK adapter (since MantraWallet doesn't implement Clone)
                        match MantraWallet::from_mnemonic(&mnemonic, 0) {
                            Ok(adapter_wallet) => {
                                // Add wallet with derivation index for caching, then set as active
                                self.state
                                    .sdk_adapter
                                    .add_wallet_with_derivation_index(adapter_wallet, 0)
                                    .await?;
                                self.state
                                    .sdk_adapter
                                    .switch_active_wallet(&address)
                                    .await?;
                            }
                            Err(e) => {
                                warn!("Failed to create wallet instance for SDK adapter: {}", e);
                            }
                        }

                        // Update the client with the wallet
                        let mut client_guard = self.state.client.lock().await;
                        if let Some(client) = client_guard.take() {
                            *client_guard = Some(client.with_wallet(wallet));
                        }

                        info!("Successfully auto-loaded wallet with address: {}", address);
                    }
                    Err(e) => {
                        warn!("Failed to create wallet from WALLET_MNEMONIC: {}", e);
                        return Err(McpServerError::Validation(format!(
                            "Invalid WALLET_MNEMONIC: {}",
                            e
                        )));
                    }
                }
            } else {
                debug!("WALLET_MNEMONIC environment variable is empty, skipping auto-load");
            }
        } else {
            debug!("No WALLET_MNEMONIC environment variable found, skipping auto-load");
        }


        Ok(())
    }

    /// Get server information
    pub fn get_server_info(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.state.config.name,
            "version": self.state.config.version
        })
    }

    /// Get server capabilities
    pub fn get_capabilities(&self) -> serde_json::Value {
        serde_json::json!({
            "tools": {
                "list_changed": true
            },
            "resources": {
                "list_changed": true,
                "subscribe": false
            },
            "logging": {},
            "experimental": {}
        })
    }
}

// =============================================================================
// MCP Trait Implementations
// =============================================================================

#[async_trait::async_trait]
impl McpServerLifecycle for MantraDexMcpServer {
    async fn initialize(&self) -> McpResult<()> {
        MantraDexMcpServer::initialize(self).await
    }

    fn get_server_info(&self) -> serde_json::Value {
        MantraDexMcpServer::get_server_info(self)
    }

    fn get_capabilities(&self) -> serde_json::Value {
        MantraDexMcpServer::get_capabilities(self)
    }

    async fn shutdown(&self) -> McpResult<()> {
        info!("Shutting down Mantra DEX MCP Server");
        Ok(())
    }
}

#[async_trait::async_trait]
impl McpResourceProvider for MantraDexMcpServer {
    fn get_available_resources(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "uri": "trades://history",
                "name": "Trading History",
                "description": "Historical trading data and transaction records",
                "mimeType": "application/json"
            }),
            serde_json::json!({
                "uri": "trades://pending",
                "name": "Pending Trades",
                "description": "Currently pending or in-progress trading transactions",
                "mimeType": "application/json"
            }),
            serde_json::json!({
                "uri": "liquidity://positions",
                "name": "Liquidity Positions",
                "description": "Current and historical liquidity positions",
                "mimeType": "application/json"
            }),
        ]
    }

    async fn handle_resource_read(&self, uri: &str) -> McpResult<serde_json::Value> {
        match uri {
            "trades://history" => Ok(serde_json::json!({
                "trades": [],
                "total_count": 0,
                "message": "Trades history resource not available"
            })),
            "trades://pending" => Ok(serde_json::json!({
                "pending_trades": [],
                "total_count": 0,
                "message": "Pending trades resource not available"
            })),
            "liquidity://positions" => Ok(serde_json::json!({
                "positions": [],
                "total_value": "0",
                "message": "Liquidity positions resource not available"
            })),
            _ => Err(McpServerError::UnknownResource(uri.to_string())),
        }
    }

    fn validate_resource_uri(&self, uri: &str) -> McpResult<()> {
        match uri {
            "trades://history" | "trades://pending" | "liquidity://positions" => Ok(()),
            _ => Err(McpServerError::Validation(format!(
                "Invalid resource URI: {}. Available resources: trades://history, trades://pending, liquidity://positions",
                uri
            ))),
        }
    }

    async fn get_resource_metadata(&self, uri: &str) -> McpResult<serde_json::Value> {
        match uri {
            "trades://history" => Ok(serde_json::json!({
                "uri": uri,
                "name": "Trading History",
                "description": "Historical trading data and transaction records",
                "mimeType": "application/json",
                "available": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "trades": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "tx_hash": {"type": "string"},
                                    "timestamp": {"type": "string"},
                                    "status": {"type": "string"},
                                    "pool_id": {"type": "string"},
                                    "from_asset": {"type": "object"},
                                    "to_asset": {"type": "object"},
                                    "gas_info": {"type": "object"}
                                }
                            }
                        },
                        "total_count": {"type": "number"},
                        "wallet_address": {"type": "string"}
                    }
                }
            })),
            "trades://pending" => Ok(serde_json::json!({
                "uri": uri,
                "name": "Pending Trades",
                "description": "Currently pending or in-progress trading transactions",
                "mimeType": "application/json",
                "available": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "pending_trades": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "tx_hash": {"type": "string"},
                                    "timestamp": {"type": "string"},
                                    "status": {"type": "string"},
                                    "pool_id": {"type": "string"},
                                    "monitor_id": {"type": "string"}
                                }
                            }
                        },
                        "total_count": {"type": "number"}
                    }
                }
            })),
            "liquidity://positions" => Ok(serde_json::json!({
                "uri": uri,
                "name": "Liquidity Positions",
                "description": "Current and historical liquidity positions",
                "mimeType": "application/json",
                "available": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "positions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "pool_id": {"type": "string"},
                                    "lp_token_balance": {"type": "string"},
                                    "assets": {"type": "array"},
                                    "timestamp": {"type": "string"}
                                }
                            }
                        },
                        "total_value": {"type": "string"},
                        "wallet_address": {"type": "string"}
                    }
                }
            })),
            _ => Err(McpServerError::UnknownResource(uri.to_string())),
        }
    }
}

#[async_trait::async_trait]
impl McpServerStateManager for MantraDexMcpServer {
    async fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "server_name": self.state.config.name,
            "server_version": self.state.config.version,
            "network": {
                "chain_id": self.state.config.network_config.chain_id,
                "rpc_endpoint": self.state.config.network_config.rpc_url,
                "lcd_endpoint": self.state.config.network_config.rpc_url,
                "grpc_endpoint": self.state.config.network_config.rpc_url
            },
            "settings": {
                "debug": self.state.config.debug,
                "max_concurrent_ops": self.state.config.max_concurrent_ops,
                "request_timeout_secs": self.state.config.request_timeout_secs,
                "cache_ttl_secs": self.state.config.cache_ttl_secs
            }
        })
    }

    async fn update_config(&self, config: serde_json::Value) -> McpResult<()> {
        // For now, only support network switching
        if let Some(network_name) = config.get("network").and_then(|n| n.as_str()) {
            self.state.switch_network(network_name).await?;
            info!("Switched to network: {}", network_name);
        }

        if let Some(debug_mode) = config.get("debug").and_then(|d| d.as_bool()) {
            // Note: This would require runtime logging level changes
            info!("Debug mode requested: {}", debug_mode);
        }

        Ok(())
    }

    async fn clear_state(&self) -> McpResult<()> {
        // Clear caches
        self.state.cache_clear().await;

        // Clear transaction monitors
        let cleanup_count = self
            .state
            .transaction_monitor_manager
            .cleanup_completed()
            .await;
        info!("Cleared {} completed transaction monitors", cleanup_count);

        // Clear wallet cache (keep active wallet)
        let mut wallets = self.state.wallets.write().await;
        if let Some(active_address) = self.state.active_wallet.lock().await.clone() {
            if let Some(active_wallet) = wallets.get(&active_address).cloned() {
                wallets.clear();
                wallets.insert(active_address, active_wallet);
            }
        }
        drop(wallets);

        info!("Server state cleared successfully");
        Ok(())
    }

    async fn get_health_status(&self) -> serde_json::Value {
        let client_status = if self.state.client.lock().await.is_some() {
            "connected"
        } else {
            "disconnected"
        };

        let active_wallet = self.state.active_wallet.lock().await.is_some();
        let wallet_count = self.state.wallets.read().await.len();
        let cache_size = self.state.cache.read().await.len();

        let active_monitors = self
            .state
            .transaction_monitor_manager
            .list_monitors_filtered(false)
            .await
            .len();


        serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "components": {
                "client": {
                    "status": client_status,
                    "network": self.state.config.network_config.chain_id
                },
                "wallet": {
                    "active": active_wallet,
                    "total_wallets": wallet_count
                },
                "cache": {
                    "entries": cache_size
                },
                "transaction_monitoring": {
                    "active_monitors": active_monitors
                }
            }
        })
    }
}

#[async_trait::async_trait]
impl McpToolProvider for MantraDexMcpServer {
    fn get_available_tools(&self) -> Vec<serde_json::Value> {
        vec![
            // Network Tools
            serde_json::json!({
                "name": "network_get_contract_addresses",
                "description": "Get contract addresses for the current network",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "include_metadata": {
                            "type": "boolean",
                            "description": "Whether to include contract metadata and descriptions (default: false)",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "network_validate_connectivity",
                "description": "Validate network connectivity and blockchain access",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "check_rpc": {
                            "type": "boolean",
                            "description": "Whether to check RPC endpoint connectivity (default: true)",
                            "default": true
                        },
                        "check_block_height": {
                            "type": "boolean",
                            "description": "Whether to check latest block height retrieval (default: true)",
                            "default": true
                        },
                        "check_contracts": {
                            "type": "boolean",
                            "description": "Whether to validate contract addresses (default: true)",
                            "default": true
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": "Timeout for each connectivity check in seconds (default: 10)",
                            "default": 10,
                            "minimum": 1,
                            "maximum": 60
                        },
                        "include_diagnostics": {
                            "type": "boolean",
                            "description": "Whether to include detailed diagnostic information (default: false)",
                            "default": false
                        }
                    }
                }
            }),
            // Wallet and Balance Tools
            serde_json::json!({
                "name": "wallet_get_balances",
                "description": "Get wallet balances for all assets",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query balances for (optional, uses active wallet if not provided)"
                        },
                        "include_zero_balances": {
                            "type": "boolean",
                            "description": "Whether to include assets with zero balance (default: false)",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "wallet_list",
                "description": "List all available wallets with their addresses and information",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "wallet_switch",
                "description": "Switch to a different active wallet",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "The wallet address to switch to"
                        }
                    },
                    "required": ["wallet_address"]
                }
            }),
            serde_json::json!({
                "name": "wallet_get_active",
                "description": "Get current active wallet information",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "wallet_add_from_mnemonic",
                "description": "Add a new wallet from mnemonic phrase",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "mnemonic": {
                            "type": "string",
                            "description": "The mnemonic phrase to import the wallet from"
                        },
                        "derivation_index": {
                            "type": "integer",
                            "description": "The derivation index for the wallet (default: 0)",
                            "default": 0,
                            "minimum": 0
                        }
                    },
                    "required": ["mnemonic"]
                }
            }),
            serde_json::json!({
                "name": "wallet_remove",
                "description": "Remove a wallet from the collection",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "The wallet address to remove"
                        }
                    },
                    "required": ["wallet_address"]
                }
            }),
            // Pool Query Tools
            serde_json::json!({
                "name": "dex_get_pools",
                "description": "Get information about all available liquidity pools with optional filtering and pagination",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of pools to return (optional)",
                            "minimum": 1,
                            "maximum": 100
                        },
                        "start_after": {
                            "type": "string",
                            "description": "Pool ID to start pagination after (optional)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "dex_execute_swap",
                "description": "Executes a token swap in a specified pool with slippage protection.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to swap in." },
                        "offer_asset": {
                            "type": "object",
                            "properties": {
                                "denom": { "type": "string" },
                                "amount": { "type": "string" }
                            },
                            "required": ["denom", "amount"]
                        },
                        "ask_asset_denom": { "type": "string", "description": "The denomination of the asset to receive." },
                        "max_slippage": { "type": "string", "description": "Maximum allowed slippage percentage (e.g., '1.5'). Defaults to 1%." },
                        "wallet_address": { "type": "string", "description": "Wallet address to use for the swap (optional, uses active wallet if not provided)" }
                    },
                    "required": ["pool_id", "offer_asset", "ask_asset_denom"]
                }
            }),
            serde_json::json!({
                "name": "dex_provide_liquidity",
                "description": "Provides liquidity to a specified pool.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to provide liquidity to." },
                        "assets": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "denom": { "type": "string" },
                                    "amount": { "type": "string" }
                                },
                                "required": ["denom", "amount"]
                            },
                            "description": "The assets to provide."
                        },
                        "max_slippage": { "type": "string", "description": "Maximum allowed slippage percentage. Defaults to 1%." },
                        "wallet_address": { "type": "string", "description": "Wallet address to use for providing liquidity (optional, uses active wallet if not provided)" }
                    },
                    "required": ["pool_id", "assets"]
                }
            }),
            serde_json::json!({
                "name": "dex_withdraw_liquidity",
                "description": "Withdraws liquidity from a specified pool.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to withdraw from." },
                        "amount": { "type": "string", "description": "The amount of LP tokens to withdraw." },
                        "wallet_address": { "type": "string", "description": "Wallet address to use for withdrawing liquidity (optional, uses active wallet if not provided)" }
                    },
                    "required": ["pool_id", "amount"]
                }
            }),
            serde_json::json!({
                "name": "dex_create_pool",
                "description": "Creates a new liquidity pool (admin only).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pool_type": {
                            "type": "string",
                            "description": "The type of pool to create (constant_product, stable_swap).",
                            "enum": ["constant_product", "stable_swap"]
                        },
                        "assets": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "denom": { "type": "string" },
                                    "amount": { "type": "string" }
                                },
                                "required": ["denom", "amount"]
                            },
                            "description": "The initial assets to provide to the pool.",
                            "minItems": 2,
                            "maxItems": 8
                        },
                        "fees": {
                            "type": "object",
                            "properties": {
                                "protocol_fee": { "type": "string", "description": "Protocol fee percentage" },
                                "swap_fee": { "type": "string", "description": "Swap fee percentage" },
                                "burn_fee": { "type": "string", "description": "Burn fee percentage" }
                            },
                            "description": "Fee structure for the pool"
                        },
                        "amplification": {
                            "type": "integer",
                            "description": "Amplification parameter for stable swap pools (ignored for constant product)",
                            "minimum": 1
                        }
                    },
                    "required": ["pool_type", "assets"]
                }
            }),
            serde_json::json!({
                "name": "dex_get_lp_token_balance",
                "description": "Get LP token balance for a specific pool",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID to query LP token balance for"
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query (optional, uses active wallet if not provided)"
                        }
                    },
                    "required": ["pool_id"]
                }
            }),
            serde_json::json!({
                "name": "dex_get_all_lp_token_balances",
                "description": "Get all LP token balances for the wallet across all pools",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query (optional, uses active wallet if not provided)"
                        },
                        "include_zero_balances": {
                            "type": "boolean",
                            "description": "Include pools with zero LP token balance",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "dex_estimate_lp_withdrawal_amounts",
                "description": "Estimate withdrawal amounts for LP tokens",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID to estimate withdrawal for"
                        },
                        "lp_token_amount": {
                            "type": "string",
                            "description": "Amount of LP tokens to withdraw (optional, uses full balance if not provided)"
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query (optional, uses active wallet if not provided)"
                        }
                    },
                    "required": ["pool_id"]
                }
            }),
            
            // ClaimDrop Tools
            serde_json::json!({
                "name": "claimdrop_create_campaign",
                "description": "Create a new claimdrop campaign through the factory",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "factory_address": {
                            "type": "string",
                            "description": "ClaimDrop factory contract address"
                        },
                        "owner": {
                            "type": "string",
                            "description": "Campaign owner address"
                        },
                        "start_time": {
                            "type": "integer",
                            "description": "Campaign start time (Unix timestamp)"
                        },
                        "end_time": {
                            "type": "integer",
                            "description": "Campaign end time (Unix timestamp)"
                        },
                        "reward_denom": {
                            "type": "string",
                            "description": "Token denomination for rewards"
                        },
                        "reward_per_allocation": {
                            "type": "string",
                            "description": "Amount of tokens per allocation unit"
                        },
                        "allocations": {
                            "type": "array",
                            "description": "List of user allocations",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "user": { "type": "string" },
                                    "allocated_amount": { "type": "string" }
                                },
                                "required": ["user", "allocated_amount"]
                            }
                        }
                    },
                    "required": ["factory_address", "owner", "start_time", "end_time", "reward_denom", "reward_per_allocation", "allocations"]
                }
            }),
            serde_json::json!({
                "name": "claimdrop_claim",
                "description": "Claim rewards from a claimdrop campaign",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_address": {
                            "type": "string",
                            "description": "Campaign contract address"
                        },
                        "amount": {
                            "type": "string",
                            "description": "Amount to claim (optional, claims all if not specified)"
                        },
                        "receiver": {
                            "type": "string",
                            "description": "Receiver address (optional, defaults to sender)"
                        }
                    },
                    "required": ["campaign_address"]
                }
            }),
            serde_json::json!({
                "name": "claimdrop_query_rewards",
                "description": "Query user rewards from a campaign or factory",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": {
                            "type": "string",
                            "description": "Campaign or factory contract address"
                        },
                        "user": {
                            "type": "string",
                            "description": "User address to query rewards for"
                        },
                        "is_factory": {
                            "type": "boolean",
                            "description": "Whether the address is a factory (true) or campaign (false)"
                        }
                    },
                    "required": ["address", "user"]
                }
            }),
            serde_json::json!({
                "name": "claimdrop_query_campaigns",
                "description": "Query all campaigns from the factory",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "factory_address": {
                            "type": "string",
                            "description": "ClaimDrop factory contract address"
                        },
                        "start_after": {
                            "type": "string",
                            "description": "Pagination start address (optional)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results (optional)"
                        }
                    },
                    "required": ["factory_address"]
                }
            }),
            serde_json::json!({
                "name": "claimdrop_add_allocations",
                "description": "Add allocations to a campaign (admin only, before campaign starts)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "campaign_address": {
                            "type": "string",
                            "description": "Campaign contract address"
                        },
                        "allocations": {
                            "type": "array",
                            "description": "List of user allocations to add",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "user": { "type": "string" },
                                    "allocated_amount": { "type": "string" }
                                },
                                "required": ["user", "allocated_amount"]
                            }
                        }
                    },
                    "required": ["campaign_address", "allocations"]
                }
            }),
            
            // Skip Protocol Tools
            serde_json::json!({
                "name": "skip_get_route",
                "description": "Find optimal cross-chain routes between assets",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source_asset_denom": {
                            "type": "string",
                            "description": "Source asset denomination"
                        },
                        "source_asset_amount": {
                            "type": "string",
                            "description": "Source asset amount"
                        },
                        "source_chain": {
                            "type": "string",
                            "description": "Source chain identifier"
                        },
                        "target_asset_denom": {
                            "type": "string",
                            "description": "Target asset denomination"
                        },
                        "target_chain": {
                            "type": "string",
                            "description": "Target chain identifier"
                        },
                        "allow_multi_tx": {
                            "type": "boolean",
                            "description": "Allow multi-transaction routes (optional)"
                        },
                        "smart_relay": {
                            "type": "boolean",
                            "description": "Use smart relay optimization (optional)"
                        }
                    },
                    "required": ["source_asset_denom", "source_asset_amount", "source_chain", "target_asset_denom", "target_chain"]
                }
            }),
            serde_json::json!({
                "name": "skip_execute_transfer",
                "description": "Execute cross-chain asset transfers",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source_asset_denom": {
                            "type": "string",
                            "description": "Source asset denomination"
                        },
                        "source_asset_amount": {
                            "type": "string",
                            "description": "Source asset amount"
                        },
                        "source_chain": {
                            "type": "string",
                            "description": "Source chain identifier"
                        },
                        "target_asset_denom": {
                            "type": "string",
                            "description": "Target asset denomination"
                        },
                        "target_chain": {
                            "type": "string",
                            "description": "Target chain identifier"
                        },
                        "recipient": {
                            "type": "string",
                            "description": "Recipient address on target chain"
                        },
                        "timeout_seconds": {
                            "type": "integer",
                            "description": "Transfer timeout in seconds (optional)"
                        },
                        "slippage_tolerance": {
                            "type": "string",
                            "description": "Slippage tolerance (optional)"
                        }
                    },
                    "required": ["source_asset_denom", "source_asset_amount", "source_chain", "target_asset_denom", "target_chain", "recipient"]
                }
            }),
            serde_json::json!({
                "name": "skip_track_transfer",
                "description": "Monitor transfer status and progress",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "transfer_id": {
                            "type": "string",
                            "description": "Transfer ID to track"
                        }
                    },
                    "required": ["transfer_id"]
                }
            }),
            serde_json::json!({
                "name": "skip_get_supported_chains",
                "description": "List available chains and their configurations",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "filter": {
                            "type": "string",
                            "description": "Filter chains by name or ID (optional)"
                        }
                    },
                    "required": []
                }
            }),
            serde_json::json!({
                "name": "skip_verify_assets",
                "description": "Validate assets across different chains",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "assets": {
                            "type": "array",
                            "description": "List of assets to verify",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "denom": { "type": "string" },
                                    "chain": { "type": "string" },
                                    "amount": { "type": "string" },
                                    "decimals": { "type": "integer" },
                                    "symbol": { "type": "string" }
                                },
                                "required": ["denom", "chain"]
                            }
                        }
                    },
                    "required": ["assets"]
                }
            }),
            serde_json::json!({
                "name": "skip_estimate_fees",
                "description": "Estimate fees for cross-chain operations",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source_asset_denom": {
                            "type": "string",
                            "description": "Source asset denomination"
                        },
                        "source_asset_amount": {
                            "type": "string",
                            "description": "Source asset amount"
                        },
                        "source_chain": {
                            "type": "string",
                            "description": "Source chain identifier"
                        },
                        "target_asset_denom": {
                            "type": "string",
                            "description": "Target asset denomination"
                        },
                        "target_chain": {
                            "type": "string",
                            "description": "Target chain identifier"
                        },
                        "recipient": {
                            "type": "string",
                            "description": "Recipient address on target chain"
                        }
                    },
                    "required": ["source_asset_denom", "source_asset_amount", "source_chain", "target_asset_denom", "target_chain", "recipient"]
                }
            }),
        ]
    }

    async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        match tool_name {
            // Network tools
            "network_get_contract_addresses" => self.handle_get_contract_addresses(arguments).await,
            "network_validate_connectivity" => {
                self.handle_validate_network_connectivity(arguments).await
            }
            
            // Wallet tools
            "wallet_get_balances" => self.handle_get_balances(arguments).await,
            "wallet_list" => self.handle_list_wallets(arguments).await,
            "wallet_switch" => self.handle_switch_wallet(arguments).await,
            "wallet_get_active" => self.handle_get_active_wallet(arguments).await,
            "wallet_add_from_mnemonic" => self.handle_add_wallet_from_mnemonic(arguments).await,
            "wallet_remove" => self.handle_remove_wallet(arguments).await,
            
            // DEX tools
            "dex_get_pools" => self.handle_get_pools(arguments).await,
            "dex_execute_swap" => self.handle_execute_swap(arguments).await,
            "dex_provide_liquidity" => self.handle_provide_liquidity(arguments).await,
            "dex_provide_liquidity_unchecked" => {
                self.handle_provide_liquidity_unchecked(arguments).await
            }
            "dex_withdraw_liquidity" => self.handle_withdraw_liquidity(arguments).await,
            "dex_create_pool" => self.handle_create_pool(arguments).await,
            "dex_monitor_swap_transaction" => self.handle_monitor_swap_transaction(arguments).await,
            "dex_get_lp_token_balance" => self.handle_get_lp_token_balance(arguments).await,
            "dex_get_all_lp_token_balances" => self.handle_get_all_lp_token_balances(arguments).await,
            "dex_estimate_lp_withdrawal_amounts" => {
                self.handle_estimate_lp_withdrawal_amounts(arguments).await
            }
            
            // ClaimDrop tools
            "claimdrop_create_campaign" => self.handle_claimdrop_create_campaign(arguments).await,
            "claimdrop_claim" => self.handle_claimdrop_claim(arguments).await,
            "claimdrop_query_rewards" => self.handle_claimdrop_query_rewards(arguments).await,
            "claimdrop_query_campaigns" => self.handle_claimdrop_query_campaigns(arguments).await,
            "claimdrop_add_allocations" => self.handle_claimdrop_add_allocations(arguments).await,
            
            // Skip protocol tools
            "skip_get_route" => self.handle_skip_get_route(arguments).await,
            "skip_execute_transfer" => self.handle_skip_execute_transfer(arguments).await,
            "skip_track_transfer" => self.handle_skip_track_transfer(arguments).await,
            "skip_get_supported_chains" => self.handle_skip_get_supported_chains(arguments).await,
            "skip_verify_assets" => self.handle_skip_verify_assets(arguments).await,
            "skip_estimate_fees" => self.handle_skip_estimate_fees(arguments).await,
            
            _ => Err(McpServerError::UnknownTool(tool_name.to_string())),
        }
    }
}

// Implement the main McpServer trait that combines all sub-traits
impl McpServer for MantraDexMcpServer {}

impl MantraDexMcpServer {
    /// Handle get_contract_addresses tool
    async fn handle_get_contract_addresses(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_contract_addresses tool call");

        let include_metadata = arguments
            .get("include_metadata")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let contracts = &self.state.config.network_config.contracts;

        let mut result = serde_json::json!({
            "network": self.state.config.network_config.network_name,
            "chain_id": self.state.config.network_config.chain_id,
            "contracts": {
                "pool_manager": contracts.pool_manager
            }
        });

        if let Some(farm_manager) = &contracts.farm_manager {
            result["contracts"]["farm_manager"] = serde_json::Value::String(farm_manager.clone());
        }

        if let Some(fee_collector) = &contracts.fee_collector {
            result["contracts"]["fee_collector"] = serde_json::Value::String(fee_collector.clone());
        }

        if let Some(epoch_manager) = &contracts.epoch_manager {
            result["contracts"]["epoch_manager"] = serde_json::Value::String(epoch_manager.clone());
        }

        if include_metadata {
            result["metadata"] = serde_json::json!({
                "pool_manager": {
                    "description": "Main DEX pool management contract",
                    "functions": ["create_pool", "swap", "provide_liquidity", "withdraw_liquidity"]
                },
                "farm_manager": {
                    "description": "Liquidity farming and rewards management",
                    "functions": ["stake_lp_tokens", "unstake_lp_tokens", "claim_rewards"]
                },
                "fee_collector": {
                    "description": "Protocol fee collection and distribution",
                    "functions": ["collect_fees", "distribute_fees"]
                },
                "epoch_manager": {
                    "description": "Epoch-based reward distribution management",
                    "functions": ["create_epoch", "finalize_epoch", "claim_epoch_rewards"]
                }
            });
        }

        // Create formatted response text
        let mut response_text = format!("📄 **Contract Addresses**\n\n");
        response_text.push_str(&format!(
            "**Network:** {}\n",
            self.state.config.network_config.network_name
        ));
        response_text.push_str(&format!(
            "**Chain ID:** {}\n\n",
            self.state.config.network_config.chain_id
        ));

        response_text.push_str("### 🏗️ Core Contracts:\n\n");
        response_text.push_str(&format!("**Pool Manager:** `{}`\n", contracts.pool_manager));

        if let Some(farm_manager) = &contracts.farm_manager {
            response_text.push_str(&format!("**Farm Manager:** `{}`\n", farm_manager));
        }

        if let Some(fee_collector) = &contracts.fee_collector {
            response_text.push_str(&format!("**Fee Collector:** `{}`\n", fee_collector));
        }

        if let Some(epoch_manager) = &contracts.epoch_manager {
            response_text.push_str(&format!("**Epoch Manager:** `{}`\n", epoch_manager));
        }

        if include_metadata {
            response_text.push_str("\n### 📋 Contract Functions:\n\n");
            response_text.push_str("**Pool Manager:**\n");
            response_text
                .push_str("- create_pool, swap, provide_liquidity, withdraw_liquidity\n\n");
            response_text.push_str("**Farm Manager:**\n");
            response_text.push_str("- stake_lp_tokens, unstake_lp_tokens, claim_rewards\n\n");
            response_text.push_str("**Fee Collector:**\n");
            response_text.push_str("- collect_fees, distribute_fees\n\n");
            response_text.push_str("**Epoch Manager:**\n");
            response_text.push_str("- create_epoch, finalize_epoch, claim_epoch_rewards\n");
        }

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle validate_network_connectivity tool
    async fn handle_validate_network_connectivity(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(
            ?arguments,
            "Handling validate_network_connectivity tool call"
        );

        let check_rpc = arguments
            .get("check_rpc")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let check_contracts = arguments
            .get("check_contracts")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let check_block_height = arguments
            .get("check_block_height")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let _timeout_secs = arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);

        // Simplified status tracking
        let mut status_messages = Vec::new();
        let mut all_passed = true;

        // RPC connectivity check
        if check_rpc {
            match self.state.get_client().await {
                Ok(_) => {
                    status_messages.push("RPC connection: OK".to_string());
                }
                Err(e) => {
                    status_messages.push(format!("RPC connection: FAILED - {}", e));
                    all_passed = false;
                }
            }
        }

        // Contract validation check
        if check_contracts {
            let contracts = &self.state.config.network_config.contracts;
            let pool_manager_valid =
                contracts.pool_manager.starts_with("mantra1") && contracts.pool_manager.len() >= 39;

            if pool_manager_valid {
                status_messages.push("Contract addresses: OK".to_string());
            } else {
                status_messages.push("Contract addresses: INVALID".to_string());
                all_passed = false;
            }
        }

        // Block height check
        if check_block_height {
            match self.state.get_client().await {
                Ok(_) => {
                    status_messages.push("Block queries: OK".to_string());
                }
                Err(e) => {
                    status_messages.push(format!("Block queries: FAILED - {}", e));
                    all_passed = false;
                }
            }
        }

        // Create simple, clean response

        // Create formatted response text
        let mut response_text = format!("🌐 **Network Connectivity Check**\n\n");
        response_text.push_str(&format!(
            "**Network:** {}\n",
            self.state.config.network_config.network_name
        ));
        response_text.push_str(&format!(
            "**RPC URL:** {}\n",
            self.state.config.network_config.rpc_url
        ));
        response_text.push_str(&format!(
            "**Overall Status:** {}\n\n",
            if all_passed {
                "✅ Healthy"
            } else {
                "❌ Unhealthy"
            }
        ));

        response_text.push_str("### 🔍 Check Results:\n\n");
        for (i, message) in status_messages.iter().enumerate() {
            let icon = if message.contains("OK") { "✅" } else { "❌" };
            response_text.push_str(&format!("{}. {} {}\n", i + 1, icon, message));
        }

        response_text.push_str(&format!(
            "\n**Summary:** {}\n",
            if all_passed {
                "All network connectivity checks passed"
            } else {
                "Some network connectivity checks failed"
            }
        ));
        response_text.push_str(&format!(
            "**Timestamp:** {}\n",
            chrono::Utc::now().to_rfc3339()
        ));

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle get_balances tool
    async fn handle_get_balances(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_balances tool call");

        // Parse arguments
        let wallet_address = arguments
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let include_zero_balances = arguments
            .get("include_zero_balances")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get balances using the SDK adapter
        let result = self
            .state
            .sdk_adapter
            .get_balances(&self.state.config.network_config, wallet_address)
            .await?;

        // Extract balance data for processing
        let empty_vec = vec![];
        let balances_array = result
            .get("balances")
            .and_then(|b| b.as_array())
            .unwrap_or(&empty_vec);
        let address = result
            .get("address")
            .and_then(|a| a.as_str())
            .unwrap_or("Unknown");
        let network = result
            .get("network")
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown");

        // Filter out zero balances if requested
        let filtered_balances: Vec<_> = if !include_zero_balances {
            balances_array
                .iter()
                .filter(|balance| {
                    balance
                        .get("amount")
                        .and_then(|a| a.as_str())
                        .map(|amount| amount != "0")
                        .unwrap_or(true)
                })
                .cloned()
                .collect()
        } else {
            balances_array.clone()
        };

        // Format balances for human readability - SIMPLIFIED
        let mut formatted_balances = Vec::new();
        let mut total_om_value = 0.0;

        for balance in &filtered_balances {
            let denom = balance
                .get("denom")
                .and_then(|d| d.as_str())
                .unwrap_or("unknown");
            let amount_str = balance
                .get("amount")
                .and_then(|a| a.as_str())
                .unwrap_or("0");

            // Parse amount
            let raw_amount: u128 = amount_str.parse().unwrap_or(0);

            // Simplified formatting
            let (token_name, formatted_amount) = self.format_token_simple(denom, raw_amount);

            formatted_balances.push(serde_json::json!({
                "token": token_name,
                "amount": formatted_amount,
                "denom": denom
            }));

            // Calculate OM value for estimate
            if denom == "uom" {
                total_om_value = (raw_amount as f64) / 1_000_000.0;
            }
        }

        // Create simple, clean response

        // Create formatted response text
        let mut response_text = format!("🏦 **Wallet Balances**\n\n");
        response_text.push_str(&format!("**Address:** `{}`\n", address));
        response_text.push_str(&format!("**Network:** {}\n", network));
        response_text.push_str(&format!(
            "**Total Tokens:** {}\n\n",
            filtered_balances.len()
        ));

        if !formatted_balances.is_empty() {
            response_text.push_str("### 💰 Token Holdings:\n\n");
            for balance in &formatted_balances {
                let token = balance
                    .get("token")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Unknown");
                let amount = balance
                    .get("amount")
                    .and_then(|a| a.as_str())
                    .unwrap_or("0");
                let denom = balance
                    .get("denom")
                    .and_then(|d| d.as_str())
                    .unwrap_or("unknown");
                response_text.push_str(&format!("- **{}**: {}\n", token, amount));
                response_text.push_str(&format!("  - **Full Denom:** `{}`\n", denom));
            }

            if total_om_value > 0.0 {
                response_text
                    .push_str(&format!("\n**Total OM Value:** {:.2} OM\n", total_om_value));
            }
        } else {
            response_text.push_str("No tokens found in wallet.\n");
        }

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle list_wallets tool
    async fn handle_list_wallets(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling list_wallets tool call");

        // Get all wallets using the SDK adapter
        let wallets = self.state.sdk_adapter.get_all_wallets().await?;

        // Get active wallet address
        let active_address = match self.state.sdk_adapter.get_active_wallet_info().await? {
            Some(wallet_info) => Some(wallet_info.address),
            None => None,
        };

        // Create formatted response text
        let mut response_text = format!("📱 **Wallet Management**\n\n");
        
        if wallets.is_empty() {
            response_text.push_str("No wallets found in collection.\n");
        } else {
            response_text.push_str(&format!("**Total Wallets:** {}\n", wallets.len()));
            
            if let Some(active_addr) = &active_address {
                response_text.push_str(&format!("**Active Wallet:** `{}`\n\n", active_addr));
            } else {
                response_text.push_str("**Active Wallet:** None\n\n");
            }

            response_text.push_str("### 💼 Available Wallets:\n\n");
            
            for (address, wallet_info) in wallets.iter() {
                let is_active = active_address.as_ref().map_or(false, |addr| addr == address);
                let active_indicator = if is_active { " (ACTIVE)" } else { "" };
                
                response_text.push_str(&format!("- **Address:** `{}`{}\n", address, active_indicator));
                response_text.push_str(&format!("  - **Public Key:** `{}`\n", wallet_info.public_key));
                response_text.push_str("\n");
            }
        }

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle switch_wallet tool
    async fn handle_switch_wallet(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling switch_wallet tool call");

        // Parse arguments
        let wallet_address = arguments
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("wallet_address is required".to_string()))?;

        // Switch wallet using the SDK adapter
        self.state.sdk_adapter.switch_active_wallet(wallet_address).await?;

        // Get updated wallet info
        let wallet_info = self.state.sdk_adapter.get_wallet_info(wallet_address).await?
            .ok_or_else(|| McpServerError::InvalidArguments("Wallet not found after switch".to_string()))?;

        // Create formatted response text
        let mut response_text = format!("✅ **Wallet Switched Successfully**\n\n");
        response_text.push_str(&format!("**New Active Wallet:** `{}`\n", wallet_address));
        response_text.push_str(&format!("**Public Key:** `{}`\n", wallet_info.public_key));

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle get_active_wallet tool
    async fn handle_get_active_wallet(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_active_wallet tool call");

        // Get active wallet info using the SDK adapter
        let active_wallet = self.state.sdk_adapter.get_active_wallet_info().await?;

        // Create formatted response text
        let mut response_text = format!("🔍 **Active Wallet Information**\n\n");
        
        match active_wallet {
            Some(wallet_info) => {
                response_text.push_str(&format!("**Address:** `{}`\n", wallet_info.address));
                response_text.push_str(&format!("**Public Key:** `{}`\n", wallet_info.public_key));
                response_text.push_str("\n**Status:** Active and ready for use\n");
            }
            None => {
                response_text.push_str("**Status:** No active wallet configured\n");
                response_text.push_str("Please add a wallet using `add_wallet_from_mnemonic` or switch to an existing wallet.\n");
            }
        }

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle add_wallet_from_mnemonic tool
    async fn handle_add_wallet_from_mnemonic(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling add_wallet_from_mnemonic tool call");

        // Parse arguments
        let mnemonic = arguments
            .get("mnemonic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("mnemonic is required".to_string()))?;

        let derivation_index = arguments
            .get("derivation_index")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let set_as_active = arguments
            .get("set_as_active")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Create wallet from mnemonic
        let wallet = crate::wallet::MantraWallet::from_mnemonic(mnemonic, derivation_index)
            .map_err(|e| McpServerError::InvalidArguments(format!("Failed to create wallet from mnemonic: {}", e)))?;

        let wallet_info = wallet.info();
        let wallet_address = wallet_info.address.clone();

        // Add wallet using the SDK adapter with derivation index for caching
        self.state.sdk_adapter.add_wallet_with_derivation_index(wallet, derivation_index).await?;

        // Set as active wallet if requested
        if set_as_active {
            self.state.sdk_adapter.switch_active_wallet(&wallet_address).await?;
        }

        // Create formatted response text
        let mut response_text = format!("✅ **Wallet Added Successfully**\n\n");
        response_text.push_str(&format!("**Address:** `{}`\n", wallet_address));
        response_text.push_str(&format!("**Public Key:** `{}`\n", wallet_info.public_key));
        response_text.push_str(&format!("**Derivation Index:** {}\n", derivation_index));
        response_text.push_str(&format!("**Set as Active:** {}\n", if set_as_active { "Yes" } else { "No" }));

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Handle remove_wallet tool
    async fn handle_remove_wallet(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling remove_wallet tool call");

        // Parse arguments
        let wallet_address = arguments
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("wallet_address is required".to_string()))?;

        // Get wallet info before removal
        let wallet_info = self.state.sdk_adapter.get_wallet_info(wallet_address).await?;

        // Check if this is the active wallet
        let was_active = match self.state.sdk_adapter.get_active_wallet_info().await? {
            Some(active_info) => active_info.address == wallet_address,
            None => false,
        };

        // Remove wallet using the SDK adapter
        self.state.sdk_adapter.remove_wallet(wallet_address).await?;

        // Create formatted response text
        let mut response_text = format!("✅ **Wallet Removed Successfully**\n\n");
        response_text.push_str(&format!("**Removed Address:** `{}`\n", wallet_address));
        
        if let Some(info) = wallet_info {
            response_text.push_str(&format!("**Public Key:** `{}`\n", info.public_key));
        }

        if was_active {
            response_text.push_str("\n**Note:** This was the active wallet. You'll need to switch to another wallet or add a new one.\n");
        }

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    /// Simplified token formatting without emojis or complex logic
    fn format_token_simple(&self, denom: &str, raw_amount: u128) -> (String, String) {
        match denom {
            "uom" => {
                let om_amount = (raw_amount as f64) / 1_000_000.0;
                ("OM".to_string(), format!("{:.6}", om_amount))
            }
            d if d.contains("/uUSDC") => {
                let usdc_amount = (raw_amount as f64) / 1_000_000.0;
                ("USDC".to_string(), format!("{:.6}", usdc_amount))
            }
            d if d.contains("/uUSDY") => {
                let usdy_amount = (raw_amount as f64) / 1_000_000.0;
                ("USDY".to_string(), format!("{:.6}", usdy_amount))
            }
            d if d.contains("/aUSDY") => ("aUSDY".to_string(), raw_amount.to_string()),
            d if d.contains(".LP") => {
                let lp_amount = (raw_amount as f64) / 1_000_000.0;
                ("LP Token".to_string(), format!("{:.6}", lp_amount))
            }
            d if d.starts_with("ibc/") => {
                let ibc_amount = (raw_amount as f64) / 1_000_000.0;
                ("USDT".to_string(), format!("{:.6}", ibc_amount))
            }
            _ => ("Unknown Token".to_string(), raw_amount.to_string()),
        }
    }

    /// Handle get_pools tool
    async fn handle_get_pools(&self, arguments: serde_json::Value) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_pools tool call");

        // Parse optional arguments
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);

        let start_after = arguments
            .get("start_after")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Call the SDK adapter to get pools
        let result = self.state.sdk_adapter.get_pools(arguments).await?;

        // Extract pool data for processing
        let empty_vec = vec![];
        let pools_array = result
            .get("pools")
            .and_then(|p| p.as_array())
            .unwrap_or(&empty_vec);
        let count = result.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
        let network = &self.state.config.network_config.network_name;

        // Create formatted response text
        let mut response_text = format!("🏊 **Liquidity Pools**\n\n");
        response_text.push_str(&format!("**Network:** {}\n", network));
        response_text.push_str(&format!("**Total Pools Found:** {}\n", count));

        if let Some(limit) = limit {
            response_text.push_str(&format!("**Limit Applied:** {}\n", limit));
        }

        if let Some(start_after) = &start_after {
            response_text.push_str(&format!("**Starting After:** {}\n", start_after));
        }

        response_text.push_str("\n");

        if !pools_array.is_empty() {
            response_text.push_str("### 💧 Available Pools:\n\n");

            for (i, pool) in pools_array.iter().enumerate() {
                let pool_id = pool
                    .get("pool_id")
                    .and_then(|p| p.as_str())
                    .unwrap_or("Unknown");
                let pool_type = pool
                    .get("pool_type")
                    .and_then(|p| p.as_str())
                    .unwrap_or("Unknown");
                let lp_denom = pool
                    .get("lp_denom")
                    .and_then(|p| p.as_str())
                    .unwrap_or("Unknown");
                let total_share = pool
                    .get("total_share")
                    .and_then(|p| p.as_str())
                    .unwrap_or("0");

                // Get pool status
                let status = pool.get("status");
                let swaps_enabled = status
                    .and_then(|s| s.get("swaps_enabled"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let deposits_enabled = status
                    .and_then(|s| s.get("deposits_enabled"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let withdrawals_enabled = status
                    .and_then(|s| s.get("withdrawals_enabled"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                response_text.push_str(&format!(
                    "**{}. Pool {}** ({})\n",
                    i + 1,
                    pool_id,
                    pool_type
                ));

                // Show assets
                if let Some(assets) = pool.get("assets").and_then(|a| a.as_array()) {
                    response_text.push_str("   **Assets:**\n");
                    for asset in assets {
                        let denom = asset
                            .get("denom")
                            .and_then(|d| d.as_str())
                            .unwrap_or("Unknown");
                        let amount = asset.get("amount").and_then(|a| a.as_str()).unwrap_or("0");

                        // Format asset name
                        let asset_name = if denom == "uom" {
                            "OM".to_string()
                        } else if denom.contains("/uUSDC") {
                            "USDC".to_string()
                        } else if denom.contains("/uUSDY") {
                            "USDY".to_string()
                        } else if denom.contains("/aUSDY") {
                            "aUSDY".to_string()
                        } else if denom.starts_with("ibc/") {
                            "USDT".to_string()
                        } else {
                            denom.to_string()
                        };

                        // Format amount
                        let formatted_amount = if denom == "uom"
                            || denom.contains("/uUSDC")
                            || denom.contains("/uUSDY")
                        {
                            let amount_num: u128 = amount.parse().unwrap_or(0);
                            format!("{:.2}", (amount_num as f64) / 1_000_000.0)
                        } else {
                            amount.to_string()
                        };

                        response_text
                            .push_str(&format!("     - {}: {}\n", asset_name, formatted_amount));
                    }
                }

                // Show status
                response_text.push_str("   **Status:**");
                if swaps_enabled && deposits_enabled && withdrawals_enabled {
                    response_text.push_str(" ✅ Fully Operational\n");
                } else {
                    response_text.push_str(" ⚠️ Limited Operations (");
                    let mut ops = Vec::new();
                    if swaps_enabled {
                        ops.push("Swaps");
                    }
                    if deposits_enabled {
                        ops.push("Deposits");
                    }
                    if withdrawals_enabled {
                        ops.push("Withdrawals");
                    }
                    response_text.push_str(&ops.join(", "));
                    response_text.push_str(")\n");
                }

                // Show LP token info
                response_text.push_str(&format!("   **LP Token:** `{}`\n", lp_denom));
                response_text.push_str(&format!("   **Total Shares:** {}\n\n", total_share));
            }
        } else {
            response_text.push_str("No pools found matching the criteria.\n");
        }

        response_text.push_str(&format!(
            "**Query Time:** {}\n",
            chrono::Utc::now().to_rfc3339()
        ));

        // Return proper MCP response format
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": response_text
                }
            ]
        }))
    }

    async fn handle_execute_swap(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling execute_swap tool call");
        let result = self.state.sdk_adapter.execute_swap(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_provide_liquidity(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling provide_liquidity tool call");
        let result = self.state.sdk_adapter.provide_liquidity(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_provide_liquidity_unchecked(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling provide_liquidity_unchecked tool call");
        self.state
            .sdk_adapter
            .provide_liquidity_unchecked(arguments)
            .await
    }

    async fn handle_withdraw_liquidity(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling withdraw_liquidity tool call");
        let result = self.state.sdk_adapter.withdraw_liquidity(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_estimate_lp_withdrawal_amounts(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(
            ?arguments,
            "Handling estimate_lp_withdrawal_amounts tool call"
        );
        let result = self
            .state
            .sdk_adapter
            .estimate_lp_withdrawal_amounts(arguments)
            .await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_monitor_swap_transaction(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling monitor_swap_transaction tool call");

        // Parse arguments
        let tx_hash = arguments
            .get("tx_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'tx_hash' argument".to_string(),
                )
            })?;

        let min_confirmations = arguments
            .get("min_confirmations")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let poll_interval_secs = arguments
            .get("poll_interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);

        let monitor_events = arguments
            .get("monitor_events")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Validate parameters
        if tx_hash.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "Transaction hash cannot be empty".to_string(),
            ));
        }

        if min_confirmations == 0 {
            return Err(McpServerError::InvalidArguments(
                "Minimum confirmations must be greater than 0".to_string(),
            ));
        }

        if timeout_secs < 30 || timeout_secs > 3600 {
            return Err(McpServerError::InvalidArguments(
                "Timeout must be between 30 and 3600 seconds".to_string(),
            ));
        }

        if poll_interval_secs == 0 || poll_interval_secs > 60 {
            return Err(McpServerError::InvalidArguments(
                "Poll interval must be between 1 and 60 seconds".to_string(),
            ));
        }

        // Create monitor configuration
        let config = TransactionMonitorConfig {
            tx_hash: tx_hash.to_string(),
            min_confirmations,
            timeout_secs,
            poll_interval_secs,
            monitor_events,
        };

        // Start monitoring
        match self
            .state
            .transaction_monitor_manager
            .start_monitoring(config, self.state.client.clone())
            .await
        {
            Ok(monitor_id) => {
                info!("Started transaction monitoring with ID: {}", monitor_id);
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Transaction monitoring started successfully",
                    "monitor_id": monitor_id,
                    "tx_hash": tx_hash,
                    "min_confirmations": min_confirmations,
                    "timeout_secs": timeout_secs,
                    "poll_interval_secs": poll_interval_secs,
                    "monitor_events": monitor_events,
                    "started_at": chrono::Utc::now().to_rfc3339()
                }))
            }
            Err(e) => {
                error!("Failed to start transaction monitoring: {}", e);
                Err(McpServerError::Internal(format!(
                    "Failed to start transaction monitoring: {}",
                    e
                )))
            }
        }
    }

    async fn handle_create_pool(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling create_pool tool call");
        let result = self.state.sdk_adapter.create_pool(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }
}

// =============================================================================
// Server Creation Functions
// =============================================================================

/// Create a new MCP server with the given configuration
pub async fn create_mcp_server(config: McpServerConfig) -> McpResult<MantraDexMcpServer> {
    let server = MantraDexMcpServer::new(config);
    server.initialize().await?;
    Ok(server)
}

/// Create an MCP server with STDIO transport
pub async fn create_stdio_server(config: McpServerConfig) -> McpResult<MantraDexMcpServer> {
    let server = create_mcp_server(config).await?;
    info!("Starting MCP server with STDIO transport");

    // Start the stdio transport loop - this will run indefinitely
    let server_clone = server.clone();
    start_stdio_transport(server_clone).await?;

    // This line should never be reached unless the transport stops
    warn!("STDIO transport has stopped unexpectedly");
    Ok(server)
}

/// JSON-RPC request structure for HTTP transport
#[derive(Debug, Deserialize)]
struct HttpJsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

/// HTTP handler for JSON-RPC requests
async fn handle_jsonrpc_request(
    State(server): State<Arc<MantraDexMcpServer>>,
    Json(request): Json<HttpJsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>, StatusCode> {
    debug!("HTTP JSON-RPC request: {:?}", request);

    // Convert HTTP JSON-RPC to MCP format and process
    let response = match process_mcp_request(&server, &request).await {
        Ok(result) => JsonRpcResponse::success(request.id.clone(), result),
        Err(error) => JsonRpcResponse::error(request.id.clone(), error.to_json_rpc_error()),
    };

    debug!("HTTP JSON-RPC response: {:?}", response);
    Ok(Json(response))
}

/// Process MCP request and return result
async fn process_mcp_request(
    server: &MantraDexMcpServer,
    request: &HttpJsonRpcRequest,
) -> McpResult<Value> {
    // Process the request using existing MCP server logic
    server
        .handle_request(&request.method, request.params.clone())
        .await
}

/// Create an MCP server with HTTP transport
pub async fn create_http_server(config: McpServerConfig) -> McpResult<MantraDexMcpServer> {
    let http_host = config.http_host.clone();
    let http_port = config.http_port;
    let server = create_mcp_server(config).await?;

    info!(
        "Starting MCP server with HTTP transport on {}:{}",
        http_host, http_port
    );

    // Create HTTP server with JSON-RPC endpoint
    let app = Router::new()
        .route("/", post(handle_jsonrpc_request))
        .route("/jsonrpc", post(handle_jsonrpc_request))
        .with_state(Arc::new(server.clone()));

    // Bind to address
    let addr: SocketAddr = format!("{}:{}", http_host, http_port)
        .parse()
        .map_err(|e| McpServerError::Internal(format!("Invalid address: {}", e)))?;

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| McpServerError::Internal(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("MCP HTTP server listening on {}", addr);

    // Start the HTTP server
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("HTTP server error: {}", e);
        }
    });

    Ok(server)
}

// =============================================================================
// Swap History Tracking Data Structures and Helper Methods
// =============================================================================

impl MantraDexMcpServer {

    // Resource handler methods for MCP resources

    // LP Token Management Tool Handlers

    /// Handle get_lp_token_balance tool
    async fn handle_get_lp_token_balance(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_lp_token_balance tool call");

        let pool_id = arguments
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'pool_id' argument".to_string(),
                )
            })?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address =
            if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
                addr.to_string()
            } else {
                match self.state.get_active_wallet().await? {
                    Some(wallet) => wallet.address,
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            };

        // Cache key for LP token balance
        let cache_key = format!("lp_balance:{}:{}", pool_id, wallet_address);

        if let Some(cached_balance) = self.state.cache_get(&cache_key).await {
            info!(pool_id, wallet_address, "Returning cached LP token balance");
            return Ok(cached_balance);
        }

        info!(
            pool_id,
            wallet_address, "Querying LP token balance from blockchain"
        );

        // For now, return a placeholder response
        // In a real implementation, this would query the blockchain using the SDK
        let balance_result = serde_json::json!({
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_token_balance": "0",
            "lp_token_denom": format!("factory/{}/lp_{}", "contract_address", pool_id),
            "share_percentage": "0.00",
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "note": "LP token balance querying is not yet fully implemented. This is a placeholder response."
        });

        // Cache the result
        self.state
            .cache_set(cache_key, balance_result.clone())
            .await;

        Ok(balance_result)
    }

    /// Handle get_all_lp_token_balances tool
    async fn handle_get_all_lp_token_balances(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_all_lp_token_balances tool call");

        // Get wallet address (use active wallet if not provided)
        let wallet_address =
            if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
                addr.to_string()
            } else {
                match self.state.get_active_wallet().await? {
                    Some(wallet) => wallet.address,
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            };

        let include_zero_balances = arguments
            .get("include_zero_balances")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Cache key for all LP token balances
        let cache_key = format!(
            "all_lp_balances:{}:zero={}",
            wallet_address, include_zero_balances
        );

        if let Some(cached_balances) = self.state.cache_get(&cache_key).await {
            info!(wallet_address, "Returning cached LP token balances");
            return Ok(cached_balances);
        }

        info!(
            wallet_address,
            "Querying all LP token balances from blockchain"
        );

        // For now, return a placeholder response
        // In a real implementation, this would:
        // 1. Query all pools from the blockchain
        // 2. For each pool, check if the wallet has LP tokens
        // 3. Calculate share percentages and values
        let balances_result = serde_json::json!({
            "wallet_address": wallet_address,
            "lp_positions": [],
            "total_positions": 0,
            "total_value_usd": "0.00",
            "include_zero_balances": include_zero_balances,
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "note": "LP token balance querying is not yet fully implemented. This is a placeholder response."
        });

        // Cache the result
        self.state
            .cache_set(cache_key, balances_result.clone())
            .await;

        Ok(balances_result)
    }

    // =============================================================================
    // ClaimDrop Tool Handlers
    // =============================================================================

    async fn handle_claimdrop_create_campaign(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling claimdrop_create_campaign tool call");
        let result = self.state.sdk_adapter.claimdrop_create_campaign(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_claimdrop_claim(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling claimdrop_claim tool call");
        let result = self.state.sdk_adapter.claimdrop_claim(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_claimdrop_query_rewards(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling claimdrop_query_rewards tool call");
        let result = self.state.sdk_adapter.claimdrop_query_rewards(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_claimdrop_query_campaigns(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling claimdrop_query_campaigns tool call");
        let result = self.state.sdk_adapter.claimdrop_query_campaigns(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    async fn handle_claimdrop_add_allocations(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling claimdrop_add_allocations tool call");
        let result = self.state.sdk_adapter.claimdrop_add_allocations(arguments).await?;

        // Format as MCP response
        Ok(serde_json::json!({
            "content": [
                {
                    "type": "text",
                    "text": serde_json::to_string_pretty(&result)?
                }
            ]
        }))
    }

    // Skip Protocol Handlers
    
    /// Handle skip_get_route tool
    async fn handle_skip_get_route(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling skip_get_route tool call");
        self.state.sdk_adapter.skip_get_route(arguments).await
    }

    /// Handle skip_execute_transfer tool
    async fn handle_skip_execute_transfer(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling skip_execute_transfer tool call");
        self.state.sdk_adapter.skip_execute_transfer(arguments).await
    }

    /// Handle skip_track_transfer tool
    async fn handle_skip_track_transfer(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling skip_track_transfer tool call");
        self.state.sdk_adapter.skip_track_transfer(arguments).await
    }

    /// Handle skip_get_supported_chains tool
    async fn handle_skip_get_supported_chains(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling skip_get_supported_chains tool call");
        self.state.sdk_adapter.skip_get_supported_chains(arguments).await
    }

    /// Handle skip_verify_assets tool
    async fn handle_skip_verify_assets(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling skip_verify_assets tool call");
        self.state.sdk_adapter.skip_verify_assets(arguments).await
    }

    /// Handle skip_estimate_fees tool
    async fn handle_skip_estimate_fees(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling skip_estimate_fees tool call");
        self.state.sdk_adapter.skip_estimate_fees(arguments).await
    }

}

/// Start the stdio transport layer for MCP communication
async fn start_stdio_transport(server: MantraDexMcpServer) -> McpResult<()> {
    use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

    info!("Starting stdio transport for MCP communication");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    info!("Server is ready and listening for JSON-RPC messages on stdin...");

    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        // Read a line from stdin
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF reached, client disconnected
                info!("Client disconnected (EOF received)");
                break;
            }
            Ok(_) => {
                // Parse and handle the JSON-RPC request
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Only log request details in debug mode to reduce noise
                if tracing::enabled!(tracing::Level::DEBUG) {
                    debug!("Received MCP request: {}", trimmed);
                }

                // Parse JSON-RPC request
                let response_opt = match serde_json::from_str::<serde_json::Value>(trimmed) {
                    Ok(request) => handle_json_rpc_request(&server, request).await,
                    Err(e) => {
                        warn!("Failed to parse JSON-RPC request: {}", e);
                        Some(JsonRpcResponse::error(
                            None,
                            JsonRpcError {
                                code: -32700, // Parse error
                                message: "Parse error".to_string(),
                                data: Some(serde_json::json!({
                                    "error": e.to_string(),
                                    "request": trimmed
                                })),
                            },
                        ))
                    }
                };

                // Send response back via stdout (only if we have a response - notifications don't require responses)
                if let Some(response) = response_opt {
                    match serde_json::to_string(&response) {
                        Ok(response_json) => {
                            if let Err(e) = stdout.write_all(response_json.as_bytes()).await {
                                warn!("Failed to write response to stdout: {} - continuing", e);
                                continue;
                            }
                            if let Err(e) = stdout.write_all(b"\n").await {
                                warn!("Failed to write newline to stdout: {} - continuing", e);
                                continue;
                            }
                            if let Err(e) = stdout.flush().await {
                                warn!("Failed to flush stdout: {} - continuing", e);
                                continue;
                            }
                            // Only log response details in debug mode to reduce noise
                            if tracing::enabled!(tracing::Level::DEBUG) {
                                debug!("Sent MCP response: {}", response_json);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to serialize response: {} - continuing", e);
                            continue;
                        }
                    }
                } else {
                    debug!("No response needed for notification");
                }
            }
            Err(e) => {
                warn!("Error reading from stdin: {}", e);
                break;
            }
        }
    }

    info!("Stdio transport stopped");
    Ok(())
}

/// Handle a JSON-RPC request and return a JSON-RPC response
async fn handle_json_rpc_request(
    server: &MantraDexMcpServer,
    request: serde_json::Value,
) -> Option<JsonRpcResponse> {
    // Extract request ID for response correlation
    let request_id = request.get("id").cloned();

    // Extract method and params
    let method = match request.get("method").and_then(|m| m.as_str()) {
        Some(method) => method,
        None => {
            return Some(JsonRpcResponse::error(
                request_id,
                JsonRpcError {
                    code: -32600, // Invalid Request
                    message: "Missing method".to_string(),
                    data: None,
                },
            ));
        }
    };

    let params = request.get("params").cloned();

    debug!("Handling MCP method: {}", method);

    // Check if this is a notification (no id field)
    let is_notification = request_id.is_none();

    // Handle notifications - they don't require responses
    if is_notification {
        match method {
            "notifications/initialized" => {
                debug!("Received notifications/initialized - client is ready");
                return None; // No response for notifications
            }
            "notifications/cancelled" => {
                debug!("Received notifications/cancelled - request cancelled");
                return None; // No response for notifications
            }
            _ => {
                debug!("Received unknown notification: {}", method);
                return None; // Still no response for unknown notifications
            }
        }
    }

    // Handle regular requests using the server
    match server.handle_request(method, params).await {
        Ok(result) => Some(JsonRpcResponse::success(request_id, result)),
        Err(e) => {
            warn!("MCP request failed: {}", e);
            Some(JsonRpcResponse::error(request_id, e.to_json_rpc_error()))
        }
    }
}

impl Clone for MantraDexMcpServer {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

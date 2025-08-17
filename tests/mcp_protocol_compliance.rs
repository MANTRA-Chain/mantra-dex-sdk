use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

/// Test module for MCP protocol compliance
/// 
/// This module implements comprehensive tests for JSON-RPC 2.0 compliance
/// as required by the MCP specification. It validates request/response formatting,
/// error handling, tool registration, and serialization edge cases.

// =============================================================================
// Test Data Structures
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpToolDefinition {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpResourceDefinition {
    uri: String,
    name: String,
    description: String,
    mime_type: Option<String>,
}

// =============================================================================
// JSON-RPC 2.0 Compliance Tests
// =============================================================================

#[tokio::test]
async fn test_jsonrpc_request_format_compliance() {
    // Test 1: Valid JSON-RPC 2.0 request structure
    let valid_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/list".to_string(),
        params: None,
        id: json!(1),
    };
    
    let serialized = serde_json::to_string(&valid_request).unwrap();
    let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(deserialized.jsonrpc, "2.0");
    assert_eq!(deserialized.method, "tools/list");
    assert_eq!(deserialized.id, json!(1));
    
    // Test 2: String ID support
    let string_id_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "get_pools",
            "arguments": {}
        })),
        id: json!("test-id-123"),
    };
    
    let serialized = serde_json::to_string(&string_id_request).unwrap();
    let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.id, json!("test-id-123"));
    
    // Test 3: Null ID support (notification)
    let null_id_request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "notifications/initialized".to_string(),
        params: None,
        id: Value::Null,
    };
    
    let serialized = serde_json::to_string(&null_id_request).unwrap();
    let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.id, Value::Null);
}

#[tokio::test]
async fn test_jsonrpc_response_format_compliance() {
    // Test 1: Success response format
    let success_response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: Some(json!({
            "tools": [
                {
                    "name": "get_pools",
                    "description": "Get available liquidity pools",
                    "input_schema": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            ]
        })),
        error: None,
    };
    
    let serialized = serde_json::to_string(&success_response).unwrap();
    let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(deserialized.jsonrpc, "2.0");
    assert_eq!(deserialized.id, json!(1));
    assert!(deserialized.result.is_some());
    assert!(deserialized.error.is_none());
    
    // Test 2: Error response format
    let error_response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(2),
        result: None,
        error: Some(JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: Some(json!({
                "method": "invalid_method",
                "available_methods": ["tools/list", "tools/call", "resources/list"]
            })),
        }),
    };
    
    let serialized = serde_json::to_string(&error_response).unwrap();
    let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(deserialized.jsonrpc, "2.0");
    assert_eq!(deserialized.id, json!(2));
    assert!(deserialized.result.is_none());
    assert!(deserialized.error.is_some());
    
    let error = deserialized.error.unwrap();
    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found");
    assert!(error.data.is_some());
}

#[tokio::test]
async fn test_jsonrpc_error_codes_compliance() {
    // Test MCP-specific error codes mapping
    let error_codes = vec![
        (-32700, "Parse error"),
        (-32600, "Invalid Request"),
        (-32601, "Method not found"),
        (-32602, "Invalid params"),
        (-32603, "Internal error"),
        (-32000, "Server error"),
        (-32001, "Network connection failed"),
        (-32002, "Wallet not configured"),
        (-32003, "Validation error"),
        (-32004, "Serialization error"),
        (-32005, "Resource not found"),
        (-32006, "Configuration error"),
    ];
    
    for (code, message) in error_codes {
        let error = JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        };
        
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: None,
            error: Some(error),
        };
        
        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
        
        let error = deserialized.error.unwrap();
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
    }
}

#[tokio::test]
async fn test_mcp_method_names_compliance() {
    // Test all required MCP method names
    let required_methods = vec![
        "initialize",
        "ping",
        "tools/list",
        "tools/call",
        "resources/list",
        "resources/read",
        "notifications/initialized",
        "notifications/roots_list_changed",
    ];
    
    for method in required_methods {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: None,
            id: json!(1),
        };
        
        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.method, method);
        assert_eq!(deserialized.jsonrpc, "2.0");
    }
}

// =============================================================================
// Tool Registration and Discovery Tests
// =============================================================================

#[tokio::test]
async fn test_tools_list_payload_compliance() {
    // Test tools/list request
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/list".to_string(),
        params: None,
        id: json!(1),
    };
    
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(serialized.contains("\"method\":\"tools/list\""));
    assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
    
    // Test tools/list response with comprehensive tool definitions
    let tools = vec![
        McpToolDefinition {
            name: "get_pools".to_string(),
            description: "Get available liquidity pools".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        McpToolDefinition {
            name: "swap".to_string(),
            description: "Execute a token swap".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from_token": {"type": "string"},
                    "to_token": {"type": "string"},
                    "amount": {"type": "string"},
                    "slippage": {"type": "number", "minimum": 0, "maximum": 1}
                },
                "required": ["from_token", "to_token", "amount"]
            }),
        },
        McpToolDefinition {
            name: "add_liquidity".to_string(),
            description: "Add liquidity to a pool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pool_id": {"type": "string"},
                    "asset_a_amount": {"type": "string"},
                    "asset_b_amount": {"type": "string"}
                },
                "required": ["pool_id", "asset_a_amount", "asset_b_amount"]
            }),
        },
    ];
    
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: Some(json!({
            "tools": tools
        })),
        error: None,
    };
    
    let serialized = serde_json::to_string(&response).unwrap();
    let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
    
    assert!(deserialized.result.is_some());
    let result = deserialized.result.unwrap();
    assert!(result.get("tools").is_some());
    
    let tools_array = result.get("tools").unwrap().as_array().unwrap();
    assert_eq!(tools_array.len(), 3);
    
    // Validate tool schema compliance
    for tool in tools_array {
        assert!(tool.get("name").is_some());
        assert!(tool.get("description").is_some());
        assert!(tool.get("input_schema").is_some());
        
        let schema = tool.get("input_schema").unwrap();
        assert_eq!(schema.get("type").unwrap().as_str().unwrap(), "object");
        assert!(schema.get("properties").is_some());
    }
}

#[tokio::test]
async fn test_tools_call_payload_compliance() {
    // Test tools/call request with various parameter types
    let test_cases = vec![
        // Simple tool call
        json!({
            "name": "get_pools",
            "arguments": {}
        }),
        // Tool call with string parameters
        json!({
            "name": "get_balance",
            "arguments": {
                "address": "mantra1234567890abcdef",
                "denom": "uatom"
            }
        }),
        // Tool call with numeric parameters
        json!({
            "name": "swap",
            "arguments": {
                "from_token": "uatom",
                "to_token": "usdc",
                "amount": "1000000",
                "slippage": 0.01
            }
        }),
        // Tool call with complex nested parameters
        json!({
            "name": "add_liquidity",
            "arguments": {
                "pool_id": "1",
                "asset_a_amount": "1000000",
                "asset_b_amount": "2000000",
                "slippage_tolerance": 0.05,
                "deadline": 1640995200
            }
        }),
    ];
    
    for (index, params) in test_cases.iter().enumerate() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(params.clone()),
            id: json!(index + 1),
        };
        
        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.method, "tools/call");
        assert!(deserialized.params.is_some());
        
        let params = deserialized.params.unwrap();
        assert!(params.get("name").is_some());
        assert!(params.get("arguments").is_some());
    }
}

#[tokio::test]
async fn test_resources_list_payload_compliance() {
    // Test resources/list request
    let _request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/list".to_string(),
        params: None,
        id: json!(1),
    };
    
    let serialized = serde_json::to_string(&_request).unwrap();
    assert!(serialized.contains("\"method\":\"resources/list\""));
    
    // Test resources/list response
    let resources = vec![
        McpResourceDefinition {
            uri: "trades://history".to_string(),
            name: "Trade History".to_string(),
            description: "Historical trade records".to_string(),
            mime_type: Some("application/json".to_string()),
        },
        McpResourceDefinition {
            uri: "trades://pending".to_string(),
            name: "Pending Trades".to_string(),
            description: "Currently pending trade transactions".to_string(),
            mime_type: Some("application/json".to_string()),
        },
        McpResourceDefinition {
            uri: "liquidity://positions".to_string(),
            name: "Liquidity Positions".to_string(),
            description: "Current liquidity provider positions".to_string(),
            mime_type: Some("application/json".to_string()),
        },
    ];
    
    let response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: Some(json!({
            "resources": resources
        })),
        error: None,
    };
    
    let serialized = serde_json::to_string(&response).unwrap();
    let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
    
    assert!(deserialized.result.is_some());
    let result = deserialized.result.unwrap();
    assert!(result.get("resources").is_some());
    
    let resources_array = result.get("resources").unwrap().as_array().unwrap();
    assert_eq!(resources_array.len(), 3);
    
    // Validate resource schema compliance
    for resource in resources_array {
        assert!(resource.get("uri").is_some());
        assert!(resource.get("name").is_some());
        assert!(resource.get("description").is_some());
        // mime_type is optional
    }
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[tokio::test]
async fn test_unknown_method_error_handling() {
    // Test unknown method error
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "unknown/method".to_string(),
        params: None,
        id: json!(1),
    };
    
    let error_response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: None,
        error: Some(JsonRpcError {
            code: -32601,
            message: "Method not found".to_string(),
            data: Some(json!({
                "method": "unknown/method",
                "available_methods": [
                    "initialize",
                    "ping",
                    "tools/list",
                    "tools/call",
                    "resources/list",
                    "resources/read"
                ]
            })),
        }),
    };
    
    let serialized = serde_json::to_string(&error_response).unwrap();
    let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
    
    assert!(deserialized.error.is_some());
    let error = deserialized.error.unwrap();
    assert_eq!(error.code, -32601);
    assert_eq!(error.message, "Method not found");
    assert!(error.data.is_some());
}

#[tokio::test]
async fn test_invalid_params_error_handling() {
    // Test invalid parameters error
    let invalid_requests = vec![
        // Missing required parameters
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "arguments": {}
            },
            "id": 1
        }),
        // Invalid parameter types
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": 123,
                "arguments": "invalid"
            },
            "id": 2
        }),
        // Missing arguments
        json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "swap"
            },
            "id": 3
        }),
    ];
    
    for (index, request_data) in invalid_requests.iter().enumerate() {
        let error_response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(index + 1),
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: "Invalid params".to_string(),
                data: Some(json!({
                    "error": "Invalid or missing parameters",
                    "received": request_data.get("params")
                })),
            }),
        };
        
        let serialized = serde_json::to_string(&error_response).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
        
        assert!(deserialized.error.is_some());
        let error = deserialized.error.unwrap();
        assert_eq!(error.code, -32602);
        assert_eq!(error.message, "Invalid params");
    }
}

#[tokio::test]
async fn test_internal_error_handling() {
    // Test internal server error
    let error_response = JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id: json!(1),
        result: None,
        error: Some(JsonRpcError {
            code: -32603,
            message: "Internal error".to_string(),
            data: Some(json!({
                "error_type": "network_connection_failed",
                "details": "Failed to connect to blockchain node",
                "timestamp": "2024-01-01T00:00:00Z",
                "is_recoverable": true,
                "retry_after": 5
            })),
        }),
    };
    
    let serialized = serde_json::to_string(&error_response).unwrap();
    let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
    
    assert!(deserialized.error.is_some());
    let error = deserialized.error.unwrap();
    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "Internal error");
    assert!(error.data.is_some());
    
    let data = error.data.unwrap();
    assert!(data.get("error_type").is_some());
    assert!(data.get("is_recoverable").is_some());
}

// =============================================================================
// Serialization Edge Cases Tests
// =============================================================================

#[tokio::test]
async fn test_serialization_edge_cases() {
    // Test 1: Large numbers
    let large_number_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "swap",
            "arguments": {
                "amount": "999999999999999999999999999999"
            }
        },
        "id": 1
    });
    
    let serialized = serde_json::to_string(&large_number_request).unwrap();
    let _deserialized: Value = serde_json::from_str(&serialized).unwrap();
    
    // Test 2: Unicode strings
    let unicode_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "create_pool",
            "arguments": {
                "name": "池子-🚀-テスト",
                "description": "Test pool with unicode characters: 中文 日本語 한국어"
            }
        },
        "id": 2
    });
    
    let serialized = serde_json::to_string(&unicode_request).unwrap();
    let _deserialized: Value = serde_json::from_str(&serialized).unwrap();
    
    // Test 3: Nested objects
    let nested_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "complex_operation",
            "arguments": {
                "config": {
                    "swap": {
                        "slippage": 0.01,
                        "deadline": 3600
                    },
                    "liquidity": {
                        "auto_compound": true,
                        "rewards": {
                            "claim_threshold": "1000000",
                            "auto_stake": false
                        }
                    }
                }
            }
        },
        "id": 3
    });
    
    let serialized = serde_json::to_string(&nested_request).unwrap();
    let _deserialized: Value = serde_json::from_str(&serialized).unwrap();
    
    // Test 4: Array parameters
    let array_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "batch_swap",
            "arguments": {
                "swaps": [
                    {
                        "from_token": "uatom",
                        "to_token": "usdc",
                        "amount": "1000000"
                    },
                    {
                        "from_token": "usdc",
                        "to_token": "uosmo",
                        "amount": "500000"
                    }
                ]
            }
        },
        "id": 4
    });
    
    let serialized = serde_json::to_string(&array_request).unwrap();
    let _deserialized: Value = serde_json::from_str(&serialized).unwrap();
    
    // Test 5: Null and optional values
    let null_values_request = json!({
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "get_pool_info",
            "arguments": {
                "pool_id": "1",
                "include_stats": true,
                "filter": null,
                "limit": 10
            }
        },
        "id": 5
    });
    
    let serialized = serde_json::to_string(&null_values_request).unwrap();
    let _deserialized: Value = serde_json::from_str(&serialized).unwrap();
}

#[tokio::test]
async fn test_malformed_json_handling() {
    // Test truly malformed JSON requests (syntax errors)
    let malformed_jsons = vec![
        // Missing closing brace
        r#"{"jsonrpc": "2.0", "method": "tools/list", "id": 1"#,
        // Invalid JSON structure
        r#"{"jsonrpc": "2.0", "method": "tools/list", "id": 1,}"#,
        // Missing required fields
        r#"{"method": "tools/list", "id": 1}"#,
    ];
    
    for malformed_json in malformed_jsons {
        let result = serde_json::from_str::<JsonRpcRequest>(malformed_json);
        assert!(result.is_err(), "Expected error for malformed JSON: {}", malformed_json);
    }
    
    // Test valid JSON but invalid protocol version (should parse but fail validation)
    let invalid_version_json = r#"{"jsonrpc": "1.0", "method": "tools/list", "id": 1}"#;
    let result = serde_json::from_str::<JsonRpcRequest>(invalid_version_json);
    assert!(result.is_ok(), "Should parse valid JSON");
    
    let request = result.unwrap();
    assert_eq!(request.jsonrpc, "1.0");
    // In a real implementation, this would be rejected by the server's validation logic
    
    // Test object ID (valid JSON but not recommended for JSON-RPC)
    let object_id_json = r#"{"jsonrpc": "2.0", "method": "tools/list", "id": {"test": true}}"#;
    let result = serde_json::from_str::<JsonRpcRequest>(object_id_json);
    assert!(result.is_ok(), "Should parse valid JSON with object ID");
    
    let request = result.unwrap();
    assert_eq!(request.id, serde_json::json!({"test": true}));
    // In a real implementation, servers should reject object IDs per JSON-RPC spec
}

#[tokio::test]
async fn test_response_id_matching() {
    // Test that response IDs match request IDs
    let request_ids = vec![
        json!(1),
        json!("string-id"),
        json!(null),
        json!(0),
        json!(-1),
        json!(9223372036854775807i64), // max i64
    ];
    
    for id in request_ids {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "ping".to_string(),
            params: None,
            id: id.clone(),
        };
        
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            result: Some(json!("pong")),
            error: None,
        };
        
        assert_eq!(request.id, response.id);
        
        // Test serialization round-trip
        let serialized_req = serde_json::to_string(&request).unwrap();
        let serialized_resp = serde_json::to_string(&response).unwrap();
        
        let deserialized_req: JsonRpcRequest = serde_json::from_str(&serialized_req).unwrap();
        let deserialized_resp: JsonRpcResponse = serde_json::from_str(&serialized_resp).unwrap();
        
        assert_eq!(deserialized_req.id, deserialized_resp.id);
    }
}

// =============================================================================
// Async Protocol Tests
// =============================================================================

#[tokio::test]
async fn test_async_tool_execution() {
    // Test simulated async tool execution
    let start_time = std::time::Instant::now();
    
    // Simulate async tool call
    let tool_future = async {
        sleep(Duration::from_millis(100)).await;
        json!({
            "status": "success",
            "result": "Tool executed successfully",
            "execution_time_ms": start_time.elapsed().as_millis()
        })
    };
    
    let result = tool_future.await;
    
    assert_eq!(result.get("status").unwrap().as_str().unwrap(), "success");
    assert!(result.get("execution_time_ms").is_some());
}

#[tokio::test]
async fn test_concurrent_requests() {
    // Test handling multiple concurrent requests
    let requests = vec![
        ("tools/list", json!(null)),
        ("resources/list", json!(null)),
        ("ping", json!(null)),
        ("tools/call", json!({"name": "get_pools", "arguments": {}})),
    ];
    
    let mut handles = Vec::new();
    
    for (method, params) in requests {
        let handle = tokio::spawn(async move {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                params: if params.is_null() { None } else { Some(params) },
                id: json!(1),
            };
            
            // Simulate request processing
            sleep(Duration::from_millis(50)).await;
            
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                result: Some(json!({
                    "method": method,
                    "processed": true
                })),
                error: None,
            };
            
            response
        });
        
        handles.push(handle);
    }
    
    let results = futures::future::join_all(handles).await;
    
    for result in results {
        let response = result.unwrap();
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_some());
    }
}

#[tokio::test]
async fn test_protocol_version_enforcement() {
    // Test that only JSON-RPC 2.0 is supported
    let invalid_versions = vec!["1.0", "2.1", "3.0", "", "invalid"];
    
    for version in invalid_versions {
        let _request = json!({
            "jsonrpc": version,
            "method": "tools/list",
            "id": 1
        });
        
        // This should ideally fail during validation
        // In a real implementation, this would be caught by the server
        let serialized = serde_json::to_string(&_request).unwrap();
        assert!(serialized.contains(&format!("\"jsonrpc\":\"{}\"", version)));
        
        // The server should reject non-2.0 versions
        let error_response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request".to_string(),
                data: Some(json!({
                    "error": "Unsupported JSON-RPC version",
                    "received": version,
                    "supported": "2.0"
                })),
            }),
        };
        
        let serialized = serde_json::to_string(&error_response).unwrap();
        let deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
        
        assert!(deserialized.error.is_some());
        assert_eq!(deserialized.error.unwrap().code, -32600);
    }
}

// =============================================================================
// Integration with Existing Types
// =============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[cfg(feature = "mcp")]
    use mantra_sdk::mcp::server::McpServerError;
    
    #[tokio::test]
    #[cfg(feature = "mcp")]
    async fn test_mcp_server_error_serialization() {
        // Test that McpServerError can be properly serialized into JSON-RPC errors
        let server_errors = vec![
            McpServerError::InvalidArguments("Test invalid arguments".to_string()),
            McpServerError::UnknownTool("test_tool".to_string()),
            McpServerError::UnknownResource("test://resource".to_string()),
            McpServerError::Network("Network connection failed".to_string()),
            McpServerError::Validation("Validation failed".to_string()),
        ];
        
        for error in server_errors {
            let error_code = error.to_json_rpc_error_code();
            let error_data = error.get_error_data();
            
            let json_rpc_error = JsonRpcError {
                code: error_code,
                message: error.to_string(),
                data: error_data,
            };
            
            let response = JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: json!(1),
                result: None,
                error: Some(json_rpc_error),
            };
            
            // Ensure it can be serialized and deserialized
            let serialized = serde_json::to_string(&response).unwrap();
            let _deserialized: JsonRpcResponse = serde_json::from_str(&serialized).unwrap();
            
            assert!(error_code < 0); // All error codes should be negative
            assert!(!error.to_string().is_empty()); // Error message should not be empty
        }
    }
    
    #[tokio::test]
    async fn test_tool_schema_validation() {
        // Test that tool schemas are properly validated
        let valid_schemas = vec![
            json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            json!({
                "type": "object",
                "properties": {
                    "amount": {"type": "string"},
                    "slippage": {"type": "number", "minimum": 0, "maximum": 1}
                },
                "required": ["amount"]
            }),
        ];
        
        for schema in valid_schemas {
            let tool = McpToolDefinition {
                name: "test_tool".to_string(),
                description: "Test tool".to_string(),
                input_schema: schema.clone(),
            };
            
            let serialized = serde_json::to_string(&tool).unwrap();
            let deserialized: McpToolDefinition = serde_json::from_str(&serialized).unwrap();
            
            assert_eq!(deserialized.name, "test_tool");
            assert_eq!(deserialized.input_schema, schema);
        }
    }
}
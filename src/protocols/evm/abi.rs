#[cfg(feature = "evm")]
use crate::error::Error;
/// ABI Loader and Registry for EVM contracts
///
/// Provides functionality to load, cache, and validate Ethereum contract ABIs
/// from JSON files, with support for function and event encoding/decoding.
#[cfg(feature = "evm")]
use alloy_json_abi::{Event, Function, JsonAbi};
#[cfg(feature = "evm")]
use std::collections::HashMap;
#[cfg(feature = "evm")]
use std::fs;
#[cfg(feature = "evm")]
use std::path::Path;

/// Contract ABI registry for caching loaded ABIs
#[cfg(feature = "evm")]
#[derive(Debug, Clone)]
pub struct AbiRegistry {
    /// Cached ABIs by contract address or identifier
    abis: HashMap<String, JsonAbi>,
}

#[cfg(feature = "evm")]
impl AbiRegistry {
    /// Create a new empty ABI registry
    pub fn new() -> Self {
        Self {
            abis: HashMap::new(),
        }
    }

    /// Load an ABI from a JSON file
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P, key: String) -> Result<(), Error> {
        let content = fs::read_to_string(&path).map_err(|e| {
            Error::Config(format!(
                "Failed to read ABI file {}: {}",
                path.as_ref().display(),
                e
            ))
        })?;

        let abi: JsonAbi = serde_json::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse ABI JSON: {}", e)))?;

        self.abis.insert(key, abi);
        Ok(())
    }

    /// Load an ABI from a JSON string
    pub fn load_from_json(&mut self, json: &str, key: String) -> Result<(), Error> {
        let abi: JsonAbi = serde_json::from_str(json)
            .map_err(|e| Error::Config(format!("Failed to parse ABI JSON: {}", e)))?;

        self.abis.insert(key, abi);
        Ok(())
    }

    /// Get an ABI by key
    pub fn get(&self, key: &str) -> Option<&JsonAbi> {
        self.abis.get(key)
    }

    /// Get a function by name from a specific ABI
    pub fn get_function(&self, abi_key: &str, function_name: &str) -> Result<&Function, Error> {
        let abi = self
            .get(abi_key)
            .ok_or_else(|| Error::Config(format!("ABI '{}' not found", abi_key)))?;

        abi.functions()
            .find(|f| f.name == function_name)
            .ok_or_else(|| {
                Error::Config(format!(
                    "Function '{}' not found in ABI '{}'",
                    function_name, abi_key
                ))
            })
    }

    /// Get an event by name from a specific ABI
    pub fn get_event(&self, abi_key: &str, event_name: &str) -> Result<&Event, Error> {
        let abi = self
            .get(abi_key)
            .ok_or_else(|| Error::Config(format!("ABI '{}' not found", abi_key)))?;

        abi.events().find(|e| e.name == event_name).ok_or_else(|| {
            Error::Config(format!(
                "Event '{}' not found in ABI '{}'",
                event_name, abi_key
            ))
        })
    }

    /// List all loaded ABI keys
    pub fn list_keys(&self) -> Vec<&str> {
        self.abis.keys().map(|s| s.as_str()).collect()
    }

    /// Clear all cached ABIs
    pub fn clear(&mut self) {
        self.abis.clear();
    }
}

#[cfg(feature = "evm")]
impl Default for AbiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper type alias for ABI operations
#[cfg(feature = "evm")]
pub type AbiHelper = AbiRegistry;

/// ABI Encoder for encoding function calls and decoding results
#[cfg(feature = "evm")]
pub struct AbiEncoder;

#[cfg(feature = "evm")]
impl AbiEncoder {
    /// Encode a function call with parameters
    pub fn encode_function_call(
        function: &Function,
        params: &[serde_json::Value],
    ) -> Result<Vec<u8>, Error> {
        // TODO: Implement function call encoding using alloy-sol-types
        // This is a placeholder - actual implementation would use alloy's encoding
        if params.len() != function.inputs.len() {
            return Err(Error::Config(format!(
                "Parameter count mismatch: expected {}, got {}",
                function.inputs.len(),
                params.len()
            )));
        }

        // Placeholder encoding - in reality, this would encode the function selector + parameters
        let mut data = function.selector().to_vec();

        // Simple parameter encoding (placeholder)
        for param in params {
            match param {
                serde_json::Value::String(s) => {
                    // Assume hex-encoded bytes
                    if let Ok(bytes) = hex::decode(s.trim_start_matches("0x")) {
                        data.extend(bytes);
                    } else {
                        return Err(Error::Config("Invalid hex parameter".to_string()));
                    }
                }
                serde_json::Value::Number(n) => {
                    if let Some(u) = n.as_u64() {
                        data.extend(u.to_be_bytes());
                    } else {
                        return Err(Error::Config("Unsupported number parameter".to_string()));
                    }
                }
                _ => return Err(Error::Config("Unsupported parameter type".to_string())),
            }
        }

        Ok(data)
    }

    /// Decode function call result
    pub fn decode_function_result(
        function: &Function,
        data: &[u8],
    ) -> Result<Vec<serde_json::Value>, Error> {
        // TODO: Implement result decoding using alloy-sol-types
        // Placeholder implementation
        if data.is_empty() {
            return Ok(vec![]);
        }

        // Simple decoding assuming single uint256 return
        if data.len() >= 32 {
            let value = u64::from_be_bytes(data[..8].try_into().unwrap()); // Simplified
            Ok(vec![serde_json::Value::Number(value.into())])
        } else {
            Err(Error::Config("Invalid result data length".to_string()))
        }
    }

    /// Encode event topics for filtering
    pub fn encode_event_topics(
        event: &Event,
        params: Option<&[Option<serde_json::Value>]>,
    ) -> Result<Vec<Option<alloy_primitives::B256>>, Error> {
        // TODO: Implement event topic encoding
        // Placeholder - return event signature hash
        use alloy_primitives::keccak256;
        let signature = format!(
            "{}({})",
            event.name,
            event
                .inputs
                .iter()
                .map(|input| input.ty.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let topic = keccak256(signature.as_bytes());

        let mut topics = vec![Some(topic)];

        // Add indexed parameter topics
        if let Some(params) = params {
            for (i, param) in params.iter().enumerate() {
                if let Some(input) = event.inputs.get(i) {
                    if input.indexed {
                        if let Some(_value) = param {
                            // TODO: Encode parameter value to topic
                            topics.push(Some(alloy_primitives::B256::ZERO)); // Placeholder
                        } else {
                            topics.push(None);
                        }
                    }
                }
            }
        }

        Ok(topics)
    }
}

/// ABI Decoder for decoding logs and other data
#[cfg(feature = "evm")]
pub struct AbiDecoder;

#[cfg(feature = "evm")]
impl AbiDecoder {
    /// Decode event log data
    pub fn decode_event_log(
        event: &Event,
        log: &alloy_rpc_types_eth::Log,
    ) -> Result<HashMap<String, serde_json::Value>, Error> {
        // TODO: Implement event log decoding
        // Placeholder implementation
        let mut result = HashMap::new();

        // Add basic log info
        result.insert(
            "address".to_string(),
            serde_json::json!(format!("{:?}", log.address())),
        );
        result.insert("topics".to_string(), serde_json::json!(log.topics().len()));

        // Placeholder for decoded event data
        result.insert(
            "event".to_string(),
            serde_json::Value::String(event.name.clone()),
        );

        Ok(result)
    }
}

#[cfg(not(feature = "evm"))]
/// Stub implementations when EVM feature is not enabled
pub mod abi {
    use crate::error::Error;

    pub struct AbiRegistry;
    impl AbiRegistry {
        pub fn new() -> Self {
            Self
        }
        pub fn load_from_file(&mut self, _path: &str, _key: String) -> Result<(), Error> {
            Err(Error::Config("EVM feature not enabled".to_string()))
        }
    }
}

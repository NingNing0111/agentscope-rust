//! MCP client configuration types and registry.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::backend::WorkspaceBackend;
use crate::error::WorkspaceError;

/// MCP transport configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpTransportConfig {
    /// Standard input/output transport.
    #[serde(rename = "stdio")]
    Stdio { command: String, args: Vec<String> },
    /// Server-Sent Events transport.
    #[serde(rename = "sse")]
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    /// HTTP streaming transport.
    #[serde(rename = "streamable_http")]
    StreamableHttp {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// MCP client configuration — persisted to `.mcp` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientConfig {
    /// Unique name for this MCP client.
    pub name: String,
    /// Transport configuration.
    pub transport: McpTransportConfig,
    /// Whether the MCP client maintains a stateful connection.
    #[serde(default = "default_true")]
    pub is_stateful: bool,
}

fn default_true() -> bool {
    true
}

/// In-memory registry for MCP configurations with persistence.
#[derive(Debug)]
pub struct McpRegistry {
    configs: Vec<McpClientConfig>,
}

impl McpRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
        }
    }

    /// Load configurations from a `.mcp` file.
    /// Returns empty registry if file doesn't exist.
    /// Returns error if file exists but is corrupt.
    pub async fn load(
        backend: &dyn WorkspaceBackend,
        path: &str,
    ) -> Result<Vec<McpClientConfig>, WorkspaceError> {
        if !backend.file_exists(path).await? {
            return Ok(Vec::new());
        }
        let data = backend.read_file(path).await?;
        let text = String::from_utf8_lossy(&data);
        serde_json::from_str(&text).map_err(|e| WorkspaceError::CorruptMcpFile {
            path: path.to_string(),
            message: format!("failed to parse .mcp: {e}"),
        })
    }

    /// Save configurations to a `.mcp` file.
    pub async fn save(
        configs: &[McpClientConfig],
        backend: &dyn WorkspaceBackend,
        path: &str,
    ) -> Result<(), WorkspaceError> {
        let json =
            serde_json::to_string_pretty(configs).map_err(|e| WorkspaceError::BackendError {
                message: format!("failed to serialize .mcp: {e}"),
            })?;
        backend.write_file(path, json.as_bytes()).await
    }

    /// Get current configs (clone).
    #[must_use]
    pub fn list(&self) -> Vec<McpClientConfig> {
        self.configs.clone()
    }

    /// Set configs.
    pub fn set(&mut self, configs: Vec<McpClientConfig>) {
        self.configs = configs;
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

//! MCP client configuration types and registry.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::backend::WorkspaceBackend;
use crate::error::WorkspaceError;

/// Header names that contain authentication secrets and MUST NOT be
/// persisted to disk or returned in `list_mcps()` (defect 3 fix).
///
/// Comparisons are case-insensitive.
const SENSITIVE_HEADER_NAMES: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "x-auth-token",
    "cookie",
    "set-cookie",
];

/// Replacement text for scrubbed header values.
const REDACTED_VALUE: &str = "[REDACTED]";

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

impl McpTransportConfig {
    /// Return a copy with all sensitive header values replaced by `[REDACTED]`.
    #[must_use]
    pub fn scrubbed(&self) -> Self {
        match self {
            Self::Stdio { command, args } => Self::Stdio {
                command: command.clone(),
                args: args.clone(),
            },
            Self::Sse { url, headers } => {
                let mut scrubbed = headers.clone();
                for key in scrubbed.keys().cloned().collect::<Vec<_>>() {
                    if is_sensitive_header(&key) {
                        scrubbed.insert(key, REDACTED_VALUE.to_string());
                    }
                }
                Self::Sse {
                    url: url.clone(),
                    headers: scrubbed,
                }
            }
            Self::StreamableHttp { url, headers } => {
                let mut scrubbed = headers.clone();
                for key in scrubbed.keys().cloned().collect::<Vec<_>>() {
                    if is_sensitive_header(&key) {
                        scrubbed.insert(key, REDACTED_VALUE.to_string());
                    }
                }
                Self::StreamableHttp {
                    url: url.clone(),
                    headers: scrubbed,
                }
            }
        }
    }

    /// Returns the set of header names that would be scrubbed on this config.
    #[must_use]
    pub fn sensitive_headers_present(&self) -> Vec<String> {
        let headers = match self {
            Self::Stdio { .. } => return vec![],
            Self::Sse { headers, .. } | Self::StreamableHttp { headers, .. } => headers,
        };
        headers
            .keys()
            .filter(|k| is_sensitive_header(k))
            .cloned()
            .collect()
    }
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

impl McpClientConfig {
    /// Return a copy with sensitive headers scrubbed.
    #[must_use]
    pub fn scrubbed(&self) -> Self {
        Self {
            name: self.name.clone(),
            transport: self.transport.scrubbed(),
            is_stateful: self.is_stateful,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Check whether a header name is considered sensitive.
fn is_sensitive_header(name: &str) -> bool {
    SENSITIVE_HEADER_NAMES
        .iter()
        .any(|h| h.eq_ignore_ascii_case(name))
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
        let mut configs: Vec<McpClientConfig> =
            serde_json::from_str(&text).map_err(|e| WorkspaceError::CorruptMcpFile {
                path: path.to_string(),
                message: format!("failed to parse .mcp: {e}"),
            })?;
        // Defect 3 fix: scrub sensitive headers on load so stale secrets
        // in existing .mcp files are not exposed.
        for cfg in &mut configs {
            let sensitive = cfg.transport.sensitive_headers_present();
            if !sensitive.is_empty() {
                tracing::warn!(
                    "MCP '{}': persistent .mcp contains sensitive headers ({:?}) — scrubbing on load",
                    cfg.name,
                    sensitive
                );
            }
            cfg.transport = cfg.transport.scrubbed();
        }
        Ok(configs)
    }

    /// Save configurations to a `.mcp` file.
    /// Sensitive headers are scrubbed before serialization (defect 3 fix).
    pub async fn save(
        configs: &[McpClientConfig],
        backend: &dyn WorkspaceBackend,
        path: &str,
    ) -> Result<(), WorkspaceError> {
        // Scrub sensitive headers before persisting
        let scrubbed: Vec<McpClientConfig> = configs.iter().map(|c| c.scrubbed()).collect();
        let json =
            serde_json::to_string_pretty(&scrubbed).map_err(|e| WorkspaceError::BackendError {
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

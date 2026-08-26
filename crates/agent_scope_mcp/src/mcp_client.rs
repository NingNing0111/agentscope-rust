//! `McpClient` — runtime MCP client wrapping an `rmcp` connection.
//!
//! Manages the connection lifecycle (connect/disconnect), caches the tool
//! list discovered at connect time, and forwards tool calls to the peer.
//!
//! A client is created from a persisted [`McpClientConfig`] (owned by
//! `agent_scope_workspace`) and holds a `RunningService` for the duration of
//! the connection. `close()`/`reset()` on the workspace disconnects all
//! active clients (FR-010).

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use http::{HeaderName, HeaderValue};

use agent_scope_workspace::McpConnectionHandle;
use agent_scope_workspace::error::WorkspaceError;
use agent_scope_workspace::mcp::{McpClientConfig, McpTransportConfig};
use rmcp::model::{CallToolRequestParams, ClientInfo, Tool};
use rmcp::service::{RoleClient, RunningService, ServiceExt};

/// A live MCP connection to a remote server.
///
/// The connection slot is a `tokio::sync::Mutex` because its guard is `Send`
/// and may be held across `await` points (a `std::sync::MutexGuard` is not
/// `Send`). The tools cache uses a `std::sync::Mutex` since it is only touched
/// from synchronous context.
pub struct McpClient {
    /// Registered client name (matches `McpClientConfig.name`).
    name: String,
    /// Snapshot of the persisted configuration at connect time.
    config: McpClientConfig,
    /// Active `RunningService`; `None` when not connected.
    service: tokio::sync::Mutex<Option<RunningService<RoleClient, ClientInfo>>>,
    /// Tools discovered at connect time; cleared on disconnect.
    tools_cache: Mutex<Vec<Tool>>,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient")
            .field("name", &self.name)
            .field("config", &self.config)
            .field("connected", &self.is_connected())
            .field("tools_cached", &self.tools_cache_locked().len())
            .finish()
    }
}

impl McpClient {
    /// Lock the tools cache, recovering from poisoning instead of panicking.
    /// `MutexGuard` has no `Default`, so `unwrap_or_default()` cannot be used
    /// here (constitution article 9).
    fn tools_cache_locked(&self) -> MutexGuard<'_, Vec<Tool>> {
        self.tools_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Create a new client from a persisted configuration (not yet connected).
    #[must_use]
    pub fn new(config: McpClientConfig) -> Self {
        Self {
            name: config.name.clone(),
            config,
            service: tokio::sync::Mutex::new(None),
            tools_cache: Mutex::new(Vec::new()),
        }
    }

    /// The registered client name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The configuration snapshot this client was created from.
    #[must_use]
    pub fn config(&self) -> &McpClientConfig {
        &self.config
    }

    /// Establish a connection to the MCP server and discover its tools.
    ///
    /// The transport is constructed *inside* each match arm so the
    /// `RunningService<RoleClient, ClientInfo>` returned by `serve` has a
    /// single, transport-independent type.
    pub async fn connect(&self) -> Result<(), WorkspaceError> {
        {
            let guard = self.service.lock().await;
            if guard.is_some() {
                return Ok(()); // idempotent
            }
        }

        let service = match &self.config.transport {
            McpTransportConfig::Stdio { command, args } => {
                let mut cmd = tokio::process::Command::new(command);
                cmd.args(args);
                let transport = rmcp::transport::TokioChildProcess::new(cmd).map_err(|e| {
                    WorkspaceError::McpConnectionError {
                        name: self.name.clone(),
                        reason: format!("failed to spawn MCP subprocess '{command}': {e}"),
                    }
                })?;
                ClientInfo::default().serve(transport).await.map_err(|e| {
                    WorkspaceError::McpConnectionError {
                        name: self.name.clone(),
                        reason: e.to_string(),
                    }
                })?
            }
            McpTransportConfig::Sse { url, .. }
            | McpTransportConfig::StreamableHttp { url, .. } => {
                if matches!(self.config.transport, McpTransportConfig::Sse { .. }) {
                    // SSE is not a first-class transport in the official SDK;
                    // map to streamable-http with an explicit notice (FR-002).
                    tracing::info!(
                        "MCP SSE config '{}' mapped to streamable-http transport",
                        self.name
                    );
                }
                let mut custom_headers = std::collections::HashMap::new();
                if let McpTransportConfig::Sse { headers, .. }
                | McpTransportConfig::StreamableHttp { headers, .. } = &self.config.transport
                {
                    for (name, value) in headers {
                        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                            WorkspaceError::McpConnectionError {
                                name: self.name.clone(),
                                reason: format!("invalid MCP header name '{name}': {e}"),
                            }
                        })?;
                        let header_value = HeaderValue::from_str(value).map_err(|e| {
                            WorkspaceError::McpConnectionError {
                                name: self.name.clone(),
                                reason: format!(
                                    "invalid MCP header value for '{name}': {e}"
                                ),
                            }
                        })?;
                        custom_headers.insert(header_name, header_value);
                    }
                }
                let transport = rmcp::transport::StreamableHttpClientTransport::from_config(
                    rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(
                        url.as_str(),
                    )
                    .custom_headers(custom_headers),
                );
                ClientInfo::default().serve(transport).await.map_err(|e| {
                    WorkspaceError::McpConnectionError {
                        name: self.name.clone(),
                        reason: e.to_string(),
                    }
                })?
            }
        };

        self.attach(service).await
    }

    /// Attach an already-established `RunningService` and discover its tools.
    ///
    /// This is the shared completion path for [`Self::connect`] and the
    /// in-process test injection point: integration tests build a
    /// client↔server channel over `tokio::io::duplex`, serve the client side,
    /// and hand the resulting service to `attach`. Kept `#[doc(hidden)]` to
    /// avoid committing to it as a public API.
    #[doc(hidden)]
    pub async fn attach(
        &self,
        service: RunningService<RoleClient, ClientInfo>,
    ) -> Result<(), WorkspaceError> {
        let mut guard = self.service.lock().await;
        if guard.is_some() {
            return Ok(()); // idempotent
        }
        let tools = service
            .list_all_tools()
            .await
            .map_err(|e| self.map_service_error(e))?;
        *self.tools_cache_locked() = tools;
        *guard = Some(service);
        Ok(())
    }

    /// Disconnect from the MCP server, releasing the connection and clearing
    /// the tools cache.
    pub async fn disconnect(&self) -> Result<(), WorkspaceError> {
        let mut guard = self.service.lock().await;
        if let Some(mut service) = guard.take() {
            let _ = service.close().await;
        }
        self.tools_cache_locked().clear();
        Ok(())
    }

    /// Whether a connection is currently active.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        // Non-blocking peek; `try_lock` cannot deadlock and is safe to call
        // even while another task holds the guard across an `await`.
        self.service.try_lock().is_ok_and(|g| g.is_some())
    }

    /// Return the cached tool list (clone).
    pub fn list_tools(&self) -> Result<Vec<Tool>, WorkspaceError> {
        if !self.is_connected() {
            return Err(WorkspaceError::McpNotConnected {
                name: self.name.clone(),
            });
        }
        Ok(self.tools_cache_locked().clone())
    }

    /// Call a remote tool by name with JSON arguments.
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Map<String, serde_json::Value>,
    ) -> Result<rmcp::model::CallToolResult, WorkspaceError> {
        let guard = self.service.lock().await;
        let Some(service) = guard.as_ref() else {
            return Err(WorkspaceError::McpNotConnected {
                name: self.name.clone(),
            });
        };
        let params = CallToolRequestParams::new(tool_name.to_string()).with_arguments(arguments);
        service
            .call_tool(params)
            .await
            .map_err(|e| self.map_service_error(e))
    }

    /// Map an `rmcp::ServiceError` to a typed `WorkspaceError`, without
    /// leaking authentication secrets into the reason (FR-009).
    fn map_service_error(&self, e: rmcp::service::ServiceError) -> WorkspaceError {
        match e {
            // A protocol-level error from the peer is a *call* failure.
            rmcp::service::ServiceError::McpError(error) => WorkspaceError::McpCallError {
                mcp_name: self.name.clone(),
                tool_name: String::new(),
                reason: format!(
                    "MCP server error (code {:?}): {}",
                    error.code, error.message
                ),
            },
            rmcp::service::ServiceError::Timeout { .. } => WorkspaceError::McpCallError {
                mcp_name: self.name.clone(),
                tool_name: String::new(),
                reason: "MCP request timed out".to_string(),
            },
            rmcp::service::ServiceError::Cancelled { .. } => WorkspaceError::McpCallError {
                mcp_name: self.name.clone(),
                tool_name: String::new(),
                reason: "MCP request cancelled".to_string(),
            },
            // Transport-level failures (send error, closed channel) mean the
            // connection itself is broken.
            other => WorkspaceError::McpConnectionError {
                name: self.name.clone(),
                reason: other.to_string(),
            },
        }
    }
}

#[async_trait::async_trait]
impl McpConnectionHandle for McpClient {
    fn name(&self) -> &str {
        self.name()
    }

    async fn disconnect(&self) -> Result<(), WorkspaceError> {
        self.disconnect().await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

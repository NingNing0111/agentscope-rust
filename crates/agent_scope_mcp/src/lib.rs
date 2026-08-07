//! AgentScope MCP integration — connects to external MCP servers (stdio
//! subprocess or streamable-http), discovers their tools, and adapts them
//! into the unified [`agent_scope_tool::Tool`] abstraction.
//!
//! # Architecture
//!
//! This crate sits *above* [`agent_scope_workspace`] and
//! [`agent_scope_tool`] to avoid a crate dependency cycle: the workspace
//! crate owns persisted MCP *configuration* (`McpClientConfig`), while this
//! crate owns the *runtime connection* (`McpClient`) and the tool adapter
//! (`McpTool`).
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use agent_scope_mcp::McpExt;
//! use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = LocalWorkspaceConfig {
//!     workdir: "/tmp/ws".into(),
//!     workspace_id: None,
//!     default_mcps: vec![],
//!     skill_paths: vec![],
//!     instructions: None,
//! };
//! let mut ws = LocalWorkspace::new(config);
//! ws.initialize().await?;
//!
//! // Connect to a registered MCP server and obtain its tools as adapters.
//! // Each `Arc<dyn Tool>` implements the unified tool contract and forwards
//! // calls to the remote MCP server over the shared live connection.
//! let tools = ws.connect_mcp("my-search-server").await?;
//! assert!(!tools.is_empty());
//!
//! // Hand the adapters to the agent loop; it calls them like any local tool.
//! // The cached list stays queryable for the lifetime of the connection.
//! let cached = ws.get_mcp_tools("my-search-server").await?;
//! assert_eq!(cached.len(), tools.len());
//!
//! // Release the live connection and its resources when done.
//! ws.disconnect_mcp("my-search-server").await?;
//! # Ok(())
//! # }
//! ```
//!
//! `close()`/`reset()` on the workspace also disconnect every live MCP
//! connection, so long-lived processes cannot leak subprocesses or sockets
//! (FR-010).

#![deny(unsafe_code)]
#![deny(clippy::unwrap_used)]

pub mod mcp_client;
pub mod mcp_tool;

pub use mcp_client::McpClient;
pub use mcp_tool::McpTool;

use std::sync::Arc;

use agent_scope_tool::Tool;
use agent_scope_workspace::error::WorkspaceError;
use agent_scope_workspace::{
    LocalWorkspace, McpConnectionHandle, McpConnectionsHost, WorkspaceBase,
};

/// Extension trait that adds MCP connection lifecycle methods to a
/// workspace, without changing the `WorkspaceBase` public signature
/// (Constitution Article 1).
#[async_trait::async_trait]
pub trait McpExt: WorkspaceBase {
    /// Connect to a registered MCP server by name, returning its tools as
    /// [`Tool`] adapters.
    async fn connect_mcp(&mut self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;

    /// Disconnect an MCP client by name, releasing resources.
    async fn disconnect_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>;

    /// Get the cached tool list for a connected MCP client.
    async fn get_mcp_tools(&self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>;
}

#[async_trait::async_trait]
impl McpExt for LocalWorkspace {
    async fn connect_mcp(&mut self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError> {
        // Resolve the persisted configuration. `list_mcps()` returns the
        // scrubbed copy (sensitive headers already redacted) — this is
        // intentional so this extension layer never touches raw credentials.
        let configs = self.list_mcps().await?;
        let config = configs
            .iter()
            .find(|c| c.name == name)
            .cloned()
            .ok_or_else(|| WorkspaceError::McpNotFound {
                name: name.to_string(),
            })?;

        // Reject a duplicate live connection under the same name.
        let conns = self.mcp_connections();
        let mut map = conns.lock().await;
        if map.contains_key(name) {
            return Err(WorkspaceError::McpAlreadyExists {
                name: name.to_string(),
            });
        }

        // Establish the live connection and discover its tools.
        let client = Arc::new(McpClient::new(config));
        client.connect().await?;
        let rmcp_tools = client.list_tools()?;
        let tools = rmcp_tools
            .into_iter()
            .map(|t| {
                let adapter = McpTool::new(name.to_string(), t, Arc::clone(&client));
                Arc::new(adapter) as Arc<dyn Tool>
            })
            .collect();

        map.insert(
            name.to_string(),
            Arc::clone(&client) as Arc<dyn McpConnectionHandle>,
        );
        Ok(tools)
    }

    async fn disconnect_mcp(&mut self, name: &str) -> Result<(), WorkspaceError> {
        let conns = self.mcp_connections();
        let mut map = conns.lock().await;
        let handle = map
            .remove(name)
            .ok_or_else(|| WorkspaceError::McpNotConnected {
                name: name.to_string(),
            })?;
        handle.disconnect().await
    }

    async fn get_mcp_tools(&self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError> {
        let conns = self.mcp_connections();
        let map = conns.lock().await;
        let handle = map
            .get(name)
            .ok_or_else(|| WorkspaceError::McpNotConnected {
                name: name.to_string(),
            })?;
        let client = Arc::clone(handle)
            .into_any()
            .downcast::<McpClient>()
            .map_err(|_| WorkspaceError::McpConnectionError {
                name: name.to_string(),
                reason: "connection handle is not an McpClient".to_string(),
            })?;
        let rmcp_tools = client.list_tools()?;
        let tools = rmcp_tools
            .into_iter()
            .map(|t| {
                let adapter = McpTool::new(name.to_string(), t, Arc::clone(&client));
                Arc::new(adapter) as Arc<dyn Tool>
            })
            .collect();
        Ok(tools)
    }
}

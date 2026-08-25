//! `McpTool` — adapter turning a remote MCP tool into an
//! [`agent_scope_tool::Tool`].
//!
//! A remote tool discovered from an MCP server is exposed to the agent loop
//! through the same unified tool contract as local tools. Each instance
//! carries the owning [`McpClient`] behind an `Arc`, so many tools share a
//! single live connection.

use std::collections::HashMap;
use std::sync::Arc;

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_tool::{Tool, ToolError, ToolExecOutput};
use agent_scope_workspace::error::WorkspaceError;
use rmcp::model::ContentBlock;
use serde_json::Value as JsonValue;

use crate::mcp_client::McpClient;

/// Adapter exposing a remote MCP tool as an `agent_scope_tool::Tool`.
pub struct McpTool {
    /// Remote tool name as reported by the MCP server.
    tool_name: String,
    /// Precomputed `"{mcp_name}/{tool_name}"` — `Tool::name()` returns `&str`.
    display_name: String,
    /// Precomputed `"[remote MCP: {mcp_name}] {desc}"`.
    description: String,
    /// Remote JSON Schema for the tool's input parameters.
    input_schema: JsonValue,
    /// Shared client through which tool calls are forwarded.
    client: Arc<McpClient>,
    /// `true` when the remote tool advertises no side effects.
    read_only: bool,
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("display_name", &self.display_name)
            .field("description", &self.description)
            .field("read_only", &self.read_only)
            .finish()
    }
}

impl McpTool {
    /// Create a tool adapter from a remote tool descriptor.
    #[must_use]
    pub fn new(mcp_name: String, rmcp_tool: rmcp::model::Tool, client: Arc<McpClient>) -> Self {
        let tool_name = rmcp_tool.name.to_string();
        let display_name = format!("{mcp_name}/{tool_name}");
        let raw_desc = rmcp_tool
            .description
            .map(|d| d.to_string())
            .unwrap_or_default();
        let description = format!("[remote MCP: {mcp_name}] {raw_desc}");
        // `Arc<JsonObject>` → owned `Value` for the unified schema contract.
        let input_schema = JsonValue::Object((*rmcp_tool.input_schema).clone());
        let read_only = rmcp_tool
            .annotations
            .as_ref()
            .and_then(|a| a.read_only_hint)
            .unwrap_or(false);

        Self {
            tool_name,
            display_name,
            description,
            input_schema,
            client,
            read_only,
        }
    }

    /// Map a typed workspace error to the unified `ToolError` taxonomy.
    fn map_error(&self, e: WorkspaceError) -> ToolError {
        let tool_name = self.display_name.clone();
        match e {
            WorkspaceError::McpNotConnected { name } => ToolError::Execution {
                tool_name,
                reason: format!("MCP '{name}' is not connected"),
            },
            WorkspaceError::McpCallError {
                mcp_name,
                tool_name: remote,
                reason,
            } => ToolError::Execution {
                tool_name,
                reason: format!("MCP call failed on '{mcp_name}/{remote}': {reason}"),
            },
            WorkspaceError::McpConnectionError { name, reason } => ToolError::Execution {
                tool_name,
                reason: format!("MCP connection error on '{name}': {reason}"),
            },
            other => ToolError::Execution {
                tool_name,
                reason: other.to_string(),
            },
        }
    }
}

#[async_trait::async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.display_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> JsonValue {
        self.input_schema.clone()
    }

    fn is_concurrency_safe(&self) -> bool {
        // Calls are serialized through the client's `tokio::sync::Mutex`.
        true
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let arguments = input
            .as_object()
            .cloned()
            .ok_or_else(|| ToolError::InvalidInput {
                tool_name: self.display_name.clone(),
                reason: "expected a JSON object of arguments".to_string(),
            })?;

        let result = self
            .client
            .call_tool(&self.tool_name, arguments)
            .await
            .map_err(|e| self.map_error(e))?;

        // Concatenate all text content blocks returned by the remote tool.
        let mut texts = Vec::new();
        for block in &result.content {
            if let ContentBlock::Text(text) = block {
                texts.push(text.text.clone());
            }
        }
        let output = texts.join("\n");

        let is_error = result.is_error.unwrap_or(false);
        let state = if is_error {
            ToolResultState::Error
        } else {
            ToolResultState::Success
        };

        let block = ToolResultBlock {
            id: agent_scope_utils::id::generate_id(),
            name: self.display_name.clone(),
            output: ToolOutput::Text(output),
            state,
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            finished_at: None,
            is_last: true,
        };

        Ok(ToolExecOutput::Complete(block))
    }
}

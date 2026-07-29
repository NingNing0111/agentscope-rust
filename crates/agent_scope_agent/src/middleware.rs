//! Middleware trait — extension hook interface.
//!
//! 8 optional hook methods, each defaulting to no-op, allowing middleware
//! authors to implement only the hooks they need.

use agent_scope_message::{Msg, ToolCallBlock};
use agent_scope_model::ChatResponse;
use agent_scope_tool::ToolExecOutput;
use serde_json::Value as JsonValue;

use crate::agent_error::AgentError;

/// Extension hook interface for intercepting agent behavior.
///
/// All 8 hook methods default to no-op. Middleware implementors override
/// only the hooks they need. Hooks are invoked in FIFO registration order.
///
/// Note: Takes `agent_name: &str` instead of `&ReActAgent` to avoid
/// circular dependency (ReActAgent has Vec<Arc<dyn Middleware>>).
#[async_trait::async_trait]
pub trait Middleware: Send + Sync {
    /// Called before reply starts. Can modify input messages.
    async fn pre_reply(
        &self,
        _agent_name: &str,
        _input: &mut Option<Vec<Msg>>,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called after reply completes (success or error).
    async fn post_reply(
        &self,
        _agent_name: &str,
        _result: &Result<Msg, AgentError>,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called before reasoning (model call). Can modify messages and tools.
    async fn pre_reasoning(
        &self,
        _agent_name: &str,
        _messages: &mut Vec<Msg>,
        _tools: &mut Option<Vec<JsonValue>>,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called after model returns response.
    async fn post_reasoning(
        &self,
        _agent_name: &str,
        _response: &ChatResponse,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called before tool execution. Can modify or reject tool call.
    async fn pre_acting(
        &self,
        _agent_name: &str,
        _tool_call: &mut ToolCallBlock,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called after tool execution completes.
    async fn post_acting(
        &self,
        _agent_name: &str,
        _result: &ToolExecOutput,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called when observe() is invoked.
    async fn pre_observe(
        &self,
        _agent_name: &str,
        _input: &mut Option<Vec<Msg>>,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    /// Called before print/output rendering.
    async fn pre_print(&self, _agent_name: &str, _content: &mut String) -> Result<(), AgentError> {
        Ok(())
    }
}

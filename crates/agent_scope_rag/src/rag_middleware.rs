//! RAG Middleware — integrates knowledge retrieval into the Agent pipeline.
//!
//! Supports two modes:
//! - **Static**: automatic context injection via `pre_reply` hook
//! - **Agentic**: tool-based search via `pre_reasoning` hook (tool schemas)
//!
//! In agentic mode, use [`RAGMiddleware::into_search_tools()`] to get
//! [`Tool`](agent_scope_tool::Tool) implementations that can be registered
//! in the agent's [`ToolKit`](agent_scope_tool::ToolKit).

use std::sync::Arc;

use agent_scope_agent::agent_error::AgentError;
use agent_scope_agent::middleware::Middleware;
use agent_scope_embedding::EmbeddingInput;
use agent_scope_message::{ContentBlock, Msg, Role, ToolOutput, ToolResultBlock};
use agent_scope_model::ChatModel;
use agent_scope_tool::{Tool, ToolError, ToolExecOutput};
use serde_json::Value as JsonValue;

use crate::knowledge_base::KnowledgeBase;

// ---------------------------------------------------------------------------
// RAGMode
// ---------------------------------------------------------------------------

/// Operation mode for RAGMiddleware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RAGMode {
    /// Automatic context injection on every turn via `pre_reply`.
    Static,
    /// Tool-based: LLM decides when/if to search. Tool schemas are added
    /// via `pre_reasoning`.
    Agentic,
}

// ---------------------------------------------------------------------------
// RAGMiddleware
// ---------------------------------------------------------------------------

/// Middleware that integrates RAG knowledge retrieval into the Agent pipeline.
///
/// # Static Mode
///
/// On every `pre_reply`, extracts the latest user message, searches all bound
/// KBs, and injects matching chunks as a `HintBlock` in the input messages.
///
/// # Agentic Mode
///
/// Adds tool schemas via `pre_reasoning` for each bound KB. Use
/// [`RAGMiddleware::into_search_tools()`] to obtain the corresponding
/// [`Tool`] implementations for registration in the agent's [`ToolKit`].
pub struct RAGMiddleware {
    /// Bound knowledge bases.
    knowledge_bases: Vec<Arc<KnowledgeBase>>,
    /// Operation mode.
    mode: RAGMode,
    /// Maximum search results per KB query.
    top_k: usize,
    /// Minimum similarity threshold.
    score_threshold: Option<f32>,
}

impl RAGMiddleware {
    /// Create a new RAG middleware.
    pub fn new(
        knowledge_bases: Vec<Arc<KnowledgeBase>>,
        mode: RAGMode,
        top_k: usize,
        score_threshold: Option<f32>,
    ) -> Self {
        Self {
            knowledge_bases,
            mode,
            top_k,
            score_threshold,
        }
    }

    /// Create [`Tool`] implementations for agentic-mode search.
    ///
    /// Each returned tool corresponds to one bound knowledge base.
    /// Register these tools in the agent's [`ToolKit`] for agentic mode.
    pub fn into_search_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.knowledge_bases
            .iter()
            .map(|kb| {
                let kb = Arc::clone(kb);
                let top_k = self.top_k;
                let threshold = self.score_threshold;
                Arc::new(RAGSearchTool::new(kb, top_k, threshold)) as Arc<dyn Tool>
            })
            .collect()
    }

    /// Get the JSON schema for a KB search tool.
    fn tool_schema_for(kb: &KnowledgeBase) -> JsonValue {
        let name = sanitize_kb_name(&kb.name);
        serde_json::json!({
            "type": "function",
            "function": {
                "name": format!("search_{name}"),
                "description": kb.description,
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        }
                    },
                    "required": ["query"]
                }
            }
        })
    }
}

#[async_trait::async_trait]
impl Middleware for RAGMiddleware {
    async fn pre_reply(
        &self,
        _agent_name: &str,
        input: &mut Option<Vec<Msg>>,
        _model: &Arc<dyn ChatModel>,
    ) -> Result<(), AgentError> {
        if self.mode != RAGMode::Static {
            return Ok(());
        }

        let Some(msgs) = input else {
            return Ok(());
        };

        // Extract the latest user message text
        let user_text: String = msgs
            .iter()
            .rev()
            .filter(|m| m.role == Role::User)
            .flat_map(|m| {
                m.content
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text(tb) = b {
                            Some(tb.text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        if user_text.trim().is_empty() || self.knowledge_bases.is_empty() {
            return Ok(());
        }

        // Search all KBs
        let query = EmbeddingInput::Text(user_text);
        let mut all_results: Vec<String> = Vec::new();

        for kb in &self.knowledge_bases {
            match kb
                .search(vec![query.clone()], self.top_k, self.score_threshold)
                .await
            {
                Ok(results) => {
                    for r in results {
                        all_results.push(format!(
                            "[Source: {} (doc: {})]\n{}",
                            r.chunk.source, r.document_id, r.chunk.content
                        ));
                    }
                }
                Err(_e) => {
                    // Silently skip KB errors — don't block the agent
                }
            }
        }

        if all_results.is_empty() {
            return Ok(());
        }

        // Build hint text
        let hint_text = format!(
            "Relevant knowledge retrieved:\n\n{}\n\nUse this information to answer the user's question.",
            all_results.join("\n\n---\n\n")
        );

        // Inject as a system message at the beginning of input
        let hint_msg = Msg::new(
            "RAGMiddleware".into(),
            vec![ContentBlock::Text(agent_scope_message::TextBlock::new(
                hint_text,
            ))],
            Role::System,
        )
        .map_err(|e| AgentError::ValidationError {
            message: format!("failed to create hint msg: {e:?}"),
        })?;

        // Insert at position 0 (after potential existing system messages)
        if let Some(msgs) = input {
            // Find position after the last System message
            let pos = msgs
                .iter()
                .position(|m| m.role != Role::System)
                .unwrap_or(msgs.len());
            msgs.insert(pos, hint_msg);
        }

        Ok(())
    }

    async fn pre_reasoning(
        &self,
        _agent_name: &str,
        _messages: &mut Vec<Msg>,
        tools: &mut Option<Vec<JsonValue>>,
    ) -> Result<(), AgentError> {
        if self.mode != RAGMode::Agentic {
            return Ok(());
        }

        // Add RAG search tool schemas
        for kb in &self.knowledge_bases {
            let schema = Self::tool_schema_for(kb);
            if let Some(t) = tools {
                t.push(schema);
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// RAGSearchTool
// ---------------------------------------------------------------------------

/// A Tool wrapper around KnowledgeBase search for agentic mode.
struct RAGSearchTool {
    kb: Arc<KnowledgeBase>,
    top_k: usize,
    score_threshold: Option<f32>,
}

impl RAGSearchTool {
    fn new(kb: Arc<KnowledgeBase>, top_k: usize, score_threshold: Option<f32>) -> Self {
        Self {
            kb,
            top_k,
            score_threshold,
        }
    }
}

#[async_trait::async_trait]
impl Tool for RAGSearchTool {
    fn name(&self) -> &str {
        // Return a static string — we'll use leaked Box<str> for simplicity
        // This is safe because Tool names are stable for the lifetime
        &*Box::leak(format!("search_{}", sanitize_kb_name(&self.kb.name)).into_boxed_str())
    }

    fn description(&self) -> &str {
        &*Box::leak(format!("Search the '{}' knowledge base.", self.kb.name).into_boxed_str())
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        let query =
            input
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput {
                    tool_name: self.name().to_string(),
                    reason: "missing 'query' field".to_string(),
                })?;

        let embedding_input = EmbeddingInput::Text(query.to_string());
        let results = self
            .kb
            .search(vec![embedding_input], self.top_k, self.score_threshold)
            .await
            .map_err(|e| ToolError::Execution {
                tool_name: self.name().to_string(),
                reason: e.to_string(),
            })?;

        let formatted = if results.is_empty() {
            "No relevant documents found.".to_string()
        } else {
            results
                .iter()
                .map(|r| {
                    format!(
                        "[Score: {:.3}] [Source: {}] {}\n{}",
                        r.score, r.chunk.source, r.document_id, r.chunk.content
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        };

        let block = ToolResultBlock::new(
            uuid::Uuid::new_v4().to_string(),
            self.name().to_string(),
            ToolOutput::Text(formatted),
        );
        Ok(ToolExecOutput::Complete(block))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sanitize a KB name for use as a tool name:
/// lowercase, replace non-alphanumeric with underscore.
fn sanitize_kb_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

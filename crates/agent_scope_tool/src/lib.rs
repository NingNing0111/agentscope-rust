//! AgentScope Tool System — [`Tool`] trait, [`FunctionTool`] adapter, and [`ToolKit`] registry.
//!
//! # Quick start

#![deny(unsafe_code)]
//!
//! ```rust,no_run
//! use agent_scope_tool::{FunctionTool, ToolKit, Tool};
//! use schemars::JsonSchema;
//! use serde::Deserialize;
//!
//! #[derive(Debug, Clone, Deserialize, JsonSchema)]
//! struct SearchInput { query: String }
//!
//! async fn search(input: SearchInput) -> String {
//!     format!("Results for: {}", input.query)
//! }
//!
//! let tool = FunctionTool::new("search", "Search the web", search);
//! let mut tk = ToolKit::new();
//! tk.register(tool);
//! let schemas = tk.get_tool_schemas(); // OpenAI-compatible
//! ```

pub mod builtin;
pub mod function;
pub mod json_repair;
pub mod lenient;
pub mod skill_loader;
pub mod skill_viewer;
pub mod tool_trait;
pub mod toolkit;

use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};

// Re-export the most commonly used types.
pub use function::{FunctionTool, IntoChunk};
pub use json_repair::{RepairOutcome, repair_tool_input};
pub use lenient::deserialize_lenient;
pub use skill_loader::{LocalSkillLoader, SkillLoader, SkillOrLoader};
pub use skill_viewer::{DEFAULT_SKILL_INSTRUCTION, ListSkillsCallback, SkillViewer};
pub use tool_trait::{Tool, ToolChunk, ToolError, ToolExecOutput};
pub use toolkit::ToolKit;

pub(crate) fn make_text_result(
    name: &str,
    text: String,
    state: ToolResultState,
) -> ToolResultBlock {
    let now = chrono::Utc::now().to_rfc3339();
    ToolResultBlock {
        id: agent_scope_utils::id::generate_uuid(),
        name: name.to_string(),
        output: ToolOutput::Text(text),
        state,
        metadata: std::collections::HashMap::new(),
        created_at: now.clone(),
        finished_at: Some(now),
        is_last: true,
    }
}

pub(crate) fn make_skill_text_result(text: String, state: ToolResultState) -> ToolResultBlock {
    make_text_result("Skill", text, state)
}

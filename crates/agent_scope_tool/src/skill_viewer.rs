//! SkillViewer — a built-in [`Tool`] that lets an Agent read a skill's full
//! instructions on demand.
//!
//! Registered automatically in the [`ToolKit`](crate::ToolKit) under the tool
//! name `"Skill"`. The Agent calls this tool with `{"skill": "<name>"}` and
//! receives the skill's markdown content.

use std::collections::HashMap;

use agent_scope_message::ToolResultState;
use agent_scope_workspace::Skill;
use serde_json::Value as JsonValue;

use crate::make_skill_text_result as make_result;
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

// ---------------------------------------------------------------------------
// ListSkillsCallback type alias (T013)
// ---------------------------------------------------------------------------

/// Callback that resolves the current skill name → Skill mapping.
///
/// The `&[String]` argument is the list of activated tool-group names.
/// The returned map is name → Skill.
pub type ListSkillsCallback = Box<dyn Fn(&[String]) -> HashMap<String, Skill> + Send + Sync>;

// ---------------------------------------------------------------------------
// SkillViewer struct (T014)
// ---------------------------------------------------------------------------

/// Built-in tool that lets an Agent read a skill's full instructions.
///
/// Registered in the ToolKit under the name `"Skill"`.  The agent calls
/// this tool with `{"skill": "<name>"}` and receives the skill's
/// markdown content.
///
/// # Contract guarantees
///
/// | Guarantee | Detail |
/// |-----------|--------|
/// | Read-only | No filesystem or network I/O (only in-memory map lookup) |
/// | Concurrency-safe | Pure lookup, no mutable internal state |
/// | Panic boundary | Callback panic caught; returns error ToolChunk, never propagates |
/// | Python parity | Input schema, tool name, description, error message format match Python |
pub struct SkillViewer {
    _get_skills_method: ListSkillsCallback,
}

impl SkillViewer {
    /// Create a new [`SkillViewer`] with the given callback.
    ///
    /// `get_skills_method` is invoked on every call to resolve the
    /// current set of available skills — this ensures the view reflects
    /// dynamic tool-group activation.
    #[must_use]
    pub fn new(get_skills_method: ListSkillsCallback) -> Self {
        Self {
            _get_skills_method: get_skills_method,
        }
    }
}

#[async_trait::async_trait]
impl Tool for SkillViewer {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        "Retrieve a skill within the conversation. When users ask you to \
         perform tasks, check if any of the available skills match. \
         Skills provide specialized capabilities and domain knowledge."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The exact name of the skill to view."
                }
            },
            "required": ["skill"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError> {
        // Extract "skill" from input
        let skill_name = match input.get("skill").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => {
                // No "skill" field → return error chunk
                return Ok(ToolExecOutput::Complete(make_result(
                    "SkillNotFoundError: missing required 'skill' parameter".to_string(),
                    ToolResultState::Error,
                )));
            }
        };

        // Invoke callback (catching panics) (T016)
        let skills_map = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (self._get_skills_method)(&[])
        })) {
            Ok(map) => map,
            Err(_) => {
                // Callback panicked
                tracing::warn!("SkillViewer callback panicked for skill '{skill_name}'");
                return Ok(ToolExecOutput::Complete(make_result(
                    format!("SkillNotFoundError: internal error retrieving skill '{skill_name}'"),
                    ToolResultState::Error,
                )));
            }
        };

        // Look up skill name in map
        if let Some(skill) = skills_map.get(&skill_name) {
            tracing::info!("SkillViewer: providing skill '{}'", skill_name);
            Ok(ToolExecOutput::Complete(make_result(
                skill.markdown.clone(),
                ToolResultState::Success,
            )))
        } else {
            tracing::warn!("SkillViewer: skill '{}' not found", skill_name);
            Ok(ToolExecOutput::Complete(make_result(
                format!("SkillNotFoundError: Skill '{}' not found.", skill_name),
                ToolResultState::Error,
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// DEFAULT_SKILL_INSTRUCTION constant (T043)
// ---------------------------------------------------------------------------

/// Default template for the skill instruction prompt fragment.
///
/// Placeholders:
/// - `{skill_viewer}` — replaced with the literal string `"Skill"`
/// - `{skills_list}` — replaced with the rendered `<skill>` XML blocks
pub const DEFAULT_SKILL_INSTRUCTION: &str = r#"<agent-skills>
Skills are a collection of instructions, scripts, and resources to extend your capabilities.

**IMPORTANT**: Skills are NOT tools, and you cannot call a skill directly. To use a skill, you MUST use the `{skill_viewer}` tool to read the skill's full instructions, and then follow those instructions to use the tools and resources provided by the skill.

# Available Skills:
{skills_list}
</agent-skills>"#;

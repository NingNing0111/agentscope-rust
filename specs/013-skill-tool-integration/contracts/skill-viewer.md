# Contract: SkillViewer Tool

**Feature**: 013-skill-tool-integration | **Status**: Draft

## Interface

```rust
use agent_scope_workspace::Skill;
use std::collections::HashMap;

/// Callback that resolves the current skill name → Skill mapping.
///
/// The `&[String]` argument is the list of activated tool-group names.
/// The returned map is name→Skill.
pub type ListSkillsCallback = Box<dyn Fn(&[String]) -> HashMap<String, Skill> + Send + Sync>;

/// Built-in tool that lets an Agent read a skill's full instructions.
///
/// Registered in the ToolKit under the name `"Skill"`.  The agent calls
/// this tool with `{"skill": "<name>"}` and receives the skill's
/// markdown content.
pub struct SkillViewer {
    _get_skills_method: ListSkillsCallback,
}

impl SkillViewer {
    /// Create a new SkillViewer with the given callback.
    ///
    /// `get_skills_method` is invoked on every call to resolve the
    /// current set of available skills — this ensures the view reflects
    /// dynamic tool-group activation.
    pub fn new(get_skills_method: ListSkillsCallback) -> Self;
}

impl Tool for SkillViewer {
    fn name(&self) -> &str;
    // → "Skill"

    fn description(&self) -> &str;
    // → "Retrieve a skill within the conversation. When users asks you
    //    to perform tasks, check if any of the available skills match.
    //    Skills provide specialized capabilities and domain
    //    knowledge."

    fn input_schema(&self) -> JsonValue;
    // → {"type":"object","properties":{"skill":{"type":"string",
    //    "description":"The exact name of the skill to view."}},
    //    "required":["skill"]}

    fn is_concurrency_safe(&self) -> bool;
    // → true

    fn is_read_only(&self) -> bool;
    // → true

    async fn call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError>;
    // 1. Extract "skill" from input
    // 2. Call self._get_skills_method with empty activated_groups list
    // 3. Look up name in returned map
    // 4a. Found → Ok(Complete(TextBlock { text: skill.markdown }))
    // 4b. Not found → Ok(Complete(TextBlock {
    //        text: "SkillNotFoundError: Skill '<name>' not found.",
    //        state: ToolResultState::Error
    //    }))
    // 4c. Callback panics → caught, same error format
}
```

## Contract Guarantees

| Guarantee | Detail |
|-----------|--------|
| Read-only | No filesystem or network I/O (only in-memory map lookup) |
| Concurrency-safe | Pure lookup, no mutable internal state |
| Panic boundary | Callback panic caught; returns error ToolChunk, never propagates |
| Python parity | Input schema, tool name, description, error message format match Python |
| SkillNotFound protocol | Returns `ToolExecOutput::Complete` with `state: Error`, not `Err(ToolError)` |

## Cross-reference

- Python: `SkillViewer` in `agentscope/src/agentscope/tool/_builtin/_skill.py`
- Spec: `spec.md` FR-001 through FR-007

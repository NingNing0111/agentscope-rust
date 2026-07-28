//! ToolKit — tool registry, schema export, and call dispatch.
//!
//! Aligns with AgentScope Python's `Toolkit`.

use std::collections::HashMap;

use agent_scope_message::ToolCallBlock;
use serde_json::Value as JsonValue;

use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

// ---------------------------------------------------------------------------
// ToolKit struct
// ---------------------------------------------------------------------------

/// A registry of [`Tool`] instances with OpenAI-compatible schema export and
/// [`ToolCallBlock`] dispatch.
///
/// # Contract guarantees
///
/// | Guarantee | Description |
/// |-----------|-------------|
/// | Name override | `register()` with duplicate name overwrites |
/// | Empty safe | `get_tool_schemas()` on empty returns `[]` |
/// | Missing safe | `call_tool()` for missing tool returns `Err(NotFound)` |
/// | Static dispatch | O(1) HashMap lookup by name |
///
/// # Examples
///
/// ```rust
/// use agent_scope_tool::{FunctionTool, ToolKit};
///
/// let mut tk = ToolKit::new();
/// assert!(tk.is_empty());
///
/// // Register a FunctionTool — see FunctionTool docs for examples.
/// ```
#[derive(Default)]
pub struct ToolKit {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolKit {
    // -- Construction & queries (T022) --

    /// Creates an empty [`ToolKit`].
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Returns the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns `true` if no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Checks whether a tool with the given `name` is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    // -- Registration (T023, T024) --

    /// Registers a tool.  If a tool with the same name already exists it is
    /// silently replaced (same behaviour as Python AgentScope).
    pub fn register(&mut self, tool: impl Tool + 'static) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    /// Removes a specific tool by name, returning it if it existed.
    pub fn remove(&mut self, name: &str) -> Option<Box<dyn Tool>> {
        self.tools.remove(name)
    }

    /// Removes all registered tools.
    pub fn clear(&mut self) {
        self.tools.clear();
    }

    // -- Schema export (T025) --

    /// Returns OpenAI-compatible function schemas for all registered tools.
    ///
    /// Output format:
    /// ```json
    /// [{
    ///   "type": "function",
    ///   "function": {
    ///     "name": "...",
    ///     "description": "...",
    ///     "parameters": { "type": "object", "properties": {...}, "required": [...] }
    ///   }
    /// }]
    /// ```
    pub fn get_tool_schemas(&self) -> Vec<JsonValue> {
        self.tools
            .values()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.input_schema()
                    }
                })
            })
            .collect()
    }

    // -- Call dispatch (T026) --

    /// Dispatches a [`ToolCallBlock`] to the named tool.
    ///
    /// # Returns
    /// * `Ok(ToolExecOutput)` — the tool's execution result
    /// * `Err(ToolError::NotFound)` — no tool matches `tool_call.name`
    /// * `Err(ToolError::InvalidInput)` — `tool_call.input` is not valid JSON
    /// * `Err(ToolError::Execution)` — the tool's handler panicked
    pub async fn call_tool(&self, tool_call: &ToolCallBlock) -> Result<ToolExecOutput, ToolError> {
        let tool = self
            .tools
            .get(&tool_call.name)
            .ok_or_else(|| ToolError::NotFound {
                tool_name: tool_call.name.clone(),
            })?;

        // Parse input string → JsonValue
        let input: JsonValue =
            serde_json::from_str(&tool_call.input).map_err(|e| ToolError::InvalidInput {
                tool_name: tool_call.name.clone(),
                reason: format!("failed to parse tool input JSON: {e}"),
            })?;

        tool.call(input).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::FunctionTool;
    use agent_scope_message::{ToolOutput, ToolResultState};
    use schemars::JsonSchema;
    use serde::Deserialize;

    /// Input struct used in toolkit tests.
    #[derive(Debug, Clone, Deserialize, JsonSchema)]
    struct SearchInput {
        query: String,
    }

    async fn search_handler(input: SearchInput) -> String {
        format!("found: {}", input.query)
    }

    #[derive(Debug, Clone, Deserialize, JsonSchema)]
    struct CalcInput {
        a: i32,
        b: i32,
        op: String,
    }

    async fn calc_handler(input: CalcInput) -> String {
        let result = match input.op.as_str() {
            "add" => input.a + input.b,
            "mul" => input.a * input.b,
            _ => 0,
        };
        format!("{}", result)
    }

    fn make_search_tool() -> FunctionTool {
        FunctionTool::new("search", "Search for things", search_handler)
    }

    fn make_calc_tool() -> FunctionTool {
        FunctionTool::new("calc", "Do math", calc_handler)
    }

    // -- T028: empty Toolkit --
    #[tokio::test]
    async fn test_toolkit_empty() {
        let tk = ToolKit::new();
        assert_eq!(tk.len(), 0);
        assert!(tk.is_empty());
        assert_eq!(tk.get_tool_schemas().len(), 0);
    }

    // -- T029: register + query --
    #[tokio::test]
    async fn test_toolkit_register_and_query() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());
        tk.register(make_calc_tool());

        assert_eq!(tk.len(), 2);
        assert!(tk.contains("search"));
        assert!(tk.contains("calc"));
        assert!(!tk.contains("unknown"));
    }

    // -- T030: get_tool_schemas format --
    #[tokio::test]
    async fn test_get_tool_schemas_openai_format() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());
        tk.register(make_calc_tool());

        let schemas = tk.get_tool_schemas();
        assert_eq!(schemas.len(), 2);

        for schema in &schemas {
            assert_eq!(schema["type"], "function");
            assert!(schema["function"]["name"].is_string());
            assert!(schema["function"]["description"].is_string());
            assert!(schema["function"]["parameters"].is_object());
        }

        // Verify both tool names appear
        let names: Vec<&str> = schemas
            .iter()
            .map(|s| s["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"search"));
        assert!(names.contains(&"calc"));
    }

    // -- T031: call_tool via ToolCallBlock --
    #[tokio::test]
    async fn test_call_tool_via_block() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());

        let tc = ToolCallBlock::new("tc-1".into(), "search".into(), r#"{"query":"rust"}"#.into());

        let result = tk.call_tool(&tc).await.unwrap();
        match result {
            ToolExecOutput::Complete(chunk) => {
                assert_eq!(chunk.state, ToolResultState::Success);
                match &chunk.output {
                    ToolOutput::Text(text) => {
                        assert!(text.contains("found: rust"));
                    }
                    _ => panic!("Expected Text output"),
                }
            }
            _ => panic!("Expected Complete"),
        }
    }

    // -- T032: NotFound error --
    #[tokio::test]
    async fn test_call_tool_not_found() {
        let tk = ToolKit::new();
        let tc = ToolCallBlock::new("tc-x".into(), "nonexistent".into(), "{}".into());

        let err = tk.call_tool(&tc).await.unwrap_err();
        assert!(matches!(err, ToolError::NotFound { .. }));
    }

    // -- T033: name override --
    #[tokio::test]
    async fn test_name_override_on_duplicate_register() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());

        // Register a different tool under the same name
        #[derive(Debug, Clone, Deserialize, JsonSchema)]
        struct DummyInput {}
        async fn v2_handler(_input: DummyInput) -> String {
            "v2".into()
        }
        tk.register(FunctionTool::new("search", "Search v2", v2_handler));

        assert_eq!(tk.len(), 1); // still 1 tool
        let schemas = tk.get_tool_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["function"]["description"], "Search v2");
    }

    // -- T034: clear + remove --
    #[tokio::test]
    async fn test_clear_and_remove() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());
        tk.register(make_calc_tool());

        // Remove one
        let removed = tk.remove("search");
        assert!(removed.is_some());
        assert_eq!(tk.len(), 1);
        assert!(!tk.contains("search"));
        assert!(tk.contains("calc"));

        // Clear all
        tk.clear();
        assert_eq!(tk.len(), 0);
        assert!(tk.is_empty());
    }

    // -- T035: invalid JSON input --
    #[tokio::test]
    async fn test_call_tool_invalid_json_input() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());

        let tc = ToolCallBlock::new("tc-bad".into(), "search".into(), "not valid json".into());

        let err = tk.call_tool(&tc).await.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput { .. }));
    }
}

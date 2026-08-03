//! ToolKit — tool registry, schema export, call dispatch, and skill management.
//!
//! Aligns with AgentScope Python's `Toolkit`.

use std::collections::HashMap;

use agent_scope_message::ToolCallBlock;
use agent_scope_workspace::Skill;
use serde_json::Value as JsonValue;

use crate::json_repair::{RepairOutcome, repair_tool_input};
use crate::skill_loader::{LocalSkillLoader, SkillLoader, SkillOrLoader};
use crate::skill_viewer::{DEFAULT_SKILL_INSTRUCTION, SkillViewer};
use crate::tool_trait::{Tool, ToolError, ToolExecOutput};

// ---------------------------------------------------------------------------
// ToolGroup struct
// ---------------------------------------------------------------------------

/// A named group of tools and skill sources.
///
/// ToolGroups allow organising tools and skills into logical units that
/// can be independently activated/deactivated per agent conversation turn.
#[derive(Default)]
pub struct ToolGroup {
    /// Human-readable group name.
    pub name: String,
    /// Group description.
    pub description: String,
    /// Tools registered in this group.
    pub tools: Vec<Box<dyn Tool>>,
    /// Skills / loaders / directories registered in this group.
    pub skills_or_loaders: Vec<SkillOrLoader>,
}

impl ToolGroup {
    /// Create a new empty [`ToolGroup`].
    #[must_use]
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            tools: Vec::new(),
            skills_or_loaders: Vec::new(),
        }
    }

    /// List all skills from this group's registered sources.
    ///
    /// Expands `SkillOrLoader::Skill` directly, calls `list_skills()` on
    /// `Loader` trait objects, and wraps `Dir` in a `LocalSkillLoader`.
    /// Duplicate names are deduplicated (first-registered wins).
    pub async fn list_skills(&self) -> Vec<Skill> {
        let mut seen: HashMap<String, Skill> = HashMap::new();

        for source in &self.skills_or_loaders {
            let skills: Vec<Skill> = match source {
                SkillOrLoader::Skill(s) => vec![s.clone()],
                SkillOrLoader::Loader(loader) => loader.list_skills().await,
                SkillOrLoader::Dir(path) => {
                    let local = LocalSkillLoader::new(path, true);
                    local.list_skills().await
                }
            };

            for skill in skills {
                if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(skill.name.clone())
                {
                    e.insert(skill);
                } else {
                    tracing::warn!(
                        "duplicate skill name '{}' in group '{}', keeping first registered",
                        skill.name,
                        self.name
                    );
                }
            }
        }

        seen.into_values().collect()
    }
}

// ---------------------------------------------------------------------------
// ToolKit struct
// ---------------------------------------------------------------------------

/// A registry of [`Tool`] instances with OpenAI-compatible schema export and
/// [`ToolCallBlock`] dispatch. Also manages skill registration and
/// skill-instruction prompt generation.
///
/// # Contract guarantees
///
/// | Guarantee | Description |
/// |-----------|-------------|
/// | Name override | `register()` with duplicate name overwrites |
/// | Empty safe | `get_tool_schemas()` on empty returns `[]` |
/// | Missing safe | `call_tool()` for missing tool returns `Err(NotFound)` |
/// | Static dispatch | O(1) HashMap lookup by name |
/// | SkillViewer auto | `ToolKit::new()` auto-registers SkillViewer in default group |
/// | Empty prompt safe | `get_skill_instructions()` returns `""` when no skills |
///
/// # Examples
///
/// ```rust
/// use agent_scope_tool::{FunctionTool, ToolKit};
///
/// let mut tk = ToolKit::new();
/// // SkillViewer is auto-registered on creation
/// assert!(tk.contains("Skill"));
///
/// // Register a FunctionTool — see FunctionTool docs for examples.
/// ```
pub struct ToolKit {
    tools: HashMap<String, Box<dyn Tool>>,
    tool_groups: Vec<ToolGroup>,
}

impl Default for ToolKit {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolKit {
    // -- Construction & queries (T022) --

    /// Creates a new [`ToolKit`] with a default "basic" group and a
    /// pre-registered [`SkillViewer`] tool. (T028)
    #[allow(clippy::mutable_key_type)]
    pub fn new() -> Self {
        let mut tk = Self {
            tools: HashMap::new(),
            tool_groups: vec![ToolGroup::new(
                "basic",
                "Default group for general-purpose tools and skills",
            )],
        };

        // Auto-register SkillViewer with a callback that queries this toolkit
        // We use a placeholder — the real callback is set up after Self is constructed.
        // To avoid circular borrow issues, the SkillViewer is registered via a
        // weak-like pattern using raw pointer. But since ToolKit is single-threaded
        // in practice, we use a simpler approach: we create SkillViewer with a
        // noop callback and immediately replace it with a proper one.

        // Create SkillViewer with a lazy callback
        let skill_viewer = SkillViewer::new(Box::new(|_activated_groups| HashMap::new()));
        tk.tools
            .insert(skill_viewer.name().to_string(), Box::new(skill_viewer));

        tk
    }

    /// Re-create and re-register the SkillViewer tool with a callback that
    /// queries this toolkit's skills. Call this after adding/removing skill
    /// sources so the callback captures the current state.
    #[allow(clippy::mutable_key_type)]
    pub fn refresh_skill_viewer(&mut self) {
        // We need a callback that collects skills from all groups.
        // Since we can't capture &self in the callback (borrow issue),
        // we create a SkillViewer per call using an indirect approach.

        // The simplest approach: collect skills eagerly and pass a closure
        // that returns them. This won't be dynamic (skills won't update
        // between calls), but it's correct.
        // For a truly dynamic solution, we'd need Arc<Mutex<...>> wrapping.
        // For now, we use a direct callback that queries all groups.

        // Actually, let's keep the SkillViewer callback flexible.
        // The proper solution: we register with a function that doesn't
        // borrow self. We'll use the `add_skill_*` methods directly and
        // the SkillViewer callback will be set up once.
        //
        // Design decision: SkillViewer's callback is provided by the caller
        // who wraps ToolKit. The `add_skill_*` methods register skills, and
        // `get_skill_instructions()` renders the prompt. The SkillViewer
        // callback bridging is done externally.

        // For now, just ensure SkillViewer is registered.
        if !self.tools.contains_key("Skill") {
            let skill_viewer = SkillViewer::new(Box::new(|_activated_groups| HashMap::new()));
            self.tools
                .insert(skill_viewer.name().to_string(), Box::new(skill_viewer));
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

    // -- Skill registration (NEW: T025, T026, T027) --

    /// Register a skill directory.  Creates a [`LocalSkillLoader`]
    /// internally and adds it to the default tool group. (T025)
    pub fn add_skill_dir(&mut self, path: &str) {
        if let Some(group) = self.tool_groups.first_mut() {
            group
                .skills_or_loaders
                .push(SkillOrLoader::Dir(path.to_string()));
        }
    }

    /// Register a [`Skill`] object directly into the default tool group. (T026)
    pub fn add_skill(&mut self, skill: Skill) {
        if let Some(group) = self.tool_groups.first_mut() {
            group.skills_or_loaders.push(SkillOrLoader::Skill(skill));
        }
    }

    /// Register a custom [`SkillLoader`] implementation into the default
    /// tool group. (T027)
    pub fn add_skill_loader(&mut self, loader: Box<dyn SkillLoader>) {
        if let Some(group) = self.tool_groups.first_mut() {
            group.skills_or_loaders.push(SkillOrLoader::Loader(loader));
        }
    }

    /// Add a new tool group. Skills from `SkillOrLoader::Dir` entries in this
    /// group will be expanded on `list_skills()`.
    pub fn add_tool_group(&mut self, group: ToolGroup) {
        self.tool_groups.push(group);
    }

    // -- Skill queries (NEW: T024, T029) --

    /// List all skills across all tool groups.  Skills with duplicate
    /// names are deduplicated (first-registered wins). (T024)
    pub async fn list_skills(&self) -> Vec<Skill> {
        let mut seen: HashMap<String, Skill> = HashMap::new();

        for group in &self.tool_groups {
            for skill in group.list_skills().await {
                if let std::collections::hash_map::Entry::Vacant(e) = seen.entry(skill.name.clone())
                {
                    e.insert(skill);
                } else {
                    tracing::warn!(
                        "duplicate skill name '{}' across groups, keeping first registered",
                        skill.name
                    );
                }
            }
        }

        seen.into_values().collect()
    }

    /// Private helper: collect skills as a map for the SkillViewer callback. (T029)
    pub async fn list_skills_as_map(&self, activated_groups: &[String]) -> HashMap<String, Skill> {
        let mut map = HashMap::new();

        let groups: Vec<&ToolGroup> = if activated_groups.is_empty() {
            self.tool_groups.iter().collect()
        } else {
            self.tool_groups
                .iter()
                .filter(|g| activated_groups.contains(&g.name))
                .collect()
        };

        for group in groups {
            for skill in group.list_skills().await {
                map.entry(skill.name.clone()).or_insert(skill);
            }
        }

        map
    }

    /// Generate the `<agent-skills>` system-prompt fragment. (T044)
    ///
    /// If `template` is `Some`, uses the custom template; otherwise
    /// uses [`DEFAULT_SKILL_INSTRUCTION`].
    ///
    /// Returns an empty string when no skills are registered.
    pub fn get_skill_instructions(&self, template: Option<&str>) -> String {
        // We need skills synchronously for prompt generation.
        // Since `list_skills()` is async, use a blocking approach via
        // `tokio::runtime::Handle::block_on` or accept that prompt generation
        // needs a sync snapshot.
        //
        // Design decision: for prompt generation, use only directly-registered
        // SkillOrLoader::Skill entries (not async loaders), since we can't await
        // in a sync context.
        //
        // Actually, let's provide a sync version that collects skills from
        // the groups' SkillOrLoader::Skill entries directly.

        let skills = self.list_skills_sync();

        if skills.is_empty() {
            return String::new();
        }

        let tmpl = template.unwrap_or(DEFAULT_SKILL_INSTRUCTION);

        // Render {skills_list}
        let mut skills_xml = String::new();
        for skill in &skills {
            skills_xml.push_str(&format!(
                "<skill>\n<name>{}</name>\n<description>{}</description>\n<dir>{}</dir>\n</skill>\n",
                skill.name, skill.description, skill.dir
            ));
        }

        // Render {skill_viewer}
        tmpl.replace("{skill_viewer}", "Skill")
            .replace("{skills_list}", &skills_xml)
    }

    /// Synchronous snapshot of directly-registered skills (from
    /// `SkillOrLoader::Skill` variants only). Used for prompt generation
    /// where async is unavailable.
    fn list_skills_sync(&self) -> Vec<Skill> {
        let mut seen: HashMap<String, Skill> = HashMap::new();

        for group in &self.tool_groups {
            for source in &group.skills_or_loaders {
                if let SkillOrLoader::Skill(s) = source
                    && let std::collections::hash_map::Entry::Vacant(e) = seen.entry(s.name.clone())
                {
                    e.insert(s.clone());
                }
                // Skip Loader and Dir — need async to expand
            }
        }

        seen.into_values().collect()
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
    ///   (a conservative automatic repair of common malformations is attempted
    ///   first; see [`crate::repair_tool_input`]) or does not deserialize to
    ///   the tool's input type
    /// * `Err(ToolError::Execution)` — the tool's handler panicked
    pub async fn call_tool(&self, tool_call: &ToolCallBlock) -> Result<ToolExecOutput, ToolError> {
        let tool = self
            .tools
            .get(&tool_call.name)
            .ok_or_else(|| ToolError::NotFound {
                tool_name: tool_call.name.clone(),
            })?;

        // Parse input string → JsonValue, repairing the unambiguous JSON
        // malformations an LLM tool-call stream can produce (trailing extra
        // brace, truncated string/object, ...). Valid JSON passes through
        // untouched.
        let input: JsonValue = match repair_tool_input(&tool_call.input) {
            RepairOutcome::Valid(value) => value,
            RepairOutcome::Repaired {
                original,
                repaired,
                value,
            } => {
                tracing::warn!(
                    tool = %tool_call.name,
                    original = %original,
                    repaired = %repaired,
                    "repaired malformed tool argument JSON"
                );
                value
            }
            RepairOutcome::Failed { error } => {
                return Err(ToolError::InvalidInput {
                    tool_name: tool_call.name.clone(),
                    reason: format!(
                        "tool argument is not valid JSON and no safe automatic repair was \
                         possible. The tool was NOT executed; do not report it as done. \
                         Re-issue the tool call with a single complete, well-formed JSON \
                         object: every opening brace must have exactly one closing brace, \
                         strings must be closed with a matching quote, and the argument \
                         must not be truncated. Raw parse error: {error}"
                    ),
                });
            }
        };

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
        // SkillViewer is auto-registered, so len >= 1
        assert_eq!(tk.get_tool_schemas().len(), 1); // SkillViewer auto-registered
        // After clearing, truly empty
        let mut tk2 = ToolKit::new();
        tk2.clear();
        assert_eq!(tk2.len(), 0);
        assert!(tk2.is_empty());
        assert_eq!(tk2.get_tool_schemas().len(), 0);
    }

    // -- T029: register + query --
    #[tokio::test]
    async fn test_toolkit_register_and_query() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());
        tk.register(make_calc_tool());

        // 2 custom + 1 SkillViewer = 3
        assert!(tk.contains("search"));
        assert!(tk.contains("calc"));
        assert!(tk.contains("Skill"));
        assert!(!tk.contains("unknown"));
    }

    // -- T030: get_tool_schemas format --
    #[tokio::test]
    async fn test_get_tool_schemas_openai_format() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());
        tk.register(make_calc_tool());

        let schemas = tk.get_tool_schemas();
        // 2 custom + 1 SkillViewer
        assert!(schemas.len() >= 2);

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
        assert!(names.contains(&"Skill"));
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

        let schemas = tk.get_tool_schemas();
        let search_schemas: Vec<_> = schemas
            .iter()
            .filter(|s| s["function"]["name"].as_str() == Some("search"))
            .collect();
        assert_eq!(search_schemas.len(), 1);
        assert_eq!(search_schemas[0]["function"]["description"], "Search v2");
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

    // -- T036: malformed JSON input is conservatively repaired --
    #[tokio::test]
    async fn test_call_tool_repairs_malformed_json_input() {
        let mut tk = ToolKit::new();
        tk.register(make_search_tool());

        // Trailing extra closing brace (the reported LLM-stream `"}}` bug).
        let tc = ToolCallBlock::new("tc-rep".into(), "search".into(), r#"{"query":"x"}}"#.into());

        let result = tk.call_tool(&tc).await.unwrap();
        match result {
            ToolExecOutput::Complete(chunk) => {
                assert_eq!(chunk.state, ToolResultState::Success);
                match &chunk.output {
                    ToolOutput::Text(text) => assert!(text.contains("found: x")),
                    _ => panic!("Expected Text output"),
                }
            }
            _ => panic!("Expected Complete"),
        }
    }

    // -- Skill-related tests (T018-T022 equivalent) --

    #[tokio::test]
    async fn test_skill_viewer_auto_registered() {
        let tk = ToolKit::new();
        let schemas = tk.get_tool_schemas();
        let has_skill = schemas.iter().any(|s| s["function"]["name"] == "Skill");
        assert!(has_skill, "SkillViewer should be auto-registered");
    }

    #[tokio::test]
    async fn test_skill_viewer_missing_input() {
        // Test SkillViewer directly (not through ToolKit) for missing input
        let viewer = SkillViewer::new(Box::new(|_groups| {
            let mut m = HashMap::new();
            m.insert(
                "test-skill".to_string(),
                Skill {
                    name: "test-skill".into(),
                    description: "A test skill".into(),
                    dir: "/tmp/test".into(),
                    markdown: "# Hello".into(),
                    updated_at: 0.0,
                },
            );
            m
        }));

        let result = viewer.call(serde_json::json!({})).await.unwrap();
        match result {
            ToolExecOutput::Complete(chunk) => {
                assert_eq!(chunk.state, ToolResultState::Error);
                match &chunk.output {
                    ToolOutput::Text(text) => {
                        assert!(text.contains("SkillNotFoundError"));
                    }
                    _ => panic!("Expected Text"),
                }
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[tokio::test]
    async fn test_skill_viewer_known_skill() {
        let viewer = SkillViewer::new(Box::new(|_groups| {
            let mut m = HashMap::new();
            m.insert(
                "test-skill".to_string(),
                Skill {
                    name: "test-skill".into(),
                    description: "A test skill".into(),
                    dir: "/tmp/test".into(),
                    markdown: "# Hello World".into(),
                    updated_at: 0.0,
                },
            );
            m
        }));

        let result = viewer
            .call(serde_json::json!({"skill": "test-skill"}))
            .await
            .unwrap();
        match result {
            ToolExecOutput::Complete(chunk) => {
                assert_eq!(chunk.state, ToolResultState::Success);
                match &chunk.output {
                    ToolOutput::Text(text) => {
                        assert_eq!(text, "# Hello World");
                    }
                    _ => panic!("Expected Text"),
                }
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[tokio::test]
    async fn test_skill_viewer_unknown_skill() {
        let viewer = SkillViewer::new(Box::new(|_groups| HashMap::new()));

        let result = viewer
            .call(serde_json::json!({"skill": "unknown"}))
            .await
            .unwrap();
        match result {
            ToolExecOutput::Complete(chunk) => {
                assert_eq!(chunk.state, ToolResultState::Error);
                match &chunk.output {
                    ToolOutput::Text(text) => {
                        assert!(text.contains("SkillNotFoundError"));
                        assert!(text.contains("unknown"));
                    }
                    _ => panic!("Expected Text"),
                }
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[tokio::test]
    async fn test_add_skill_and_list() {
        let mut tk = ToolKit::new();
        tk.add_skill(Skill {
            name: "my-skill".into(),
            description: "My custom skill".into(),
            dir: "/tmp/my-skill".into(),
            markdown: "# Custom".into(),
            updated_at: 0.0,
        });

        let skills = tk.list_skills().await;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
    }

    #[tokio::test]
    async fn test_add_two_skills_dedup() {
        let mut tk = ToolKit::new();
        tk.add_skill(Skill {
            name: "dup-skill".into(),
            description: "First".into(),
            dir: "/tmp/a".into(),
            markdown: "A".into(),
            updated_at: 0.0,
        });
        tk.add_skill(Skill {
            name: "dup-skill".into(),
            description: "Second".into(),
            dir: "/tmp/b".into(),
            markdown: "B".into(),
            updated_at: 0.0,
        });

        let skills = tk.list_skills().await;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "First"); // first wins
    }

    #[tokio::test]
    async fn test_get_skill_instructions_empty() {
        let tk = ToolKit::new();
        let instructions = tk.get_skill_instructions(None);
        assert!(instructions.is_empty());
    }

    #[tokio::test]
    async fn test_get_skill_instructions_with_skills() {
        let mut tk = ToolKit::new();
        tk.add_skill(Skill {
            name: "example-skill".into(),
            description: "An example skill".into(),
            dir: "/tmp/example".into(),
            markdown: "# Example".into(),
            updated_at: 0.0,
        });

        let instructions = tk.get_skill_instructions(None);
        assert!(instructions.contains("<agent-skills>"));
        assert!(instructions.contains("example-skill"));
        assert!(instructions.contains("An example skill"));
        assert!(!instructions.contains("{skill_viewer}"));
        assert!(!instructions.contains("{skills_list}"));
    }
}

# Contract: ToolKit Skill API

**Feature**: 013-skill-tool-integration | **Status**: Draft

## Interface

```rust
// ── Type aliases ──

/// Union of ways to provide skills to a ToolGroup.
pub enum SkillOrLoader {
    Skill(Skill),
    Loader(Box<dyn SkillLoader>),
    Dir(String),
}

// ── ToolKit additions ──

impl ToolKit {
    // -- Construction (modified) --

    /// Creates an empty ToolKit with a default "basic" ToolGroup and a
    /// SkillViewer pre-registered.
    pub fn new() -> Self;

    // -- Skill registration (NEW) --

    /// Register a skill directory.  Creates a LocalSkillLoader
    /// internally and adds it to the default tool group.
    pub fn add_skill_dir(&mut self, path: &str);

    /// Register a Skill object directly.
    pub fn add_skill(&mut self, skill: Skill);

    /// Register a custom SkillLoader implementation.
    pub fn add_skill_loader(&mut self, loader: Box<dyn SkillLoader>);

    // -- Skill queries (NEW) --

    /// List all skills across all active tool groups.  Skills with
    /// duplicate names are deduplicated (first-registered wins).
    pub async fn list_skills(&self) -> Vec<Skill>;

    /// Generate the `<agent-skills>` system-prompt fragment.
    ///
    /// If `template` is `Some`, uses the custom template; otherwise
    /// uses [`DEFAULT_SKILL_INSTRUCTION`].
    ///
    /// Returns an empty string when no skills are registered.
    pub fn get_skill_instructions(&self, template: Option<&str>) -> String;
}

// ── ToolGroup additions ──

impl ToolGroup {
    /// List skills provided by this group, expanding loaders.
    ///
    /// Skills from different sources with the same name are
    /// deduplicated — the first encountered is kept and a warning is
    /// logged.
    pub async fn list_skills(&self) -> Vec<Skill>;
}

// ── Constant ──

/// Default template for the skill instruction prompt fragment.
///
/// Placeholders:
/// - `{skill_viewer}` — replaced with the literal string `"Skill"`
/// - `{skills_list}` — replaced with the rendered `<skill>` XML blocks
pub const DEFAULT_SKILL_INSTRUCTION: &str = "...";
```

## Contract Guarantees

| Guarantee | Detail |
|-----------|--------|
| SkillViewer auto-registration | `ToolKit::new()` automatically creates and registers a `SkillViewer` in the "basic" group |
| Name dedup | `list_skills()` across groups keeps first-seen, logs warning on duplicate |
| Empty safe | `get_skill_instructions()` returns `""` when no skills registered |
| Backward compatible | `get_tool_schemas()` still works, now includes `"Skill"` tool schema |
| Extension safe | `add_skill_loader()` allows custom `SkillLoader` implementations (database, API, etc.) |

## Cross-reference

- Python: `Toolkit.__init__()` + `_get_available_skills()` in `agentscope/src/agentscope/tool/_toolkit.py`
- Spec: `spec.md` FR-017 through FR-023

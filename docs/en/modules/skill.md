# Skill System / Skill

> One-liner: The Skill system lets you define reusable Agent instructions and capabilities as Markdown files — the `agent_scope_tool` side handles skill loading and tool conversion, while the `agent_scope_workspace` side handles skill file management, indexing, and lifecycle.

## 1. Module Overview (Overview)

The Skill system is distributed across two crates:

| Location | Component | Responsibility |
|----------|-----------|----------------|
| `agent_scope_tool` | `SkillLoader`, `LocalSkillLoader`, `SkillViewer` | Loading skills from the filesystem, converting skills to Tools, listing skills |
| `agent_scope_workspace` | `Skill`, `SkillEntry`, `SkillManager`, `SkillsIndex` | Skill data model, file indexing, load/unload lifecycle |

**Core idea**: A Skill file is just a Markdown file with YAML frontmatter for metadata and a Markdown body as instructions sent to the model. `SkillLoader` parses `.md` files into Tools consumable by Agents.

**When to use**: Injecting domain knowledge into Agents; writing reusable prompt templates; hot-loading new capabilities from the filesystem.

**Prerequisites**: [Agent System](./agent.md), [Tool System](./tool.md), [Workspace](./workspace.md)

## 2. Core Concepts & Main Public Types (Core Concepts)

### 2.1 Skill File Format

A Skill is a Markdown file with YAML frontmatter:

```markdown
---
name: weather-reporter
description: Report weather for a given city
---

You are a weather reporter. When asked about weather:
1. Use the `get_weather` tool if available
2. Format the response in a friendly way
3. Mention temperature, humidity, and conditions
```

### 2.2 `Skill` Data Model

| Field | Description |
|-------|-------------|
| `name` | Unique identifier (parsed from filename or frontmatter) |
| `description` | One-line description, used in `SkillsIndex` |
| `content` | Markdown body, injected as system prompt |

### 2.3 `SkillLoader` trait and `LocalSkillLoader`

```rust
pub trait SkillLoader: Send + Sync {
    async fn load(&self, path: &str) -> Result<Skill, ToolError>;
    async fn load_dir(&self, dir: &str) -> Result<Vec<Skill>, ToolError>;
}
```

`LocalSkillLoader` loads `.md` files from the local filesystem.

### 2.4 `SkillViewer`

Formats a skill list into an Agent-readable format:

```rust
pub struct SkillViewer;
impl SkillViewer {
    pub fn format_skills(skills: &[Skill]) -> String;
}
```

The default instruction `DEFAULT_SKILL_INSTRUCTION` tells the Agent how to use loaded skills.

### 2.5 `SkillManager` (workspace side)

`SkillManager` manages the full skill lifecycle:
- `load(path)` — load a single skill file
- `load_dir(dir)` — load all skills from a directory
- `unload(name)` — unload a skill
- `list()` — list all loaded skills
- `index()` — generate `SkillsIndex` (for context injection)

### 2.6 Skill Tool Conversion

`agent_scope_tool` provides patterns for injecting skills as Tools into a `ToolKit`:
- `SkillOrLoader` enum — either a pre-loaded `Skill` or a path to load
- Skill Tools appear as available tools during Agent reasoning

## 3. Quick Example (Quick Example)

```rust
use agent_scope_tool::{LocalSkillLoader, SkillLoader, SkillViewer};

let loader = LocalSkillLoader::new("/path/to/skills");

// Load a single skill
let skill = loader.load("weather-reporter.md").await?;

// Load an entire directory
let skills = loader.load_dir("/path/to/skills").await?;

// Format for display
let view = SkillViewer::format_skills(&skills);
println!("{}", view);
```

Workspace-side usage:

```rust
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let config = LocalWorkspaceConfig {
    workdir: "/tmp/ws".into(),
    skill_paths: vec!["/path/to/skills".into()],
    ..Default::default()
};
let mut ws = LocalWorkspace::new(config);
ws.initialize().await?;
// Skills are auto-loaded and accessible via workspace's skill_manager
```

## 4. Key Usage Patterns (Usage Patterns)

### 4.1 Registering a Skill as a Tool

```rust
use agent_scope_tool::{FunctionTool, ToolKit};

let skill = loader.load("code-reviewer.md").await?;
let tool = FunctionTool::new(
    "skill_code_reviewer",
    "Use this to get code review guidance",
    move |_: serde_json::Value| {
        let content = skill.content.clone();
        async move { content }
    },
);
toolkit.register(tool);
```

### 4.2 Skill Directory Structure

```
skills/
├── code-reviewer.md
├── weather-reporter.md
└── data-analyzer.md
```

Each `.md` file defines frontmatter with:
- `name`: unique identifier
- `description`: brief summary

### 4.3 Workspace Integration

During initialization, the workspace auto-loads skills from `skill_paths`. Agents can access the loaded skill list through workspace tools.

## 5. Errors & Unsupported Capabilities (Errors & Unsupported)

| Error | Cause |
|-------|-------|
| `ToolError::NotFound` | Skill file not found |
| `ToolError::InvalidInput` | Invalid skill file frontmatter format |
| `WorkspaceError::InvalidSkill` | Skill loading failure in workspace |

**Unsupported**:
- Remote skill loading (URL fetching) is out of scope.
- Inter-skill dependencies are not defined.
- Hot reload (real-time file change detection) is not implemented.

## 6. Compatibility (Compatibility)

- **Compatibility Level**: **L2** (core skill behavior)
- **Authority**: `specs/013-skill-tool-integration/spec.md`
- **Known Deviations**:
  - Rust side splits Skill into tool-side (loading) and workspace-side (management); Python side is more centralized
  - `DEFAULT_SKILL_INSTRUCTION` content may differ slightly from Python version

## 7. See Also (Related Modules)

- [Tool System](./tool.md) — How Skills are converted to Tools
- [Workspace](./workspace.md) — Skill management within workspace
- [Agent System](./agent.md) — How Agents consume Skills

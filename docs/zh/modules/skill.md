# 技能系统 / Skill

> 一句话定位：Skill 系统允许用 Markdown 文件定义可复用的 Agent 指令和能力——`agent_scope_tool` 侧负责技能加载与工具化转换，`agent_scope_workspace` 侧负责技能文件管理、索引与生命周期。

## 1. 模块概述 (Overview)

Skill 系统分布在两个 crate 中：

| 位置 | 组件 | 职责 |
|------|------|------|
| `agent_scope_tool` | `SkillLoader`、`LocalSkillLoader`、`SkillViewer` | 从文件系统加载技能、将技能转换为 Tool、技能列表查看 |
| `agent_scope_workspace` | `Skill`、`SkillEntry`、`SkillManager`、`SkillsIndex` | 技能数据模型、文件索引、加载/卸载生命周期 |

**核心理念**：Skill 文件就是 Markdown 文件，其 frontmatter 定义元数据，正文为发给模型的指令。`SkillLoader` 将 `.md` 文件解析为可供 Agent 使用的 Tool。

**适用场景**：为 Agent 注入领域知识；编写可复用的 prompt 模板；通过文件系统热加载新能力。

**前置阅读**：[Agent 系统](./agent.md)、[工具系统](./tool.md)、[工作空间](./workspace.md)

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 技能文件格式

一个 Skill 是带有 YAML frontmatter 的 Markdown 文件：

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

### 2.2 `Skill` 数据模型

| 字段 | 说明 |
|------|------|
| `name` | 唯一标识符（从文件名或 frontmatter 解析） |
| `description` | 一行描述，用于 `SkillsIndex` |
| `content` | Markdown 正文，作为系统提示词注入 |

### 2.3 `SkillLoader` trait 与 `LocalSkillLoader`

```rust
pub trait SkillLoader: Send + Sync {
    async fn load(&self, path: &str) -> Result<Skill, ToolError>;
    async fn load_dir(&self, dir: &str) -> Result<Vec<Skill>, ToolError>;
}
```

`LocalSkillLoader` 从本地文件系统加载 `.md` 文件。

### 2.4 `SkillViewer`

将技能列表展示为 Agent 可理解的格式：

```rust
pub struct SkillViewer;
impl SkillViewer {
    pub fn format_skills(skills: &[Skill]) -> String;
}
```

默认指令 `DEFAULT_SKILL_INSTRUCTION` 告诉 Agent 如何利用已加载的技能。

### 2.5 `SkillManager`（workspace 侧）

`SkillManager` 管理技能的完整生命周期：
- `load(path)` — 加载单个技能文件
- `load_dir(dir)` — 加载目录下所有技能
- `unload(name)` — 卸载一个技能
- `list()` — 列出所有已加载的技能
- `index()` — 生成 `SkillsIndex`（供上下文注入）

### 2.6 技能工具化

`agent_scope_tool` 提供了将技能作为 Tool 注入 `ToolKit` 的模式：
- `SkillOrLoader` 枚举 — 既可以是已加载的 `Skill`，也可以是待加载的路径
- 技能 Tool 在 Agent reasoning 阶段作为可用的工具出现

## 3. 快速示例 (Quick Example)

```rust
use agent_scope_tool::{LocalSkillLoader, SkillLoader, SkillViewer};

let loader = LocalSkillLoader::new("/path/to/skills");

// 加载单个技能
let skill = loader.load("weather-reporter.md").await?;

// 加载整个目录
let skills = loader.load_dir("/path/to/skills").await?;

// 格式化展示
let view = SkillViewer::format_skills(&skills);
println!("{}", view);
```

Workspace 侧用法：

```rust
use agent_scope_workspace::{LocalWorkspace, LocalWorkspaceConfig, WorkspaceBase};

let config = LocalWorkspaceConfig {
    workdir: "/tmp/ws".into(),
    skill_paths: vec!["/path/to/skills".into()],
    ..Default::default()
};
let mut ws = LocalWorkspace::new(config);
ws.initialize().await?;
// 技能已自动加载并可通过 workspace 的 skill_manager 访问
```

## 4. 关键用法模式 (Usage Patterns)

### 4.1 将技能注册为 Tool

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

### 4.2 技能目录结构

```
skills/
├── code-reviewer.md
├── weather-reporter.md
└── data-analyzer.md
```

每个 `.md` 文件中 frontmatter 定义：
- `name`：唯一标识
- `description`：简要说明

### 4.3 与 Workspace 集成

Workspace 初始化时自动加载 `skill_paths` 中的技能，Agent 可以通过 workspace 工具访问已加载的技能列表。

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 原因 |
|------|------|
| `ToolError::NotFound` | 技能文件不存在 |
| `ToolError::InvalidInput` | 技能文件 frontmatter 格式错误 |
| `WorkspaceError::InvalidSkill` | workspace 中技能加载失败 |

**不支持**：
- 远程技能加载（URL 获取）不在当前范围。
- 技能之间的依赖关系未定义。
- 技能热重载（文件变更实时生效）未实现。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L2**（核心技能行为）
- **权威来源**: `specs/013-skill-tool-integration/spec.md`
- **已知偏差**:
  - Rust 侧将 Skill 拆分为 tool 侧（加载）和 workspace 侧（管理），Python 侧更集中
  - `DEFAULT_SKILL_INSTRUCTION` 内容可能与 Python 版本略有差异

## 7. 相关模块 (See Also)

- [工具系统](./tool.md) — Skill 如何转换为 Tool
- [工作空间](./workspace.md) — 技能在 workspace 中的管理
- [Agent 系统](./agent.md) — Agent 如何消费技能

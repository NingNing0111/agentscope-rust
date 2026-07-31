# Data Model: Skill Tool Integration

**Feature**: 013-skill-tool-integration | **Date**: 2026-07-31

## Entity Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     ToolKit (modified)                          │
│  tools: HashMap<String, Box<dyn Tool>>                          │
│  tool_groups: Vec<ToolGroup>                       ← NEW        │
│  skill_viewer: SkillViewer                          ← NEW        │
│  ────────────────────────────────────────────────────           │
│  + register(tool)                                  (existing)   │
│  + call_tool(tool_call)                            (existing)   │
│  + get_tool_schemas()                              (existing)   │
│  + add_skill_dir(path)                             ← NEW        │
│  + add_skill(skill: Skill)                         ← NEW        │
│  + add_skill_loader(loader)                        ← NEW        │
│  + list_skills() -> Vec<Skill>                      ← NEW        │
│  + get_skill_instructions(template?) -> String      ← NEW        │
└───────────────┬─────────────────────────────────────────────────┘
                │ owns
                ▼
┌─────────────────────────────────────────────────────────────────┐
│                     ToolGroup (modified)                         │
│  name: String                                                   │
│  description: String                                            │
│  tools: Vec<Box<dyn Tool>>                     (existing)       │
│  skills_or_loaders: Vec<SkillOrLoader>          ← NEW           │
│  mcps: Vec<...>                                 (existing)       │
│  ────────────────────────────────────────────────────           │
│  + list_skills() -> Vec<Skill>                   ← NEW           │
└───────────────┬─────────────────────────────────────────────────┘
                │ contains
                ▼
┌─────────────────────────────────────────────────────────────────┐
│                  SkillOrLoader (enum)            ← NEW           │
│  ┌──────────────────────────────────────────────┐               │
│  │ Skill(Skill)         — direct value           │               │
│  │ Loader(Box<dyn SkillLoader>) — async loader   │               │
│  │ Dir(String)          — auto→LocalSkillLoader  │               │
│  └──────────────────────────────────────────────┘               │
└───────────────┬─────────────────────────────────────────────────┘
                │ uses
                ▼
┌─────────────────────────────────────────────────────────────────┐
│              SkillLoader (trait)                 ← NEW           │
│  ┌──────────────────────────────────────────────┐               │
│  │ async fn list_skills(&self) -> Vec<Skill>     │               │
│  └──────────────────────────────────────────────┘               │
│                         ▲                                        │
│                         │ implements                             │
│  ┌──────────────────────────────────────────────┐               │
│  │         LocalSkillLoader (struct)             │               │
│  │  directory: String                            │               │
│  │  scan_subdir: bool                            │               │
│  │  _cache: HashMap<String, Skill>  (private)    │               │
│  └──────────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│              SkillViewer (struct)                ← NEW           │
│  ────────────────────────────────────────────────────           │
│  implements Tool trait                                           │
│  ────────────────────────────────────────────────────           │
│  name() -> "Skill"                                               │
│  description() -> "Retrieve a skill within the conversation..."  │
│  input_schema() -> {"skill": "string"}                           │
│  ────────────────────────────────────────────────────           │
│  _get_skills_method: ListSkillsCallback         ← callback       │
│  ────────────────────────────────────────────────────           │
│  call(input) -> ToolExecOutput                                   │
│    1. extract "skill" from input                                 │
│    2. invoke _get_skills_method with activated_groups            │
│    3. lookup skill name in map                                   │
│    4. return markdown or error ToolChunk                         │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│              Skill (from agent_scope_workspace)     (existing)   │
│  name: String                                                    │
│  description: String                                             │
│  dir: String                                                     │
│  markdown: String                                                │
│  updated_at: f64                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## 1. SkillLoader (trait) — NEW

**Purpose**: 抽象 skill 加载接口，定义从任意来源获取 skills 的能力。

| Method | Signature | Description |
|--------|-----------|-------------|
| `list_skills` | `async fn list_skills(&self) -> Vec<Skill>` | 返回此 loader 能提供的所有 skills |

**Super trait bounds**: `Send + Sync`

**Validation**: 无（trait 本身无状态）

## 2. LocalSkillLoader (struct) — NEW

**Purpose**: 从本地文件系统目录扫描 SKILL.md 文件并解析为 `Skill` 对象。

| Field | Type | Description |
|-------|------|-------------|
| `directory` | `String` | 要扫描的根目录绝对路径 |
| `scan_subdir` | `bool` | 是否递归扫描子目录 |
| `_cache` | `HashMap<String, Skill>` | 缓存：key=skill_dir_path, value=Skill（私有） |

**Relationship**: implements `SkillLoader` trait

**State transitions**: 无状态机（纯查询对象，缓存内部管理）

**Validation**:
- `directory` 不存在时返回空列表（不报错）
- name 或 description 缺失的 SKILL.md → 跳过 + `tracing::warn!`
- frontmatter 格式错误 → 跳过 + warn
- 缓存基于 `updated_at` (mtime) 检测变化

## 3. SkillOrLoader (enum) — NEW

**Purpose**: Tagged union 表示可注册到 ToolGroup 的 skill 来源。

| Variant | Contains | Description |
|---------|----------|-------------|
| `Skill` | `agent_scope_workspace::Skill` | 直接传入的 Skill 对象 |
| `Loader` | `Box<dyn SkillLoader>` | SkillLoader trait object |
| `Dir` | `String` | 目录路径，内部自动转为 `LocalSkillLoader { scan_subdir: true }` |

**Validation**: `Dir` 变体在 `list_skills()` 时懒验证（目录不存在则返回空列表）

## 4. SkillViewer (struct) — NEW

**Purpose**: 实现 `Tool` trait 的内置工具，供 Agent 调用以获取 skill 内容。

| Field | Type | Description |
|-------|------|-------------|
| `_get_skills_method` | `ListSkillsCallback` | 回调：输入 `&[String]`（激活 group 名列表），返回 `HashMap<String, Skill>` |

**Implements**: `Tool` trait

**Tool metadata**:
- `name()` → `"Skill"`
- `description()` → `"Retrieve a skill within the conversation. When users ask you to perform tasks, check if any of the available skills match. Skills provide specialized capabilities and domain knowledge."`
- `input_schema()` → `{"type": "object", "properties": {"skill": {"type": "string", "description": "The exact name of the skill to view."}}, "required": ["skill"]}`
- `is_concurrency_safe()` → `true`
- `is_read_only()` → `true`

**call() behavior**:
1. 从 `input` JSON 中提取 `skill` 字符串
2. 调用 `_get_skills_method` 获取当前可用的 skills map
3. 在 map 中查找 skill 名称：
   - 找到 → `Ok(ToolExecOutput::Complete(chunk))`，`chunk.state = Success`，`chunk.output = Text(skill.markdown)`
   - 未找到 → `Ok(ToolExecOutput::Complete(chunk))`，`chunk.state = Error`，`chunk.output = Text("SkillNotFoundError: Skill '<name>' not found.")`
4. 回调异常 → `Ok(ToolExecOutput::Complete(chunk))` with error message（不传播异常）
5. `input` 不包含 `skill` 字段 → 同上 error 处理

## 5. ToolGroup (struct) — MODIFIED

**Purpose**: 扩展 Python 对齐的 ToolGroup，新增 skill 支持。

**New fields**:

| Field | Type | Description |
|-------|------|-------------|
| `skills_or_loaders` | `Vec<SkillOrLoader>` | 注册的 skills / loaders / 目录路径 |

**New methods**:

| Method | Signature | Description |
|--------|-----------|-------------|
| `list_skills` | `async fn list_skills(&self) -> Vec<Skill>` | 展开所有 loader 并合并，按名称去重（先注册优先） |

**Existing fields**: `name`, `description`, `tools`, `mcps`（不变）

## 6. ToolKit (struct) — MODIFIED

**Purpose**: 注册中心，管理 tools + tool groups + skills。

**New fields**:

| Field | Type | Description |
|-------|------|-------------|
| `tool_groups` | `Vec<ToolGroup>` | （如果尚未存在）工具组列表 |
| `skill_viewer` | `SkillViewer` | 内置的 Skill 查看工具实例 |

**New methods**:

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_skill_dir` | `fn add_skill_dir(&mut self, path: &str)` | 向默认 ToolGroup 注册 skill 目录 |
| `add_skill` | `fn add_skill(&mut self, skill: Skill)` | 向默认 ToolGroup 注册 Skill 对象 |
| `add_skill_loader` | `fn add_skill_loader(&mut self, loader: Box<dyn SkillLoader>)` | 向默认 ToolGroup 注册 loader |
| `list_skills` | `async fn list_skills(&self) -> Vec<Skill>` | 从所有激活的 ToolGroup 收集 skills |
| `get_skill_instructions` | `fn get_skill_instructions(&self, template: Option<&str>) -> String` | 渲染 skill 的 system prompt 片段 |

**Existing fields**: `tools`（不变，SkillViewer 自动注册到其中）

## 7. ToolError (enum) — MODIFIED

**New variant**:

| Variant | Fields | Description |
|---------|--------|-------------|
| `SkillNotFound` | `{ skill_name: String }` | SkillViewer 回调中 skill 未找到 |

**Existing variants**: `NotFound`, `InvalidInput`, `Execution`, `Interrupted`（不变）

## 8. ListSkillsCallback (type alias) — NEW

```rust
pub type ListSkillsCallback = Box<dyn Fn(&[String]) -> HashMap<String, Skill> + Send + Sync>;
```

**Purpose**: SkillViewer 构造参数——在每次调用时从激活的 ToolGroup 动态收集 skills。

## 9. DEFAULT_SKILL_INSTRUCTION (constant) — NEW

```text
<agent-skills>
Skills are a collection of instructions, scripts, and resources to extend your capabilities.

**IMPORTANT**: Skills are NOT tools, and you cannot call a skill directly. To use a skill, you MUST use the `{skill_viewer}` tool to read the skill's full instructions, and then follow those instructions to use the tools and resources provided by the skill.

# Available Skills:
{skills_list}
</agent-skills>
```

其中 `{skill_viewer}` 替换为 `"Skill"`，`{skills_list}` 替换为每个 skill 的迭代渲染：

```xml
<skill>
<name>{skill.name}</name>
<description>{skill.description}</description>
<dir>{skill.dir}</dir>
</skill>
```

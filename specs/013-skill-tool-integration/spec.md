# Feature Specification: Skill Tool Integration

**Feature Branch**: `013-skill-tool-integration`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description: "参考 Python agentscope 的 Skill 功能，将 Skill 的 Tool/Agent 集成层从 Python 重构到 Rust，使 Agent 可以通过 SkillViewer 工具获取和使用 Skill"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Agent 通过 SkillViewer 工具获取 Skill 内容 (Priority: P1)

作为 Agent，我需要在对话过程中调用名为 `Skill` 的内置工具，传入 skill 名称，获取对应 SKILL.md 的完整 markdown 正文，以便我能按照 skill 中的指令来使用工具和资源。

**Why this priority**: 这是 Skill 功能的核心——Agent 必须能够读取 skill 内容才能使用它。没有 SkillViewer 工具，Workspace 中存储的 skills 对 Agent 完全不可见。

**Independent Test**: 通过注册一个包含 SKILL.md 的 skill，让 Agent 调用 `Skill` 工具并验证返回的 markdown 内容与 SKILL.md 正文一致。

**Acceptance Scenarios**:

1. **Given** ToolKit 中注册了一个名为 `my-skill` 的 skill（其 SKILL.md 包含 `# My Skill\n\nInstructions here`），**When** Agent 调用 `Skill` 工具并传入 `{"skill": "my-skill"}`，**Then** 工具返回包含 markdown 正文 `# My Skill\n\nInstructions here` 的 `ToolChunk`，状态为 SUCCESS。
2. **Given** ToolKit 中没有名为 `nonexistent` 的 skill，**When** Agent 调用 `Skill` 工具并传入 `{"skill": "nonexistent"}`，**Then** 工具返回错误状态的 `ToolChunk`，包含 `SkillNotFoundError` 消息。
3. **Given** ToolKit 中有同名的两个 skill（来自不同 ToolGroup 的重复名称），**When** Agent 调用 `Skill` 工具并传入该名称，**Then** 返回第一个匹配的 skill 内容（按 ToolGroup 注册顺序）。

---

### User Story 2 - 开发者注册 Skill 到 ToolKit/ToolGroup (Priority: P2)

作为开发者，我需要通过 ToolKit 或 ToolGroup API 注册 skill 目录路径、`Skill` 对象、或 `SkillLoader`，使得注册的 skills 能被 Agent 的 SkillViewer 工具发现和调用。

**Why this priority**: Skill 注册是 SkillViewer 的数据来源。虽然 Agent 通过 SkillViewer 获取 skill 是最关键的流程，但如果没有注册能力，系统中就没有可用 skill。

**Independent Test**: 通过 ToolKit API 注册一个 `LocalSkillLoader` 指向包含 3 个 SKILL.md 的目录，调用 `list_skills()` 验证返回 3 个 Skill 对象。

**Acceptance Scenarios**:

1. **Given** 一个包含有效 `SKILL.md` 的目录，**When** 开发者通过 `ToolKit.add_skill_dir(path)` 注册该目录，**Then** 该 skill 被加入默认 ToolGroup 的 skills 列表，可通过 `list_skills()` 查询到。
2. **Given** 一个不包含 `SKILL.md` 的目录，**When** 开发者尝试注册，**Then** 系统返回错误，提示 SKILL.md 缺失。
3. **Given** 开发者注册了 skill 目录 A（含 `a-skill`）和 skill 目录 B（含 `b-skill`），**When** 调用 `ToolKit.list_skills()`，**Then** 返回包含 `a-skill` 和 `b-skill` 的列表。

---

### User Story 3 - Agent System Prompt 中包含可用 Skill 列表 (Priority: P3)

作为系统，我需要在 Agent 的 system prompt 中注入可用 skills 的摘要列表（名称、描述、目录），引导 Agent 在适当时机使用 `Skill` 工具获取更详细的 skill 内容。

**Why this priority**: Prompt 注入优化了 Agent 的 skill 发现体验，但即使没有它，Agent 仍可通过工具 schema 发现 `Skill` 工具的存在。这是增强性功能。

**Independent Test**: 注册 2 个 skills 后，获取 ToolKit 生成的 skill instruction prompt，验证其中包含 `<agent-skills>` XML 块，内含每个 skill 的 `<name>`、`<description>`、`<dir>` 元素。

**Acceptance Scenarios**:

1. **Given** ToolKit 中注册了 skills `["code-review", "deploy"]`，**When** 调用 `ToolKit.get_skill_instructions()`，**Then** 返回包含 `<agent-skills>` 标签的文本，内含每个 skill 的 `<skill>` 块，按原 skill instruction 模板格式排列。
2. **Given** ToolKit 中没有注册任何 skill，**When** 调用 `ToolKit.get_skill_instructions()`，**Then** 返回空字符串（不注入 skill 相关的 prompt 片段）。
3. **Given** 开发者自定义了 skill instruction 模板，**When** 调用 `ToolKit.get_skill_instructions()`，**Then** 返回使用自定义模板渲染的内容。

---

### User Story 4 - 独立 SkillLoader 扫描和缓存 (Priority: P2)

作为开发者，我需要使用 `LocalSkillLoader` 从指定目录扫描所有 `SKILL.md` 文件，自动解析 frontmatter 元数据和 markdown 正文，并以 `Skill` 对象列表的形式获取结果。Loader 应支持缓存（基于文件修改时间更新），以及并发加载多个 skill。

**Why this priority**: LocalSkillLoader 是与 Workspace 解耦的 skill 加载机制，适用于不需要完整 Workspace 的轻量场景。与 US2（ToolKit 注册）同级重要，因为它是注册的主要数据来源。

**Independent Test**: 创建一个包含 3 个不同 SKILL.md 的目录（每种子目录一个），用 `LocalSkillLoader` 扫描，验证返回 3 个 Skill 对象，且 name/description/markdown/updated_at 字段正确。

**Acceptance Scenarios**:

1. **Given** 目录 `/skills` 下有子目录 `skill-a/`（含 SKILL.md）和 `skill-b/`（含 SKILL.md），**When** `LocalSkillLoader` 以 `scan_subdir=true` 扫描，**Then** 返回 2 个 Skill 对象，name 分别为 SKILL.md frontmatter 中定义的名称。
2. **Given** 目录第一次被扫描并缓存，**When** 修改 `skill-a/SKILL.md` 后再次扫描，**Then** `skill-a` 的缓存被更新（`updated_at` 变化），`skill-b` 的缓存被复用（`updated_at` 不变）。
3. **Given** 目录中有 10 个 skill 子目录，**When** `LocalSkillLoader.list_skills()` 被调用，**Then** 所有 SKILL.md 文件被并发加载（不阻塞在单个文件的 I/O 上）。
4. **Given** SKILL.md 的 frontmatter 中缺少 `name` 字段，**When** 扫描该目录，**Then** 该 skill 被跳过（不在返回列表中），并发出警告日志。

---

### Edge Cases

- 当 skill 的 SKILL.md frontmatter 格式错误（非 YAML）时，系统必须以优雅降级的方式处理，记录警告并跳过该 skill，而不是崩溃或返回错误。
- 当两个 skill 具有相同的 `name`（来自不同目录或不同 loader），ToolKit 必须去重（保留先注册的），并记录警告。
- 当 `SkillViewer` 工具被调用时，如果底层 `_get_skills_method` 抛出异常，工具必须捕获异常并返回错误状态的 ToolChunk。
- 当 skill 目录在加载过程中被外部删除，`list_skills()` 必须处理文件不存在的情况并返回空列表或跳过该 skill。
- 当 skill 的 markdown 正文为空（frontmatter 之后无内容）时，SkillViewer 仍应返回空正文的成功响应。
- 当同一 hash 的 skill 被重复添加到 ToolKit 时，应略过（基于 SHA-256 去重）。

## Requirements *(mandatory)*

### Functional Requirements

#### SkillViewer Tool

- **FR-001**: 系统 MUST 提供 `SkillViewer` 工具，工具名称为 `"Skill"`，描述为引导 Agent 在需要时使用该工具来获取 skill 的详细指令。
- **FR-002**: `SkillViewer` MUST 接受 `{"skill": "<name>"}` 的 JSON Schema 输入参数，其中 `skill` 为必填字符串字段。
- **FR-003**: `SkillViewer` MUST 标记为只读 (`is_read_only: true`) 和并发安全 (`is_concurrency_safe: true`)。
- **FR-004**: `SkillViewer` 在调用时 MUST 通过注入的回调方法 (`get_skills_method`) 获取当前可用的 skills 映射（`HashMap<String, Skill>`）。
- **FR-005**: 当请求的 skill 名称存在于映射中时，`SkillViewer` MUST 返回包含 `skill.markdown` 正文的成功 ToolChunk。
- **FR-006**: 当请求的 skill 名称不存在时，`SkillViewer` MUST 返回错误状态的 ToolChunk，包含 `"SkillNotFoundError: Skill '<name>' not found."` 消息。
- **FR-007**: `SkillViewer` 的权限检查 MUST 始终返回 ALLOW（skill 查看始终被允许）。

#### SkillLoader Trait & LocalSkillLoader

- **FR-008**: 系统 MUST 定义 `SkillLoader` trait，包含唯一的 `async fn list_skills(&self) -> Vec<Skill>` 方法。
- **FR-009**: `LocalSkillLoader` MUST 实现 `SkillLoader` trait，接受 `directory: String` 和 `scan_subdir: bool` 配置参数。
- **FR-010**: `LocalSkillLoader` MUST 在 `scan_subdir=false` 时只扫描根目录下的 `SKILL.md`（当前目录含 SKILL.md 即视为一个 skill）。
- **FR-011**: `LocalSkillLoader` MUST 在 `scan_subdir=true` 时递归扫描所有直接子目录中是否有 SKILL.md。
- **FR-012**: `LocalSkillLoader` MUST 解析 SKILL.md 的 YAML frontmatter，提取 `name` 和 `description` 字段。`name` 或 `description` 缺失时该 skill 被跳过并发出警告。
- **FR-013**: `LocalSkillLoader` MUST 基于文件的 `updated_at` (mtime) 实现缓存，仅当文件修改时间变化时才重新读取和解析。
- **FR-014**: `LocalSkillLoader` MUST 支持并发加载多个 skill（使用 `join_all` 或等效并发机制）。
- **FR-015**: `LocalSkillLoader` MUST 在目录不存在时返回空列表（不报错），并记录警告日志。
- **FR-016**: `LocalSkillLoader` MUST 在个别 skill 加载失败时继续加载其他 skills（不因一个失败而中断全部），记录警告日志。

#### ToolKit & ToolGroup Skill 集成

- **FR-017**: `ToolGroup` MUST 新增 `skills_or_loaders: Vec<SkillOrLoader>` 字段，其中 `SkillOrLoader` 是 `Skill | Box<dyn SkillLoader> | String`（路径）的枚举。
- **FR-018**: `ToolGroup` MUST 提供 `async fn list_skills(&self) -> Vec<Skill>` 方法，展开所有 loader 并合并 Skills。
- **FR-019**: `ToolKit` MUST 提供 `async fn get_skill_instructions(&self, template: Option<&str>) -> String` 方法，使用默认或自定义模板渲染 skill 的 prompt 片段。
- **FR-020**: `ToolKit` 的 `SkillViewer` 初始化 MUST 注入 `_get_available_skills` 回调，该方法从所有激活的 ToolGroup 中收集 skills 并按名称去重（先注册优先）。
- **FR-021**: 默认 skill instruction 模板 MUST 包含 `<agent-skills>` 包装器，说明 skills 不是可直接调用的工具，引导 Agent 使用 `Skill` 工具获取详细指令。模板 MUST 为每个 skill 渲染 `<name>`、`<description>`、`<dir>` 标签。
- **FR-022**: `ToolKit` MUST 支持通过 `add_skill_dir()`, `add_skill_loader()`, `add_skill()` 方法向默认 ToolGroup 注册 skill。
- **FR-023**: 当 ToolGroup 中包含两个同名 skill 时，`list_skills()` MUST 去重（保留先出现的），并记录警告。

#### Workspace 集成

- **FR-024**: `WorkspaceBase` 现有的 `list_skills()` 方法返回的 `Vec<Skill>` MUST 可以被注入到 `SkillViewer` 的 `get_skills_method` 回调中。
- **FR-025**: `WorkspaceBase` 的 `get_instructions()` SHOULD 在返回的 prompt 中包含 skill 列表摘要（通过复用 FR-021 的模板机制）。

#### 错误处理

- **FR-026**: Skill 相关错误类型 MUST 包含 `SkillNotFound { name: String }` 和 `InvalidSkill { path: String, reason: String }`（已在 `workspace::error.rs` 中定义，Tool 层可用自己的错误类型或复用这些错误）。
- **FR-027**: `SkillViewer` 回调执行异常时 MUST 被捕获并转换为错误状态的 ToolChunk，而非向上传播异常。

### Key Entities *(include if feature involves data)*

- **`Skill`** (已存在于 workspace crate): Agent 可见的 skill 元数据。包含 name、description、dir（路径）、markdown（正文）、updated_at。
- **`SkillLoader` (trait)**: 抽象 skill 加载接口，定义 `list_skills()` 方法。由 `LocalSkillLoader` 实现。
- **`LocalSkillLoader`**: 从本地文件系统目录扫描 SKILL.md 文件并解析为 Skill 对象的加载器。支持缓存和并发。
- **`SkillOrLoader` (enum)**: Tagged union，表示可注册到 ToolGroup 的 skill 来源——直接的 Skill 对象、SkillLoader 实例、或目录路径字符串。
- **`SkillViewer`**: 实现 `Tool` trait 的内置工具，名为 `"Skill"`。接收 skill 名称，通过回调获取 skills 映射，返回对应 markdown。
- **`ToolGroup`** (扩展): 新增 `skills_or_loaders` 字段和 `list_skills()` 方法。
- **`ToolKit`** (扩展): 新增 skill 注册 API、`get_skill_instructions()`、内部 `SkillViewer` 实例管理。
- **`DEFAULT_SKILL_INSTRUCTION`**: 常量模板字符串，用于生成 Agent system prompt 中的 `<agent-skills>` XML 块。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Agent 可以通过 `Skill` 工具成功获取任意已注册 skill 的完整 markdown 内容，端到端流程在 100ms 内完成（不含 I/O）。
- **SC-002**: `LocalSkillLoader` 能在 1 秒内从包含 20 个 skill 目录中完成全部扫描和加载（缓存热路径）。
- **SC-003**: 所有 Skill 相关测试覆盖率 ≥ 85%，包括 SkillViewer 工具调用、LocalSkillLoader 扫描/缓存、ToolGroup/ToolKit 注册与去重。
- **SC-004**: 并发加载 10+ skills 时不出现死锁、竞态或数据丢失（通过并发测试验证）。
- **SC-005**: 与 Python agentscope 的 Skill 功能保持协议兼容——对于相同输入（skill 注册、SkillViewer 调用），Rust 实现产生与 Python 参考实现等效的可观察输出。

## Assumptions

- `Skill` struct 已存在于 `agent_scope_workspace` crate，无需重新定义。Feature 013 可直接依赖该类型。
- `SkillViewer` 工具将实现在 `agent_scope_tool` crate 中，扩展 `ToolKit` 和 `ToolGroup`。
- `LocalSkillLoader` 将放置在 `agent_scope_tool` crate 中（与 Tool 系统同 crate），或放置在独立的 `agent_scope_skill` crate 中，具体由 plan 阶段的复杂度分析决定。
- 默认 skill instruction 模板使用与 Python 实现相同的 XML 结构（`<agent-skills>`），但不使用 Jinja2 模板引擎——改用 Rust 的字符串插值或 `Tera`/`Handlebars` 等轻量模板库。
- `SkillViewer` 的权限检查始终返回 ALLOW，符合 Python 实现的行为。
- 依赖 `serde`, `serde_json`, `sha2`, `tracing` 等已在 workspace 中声明。
- Workspace 中的 `_sanitize_dir_name()`（含 CJK 字符处理）暂不纳入 Feature 013 范围，因为 Python `LocalSkillLoader` 不直接使用它（那是 `LocalWorkspace` 的内部方法）。
- Feature 013 不改变现有 Workspace 层 Skill 存储 API，仅在 Tool/Agent 层新增集成代码。

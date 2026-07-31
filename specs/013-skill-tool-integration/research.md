# Research Document: Skill Tool Integration

**Feature**: 013-skill-tool-integration | **Date**: 2026-07-31

## Decision 1: SkillViewer — 回调模式 vs 持有 Map

**Decision**: 使用回调闭包模式 (`get_skills_method: Box<dyn Fn() -> HashMap<String, Skill>>`)

**Rationale**: Python `SkillViewer` 通过 `_get_skills_method` 注入回调，该回调在每次调用时从激活的 ToolGroup 动态收集 skills。这使得 skills 可以随 ToolGroup 的激活/停用而动态变化。如果 SkillViewer 直接持有一个静态 HashMap，则无法反映 ToolGroup 状态变化。

**Alternatives considered**:
- **持有 `HashMap<String, Skill>`**: 简单但静态，无法反映动态 group 变化。每次 group 变化需主动更新 SkillViewer 内部状态（增加同步复杂度）。
- **持有 `Arc<RwLock<HashMap<String, Skill>>>`**: 可实现动态更新但引入了额外的锁争用和生命周期管理。

**Impact**: `SkillViewer` struct 需要一个 `Box<dyn Fn(&[String]) -> HashMap<String, Skill> + Send + Sync>` 类型的回调字段（接受激活 group 名称列表）。ToolKit 注入的回调从自身的 tool groups 中动态提取 skills。

## Decision 2: YAML Frontmatter 解析

**Decision**: 使用 `serde_yml` (formerly `serde_yaml`) 配合手动行解析进行 frontmatter 提取

**Rationale**: Python 使用 `python-frontmatter` 库解析 `---\n...\n---` 分隔的 YAML frontmatter。Rust 生态中没有直接等效的单文件库。我们只需要提取 name 和 description 两个简单字段，不需要完整的 YAML 类型系统。使用手写的 frontmatter 分隔逻辑（查找 `---` 标记）配合 `serde_yml` 解析 YAML 部分。

前端 delimiter 解析逻辑：
1. 检查字符串是否以 `---` 开头
2. 找到下一个 `---` 分隔符
3. 对中间的 YAML 部分使用 `serde_yml::from_str::<serde_json::Value>()` 
4. 提取 `name` 和 `description` 字符串字段
5. 保留 `---` 之后的 markdown 正文

**Alternatives considered**:
- **`serde_yaml`**: 已 deprecated，迁移到 `serde_yml`
- **纯手写 YAML 行解析**: 对于简单 key-value 够用但不健壮（无法处理引号、多行值等）
- **`yaml-rust`**: 更重依赖，API 不如 serde 风格友好

**Note**: Rust 中 `workspace::skill.rs` 已有 `parse_skill_md()` 函数用于手写 frontmatter 解析（行级 `name:` / `description:` 前缀匹配）。Tool crate 的 `LocalSkillLoader` 也应使用相同算法以保持一致性，但需确认是否需要引入 `serde_yml`。**最终选择**：复用 workspace crate 已有的 `parse_skill_md()` 逻辑（纯行解析，无外部 YAML 库依赖），直接从 `agent_scope_workspace::skill::parse_skill_md` 暴露（或复制该辅助函数）。

## Decision 3: Skill Instruction 模板系统

**Decision**: 使用简单字符串插值 + `replace` 宏模式，不引入模板引擎

**Rationale**: Python 使用 Jinja2 模板引擎渲染 `DEFAULT_SKILL_INSTRUCTION`。Rust 侧的模板仅有两个变量：`skill_viewer`（工具名称）和 `skills`（skill 列表的循环渲染）。复杂度和模板规模不值得引入 `Tera`、`Handlebars` 等依赖。

实现方式：
- `DEFAULT_SKILL_INSTRUCTION` 常量包含 `<agent-skills>` 模板，skills 列表通过字符串拼接渲染
- `get_skill_instructions()` 接受 `Option<&str>` 的自定义模板参数（以覆盖默认值）
- 模板中的 `{{ skill_viewer }}` 占位符被替换为 `"Skill"`（工具名称）
- skills 列表通过迭代 `Vec<Skill>` 并格式化为 `<skill>` XML 块来渲染

**Alternatives considered**:
- **Tera**: 最接近 Jinja2 但引入多个依赖 (`tera`, `pest`, `unic-segment`等)，编译时间增加 5-10s
- **Handlebars**: 较轻量但仍有额外依赖
- **纯 `format!()` 宏**: 对于固定模板结构最直接，但自定义模板功能受限

**Impact**: 如果支持自定义模板，需要实现一个 minimal 的模板引擎或限制自定义模板只允许替换 `{skills}` 和 `{skill_viewer}` 占位符。

## Decision 4: ToolError 扩展

**Decision**: 在 `ToolError` enum 中新增 `SkillNotFound` variant

**Rationale**: Python SkillViewer 在 skill 不存在时返回 `ToolResultState::ERROR` 的 ToolChunk 而非抛出异常。Rust 中，`Tool::call()` 返回 `Result<ToolExecOutput, ToolError>`。需要区分"工具调用失败"（应报错）和"skill 不存在"（应返回错误 ToolChunk 而非协议级错误）。

方案：SkillViewer 的 `call()` 在 skill 不存在时不返回 `Err(ToolError)`，而是返回 `Ok(ToolExecOutput::Complete(chunk))` 其中 `chunk.state = ToolResultState::Error`。这保持了与 Python 的协议兼容——错误在工具输出层，不在协议层。

如果确实需要 `ToolError` 变体用于其他场景（如 `_get_skills_method` 回调本身失败），可新增：
```rust
#[error("skill '{skill_name}' not found")]
SkillNotFound { skill_name: String },
```

**Impact**: `ToolError` enum 增加一个 variant（破坏性变更，但 Feature 006 刚引入，下游使用有限）。新 variant 需要在所有 `match ToolError` 位置处理。

## Decision 5: LocalSkillLoader 的放置位置

**Decision**: 放在 `agent_scope_tool` crate (而非新 crate 或 workspace crate)

**Rationale**: 
1. `LocalSkillLoader` 是与 ToolGroup/ToolKit 直接配合使用的——它的输出 (`Vec<Skill>`) 被 ToolGroup 消费
2. Python agentscope 的结构中 `LocalSkillLoader` 和 `SkillViewer` 虽在 `skill/` 和 `tool/` 不同目录，但在同一 package 中
3. 创建新 crate 会增加 Cargo.toml 复杂度、编译时间（新 crate 有独立的 dependency graph）、额外的版本管理
4. `agent_scope_workspace` 已定义 `Skill` 类型，`agent_scope_tool` 通过依赖方向 (`tool` → `workspace`) 复用该类型

**Alternatives considered**:
- **独立 `agent_scope_skill` crate**: 解耦更好但 3 个文件不值得一个 crate
- **合并到 `agent_scope_workspace`**: 违反关注点分离原则（workspace 管存储，tool 管使用）

**Impact**: `agent_scope_tool/Cargo.toml` 新增 `agent_scope_workspace` 依赖和 `sha2` 依赖。

## Decision 6: `SkillOrLoader` enum 设计

**Decision**: 使用 tagged enum 表示三种 skill 来源

```rust
pub enum SkillOrLoader {
    Skill(Skill),                        // 直接传入的 Skill 对象
    Loader(Box<dyn SkillLoader>),        // SkillLoader trait object
    Dir(String),                         // 目录路径 → 内部转为 LocalSkillLoader
}
```

**Rationale**: Python ToolGroup 接受 `str | Skill | SkillLoaderBase` 三种类型。Rust 版用 enum 在编译时保证类型安全。`Dir` 变体在内部自动转换为 `LocalSkillLoader` 以简化 API。

**Alternatives considered**:
- **使用 trait 统一**: `Skill` 和 `SkillLoader` 都实现 `Into<Vec<Skill>>` trait，但异步问题（`list_skills()` 是 async 的，无法用 `Into` trait 表达）
- **分开的 Vec 字段**: `skills: Vec<Skill>` + `loaders: Vec<Box<dyn SkillLoader>>` + `skill_dirs: Vec<String>` —— 3 个字段增加维护成本

## Decision 7: `ListSkillsCallback` 类型定义

**Decision**: 在 `agent_scope_tool` 中定义类型别名引用 workspace 的 `Skill`

```rust
use agent_scope_workspace::Skill;
pub type ListSkillsCallback = Box<dyn Fn(&[String]) -> HashMap<String, Skill> + Send + Sync>;
```

**Rationale**: 使类型签名可读且不重复。接受 `&[String]`（激活 group 名列表）以匹配 Python 的 `activated_groups` 参数。

**Impact**: SkillViewer 和 ToolKit 使用此类型别名，保持 API 一致。

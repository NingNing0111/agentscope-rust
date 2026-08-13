# Quickstart: Agent Workspace Built-in Tools 验证指南

**Feature**: 029-agent-workspace-tools | **Status**: Draft

本指南提供可运行的端到端验证场景，证明 feature 完整工作。实现细节见 `tasks.md`（/speckit-tasks 生成）；契约细节见 `contracts/`；数据模型见 `data-model.md`。

## 前置条件

- Rust workspace 可编译：`cargo build`
- 依赖 crate：`agent_scope_tool`、`agent_scope_workspace`、`agent_scope_agent`、`agent_scope_state`
- vendored Python 参考实现（`agentscope/src/`）用于 diff 测试对照

## 验证场景

### 场景 1: workspace 启用后自动注入内置工具（FR-001/FR-002, SC-001/SC-002）

**设置**: 创建两个 agent——一个配置 workspace，一个不配置。

```rust
// 启用 workspace 的 agent
let ws = Arc::new(LocalWorkspace::new(LocalWorkspaceConfig {
    workdir: "/tmp/ws-a".into(),
    ..Default::default()
}));
ws.initialize().await?;
let agent = ReActAgent::new(AgentConfig::builder()
    .name("ws-agent")
    .model(model)
    .workspace(Arc::clone(&ws))   // 029: 显式 workspace 配置
    .build()?)?;

// 未启用 workspace 的 agent
let plain = ReActAgent::new(AgentConfig::builder()
    .name("plain-agent")
    .model(model)
    .build()?)?;
```

**验证**:
- `ws-agent` 的 `toolkit.get_tool_schemas()` 包含 `Bash`/`Read`/`Edit`/`Write`/`Grep`/`Glob`/`ResetTools`/`Skill`（Windows 上还含 `PowerShell`）。
- `plain-agent` 的 schema **不包含** 上述任何文件/命令工具。
- 每个工具都有名称、描述、JSON Schema 输入契约（FR-022）。

**命令**: `cargo test -p agent_scope_tool --test builtin_injection_tests`

### 场景 2: 安全、可审计地修改 workspace 文件（FR-008/FR-012, SC-003）

**设置**: 准备一个已存在文件 `workdir/note.txt`，内容含唯一与重复字符串。

```rust
// 1. 未读取就 Edit → 拒绝
let result = call_tool("Edit", {"file_path": "/tmp/ws-a/note.txt",
    "old_string": "x", "new_string": "y"});
assert!(result.is_rejected_with("read_before_modify_required"));

// 2. Read 后 Edit 唯一字符串 → 精确替换
call_tool("Read", {"file_path": "/tmp/ws-a/note.txt"});
let result = call_tool("Edit", {"file_path": "/tmp/ws-a/note.txt",
    "old_string": "unique", "new_string": "replaced"});
assert!(result.is_success());

// 3. 重复字符串未 replace_all → 拒绝
let result = call_tool("Edit", {"file_path": "/tmp/ws-a/note.txt",
    "old_string": "dup", "new_string": "z"});
assert!(result.is_rejected_with("ambiguous_edit"));

// 4. 新文件 Write → 创建成功
call_tool("Write", {"file_path": "/tmp/ws-a/new.txt", "content": "hi"});
assert!(std::fs::exists("/tmp/ws-a/new.txt"));
```

**命令**: `cargo test -p agent_scope_tool --test builtin_edit_write_tests`

### 场景 3: 高效搜索与技能查看（FR-013~016, FR-020~021, SC-005/SC-007）

**设置**: workspace 内含若干 `.rs` 文件与一个技能。

```rust
// Glob 查找文件（限定 workspace 内）
let result = call_tool("Glob", {"pattern": "src/**/*.rs"});
assert!(result.files().all(|p| p.starts_with(workdir)));

// Grep 内容搜索（有界结果）
let result = call_tool("Grep", {"pattern": "Error", "output_mode": "count"});
assert!(result.is_success());

// Skill 查看
let result = call_tool("Skill", {"skill": "example-skill"});
assert!(result.content().contains("# Example"));
```

**命令**: `cargo test -p agent_scope_tool --test builtin_search_tests`

### 场景 4: ResetTools 工具组切换（FR-019）

**设置**: agent 有多个工具组（`basic` + 若干自定义组）。

```rust
// 激活特定组（final-state 语义：未列出组停用）
let result = call_tool("ResetTools", {"coding": true, "docs": false});
assert_eq!(activated_groups(), vec!["coding"]);  // basic 始终激活

// 越权请求被拒绝（FR-019）
let result = call_tool("ResetTools", {"admin": true});
assert!(result.is_rejected_with("permission_denied"));
```

**命令**: `cargo test -p agent_scope_tool --test builtin_reset_tools_tests`

### 场景 5: 命令工具超时（FR-004, SC-008）

**设置**: 运行超过配置超时的命令。

```rust
let result = call_tool("Bash", {"command": "sleep 10", "timeout": 200});
assert!(result.error_category() == Timeout);  // 超时终止，非静默
```

**命令**: `cargo test -p agent_scope_tool --test builtin_bash_tests`

## Trace 验证（FR-025, SC-006）

- 运行一个 agent 场景（搜索→读→编辑→创建→查看技能），工具调用事件在 trace 中以实际发生顺序出现。
- 每条工具调用含：工具名、参数概要（脱敏）、成功/错误类别、与 agent 事件相对顺序。

## 与 Python 参考实现的 diff 对照（宪法 Art.1/Art.3/Art.6）

- 对每个内置工具，用相同输入运行 vendored Python 实现（`agentscope/src/agentscope/tool/_builtin/`）与 Rust 实现，比较：
  - 工具名与输入 schema（结构层）
  - 成功/错误结果文本（`read_before_modify_required`、`SkillNotFoundError` 等）
  - 事件/trace 顺序（工具调用相对位置）
- 工具契约见 `contracts/`；兼容等级目标 L2。

## 预期结果汇总

| 场景 | 结果 |
|------|------|
| 1. workspace 注入 | 启用者含全部内置工具；未启用者无 |
| 2. read-before-modify | 未读拒绝、唯一替换、非唯一拒绝、新文件创建 |
| 3. 搜索与技能 | Glob/Grep 有界结果、Skill 命中/未找到清晰反馈 |
| 4. ResetTools | final-state 激活、越权拒绝 |
| 5. 超时 | 超时终止并报 Timeout 类别 |
| Trace | 工具调用顺序可观察 |

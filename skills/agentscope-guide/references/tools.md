# 参考:工具系统(`agent_scope_tool`)

> 详细 API 参考:`Tool` trait、`FunctionTool` 适配器、`ToolKit` 注册表、`ToolExecOutput`、`ToolError`、Skill 集成、内置 workspace 工具与工具调用生命周期。

## 1. `Tool` trait

核心扩展点,要求 `Send + Sync`:

| 方法 | 说明 |
|------|------|
| `name() -> &str` | 工具唯一名;注册表 key + schema 的 `function.name` |
| `description() -> &str` | 给模型看的说明 |
| `input_schema() -> JsonValue` | JSON Schema 参数定义 |
| `is_concurrency_safe() -> bool` | 默认 `true` |
| `is_read_only() -> bool` | 默认 `false` |
| `call(input: JsonValue) -> Result<ToolExecOutput, ToolError>` | 执行入口 |

契约:元数据稳定、错误类型化(所有失败走 `ToolError`)、panic 边界(`call()` 不应把 panic 传播给调用方)。

## 2. `ToolExecOutput`

```rust
pub enum ToolExecOutput {
    Complete(ToolResultBlock),
    Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>),
}
```

- `Complete`:一次性完成。
- `Stream`:流式输出;框架**不自动累积**,由调用方消费并依赖 `is_last` 判定结束。

## 3. `FunctionTool`:把 async 函数包装成工具

```rust
use agent_scope_tool::{FunctionTool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CalcInput { expression: String }

async fn calc(input: CalcInput) -> String {
    format!("calced: {}", input.expression)
}

let tool = FunctionTool::new("calculator", "Evaluate a math expression", calc);
```

内部行为:

1. 把 `JsonValue` 反序列化成 handler 输入类型 `T`(`T: JsonSchema + DeserializeOwned + Send + 'static`)。
2. `catch_unwind` 捕获 handler panic。
3. 返回值经 `IntoChunk` 转为 `ToolResultBlock`。

内置 `IntoChunk`:

- `String` → `ToolOutput::Text`,state=Success,`is_last: true`。
- `ToolResultBlock` → 透传,但强制 `is_last = true`。

**带状态的 handler**:如需访问共享状态,用 `Arc` 共享并在闭包里 clone。

```rust
use std::sync::Arc;

let state = Arc::new(SharedState::default());
let tool_state = Arc::clone(&state);
toolkit.register(FunctionTool::new(
    "Read",
    "Read a UTF-8 text file.",
    move |input: ReadInput| {
        let state = Arc::clone(&tool_state);
        async move { read_tool(&state, input).into_block("Read") }
    },
));
```

## 4. `ToolKit`:注册与调度

| 能力 | 说明 |
|------|------|
| `new()` | 创建注册表,并**自动注册 `SkillViewer`** 到默认 `basic` 组 |
| `register(tool)` | 注册工具;同名覆盖 |
| `try_register(tool)` | 注册工具;同名返回 `ToolError::DuplicateRegistration`,适合 fail-closed 构造流程 |
| `remove(name)` / `clear()` | 删除 |
| `contains(name)` / `len()` / `is_empty()` | 查询 |
| `register_in_group(group, tool)` / `try_register_in_group(group, tool)` | 将工具放入非 `basic` 组,供 `ResetTools` 动态启停 |
| `set_workspace_session(session)` | 绑定 workspace 工具共享 session,让 `ResetTools` 变更立即影响 schema 可见性 |
| `get_tool_schemas()` | 导出 OpenAI-compatible function schema 数组 |
| `call_tool(&ToolCallBlock)` | 解析 `input` JSON,按 `name` 分发 |
| `add_skill_dir()` / `add_skill()` / `add_skill_loader()` | 注册 Skill 来源 |
| `list_skills()` / `get_skill_instructions()` | 枚举 Skill、生成 `<agent-skills>` prompt 片段 |

`call_tool()` 调度流程:

1. 按 `tool_call.name` 查找工具,不存在 → `ToolError::NotFound`。
2. 解析 `tool_call.input` 为 `JsonValue`,失败 → `InvalidInput`。
3. 调用 `tool.call(input).await`。

## 5. 内置 workspace 工具

`ReActAgent` 绑定 `LocalWorkspace` 后会把 workspace 工具注入到 `ToolKit`。注入是 fail-closed 的:backend 不可用或工具名冲突会使 Agent 构造失败,避免暴露半套工具。

| 工具 | 名称 | 说明 |
|------|------|------|
| Bash | `Bash` / `bash` | 在 workspace backend 中执行 shell 命令 |
| PowerShell | `PowerShell` / `powershell` | Windows 下执行 PowerShell 命令;非 Windows 默认不注入 |
| Read | `Read` / `read` | 读取 UTF-8 文本文件,并记录 read-state |
| Write | `Write` / `write` | 写入文件;复用 read-state 防止未读先改 |
| Edit | `Edit` / `edit` | 文本替换编辑;复用 read-state 防止未读先改 |
| Glob | `Glob` | 按 glob pattern 找文件,按 mtime 新到旧排序 |
| Grep | `Grep` / `grep` | 在 workspace 内搜索文本或正则 |
| Find | `find` | pi-compatible 文件发现工具,返回 workspace-relative 路径 |
| Ls | `ls` | pi-compatible 目录列表工具 |
| ResetTools | `ResetTools` | 启停非 `basic` 工具组,更新当前 session 的 active groups |
| Skill | `Skill` | session-aware skill viewer,受 `ResetTools` 激活组过滤 |

这些工具共享:

- `BuiltInToolContext`:持有 `Arc<dyn WorkspaceBackend>`、workspace 根路径和 `WorkspaceToolSession`。
- `WorkspaceToolSession`:保存 workspace id、已读路径、授权工具组和当前激活工具组。
- `WorkspaceBackend`:所有文件和命令 I/O 都经过 backend,不会绕过 workspace containment。

常见接入方式是让 `ReActAgent` 自动注入。手工注册时要复用同一个 `WorkspaceToolSession`,否则 `Read`/`Edit`/`Write` 的 read-state 和 `ResetTools` 的激活状态会互相看不见。

```rust
use std::sync::{Arc, RwLock};

use agent_scope_tool::builtin::{
    BuiltInToolContext, ReadTool, ResetToolsTool, WorkspaceToolSession, WriteTool,
};
use agent_scope_tool::ToolKit;
use agent_scope_workspace::backend::{LocalBackend, WorkspaceBackend};

let backend: Arc<dyn WorkspaceBackend> = Arc::new(LocalBackend::new());
let session = Arc::new(RwLock::new(WorkspaceToolSession::with_authorized_groups(
    "ws-1",
    ["coding", "docs"],
)));
let ctx = BuiltInToolContext::new(backend, "/tmp/ws", Arc::clone(&session));

let mut toolkit = ToolKit::new();
toolkit.set_workspace_session(Arc::clone(&session));
toolkit.register(ReadTool::new(ctx.clone()));
toolkit.register(WriteTool::new(ctx.clone()));
toolkit.register(ResetToolsTool::new(ctx));
```

## 6. 工具调用生命周期

```text
ToolCallBlock(Pending)
→ ToolCallStart / ToolCallDelta* / ToolCallEnd
→ ToolResultStart / ToolResultTextDelta* / ToolResultDataDelta*
→ ToolResultEnd(Success | Error | Interrupted | Denied)
```

## 7. 手工调度 `ToolCallBlock`(绕过 Agent)

```rust
use agent_scope_message::{ContentBlock, ToolCallBlock};

let call = ToolCallBlock::new(
    "tc-1".into(),
    "calculator".into(),
    r#"{"expression":"2+2"}"#.into(),
);
let output = toolkit.call_tool(&ContentBlock::ToolCall(call)).await?;
```

注意:`call_tool()` 会先解析 `call.input` 的 JSON 字符串,必须是合法 JSON。

## 8. 返回 `ToolResultBlock`

handler 可直接返回 `ToolResultBlock` 以便自定义状态/元数据:

```rust
use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_utils::id::generate_uuid;

let block = ToolResultBlock {
    id: generate_uuid(),
    name: "calculator".into(),
    output: ToolOutput::Text("42".into()),
    state: ToolResultState::Success,
    is_last: true,
    metadata: std::collections::HashMap::new(),
    created_at: chrono::Utc::now().to_rfc3339(),
    finished_at: None,
};
```

也可以先用消息层构造函数,再显式设置终态字段:

```rust
use agent_scope_message::{ToolOutput, ToolResultBlock, ToolResultState};
use agent_scope_utils::id::generate_uuid;

let mut block = ToolResultBlock::new(
    generate_uuid(),
    "calculator".into(),
    ToolOutput::Text("42".into()),
);
block.state = ToolResultState::Success;
block.is_last = true;
block.finished_at = Some(chrono::Utc::now().to_rfc3339());
```

## 9. Skill 集成

- 直接 Skill 对象:`add_skill(skill)`
- 本地目录:`add_skill_dir(path)`
- 自定义 loader:`add_skill_loader(loader)`
- `ToolKit::new()` 默认带 `SkillViewer`,把可用 Skill 暴露给 Agent。

`SkillViewer::new(callback)` 注册为工具后,Agent 通过 `Skill` 工具按需读取技能说明。

**实时扫描**:用 `LocalSkillLoader` 作为回调,每次调用实时重扫目录,运行期新增的 skill 立即生效,无需重启:

```rust
use agent_scope_tool::{LocalSkillLoader, SkillViewer};

toolkit.remove("Skill"); // 移除 ToolKit::new() 默认注册的基于快照的 SkillViewer
toolkit.register(SkillViewer::new(Box::new(move |_groups| {
    LocalSkillLoader::new(skills_dir, true)
        .list_skills_blocking()
        .into_iter()
        .map(|skill| (skill.name.clone(), skill))
        .collect()
})));
```

## 10. 错误

| 错误 | 触发条件 |
|------|----------|
| `ToolError::NotFound { tool_name }` | 调用未注册的工具 |
| `ToolError::InvalidInput { tool_name, reason }` | 输入 JSON 非法或无法反序列化为参数类型 |
| `ToolError::Execution { tool_name, reason }` | handler 运行时错误或 panic |
| `ToolError::Interrupted { tool_name }` | 工具被中断 |
| `ToolError::DuplicateRegistration { tool_name }` | `try_register*` 注册同名工具 |
| `ToolError::SkillNotFound { skill_name }` | Skill 名不存在 |

内置 workspace 工具还会把错误归类到稳定机器码前缀,见 `ToolErrorCategory::{ValidationFailure, PermissionDenied, UnsupportedCapability, Timeout, ExecutionFailure, InternalFailure}`。

## 11. 常见问题

- **工具输入为什么先是字符串再 parse JSON**:与消息层 `ToolCallBlock.input` 的稳定协议保持一致,Foundation 层不提前解析参数。
- **handler panic 会怎样**:`FunctionTool` 内部 `catch_unwind`,返回 `ToolError::Execution { reason: "handler panicked" }`。
- **schema 从哪来**:`schemars` 从参数类型自动推导;用 `new_with_schema` 可手写 escape hatch。
- **LLM 把数字/布尔传成字符串怎么办**:`FunctionTool` 与 `deserialize_lenient` 内置容错——严格反序列化失败后才对字符串化数字/布尔做 coerce(如 `"30"` → `30`、`"true"` → `true`),严格输入永不改写。因此无需手写自定义 `Deserialize` 来容忍这类 LLM 输出。

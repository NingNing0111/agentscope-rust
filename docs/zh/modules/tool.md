# 工具系统 / Tool

> 一句话定位：把 Rust 异步函数包装成可被 Agent 调用的结构化工具——定义 `Tool` trait、`FunctionTool` 适配器、`ToolKit` 注册表，以及 ToolCall/ToolResult 的完整生命周期。

## 1. 模块概述 (Overview)

本模块对应 `agent_scope_tool` crate，位于 Agent 与模型之间：模型生成 `ToolCallBlock`，工具系统负责校验输入、调度已注册工具、返回 `ToolResultBlock`，并向上层暴露 OpenAI-compatible 的函数 schema。

**适用场景**：

- 给 Agent 注册自定义工具
- 把已有 async Rust 函数包装为 `FunctionTool`
- 导出工具 schema 供模型选择调用
- 处理一次性或流式工具结果
- 管理 Skill 目录、Skill 对象与 SkillViewer 工具

**前置阅读**：
- [消息与基础类型](./message-types.md) — `ToolCallBlock` / `ToolResultBlock` 数据结构
- [事件与流式](./event-streaming.md) — `ToolCall*` / `ToolResult*` 事件序列
- [Agent 系统](./agent.md) — Agent 如何消费 `ToolKit`

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `Tool` trait

`Tool` 是核心扩展点，要求 `Send + Sync`，便于以 `Arc<dyn Tool>` 或 boxed trait object 共享：

| 方法 | 说明 |
|------|------|
| `name() -> &str` | 工具唯一名；既是注册表 key，也是 schema 中的 `function.name` |
| `description() -> &str` | 给模型看的说明文本 |
| `input_schema() -> JsonValue` | JSON Schema 参数定义 |
| `is_concurrency_safe() -> bool` | 是否可并发调用，默认 `true` |
| `is_read_only() -> bool` | 是否无副作用，默认 `false` |
| `call(input: JsonValue) -> Result<ToolExecOutput, ToolError>` | 执行入口 |

契约保证：

- 元数据稳定：`name` / `description` / `input_schema` 应返回稳定值
- 错误类型化：所有失败通过 `ToolError`
- panic 边界：`call()` 不应把 panic 传播给调用方

### 2.2 `ToolExecOutput`

工具执行结果统一为两种形式：

```rust
pub enum ToolExecOutput {
    Complete(ToolResultBlock),
    Stream(Pin<Box<dyn Stream<Item = Result<ToolResultBlock, ToolError>> + Send>>),
}
```

- `Complete`：一次性完成，典型同步/短操作
- `Stream`：流式输出；框架**不会自动累积**，由调用方消费并依赖 `is_last` 判定结束

### 2.3 `ToolError`

工具层的类型化错误模型：

| 变体 | 触发条件 |
|------|----------|
| `NotFound { tool_name }` | `ToolKit` 中没有该工具 |
| `InvalidInput { tool_name, reason }` | 输入 JSON 无法反序列化为工具参数类型，或原始输入字符串不是合法 JSON |
| `Execution { tool_name, reason }` | 工具运行失败，含 handler panic |
| `Interrupted { tool_name }` | 工具执行被中断 |
| `SkillNotFound { skill_name }` | 请求的 Skill 不存在 |

### 2.4 `FunctionTool`

`FunctionTool` 用来把普通 async 函数适配成 `Tool`：

- `FunctionTool::new(name, description, handler)`：从参数类型 `T: JsonSchema + DeserializeOwned` 自动推导 JSON Schema
- `FunctionTool::new_with_schema(name, description, schema, handler)`：手工提供 schema 的 escape hatch

其内部行为：

1. 先把 `JsonValue` 反序列化成 handler 的输入类型 `T`
2. 捕获 handler panic（`catch_unwind`）
3. 将返回值通过 `IntoChunk` 转为 `ToolResultBlock`

内置 `IntoChunk`：

- `String` → `ToolOutput::Text`，`state: Success`，`is_last: true`
- `ToolResultBlock` → 透传，但强制 `is_last = true`

### 2.5 `ToolKit`

`ToolKit` 是工具注册与调度中心：

| 能力 | 说明 |
|------|------|
| `new()` | 创建注册表，并自动注册 `SkillViewer` 工具到默认 `basic` 组 |
| `register(tool)` | 注册工具；同名覆盖（与 Python AgentScope 一致） |
| `remove(name)` / `clear()` | 删除单个或全部工具 |
| `contains(name)` / `len()` / `is_empty()` | 查询 |
| `get_tool_schemas()` | 导出 OpenAI-compatible function schema 数组 |
| `call_tool(&ToolCallBlock)` | 解析 `input` JSON，按 `name` 分发到对应工具 |
| `add_skill_dir()` / `add_skill()` / `add_skill_loader()` | 注册 Skill 来源 |
| `list_skills()` / `get_skill_instructions()` | 枚举 Skill、生成 `<agent-skills>` prompt 片段 |

`ToolKit::call_tool()` 的调度流程：

1. 用 `tool_call.name` 查找工具，不存在则 `NotFound`
2. 解析 `tool_call.input` 为 `JsonValue`，失败则 `InvalidInput`
3. 调用 `tool.call(input).await`

### 2.6 ToolCall / ToolResult 生命周期

配合消息与事件层，一个工具调用的可观察生命周期是：

```text
ToolCallBlock(Pending)
→ ToolCallStart / ToolCallDelta* / ToolCallEnd
→ ToolResultStart / ToolResultTextDelta* / ToolResultDataDelta*
→ ToolResultEnd(Success | Error | Interrupted | Denied)
```

在基础消息结构中：

- `ToolCallBlock.input` 是**原始 JSON 字符串**，Tool 层才解析
- `ToolCallState`：`pending` → `asking` → `allowed` → `submitted` → `finished`
- `ToolResultState`：`running` → `success` / `error` / `interrupted` / `denied`
- `ToolResultBlock.is_last` 标记流式工具结果的最后一块

## 3. 快速示例 (Quick Example)

仓库中的 calculator 工具是最标准的 `FunctionTool` 用法：

<!-- source: examples/common.rs:L311-L316 -->
```rust
pub fn create_calculator_tool() -> FunctionTool {
    FunctionTool::new(
        "calculator",
        "Evaluate a mathematical expression. Supports +, -, *, /, ^ (power), (), and constants pi/e. Example: \"2 + 3 * (4 - 1) ^ 2\"",
        calc_handler,
    )
}
```

这个工具随后可放入 `ToolKit`，再注入 `ReActAgent`。完整调用链见 `examples/common.rs` 的 `build_agent()` 与 `examples/streaming_tool_test.rs` 的工具调用测试。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 从 typed input 自动生成 schema

如果参数类型实现了 `Deserialize + JsonSchema`，可直接包装：

```rust
#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SearchInput {
    query: String,
}

async fn search(input: SearchInput) -> String {
    format!("Results for: {}", input.query)
}

let tool = FunctionTool::new("search", "Search the web", search);
```

这是推荐路径：schema 与 Rust 类型保持同源，减少手写 JSON Schema 漂移。

### 4.2 注册到 `ToolKit`

```rust
let mut tk = ToolKit::new();
tk.register(create_calculator_tool());
let schemas = tk.get_tool_schemas();
```

`schemas` 输出格式为 OpenAI-compatible：

```json
{
  "type": "function",
  "function": {
    "name": "calculator",
    "description": "...",
    "parameters": { "type": "object", "properties": { ... } }
  }
}
```

### 4.3 让 Agent 真正调用工具

流式工具调用测试展示了 Agent 发起实际工具调用的最小路径：

<!-- source: examples/streaming_tool_test.rs:L229-L241 -->
```rust
async fn run_single_tool_call(
    agent: &impl Agent,
) -> Result<EventTrace, Box<dyn std::error::Error>> {
    let msg = user_msg("user", "Calculate 3.14 * 2.718 using the calculator tool.")
        .map_err(|e| format!("{e:?}"))?;
    let mut stream = agent.reply_stream(Some(vec![msg])).await?;

    let mut trace = EventTrace::new();
    while let Some(event) = stream.next().await {
        trace.record(&event);
    }
```

这个例子说明：读者只需注册工具并向 Agent 发送自然语言请求，后续 ToolCall/ToolResult 生命周期由 Agent + ToolKit 驱动。

### 4.4 手工调度 `ToolCallBlock`

在不通过 Agent 的场景中，可以直接调用：

```rust
let call = ToolCallBlock::new(
    "tc-1".into(),
    "calculator".into(),
    r#"{"expression":"2+2"}"#.into(),
);
let output = toolkit.call_tool(&call).await?;
```

注意：`call_tool()` 会先解析 `call.input` 的 JSON 字符串，因此字符串必须是合法 JSON。

### 4.5 Skill 集成

`ToolKit` 除普通工具外还可注册：

- 直接 Skill 对象：`add_skill(skill)`
- 本地目录：`add_skill_dir(path)`
- 自定义 loader：`add_skill_loader(loader)`

`ToolKit::new()` 默认自动带有 `SkillViewer`，便于把可用 Skill 暴露给 Agent。

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误类型 | 触发条件 |
|----------|----------|
| `ToolError::NotFound` | 调用了未注册的工具 |
| `ToolError::InvalidInput` | `ToolCallBlock.input` 不是合法 JSON，或 JSON 不能反序列化为目标参数类型 |
| `ToolError::Execution` | handler 运行时错误或 panic（panic 会被捕获并转成该错误） |
| `ToolError::Interrupted` | 工具被中断 |
| `ToolError::SkillNotFound` | Skill 名不存在 |

**不支持的能力**：

- 无固定全局 `UnsupportedFeature` 列表；工具是否支持流式、并发安全、只读能力由具体实现决定
- 若工具实现不支持某能力，应通过 `ToolError` 或上层协议显式失败，而不是静默降级

**常见问题**：

- *为什么工具输入要先是字符串再 parse JSON*：这是为了与消息层 `ToolCallBlock.input` 的稳定协议保持一致，Foundation 层不提前解析参数。
- *handler panic 会怎样*：`FunctionTool` 内部 `catch_unwind`，最终返回 `ToolError::Execution { reason: "handler panicked" }`。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L1**（ToolCall/ToolResult 数据结构与 schema 导出格式）；**L2**（注册、调度、typed error、Skill 集成等行为语义）
- **权威来源**: `specs/001-compatibility-baseline/capability-matrix.json`
- **已知偏差**:
  - 矩阵 `status` 字段当前全部为 `NOT_ANALYZED`；本页等级以 `tool` 类目 `target_level` + `specs/006-tool-system`、`specs/013-skill-tool-spec` + 代码实际状态交叉核实为准。
  - `ToolKit::new()` 自动注册 `SkillViewer` 到默认组——这是 Rust 侧的人体工学增强，空注册表并非真正完全空。
  - `FunctionTool` 通过 `schemars` 自动推导 schema，是 Rust 静态类型体系下的惯用做法；与 Python 运行时 schema 构造路径不同。
  - `call_tool()` 只在 Tool 层解析 `ToolCallBlock.input`，保持消息层原始字符串协议不变。
- **不支持的能力**: 无统一清单；具体能力由具体 Tool 明确暴露或拒绝。

## 7. 相关模块 (See Also)

- [Agent 系统 / agent](./agent.md) — ToolKit 的主要消费方
- [事件与流式 / event-streaming](./event-streaming.md) — ToolCall/ToolResult 事件生命周期
- [消息与基础类型 / message-types](./message-types.md) — `ToolCallBlock` / `ToolResultBlock` 结构
- [Skill / skill](./skill.md) — ToolKit 的 Skill 集成部分

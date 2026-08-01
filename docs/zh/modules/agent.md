# Agent 系统 / Agent

> 一句话定位：`agent_scope_agent` 是模型、消息、工具、事件、记忆与会话状态之间的编排层——用统一的 `Agent` trait 暴露 `reply()` / `reply_stream()` / `observe()`，并以 `ReActAgent` 实现 reasoning → acting 的多轮工具调用循环。

## 1. 模块概述 (Overview)

本模块对应 `agent_scope_agent` crate。它不直接实现具体模型 Provider，也不直接定义工具输入输出格式，而是把下列模块组合起来：

- [模型抽象](./model.md)：通过 `Arc<dyn ChatModel>` 发起模型调用
- [消息与基础类型](./message-types.md)：用 `Msg` / `ContentBlock` 表示上下文和回复
- [工具系统](./tool.md)：通过 `ToolKit` 注册并执行工具
- [事件与流式](./event-streaming.md)：通过 `AgentEvent` 暴露可观察运行轨迹
- 记忆：通过 `MemoryMiddleware` 注入长期记忆
- 会话管理：通过 `AgentState` 保存上下文、session id 与 reply context

**适用场景**：构建一个可对话的 Agent；给 Agent 注册工具；消费实时事件流；在回复前后挂接 middleware；为工具执行配置权限；启用上下文压缩与记忆增强。

**前置阅读**：建议先阅读 [模型抽象](./model.md)、[工具系统](./tool.md) 与 [事件与流式](./event-streaming.md)。如果只想先跑起来，可以直接看 [快速上手](../getting-started.md)。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `Agent` trait

`Agent` 是所有 Agent 类型的共同接口，目前主要实现是 `ReActAgent`：

| 方法 | 说明 |
|------|------|
| `reply(input)` | 非流式调用入口；返回最终 assistant `Msg` |
| `reply_stream(input)` | 流式调用入口；返回 `Stream<Item = AgentEvent>` |
| `observe(input)` | 只把消息追加到上下文，不触发模型回复 |
| `name()` | 返回 Agent 配置名 |
| `state()` | trait 层状态访问接口；`ReActAgent` 请使用 `try_state()` |

`reply(None)` 的语义是“基于已有上下文继续回复”。如果上下文为空，会返回 `AgentError::NoContentToReply`。

### 2.2 `ReActAgent`

`ReActAgent` 是当前主要 Agent 类型，内部执行 reasoning → acting 循环：

```text
用户输入 / 已有上下文
→ middleware.pre_reply
→ middleware.on_system_prompt
→ loop(max_iters):
   → middleware.pre_reasoning
   → model.call(messages, tool_schemas, tool_choice)
   → middleware.post_reasoning
   → 如果模型返回文本：累积为最终回复
   → 如果模型返回 ToolCallBlock：权限检查 → middleware.pre_acting → ToolKit.call_tool → middleware.post_acting
   → 工具结果追加回上下文，进入下一轮 reasoning
→ middleware.post_reply
→ 返回最终 Msg 或事件流收尾
```

几个重要行为：

- 模型可以返回非流式 `Complete(ChatResponse)`，也可以返回流式 `Stream(...)`；`ReActAgent` 会在非流式 `reply()` 路径中用 `StreamAccumulator` 累积完整响应。
- 单个 `ReActAgent` 同一时间只允许一个 `reply()` 或 `reply_stream()` 活跃；并发启动第二个回复会返回 `AgentError::AlreadyStreaming`。
- `interrupt()` 可中断进行中的回复；框架会发出 `UserInterrupt`，并以 `ReplyEnd(finished_reason: interrupted)` 收尾。
- `try_state()` 提供 lock-aware 的状态读取；不要直接调用 `ReActAgent` 的 `state()`，它会 panic。

### 2.3 `AgentConfig`

`AgentConfig` 是构造时配置，使用 builder 创建：

| 字段 / builder | 说明 |
|----------------|------|
| `name(...)` | Agent 名称，必填；用于消息和事件中的 `name` |
| `system_prompt(...)` | 系统提示词；可为空 |
| `model(...)` | `Arc<dyn ChatModel>`，必填 |
| `toolkit(...)` | 可选工具注册表 |
| `permission_context(...)` / `permission_mode(...)` | 工具执行权限上下文 |
| `with_stream_channel_capacity(...)` | 流式事件通道容量；`None` 表示无界，`Some(n)` 必须 `n > 0` |

最小构造需要 `name` 和 `model`。如果缺少任一必填项，`build()` 会返回 `AgentError::InvalidConfig`。

### 2.4 `ReActConfig`

控制 ReAct 循环行为：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `max_iters` | `20` | 单次回复最多 reasoning/acting 迭代数，必须大于 0 |
| `stop_on_reject` | `false` | 工具权限拒绝时是否停止 |
| `interruption_message` | `"The execution was interrupted."` | 中断时返回的 assistant 文本 |
| `structured_output_grace_iters` | `3` | 结构化输出解析失败时的额外容错迭代数 |

### 2.5 `ContextConfig`

控制上下文窗口压缩：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `enable` | `false` | 是否启用压缩 |
| `trigger_ratio` | `0.8` | token 数超过 `context_size * trigger_ratio` 时触发 |
| `reserve_ratio` | `0.1` | 为模型回复保留的上下文比例 |
| `compression_prompt` | `"<STD_CP_PROMPT>"` | 压缩模型调用的系统提示 |
| `tool_result_limit` | `4096` | 工具结果内容截断限制 |

启用后，Agent 每轮模型调用前会估算当前上下文 token 数；超过阈值时调用压缩逻辑修剪上下文。

### 2.6 `Middleware`

`Middleware` 是 Agent 扩展点，所有 hook 默认 no-op，按注册顺序 FIFO 调用：

| Hook | 调用时机 | 常见用途 |
|------|----------|----------|
| `pre_reply` | 回复开始前 | 修改输入、启动异步检索、捕获模型引用 |
| `post_reply` | 回复结束后 | 记录审计日志、持久化状态 |
| `on_system_prompt` | 首次模型调用前 | 追加记忆、策略或动态说明 |
| `pre_reasoning` | 每轮模型调用前 | 修改上下文消息或工具 schema |
| `post_reasoning` | 模型返回后 | 记录模型响应、统计用量 |
| `pre_acting` | 工具执行前 | 修改或拒绝工具调用 |
| `post_acting` | 工具执行后 | 记录工具结果、触发副作用 |
| `pre_observe` | `observe()` 被调用时 | 规范化被观察消息 |
| `pre_print` | 输出渲染前 | 修改展示内容 |

内置的 `MemoryMiddleware` 会在 `on_system_prompt` 中追加 `MEMORY.md` 索引，并可在 `pre_reply` / `pre_reasoning` 中异步检索相关记忆，将结果以 `HintBlock` 注入用户消息。

### 2.7 权限系统

工具执行前，Agent 会使用 `PermissionEngine` 根据 `PermissionContext` 做检查。当前权限模式包括：

| 模式 | 默认行为 |
|------|----------|
| `Default` | 无匹配规则时允许 |
| `AcceptEdits` | 无匹配规则时允许 |
| `Explore` | 只读规划模式；无 allow 规则时拒绝未分类工具调用 |
| `Bypass` | 无匹配规则时允许 |
| `DontAsk` | ask 决策转换为 deny；无匹配规则时允许 |

规则优先级是：`deny` → `ask` → `allow` → 模式默认值。规则支持精确匹配、`*` 全匹配，以及 `prefix*` 前缀匹配；可用 `rule_content` 对序列化后的工具输入做子串匹配。

## 3. 快速示例 (Quick Example)

下面是仓库示例中的标准构造方式：创建模型，按需注册工具，然后构造 `ReActAgent`。

<!-- source: examples/common.rs:L327-L356 -->
```rust
use agent_scope_agent::{AgentConfig, ContextConfig, ReActAgent, ReActConfig};
use agent_scope_tool::ToolKit;

let system_prompt = concat!(
    "You are a helpful AI assistant. ",
    "When the user asks a mathematical question, use the 'calculator' tool."
);

let mut toolkit = ToolKit::new();
toolkit.register(create_calculator_tool());

let config = AgentConfig::builder()
    .name("assistant")
    .system_prompt(system_prompt)
    .model(model)
    .toolkit(toolkit)
    .build()?;

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![],
)?;
```

发送一条用户消息并等待最终回复：

```rust
use agent_scope_agent::Agent;
use agent_scope_message::factory::user_msg;

let reply = agent
    .reply(Some(vec![user_msg("user", "帮我计算 15 * 27 + 3")?]))
    .await?;

println!("{}", reply.get_text_content("\n").unwrap_or_default());
```

## 4. 关键用法模式 (Usage Patterns)

### 4.1 运行仓库内置终端 Agent

`examples/chat.rs` 是最完整的 Agent 使用示例：

```bash
cargo run --example chat -- -k sk-your-real-key
```

默认行为：

- 使用 DashScope `qwen-plus`
- 默认开启 thinking 模式
- 注册 calculator 工具
- 用 `reply_stream()` 消费并打印全部 `AgentEvent`
- 输入 `exit` / `quit` 退出，`Ctrl+C` 中断当前回复

如果不想显示 thinking：

```bash
cargo run --example chat -- -k sk-your-real-key --no-thinking
```

### 4.2 非流式回复：`reply()`

`reply()` 适合命令行任务、测试和后端接口：调用者只关心最终消息，不需要逐 token 渲染。

```rust
let input = vec![user_msg("user", "介绍一下 AgentScope Rust")?];
let output = agent.reply(Some(input)).await?;

for block in &output.content {
    println!("{block:?}");
}
```

注意：即使底层模型默认流式，`reply()` 也会在内部累积为完整 `ChatResponse`，然后返回最终 `Msg`。

### 4.3 流式回复：`reply_stream()`

`reply_stream()` 适合终端 UI、WebSocket/SSE、trace 记录器等需要实时反馈的场景。

```rust
use futures::StreamExt;

let mut stream = agent
    .reply_stream(Some(vec![user_msg("user", "一步步计算 (2+3)*4")?]))
    .await?;

while let Some(event) = stream.next().await {
    match event {
        AgentEvent::TextBlockDelta(e) => print!("{}", e.delta),
        AgentEvent::ToolCallStart(e) => eprintln!("calling tool: {}", e.tool_call_name),
        AgentEvent::ToolResultEnd(e) => eprintln!("tool result: {:?}", e.state),
        AgentEvent::ReplyEnd(e) => eprintln!("finished: {:?}", e.finished_reason),
        _ => {}
    }
}
```

消费方应至少处理：

- `ReplyStart` / `ReplyEnd`：一次回复的边界
- `ModelCallStart` / `ModelCallEnd`：模型调用边界与 token 用量
- `TextBlock*` / `ThinkingBlock*`：文本和推理内容
- `ToolCall*` / `ToolResult*`：工具调用与工具结果
- `UserInterrupt` / `ExceedMaxIters`：控制流异常

更完整的事件渲染参考 `examples/chat.rs` 的 `render_event()`。

### 4.4 观察消息：`observe()`

`observe()` 只把消息追加到 Agent 上下文，不触发模型调用。适合把系统外部事件、用户历史消息或其他 Agent 的输出注入当前 Agent。

```rust
agent
    .observe(Some(vec![user_msg("user", "我偏好简洁回答")?]))
    .await?;

// 稍后基于已有上下文继续回复
let reply = agent.reply(None).await?;
```

如果调用 `reply(None)` 前没有任何上下文，会得到 `AgentError::NoContentToReply`。

### 4.5 注册工具并让模型自动调用

Agent 本身不实现工具；它通过 `ToolKit` 暴露工具 schema，并在模型返回 `ToolCallBlock` 后执行工具。

```rust
let mut toolkit = ToolKit::new();
toolkit.register(create_calculator_tool());

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .toolkit(toolkit)
    .build()?;
```

工具调用生命周期通常是：

```text
模型返回 ToolCallBlock
→ ToolCallStart / ToolCallDelta* / ToolCallEnd
→ PermissionEngine 检查
→ ToolKit.call_tool(...)
→ ToolResultStart / ToolResultTextDelta* / ToolResultEnd
→ 工具结果写回上下文，进入下一轮模型调用
```

工具 schema、`FunctionTool` 和 `ToolKit` 细节见 [工具系统](./tool.md)。

### 4.6 使用 Middleware 增强 Agent

创建 Agent 时最后一个参数是 `Vec<Arc<dyn Middleware>>`。例如注入记忆：

<!-- source: examples/common.rs:L363-L406 -->
```rust
use std::sync::Arc;
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, MemoryConfig};

let memory_config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
let memory = Arc::new(FileMemory::new(workdir, memory_config.clone(), None));
let middleware = Arc::new(MemoryMiddleware::new(memory, memory_config));

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![middleware],
)?;
```

自定义 middleware 时只需要实现关心的 hook：

```rust
use agent_scope_agent::{AgentError, Middleware};
use agent_scope_message::Msg;
use agent_scope_model::ChatModel;
use std::sync::Arc;

struct AuditMiddleware;

#[async_trait::async_trait]
impl Middleware for AuditMiddleware {
    async fn pre_reply(
        &self,
        agent_name: &str,
        input: &mut Option<Vec<Msg>>,
        _model: &Arc<dyn ChatModel>,
    ) -> Result<(), AgentError> {
        tracing::info!(agent = agent_name, has_input = input.is_some(), "reply started");
        Ok(())
    }
}
```

### 4.7 配置工具权限

默认模式下，无匹配规则时工具调用会被允许。如果你在只读探索场景中运行 Agent，可切到 `Explore` 并显式 allow 安全工具：

```rust
use agent_scope_agent::{PermissionContext, PermissionMode, PermissionRule};

let mut permission = PermissionContext::new(PermissionMode::Explore);
permission.add_rule(PermissionRule::allow("calculator"));
permission.add_rule(PermissionRule::deny("shell*"));

let config = AgentConfig::builder()
    .name("assistant")
    .model(model)
    .toolkit(toolkit)
    .permission_context(permission)
    .build()?;
```

`deny` 规则优先于 `allow`。需要用户确认的 `ask` 规则在 `DontAsk` 模式下会自动转为拒绝。

### 4.8 控制上下文压缩

默认不启用上下文压缩。长对话或大工具结果场景可开启：

```rust
let context_config = ContextConfig {
    enable: true,
    trigger_ratio: 0.8,
    reserve_ratio: 0.1,
    tool_result_limit: 4096,
    ..ContextConfig::default()
};

let agent = ReActAgent::new(config, ReActConfig::default(), context_config, vec![])?;
```

压缩发生在每轮模型调用前；是否触发由 `model.count_tokens(...)` 与 `model.context_size()` 决定。

### 4.9 中断正在运行的回复

`ReActAgent::interrupt()` 可从任意线程调用。常见用法是 UI 收到 Ctrl+C 或用户点击停止按钮时调用：

```rust
agent.interrupt();
```

中断后：

- 活跃模型调用或流消费会通过 `CancellationToken` 停止
- 事件流收到 `UserInterrupt`
- `ReplyEnd.finished_reason` 为 `Interrupted`
- 下一次 `reply()` / `reply_stream()` 会自动使用新的 cancellation token，可正常继续

## 5. 错误处理 (Errors)

`AgentError` 是 Agent 层统一错误类型：

| 错误 | 常见原因 | 处理建议 |
|------|----------|----------|
| `InvalidConfig` | 缺少 name/model，或配置值非法 | 构造阶段尽早 fail-fast |
| `NoContentToReply` | `reply(None)` 且上下文为空 | 先传入用户消息或调用 `observe()` |
| `AlreadyStreaming` | 已有活跃 `reply()` / `reply_stream()` | 消费完或 drop 当前 stream 后再启动下一次 |
| `ModelError` | Provider 调用失败 | 读取 source，按认证、限流、网络等分类处理 |
| `ToolError` | 工具不存在、输入非法或执行失败 | 检查工具注册和模型生成的 JSON 输入 |
| `PermissionDenied` | 权限规则拒绝工具执行 | 调整 `PermissionContext` 或提示用户授权 |
| `MaxItersExceeded` | ReAct 循环超过 `max_iters` | 增大上限、改进系统提示或限制工具循环 |
| `CancellationError` | 回复被取消 | 通常作为正常控制流处理 |
| `ContextCompressionFailed` | 压缩模型调用失败 | 关闭压缩或检查模型可用性 |

## 6. 与其他模块的关系

```text
用户 / 应用
   │
   ▼
Agent trait ───────────────┐
   │                       │
   ▼                       │
ReActAgent                 │ observe/reply/reply_stream
   │
   ├─ AgentState           → 会话上下文、reply id、迭代状态
   ├─ ChatModel            → 模型调用与 token 估算
   ├─ ToolKit              → 工具 schema 与工具执行
   ├─ PermissionEngine     → 工具执行授权
   ├─ Middleware           → 记忆、RAG、审计、自定义扩展
   └─ AgentEvent           → 流式 UI 与 trace
```

Agent 模块的设计目标是“编排而不耦合”：模型 Provider、工具实现、记忆存储和 UI 渲染都在各自 crate 中独立演进，Agent 只依赖它们的 trait 与数据协议。

## 7. 常见坑 (Pitfalls)

1. **`chat.rs` 不自动读取环境变量作为 API key**：它只接受 `-k` / `--api-key`，即使 `.env` 中有 `API_KEY` 也要显式传参。
2. **不要并发启动同一个 Agent 的两个回复**：第二个会得到 `AlreadyStreaming`。如需并发，为每个会话创建独立 Agent 或等待当前 stream 结束。
3. **`reply(None)` 需要已有上下文**：先调用 `reply(Some(...))` 或 `observe(Some(...))`。
4. **流式消费必须读到结束或主动 drop stream**：否则 Agent 会认为仍有回复在进行。
5. **`ReActAgent::state()` 不适合直接调用**：读取状态请用 `try_state()`。
6. **工具输入是模型生成的 JSON 字符串**：工具层才会解析；工具 schema 描述越清晰，模型越不容易生成非法输入。
7. **权限默认不是 sandbox**：`Default` 模式无匹配规则时允许工具调用；只读场景请显式使用 `PermissionMode::Explore` 并配置 allow 规则。

## 8. 延伸阅读

- [快速上手](../getting-started.md) — 从命令行跑通 `chat` 示例
- [模型抽象](./model.md) — `ChatModel` 与流式/非流式模型返回
- [DashScope Provider](./dashscope.md) — 当前内置 Provider 的配置方式
- [工具系统](./tool.md) — `FunctionTool` / `ToolKit` / 工具调用生命周期
- [事件与流式](./event-streaming.md) — `AgentEvent` 与实时 UI 渲染
- [记忆与 Session 管理](../getting-started.md#5-十分钟看懂代码) — 通过 `MemoryMiddleware`、`AgentState` 与 session store 扩展 Agent

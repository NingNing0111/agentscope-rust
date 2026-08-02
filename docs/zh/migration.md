# Python → Rust 迁移参考

> 从 AgentScope Python 迁移到 AgentScope Rust 的实用指南。

## 架构差异速览

| 维度 | Python AgentScope | AgentScope Rust |
|------|------------------|-----------------|
| 类型系统 | 动态类型，`dict` / `Any` | 静态类型，`struct` / `enum` |
| 抽象方式 | 类继承（`class ReActAgent(AgentBase)`） | Trait + trait object（`Arc<dyn Agent>`） |
| 异步模型 | `async def` / `await` | `async fn` / `.await`（tokio runtime） |
| 错误处理 | 异常（`raise AgentError`） | `Result<T, AgentError>` |
| 共享所有权 | 引用计数（自动） | `Arc<dyn Trait>`（显式） |
| 序列化 | `json.dumps()` / `json.loads()` | `serde_json::to_string()` / `serde_json::from_str()` |

## 核心类型映射

### 消息与内容块

| Python | Rust |
|--------|------|
| `Msg(name, content, role)` | `Msg::new(name, content_blocks, role)` |
| `Msg(role="user")` | `factory::user_msg(name, text)` |
| `Msg(role="assistant")` | `factory::assistant_msg(name, text)` |
| `Msg(role="system")` | `factory::system_msg(name, text)` |
| `ContentBlock(type="text")` | `ContentBlock::TextBlock(TextBlock { ... })` |
| `ContentBlock(type="tool_call")` | `ContentBlock::ToolCallBlock(ToolCallBlock { ... })` |
| `ContentBlock(type="tool_result")` | `ContentBlock::ToolResultBlock(ToolResultBlock { ... })` |
| `ContentBlock(type="thinking")` | `ContentBlock::ThinkingBlock(ThinkingBlock { ... })` |
| `msg.get_text_content(sep)` | `msg.get_text_content(sep)` ✅ 一致 |

### Agent

| Python | Rust |
|--------|------|
| `ReActAgent(name, model, toolkit, ...)` | `ReActAgent::new(config, react_config, context_config, middlewares)` |
| `agent.reply(input)` | `agent.reply(input).await` |
| `agent.observe(msg)` | `agent.observe(input).await` |
| `agent.name` | `agent.name()` |
| `agent.reply_stream(input)` | `agent.reply_stream(input)` (返回 `Stream<Item = AgentEvent>`) |

### 工具系统

| Python | Rust |
|--------|------|
| `Toolkit()` | `ToolKit::new()` |
| `tk.register(tool)` | `tk.register(tool)` |
| `tk.get_tool_schemas()` | `tk.get_tool_schemas()` |
| `FunctionTool(name, desc, fn)` | `FunctionTool::new(name, desc, fn)` |

### 记忆系统

| Python | Rust |
|--------|------|
| `Memory.write(entry)` | `memory.write(entry).await` |
| `Memory.read(name)` | `memory.read(name).await` |
| `Memory.delete(name)` | `memory.delete(name).await` |
| `Memory.search(query, type)` | `memory.search(query, type_filter).await` |
| `MemoryMiddleware(memory)` | `MemoryMiddleware::new(memory, config)` |

### 事件系统

| Python | Rust |
|--------|------|
| `EventBase` | `EventBase` ✅ |
| `AgentEvent` (union type) | `AgentEvent` (enum) |
| `reply_start`, `reply_end` | `ReplyStartEvent`, `ReplyEndEvent` |
| `text_block_start/delta/end` | `TextBlockStartEvent/DeltaEvent/EndEvent` |
| `tool_call_start/end` | `ToolCallStartEvent`, `ToolCallEndEvent` |
| `tool_result_block` | `ToolResultBlockEvent` |

### 工作空间

| Python | Rust |
|--------|------|
| `LocalWorkspace(config)` | `LocalWorkspace::new(config)` |
| `ws.initialize()` | `ws.initialize().await` |
| `ws.close()` | `ws.close().await` |
| `ws.list_tools()` | `ws.list_tools().await` |

### 沙箱

| Python | Rust |
|--------|------|
| `SandboxSession(config)` | `LocalSandboxSession::new(config)` |
| `ss.execute(req)` | `ss.execute(request).await` |
| `ss.read_file(path)` | `ss.read_file(path).await` |

## 常见迁移模式

### 1. Agent 创建

```python
# Python
from agentscope import ReActAgent

agent = ReActAgent(
    name="assistant",
    model=model,
    toolkit=toolkit,
    sys_prompt="You are helpful."
)
```

```rust
// Rust
use agent_scope_agent::{AgentConfig, ReActAgent, ReActConfig, ContextConfig};
use std::sync::Arc;

let config = AgentConfig::builder()
    .name("assistant")
    .system_prompt("You are helpful.")
    .model(model) // Arc<dyn ChatModel>
    .toolkit(toolkit)
    .build()?;

let agent = ReActAgent::new(
    config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![], // middlewares
)?;
```

### 2. 消息构造与发送

```python
# Python
from agentscope import Msg

msg = Msg("user", "Hello!", "user")
reply = await agent(msg)
print(reply.get_text_content())
```

```rust
// Rust
use agent_scope_message::factory::user_msg;

let msgs = vec![user_msg("user", "Hello!")?];
let reply = agent.reply(Some(msgs)).await?;
println!("{}", reply.get_text_content(" ").unwrap_or_default());
```

### 3. 工具注册

```python
# Python
from agentscope import FunctionTool

def search(query: str) -> str:
    return f"Results: {query}"

tool = FunctionTool("search", "Search the web", search)
tk = Toolkit()
tk.register(tool)
```

```rust
// Rust
use agent_scope_tool::{FunctionTool, ToolKit};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SearchInput { query: String }

async fn search(input: SearchInput) -> String {
    format!("Results: {}", input.query)
}

let tool = FunctionTool::new("search", "Search the web", search);
let mut tk = ToolKit::new();
tk.register(tool);
```

> **注意**: Rust 侧 `FunctionTool` 使用 `schemars::JsonSchema` 自动生成 JSON Schema，输入参数需要是 `Deserialize + JsonSchema` 结构体，而非函数参数。

### 4. Memory middleware 注入

```python
# Python
from agentscope import MemoryMiddleware

memory = FileMemory(workdir, config)
mw = MemoryMiddleware(memory)
agent = ReActAgent(name="a", model=m, toolkit=tk, middlewares=[mw])
```

```rust
// Rust
use agent_scope_agent::MemoryMiddleware;
use agent_scope_memory::{FileMemory, MemoryConfig};
use std::sync::Arc;

let config = MemoryConfig {
    memory_dir: "memory_data".into(),
    ..Default::default()
};
let memory: Arc<dyn Memory> = Arc::new(FileMemory::new(workdir, config.clone(), None));
let mw = Arc::new(MemoryMiddleware::new(memory, config));
let agent = ReActAgent::new(
    agent_config,
    ReActConfig::default(),
    ContextConfig::default(),
    vec![mw],
)?;
```

### 5. 事件流消费

```python
# Python
async for event in agent.reply_stream(msg):
    match event.type:
        case "text_block_delta":
            print(event.delta, end="")
        case "tool_call_start":
            print(f"\n[Tool: {event.tool_name}]")
```

```rust
// Rust
use futures::StreamExt;
use agent_scope_event::AgentEvent;

let mut stream = agent.reply_stream(Some(msgs));
while let Some(event) = stream.next().await {
    match event {
        AgentEvent::TextBlockDelta(ev) => print!("{}", ev.delta),
        AgentEvent::ToolCallStart(ev) => println!("\n[Tool: {}]", ev.tool_name),
        _ => {}
    }
}
```

### 6. 错误处理

```python
# Python
try:
    reply = await agent(msg)
except AgentError as e:
    print(f"Error: {e}")
```

```rust
// Rust
match agent.reply(Some(msgs)).await {
    Ok(reply) => { /* handle */ }
    Err(AgentError::ModelError { .. }) => { /* model error */ }
    Err(AgentError::ToolError { .. }) => { /* tool error */ }
    Err(e) => eprintln!("Error: {e}"),
}
```

## 已知偏差

| 方面 | Python | Rust | 影响 |
|------|--------|------|------|
| AgentState | dict 自由扩展 | 固定 struct 字段 | 自定义状态字段不可用 |
| 模型配置 | 运行时 dict | 编译时 struct | 需要编译期确定模型参数 |
| 中间件顺序 | 注册顺序 | 注册顺序 ✅ | 一致 |
| 流式 Tool Call | 异步事件流 | 异步事件流 ✅ | 一致 |
| 上下文压缩 | 模型压缩 + 回退 | 模型压缩 + 回退 ✅ | 已实现 |
| Memory search | 子串匹配 | 子串匹配 ✅ | 一致 |
| 沙箱硬隔离 | 支持 Docker | 不支持，显式报告 | 生产环境需要用外部沙箱 |
| 多 Agent 协作 | 未实现 | 未实现 | 均在路线图上 |

## 快速检查清单

- [ ] 将 `class X(Y)` 继承改为 `trait X` + `impl X for Y`
- [ ] 将 `dict` 参数改为 `struct` 或 `Config` builder
- [ ] 将 `try/except` 改为 `match` / `?` 运算符
- [ ] 将共享引用用 `Arc<dyn Trait>` 包裹
- [ ] 将 `async def → await` 改为 `async fn → .await`
- [ ] 函数工具输入参数用 `#[derive(Deserialize, JsonSchema)]` 结构体替代函数签名
- [ ] 确认模型 provider 对应（当前仅 DashScope）
- [ ] 检查沙箱能力报告——硬隔离和网络隔离在 Rust 侧为 `false`

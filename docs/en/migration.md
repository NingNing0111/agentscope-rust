# Python → Rust Migration Guide

> A practical guide for migrating from AgentScope Python to AgentScope Rust.

## Architectural Differences at a Glance

| Dimension | Python AgentScope | AgentScope Rust |
|-----------|------------------|-----------------|
| Type System | Dynamic, `dict` / `Any` | Static, `struct` / `enum` |
| Abstraction | Class inheritance (`class ReActAgent(AgentBase)`) | Traits + trait objects (`Arc<dyn Agent>`) |
| Async Model | `async def` / `await` | `async fn` / `.await` (tokio runtime) |
| Error Handling | Exceptions (`raise AgentError`) | `Result<T, AgentError>` |
| Shared Ownership | Reference counting (automatic) | `Arc<dyn Trait>` (explicit) |
| Serialization | `json.dumps()` / `json.loads()` | `serde_json::to_string()` / `serde_json::from_str()` |

## Core Type Mapping

### Messages & Content Blocks

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
| `msg.get_text_content(sep)` | `msg.get_text_content(sep)` ✅ Same |

### Agent

| Python | Rust |
|--------|------|
| `ReActAgent(name, model, toolkit, ...)` | `ReActAgent::new(config, react_config, context_config, middlewares)` |
| `agent.reply(input)` | `agent.reply(input).await` |
| `agent.observe(msg)` | `agent.observe(input).await` |
| `agent.name` | `agent.name()` |
| `agent.reply_stream(input)` | `agent.reply_stream(input)` (returns `Stream<Item = AgentEvent>`) |

### Tool System

| Python | Rust |
|--------|------|
| `Toolkit()` | `ToolKit::new()` |
| `tk.register(tool)` | `tk.register(tool)` |
| `tk.get_tool_schemas()` | `tk.get_tool_schemas()` |
| `FunctionTool(name, desc, fn)` | `FunctionTool::new(name, desc, fn)` |

### Memory System

| Python | Rust |
|--------|------|
| `Memory.write(entry)` | `memory.write(entry).await` |
| `Memory.read(name)` | `memory.read(name).await` |
| `Memory.delete(name)` | `memory.delete(name).await` |
| `Memory.search(query, type)` | `memory.search(query, type_filter).await` |
| `MemoryMiddleware(memory)` | `MemoryMiddleware::new(memory, config)` |

### Event System

| Python | Rust |
|--------|------|
| `EventBase` | `EventBase` ✅ |
| `AgentEvent` (union type) | `AgentEvent` (enum) |
| `reply_start`, `reply_end` | `ReplyStartEvent`, `ReplyEndEvent` |
| `text_block_start/delta/end` | `TextBlockStartEvent/DeltaEvent/EndEvent` |
| `tool_call_start/end` | `ToolCallStartEvent`, `ToolCallEndEvent` |
| `tool_result_block` | `ToolResultBlockEvent` |

### Workspace

| Python | Rust |
|--------|------|
| `LocalWorkspace(config)` | `LocalWorkspace::new(config)` |
| `ws.initialize()` | `ws.initialize().await` |
| `ws.close()` | `ws.close().await` |
| `ws.list_tools()` | `ws.list_tools().await` |

### Sandbox

| Python | Rust |
|--------|------|
| `SandboxSession(config)` | `LocalSandboxSession::new(config)` |
| `ss.execute(req)` | `ss.execute(request).await` |
| `ss.read_file(path)` | `ss.read_file(path).await` |

## Common Migration Patterns

### 1. Agent Creation

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

### 2. Message Construction & Sending

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

### 3. Tool Registration

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

> **Note**: Rust-side `FunctionTool` uses `schemars::JsonSchema` for auto schema generation. Input parameters must be `Deserialize + JsonSchema` structs, not function parameters.

### 4. Memory Middleware Injection

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

### 5. Event Stream Consumption

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

### 6. Error Handling

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

## Known Deviations

| Aspect | Python | Rust | Impact |
|--------|--------|------|--------|
| AgentState | Free-form dict | Fixed struct fields | Custom state fields unavailable |
| Model config | Runtime dict | Compile-time struct | Model params determined at compile time |
| Middleware order | Registration order | Registration order ✅ | Same |
| Streaming tool calls | Async event stream | Async event stream ✅ | Same |
| Context compression | Model compression + fallback | Model compression + fallback ✅ | Implemented |
| Memory search | Substring match | Substring match ✅ | Same |
| Sandbox hard isolation | Docker supported | Not supported, explicitly reported | Use external sandbox for production |
| Multi-agent collaboration | Not implemented | Not implemented | Both on roadmap |

## Quick Migration Checklist

- [ ] Replace `class X(Y)` inheritance with `trait X` + `impl X for Y`
- [ ] Replace `dict` params with `struct` or Config builders
- [ ] Replace `try/except` with `match` / `?` operator
- [ ] Wrap shared references in `Arc<dyn Trait>`
- [ ] Replace `async def → await` with `async fn → .await`
- [ ] Use `#[derive(Deserialize, JsonSchema)]` structs instead of function parameters for tool inputs
- [ ] Confirm model provider mapping (currently only DashScope)
- [ ] Check sandbox capability report — hard isolation and network isolation are `false` in Rust

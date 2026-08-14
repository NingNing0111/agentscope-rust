---
title: "中间件"
description: "在智能体生命周期的关键位置拦截并扩展行为"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L3）。`Middleware` trait（9 个钩子点）在 AgentScope Rust 中可用。兼容基线为 AgentScope Python v2.0.5。
</Note>

智能体中间件是在不修改智能体或模型代码的前提下，向智能体执行流程中的关键位置注入自定义逻辑（日志、追踪、输入改写、访问控制等）的机制。

## Middleware trait

`Middleware`（`agent_scope_agent`）暴露 9 个异步钩子，全部默认为 no-op，按 FIFO 顺序调用：

| 钩子 | 触发时机 |
|------|----------|
| `pre_reply` / `post_reply` | 一次完整 `reply` / `reply_stream` 前后 |
| `on_system_prompt` | 每次组装 system prompt 时 |
| `pre_reasoning` / `post_reasoning` | 一轮 ReAct 推理步骤（输入组装 → 模型调用 → 流式解码）前后 |
| `pre_acting` / `post_acting` | 一次工具调用的执行前后 |
| `pre_observe` | `observe` 注入消息时 |
| `pre_print` | 打印输出前（用于改写） |

## 自定义中间件

实现 `Middleware` trait 并注入 `ReActAgent`：

```rust
use agent_scope_agent::{Middleware, AgentError};

struct LoggingMiddleware;

#[async_trait::async_trait]
impl Middleware for LoggingMiddleware {
    async fn pre_reply(&self, agent_name: &str, _input: &Option<Vec<agent_scope_message::Msg>>) -> Result<(), AgentError> {
        println!("[middleware] {agent_name} starting reply");
        Ok(())
    }
    async fn post_reply(&self, agent_name: &str, _input: &Option<Vec<agent_scope_message::Msg>>, _output: &agent_scope_message::Msg) -> Result<(), AgentError> {
        println!("[middleware] {agent_name} reply finished");
        Ok(())
    }
}
```

注入中间件列表：

```rust
let middlewares: Vec<Arc<dyn Middleware>> = vec![Arc::new(LoggingMiddleware)];
let agent = ReActAgent::new(config, ReActConfig::default(), ContextConfig::default(), middlewares)?;
```

## 内置中间件

| 中间件 | 职责 |
|--------|------|
| `MemoryMiddleware`（`agent_scope_agent`） | 长期记忆索引注入与检索（见 [长期记忆](long-term-memory)） |
| `RAGMiddleware`（`agent_scope_rag`） | 检索增强问答注入（见 [RAG](rag)） |

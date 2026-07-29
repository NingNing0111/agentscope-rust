# Contract: Middleware trait

**Feature**: 007-agent-system | **Trait**: `agent_scope_agent::Middleware`

## Purpose

The `Middleware` trait provides extension points (hooks) that allow external code to intercept and modify agent behavior at specific points in the reply lifecycle. Middleware enables logging, content filtering, custom permission policies, and other cross-cutting concerns without modifying agent source code.

## Interface

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    // Reply lifecycle
    async fn pre_reply(&self, _agent: &ReActAgent, _input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> { Ok(()) }
    async fn post_reply(&self, _agent: &ReActAgent, _result: &Result<Msg, AgentError>) -> Result<(), AgentError> { Ok(()) }

    // Reasoning step
    async fn pre_reasoning(&self, _agent: &ReActAgent, _messages: &mut Vec<Msg>, _tools: &mut Option<Vec<JsonValue>>) -> Result<(), AgentError> { Ok(()) }
    async fn post_reasoning(&self, _agent: &ReActAgent, _response: &ChatResponse) -> Result<(), AgentError> { Ok(()) }

    // Acting step (tool execution)
    async fn pre_acting(&self, _agent: &ReActAgent, _tool_call: &mut ToolCallBlock) -> Result<(), AgentError> { Ok(()) }
    async fn post_acting(&self, _agent: &ReActAgent, _result: &ToolExecOutput) -> Result<(), AgentError> { Ok(()) }

    // Observation
    async fn pre_observe(&self, _agent: &ReActAgent, _input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> { Ok(()) }

    // Print/output
    async fn pre_print(&self, _agent: &ReActAgent, _content: &mut String) -> Result<(), AgentError> { Ok(()) }
}
```

## Hook Execution Contract

### Invocation Order

For each hook point, middlewares are invoked in their registration order (FIFO). The registration order is the order of `Vec<Arc<dyn Middleware>>` passed to `ReActAgent::new()`.

### Mutable Context

Hooks that receive `&mut` parameters MAY modify them:
- `pre_reply`: MAY add, remove, or modify input messages.
- `pre_reasoning`: MAY modify messages sent to the model, or add/remove available tools.
- `pre_acting`: MAY modify the tool call input or name before execution.
- `pre_observe`: MAY filter or modify messages before they enter state context.
- `pre_print`: MAY modify the content string before it is printed.

### Error Handling

If any middleware returns `Err(AgentError)` from a hook:
- **`pre_reply`**: The reply is aborted, no model calls are made, `post_reply` is called with `Err(...)`.
- **`pre_reasoning`**: The current iteration is skipped; the error is fed to the model as context (if possible) or the loop exits.
- **`pre_acting`**: The tool call is skipped, a `ToolError` result is fed back to the model.
- **`pre_observe`**: The observe operation returns `Err(...)`.
- **`post_*` hooks**: Errors are logged but do NOT affect the reply result (reply has already completed).

### Panic Safety

If a middleware panics during hook execution (in the async context):
- The panic is caught at the dispatch boundary (using `std::panic::catch_unwind` or `AssertUnwindSafe`).
- The panic is converted to `AgentError::InternalError` with a description of which hook+middleware panicked.
- Subsequent middlewares for that hook point are NOT invoked.
- The agent continues with the error path appropriate for that hook position.

## Hook Point Mapping

Mapping to `agent_scope_types::hook` constants:

| Middleware Method | Hook Constant | Phase |
|------------------|---------------|-------|
| `pre_reply` | `agent_hooks::PRE_REPLY` | Before reply starts |
| `post_reply` | `agent_hooks::POST_REPLY` | After reply completes |
| `pre_reasoning` | `react_agent_hooks::PRE_REASONING` | Before model call |
| `post_reasoning` | `react_agent_hooks::POST_REASONING` | After model response |
| `pre_acting` | `react_agent_hooks::PRE_ACTING` | Before tool execution |
| `post_acting` | `react_agent_hooks::POST_ACTING` | After tool execution |
| `pre_observe` | `agent_hooks::PRE_OBSERVE` | Before context append |
| `pre_print` | `agent_hooks::PRE_PRINT` | Before content print |

`agent_hooks::POST_OBSERVE` and `agent_hooks::POST_PRINT` are NOT exposed in the `Middleware` trait — they are internal hooks for agent state management.

## Usage Examples

### Logging Middleware

```rust
struct LoggingMiddleware;

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn pre_reply(&self, agent: &ReActAgent, input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> {
        tracing::info!(agent = agent.name(), msg_count = input.as_ref().map(|m| m.len()).unwrap_or(0), "reply started");
        Ok(())
    }

    async fn post_reply(&self, agent: &ReActAgent, result: &Result<Msg, AgentError>) -> Result<(), AgentError> {
        match result {
            Ok(msg) => tracing::info!(agent = agent.name(), "reply completed"),
            Err(e) => tracing::error!(agent = agent.name(), error = %e, "reply failed"),
        }
        Ok(())
    }
}
```

### Content Filtering Middleware

```rust
struct ContentFilter {
    blocked_terms: Vec<String>,
}

#[async_trait]
impl Middleware for ContentFilter {
    async fn pre_reply(&self, _agent: &ReActAgent, input: &mut Option<Vec<Msg>>) -> Result<(), AgentError> {
        if let Some(msgs) = input {
            for msg in msgs.iter_mut() {
                for block in &mut msg.content {
                    if let ContentBlock::Text(tb) = block {
                        for term in &self.blocked_terms {
                            tb.text = tb.text.replace(term, "[FILTERED]");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
```

### Tool Approval Middleware

```rust
struct ToolApproval {
    allowed_tools: HashSet<String>,
}

#[async_trait]
impl Middleware for ToolApproval {
    async fn pre_acting(&self, agent: &ReActAgent, tool_call: &mut ToolCallBlock) -> Result<(), AgentError> {
        if !self.allowed_tools.contains(&tool_call.name) {
            return Err(AgentError::PermissionDenied {
                tool_name: tool_call.name.clone(),
                reason: format!("{} is not in the allowed list", tool_call.name),
            });
        }
        Ok(())
    }
}
```

## Test Contract

Middleware tests MUST verify:

1. **Isolation**: Each hook can be intercepted independently (register middleware implementing only one hook).
2. **FIFO order**: Middlewares registered as `[A, B, C]` fire in that order for each hook.
3. **Error propagation**: `pre_*` hook returning `Err` → expected error behavior per hook point.
4. **Mutation**: `pre_reasoning` modifying messages → modified messages sent to model.
5. **Post-hook safety**: `post_*` hook errors logged but don't change reply result.
6. **Panic safety**: Panicking middleware doesn't crash the agent; error is surfaced cleanly.

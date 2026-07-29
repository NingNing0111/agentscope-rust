# Contract: ReActAgent

**Feature**: 007-agent-system | **Struct**: `agent_scope_agent::ReActAgent`

## Purpose

`ReActAgent` is the primary agent implementation — it implements the `Agent` trait and drives the Reasoning + Acting loop. It combines a `ChatModel`, optional `ToolKit`, and optional `Middleware`s to process user input through iterative reasoning and tool execution.

## Construction

```rust
impl ReActAgent {
    pub fn new(
        config: AgentConfig,
        react_config: ReActConfig,
        context_config: ContextConfig,
        middlewares: Vec<Arc<dyn Middleware>>,
    ) -> Result<Self, AgentError>;
}
```

**Preconditions**:
- `config.name` MUST NOT be empty
- `config.model` MUST be `Some(model)`
- `react_config.max_iters` MUST be > 0
- `context_config.trigger_ratio` MUST be in (0.0, 1.0)
- `context_config.reserve_ratio` MUST be < `context_config.trigger_ratio`

**Postconditions**:
- Returns `Err(AgentError::InvalidConfig { field, message })` on validation failure.
- Returns `Ok(ReActAgent)` with initialized `AgentState`, internal event channel, and cancellation token.

## Reasoning-Acting Loop

The core loop invoked by `reply()`:

```text
1.  pre_reply hook
2.  emit ReplyStart
3.  append input to state.context (if Some)
4.  loop:
    a.  check cancel_token → if cancelled, break with interruption
    b.  check max_iters → if exceeded, emit ExceedMaxItersEvent, break
    c.  check context size → if exceeds trigger_ratio, compress
    d.  pre_reasoning hook
    e.  emit ModelCallStart
    f.  model.call(messages, tools, tool_choice) → response
    g.  emit ModelCallEnd
    h.  post_reasoning hook
    i.  if response has tool_calls:
        - for each tool_call:
            pre_acting hook
            permission check
            emit ToolCallStart
            toolkit.call_tool(tool_call) → result
            emit ToolCallEnd → ToolResultStart → ... → ToolResultEnd
            post_acting hook
            append tool result to state.context
        - continue loop (next iteration includes tool results in model input)
    j.  if response has text blocks:
        - emit TextBlockStart → TextBlockDelta... → TextBlockEnd
        - break loop (final response)
5.  post_reply hook
6.  emit ReplyEnd(finished_reason, final_msg)
7.  return final Msg
```

## Tool Call Lifecycle

When the model returns `ContentBlock::ToolCall` blocks:

1. **Detection**: After `model.call()`, scan `ChatResponse::content` for `ContentBlock::ToolCall` blocks.
2. **Pre-acting hook**: Fire `middleware.pre_acting(&self, &mut tool_call)` for each tool call. Middleware may modify the tool call input or reject it.
3. **Permission check**: `PermissionEngine::check(tool_name, input)` → Allow/Deny/RequireConfirm.
   - Deny → emit `RequireUserConfirmEvent`, wait for external confirmation (or stop if `stop_on_reject`).
4. **Execution**: `toolkit.call_tool(&tool_call)` → `ToolExecOutput`.
5. **Event emission**: `ToolCallStart` → `ToolCallEnd` → `ToolResultStart` → (text delta events) → `ToolResultEnd`.
6. **Context update**: Append `ContentBlock::ToolResult` to `state.context`.
7. **Loop continuation**: Feed tool results back to model in the next reasoning step.

## Context Compression

Triggered when estimated tokens > `context_size * trigger_ratio`:

1. Calculate `reserved_tokens = context_size * reserve_ratio`.
2. Identify the oldest messages whose cumulative token count exceeds `current_tokens - reserved_tokens`.
3. Call model with compression prompt to summarize those messages into a `summary` content block.
4. Replace compressed messages in `state.context[0..n]` with the summary.
5. If compression fails: emit error, fall back to simple truncation (keep last N messages fitting within reserve_ratio).

## Interruption

External interruption via `agent.interrupt()`:

1. Sets the internal `cancel_token`.
2. The agent checks `cancel_token` at each loop iteration boundary (step 4a).
3. If cancelled:
   - Mark all pending tool calls as interrupted.
   - Emit `UserInterruptEvent`.
   - Emit `ReplyEnd(finished_reason = Interrupted)`.
   - Return `Msg` with `interruption_message` as content.

## Structured Output

When `reply_context.structured_schema` is set:

1. The agent calls `model.generate_structured_output(messages, schema)` instead of `model.call()`.
2. If parsing fails, the agent retries up to `structured_output_grace_iters` times.
3. On success, the parsed JSON is stored in `reply_context.structured_output` and included in the final Msg.

## Concurrency & Safety

- All methods take `&self` (shared reference). Internal state is protected by `RwLock<AgentState>`.
- `reply()` acquires a write lock on state during context updates; read lock during hook dispatch.
- Event emission is non-blocking via `broadcast::Sender::send()`.
- The agent is `Send + Sync` (all its components are).

## Error Handling

| Error Scenario | Behavior |
|---------------|----------|
| Model call fails after retries | Return `Err(AgentError::ModelError)` |
| Tool execution fails | Emit `ToolResultEnd(state=execution_error)`, feed error to model, continue loop |
| Permission denied + `stop_on_reject=true` | Emit `ReplyEnd`, return Err |
| `max_iters` exceeded | Emit `ExceedMaxItersEvent`, return last model response as Msg |
| Context compression model call fails | Fall back to truncation, log warning |
| Interruption during reply | Return `interruption_message` as Msg content |

## Test Contract

Tests MUST verify:

1. **Event sequence**: Full `AgentEvent` trace for a complete ReAct cycle matches expected order.
2. **Tool dispatch**: Mock model returns tool_call → agent executes tool → result fed back → final text response.
3. **Max iterations**: Mock model always returns tool_call → loop exits at max_iters → `ExceedMaxItersEvent` emitted.
4. **Interruption**: Cancel during tool execution → loop exits cleanly → final Msg has interruption message.
5. **Context compression**: Context exceeds trigger → compression invoked → context length reduced.
6. **Middleware hooks**: Each hook fires at correct time with correct context.
7. **Permission**: Tool denied → `RequireUserConfirmEvent` emitted (or stop if `stop_on_reject`).
8. **Error propagation**: `ToolError` from tool → `ToolResultEnd` with error → model receives error context.

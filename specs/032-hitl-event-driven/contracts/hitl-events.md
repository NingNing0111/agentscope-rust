# Feature 032 接口契约

**Feature**: 事件驱动 HITL 确认机制与 Python 对齐
**Date**: 2026-08-14

## 契约 1: `Agent::reply_stream` 输入类型

**契约目的**: Rust `reply_stream` 接受事件类输入，与 Python `_reply_impl` 对齐。

**当前签名**（`agent_trait.rs:33-36`）:
```rust
async fn reply_stream(
    &self,
    input: Option<Vec<Msg>>,
) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;
```

**目标签名**（新增事件输入，保持消息输入兼容）:
```rust
/// 输入类型：普通消息或 HITL 事件。
#[derive(Debug)]
pub enum AgentInput {
    /// 普通消息列表（现有行为）。
    Messages(Vec<Msg>),
    /// 用户确认结果（暂停后恢复）。
    Confirm(UserConfirmResultEvent),
    /// 用户中断（打断当前回复）。
    Interrupt(UserInterruptEvent),
    /// 外部执行结果（外部执行后恢复）。
    ExternalResult(ExternalExecutionResultEvent),
}

async fn reply_stream(
    &self,
    input: Option<AgentInput>,
) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;
```

**兼容性**: 现有调用 `reply_stream(Some(vec![msg]))` 遍布 6 示例 + 多测试（18 处调用点）。为最小化破坏并保持向后兼容，**采用增量方法**：

```rust
// 现有签名不变（向后兼容）
async fn reply_stream(
    &self,
    input: Option<Vec<Msg>>,
) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;

// 新增事件输入方法（对齐 Python，供 HITL 宿主使用）
async fn reply_stream_event(
    &self,
    input: EventInput,
) -> Result<Pin<Box<dyn Stream<Item = AgentEvent> + Send>>, AgentError>;
```

其中 `EventInput` 为事件联合：

```rust
pub enum EventInput {
    Confirm(UserConfirmResultEvent),
    Interrupt(UserInterruptEvent),
    ExternalResult(ExternalExecutionResultEvent),
}
```

**对齐**（Python `_agent.py:758-793`）:
```python
inputs: Msg | list[Msg] | UserConfirmResultEvent | UserInterruptEvent
        | ExternalExecutionResultEvent | None
```

> 决策：Python 用统一 `inputs` 类型，Rust 为减少破坏保留 `reply_stream` + 新增 `reply_stream_event`。两个方法共享同一底层 `_reply_impl` 逻辑（事件 vs 消息 dispatch），语义等价。

**对齐**（Python `_agent.py:758-793`）:
```python
inputs: Msg | list[Msg] | UserConfirmResultEvent | UserInterruptEvent
        | ExternalExecutionResultEvent | None
```

## 契约 2: `RequireUserConfirmEvent` 载荷

**契约目的**: 事件携带完整 awaiting 信息（含 `suggested_rules`），宿主可据此向用户展示并构建确认结果。

**对齐**（Python hitl_user_confirmation_test.py:365-386）:
```json
{
  "type": "REQUIRE_USER_CONFIRM",
  "reply_id": "...",
  "tool_calls": [
    {
      "id": "...", "name": "...", "input": "...",
      "state": "asking",
      "suggested_rules": [ { "tool_name": "...", "rule_content": null,
                             "behavior": "allow", "source": "suggested" } ]
    }
  ]
}
```

**要求**: `state="asking"`、`suggested_rules` 非空（FR-003）。

## 契约 3: `UserConfirmResultEvent` 恢复语义

**契约目的**: 宿主注入确认结果恢复暂停的回复。

**对齐**（Python `_handle_incoming_event`）:
```json
{
  "type": "USER_CONFIRM_RESULT",
  "reply_id": "...",
  "confirm_results": [
    { "confirmed": true, "tool_call": { "id": "...", "name": "...", "input": "..." },
      "rules": [ { "tool_name": "...", "behavior": "allow" } ] }
  ]
}
```

**校验规则**:
- agent 无 awaiting 确认 → 报错（FR-007）
- confirm_results 的 id 集 ⊆ awaiting 确认 id 集，否则报错指出额外 id（FR-008）
- reply_id 匹配暂停回复（FR-010）
- `confirmed=true` → 执行工具，`rules` 采纳（FR-005/009）
- `confirmed=false` → 生成 `state=DENIED` tool_result（FR-006）

## 契约 4: `UserInterruptEvent` 语义

**对齐**（Python `_agent.py:807-814`）:
- agent 有 awaiting tool calls → 以 `ReplyEnd(finished_reason=INTERRUPTED)` 结束
- 无 → 静默 no-op

## 契约 5: `ExternalExecutionResultEvent` 恢复语义

**对齐**（Python `_agent.py:1627-1649`）:
- 结果追加到 context，更新工具状态 finished（FR-014）
- 类型/id 不匹配 → 报错（FR-015）

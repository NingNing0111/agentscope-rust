---
title: "人机交互"
description: "暂停以等待用户确认，再通过结果事件恢复"
---

<Note>
**Rust 实现状态**: 已实现（兼容等级 L2）——事件级 + 权限级，对齐 Python 的「暂停 → 确认 → 恢复」状态机。当权限系统判定某工具调用需要确认时，`ReActAgent` 发出 `RequireUserConfirmEvent` 并**暂停**当前 reply_stream（不喂 denied、无 ReplyEnd）。宿主收集确认后以 `UserConfirmResultEvent` 通过 `reply_stream_event` **恢复同一 agent**，按 tool_call_id 精确匹配执行/拒绝。兼容基线为 AgentScope Python v2.0.5。
</Note>

当权限系统判断某个工具调用需要用户批准时，智能体会发出 `RequireUserConfirmEvent` 并暂停，等待宿主（调用方）以确认事件恢复。

## 暂停-确认-恢复流程

```rust
use agent_scope_agent::event_input::EventInput;
use agent_scope_event::{ConfirmResult, EventBase, UserConfirmResultEvent};

// 1. 首次调用：遇到需确认的工具时流会暂停（无 ReplyEnd）。
let mut stream = agent.reply_stream(Some(vec![msg])).await?;
let mut confirm = None;
while let Some(event) = stream.next().await {
    match &event {
        AgentEvent::RequireUserConfirm(c) => {
            // 宿主在这里询问用户 y/n/a；c.tool_calls 含 state="asking" 与 suggested_rules。
            confirm = Some(c.clone());
        }
        AgentEvent::ToolResultTextDelta(d) => print!("{}", d.delta),
        AgentEvent::ReplyEnd(e) => println!("[end] {:?}", e.finished_reason),
        _ => {}
    }
    if confirm.is_some() { break; }
}

// 2. 以确认事件恢复同一 agent（不重建、不截断历史）。
if let Some(confirm) = confirm {
    let resume = UserConfirmResultEvent {
        base: EventBase::new(),
        reply_id: confirm.reply_id.clone(),
        confirm_results: vec![
            ConfirmResult {
                confirmed: true,          // false = 拒绝 → 生成 DENIED 结果
                tool_call: confirm.tool_calls[0].clone(),
                rules: None,              // 可携带 allow 规则，采纳后不再询问
            },
        ],
    };
    stream = agent.reply_stream_event(EventInput::Confirm(resume)).await?;
    while let Some(event) = stream.next().await { /* 继续消费 */ }
}
```

## 事件输入

`reply_stream_event` 接受三类 HITL 事件（对齐 Python `_reply_impl` 的 `inputs` 联合）：

| 事件 | 语义 |
|------|------|
| `UserConfirmResultEvent` | 恢复暂停的确认：`confirmed=true` 执行工具（可带 `rules` 采纳）、`confirmed=false` 生成 `DENIED` 结果 |
| `ExternalExecutionResultEvent` | 恢复外部执行暂停（外部工具触发 `RequireExternalExecutionEvent` 后暂停） |
| `UserInterruptEvent` | 中断当前回复，以 `ReplyEnd(finished_reason=INTERRUPTED)` 结束；无进行中回复时静默 no-op |

校验契约（非法恢复返回明确错误）：无 awaiting 时注入确认事件报错；tool_call_id 与等待状态不匹配报错；`reply_id` 与暂停回复不匹配报错。

## `RequireUserConfirmEvent` 的结构

| 字段 | 类型 | 说明 |
|------|------|------|
| `reply_id` | `String` | 当前回复的 ID（恢复时需匹配） |
| `tool_calls` | `Vec<ToolCallBlock>` | 等待确认的工具调用列表，每个含 `name` / `input` / `state="asking"` / `suggested_rules` |

## 权限系统联动

是否需要确认由权限系统决定（[权限系统](../permission-system/overview)）：

- `PermissionRule::allow` → 直接执行
- `PermissionRule::deny` → 拒绝并把错误结果喂给模型
- `PermissionRule::ask` → 发出 `RequireUserConfirmEvent` 并暂停；恢复时 `confirmed=true` 执行、`confirmed=false` 生成 `DENIED`

## 完整示例

见 [`examples/human-in-the-loop`](https://github.com/NingNing0111/agentscope-rust/tree/master/examples/human-in-the-loop/)（`cargo run -p human-in-the-loop`），完整演示暂停-确认-恢复闭环：`ask` 规则触发 `RequireUserConfirmEvent` → 宿主在 stdin 询问用户 y/n/a → `y` 以 `confirmed=true` 恢复执行、`n` 以 `confirmed=false` 拒绝、`a` 携带 allow 规则采纳进引擎（此后不再询问）。同一 agent 实例贯穿整个会话，**不截断历史、不重建 agent**。

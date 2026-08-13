# Feature 032 数据模型

**Feature**: 事件驱动 HITL 确认机制与 Python 对齐
**Date**: 2026-08-14

## 实体与字段

### AgentEvent（事件协议，扩展）

事件枚举已含所需变体（`agent_scope_event` crate）。本 feature 改变其**生产与消费语义**：

| 变体 | 现有字段 | 本 feature 变更 |
|------|---------|----------------|
| `RequireUserConfirmEvent` | `{ base, reply_id, tool_calls: Vec<ToolCallBlock> }` | `tool_calls` 需携带 `state=asking` 与 `suggested_rules`（FR-003）；支持多工具并发（FR-011） |
| `UserConfirmResultEvent` | `{ base, reply_id, confirm_results: Vec<ConfirmResult> }` | 引擎开始**消费**（恢复暂停），校验 reply_id 与 awaiting 状态（FR-004/007/008/010） |
| `ConfirmResult` | `{ confirmed: bool, tool_call: ToolCallBlock, rules: Option<Vec<PermissionRule>> }` | 已定义；`rules` 被采纳（FR-009） |
| `UserInterruptEvent` | `{ base, reply_id }` | 引擎开始**消费**（FR-016） |
| `RequireExternalExecutionEvent` | `{ base, reply_id, tool_calls }` | 引擎开始**生产**并暂停（FR-013） |
| `ExternalExecutionResultEvent` | `{ base, reply_id, execution_results: Vec<ToolResultBlock> }` | 引擎开始**消费**（FR-014） |

### ToolCallBlock（工具调用块）

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | String | 工具调用 id（暂停/恢复匹配键，FR-004） |
| `name` | String | 工具名 |
| `input` | String | JSON 参数字符串 |
| `state` | ToolCallState | `asking`=等待确认（暂停点）、`submitted`=等待外部执行、`allowed`/`finished`/`denied` |
| `suggested_rules` | Vec\<PermissionRule\> | ASK 决策附带建议（FR-003） |

### ToolCallState（状态机）

```
Pending → Asking → (confirmed=true) → Allowed → Finished
                 → (confirmed=false) → Denied
                 → (UserInterrupt)   → Interrupted
Pending → Submitted → (外部结果) → Finished
```

### AgentState.context（权威对话历史）

末尾 assistant 消息含 tool_call blocks（state=asking/submitted）与 tool_result blocks。**Awaiting 判定**（对齐 Python `get_awaiting_tool_calls`）：

- 扫描 context 末尾 assistant 消息
- 取 `state==asking` 的 tool_call，或 `state==submitted` 且无匹配 tool_result 的 tool_call

### AgentInner（运行时状态，需变更）

| 字段 | 当前 | 变更 |
|------|------|------|
| `config.permission_context` | 不可变 `PermissionContext` | **新增**可变 `Arc<RwLock<PermissionEngine>>`（FR-009 采纳 rules） |
| `state` | `Arc<RwLock<AgentState>>` | 已有，awaiting 判定从 context 提取 |

### ReActAgent::reply_stream 输入（API 变更）

```
保留: reply_stream(input: Option<Vec<Msg>>)          // 现有签名，18 处调用点不动
新增: reply_stream_event(input: EventInput)          // 事件输入，对齐 Python
      EventInput = Confirm(UserConfirmResultEvent)
                  | Interrupt(UserInterruptEvent)
                  | ExternalResult(ExternalExecutionResultEvent)
```

（Python 用统一 `_reply_impl(inputs: Msg | list[Msg] | UserConfirmResultEvent | UserInterruptEvent | ExternalExecutionResultEvent | None)`；Rust 为减少破坏用增量方法，两个方法共享底层 dispatch 逻辑）

## 验证规则（来自 FR）

- FR-007: 无 awaiting 确认时注入 `UserConfirmResultEvent` → 报错
- FR-008: 确认结果 id 与 awaiting 不匹配 → 报错并指出额外 id
- FR-010: 确认事件 reply_id 与暂停回复不匹配 → 报错
- FR-015: 外部执行结果类型/ id 不匹配 → 报错

## 状态转换

暂停 → 恢复由宿主驱动：
1. 引擎 Ask → emit `RequireUserConfirmEvent` → **结束当前 reply_stream**（不喂 denied）
2. 宿主读 awaiting tool_calls，构建 `UserConfirmResultEvent`
3. 再次 `reply_stream(event)` → 引擎校验 → 按 id 匹配执行/拒绝 → 继续循环

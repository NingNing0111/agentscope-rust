# Feature 032 研究记录

**Feature**: 事件驱动 HITL 确认机制与 Python 对齐
**Date**: 2026-08-14

## 研究任务与结论

### R-1: Rust 引擎当前 Ask 处理机制（暂停 vs 重放）

**Decision**: Rust 当前 Ask 时**不暂停**，`execute_tool_calls` 对 Ask 工具 `continue`，主循环继续 → 模型被喂 denied 后重新生成。需改为 Python 式暂停。

**Rationale**: 已确认 `streaming_reactor.rs:1041-1053` 与 `react_loop.rs:541-555` 的 Ask 分支：设 `state=Asking`、emit `RequireUserConfirmEvent`、emit denied tool_result、`continue`。主循环 `streaming_reactor.rs:370-386` 对 tool_calls 非空时"Continue ReAct loop"。这与 Python `_reply_impl` 的暂停语义（tool_call 保持 asking，等待 `UserConfirmResultEvent`）根本不同。

**Alternatives considered**:
- 保持现状（宿主重放）→ 不满足宪法第一条兼容性优先。

**关键证据**:
- `streaming_reactor.rs:1041-1053`：Ask → `continue`
- `react_loop.rs:541-555`：Ask → `continue`
- `process_response_and_continue`（streaming_reactor.rs:903-974）：tool_calls 非空 → `Outcome::Continue`

### R-2: reply_stream 输入类型改造

**Decision**: Rust `reply_stream(input: Option<Vec<Msg>>)` 需扩展为接受事件类输入，与 Python `_reply_impl(inputs: Msg | list[Msg] | UserConfirmResultEvent | UserInterruptEvent | ExternalExecutionResultEvent | None)` 对齐。

**Rationale**: `agent_trait.rs:33-36` 当前只接受 `Option<Vec<Msg>>`。Python 统一接受事件与消息。Q2=B 决策要求三类事件输入全对齐。

**关键证据**:
- `agent_trait.rs:25-54`：`Agent` trait `reply_stream(input: Option<Vec<Msg>>)`
- Python `_agent.py:758-793`：`_reply_impl` 按类型 dispatch 事件 vs 消息

### R-3: 暂停状态如何追踪（awaiting tool calls）

**Decision**: 从 `state.context` 末尾 assistant 消息**提取** awaiting tool calls（对齐 Python `get_awaiting_tool_calls`），不新增独立字段。

**Rationale**: Python `_state.py:312-339` 的 `get_awaiting_tool_calls` 扫描 context 末尾 assistant 消息，返回 `state==ASKING` 或 `state==SUBMITTED`（且无对应 tool_result）的 tool_call blocks。Rust 对齐此行为：`state.context` 末尾 assistant 消息含 tool_call blocks，其 state 已由 Ask/Submit 分支设置。恢复时校验"agent 是否在等待"及"id 匹配"（FR-007/008）。

**Alternatives considered**:
- A: 从 context 末尾 assistant 消息扫描（对齐 Python）→ **采用**
- B: 在 `ReplyContext` 增加 `awaiting_tool_calls` 字段（结构化，但偏离 Python 行为基准、需维护序列化）

**Decision**: A——从 context 提取，与 Python 行为基准一致，无需新增持久化字段。

### R-4: 采纳 rules 到权限引擎

**Decision**: `ConfirmResult.rules` 需采纳进引擎（FR-009）。当前 `AgentConfig.permission_context` 不可变（config.rs:36，`PermissionContext` 值类型），`PermissionEngine::with_context` 每次克隆。需在 `AgentInner` 增加**可变** `Arc<RwLock<PermissionContext>>` 或 `Arc<RwLock<PermissionEngine>>`，使运行中可 add_rule。

**Rationale**: Python `_handle_incoming_event` 在 `confirmation.rules` 时 `self._engine.add_rule(rule)`（_agent.py:1607-1609），引擎是实例可变状态。Rust 需等价能力。

**Alternatives considered**:
- A: `AgentInner` 持有 `Arc<RwLock<PermissionEngine>>`，运行中 `add_rule`
- B: 重建 agent（现状）→ 无法满足"同一 agent 恢复"，违反 FR-004/005

**Decision**: A——`AgentInner` 持有可变 `PermissionEngine`。需评估对 `react_loop`/`streaming_reactor` 中现有 `PermissionEngine::with_context(ctx.permission_context.clone())` 调用点的改动。

### R-5: 并发多工具确认

**Decision**: 支持多工具并发确认（Q1=B）。`RequireUserConfirmEvent` 一次携带全部 asking tool_call，`UserConfirmResultEvent` 支持多个 `ConfirmResult` 按 id 匹配，Python 规则去重。

**Rationale**: Python `_handle_incoming_event`（_agent.py:1580-1625）遍历 `last_msg` 的 tool_call blocks，按 `confirmed_tool_calls[tool_call.id]` 逐个处理，`confirmed` 则执行、否则 `_handle_error_tool_call` 生成 DENIED。`_check_incoming_event`（_agent.py:1524-1531）校验额外 id 报错。

**关键证据**:
- Python `_check_incoming_event`：`extra_ids = set(_.tool_call.id for _ in event.confirm_results) - set(awaiting_confirmations)` → 报错
- Python `_handle_incoming_event`：按 id 逐个处理，`rules` 采纳到 `_engine.add_rule`

### R-6: 拒绝语义（DENIED tool_result）

**Decision**: `confirmed=false` 时工具不执行，生成 `state=DENIED` 的 tool_result，内容含 `<system-reminder>` 提示（对齐 Python `_handle_error_tool_call`，message="The execution of tool ... is denied by user!"）。

**Rationale**: Python `_handle_incoming_event`（_agent.py:1611-1622）`_handle_error_tool_call(tool_call, message=<system-reminder>..., state=DENIED)`。Rust 现有 `emit_permission_denied_result` 已生成 `ToolResultState::Denied`（react_loop.rs:851-904），可复用该机制。

### R-7: 中断（UserInterruptEvent）语义

**Decision**: 注入 `UserInterruptEvent` 时，若 agent 有 awaiting tool calls（进行中或等待确认），以 `ReplyEnd(finished_reason=INTERRUPTED)` 结束；无则静默 no-op。

**Rationale**: Python `_agent.py:807-814`：`if isinstance(inputs, UserInterruptEvent): if self.state.has_awaiting_tool_calls(self.name): end_event = ReplyEndEvent(INTERRUPTED); return`。Rust 现有 `interrupt()` 方法（react_agent.rs:211）已设 `inner.interrupted`，但事件输入路径需新增。

### R-8: 外部执行（ExternalExecutionResultEvent）

**Decision**: 工具请求外部执行（`RequireExternalExecutionEvent`）时暂停，`ExternalExecutionResultEvent` 恢复，结果追加 context 并更新工具状态 finished（对齐 Python `_handle_incoming_event` 的 elif 分支，_agent.py:1627-1649）。

**Rationale**: Python 该分支直接 append execution_results 到 context，更新 tool_call 状态为 FINISHED。Rust 已有 `RequireExternalExecutionEvent`/`ExternalExecutionResultEvent` 类型（control_events.rs:62-77）但引擎不消费。

### R-9: suggested_rules 事件载荷

**Decision**: `RequireUserConfirmEvent` 的 tool_call 需携带 `suggested_rules`（对齐 Python 事件里 `suggested_rules: [{tool_name, rule_content, behavior: ALLOW, source: "suggested"}]`）。

**Rationale**: Python 测试（hitl_user_confirmation_test.py:376-384）明确断言事件里 tool_call 带 `suggested_rules`。Rust `PermissionDecision.suggested_rules` 已在 check_decision 生成（permission.rs:270-272），但 `emit_require_user_confirm` 构造事件时未填充。需在 Ask 分支把 `decision.suggested_rules` 写入 tool_call.suggested_rules。

### R-10: 黄金快照/差分测试策略

**Decision**: 新增/更新测试用 **Mock Model**（确定性响应序列）驱动，比较完整事件 trace，而非真实 LLM 输出。

**Rationale**: 宪法第六条要求确定性测试。Python 测试用 `MockModel.set_responses([...])` 精确控制模型响应序列，断言完整事件列表。Rust 测试需对齐此模式——构造 mock model 返回"tool_call → 无工具纯文本"的序列，验证暂停/恢复事件顺序。

## 关键设计决策汇总

| # | 决策 | 依据 |
|---|------|------|
| D-1 | Ask 暂停而非重放 | R-1，宪法第一条 |
| D-2 | reply_stream 接受三类事件输入 | R-2，Q2=B |
| D-3 | 从 context 末尾 assistant 消息提取 awaiting tool calls（对齐 Python get_awaiting_tool_calls） | R-3，FR-007/008 |
| D-4 | AgentInner 持有可变 PermissionEngine | R-4，FR-009 |
| D-5 | 并发多工具确认 + 去重 | R-5，Q1=B |
| D-6 | 拒绝生成 DENIED + system-reminder | R-6，FR-006 |
| D-7 | 中断 no-op 当无 awaiting | R-7，FR-016 |
| D-8 | 外部执行结果恢复 | R-8，FR-013/014 |
| D-9 | 事件带 suggested_rules | R-9，FR-003 |
| D-10 | Mock Model 驱动黄金快照 | R-10，宪法第六条 |

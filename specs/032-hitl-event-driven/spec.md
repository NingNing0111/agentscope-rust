# Feature Specification: 事件驱动 HITL 确认机制与 Python 对齐

**Feature Branch**: `032-hitl-event-driven`

**Created**: 2026-08-13

**Status**: Draft

**Input**: User description: "修改现有的事件驱动，保证human-in-loop和python的保持一致"

## 背景与动机

现有 Rust 引擎的 human-in-the-loop 确认闭环与 Python 参考实现存在根本差异：

- **Python**：`REQUIRE_USER_CONFIRM` 事件后 reply_stream **暂停**，tool_call 保持 `state="asking"`，宿主注入 `UserConfirmResultEvent` 后**同一 agent 恢复**，按 tool_call_id 精确匹配放行/拒绝；拒绝时工具收到 `state=DENIED` 的 `<system-reminder>` 结果。同一回复可含**多个需确认工具**（并发/去重）。事件输入统一支持三类：`UserConfirmResultEvent` / `UserInterruptEvent` / `ExternalExecutionResultEvent`。
- **Rust**：`RequireUserConfirmEvent` 后引擎 `continue`，把 denied 结果喂回模型，模型**同一流内重新生成**；宿主只能截断历史回退 + 重建 agent 重放，无法精确匹配 tool_call_id，也不支持多工具并发确认与外部执行/中断事件输入。

宪法第一条（兼容性优先）与第三条（Python 是行为基准）要求：Rust 的事件驱动 HITL 确认机制 MUST 与 Python 保持可观察一致。本 feature 修改事件驱动核心，使 Rust 的确认闭环与 Python 一致——含**多工具并发确认**与**三类事件输入**。

## 用户故事与验收 (mandatory)

### User Story 1 - 宿主以事件恢复暂停的回复 (Priority: P1)

开发者（宿主）调用 agent 的流式回复，agent 调用需确认的工具时暂停；宿主收集确认结果后以事件恢复**同一 agent** 继续执行。同一回复可含多个需确认工具，宿主逐个确认。

**Why this priority**: 这是 Python 与 Rust HITL 一致性的核心——"暂停-确认-恢复"状态机取代"截断历史-重建 agent 重放"。缺少它，所有上层交互（human-in-loop 示例、工具确认 UI）都无法与 Python 对齐。

**Independent Test**: 仅实现"暂停 + 以 UserConfirmResultEvent 恢复同一 agent"即可独立验证：一个 mock 工具返回 `PermissionBehavior::Ask`，宿主确认后工具执行、agent 从暂停点继续。

**Acceptance Scenarios**:

1. **Given** 工具触发 `RequireUserConfirmEvent`，**When** 宿主暂停当前流并读取待确认的 tool_call（含 id、name、input、state="asking"、suggested_rules），**Then** 事件携带完整信息且 reply_stream 结束（不喂 denied 给模型）。
2. **Given** 宿主构建 `UserConfirmResultEvent{ confirm_results: [{ confirmed: true, tool_call }] }`，**When** 再次调用 `reply_stream` 注入该事件，**Then** 同一 agent 恢复，匹配 tool_call_id 执行工具，从暂停点继续。
3. **Given** 宿主确认结果里 tool_call_id 与暂停的 asking 工具匹配，**When** 恢复，**Then** 工具正常执行并产生 tool_result，后续回复继续。
4. **Given** 宿主构建 `confirmed: false` 的确认结果，**When** 恢复，**Then** 工具**不执行**，生成 `state=DENIED` 的 tool_result（含 `<system-reminder>` 拒绝提示），agent 调整继续。
5. **Given** 同一回复含多个需确认工具（并发，均 state="asking"），**When** 宿主一次性注入多个 `ConfirmResult` 或逐个注入，**Then** 按 tool_call_id 逐个匹配执行/拒绝，全部处理完毕 agent 才继续。

### User Story 2 - 拒绝未等待确认的恢复请求 (Priority: P2)

宿主错误地注入与当前等待确认状态不匹配的确认事件时，系统给出明确错误而非静默接受。

**Why this priority**: Python `_check_incoming_event` 会校验"agent 是否在等待确认"及"tool_call_id 是否匹配"。Rust 需对齐该错误契约，防止状态机错乱。

**Independent Test**: agent 不处于等待确认状态时注入 `UserConfirmResultEvent`，应报错。

**Acceptance Scenarios**:

1. **Given** agent 无等待确认的 tool_call，**When** 注入 `UserConfirmResultEvent`，**Then** 返回明确错误（如 "Agent is not waiting for user confirmation"）。
2. **Given** agent 等待确认的 tool_call_id 与注入的不匹配，**When** 恢复，**Then** 返回明确错误指出额外 tool_call_id。

### User Story 3 - 确认结果可携带放行规则 (Priority: P3)

宿主确认接受时可附带允许规则，引擎采纳后后续同类调用不再询问（Python `ConfirmResult.rules` → `engine.add_rule`）。

**Why this priority**: Python 支持"接受 + 添加 allow 规则"（对应 Rust 示例的 `a`=总是允许）。对齐此能力让上层可复用 Python 语义。

**Independent Test**: 确认结果带 `rules: [allow(...)]`，恢复后同一工具再次调用不再触发确认。

**Acceptance Scenarios**:

1. **Given** 确认结果携带 `rules: [PermissionRule::allow(tool)]`，**When** 恢复并执行，**Then** 该规则被采纳进引擎，后续同类调用直接放行。

### User Story 4 - 外部执行结果以事件注入 (Priority: P2)

工具请求外部执行（`RequireExternalExecutionEvent`）后 reply_stream 暂停；宿主收集外部执行结果后以 `ExternalExecutionResultEvent` 恢复同一 agent。

**Why this priority**: Python `reply_stream(inputs=...)` 统一接受 `ExternalExecutionResultEvent`，是三类事件输入之一。对齐后外部工具（如 MCP server、sandbox）可复用同一暂停/恢复机制。

**Independent Test**: mock 工具触发 `RequireExternalExecutionEvent`，宿主注入 `ExternalExecutionResultEvent` 后工具结果入 context、agent 继续。

**Acceptance Scenarios**:

1. **Given** 工具触发 `RequireExternalExecutionEvent`，**When** reply_stream 暂停并携带 `tool_calls`，**Then** 事件载荷含待外部执行的 tool_call 且流结束。
2. **Given** 宿主注入 `ExternalExecutionResultEvent{ execution_results: [...] }`，**When** 恢复，**Then** 工具结果追加到 context，agent 从暂停点继续。
3. **Given** 注入的外部执行结果 tool_call_id 与暂停状态不匹配，**When** 恢复，**Then** 返回明确错误。

### User Story 5 - 用户中断事件 (Priority: P3)

宿主在回复进行中注入 `UserInterruptEvent` 打断当前回复，agent 生成中断结束事件。

**Why this priority**: Python `UserInterruptEvent` 使 reply_stream 以 `REPLY_END finished_reason=INTERRUPTED` 结束，是三类事件输入之一。对齐后可复用同一输入类型。

**Independent Test**: 注入 `UserInterruptEvent` 后 agent 以 interrupted 结束回复。

**Acceptance Scenarios**:

1. **Given** agent 正在回复或等待确认，**When** 宿主注入 `UserInterruptEvent`，**Then** agent 以 `ReplyEnd(finished_reason=INTERRUPTED)` 结束。
2. **Given** agent 无进行中回复，**When** 注入 `UserInterruptEvent`，**Then** 静默 no-op（对齐 Python "session effectively idle" 语义）。

---

### Edge Cases

- 暂停后宿主再次注入普通消息（非事件）：应报错，因为 agent 仍处于等待确认状态。
- `RequireUserConfirmEvent` 载荷里 tool_call 的 `state` MUST 为 "asking"（对齐 Python）。
- 拒绝确认后工具状态在 context 中 MUST 标记为 denied/finished（对齐 Python `ToolResultBlock` 追加）。
- 同一回复内多个需确认工具：**支持并发确认**（多个 tool_call 均 state="asking"，一次事件可携带多个），确认结果按 id 逐个匹配；同工具名的重复确认按 Python 规则去重。
- 并发确认中部分批准、部分拒绝：批准的执行、拒绝的生成 DENIED tool_result，全部处理完 agent 才继续。
- 外部执行结果与确认结果混用同一回复：状态机需明确 agent 当前等待的是确认还是外部执行，注入错误类型时报错。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `reply_stream` MUST 接受事件类输入，含 `UserConfirmResultEvent`、`UserInterruptEvent`、`ExternalExecutionResultEvent`（与 Python `_reply_impl` 的 `inputs` 统一语义一致）。
- **FR-002**: 工具触发 `PermissionBehavior::Ask` 时，引擎 MUST emit `RequireUserConfirmEvent` 后**暂停** reply_stream（结束当前流、不喂 denied 给模型、不继续循环）。
- **FR-003**: `RequireUserConfirmEvent` 的 tool_call MUST 携带 `state="asking"` 与 `suggested_rules`（Python 事件对齐）。
- **FR-004**: 注入 `UserConfirmResultEvent` 恢复时，MUST 按 `tool_call_id` 精确匹配暂停的 asking 工具执行/拒绝。
- **FR-005**: `confirmed=true` 恢复后工具 MUST 正常执行并产生 tool_result，agent 从暂停点继续 reasoning-acting。
- **FR-006**: `confirmed=false` 时工具 MUST 不执行，MUST 生成 `state=DENIED` 的 tool_result，内容含明确拒绝提示（对齐 Python `<system-reminder>`）。
- **FR-007**: agent 无等待确认时注入 `UserConfirmResultEvent` MUST 返回明确错误。
- **FR-008**: 注入确认结果携带与等待状态不匹配的 tool_call_id MUST 返回明确错误。
- **FR-009**: 确认结果可携带 `rules`，恢复时引擎 MUST 采纳为 allow 规则。
- **FR-010**: 确认事件需携带 `reply_id`，恢复时 MUST 校验其与暂停的回复匹配。
- **FR-011**: 同一回复含多个需确认工具时，`RequireUserConfirmEvent` MUST 携带全部待确认 tool_call（均 state="asking"），`UserConfirmResultEvent` MUST 支持多个 `ConfirmResult` 按 id 逐个匹配执行/拒绝。
- **FR-012**: 多个待确认工具中若 id 重复（同工具多调用），MUST 按 Python 规则去重确认。
- **FR-013**: 工具请求外部执行（`RequireExternalExecutionEvent`）时，引擎 MUST emit 该事件并**暂停** reply_stream。
- **FR-014**: 注入 `ExternalExecutionResultEvent` 恢复时，MUST 将执行结果追加到 context 并更新工具状态为 finished，agent 从暂停点继续。
- **FR-015**: 注入的 `ExternalExecutionResultEvent` 与等待状态不匹配（类型错误或 id 不匹配）MUST 返回明确错误。
- **FR-016**: 注入 `UserInterruptEvent` 时，若 agent 正在进行中或等待确认，MUST 以 `ReplyEnd(finished_reason=INTERRUPTED)` 结束；若无进行中回复则静默 no-op。

### Key Entities

- **UserConfirmResultEvent**: 宿主注入的确认事件，含 `reply_id` 与 `confirm_results: Vec<ConfirmResult>`。
- **ConfirmResult**: 单个工具确认结果，`{ confirmed, tool_call, rules? }`。
- **UserInterruptEvent**: 宿主注入的中断事件，使回复以 INTERRUPTED 结束。
- **ExternalExecutionResultEvent**: 宿主注入的外部执行结果事件，含 `execution_results: Vec<ToolResultBlock>`。
- **ToolCallState**: 工具调用状态机，`asking` 表示等待确认（暂停点）、`submitted` 表示等待外部执行结果、`finished` 表示完成。
- **RequireUserConfirmEvent**: 引擎发出的暂停信号，含待确认的 `tool_calls`（可多个）。
- **RequireExternalExecutionEvent**: 引擎发出的外部执行暂停信号，含待外部执行的 `tool_calls`。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 单元测试覆盖"暂停 → 确认 true 恢复 → 工具执行 → 继续"全链路，事件顺序与 Python 黄金快照一致。
- **SC-002**: `confirmed=false` 路径生成 `state=DENIED` tool_result，工具副作用为零（未执行）。
- **SC-003**: 非法恢复（未等待/ id 不匹配/ 类型错误）100% 返回明确错误，无静默接受。
- **SC-004**: `examples/human-in-the-loop` 改造为"暂停-确认-恢复"交互，无需截断历史/重建 agent。
- **SC-005**: 现有 `cargo test` 全量通过；原依赖"denied 喂回"行为的测试按 Python 语义更新。
- **SC-006**: 多工具并发确认（含去重、部分批准/拒绝混合）有测试覆盖，事件顺序与 Python 黄金快照一致。
- **SC-007**: `ExternalExecutionResultEvent` 与 `UserInterruptEvent` 事件输入有测试覆盖，恢复/中断行为对齐 Python。

## Assumptions

- 暂停语义为"reply_stream 结束当前流"，恢复为"再次调用 reply_stream 注入事件"——与 Python async generator 行为一致（不是同一流挂起）。
- 旧的"Ask → continue 喂 denied"行为被新的暂停语义取代（Python 对齐），不再保留旧路径。
- `ToolCallBlock.suggested_rules` 字段已存在，本次只需在事件载荷中填充。
- 事件输入改造范围：`UserConfirmResultEvent`、`UserInterruptEvent`、`ExternalExecutionResultEvent` 三类全部对齐 Python（Q2=B 决策）。
- 多工具并发确认：对齐 Python 并发/去重语义（Q1=B 决策），含并发工具逐个确认与同工具名去重。

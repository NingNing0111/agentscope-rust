# Phase 0 Research: 任务工具输出质量优化

**Feature**: 033-task-tools-optimization | **Date**: 2026-08-17

**上游兼容基线（宪法第二条）**: Python AgentScope `9d1026fa`（v2.0.5 基线，Feature 024 锁定）。本特性**不改变**兼容基线，仅在 Rust 侧优化任务工具输出文本（已批准偏差）。

**参考文件**:
- 现状实现：`crates/agent_scope_agent/src/task_tools.rs`、`streaming_reactor.rs`、`react_loop.rs`
- 现状契约：`specs/024-agent-task-planning/contracts/task-tools.md`（输出文本逐字对齐 Python）
- 兼容矩阵：`specs/001-compatibility-baseline/capability-matrix.json`（`tool-task-create/list/get/update` 条目 status=NOT_ANALYZED）
- 示例：`examples/plan-react-agent/src/main.rs`

---

## Decision 1: 输出文本偏差的范围与治理路径

**Decision**: 任务工具的成功输出文本不再逐字对齐 Python `9d1026fa`，改为 Rust 优化版（换行终止 + 报实际值 + 截断）；按宪法第一条例外路径在兼容矩阵登记偏差。

**Rationale**:
- 用户经 `/speckit-specify` 明确选择"全面优化输出"而非"保持对齐仅修流式换行"，该选择为人工批准，满足宪法第十九条治理流程。
- 偏差范围**严格限定**为"成功输出文本 + 流式展示层"：工具名、输入 Schema、状态/依赖/错误语义、数据模型零变更（FR-005）。核心行为（工具生命周期、事件序列）仍对齐，兼容等级 L2/L3 不降级（仅 `notes` 登记偏差）。
- 兼容矩阵中 `tool-task-create/list/get/update` 四条已有登记（target_level=L2，status=NOT_ANALYZED）；按 Feature 029 ResetTools 命名偏差的既有做法，在 `notes` 补记输出文本偏差与原因。

**Alternatives considered**:
- 保持逐字对齐、仅流式层补换行（spec 选项 B）：能修拼接显示，但 TaskUpdate 仍不报实际值，模型核实需求未解决；用户否决。
- 仅 TaskUpdate 报值、其余保持对齐（spec 选项 C）：拼接显示问题仍需流式层修复且 TaskGet 长描述问题未处理，用户未选。

## Decision 2: 换行终止——工具文本 + 展示层双层保障

**Decision**: 两层机制：
1. **工具层**：`task_tools.rs` 的 `text_chunk` 统一为输出的 `text` 追加尾随 `\n`。所有 4 个任务工具（Success/Error/删除/无变更路径）的完整结果文本均以 `\n` 结尾。
2. **展示层**：`streaming_reactor.rs`（`emit_tool_result_and_collect` 的 Complete 与 Stream 完成路径、`emit_denied_tool_result`）与 `react_loop.rs`（批处理路径对应发射点）在发射完整工具结果的文本增量前，若文本未以 `\n` 结尾则追加 `\n`（幂等：已以 `\n` 结尾的不重复追加）。该规则对所有工具统一生效（FR-002），覆盖非任务工具（Bash/Grep 等无尾随换行的文本）。

**Rationale**:
- 工具层保证任务工具自身文本干净；展示层防御第三方/其它工具遗漏，形成"工具自持 + 平台兜底"双层。
- 展示层补全的文本与存入上下文的文本一致（`emit_tool_result_and_collect` 返回的 `text` 同时用于 delta 发射与 `add_tool_result_to_context`），模型上下文的工具结果消息也以换行终止，利于解析。
- 只在"完整结果"上补：中断/取消路径（`Interrupted`）不补，无残留。

**Alternatives considered**:
- 仅在示例渲染层加换行：治标不治本——其它消费者（文档、未来 CLI）仍拼接；用户看到的拼接正是库层文本无换行所致。
- 仅工具层改、展示层不动：非任务工具（如 Bash 输出无尾随换行）的连续结果仍拼接，通用修复目标（FR-002）不满足。

## Decision 3: TaskUpdate 输出格式——报实际变更值

**Decision**: 输出从 `Update task (id={id}) {field1, field2}.` 改为：

```text
Updated task (id={id}): {field}={value}, {field}={value}
```

字段按处理顺序列出，每项携带实际值：

| 字段 | 值表示 |
|------|--------|
| `subject` | `subject={新标题}` |
| `description` | `description={新描述}` |
| `add_blocks` | `add_blocks=[{实际新增的 id，逗号分隔}]` |
| `add_blocked_by` | `add_blocked_by=[{实际新增的 id，逗号分隔}]` |
| `status` | `status={pending\|in_progress\|completed}` |
| `owner` | `owner={负责人}` |
| `metadata` | `metadata=[{受影响键列表}]` |

更新后状态为 `completed` 时追加（保持 024 既有文案，仅整体尾随 `\n`）：
```text
\nTask completed. Call TaskList now to find your next available task or see if your work unblocked others.
```

**Rationale**:
- 模型的核实需求：调用后需确认"哪个字段改成了什么"。逐字段报值是模型可验证的最小信息集。
- 依赖变更可见：`add_blocked_by=[4]` 明确告知模型依赖已建立——这正是示例中模型陷入混乱（创建"向用户索取 README"任务）的根因之一。
- `add_blocks`/`add_blocked_by` 需跟踪**实际新增**的 id（跳过自引用、已存在、不存在的 id），与当前"added_any bool"改法一致，仅需收集具体 id。

**Alternatives considered**:
- 保留字段名列表（Python 原样）：信息不足，模型无法核实，否决。
- 报变更前后值 `status: pending → in_progress`：信息更全但冗长；模型关心的核心是"现在是什么"，报现值足够，格式从简。

## Decision 4: TaskGet 描述截断

**Decision**: description 长度超过 200 字符时截断：

```text
Description: {前 200 字符}… (truncated, {完整长度} chars total)
```

未超过阈值输出完整描述；空描述输出空行。阈值以常量 `TASK_DESCRIPTION_MAX_CHARS = 200` 实现并文档化；达到或超过阈值即截断（边界规则：`len > 200` 截断，`len <= 200` 完整）。

**Rationale**:
- 示例中 TaskGet 原样倾倒模型自写的多段长描述，膨胀工具结果并导致展示上与后续推理粘连；截断控制体积（FR-004、SC-003）。
- 长度提示（`{完整长度} chars total`）让模型可判断截断损失，必要时可自行决定是否继续依赖已获信息。
- 截断不改变描述存储（数据模型零变更），仅影响 TaskGet 的输出文本。

**Alternatives considered**:
- 完整输出（spec 选项 B）：长描述膨胀上下文、展示仍易粘连；用户否决。
- 分页/按需加载：TaskUpdate 无分页参数，复杂度过高，超出本次优化范围。

## Decision 5: 示例渲染微调范围

**Decision**: `plan-react-agent/src/main.rs` 渲染仅做最小调整。工具结果换行问题主要由 Decision 2 的协议解决（工具文本自带尾随 `\n`），示例的 `print!` 即可正确分隔连续工具结果与后续文本。需补充的仅是事件组之间的视觉分隔（如 ToolCallEnd 输入打印后的换行），使"工具调用→输入→结果"在终端可对应。

**Rationale**: 协议修复后示例输出已大幅可读；渲染层改动越少，回归面越小。FR-009 的验收以"输出分段清晰、输入与结果可对应"为准，不追求重写渲染。

**Alternatives considered**:
- 重写示例渲染为结构化面板：超出本特性范围（示例只是演示窗口），不必要。

## Decision 6: 测试迁移与新增

**Decision**: `task_tools_tests.rs` 中所有断言精确文本的用例迁移到新输出协议（`text` 带尾随 `\n`、TaskUpdate 报值格式、TaskGet 截断格式），并新增三类断言：
1. 尾随换行：每个工具完整结果文本以 `\n` 结尾
2. 报值格式：TaskUpdate 多字段变更时输出含各字段及实际值；无变更/删除/不存在路径保持既有文案（+尾随换行）
3. 截断提示：TaskGet 超阈值/边界（==200）/正常/空描述四例

同时排查 `task_tools_e2e_tests.rs` 是否断言输出文本，若有则同步迁移。`streaming_reactor`/`react_loop` 的换行补全规则补充事件级断言（连续两个完整工具结果之间以换行分隔）。

**Rationale**: 宪法第六条要求测试驱动；输出协议变更必须由契约断言锁定，防止回归。既有测试覆盖了字段处理语义，仅文本格式部分迁移，保留状态/依赖/错误语义断言不变。

**Alternatives considered**: 无——测试迁移是协议变更的强制伴生项。

---

## 研究结论汇总

所有 Technical Context 未知项已解决，无遗留 NEEDS CLARIFICATION。实现路径明确：`text_chunk` 换行 → TaskUpdate 报值 → TaskGet 截断 → 展示层补换行（streaming + batch）→ 示例渲染微调 → 测试迁移与新增 → 兼容矩阵登记。偏差范围受控（仅成功输出文本 + 展示层），宪法合规。

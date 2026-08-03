# Contracts: 任务提醒注入（Task Reminder Injection）

**Feature**: 024-agent-task-planning | **Date**: 2026-08-03
**上游基准**: Python AgentScope `9d1026fa` `agent/_agent.py::_inject_runtime_state`（Step 3 任务维度）、`agent/_config.py::InjectionConfig`

## 触发时机

每次推理迭代开始前（batch `react_loop` 与 streaming `streaming_reactor` 两条路径一致），在 `task_tools_enabled = true` 时评估。

## 注入条件（全部满足才注入）

1. `tasks_context` 中存在 `pending` 或 `in_progress` 状态的任务；
2. 当前对话上下文中**不存在**任务工具调用痕迹——反向扫描 assistant 消息，未发现名为 `TaskCreate` / `TaskGet` / `TaskList` / `TaskUpdate` 的 ToolCallBlock；
3. 当前对话上下文中**不存在**先前的任务提醒——反向扫描 assistant 消息，未发现 `source` 等于固定来源标识且文本包含 `<tasks>` 的 HintBlock。

条件 2、3 的扫描在遇到任一命中时即可终止（感知已确认）。仅扫描 `role = assistant` 的消息（对齐 Python）。

## 注入内容

向 `state.context` **追加**一条 assistant 消息，内容块为单个 HintBlock：

| 字段 | 值 |
|------|-----|
| `source` | `{"label": "System", "sublabel": "Runtime State"}` |
| `hint` | 模板文本（见下） |
| `id` / `created_at` | 按消息模型默认生成 |

**模板**（对齐 Python `InjectionConfig.template`，`{runtime_state}` 替换为 `<tasks>` 字段）:

```text
<system-reminder>Treat the following as the ground truth at this point of the conversation. Anything stated earlier is outdated, and a later reminder, if any, supersedes this one:
<tasks>You have {in_progress} in-progress tasks and {pending} pending tasks. Use `TaskList` to view them if you don't know.</tasks>
</system-reminder>
```

## 行为约束

- **非瞬时**: 注入追加到持久上下文（对齐 Python 设计意图——让 Agent 感知时间流逝与步骤推进），不修改系统提示词（提示缓存友好，FR-009）。
- **幂等**: 同一上下文中至多存在一条有效任务提醒（先前的提醒会抑制后续注入，直至被压缩移除）。
- **零事件**: 不新增 AgentEvent 类型；注入后的上下文变化通过后续 model request trace 可观测（spec Assumptions 已记录）。
- **禁用语义**: `task_tools_enabled = false` 时本机制完全不激活。
- **锁纪律**: 评估与追加在同一个 `state` 写锁临界区内完成（读任务统计 + 扫描上下文 + 追加消息为原子操作），防止与工具执行的并发写交错产生重复注入。

## 与上下文压缩的交互

压缩替换/截断 `state.context` 后，若任务工具调用痕迹与先前提醒均被移除，下一次迭代将重新注入——这是设计意图（SC-003）。压缩逻辑本身无需修改。

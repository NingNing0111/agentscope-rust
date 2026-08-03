# Contracts: 运行时状态注入管线（Runtime State Injection）

**Feature**: 026-runtime-state-injection | **Date**: 2026-08-04
**上游基准**: Python AgentScope `9d1026fa` `agent/_agent.py::_inject_runtime_state`、`tests/agent_injection_test.py`

## 触发时机

每次推理迭代开始前（batch `react_loop` 与 streaming `streaming_reactor` 两条路径一致），当 `injection_config.inject_runtime_state == true` 时评估。时间与任务维度每轮评估；上下文用量维度仅首轮评估（Rust 局部 `cur_iter == 1`，对齐 Python `state.cur_iter == 0`）。

## 三维注入条件

三个维度各自独立评估，任一命中即注入一次；全部未命中则零注入。

### 时间维度

`now = 当前墙钟时间 @ timezone`。注入当且仅当：

1. 上下文中**无**记录时间的注入（首次回复或压缩后）；或
2. 最新记录时间无法按 `time_format` 解析；或
3. 距最新记录时间超过 `time_interval` 小时；或
4. 记录时间晚于当前时间（时钟回拨，elapsed < 0）。

记录时间的流逝计算基于记录时刻旁的 `<timezone>` 标注（`test_recorded_timezone_is_honored`），保证会话中途修改时区后计算正确。

**注入字段**（按序，进入 `joined_fields`）:

```text
<current-time>{now.strftime(time_format)}</current-time>
<timezone>{timezone}</timezone>
```

### 任务维度

注入当且仅当（**同时满足**）：

1. `tasks_context` 中存在 `pending` 或 `in_progress` 任务；且
2. 上下文中**无** `source == injection_source` 且含 `<tasks>` 的 HintBlock，也**无** `name ∈ task_tool_names` 的 ToolCallBlock（反向扫描 assistant 消息）；且
3. `inject_runtime_state && task_tools_enabled` 双开关开启。

**注入字段**:

```text
<tasks>You have {in_progress} in-progress tasks and {pending} pending tasks. Use `TaskList` to view them if you don't know.</tasks>
```

任务注入文本与来源标识与 Feature 024 既有实现**逐字一致**（兼容基线，SC-002）。

### 上下文用量维度

注入当且仅当（**同时满足**）：

1. 当前为回复首轮迭代（Rust 局部 `cur_iter == 1`）；且
2. `input_tokens > max(0, trigger_ratio - context_buffer_ratio) * context_size`。

其中 `input_tokens = model.count_tokens(hook_messages, tool_schemas)`，`trigger_ratio` 取自 `ContextConfig`，`trigger_tokens = trigger_ratio * context_size`。

**注入字段**:

```text
<context-length>Your current context contains {input_tokens} tokens. When reaching {trigger_tokens} tokens, your context will be compressed.</context-length>
```

## 组装与追加

一次注入组装**单条** HintBlock（FR-013）：

| 字段 | 值 |
|------|-----|
| `source` | `injection_config.injection_source` |
| `hint` | `template.replace("{runtime_state}", joined_fields)` |
| `id` / `created_at` | 按消息模型默认生成 |

`joined_fields` 顺序固定：`current-time → timezone → tasks → context-length → extra_fields`。使用 `replace` 而非 `format`，保留模板中其他花括号（对齐 Python `test_template_with_curly_braces_is_kept`）。追加目标为 `state.context` 尾部 assistant 消息或新消息（复用 `AgentState::append_context` 语义）。

## 事件发射

注入发生时，若 `emit_hint_event == true`，调用点发送 `AgentEvent::HintBlock(HintBlockEvent)`：

| 字段 | 值 |
|------|-----|
| `reply_id` | 当前 reply id |
| `block_id` | 注入 HintBlock 的 id |
| `source` | `injection_config.injection_source` |
| `hint` | 注入 HintBlock 的 hint 内容 |

复用既有 `agent_scope_event::HintBlockEvent`，不新增事件类型。

## 行为约束

- **非瞬时**: 注入追加到持久上下文，不修改系统提示词（提示缓存友好，FR-012）。
- **单条**: 一次调用至多追加一条 HintBlock、至多发射一个 HintBlockEvent（FR-013）。
- **幂等**: 已注入的任务提醒抑制后续任务注入，直至被压缩移除。
- **附加字段**: 附着于每次注入的同一 HintBlock，单独配置不触发注入（FR-009）。
- **总开关**: `inject_runtime_state = false` 时三维均不评估、不注入、不发事件（FR-011）。
- **锁纪律**: 评估与追加在同一个 `state` 写锁临界区内完成（读任务统计 + 扫描上下文 + 组装 + 追加原子），防止与工具执行的并发写交错产生重复注入。事件发射在写锁释放后由调用点进行。

## 与上下文压缩的交互

压缩替换/截断 `state.context` 后，各维度按自身规则重新注入：时间维度因记录时间消失而重注入；任务维度因感知证据消失而重注入；上下文维度仅在下一 reply 首轮评估。压缩逻辑本身无需修改。

## 错误语义

- 无效时区名：解析失败**回退 UTC**，不报错（对齐 Python `test_invalid_timezone_falls_back_to_utc`）。
- 记录时间解析失败：视为无记录时间，重新注入（对齐 Python `ValueError → None`）。
- 配置非法（模板缺占位符、格式不可往返、间隔为负、缓冲比例越界）：在 `AgentConfig` 构建时返回 `AgentError::InvalidConfig`，推理循环不启动。

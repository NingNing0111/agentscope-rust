# Contracts: 任务工具输出文本协议（Rust 优化版）

**Feature**: 033-task-tools-optimization | **Date**: 2026-08-17
**取代**: `specs/024-agent-task-planning/contracts/task-tools.md` 的"输出文本协议"部分（工具名、输入 Schema、行为语义、错误语义不变，仍以 024 契约为准）

**偏差声明（宪法第一/三/十八条）**: 本契约定义的任务工具成功输出文本为 **Rust 优化版**，不再逐字对齐 Python AgentScope `9d1026fa`。原因：原对齐文本不报告实际变更值、无换行终止，实测降低模型核实能力与输出可读性。偏差已获用户人工批准，并在 `capability-matrix.json` 的 `tool-task-create/list/get/update` 条目登记。

**适用范围**: 本契约覆盖 4 个任务工具（TaskCreate / TaskList / TaskGet / TaskUpdate）的成功/错误输出文本，以及流式展示层对所有完整工具结果文本的换行终止规则。

---

## 0. 公共规则：换行终止（FR-001 / FR-002）

1. **工具层**：任务工具的每个完整结果文本（Success 与 Error 各路径）一律以 `\n` 结尾。实现上由 `task_tools::text_chunk` 统一追加。
2. **展示层**：`streaming_reactor` / `react_loop` 在发射"完整"工具结果文本增量（含存入上下文）前，若文本未以 `\n` 结尾则追加 `\n`（幂等——已以 `\n` 结尾不重复追加）。该规则对**所有**工具（含非任务工具）统一生效。
3. 中断/取消路径（`ToolResultState::Interrupted`）无完整文本，不适用补全。

**效果**：同一轮连续多个工具结果在展示层各自独立成行；工具结果与紧随的模型推理文本以换行分隔；模型上下文的工具结果消息以 `\n` 终止。

---

## 1. TaskCreate

**成功输出**（state = Success）:

```text
Task (id={next_id}) created successfully: {subject}
```

- 与 024 内容一致，仅统一尾随 `\n`（无尾随换行的旧文本见 024 契约）。
- `{next_id}` 为 `tasks_context.next_sequential_id()`；`{subject}` 为参数标题。

---

## 2. TaskList

**空列表**（state = Success）:

```text
No tasks available.
```

**非空**（state = Success），每行一个任务、行间 `\n`、末尾 `\n`:

```text
{id} [{state}] {subject}{owner}[blocked by {blocked_by_csv}]
```

- `{owner}` 形如 `(alice)`，仅 owner 非空时出现
- `[blocked by ...]` 仅 blocked_by 非空时出现，id 以 `, ` 分隔
- `{state}` 为 snake_case（`pending` / `in_progress` / `completed`）
- 与 024 内容一致，仅统一尾随 `\n`

---

## 3. TaskGet

**任务不存在**（state = Error）:

```text
Task not found
```

**成功**（state = Success），逐行拼接、末尾 `\n`；可选行仅对应字段非空时出现:

```text
Task (id={id}): {subject}
Status: {state}
Description: {description 或截断描述}
Owner: {owner}                      # 仅 owner 非空
Blocked by: #{id1}, #{id2}          # 仅 blocked_by 非空，id 前缀 #
Blocks: #{id1}, #{id2}              # 仅 blocks 非空，id 前缀 #
Metadata: {py_dict_repr}            # 仅 metadata 非空
```

**描述截断规则**（阈值 `TASK_DESCRIPTION_MAX_CHARS = 200`）:

- `len(description) > 200`：

  ```text
  Description: {前 200 字符}… (truncated, {len} chars total)
  ```

- `len(description) <= 200`：`Description: {完整描述}`
- 空描述：`Description: `（空行）
- 截断仅作用于输出文本，`Task.description` 存储不变

---

## 4. TaskUpdate

**任务不存在**（state = Error）:

```text
TaskNotFoundError: The task (id={task_id}) does not exist.
```

**删除成功**（state = Success）:

```text
Task (id={task_id}) has been deleted.
```

**无任何字段更新**（state = Success）:

```text
No updates were made to the task (id={task_id}). Make sure you provided at least one field to update and the values are correct.
```

**有字段更新**（state = Success），字段按处理顺序列出，每项携带实际值，`; ` 分隔:

```text
Updated task (id={task_id}): {field}={value}; {field}={value}
```

字段-值对格式（与 `task_tools.rs` 处理顺序一致）：

| 顺序 | 字段 | 值格式 | 说明 |
|------|------|--------|------|
| 1 | `subject` | `{新标题}` | 仅非空串更新时出现 |
| 2 | `description` | `{新描述}` | 提供了即出现（含空串） |
| 3 | `add_blocks` | `[{实际新增 id，`, ` 分隔}]` | 仅实际新增的 id（跳过自引用/已存在/不存在 id） |
| 4 | `add_blocked_by` | `[{实际新增 id，`, ` 分隔}]` | 同上 |
| 5 | `status` | `{pending\|in_progress\|completed}` | `deleted` 走删除路径不在此列 |
| 6 | `owner` | `{负责人}` | 提供了即出现 |
| 7 | `metadata` | `[{受影响键，`, ` 分隔}]` | 含合并新增与删除（null）的键 |

**completed 追加段**：更新后状态为 `completed` 时，在上述文本后追加：

```text

Task completed. Call TaskList now to find your next available task or see if your work unblocked others.
```

（即原有 `\n\n` 引导段，整体以 `\n` 结尾）

**示例**（单次调用同时更新状态与依赖）:

```text
Updated task (id=1): status=in_progress; add_blocked_by=[4]
```

```text
Updated task (id=3): status=completed

Task completed. Call TaskList now to find your next available task or see if your work unblocked others.
```

---

## 5. 错误契约（不变）

| 场景 | 表现 | state |
|------|------|-------|
| task_id 不存在（TaskUpdate） | `TaskNotFoundError: The task (id=X) does not exist.` | Error |
| task_id 不存在（TaskGet） | `Task not found` | Error |
| 非法 status / 缺必填字段 / 参数类型错误 | ToolKit 层 `ToolError::InvalidInput` → 错误工具结果 | Error |
| add_blocks/add_blocked_by 引用不存在的 id | 静默忽略该引用，其余字段照常更新 | Success |

- 错误文本同样以 `\n` 结尾（公共规则）。
- 所有错误路径不得 panic、不得中断 ReAct 循环、不得损坏 AgentState。

## 6. 兼容矩阵登记要求

更新 `specs/001-compatibility-baseline/capability-matrix.json` 中 `tool-task-create` / `tool-task-list` / `tool-task-get` / `tool-task-update` 四条的 `notes`，登记：

```text
Output-text deviation (Feature 033, Art.1/Art.19 approved): success output text is Rust-optimized
(newline-terminated; TaskUpdate reports actual field values; TaskGet truncates descriptions >200 chars)
instead of verbatim Python 9d1026fa alignment. Tool names, input schemas, state/dependency/error
semantics and data model unchanged. Contract: specs/033-task-tools-optimization/contracts/task-tools-output.md.
```

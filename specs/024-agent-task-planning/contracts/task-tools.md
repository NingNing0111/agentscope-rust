# Contracts: 内置任务工具（Task Tools）

**Feature**: 024-agent-task-planning | **Date**: 2026-08-03
**上游基准**: Python AgentScope `9d1026fa` `agentscope/src/agentscope/tool/_task/`

本契约定义 4 个内置任务工具的公开表面：工具名、输入 JSON Schema、行为语义、输出文本协议与错误契约。

> **⚠️ 输出文本协议已由 Feature 033 取代（Rust 优化版）**：任务工具的成功输出文本自 `specs/033-task-tools-optimization/` 起不再逐字对齐 Python 参考实现，以 `contracts/task-tools-output.md` 为唯一基准——所有完整结果文本统一尾随 `\n`；TaskUpdate 报告实际字段值；TaskGet 截断超过 200 字符的描述（FR-006）。**工具名、输入 Schema、行为/错误语义仍以本契约为准，不变**；工具描述（description）仍逐字采用 Python。

所有工具共同属性（对齐 Python `_TaskToolBase`）：

| 属性 | 值 |
|------|-----|
| `is_concurrency_safe` | `true` |
| `is_read_only` | `false` |
| 权限 | 始终 `Allow`（PermissionEngine 内置放行名单） |
| 事件 | 复用现有 ToolCallStart/End、ToolResultStart/TextDelta/End 管线，无专属事件类型 |
| 状态访问 | 共享 `Arc<RwLock<AgentState>>`，仅操作 `tasks_context`，短锁临界区 |

---

## 1. TaskCreate

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "subject":     { "type": "string", "description": "A brief title for the task" },
    "description": { "type": "string", "description": "What needs to be done" },
    "metadata":    { "type": ["object", "null"], "description": "Arbitrary metadata to attach to the task" }
  },
  "required": ["subject", "description"]
}
```

**行为**:
1. 计算 `next_id = tasks_context.next_sequential_id()`（现存数值 id 最大值 +1，忽略非数值 id）。
2. 构造 Task：`id = next_id`，`state = pending`，`created_at = 当前 RFC3339`，`owner = None`，`blocks/blocked_by = []`，`metadata = 参数或 {}`。
3. 追加到 `tasks_context.tasks`。

**成功输出**（state = Success，文本以 `\n` 结尾，Feature 033）:

```text
Task (id={next_id}) created successfully: {subject}
```

**工具描述（description）**: 逐字采用 Python `TaskCreate.description`（`tool/_task/_create_task.py`），含 "When to Use / When NOT to Use / Task Fields / Tips" 四节，引导模型在复杂多步骤场景使用、琐碎场景不用。

---

## 2. TaskList

**Input Schema**:

```json
{ "type": "object", "properties": {} }
```

**行为**: 无参数。遍历 `tasks_context.tasks` 输出摘要列表。

**输出**（均以 `\n` 结尾，Feature 033）:

- 空列表（state = Success）：

```text
No tasks available.
```

- 非空（state = Success），每行一个任务：

```text
{id} [{state}] {subject}({owner})[blocked by {blocked_by_csv}]
```

其中 `({owner})` 仅当 owner 非空时出现；`[blocked by ...]` 仅当 blocked_by 非空时出现，id 列表以 `, ` 分隔。`{state}` 为 snake_case（`pending` / `in_progress` / `completed`）。

**工具描述**: 逐字采用 Python `TaskList.description`（`tool/_task/_list_task.py`）。

---

## 3. TaskGet

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "task_id": { "type": "string", "description": "The ID of the task to retrieve" }
  },
  "required": ["task_id"]
}
```

**行为**: 按 id 查找任务。

**成功输出**（state = Success，文本以 `\n` 结尾，Feature 033），逐行拼接，可选行仅在对应字段非空时出现；`Description` 行超过 200 字符时截断为 `{前 200 字符}… (truncated, {len} chars total)`（FR-004，完整规则见 `task-tools-output.md` §3）：

```text
Task (id={id}): {subject}
Status: {state}
Description: {description}
Owner: {owner}                      ← 仅 owner 非空
Blocked by: #{id1}, #{id2}          ← 仅 blocked_by 非空，id 前缀 #
Blocks: #{id1}, #{id2}              ← 仅 blocks 非空，id 前缀 #
Metadata: {metadata}                ← 仅 metadata 非空，Debug/JSON 表示
```

**错误输出**（state = Error，文本以 `\n` 结尾）:

```text
Task not found
```

**工具描述**: 逐字采用 Python `TaskGet.description`（`tool/_task/_get_task.py`）。

---

## 4. TaskUpdate

**Input Schema**:

```json
{
  "type": "object",
  "properties": {
    "task_id":        { "type": "string", "description": "The task id." },
    "subject":        { "type": ["string", "null"], "description": "New subject for the task" },
    "description":    { "type": ["string", "null"], "description": "New description for the task" },
    "add_blocks":     { "type": ["array", "null"], "items": { "type": "string" }, "description": "Task IDs that this task blocks" },
    "status":         { "type": ["string", "null"], "enum": ["pending", "in_progress", "completed", "deleted"], "description": "New status for the task" },
    "add_blocked_by": { "type": ["array", "null"], "items": { "type": "string" }, "description": "Task IDs that block this task" },
    "owner":          { "type": ["string", "null"], "description": "New owner for the task" },
    "metadata":       { "type": ["object", "null"], "description": "Metadata keys to merge into the task. Set a key to null to delete it." }
  },
  "required": ["task_id"]
}
```

**行为**（严格按以下顺序处理字段，与 Python 一致）:

1. 按 `task_id` 查找；不存在 → 返回错误（见下）。
2. `subject`：非空字符串才更新（空串视为未提供）。
3. `description`：提供了（含空串）即更新。
4. `add_blocks`：过滤出"当前 blocks 中不存在且任务集合中存在"的 id，逐个调用 `update_block_relation(task_id, block_id)` 双向写入。
5. `add_blocked_by`：同上过滤，逐个调用 `update_block_relation(blocked_by_id, task_id)`。
6. `status`：
   - `deleted` → 立即从集合移除该任务，并清理所有任务 blocks/blocked_by 中对其的引用；直接返回删除输出，不再处理后续字段。
   - 其他 → 更新 `state`。
7. `owner`：提供了即更新。
8. `metadata`：逐键合并；值为 `null` 的键从 metadata 中删除。

**输出**:

- 任务不存在（state = Error）:

```text
TaskNotFoundError: The task (id={task_id}) does not exist.
```

- 删除成功（state = Success，文本以 `\n` 结尾）:

```text
Task (id={task_id}) has been deleted.
```

- 有字段被更新（state = Success，Feature 033 报实际值、文本以 `\n` 结尾）:

```text
Updated task (id={task_id}): {field}={value}; {field}={value}
```

字段-值对按处理顺序记录（`subject`, `description`, `add_blocks`, `add_blocked_by`, `status`, `owner`, `metadata`），每项报告实际应用值：状态报新值（`pending`/`in_progress`/`completed`）、`add_blocks`/`add_blocked_by` 报实际新增 id 列表（`[1, 2]`）、`owner`/`subject`/`description` 报新值、`metadata` 报受影响键（含置 `null` 删除的键）；完整格式见 `task-tools-output.md` §4。若更新后状态为 `completed`，追加：

```text


Task completed. Call TaskList now to find your next available task or see if your work unblocked others.
```

- 无任何字段更新（state = Success，文本以 `\n` 结尾）:

```text
No updates were made to the task (id={task_id}). Make sure you provided at least one field to update and the values are correct.
```

**工具描述**: 逐字采用 Python `TaskUpdate.description`（`tool/_task/_update_task.py`），含状态流转纪律说明。

---

## 错误契约汇总

| 场景 | 表现 | state |
|------|------|-------|
| task_id 不存在（TaskUpdate） | `TaskNotFoundError: The task (id=X) does not exist.` | Error |
| task_id 不存在（TaskGet） | `Task not found` | Error |
| 非法 status 值 / 缺必填字段 / 参数类型错误 | ToolKit 层 `ToolError::InvalidInput` 转为错误工具结果回流模型 | Error |
| 空任务列表（TaskList） | `No tasks available.` | Success（非错误） |
| add_blocks/add_blocked_by 引用不存在的 id | 静默忽略该引用，其余字段照常更新 | Success |

所有错误路径不得 panic、不得中断 ReAct 循环、不得损坏 AgentState（FR-013、SC-005）。

## 权限契约

PermissionEngine 在规则评估前检查内置放行名单 `{TaskCreate, TaskList, TaskGet, TaskUpdate}`，命中即返回：

```text
PermissionDecision::Allow, message = "{tool_name} is always allowed to be called."
```

## 配置契约

`AgentConfig` 新增字段：

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `task_tools_enabled` | `bool` | `true` | `true` 时 ReActAgent 构造期自动注册 4 个任务工具并启用任务提醒注入；`false` 时完全不注册、不注入 |

Builder 方法：`.task_tools_enabled(bool)`。既有构造代码不传该字段时行为 = 启用（破坏性仅为新增默认工具，spec 已批准）。

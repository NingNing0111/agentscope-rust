# Phase 1 Data Model: 任务工具输出质量优化

**Feature**: 033-task-tools-optimization | **Date**: 2026-08-17
**上游基准**: Python AgentScope `9d1026fa`（数据模型对齐，Feature 024 交付，**本特性零变更**）

## 实体总览

```text
AgentState (已存在, agent_scope_state, 零变更)
└── tasks_context: TaskContext (已存在, 零变更)
    └── tasks: Vec<Task> (已存在, 零变更)
        ├── id: String
        ├── subject: String
        ├── description: String
        ├── metadata: Map<String, JsonValue>
        ├── created_at: String
        ├── state: TaskState
        ├── owner: Option<String>
        ├── blocks: Vec<String>
        └── blocked_by: Vec<String>

AgentState.context: Vec<Msg>            # 工具结果消息的文本携带尾随 \n（展示层补全）
```

## 核心声明：数据模型零变更

本特性的所有改动**不触及持久化数据模型**：

| 模型 | 现状 | 本特性 |
|------|------|--------|
| `Task`（agent_scope_state/src/task.rs） | 字段与 serde 布局对齐 Python，Feature 024 交付 | **零变更**——字段、serde 属性、状态机、序列化布局不变 |
| `TaskContext` | 方法集完整（add/get/delete/update_block_relation/next_sequential_id） | **零变更** |
| `AgentState.tasks_context` | `tasks_context: TaskContext` 随会话持久化 | **零变更** |

依据：FR-005 / SC-006 要求对外 API 与数据协议向后兼容 Feature 024。会话存档无需迁移，`Task`/`TaskContext` 的 JSON 往返不受影响（宪法第十二条）。

## 输出文本层的数据约束（非持久化）

本特性的"数据"变化集中在工具结果的**输出文本协议**，不在存储模型。定义如下约束：

1. **完整结果文本以 `\n` 终止**（FR-001/FR-002）：
   - 任务工具经 `text_chunk` 生成的 `text` 统一以 `\n` 结尾（含 Success / Error / 删除 / 无变更各路径）
   - 流式/批处理层对任意完整工具结果文本若未以 `\n` 结尾则补 `\n`（幂等）
   - 该文本同时进入 `ToolResultTextDelta` 事件与 `add_tool_result_to_context` 存储的工具结果消息

2. **TaskUpdate 输出携带实际变更值**（FR-003）：
   - 文本形如 `Updated task (id={id}): {field}={value}, ...`
   - 值来自变更时收集的"字段名 + 实际生效值"对（非仅字段名）
   - 数据约束：`add_blocks` / `add_blocked_by` 只报告**实际新增**的 id（跳过自引用、已存在、不存在的 id）；`metadata` 只列受影响键

3. **TaskGet 描述截断**（FR-004）：
   - 阈值常量 `TASK_DESCRIPTION_MAX_CHARS = 200`
   - `len > 200` → 输出 `{前 200 字符}… (truncated, {len} chars total)`
   - `len <= 200` → 输出完整描述；空描述输出空行
   - 截断仅作用于输出文本，`Task.description` 存储原样保留

## 不变式（继承 Feature 024，不受本特性影响）

- 双向一致性：`A.blocks` 含 `B` ⟺ `B.blocked_by` 含 `A`（仅经 `update_block_relation` / `delete_task` 维护）
- 引用悬空防护：删除清理全部引用；建依赖忽略不存在的 id
- id 唯一性：顺序分配 + UUID 兼容路径共同保证
- 会话序列化布局不变：`"tasks_context": {"tasks": [...]}`

## 序列化兼容性

- 数据层零变更 → 既有会话存档 100% 兼容，无需迁移（宪法第十二条）
- 工具结果文本携带尾随 `\n` 属于消息层文本内容变化，不改变消息结构（`ContentBlock::ToolResult` 序列化结构不变）
- 兼容矩阵登记的是**输出文本偏差**（Rust 优化版 vs Python 逐字版），非数据协议变更（`capability-matrix.json` `tool-task-*` 条目 `notes`）

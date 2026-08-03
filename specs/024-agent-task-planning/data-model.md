# Phase 1 Data Model: Agent 任务规划重构（内置任务规划工具）

**Feature**: 024-agent-task-planning | **Date**: 2026-08-03
**上游基准**: Python AgentScope `9d1026fa`（`state/_task.py`、`state/_state.py`、`tool/_task/`）

## 实体总览

```text
AgentState (已存在, agent_scope_state)
└── tasks_context: TaskContext (已存在, 本特性扩展方法)
    └── tasks: Vec<Task>
        ├── id: String            — 顺序数值 id（TaskCreate 赋值）或 UUID（Task::new 兼容路径）
        ├── subject: String
        ├── description: String
        ├── metadata: Map<String, JsonValue>
        ├── created_at: String    — RFC3339 时间戳
        ├── state: TaskState      — pending | in_progress | completed
        ├── owner: Option<String>
        ├── blocks: Vec<String>   — 本任务阻塞的其他任务 id
        └── blocked_by: Vec<String> — 阻塞本任务的其他任务 id
```

## Task（已存在，不修改字段）

位置：`crates/agent_scope_state/src/task.rs`

| 字段 | 类型 | serde | 说明 |
|------|------|-------|------|
| `subject` | `String` | 必填 | 任务标题（祈使句） |
| `description` | `String` | 必填 | 任务详细描述 |
| `metadata` | `HashMap<String, Value>` | `default` | 任意键值元数据 |
| `created_at` | `String` | `default = now(RFC3339)` | 创建时间 |
| `state` | `TaskState` | `default = pending` | 生命周期状态 |
| `id` | `String` | `default = uuid` | 唯一标识；TaskCreate 工具显式覆盖为顺序数值 |
| `owner` | `Option<String>` | `default`, `skip_serializing_if none` | 负责人 |
| `blocks` | `Vec<String>` | `default` | 正向阻塞引用 |
| `blocked_by` | `Vec<String>` | `default` | 反向阻塞引用 |

**状态机**:

```text
pending ──→ in_progress ──→ completed
   │             │
   └──────┬──────┘
          ▼
       deleted（立即从集合移除，非状态值；同时清理所有 blocks/blocked_by 引用）
```

- Python 不校验状态回退（in_progress → pending 允许），Rust 保持同样宽松语义：除 `deleted` 触发的移除外，任意 `pending|in_progress|completed` 赋值均合法。
- `deleted` 仅存在于 TaskUpdate 工具的输入枚举，不进入 `TaskState`（避免持久化出现 Python 不存在的数据形态）。

## TaskContext（扩展方法，字段不变）

位置：`crates/agent_scope_state/src/task.rs`

| 方法 | 签名 | 语义（对齐 Python） |
|------|------|---------------------|
| `next_sequential_id` | `(&self) -> String` | 现有任务 id 中可解析为 `u64` 的最大值 +1；非数值 id 忽略；空集合返回 `"1"` |
| `delete_task` | `(&mut self, id) -> bool` | 移除任务；遍历其余任务，从其 `blocks` / `blocked_by` 中移除该 id；返回是否找到并删除 |
| `update_block_relation` | `(&mut self, block_id, blocked_by_id)` | 双向同步：`block_id.blocks += blocked_by_id`（去重）、`blocked_by_id.blocked_by += block_id`（去重）；任一 id 不存在则对应方向的写入跳过 |
| 既有 `add_task` / `get_task` / `get_task_mut` / `update_task_state` / `tasks_by_state` / `tasks_by_owner` | — | 保持不变 |

**不变量**:
- 双向一致性：若 `A.blocks` 含 `B`，则 `B.blocked_by` 必含 `A`（仅通过 `update_block_relation` 与 `delete_task` 维护，工具层不得直写字段）。
- 引用悬空防护：`delete_task` 清理全部引用；`update_block_relation` 忽略不存在的 id。
- id 唯一性由 TaskCreate 的顺序分配 + UUID 兼容路径共同保证（顺序分配基于现存最大值，不与 UUID 冲突——UUID 非数值，不参与递增）。

## AgentState（不修改字段，仅共享方式变化）

位置：`crates/agent_scope_state/src/agent_state.rs:185` — `pub tasks_context: TaskContext` 已存在。

- 序列化布局不变（`"tasks_context": {"tasks": [...]}`），会话保存/加载自动覆盖任务字段（满足 FR-007、SC-002）。
- `agent_scope_agent` 侧持有方式从 `RwLock<AgentState>` 变为 `Arc<RwLock<AgentState>>`（见 research.md Decision 1），不改变 state crate 本身。

## TaskUpdate 工具输入模型（新增，工具参数层）

位置：`agent_scope_agent::task_tools`（crate 内部，serde 反序列化用）

```text
TaskUpdateParams {
  task_id: String                — 必填
  subject: Option<String>        — 空字符串视为未提供（Python truthy 语义）
  description: Option<String>    — 提供即更新（含空字符串）
  add_blocks: Option<Vec<String>>
  status: Option<TaskUpdateStatusInput>  — pending | in_progress | completed | deleted
  add_blocked_by: Option<Vec<String>>
  owner: Option<String>
  metadata: Option<Map<String, Value>>   — 合并语义；值为 null 的键删除
}
```

- 非法 `status` 值 → serde 反序列化失败 → ToolKit 层 `ToolError::InvalidInput` → 错误工具结果回流模型（FR-013）。

## 任务提醒注入块（复用既有消息模型）

- `ContentBlock::Hint(HintBlock)`（`agent_scope_message`，block.rs:113）——无字段变更。
- 注入实例：`source = Some(r#"{"label": "System", "sublabel": "Runtime State"}"#)`，`hint = HintContent::Text(模板文本)`，宿主消息 role = assistant。
- 模板（对齐 Python InjectionConfig.template，`{runtime_state}` 替换为 `<tasks>...</tasks>`）:

```text
<system-reminder>Treat the following as the ground truth at this point of the conversation. Anything stated earlier is outdated, and a later reminder, if any, supersedes this one:
<tasks>You have {in_progress} in-progress tasks and {pending} pending tasks. Use `TaskList` to view them if you don't know.</tasks>
</system-reminder>
```

## 序列化兼容性

- `Task` / `TaskContext` 全部新增方法不影响 serde；字段零变更 → 既有会话存档兼容（宪法第十二条）。
- 往返测试要求：`Task` 全字段（含双向引用、元数据、owner）JSON round-trip 100% 保留（SC-002）。
- 未知字段容忍：`Task` 当前无 `deny_unknown_fields`，上游新增字段不导致反序列化失败，保持现状。

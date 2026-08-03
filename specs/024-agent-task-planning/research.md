# Phase 0 Research: Agent 任务规划重构（内置任务规划工具）

**Feature**: 024-agent-task-planning | **Date**: 2026-08-03

**上游兼容基线（宪法第二条）**: Python AgentScope commit `9d1026fad17e6a985873c0981bb8d4aeacf98cf9`（tag base `v2.0.5`，2026-08-01），本地 checkout 位于仓库根目录 `agentscope/`。关键参考文件：

- `agentscope/src/agentscope/tool/_task/` — TaskCreate / TaskList / TaskGet / TaskUpdate 四个内置工具
- `agentscope/src/agentscope/state/_task.py` — Task 数据模型
- `agentscope/src/agentscope/state/_state.py` — TaskContext / AgentState
- `agentscope/src/agentscope/agent/_agent.py` `_inject_runtime_state` — 任务提醒注入
- `agentscope/src/agentscope/agent/_config.py` — InjectionConfig（task_tool_names、模板、来源标识）

---

## Decision 1: 任务工具如何访问 Agent 状态（状态注入机制）

**Decision**: 任务工具存放于 `agent_scope_agent` crate，在 `ReActAgent::new` 构造时自动注册到 ToolKit；工具通过共享 `Arc<RwLock<AgentState>>` 读写 `tasks_context`。`AgentInner.state` 的类型从 `RwLock<AgentState>` 改为 `Arc<RwLock<AgentState>>`。

**Rationale**:
- Python 通过 `_agent_state` 参数注入（`is_state_injected: True`）。Rust 的 `Tool::call(&self, input)` 签名无上下文参数，扩展签名会破坏所有既有工具实现与公开 API（Feature 006 已发布）。
- 共享句柄方案对 `Tool` trait 零侵入，符合宪法第八条（Rust 原生：`Arc` 管理共享所有权）。
- 已验证死锁安全：`react_loop.rs` 在 `tk.call_tool(&tc_mut).await`（react_loop.rs:423）期间不持有 state 锁，所有 `state.write()/read()` 均为短临界区（react_loop.rs:158, 330, 355, 484, 520）。任务工具同样只取短锁，不会自死锁。
- 任务工具需要 `AgentState`（来自 `agent_scope_state`）和 `Tool`/`ToolKit`（来自 `agent_scope_tool`），只有 `agent_scope_agent` 同时依赖两者，且分层方向合规（agent → tool/state，无反向依赖，符合宪法第十一条）。

**Alternatives considered**:
- 扩展 `Tool::call` 增加上下文参数（Python 式 `is_state_injected`）：破坏 Feature 006 公开 API，所有既有工具需改签名，拒绝。
- `AgentState` 内嵌 `Arc<RwLock<TaskContext>>`：破坏现有 serde 布局（`tasks_context` 已是直接字段，agent_state.rs:185），需自定义序列化，复杂度高于收益，拒绝。
- 工具放 `agent_scope_tool` crate：tool crate 会反向依赖 state crate 的 AgentState，污染工具层抽象，拒绝。

## Decision 2: 任务工具注册与配置开关

**Decision**: `AgentConfig` 新增 `task_tools_enabled: bool`（默认 `true`），builder 提供 `.task_tools_enabled(bool)`。`ReActAgent::new` 接收 `mut config`，构造时若启用则：先建 `Arc<RwLock<AgentState>>`，构造 4 个任务工具并注册进 toolkit（`None` 时新建默认 ToolKit）。同一开关同时控制任务提醒注入；禁用后行为完全退化为纯 ReAct 循环。

**Rationale**:
- 满足 SC-004（默认零配置获得规划能力，禁用只需 1 个配置项）。
- Python 在 app 服务层注册任务工具（`app/_service/_toolkit.py:136`），而 Rust 无对应服务层；在 Agent 构造期注册是最贴近"内置能力"定位的等价点。
- 注册发生在构造期而非每次 reply，保证工具 schema 稳定、权限检查路径统一。

**Alternatives considered**:
- 由用户手动注册工具：违背"内置"定位，且用户手动注册无法获得共享 state 句柄（state 在 `ReActAgent::new` 内部创建），拒绝。
- 独立 `TaskToolsConfig` 结构承载模板/来源标识等：Python 的 InjectionConfig 含时间/上下文维度，本特性仅落地任务维度，暂不需要独立结构；常量先行，后续特性引入时间维度时再提取为 config（记录为后续扩展点）。

## Decision 3: 任务数据模型与 TaskContext 扩展

**Decision**: 复用 `agent_scope_state::task` 已有的 `Task` / `TaskState` / `TaskContext`（字段已与 Python 对齐），做如下扩展：

1. `TaskContext::next_sequential_id()` — 遍历现有任务，取可解析为整数的 id 的最大值 +1，忽略非数值 id（对齐 Python TaskCreate 逻辑）。
2. `TaskContext::delete_task(id)` — 移除任务并清理所有其他任务 blocks/blocked_by 中对该 id 的引用。
3. `TaskContext::update_block_relation(block_id, blocked_by_id)` — 双向同步阻塞关系；引用不存在的任务 id 时忽略。
4. `Task` 增加 `Deleted` 不变体 —— 不增加。Python 中 `deleted` 是立即删除而非状态，`TaskState` 枚举保持 Pending/InProgress/Completed 三态；删除输入通过工具参数层的独立枚举（`TaskUpdateStatusInput: pending|in_progress|completed|deleted`）表达。

**Rationale**:
- 现有 `Task` 结构（task.rs:27-45）字段与 Python `_task.py` 完全对齐（subject/description/metadata/created_at/state/id/owner/blocks/blocked_by），且 serde 默认值齐全（`#[serde(default)]`），符合宪法第十二条。
- `Task::new` 保留 UUID 默认 id（向后兼容既有构造路径），顺序 id 由 TaskCreate 工具显式赋值——两条构造路径互不干扰。
- 删除即移除（而非墓碑状态）与 Python 行为一致，序列化结果无 deleted 态残留。

**Alternatives considered**:
- 给 `TaskState` 加 `Deleted` 变体：会在持久化状态中产生 Python 不存在的数据形态，且删除后仍需清理引用，两步合一更简单，拒绝。
- 重写 task.rs 全新模型：已有模型字段级对齐 Python，重写是无谓 churn，拒绝。

## Decision 4: 任务提醒注入（runtime state injection 的任务维度）

**Decision**: 新增 `agent_scope_agent::task_reminder` 模块，提供 `maybe_inject_task_reminder(state, agent_name) -> bool`，在 batch（react_loop.rs）与 streaming（streaming_reactor.rs）两条循环的每次推理迭代开始时调用。逻辑对齐 Python `_inject_runtime_state` 的 Step 3：

1. 统计 `tasks_context` 中 pending / in_progress 数量；均为 0 → 不注入。
2. 反向扫描 `state.context` 中 assistant 消息：遇到任务工具名（`TaskCreate`/`TaskGet`/`TaskList`/`TaskUpdate`）的 ToolCallBlock，或来源标识匹配的 HintBlock 且文本含 `<tasks>` → 判定已感知，不注入。
3. 否则向 `state.context` 追加一条 assistant 消息，内容为 HintBlock：`source = {"label": "System", "sublabel": "Runtime State"}`，hint 文本套用 Python 模板（`<system-reminder>...<tasks>You have N in-progress tasks and M pending tasks. Use \`TaskList\` to view them if you don't know.</tasks>...</system-reminder>`）。

**Rationale**:
- HintBlock 已存在于消息模型（block.rs:113-125，含 `source: Option<String>`），序列化协议无需变更。
- 注入为追加持久上下文（非系统提示词修改），满足 FR-009 的提示缓存友好要求。
- 感知检测复用同一固定来源标识，压缩后工具调用痕迹消失时提醒会重新注入，满足 SC-003。
- Python 每轮迭代都评估注入（函数在推理流程中逐迭代调用），Rust 保持同频率；由于"已感知则跳过"，实际注入至多一次直到压缩。

**Alternatives considered**:
- 仅在 reply 第一轮（cur_iter == 0）检查：若任务在第一轮之后创建则永远不会触发提醒判定偏差——虽然感知检测通常也会抑制，但 Python 语义是每轮评估，偏离基准无收益，拒绝。
- 注入时间/上下文用量维度：spec 已声明为后续特性（Assumptions），不做。
- 新增专用 HintEvent 事件类型：spec 明确不作为验收门槛；复用现有事件管线，不新增事件类型（如需可观测，注入后上下文变化本身体现在后续 model request trace 中）。

## Decision 5: 权限始终放行

**Decision**: 在 `PermissionEngine::check_decision`（permission.rs:254）的规则评估前增加内置放行名单：4 个任务工具名命中时直接返回 `PermissionDecision::Allow`（message 对齐 Python：`"<name> is always allowed to be called."`）。

**Rationale**:
- Python `_TaskToolBase.check_permissions` 无条件 ALLOW（`_task_tool_base.py`）。
- 任务工具只读写 Agent 自身状态，无外部副作用，属于框架内部安全操作；若走默认审批流会打断 ReAct 循环（FR-011）。
- 放行名单写在 PermissionEngine 内部而非 react_loop 特判，保证 batch/streaming 两条路径及未来其他调用方行为一致。

**Alternatives considered**:
- 在 react_loop 中按工具名跳过权限检查：分散特判逻辑、两条循环路径各一份，易漂移，拒绝。
- 要求用户配置 allow 规则：违背"默认零配置"目标（SC-004），拒绝。

## Decision 6: Planner 移除范围

**Decision**: 完整移除以下产物（破坏性变更，spec Assumptions 已批准）：

- 源码：`agent_scope_agent/src/{plan.rs, planner.rs, planner_error.rs, planner_stream.rs, planning_trace.rs}` 及 lib.rs 对应 `pub mod` 与 re-export。
- 测试：`agent_scope_agent/tests/planner_*.rs` 共 11 个文件（含 planner_mocks.rs）。
- 事件 fixture 清理：`agent_scope_event/tests/event_serde_tests.rs` 的 planner.lifecycle 用例改为其他 Custom 事件名或删除；`agent_scope_message/tests/append_event_tests.rs` 中 `source: "planner"` 的用例改名（该用例测的是 HintBlock source 透传，与 planner 无语义耦合）。
- 文档：`docs/zh/modules/agent.md`、`docs/en/modules/agent.md` 中的 planner 章节重写为任务工具说明。
- `AgentEvent::Custom` 事件类型本身保留（通用扩展点，非 planner 专属）。

**Rationale**:
- 宪法第五条：不允许伪兼容——不留 deprecated stub，彻底移除。
- `specs/021-planner-react-agent/` 目录作为历史规格保留（spec 是历史记录，不是活文档）。
- 已确认 examples/ 下无 planner 引用（仅 pi-rust，不使用 planner）。

**Alternatives considered**:
- 保留 plan.rs 数据模型供任务工具复用：plan.rs 的 Plan/PlanStep 模型（PlanStatus 七态、PlanRevision 等）与 Python Task 模型语义不同，复用会制造两套任务概念，拒绝；Task 模型以 state crate 现有 task.rs 为准。
- 标记 deprecated 保留一个版本：项目处于 1.0 前快速演进期，无下游稳定承诺，维护双轨成本高，拒绝（spec Assumptions 已记录）。

## Decision 7: 工具错误与输出文本协议

**Decision**: 任务工具的用户级错误（任务不存在、空任务列表等）以 `ToolResultBlock` + `ToolResultState::Error` 返回给模型自我纠正；参数反序列化失败（非法 status 值、缺必填字段）由 ToolKit 层转为 `ToolError::InvalidInput` 并同样以错误工具结果回流循环。输出文本逐字对齐 Python（见 `contracts/task-tools.md`）。

**Rationale**:
- 满足 FR-013 与 SC-005（零崩溃、100% 结构化错误结果）。
- 文本级对齐使黄金快照/差分测试可逐字符比较（宪法第三条、第七条）。

**Alternatives considered**:
- 自定义更"Rust 风格"的输出措辞：偏离行为基准，差分测试无法通过，拒绝。

## Decision 8: 中断与并发行为

**Decision**: 任务工具为同步内存操作（Vec 上 O(n)），`is_concurrency_safe = true`（对齐 Python `_TaskToolBase`）；不引入新的 spawn/channel。中断时任务状态保持中断时刻一致态（工具执行是原子的短锁写，不存在半完成状态）。

**Rationale**: 宪法第十条（结构化并发）——无新后台任务即无新生命周期负担；ReAct 循环内工具串行执行，锁竞争仅发生在工具与循环的短临界区之间。

---

## 研究结论汇总

所有 Technical Context 未知项已解决，无遗留 NEEDS CLARIFICATION。实现路径明确：state 扩展 → 任务工具 → 构造期注册 → 提醒注入 → 权限放行 → planner 移除 → 测试与文档迁移。

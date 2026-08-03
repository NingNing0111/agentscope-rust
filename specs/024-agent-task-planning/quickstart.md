# Quickstart: Agent 任务规划重构（内置任务规划工具）验证指南

**Feature**: 024-agent-task-planning | **Date**: 2026-08-03

本文档定义端到端验证场景，证明特性按 spec 工作。实现细节见 `research.md`、`data-model.md`、`contracts/`。

## 前置条件

```bash
cd /Users/pgthinker/StudyCode/GithubProject/agentscope-rust
cargo --version   # stable toolchain
```

无需真实 LLM——所有核心场景使用 Scripted/Mock Model（宪法第六条）。

## 场景 1：任务工具端到端（对应 US1 / FR-001~006, FR-012）

Scripted Model 按脚本依次产生：`TaskCreate` ×2 → `TaskUpdate`（建立依赖）→ `TaskList` → `TaskUpdate`（in_progress → completed）→ 最终文本答复。

```bash
rtk cargo test -p agent_scope_agent task_tools
```

**预期**:
- 工具经 ToolKit 正常调用，事件管线出现完整 ToolCallStart/End + ToolResult 序列
- `agent.try_state().tasks_context.tasks` 含 2 个任务，id 为 `"1"`、`"2"`（顺序数值）
- 依赖双向一致：任务 2 的 `blocked_by` 含 `"1"`，任务 1 的 `blocks` 含 `"2"`
- 完成后任务 1 状态为 `completed`，工具输出文本与 `contracts/task-tools.md` 逐字一致

## 场景 2：错误输入自愈（对应 FR-013 / SC-005）

Scripted Model 依次产生：`TaskUpdate`（不存在的 id）→ `TaskGet`（不存在的 id）→ 非法 `status` 值 → 正常收尾。

**预期**:
- 前两次调用返回 Error 状态工具结果，文本分别为 `TaskNotFoundError: ...` 与 `Task not found`
- 非法 status 经 InvalidInput 转为错误工具结果
- ReAct 循环不中断、不 panic，Agent 产出最终答复

## 场景 3：删除与引用清理（对应 FR-006 / Edge Cases）

预置 3 个任务（1←2←3 依赖链），Scripted Model 删除任务 2。

**预期**: 任务 2 从集合移除；任务 1 的 `blocks`、任务 3 的 `blocked_by` 中 `"2"` 均被清理；输出 `Task (id=2) has been deleted.`

## 场景 4：会话持久化往返（对应 US2 / FR-007 / SC-002）

创建含任务（含 owner、metadata、双向依赖）的 Agent → Session 保存 → 重新加载。

```bash
rtk cargo test -p agent_scope_state
```

**预期**: 所有任务字段 100% 保留；`tasks_context` JSON 布局与既有格式一致（向后兼容）。

## 场景 5：压缩后任务提醒（对应 US2 / FR-008~009 / SC-003）

构造含未完成任务、但上下文无任务工具痕迹的 AgentState → 触发一次 reply 迭代。

```bash
rtk cargo test -p agent_scope_agent task_reminder
```

**预期**:
- 上下文中追加一条 assistant 消息，含 `source = {"label": "System", "sublabel": "Runtime State"}` 的 HintBlock，文本含 `<tasks>You have N in-progress tasks and M pending tasks...`
- 再次迭代（提醒已在上下文）→ 不重复注入
- 上下文含任务工具调用痕迹 → 不注入
- 全部任务完成 → 不注入

## 场景 6：默认启用与禁用（对应 FR-010 / SC-004）

**预期**:
- 默认构造的 ReActAgent：toolkit schema 列表含 4 个任务工具
- `.task_tools_enabled(false)` 构造：schema 不含任务工具；场景 5 条件下不注入提醒
- 权限模式为 Ask 时，任务工具调用不触发审批（PermissionDecision::Allow）

## 场景 7：Planner 移除（对应 US3 / FR-014 / SC-006）

```bash
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo fmt --check
```

**预期**:
- `agent_scope_agent` 公开 API 中不存在 `Planner` / `PlannerConfig` / `Plan` / `PlanStep` / `PlanningTrace` / `PlannerError` 等导出（`grep -r "pub use.*[Pp]lanner\\|pub mod plan" crates/agent_scope_agent/src/lib.rs` 无结果）
- `crates/agent_scope_agent/src/{plan,planner,planner_error,planner_stream,planning_trace}.rs` 与 `tests/planner_*.rs` 已删除
- `docs/zh/modules/agent.md`、`docs/en/modules/agent.md` 无 planner 章节，含任务工具说明
- 除被移除的 planner 测试外，全 workspace 测试通过、clippy 零警告、fmt 通过

## 回归基线

```bash
rtk cargo test --workspace   # 既有 706+ 测试（减去 planner 测试数）全部通过
```

完成定义参照宪法第十七条 checklist（单元测试、无静默降级、文档更新、示例可编译、clippy/fmt 通过、无未登记 UnsupportedFeature）。

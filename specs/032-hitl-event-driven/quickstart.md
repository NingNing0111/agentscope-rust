# Feature 032 快速验证指南

**Feature**: 事件驱动 HITL 确认机制与 Python 对齐

## 前置条件

- Rust 工具链（cargo）
- 仓库已 checkout，`agentscope/`（Python 参考）存在
- 无真实模型调用需求：验证用 Mock Model（确定性）

## 验证场景 1: 单工具暂停 → 确认 true → 恢复

**目的**: 验证核心"暂停-确认-恢复"闭环（FR-001/002/004/005）。

**操作**:
```bash
cargo test -p agent_scope_agent hitl_confirm_resume
```

**预期**: 
- 首个 `reply_stream`：`RequireUserConfirmEvent` 后流结束（无 denied 喂回、无 ReplyEnd），tool_call 带 `state="asking"` 与 `suggested_rules`。
- 注入 `UserConfirmResultEvent{confirmed:true}` 后再次 `reply_stream`：工具执行 → tool_result → `ReplyEnd(completed)`。
- 事件顺序与 Python 黄金快照一致。

## 验证场景 2: 拒绝（confirmed=false）

**目的**: 验证拒绝语义（FR-006）。

**操作**:
```bash
cargo test -p agent_scope_agent hitl_confirm_deny
```

**预期**: 工具不执行（副作用为零），生成 `state=DENIED` 的 tool_result（含拒绝提示），agent 调整继续。

## 验证场景 3: 非法恢复报错

**目的**: 验证错误契约（FR-007/008/010）。

**操作**:
```bash
cargo test -p agent_scope_agent hitl_invalid_confirm
```

**预期**: agent 无 awaiting 时注入确认事件 → 明确错误；id 不匹配 → 明确错误指出额外 id。

## 验证场景 4: 多工具并发确认

**目的**: 验证并发与去重（FR-011/012）。

**操作**:
```bash
cargo test -p agent_scope_agent hitl_concurrent_confirm
```

**预期**: `RequireUserConfirmEvent` 携带全部 asking tool_call；`UserConfirmResultEvent` 多个 ConfirmResult 逐个匹配；同工具名去重。

## 验证场景 5: 外部执行 + 中断事件

**目的**: 验证三类事件输入（FR-013~016）。

**操作**:
```bash
cargo test -p agent_scope_agent hitl_external_exec
cargo test -p agent_scope_agent hitl_interrupt
```

**预期**: 外部执行结果恢复并更新状态 finished；中断以 `ReplyEnd(INTERRUPTED)` 结束、无 awaiting 时 no-op。

## 验证场景 6: 现有测试全量回归

**目的**: 确认无破坏。

**操作**:
```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --check
```

**预期**: 全绿。原依赖"denied 喂回"行为的测试已按 Python 语义更新。

## 端到端（真实模型，可选）

改造后 `examples/human-in-the-loop` 应为"暂停-确认-恢复"交互：

```bash
# 需 DASHSCOPE_API_KEY
cargo run -p human-in-the-loop
```

**预期**: 普通对话不确认；调用 write_note 时暂停询问 y/n/a；y=恢复执行写入、n=DENIED 拒绝、a=采纳 allow 规则后续不再询问。**不再**截断历史/重建 agent。

## 参考

- 契约: [contracts/hitl-events.md](contracts/hitl-events.md)
- 数据模型: [data-model.md](data-model.md)

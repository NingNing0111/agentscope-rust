# Quickstart: Agent 运行时状态注入验证指南

**Feature**: 026-runtime-state-injection | **Date**: 2026-08-04

本指南验证运行时状态注入管线端到端可用。实现细节见 `data-model.md` 与 `contracts/`；本指南只描述可运行的验证场景与期望结果。

## 前置

- Rust 工作区可编译：`rtk cargo build`
- 新增依赖 `chrono-tz` 已在 `Cargo.toml` workspace 声明
- 测试夹具：固定时钟 `now` 传入注入函数；`ScriptedModel` / MockModel 提供确定 `count_tokens`

## 验证场景

### 1. 首次回复注入当前时间

```rust
// 空上下文 + 首轮 + now = 2026-07-01T12:00:00Z
let event = maybe_inject_runtime_state(&state, "assistant", &config, now, /*cur_iter=*/1, None, /*task_tools_enabled=*/true);
```

**期望**: 注入一条 `HintBlock`，`hint` 含：

```text
<current-time>2026-07-01T12:00:00</current-time>
<timezone>UTC</timezone>
```

被 `<system-reminder>` 模板包裹；`source == {"label": "System", "sublabel": "Runtime State"}`；发射 `HintBlockEvent`（`emit_hint_event=true` 时）。

### 2. 未完成任务提醒（兼容基线）

```rust
// state.tasks_context 含 1 个 pending 任务，上下文无任务工具痕迹
```

**期望**: `hint` 含 `<tasks>You have 0 in-progress tasks and 1 pending tasks. Use `TaskList` to view them if you don't know.</tasks>`，与 Feature 024 逐字一致。

### 3. 上下文用量预警

```rust
// 首轮 + model.count_tokens = 700, context_size = 1000, trigger_ratio = 0.8, buffer = 0.2
// 700 > (0.8 - 0.2) * 1000 == 600 → 命中
```

**期望**: `hint` 含 `<context-length>Your current context contains 700 tokens. When reaching 800 tokens, your context will be compressed.</context-length>`。

### 4. 近间隔时间不重复注入

```rust
// 上下文已有 10 分钟前的注入（含 <current-time>2026-07-01T11:50:00</current-time>）
// now = 2026-07-01T12:00:00Z → elapsed 0.17h < 0.5h
```

**期望**: 零注入、零事件。

### 5. 压缩后重新注入

```rust
// 清空 state.context（模拟压缩）后再次调用
```

**期望**: 时间维度重新注入。

### 6. 总开关关闭

```rust
// InjectionConfig { inject_runtime_state: false, .. }
```

**期望**: 任意维度条件满足均不注入、上下文无追加、零事件。

### 7. 配置校验

```rust
// InjectionConfig { template: "<system-reminder></system-reminder>", .. }  // 缺占位符
```

**期望**: `AgentConfig::build()` 返回 `Err(AgentError::InvalidConfig)`。

### 8. 无效时区回退 UTC

```rust
// InjectionConfig { timezone: "Mars/Olympus_Mons", .. }
```

**期望**: 不报错；注入的 `<timezone>` 文本仍为 `Mars/Olympus_Mons`（Python 注入原始配置值），墙钟时间按 UTC 计算。

## 测试命令

```bash
rtk cargo test -p agent_scope_agent          # 全部 agent 测试
rtk cargo test -p agent_scope_agent runtime_injection   # 本特性核心测试
rtk cargo test -p agent_scope_agent task_reminder      # 024 兼容基线（不回归）
rtk cargo clippy -p agent_scope_agent
rtk cargo fmt --check -p agent_scope_agent
```

## 期望结果

- 新测试 `runtime_injection_tests.rs` 全部通过，覆盖 research R10 的 13 个场景。
- 既有 `task_reminder_tests.rs` 全部通过（兼容封装不回归）。
- 既有 `task_tools_tests.rs` / `task_tools_e2e_tests.rs` 不回归。
- `cargo clippy` / `cargo fmt --check` 无告警。

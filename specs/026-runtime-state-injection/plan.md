# Implementation Plan: Agent 运行时状态注入系统

**Branch**: `026-runtime-state-injection` | **Date**: 2026-08-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/026-runtime-state-injection/spec.md`

## Summary

将既有任务维度提醒注入（Feature 024 `task_reminder.rs`）泛化为 Python 参考实现的**统一运行时状态注入管线**：在一次调用内评估时间、未完成任务、上下文用量三个维度，组装为单条 `HintBlock` 追加到持久上下文，并在 `emit_hint_event` 开启时发射 `HintBlockEvent`。新增 `InjectionConfig` 全量配置（总开关、时区、时间格式、时间间隔、缓冲比例、模板、来源标识、任务工具名列表、附加字段、事件开关）。任务维度文本、来源标识、感知检测与 024 逐字兼容；无效时区回退 UTC；`inject_runtime_state` 与 `task_tools_enabled` 独立。技术方案见 [research.md](./research.md)。

## Technical Context

**Language/Version**: Rust 2024 edition（workspace 统一）
**Primary Dependencies**: `chrono`（已有）、`chrono-tz`（新增，IANA 时区解析，对齐 Python `ZoneInfo`）、`serde`/`serde_json`、`uuid`（已有）、`agent_scope_message` / `agent_scope_event` / `agent_scope_state`（内部 crate）
**Storage**: N/A（注入写入 `AgentState.context`，随会话持久化，复用 Feature 025 存储管线）
**Testing**: `cargo test` / `cargo clippy` / `cargo fmt`；固定时钟 `now` 参数 + 断言（对齐 Python `agent_injection_test.py`）；`ScriptedModel` / MockModel 提供确定 `count_tokens`
**Target Platform**: 跨平台（macOS / Linux 开发验证）；`chrono-tz` 内嵌 IANA 数据库，无运行时系统 tz 依赖
**Project Type**: Rust 多 crate 库（workspace `crates/*`）
**Performance Goals**: 注入评估为每次推理迭代一次 O(context) 反向扫描 + 至多一次 append，与既有 `task_reminder` 同级；不引入额外模型调用
**Constraints**: 不修改系统提示词（提示缓存友好）；不新增事件类型（复用 `HintBlockEvent`）；任务维度注入文本与 024 逐字一致；`inject_runtime_state` 与 `task_tools_enabled` 独立开关
**Scale/Scope**: 单 agent 会话内的注入管线；覆盖 time / tasks / context-length 三维 + `InjectionConfig` 全量配置 + `HintBlockEvent` 发射 + 配置校验

## Constitution Check

*GATE: 通过。设计后复查通过。*

| 条款 | 符合性 | 说明 |
|------|--------|------|
| 第一条（兼容性优先） | ✅ | 注入字段文本、来源标识、默认值、事件语义对齐 Python `_inject_runtime_state` / `InjectionConfig`；任务维度与 024 逐字一致（SC-002） |
| 第二条（锁定上游版本） | ✅ | 兼容目标锁定 Python commit `9d1026fa`，记录于各设计工件与 CHANGELOG |
| 第三条（Python 行为基准） | ✅ | 以 `tests/agent_injection_test.py` 实测行为为基准；无效时区回退 UTC 依此校准 |
| 第五条（不允许伪兼容） | ✅ | 三维注入按 Python 语义实现，不空实现；配置非法返回 `AgentError::InvalidConfig` |
| 第六条（测试驱动兼容） | ✅ | 固定时钟 `now` 参数 + MockModel 确定 `count_tokens`，对齐 Python 测试 13 场景；含 024 兼容回归测试 |
| 第七条（Trace 验收产物） | ✅ | 事件发射复用 `AgentEvent::HintBlock`，事件顺序纳入 trace 比较；注入的上下文变化经 model request trace 可观测 |
| 第八条（Rust 原生设计） | ✅ | 用 `InjectionConfig` struct + `validate()` 表达配置与校验；统一管线函数而非 Python 的生成器/继承 |
| 第九条（安全 Rust 优先） | ✅ | 无 `unsafe`；无新增 panic 倾向调用 |
| 第十条（结构化并发） | ✅ | 注入为同步函数，在调用点写锁临界区执行，不 spawn 任务；事件发射在调用点 `event_tx` 发送 |
| 第十一条（分层与依赖方向） | ✅ | `runtime_injection` 位于 `agent_scope_agent`，依赖 `message`/`event`/`state` 抽象层，无反向污染 |
| 第十二条（稳定数据协议） | ✅ | `InjectionConfig` 序列化字段稳定；复用既有 `HintBlock`/`HintBlockEvent` 结构 |
| 第十三条（稳定错误模型） | ✅ | 配置非法返回 `AgentError::InvalidConfig { field, message }`，类型化错误 |
| 第十四条（可观测性） | ✅ | 注入评估失败以 `tracing::warn` 记录；事件发射经标准事件管线可观测 |
| 第十五条（性能优先正确性） | ✅ | 不改变事件顺序、不吞错；注入为只读评估 + 一次追加 |
| 第十六条（小步交付） | ✅ | 单特性仅覆盖注入管线，不包含任务分发调度（明确排除于 spec Assumptions） |
| 第十七条（完成定义） | ✅ | 满足条件见 tasks.md；含 024 回归、clippy、fmt、文档更新 |
| 第十八条（兼容性分级） | ✅ | 目标 L2 核心行为兼容（注入行为可观察一致）；配置语义 L3 等价 |
| 第十九条（变更治理） | ✅ | 无宪法违反；唯一校准（无效时区回退而非拒绝）基于第三条以 Python 实测行为为准，记录于 research R8 与 spec 修订 |

**复杂度跟踪**：无违反，不填。

## Project Structure

### Documentation (this feature)

```text
specs/026-runtime-state-injection/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── runtime-state-injection.md
│   └── injection-config.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/agent_scope_agent/
├── src/
│   ├── config.rs              # [MODIFY] +InjectionConfig struct/validate + AgentConfigBuilder::injection_config
│   ├── runtime_injection.rs   # [ADD] 统一注入管线：maybe_inject_runtime_state + 三维评估 + HintBlock 组装
│   ├── task_reminder.rs       # [MODIFY] 降为兼容薄封装，委托 runtime_injection（任务维度）
│   ├── react_loop.rs          # [MODIFY] 调用点：传入 injection_config/now/cur_iter/token 计数；发射 HintBlockEvent
│   ├── streaming_reactor.rs   # [MODIFY] 调用点：同上
│   └── lib.rs                 # [MODIFY] 导出 runtime_injection 模块与 InjectionConfig
├── tests/
│   ├── task_reminder_tests.rs # [KEEP] 024 兼容基线不回归（经薄封装仍通过）
│   └── runtime_injection_tests.rs  # [ADD] 新特性测试（对齐 Python 13 场景）
Cargo.toml                      # [MODIFY] +chrono-tz 依赖
```

**Structure Decision**: 单一 agent crate 内新增 `runtime_injection` 模块，与既有 `task_reminder` 并存（后者降为薄封装）。`InjectionConfig` 放 `config.rs`（与 `AgentConfig` 同处）。依赖通过 workspace `Cargo.toml` 增加 `chrono-tz`。测试按 crate 惯例放 `tests/` 目录。

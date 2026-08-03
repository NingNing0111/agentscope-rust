# Research: Agent 运行时状态注入系统

**Feature**: 026-runtime-state-injection | **Date**: 2026-08-04
**上游基准**: Python AgentScope `9d1026fa` `agent/_agent.py::_inject_runtime_state`、`agent/_config.py::InjectionConfig`、`tests/agent_injection_test.py`

## 背景与范围

Python 参考实现在每次推理迭代前将「时间 / 未完成任务 / 上下文用量」三维运行时状态注入对话上下文（`_inject_runtime_state`），由 `InjectionConfig` 完整配置，并发射 `HintBlockEvent`。Rust 端 Feature 024 已实现任务维度的提醒注入（`task_reminder.rs`，硬编码 source/template/感知检测），但：
- 仅覆盖任务维度，未实现时间与上下文用量维度；
- 配置硬编码，无 `InjectionConfig` 等价物；
- 不发射 `HintBlockEvent`（spec 024 Assumptions 明确"零事件"，本次需升级为发射事件）；
- 无时区解析能力（仅依赖 `chrono`，缺 IANA tz）。

本特性将既有任务提醒注入**泛化为统一注入管线**，补齐时间与上下文用量维度与全量配置，并保留 024 任务维度的逐字兼容行为。

---

## R1: 时区解析与 IANA 时区名支持

- **Decision**: 使用 `chrono-tz`（IANA 时区数据库，`Tz` 枚举 + `Tz::from_str`），等效 Python `ZoneInfo(name)`。
- **Rationale**: Python `InjectionConfig.timezone` 使用 IANA 时区名（如 `Asia/Shanghai`、`UTC`），默认 `UTC`。`chrono` 本体不支持按名称解析 IANA 时区；`chrono-tz` 内嵌 IANA 数据库，`chrono_tz::Tz::from_str("Asia/Shanghai")` 即等效 Python `ZoneInfo("Asia/Shanghai")`，且实现了 `TimeZone` trait，可与 `chrono::DateTime<Tz>` 无缝协作。
- **Alternatives considered**:
  - `iana-time-zone`：仅返回**系统**时区名，无法按任意名称解析，不适用。
  - `time` crate 的 `UtcOffset`：仅支持固定偏移，无法表达 DST 规则与 IANA 名称，不适用。
  - 手写 tz 数据解析：工作量过大且易错，不适用。
- **Impact**: 需在 `workspace.dependencies` 新增 `chrono-tz`。`agent_scope_agent` crate 引入该依赖。

---

## R2: 时间格式化 / 解析（`time_format`）

- **Decision**: 默认 `time_format = "%Y-%m-%dT%H:%M:%S"` 直接映射到 `chrono::format` 语法（`%Y-%m-%dT%H:%M:%S` 中 `T` 为字面量），格式化用 `DateTime::format`，解析用 `NaiveDateTime::parse_from_str`。配置构建时校验格式可往返（format → parse → 相同值）。
- **Rationale**: Python 默认格式 `%Y-%m-%dT%H:%M:%S`（`strftime`/`strptime`）与 chrono 语法基本兼容：`%Y/%m/%d/%H/%M/%S` 在两边的含义一致，`T` 在 chrono 中作为字面量匹配。Python 测试 `agent_injection_test.py` 使用 `2026-07-01T12:00:00`，chrono 可直接往返。
- **Alternatives considered**:
  - 自建 strftime 兼容解析器：覆盖 Python 全量指令成本高，且本项目仅需支持默认格式与常见指令，不值得。
  - 直接透传格式串给 chrono 不做校验：用户传无法往返的格式会导致解析失败静默注入（违反 FR-014），故增加往返校验。
- **Impact / 兼容性记录**: chrono 支持的格式指令集是 Python strftime 的**子集**。极少数 Python 合法格式（如 `%f` 微秒、`%s` 纪元秒、`%j` 儒略日）chrono 可能不支持。默认格式与常用格式均兼容；对无法往返的格式在配置构建时拒绝（FR-014），并在文档标注为 Rust 侧增强——不破坏 Python 合法输入（Python 默认与常见格式可往返）。

---

## R3: 注入管线架构

- **Decision**: 新增 `crates/agent_scope_agent/src/runtime_injection.rs`，提供统一评估函数 `maybe_inject_runtime_state`，一次调用内完成三维（time / tasks / context-length）评估、组装为单条 HintBlock、追加到 `state.context`，并返回待发射的 `HintBlockEvent`（若 `emit_hint_event` 开启）。现有 `task_reminder.rs` 的 `maybe_inject_task_reminder` 保留为兼容薄封装（内部委托统一管线），避免 024 既有调用点与测试大面积改动。
- **Rationale**: 三维共享同一份"扫描上下文 + 组装 HintBlock + 追加"骨架，统一管线避免三份重复逻辑；保留旧封装则 `react_loop.rs` / `streaming_reactor.rs` / `task_reminder_tests.rs` 零破坏迁移（宪法第十七条"不回归"）。
- **Alternatives considered**:
  - 直接改写现有 `task_reminder.rs` 为三维管线：需同步修改既有测试与调用点，回归风险更高。
  - 三个独立函数分别注入：会产生多条 HintBlock（违反 FR-013 同一条提示块约束），且扫描上下文重复。
- **Impact**: 新模块 + `InjectionConfig`；调用点改为传入 `injection_config` 与运行时参数（`now`、`cur_iter`、token 计数）。

---

## R4: `cur_iter` 语义对齐

- **Decision**: 注入函数接收调用方当前的**迭代序号**（Rust 侧局部变量 `cur_iter`，首轮为 1）。上下文用量维度仅在首轮（`cur_iter == 1`）评估，对齐 Python `self.state.cur_iter == 0`（Python 首轮 `cur_iter` 为 0，每轮结束 +1；Rust 局部 `cur_iter` 循环顶部 +1，首轮为 1）。
- **Rationale**: Python `_inject_runtime_state` 中 `if self.state.cur_iter == 0` 判定上下文维度仅首轮执行。Rust 侧 `react_loop` 与 `streaming_reactor` 均用局部 `cur_iter`（初始 0，循环顶部 `+= 1`），首轮进入循环体时 `cur_iter == 1`。故判断条件为 `cur_iter == 1` 即与 Python 语义等价。
- **Alternatives considered**:
  - 同步维护 `state.reply_context.cur_iter` 并读取：该字段存在于 `AgentState`，但 react_loop 当前未在循环中更新它（仅 `do_reply_stream` 初始化时置 0），引入跨路径不一致风险。
  - 在注入函数内部维护自增计数：无法感知"新 reply 已开始"的边界，易错。
- **Impact**: 调用点在每次迭代将局部 `cur_iter` 传入注入函数；时间与任务维度不受 cur_iter 影响（每轮评估），上下文维度仅首轮评估（对齐 Python 每回复只评估一次）。

---

## R5: 事件发射（`emit_hint_event`）

- **Decision**: 注入发生时，若 `InjectionConfig.emit_hint_event == true`，统一管线返回构造好的 `HintBlockEvent`（携带 `reply_id`、`block_id`、`source`、`hint`），由调用点（batch `react_loop` / streaming `streaming_reactor`）通过 `event_tx` 发送 `AgentEvent::HintBlock`。`HintBlockEvent` 类型已存在（`agent_scope_event::block_events`），无需新增事件类型。
- **Rationale**: Python `_inject_runtime_state` 在注入时 `yield HintBlockEvent`。Rust 端 `AgentEvent::HintBlock` 枚举变体与 `HintBlockEvent` 结构均已存在且被 `append_event.rs` 处理，直接复用。
- **Alternatives considered**:
  - 事件内嵌到注入函数自发送：注入函数无 `event_tx` 引用，且 batch/streaming 的通道类型一致但生命周期不同，由调用点发送更清晰。
- **Impact**: 调用点新增发送逻辑；spec 024 "零事件" 假设被取代（记录于 spec 026 Assumptions）。

---

## R6: `InjectionConfig` 配置位置与默认值

- **Decision**: 在 `AgentConfig` 新增 `injection_config: InjectionConfig` 字段（默认 `InjectionConfig::default()`），builder 提供 `.injection_config(...)`。字段、命名、默认值与 Python `InjectionConfig` 对齐：`inject_runtime_state=true`、`timezone="UTC"`、`time_format="%Y-%m-%dT%H:%M:%S"`、`time_interval=0.5`、`context_buffer_ratio=0.2`、`template`（含 `{runtime_state}` 占位符）、`injection_source`、`task_tool_names`（四任务工具名）、`extra_fields={}`、`emit_hint_event=true`。
- **Rationale**: Python 通过 `Agent(..., injection_config=InjectionConfig())` 构造。Rust 端 `AgentConfig` 是构造参数集合，新增字段是等价映射。默认值逐一对齐 Python `_config.py`。
- **Alternatives considered**:
  - 独立于 `AgentConfig` 的构造参数：Rust 端 `ReActAgent::new(config, react_config, context_config, middlewares)` 已固定 4 参，独立参数会破坏签名；并入 `AgentConfig` 与 `task_tools_enabled`/`context_config` 一致。
- **Impact**: `AgentConfig` / `AgentConfigBuilder` 扩展；`config.rs` 增测试。

---

## R7: 总开关与任务工具开关的关系（兼容性关键）

- **Decision**: 时间与上下文维度仅受 `injection_config.inject_runtime_state` 控制；**任务维度**同时要求 `inject_runtime_state && task_tools_enabled` 才注入。
- **Rationale**: Python 中任务注入只受 `inject_runtime_state` 控制（工具注册是 Toolkit 的事）。但 Rust 024 中 `task_tools_enabled=false` 同时抑制工具注册**与**任务提醒注入，且有既有测试 `test_disabled_flag_via_loop_does_not_inject` 断言该行为。为不回归 024（宪法第三条、第十七条），任务维度注入在 `task_tools_enabled=false` 时不激活；`inject_runtime_state=false` 则三个维度全部不激活（对齐 Python `test_disabled_injection`）。
- **Alternatives considered**:
  - 任务维度仅受 `inject_runtime_state` 控制：会让 `task_tools_enabled=false` 的既有 agent 开始注入任务提醒，破坏 024 契约与测试。
- **Impact**: 调用点同时传入 `task_tools_enabled` 与 `injection_config`；`runtime_injection` 内部对任务维度做双开关判断。

---

## R8: 无效时区与非法配置的处理（spec FR-014 校准）

- **Decision**: 无效时区名（如 `Mars/Olympus_Mons`）**回退 UTC 而非拒绝**（对齐 Python `test_invalid_timezone_falls_back_to_utc`，Python `_resolve_timezone` 解析失败回退 `timezone.utc`）。配置校验仅强制：模板缺失 `{runtime_state}` 占位符时拒绝（Python `test_template_without_placeholder_is_rejected`）、`time_format` 无法往返时拒绝、`time_interval` 为负拒绝、`context_buffer_ratio` 超出 `[0,1]` 拒绝、`context_buffer_ratio >= trigger_ratio` 拒绝。
- **Rationale**: Python `InjectionConfig.timezone` 是普通字符串字段，无 pydantic 校验；无效时区在运行时回退。spec 026 FR-014 原文写"时区非法拒绝"，与 Python 实际行为（回退）冲突。依宪法第三条（实际运行结果为行为基准），校准为**回退**；其余校验项保留。
- **Alternatives considered**:
  - 按 FR-014 原文拒绝无效时区：与 Python `test_invalid_timezone_falls_back_to_utc` 行为冲突，属破坏兼容，放弃。
- **Impact**: spec FR-014 需同步修订（"时区非法"改为"时区解析失败回退 UTC"）；plan 中标注该校准及宪法依据。

---

## R9: token 计数与上下文用量阈值

- **Decision**: 上下文用量注入条件（对齐 Python Step 4）：`input_tokens > max(0, trigger_ratio - context_buffer_ratio) * context_size`，其中 `input_tokens = model.count_tokens(hook_messages, tool_schemas)`、`trigger_ratio` 取自 `ContextConfig.trigger_ratio`、`context_size = model.context_size()`。仅首轮评估。
- **Rationale**: Python 用 `_prepare_model_input` 计数、`trigger_ratio * context_size` 作阈值，注入文本含 `input_tokens` 与 `trigger_tokens`。Rust 端 `ChatModel::count_tokens` / `context_size` 已存在（`model_trait.rs`），`trigger_ratio` 在 `ContextConfig`。
- **Alternatives considered**:
  - 在注入函数内重新组装 messages 计数：与调用点已计算的 token 重复；由调用点把 `input_tokens` 传入注入函数即可（batch/streaming 压缩检查处已计算，可复用或复用近似）。
- **Impact**: 调用点需在首轮提供 token 计数；测试用固定 `count_tokens`（对齐 Python `MockModel.count_tokens = AsyncMock(return_value=700)`）。

---

## R10: 测试策略（对齐 Python `agent_injection_test.py`）

- **Decision**: 注入函数接收显式 `now: DateTime<FixedOffset>` 参数（而非内部调用 `Utc::now()`），使测试可注入固定时钟（对齐 Python `_FrozenDatetime` patch）。核心逻辑用单元测试验证，覆盖 Python 测试全部 11 个场景的 Rust 等价形式：
  1. 首轮触发时间注入；2. 超间隔/近间隔；3. 压缩后重注入；4. 待办任务触发；5. 记录时区生效；6. 附加字段附着；7. 附加字段不触发；8. 总开关关闭；9. 上下文用量触发；10. 上下文用量与其他维度独立共存；11. 模板缺占位符拒绝；12. 模板花括号保留；13. 无效时区回退 UTC。
- **Rationale**: 宪法第六条要求确定性组件（固定 Clock、Mock/Recorded Model）。Python 测试文件 `agent_injection_test.py` 是行为基准，Rust 单元测试逐条对齐；对可达路径（如时间间隔、上下文用量阈值）用固定参数精确验证。既有 `task_reminder_tests.rs` 保持通过（兼容封装）。
- **Alternatives considered**:
  - 直接复制 Python golden JSON 快照逐字符对比：Python 测试用 `AnyString()` 抹平随机 id，Rust 侧同样用断言而非原始快照；保留为事件级断言。
- **Impact**: 新增 `runtime_injection_tests.rs`；`mocks.rs` 可能需扩展固定 `count_tokens` 的 MockModel。

---

## 结论

统一注入管线将既有任务提醒泛化为三维运行时状态注入，配置、字段文本、事件语义与 Python 参考实现对齐；关键兼容性决策（任务维度双开关、无效时区回退、cur_iter 语义、格式往返校验）已明确。设计工件：`data-model.md`、`contracts/runtime-state-injection.md`、`contracts/injection-config.md`、`quickstart.md`。

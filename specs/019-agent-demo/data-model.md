# Data Model: Complete Agent Demo

**Date**: 2026-08-01 | **Feature**: [spec.md](./spec.md)

本特性是 example/demo 特性。“数据模型”描述 demo 的运行实体、trace 实体、配置实体与能力覆盖关系。实现阶段可用 Rust struct/enum 表达这些实体，也可将部分实体仅体现为 README/checklist 与运行输出；外部可观察契约以本文件和 `contracts/` 为准。

## 实体一览

```text
AgentDemo
├── DemoConfiguration
├── RunMode
├── DemoScenario
│   └── ScenarioStep ×N
├── CapabilityCoverageItem ×N
├── DemoTrace
│   └── TraceEvent ×N
├── DemoTool ×N
├── DemoSessionState
├── DemoMemoryRecord ×N
├── WorkspaceArtifact ×N
└── DemoError / CapabilityStatus
```

## 实体定义

### AgentDemo

`examples/agent-demo` 交付物整体。

| 字段 | 约束 |
|------|------|
| location | 必须为 `examples/agent-demo/` |
| cargo example name | `agent_demo` |
| entrypoint | `examples/agent-demo/main.rs` |
| documentation | `examples/agent-demo/README.md` |
| validation mode | 必须支持 deterministic/offline |
| optional mode | 应支持 live-model opt-in |

**Validation**: `cargo run --example agent_demo -- --mode deterministic` 可运行；`cargo build --examples` 覆盖该 entrypoint。

### DemoConfiguration

用户或环境提供的运行配置。

| 字段 | 类型 | 约束 |
|------|------|------|
| mode | RunMode | 必填，有默认值（推荐 deterministic） |
| api_key | Option<String> | live mode 需要；可来自 CLI 或 `API_KEY`；输出必须脱敏 |
| model | String | live mode 使用，默认可为现有 examples 约定的 DashScope model |
| workspace_dir | Option<PathBuf> | 默认使用可清理 demo 目录；不得默默写入未知位置 |
| trace_json | Option<PathBuf> | 若提供则写结构化 trace |
| show_coverage | bool | 打印 capability coverage table |
| fail_tool | bool | deterministic tool failure injection |
| cancel_after_step | Option<String> | deterministic cancellation injection |
| verbose | bool | 仅增加非敏感诊断；不得输出 raw secret |

**Validation rules**:
1. `mode = live` 且缺少 `api_key` 时，启动前返回 actionable config error。
2. `api_key` 在所有输出中只能显示为 masked form（如 `sk-***abcd`）或完全省略。
3. `workspace_dir` 存在旧状态时，必须报告是否复用、清理或隔离。

### RunMode

Demo 的运行方式。

| Variant | 含义 | 要求 |
|---------|------|------|
| deterministic | 离线、稳定、可用于 CI/maintainer validation | 不依赖网络/真实 LLM/API key；输出高层 trace 稳定 |
| live | 可选真实 provider 路径 | 明确 opt-in；缺配置不运行；provider error 清楚分类 |

**State transitions**:

```text
Configured -> Preflight -> Running -> Completed
                       ├-> Failed(Config/Tool/Model/Internal)
                       └-> Cancelled
```

### DemoScenario

完整 walkthrough 的定义。

| 字段 | 类型 | 约束 |
|------|------|------|
| id | String | 稳定，如 `complete-agent-walkthrough` |
| title | String | README 与输出一致 |
| user_task | String | 安全、非敏感、可重复 |
| steps | Vec<ScenarioStep> | 至少覆盖 8 个 demonstrated capability |
| expected_summary | String | deterministic mode 下稳定 |
| non_goals | Vec<String> | 标注未覆盖 roadmap 能力 |

**Validation**: README 中说明 scenario；运行输出包含 scenario id/title 与最终 summary。

### ScenarioStep

Scenario 中可观察的步骤。

| 字段 | 类型 | 约束 |
|------|------|------|
| step_id | String | 稳定、可在 trace/checklist 中引用 |
| label | String | 人类可读 |
| purpose | String | 说明为什么存在该步骤 |
| capabilities | Vec<String> | 引用 CapabilityCoverageItem id |
| expected_events | Vec<String> | 对应 TraceEvent kind |
| required_by_default | bool | deterministic 主路径是否必须执行 |

**Validation**: 每个 demonstrated capability 至少被一个 ScenarioStep 引用。

### CapabilityCoverageItem

能力覆盖条目。

| 字段 | 类型 | 约束 |
|------|------|------|
| capability_id | String | 稳定 kebab-case，如 `tool-invocation` |
| title | String | 人类可读 |
| status | CapabilityStatus | demonstrated/optional/skipped/unsupported |
| evidence | Vec<String> | step id、trace event kind、artifact path 或 README section |
| notes | String | 限制、环境要求或 unsupported reason |

**Validation**: `--show-coverage` 与 README checklist 必须包含同一组 capability ids；不得将未执行能力标记为 demonstrated。

### CapabilityStatus

| Variant | 含义 |
|---------|------|
| demonstrated | 默认 deterministic 主路径中有可观察证据 |
| optional | 需要 live mode、平台支持或额外 flag |
| skipped | 当前环境或默认安全策略跳过，但不是框架不支持 |
| unsupported | 当前项目未实现或该模式明确不支持，必须给出原因 |

### DemoTrace

一次 demo run 的结构化记录。

| 字段 | 类型 | 约束 |
|------|------|------|
| schema_version | String | 初始 `1` |
| run_id | String | 可标准化；不用于行为判断 |
| mode | RunMode | deterministic/live |
| scenario_id | String | 引用 DemoScenario |
| started_at / finished_at | Option<String> | 可省略或标准化；不得影响 deterministic 验证 |
| events | Vec<TraceEvent> | 按发生顺序排列 |
| coverage | Vec<CapabilityCoverageItem> | 最终状态快照 |
| final_status | completed/failed/cancelled | 与 exit code 对应 |

**Validation**: events 顺序不可在比较中忽略；非确定字段允许标准化。

### TraceEvent

单个可观察事件。

| 字段 | 类型 | 约束 |
|------|------|------|
| sequence | u64 | 单调递增 |
| step_id | Option<String> | 能映射 ScenarioStep |
| kind | String | 稳定枚举名，如 `agent_started`、`message_sent`、`tool_called` |
| summary | String | 默认安全摘要 |
| capability_ids | Vec<String> | 可为空，但关键事件应标注 |
| status | started/succeeded/failed/skipped | 稳定 |
| metadata | serde_json::Value | 只含脱敏/非敏感信息 |

**Must not contain**: raw API key、access token、未脱敏密码、默认情况下的完整敏感对话。

### DemoTool

Demo 中被 Agent 调用的安全工具。

| 字段 | 类型 | 约束 |
|------|------|------|
| name | String | 稳定，如 `calculator`、`knowledge_lookup`、`workspace_writer` |
| input_schema | JSON Schema | 可由 `schemars` 导出或文档化 |
| safe_summary | String | trace 中展示参数概要 |
| deterministic_result | JSON/Text | deterministic mode 固定 |
| failure_mode | Option<DemoError> | `--fail-tool` 可触发 |

**Validation**: Tool call trace 包含工具名、参数安全摘要、结果处理，不泄露敏感内容。

### DemoSessionState

跨 turn 的会话状态摘要。

| 字段 | 类型 | 约束 |
|------|------|------|
| session_id | String | 可标准化 |
| turn_count | u64 | 至少 2 才能证明 session continuity |
| remembered_keys | Vec<String> | 只包含安全 key/摘要 |
| state_store | String | in-memory/file 等安全摘要 |

**Validation**: 第二轮输出或 trace 必须引用第一轮产生的上下文。

### DemoMemoryRecord

Demo 中写入或召回的 memory/context 条目。

| 字段 | 类型 | 约束 |
|------|------|------|
| key | String | 稳定、安全 |
| value_summary | String | 摘要，不含 raw secret |
| source_step_id | String | 产生来源 |
| recalled_step_id | Option<String> | 被召回位置 |

**Validation**: memory recall 必须可从 trace 或最终 summary 中核对。

### WorkspaceArtifact

Demo 产生的可检查文件或 artifact。

| 字段 | 类型 | 约束 |
|------|------|------|
| path | PathBuf | 位于 demo workspace_dir 下 |
| kind | String | trace/report/config 等 |
| safe_to_delete | bool | 默认 true |
| summary | String | 输出中展示 |

**Validation**: 默认路径可清理；不覆盖用户未授权文件。

### DemoError

用户可见错误模型。

| 字段 | 类型 | 约束 |
|------|------|------|
| category | String | `config_error`/`tool_error`/`model_error`/`unsupported_feature`/`cancelled`/`internal_error` |
| code | String | 稳定 kebab-case |
| message | String | actionable，不含 secret |
| recovery_hint | Option<String> | 缺配置或可恢复错误必须提供 |
| step_id | Option<String> | 若运行中失败则指向步骤 |

**Validation**: 缺 `API_KEY`、tool failure、cancellation 至少在 deterministic validation 中可触发或文档化。

## 关系与一致性规则

1. **Scenario-Coverage 规则**: 每个 `CapabilityCoverageItem(status=demonstrated)` 必须至少被一个 `ScenarioStep` 和一个 `TraceEvent` 引用。
2. **Trace 顺序规则**: `TraceEvent.sequence` 单调递增；事件顺序是验收内容，不可在验证中忽略。
3. **Secret 脱敏规则**: `DemoConfiguration.api_key` 不得出现在 `TraceEvent.metadata`、README expected output、terminal output 或 WorkspaceArtifact 中。
4. **Mode 分离规则**: deterministic mode 不读取或要求 API key；live mode 缺配置时在 Preflight 阶段失败。
5. **Workspace 隔离规则**: 所有 artifact 默认位于 demo workspace；若用户显式指定目录，启动时报告并避免覆盖未知文件。
6. **Unsupported 透明规则**: optional/skipped/unsupported 能力必须在 coverage notes 中说明原因，不得计入 demonstrated 数量。

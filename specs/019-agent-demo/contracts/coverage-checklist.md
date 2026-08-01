# Contract: Agent Demo Capability Coverage Checklist

**Date**: 2026-08-01 | **Feature**: [../spec.md](../spec.md)

本文定义 Feature 019 demo 必须在 README、`--show-coverage` 输出和 trace coverage 中共同维护的 capability checklist。实现阶段可新增条目，但不得删除 required demonstrated 条目，除非同步修改 spec/plan 并解释原因。

## Status values

| Status | Meaning |
|--------|---------|
| `demonstrated` | 默认 deterministic 主路径有可观察证据 |
| `optional` | 需要 `--mode live`、额外 flag 或平台支持 |
| `skipped` | 当前默认环境安全跳过，但框架能力存在 |
| `unsupported` | 当前项目或该 demo 模式明确不支持，必须说明原因 |

## Required checklist

| Capability ID | Title | Required status | Minimum evidence |
|---------------|-------|-----------------|------------------|
| `agent-interaction` | Agent interaction loop | `demonstrated` | scenario timeline 包含 user input → agent steps → final output |
| `structured-messages` | Structured messages/content | `demonstrated` | trace 中出现 message sent/received 与 role/content summary |
| `event-progress` | Event/progress reporting | `demonstrated` | terminal timeline 或 trace events 展示生命周期 |
| `streaming-incremental-output` | Streaming or incremental output | `demonstrated` 或 `optional` | deterministic 可模拟 stable deltas；live 可展示 provider stream |
| `tool-invocation` | Tool invocation lifecycle | `demonstrated` | tool_called + tool_completed/tool_failed；参数安全摘要；结果被 Agent 使用 |
| `session-continuity` | Multi-turn session continuity | `demonstrated` | 至少两轮，第二轮引用第一轮上下文 |
| `memory-context-recall` | Memory/context recall | `demonstrated` | memory_written + memory_recalled 或等价输出证据 |
| `middleware-observation` | Middleware-style cross-cutting behavior | `demonstrated` | middleware_entered/completed 或 policy/enrichment trace |
| `trace-observability` | Sanitized trace output | `demonstrated` | terminal timeline + optional trace JSON contract |
| `configuration-handling` | Missing config/actionable setup | `demonstrated` | deterministic no-key preflight；live missing key error path documented/testable |
| `safe-secret-handling` | Secret redaction | `demonstrated` | README/trace examples不含 raw key；live preflight masks key |
| `typed-error-handling` | Stable error categories | `demonstrated` | `--fail-tool` 或 missing config 输出 category/code |
| `cancellation-handling` | Cancellation visibility | `demonstrated` 或 `optional` | `--cancel-after-step` 产生 cancellation event，保留已完成 trace |
| `rag-context-enrichment` | RAG/context enrichment | `demonstrated` 或 `optional` | deterministic mock lookup 或 live/optional RAG step |
| `workspace-artifact` | Workspace artifact handling | `demonstrated` | demo workspace 中写入 summary/trace/artifact 并在输出中报告 |
| `sandbox-policy` | Sandbox or permission/policy handling | `optional` 或 `skipped` | capability report、policy check、unsupported/skip reason；不得伪执行危险操作 |
| `live-provider` | Optional live model provider | `optional` | `--mode live` 文档与 preflight；缺 API key actionable failure |

## Non-goal / roadmap items

以下能力不得在 Feature 019 中标为默认 `demonstrated`，除非项目已有对应实现且 demo 真实执行：

| Capability ID | Expected status | Notes |
|---------------|-----------------|-------|
| `multi-agent-collaboration` | `unsupported` 或 `skipped` | Roadmap Feature 014 in constitution list，当前 demo 只覆盖 single-agent |
| `distributed-runtime` | `unsupported` 或 `skipped` | Roadmap Feature 015 in constitution list，非本 demo 范围 |
| `production-hardening` | `skipped` | README 必须说明 demo 不是 production template |

## Output requirements

### README checklist

`examples/agent-demo/README.md` 必须包含表格，字段至少为：

- Capability
- Status
- Where to observe
- Notes / requirements

### Runtime checklist

当用户传入 `--show-coverage` 时，CLI 必须打印同一组 capability ids，并说明：

- demonstrated count
- optional/skipped/unsupported count
- 每个非 demonstrated 项的原因或 opt-in path

### Trace checklist

`DemoTrace.coverage` 必须包含同一组 capability ids，字段遵循 [trace-schema.md](./trace-schema.md)。

## Validation rules

1. `Required status = demonstrated` 的条目不得只出现在 README；必须有 runtime output 或 trace event evidence。
2. `optional` 条目必须说明启用方式或环境要求。
3. `skipped` 条目必须说明为什么默认跳过。
4. `unsupported` 条目必须说明当前不支持的边界，禁止空实现冒充。
5. Coverage checklist 不得包含 raw credentials、sensitive prompt 或用户私有路径（路径可相对化）。

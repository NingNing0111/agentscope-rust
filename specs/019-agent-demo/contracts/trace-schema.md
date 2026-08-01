# Contract: Agent Demo Trace Schema

**Date**: 2026-08-01 | **Feature**: [../spec.md](../spec.md)

本文定义 `agent_demo` 可选 `--trace-json` 输出的结构化 trace 契约。实现可使用 Rust struct + `serde` 生成 JSON；字段名和关键枚举值应保持稳定，以支持维护者回归验证。

## Top-level schema

```json
{
  "schema_version": "1",
  "run_id": "demo-run-0001",
  "mode": "deterministic",
  "scenario_id": "complete-agent-walkthrough",
  "final_status": "completed",
  "events": [],
  "coverage": [],
  "artifacts": []
}
```

## `DemoTrace`

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `schema_version` | string | yes | 初始值为 `1` |
| `run_id` | string | yes | 可包含 UUID；验证时允许标准化 |
| `mode` | string | yes | `deterministic` 或 `live` |
| `scenario_id` | string | yes | 推荐 `complete-agent-walkthrough` |
| `started_at` | string/null | no | ISO-8601；验证时允许标准化或省略 |
| `finished_at` | string/null | no | ISO-8601；验证时允许标准化或省略 |
| `final_status` | string | yes | `completed` / `failed` / `cancelled` |
| `events` | array&lt;TraceEvent&gt; | yes | 必须按发生顺序排列 |
| `coverage` | array&lt;CoverageItem&gt; | yes | 与 README / `--show-coverage` capability ids 一致 |
| `artifacts` | array&lt;ArtifactSummary&gt; | no | workspace 产物安全摘要 |

## `TraceEvent`

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `sequence` | number | yes | 从 1 开始单调递增 |
| `step_id` | string/null | no | 能映射到 scenario step |
| `kind` | string | yes | 见下方稳定 event kinds |
| `summary` | string | yes | 人类可读安全摘要 |
| `capability_ids` | array&lt;string&gt; | yes | 可为空；关键事件必须标注 |
| `status` | string | yes | `started` / `succeeded` / `failed` / `skipped` |
| `metadata` | object | yes | 仅非敏感信息或脱敏摘要 |

### Stable event kinds

实现阶段可新增 event kind，但以下 kind 一旦使用应保持稳定：

| Kind | Meaning |
|------|---------|
| `preflight_started` | 配置检查开始 |
| `preflight_completed` | 配置检查完成 |
| `agent_started` | Agent 场景开始 |
| `message_sent` | 用户/系统消息进入 Agent 流程 |
| `message_received` | Agent 生成消息或最终输出 |
| `stream_delta` | 流式/增量输出片段 |
| `tool_called` | 工具调用开始 |
| `tool_completed` | 工具调用成功 |
| `tool_failed` | 工具调用失败 |
| `memory_written` | Memory/context 写入 |
| `memory_recalled` | Memory/context 召回 |
| `session_saved` | Session 状态保存 |
| `session_loaded` | Session 状态加载/延续 |
| `middleware_entered` | Middleware hook 进入 |
| `middleware_completed` | Middleware hook 完成 |
| `rag_context_added` | RAG/context enrichment 发生 |
| `workspace_artifact_written` | 工作区 artifact 写入 |
| `sandbox_checked` | Sandbox capability/policy 检查或可控执行 |
| `error_reported` | 用户可见错误被记录 |
| `cancellation_requested` | 取消被触发 |
| `scenario_completed` | walkthrough 完成 |

## `CoverageItem`

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `capability_id` | string | yes | 稳定 kebab-case |
| `title` | string | yes | 人类可读 |
| `status` | string | yes | `demonstrated` / `optional` / `skipped` / `unsupported` |
| `evidence` | array&lt;string&gt; | yes | step id、event kind 或 artifact path |
| `notes` | string | yes | 限制、环境要求或 unsupported 原因 |

## `ArtifactSummary`

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `path` | string | yes | 建议为 workspace-dir 相对路径 |
| `kind` | string | yes | `trace-json` / `summary` / `workspace-file` 等 |
| `safe_to_delete` | bool | yes | demo 默认产物应为 true |
| `summary` | string | yes | 不含敏感内容 |

## Sanitization rules

Trace JSON 必须遵守：

1. 不包含 raw `API_KEY`、`--api-key`、access token、密码。
2. Live mode request/response 默认只记录摘要；不得默认 dump 完整敏感对话。
3. Tool arguments 只记录 safe summary；如参数本身可能含 secret，必须 mask。
4. 非确定字段（timestamps、UUID、latency）可存在，但不得作为 deterministic correctness 的唯一判断依据。
5. 事件顺序、错误类别、capability status 不得在验证中忽略。

## Minimal deterministic example

```json
{
  "schema_version": "1",
  "run_id": "normalized",
  "mode": "deterministic",
  "scenario_id": "complete-agent-walkthrough",
  "final_status": "completed",
  "events": [
    {
      "sequence": 1,
      "step_id": "preflight",
      "kind": "preflight_completed",
      "summary": "deterministic mode ready; no API key required",
      "capability_ids": ["configuration-handling"],
      "status": "succeeded",
      "metadata": {"requires_network": false}
    },
    {
      "sequence": 2,
      "step_id": "tool-use",
      "kind": "tool_called",
      "summary": "calculator called with safe arithmetic expression",
      "capability_ids": ["tool-invocation"],
      "status": "started",
      "metadata": {"tool_name": "calculator", "args_summary": "arithmetic expression"}
    }
  ],
  "coverage": [],
  "artifacts": []
}
```

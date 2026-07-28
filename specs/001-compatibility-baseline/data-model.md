# Data Model: AgentScope Compatibility Baseline

**Feature**: 001-compatibility-baseline | **Date**: 2026-07-28

本文档定义兼容性基线中所有数据产物的实体模型、字段和关系。

## Entity Overview

```text
VersionLock ─────────────────────────────────────────────
  │
  ▼
Module ──► Capability ──► ObservableBehavior
  │            │
  │            ├──► dependencies: Capability[]
  │            ├──► test_fixture_ids: string[]
  │            └──► examples: ExampleReference[]
  │
  ├──► DependencyMap (derived from Capability.dependencies)
  │
  └──► ExclusionList (capabilities explicitly unsupported)

TraceSchema ──► NormalizationRules
```

## Version Lock (`version-lock.json`)

记录兼容目标的上游 AgentScope 精确版本信息。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `schema_version` | string | ✅ | Schema 版本号（如 `"1.0.0"`） |
| `repository_url` | string | ✅ | 上游仓库 HTTPS URL |
| `release_tag` | string | ✅ | Release/Tag 名称（如 `"v1.0.0"`） |
| `commit_hash` | string (40-char hex) | ✅ | 完整 Git commit SHA-1 |
| `python_version` | string | ✅ | Python 版本约束（如 `">=3.10"`） |
| `core_dependencies` | object | ✅ | `{ "package_name": "version_spec" }` |
| `generated_date` | string (ISO 8601) | ✅ | 基线生成日期 |
| `generated_by` | string | ❌ | 生成工具/人员标识 |

**Validation Rule**: `commit_hash` MUST 匹配 `^[a-f0-9]{40}$`。

**Identity**: 每份基线只有一个 Version Lock 文件，无 identity 字段。

---

## Capability (`api-inventory.json`)

AgentScope 的一个可识别、可追踪的公开能力单元。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `capability_id` | string (kebab-case) | ✅ | 唯一标识符，如 `"message-msg"` |
| `category` | string | ✅ | 所属功能域，如 `"messaging"`、`"model"`、`"tool"` |
| `module` | string | ✅ | 所属 AgentScope 模块名 |
| `python_import_path` | string | ✅ | 完整 Python import 路径 |
| `symbol_name` | string | ✅ | 符号名称 |
| `symbol_type` | enum | ✅ | 见 SymbolType 枚举 |
| `description` | string | ✅ | 功能说明 |
| `source_location` | string | ✅ | 源码文件路径 + 行号，如 `"agentscope/message.py:45"` |
| `doc_location` | string \| null | ❌ | 文档链接 |
| `is_public_api` | boolean | ✅ | 是否属于公开 API |
| `has_runtime_behavior` | boolean | ✅ | 是否存在运行时行为 |
| `dependencies` | string[] | ✅ | 依赖的 capability_id 列表 |
| `observable_behaviors` | ObservableBehavior[] | ❌ | 可观察行为描述（仅 `has_runtime_behavior=true` 时） |

**Identity**: `capability_id` 全局唯一。

**SymbolType 枚举**:

| 值 | 描述 |
|----|------|
| `module` | Python 模块 |
| `class` | 类 |
| `function` | 顶层函数 |
| `method` | 类方法 |
| `protocol` | 协议/Protocol/ABC |
| `enum` | 枚举类型 |
| `event` | 事件类型 |
| `exception` | 异常类 |
| `serialized_structure` | 可序列化的数据结构（dataclass/Pydantic model/TypedDict） |
| `decorator` | 装饰器函数 |
| `extension_point` | 扩展点（hook、plugin interface 等） |

---

## Observable Behavior

一项能力的可观察行为详细信息。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `input_params` | ParamSpec[] | ❌ | 输入参数列表 |
| `param_defaults` | object | ❌ | `{ "param_name": "default_value_json" }` |
| `return_type` | string | ❌ | 返回值类型描述 |
| `serialization_format` | string | ❌ | JSON schema 引用或描述 |
| `event_types` | string[] | ❌ | 发出的事件类型列表 |
| `event_order` | string[] | ❌ | 事件发布顺序 |
| `streaming_order` | string | ❌ | 流式 chunk 顺序描述 |
| `state_changes` | StateChange[] | ❌ | 状态变化列表 |
| `tool_calls` | boolean | ❌ | 是否发起 Tool 调用 |
| `memory_writes` | boolean | ❌ | 是否写入 Memory |
| `exceptions` | string[] | ❌ | 可能抛出的异常类型 |
| `timeout_behavior` | string | ❌ | 超时行为描述 |
| `cancellation_behavior` | string | ❌ | 取消行为描述 |
| `side_effects` | string[] | ❌ | 副作用描述列表 |

**ParamSpec**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `name` | string | ✅ | 参数名称 |
| `type` | string | ✅ | 参数类型（Python annotation） |
| `required` | boolean | ✅ | 是否必填 |
| `default` | string \| null | ❌ | 默认值的 JSON 表示 |

**StateChange**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `attribute` | string | ✅ | 变化的属性名 |
| `from` | string | ❌ | 变化前状态 |
| `to` | string | ✅ | 变化后状态 |

---

## Capability Matrix Entry (`capability-matrix.json`)

能力在兼容矩阵中的条目，扩展 Capability 的基础信息。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `capability_id` | string | ✅ | 引用 `api-inventory.json` 中的能力 ID |
| `category` | string | ✅ | 所属功能域 |
| `upstream_symbol` | string | ✅ | AgentScope 中的符号全名 |
| `source_location` | string | ✅ | 源码位置 |
| `description` | string | ✅ | 简要功能描述 |
| `dependencies` | string[] | ✅ | 依赖的其他 capability_id |
| `priority` | enum | ✅ | 见 Priority 枚举 |
| `target_level` | enum | ✅ | 见 CompatibilityLevel 枚举 |
| `status` | enum | ✅ | 见 CapabilityStatus 枚举 |
| `test_fixture_ids` | string[] | ❌ | 关联的测试场景 ID |
| `notes` | string | ❌ | 补充说明 |

**Priority 枚举**:

| 值 | 描述 |
|----|------|
| `MVP_REQUIRED` | 第一阶段 MVP 必须实现 |
| `CORE_REQUIRED` | 第二阶段核心能力 |
| `ADVANCED` | 第三阶段高级能力 |
| `DEFERRED` | 明确延期 |
| `INTENTIONALLY_UNSUPPORTED` | 明确不支持 |

**CompatibilityLevel 枚举**:

| 值 | 名称 | 定义 |
|----|------|------|
| `L0` | 尚未支持 | 尚未开始实现 |
| `L1` | 数据协议兼容 | 数据结构/序列化格式兼容 |
| `L2` | 核心运行行为兼容 | 核心流程外部可观察行为兼容 |
| `L3` | 公开 API 语义兼容 | 公开接口语义等价 |
| `L4` | 示例迁移兼容 | 官方示例可低成本迁移 |
| `L5` | 完整目标范围兼容 | 所有目标范围兼容 |

**CapabilityStatus 枚举**:

| 值 | 描述 |
|----|------|
| `NOT_ANALYZED` | 尚未分析 |
| `ANALYZING` | 分析中 |
| `SPECIFIED` | 已书写 specification |
| `IMPLEMENTING` | 实现中 |
| `PARTIAL` | 部分实现 |
| `COMPATIBLE` | 完全兼容 |
| `DEFERRED` | 已延期 |
| `UNSUPPORTED` | 明确不支持 |
| `BLOCKED` | 被依赖阻断 |

---

## Dependency Map (`dependency-map.json`)

能力间依赖关系的有向图。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `schema_version` | string | ✅ | Schema 版本号 |
| `nodes` | DepNode[] | ✅ | 图中所有节点 |
| `edges` | DepEdge[] | ✅ | 图中所有有向边 |
| `topological_order` | string[] | ✅ | 拓扑排序的建议实现顺序 |

**DepNode**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `capability_id` | string | ✅ | 能力 ID |
| `layer` | string | ✅ | 所属层：`foundation` \| `model` \| `tool` \| `agent` \| `extended` |
| `independent` | boolean | ✅ | 是否可独立实现 |

**DepEdge**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `from` | string | ✅ | 依赖方 capability_id |
| `to` | string | ✅ | 被依赖方 capability_id |
| `relation` | string | ✅ | 关系类型：`requires` \| `extends` \| `uses` |

---

## Example Reference (`example-inventory.json`)

AgentScope 官方示例清单。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `schema_version` | string | ✅ | Schema 版本号 |
| `examples` | ExampleEntry[] | ✅ | 示例条目列表 |

**ExampleEntry**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `example_id` | string (kebab-case) | ✅ | 示例唯一 ID |
| `title` | string | ✅ | 示例名称 |
| `description` | string | ✅ | 示例描述 |
| `source_path` | string | ✅ | 示例源码在 AgentScope 仓库中的路径 |
| `capabilities_used` | string[] | ✅ | 使用的 capability_id 列表 |
| `complexity` | enum | ✅ | `simple` \| `medium` \| `complex` |

---

## Trace Schema (`trace-schema.json`)

差分测试使用的标准 Trace 结构定义。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `schema_version` | string | ✅ | Schema 版本号 |
| `trace_id` | string | ❌ (generated) | Trace 唯一标识符 |
| `input` | object | ✅ | 输入参数 |
| `model_requests` | ModelRequest[] | ❌ | 模型请求记录 |
| `model_responses` | ModelResponse[] | ❌ | 模型响应记录 |
| `streaming_chunks` | StreamingChunk[] | ❌ | 流式分块记录 |
| `tool_calls` | ToolCallRecord[] | ❌ | Tool 调用记录 |
| `tool_results` | ToolResultRecord[] | ❌ | Tool 结果记录 |
| `events` | EventRecord[] | ❌ | 事件记录 |
| `memory_mutations` | MemoryMutation[] | ❌ | Memory 写入记录 |
| `state_transitions` | StateTransition[] | ❌ | 状态变化记录 |
| `errors` | ErrorRecord[] | ❌ | 错误记录 |
| `cancellation` | CancellationRecord | ❌ | 取消记录 |
| `final_result` | any | ❌ | 最终输出 |

详细子结构定义见 `contracts/trace-schema.schema.json`。

---

## Normalization Rules (`normalization-rules.json`)

差分比较时的字段归一化规则。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `schema_version` | string | ✅ | Schema 版本号 |
| `normalizable_fields` | NormalizationRule[] | ✅ | 允许标准化的字段 |
| `immutable_fields` | string[] | ✅ | 禁止忽略的字段（JSONPath 表达式） |

**NormalizationRule**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `field_path` | string | ✅ | JSONPath 表达式指向目标字段 |
| `normalization_strategy` | string | ✅ | 标准化策略：`placeholder` \| `order_normalize` \| `epsilon_compare` \| `remove` |
| `description` | string | ✅ | 标准化原因说明 |

**ImmutableFields（至少包含）**: 见 FR-015 定义。
- `$.events[*].type` (事件类型)
- `$.events[*].order` (事件顺序)
- `$.tool_calls[*].arguments` (Tool 参数)
- `$.tool_calls[*].name` (Tool 名称)
- `$.model_responses[*].message.role` (Message Role)
- `$.model_responses[*].finish_reason` (Finish Reason)
- `$.errors[*].category` (Error Category)
- `$.state_transitions[*]` (State Mutation)
- `$.cancellation` (Cancellation State)
- `$.side_effects[*]` (Side Effects)

---

## Exclusion List (`exclusion-list.json`)

明确排除的能力清单。

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `schema_version` | string | ✅ | Schema 版本号 |
| `exclusions` | ExclusionEntry[] | ✅ | 排除条目列表 |

**ExclusionEntry**:

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `exclusion_id` | string | ✅ | 排除条目 ID |
| `capability_name` | string | ✅ | 被排除的能力名称 |
| `reason` | string | ✅ | 排除原因 |
| `alternative` | string \| null | ❌ | 替代建议 |

## Field Stability & Versioning

所有 JSON 产物都包含 `schema_version` 字段，遵循语义化版本 `MAJOR.MINOR.PATCH`：

- **MAJOR**: 移除或重命名必填字段
- **MINOR**: 新增可选字段
- **PATCH**: 修正字段描述或枚举值（不改变含义）

新增顶层产物文件视为 MINOR 变更；从 FR-018 列表中移除产物文件视为 MAJOR 变更。

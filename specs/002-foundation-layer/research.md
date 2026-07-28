# Research: AgentScope Foundation Layer

**Feature**: 002-foundation-layer | **Date**: 2026-07-28

## Research Tasks & Decisions

### 1. serde 标签化枚举（Tagged Enum）策略

**Decision**: 使用 `#[serde(tag = "type")]` 外部标签模式序列化 ContentBlock 和 Event 的多态类型。ContentBlock 的标签值为字面量（"text"、"data"、"tool_call" 等），Event 的标签值为 `EventType` 枚举的字符串形式。

**Rationale**:
- Python Pydantic 实现使用 `Literal["text"] = "text"` 作为判别字段——`type: "text"` 是 JSON 中的一个 top-level key
- serde 的 `#[serde(tag = "type")]` 恰好产生同构的 JSON 结构 `{"type": "text", "text": "...", ...}`
- 序列化输出与 Python `.model_dump()` 的结果完全一致（经测试验证）

**Alternatives considered**:
- `#[serde(untagged)]` — 无 type 标签，依赖字段存在性判断（`{"text": "..."}`）。问题：TextBlock 和 ThinkingBlock 字段不同但可能误匹配；Python 输出明确有 type 字段
- `#[serde(tag = "type", content = "...")]` — adjacent tagging，产生 `{"type": "text", "content": {...}}`。问题：与 Pydantic 输出结构不同
- 手动实现 `Serialize`/`Deserialize` — 可行但增加大量样板代码，serde tag 机制已足够

**Implementation note**: 为处理未知 ContentBlock 类型，保留一个 `#[serde(other)]` catch-all variant；为处理 ThinkingBlock 的 provider 额外字段，使用 `#[serde(flatten)]` + `HashMap<String, serde_json::Value>` 模式。

### 2. ThinkingBlock 的 `extra="allow"` 等效实现

**Decision**: ThinkingBlock 结构体包含一个 `#[serde(flatten)] extras: HashMap<String, serde_json::Value>` 字段来捕获并透传 provider 特定字段（如 Anthropic 的 `signature`、`redacted_thinking_data`）。

**Rationale**:
- Python Pydantic 的 `ConfigDict(extra="allow")` 将未知字段直接透传
- Rust serde 等效方案是 `#[serde(flatten)]` 捕获额外键值到 `HashMap`
- 序列化时自动输出（反向透传），保证 round-trip
- 方案已验证可用于类似场景（OpenAI function calling 响应中的 provider 元数据）

**Alternatives considered**:
- `#[serde(untagged)]` with `serde_json::Value` catch-all — 丢掉类型安全
- 忽略 `#[serde(skip)]` — 违反兼容性要求
- 为每个 provider 单独定义 struct（AnthropicThinkingBlock、OpenAIThinkingBlock 等）— 过度工程，且新 provider 需要修改核心类型

### 3. 枚举值字符串序列化（StrEnum 等效）

**Decision**: 所有枚举（`ToolCallState`、`ToolResultState`、`EventType`、`ReplyFinishedReason`、`ErrorType`）使用 `#[serde(rename_all = "lowercase")]` 或显式 `#[serde(rename = "...")]` 实现蛇形字符串序列化。

**Rationale**:
- Python `StrEnum` 将成员值序列化为其 value（蛇形字符串），如 `ToolCallState.PENDING` → `"pending"`
- serde 的 `rename_all = "snake_case"` 不完全相同——它转换 Rust 的 PascalCase 为 snake_case
- 对于直接匹配蛇形值的枚举（如 `ReplyFinishedReason::COMPLETED → "completed"`），使用 `rename_all = "lowercase"` 是最简方案
- 对于值不规则的枚举（如 `ErrorType::RATE_LIMIT → "rate_limit"`），使用 `rename_all = "snake_case"` 或显式 rename

**Implementation note**: `ToolCallState::PENDING` 需显式 rename 为 `"pending"`（全小写），因为在 Rust 枚举 variant 中使用 PascalCase 是惯例。

### 4. 联合类型（Union Types）Rust 等效

**Decision**: ContentBlock 的联合类型（`TextBlock | ThinkingBlock | ...`）和 AgentEvent 的联合类型使用 `enum` 表示，配合 serde 的 tagged enum 序列化。方法参数中的联合类型（如 `hint: str | list[TextBlock | DataBlock]`）使用专用 enum 或第三方 `either` crate。

**Rationale**:
- Python 的 `TypeAlias = A | B | C` 是判别联合类型，Rust 的 `enum` 是最自然等效
- 对于 `str | list[Something]` 这种值级别（非 tagged）的联合，使用：
  - `#[serde(untagged)] enum HintContent { Text(String), Blocks(Vec<ContentBlock>) }` 或
  - 备选：先尝试反序列化为一种类型，失败再试另一种（需自定义 Deserialize）

**Alternatives considered**:
- Trait object (`Box<dyn ContentBlockTrait>`) — 丢失具体类型信息，序列化困难
- 泛型 `Msg<C: ContentBlock>` — API 复杂度过高，不必要

### 5. Tool Call input 字段处理

**Decision**: `ToolCallBlock.input` 字段保持为 `String`（原始 JSON 文本），不做运行时 JSON 解析。仅在需要结构化读取时由上层（Tool 执行层）解析。

**Rationale**:
- Python 实现中 `ToolCallBlock.input` 类型为 `str`，不做解析
- 流式构建时 input 逐步拼接，中间态不是合法 JSON
- 在 Foundation 层做 JSON 解析会产生不必要的开销和错误风险

**Implementation note**: 未来可添加一个便捷方法 `fn input_json(&self) -> Result<serde_json::Value, serde_json::Error>`。

### 6. DataBlock base64 拼接策略

**Decision**: DataBlock 的流式增量使用二进制字节拼接（而非字符串拼接），每个 delta 先解码为 bytes，拼接后重新编码为 base64 字符串。

**Rationale**:
- Python 实现中明确强调："Each delta is an independently base64-encoded chunk (with its own padding); naive string concat would corrupt the byte stream. Decode, concat bytes, re-encode."
- Rust 实现完全遵循相同逻辑：先 `base64 decode` → `Vec<u8>` 拼接 → `base64 encode`
- Rust 的 `base64` crate（或 `data_encoding`）可高效处理

### 7. `use_enum_values=True` 的 serde 等效

**Decision**: 所有使用 `ConfigDict(use_enum_values=True)` 的 struct（如 `ToolCallBlock`、`ToolResultBlock`、所有 Event 类）通过 `#[serde(rename_all = "lowercase")]` + 字段类型本身的 serde 配置来实现等效行为。

**Rationale**:
- Python 中 `use_enum_values=True` 使枚举字段序列化为其值（字符串）而非枚举成员对象
- serde 默认将枚举序列化为其字符串表示（配合 `rename_all`），效果相同
- 无需特殊处理，只要枚举本身配置了正确的 serde rename

### 8. `_generate_id` 和 `_generate_timestamp` 的 Rust 等效

**Decision**: 使用 `uuid::Uuid::new_v4().as_simple().to_string()` 生成 32 字符 hex ID，使用 `chrono::Utc::now().to_rfc3339()` 生成 ISO 8601 时间戳。

**Rationale**:
- Python 的 `uuid.uuid4().hex` 产生 32 字符无连字符 UUID hex 字符串
- Rust 的 `uuid` crate 的 `as_simple()` 产生相同格式
- Python 的 `datetime.now().isoformat()` 产生 ISO 8601 格式
- Rust 的 `chrono` crate 的 `to_rfc3339()` 产生 RFC 3339 格式（ISO 8601 的子集）
- 细微差异：chrono 默认包含时区信息 `+00:00`，而 Python `.isoformat()` 不包含。在差分测试中可使用归一化规则处理，或在 Foundation 层使用自定义格式化函数

### 9. ContentBlock 和 AgentEvent 的 `type` 判别字段实现

**Decision**: 使用 serde 的 internally tagged enum 模式。每个 struct 的 `type` 字段通过 `#[serde(tag = "type")]` 自动注入，不显式存储在 struct fields 中，仅通过 serde 导出/导入。

**Rationale**:
- Python Pydantic 使用 `type: Literal["text"] = "text"` 作为显式字段
- serde tagged enum 自动处理 tag 的序列化和反序列化，产出的 JSON 结构完全相同
- 避免了手动维护 type 字段与 enum variant 的双重映射

**Implementation note**: 对于 Rust 侧的运行时类型检查（如 `get_content_blocks("text")`），可以通过 enum 的 variant 判别或手动实现 `block_type()` 方法返回字符串。

### 10. 黄金快照差分测试基础设施

**Decision**: 使用 Python 脚本生成每种类型的黄金快照 JSON 文件，存储在 `tests/compatibility/fixtures/` 中。Rust 测试读取这些文件并验证 Rust 序列化输出一致（经归一化规则处理后）。

**Rationale**:
- 遵循宪法第七条（Trace 是核心验收产物）
- 黄金快照作为"单一事实来源"——Python 输出是兼容性基准
- 归一化规则：时间戳（`created_at`、`finished_at`）、UUID（`id`）在比较前替换为占位符
- 差分测试框架从 Feature 001 的 `trace-schema.json` 和 `normalization-rules.json` 中继承标准化规则

## Technology Stack Summary

| 用途 | 工具/技术 | 理由 |
|------|----------|------|
| 序列化/反序列化 | serde + serde_json | Rust 生态标准，tagged enum 完美匹配 Pydantic 输出 |
| UUID 生成 | uuid (v4, simple format) | 匹配 Python `uuid4().hex` |
| 时间戳 | chrono | ISO 8601 / RFC 3339 格式 |
| base64 | base64 crate | DataBlock 流式拼接需要 decode/encode |
| 测试 | cargo test | 标准 Rust 测试框架 |
| 兼容性验证 | JSON 黄金快照 + 归一化脚本 | Feature 001 定义的标准化规则 |
| JSON Schema (可选) | schemars | 为公开类型生成 JSON Schema |
| 占位 dict 类型 | serde_json::Value / HashMap | 替换 Python 的 `dict[str, Any]` |

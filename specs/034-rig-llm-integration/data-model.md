# Phase 1 Data Model: Rig LLM Provider Integration

**Feature**: 034-rig-llm-integration | **Date**: 2026-08-17
**上游基准**: Python AgentScope（数据模型对齐，Feature 003/004/005 交付，**本特性公共数据模型零变更**）

## 实体总览

```text
# ── 公共数据协议（已存在，agent_scope_model / agent_scope_message / agent_scope_embedding，零变更）──
ChatModel trait (model_trait.rs)              # Arc<dyn ChatModel>，agent 引擎消费点零感知
├── call_api(model_name, &[Msg], tools, tool_choice) -> Result<ModelCallResult, ModelError>
├── call() 重试循环（max_retries/retry_delay/retryable_errors）
├── generate_structured_output(messages, schema) -> Result<StructuredResponse, ModelError>
└── count_tokens / validate_tool_choice / context_size / list_models
ModelCallResult = Complete(ChatResponse) | Stream(Pin<Box<dyn Stream<Item=Result<ChatResponse,ModelError>>+Send>>)
ChatResponse  { content: Vec<ContentBlock>, is_last, id, created_at, type="chat_response",
                usage: Option<ChatUsage>, finished_reason: Completed|Interrupted,
                metadata: Map, tool_call_id_map: Map<tc_idx, provider_id> (skip 序列化) }
ContentBlock  (serde tag="type"): Text | Thinking | Hint | Data | ToolCall | ToolResult | Unknown
ToolChoice    { mode: String (auto|none|required|<tool_name>), tools: Option<Vec<String>> }
ModelError / ModelErrorKind (ApiConnection|ApiTimeout|RateLimit|InternalServer|BadRequest|Authentication|...)
Msg           { name, content: Vec<ContentBlock>, role, id, metadata, created_at, usage,
                finished_at, finished_reason, structured_output, error }
EmbeddingModel trait: embed(Vec<EmbeddingInput>) -> Result<EmbeddingResponse, EmbeddingError>
EmbeddingInput = Text(String) | DataBlock(String)
EmbeddingResponse { embeddings: Vec<Vec<f32>>, usage: EmbeddingUsage }
EmbeddingModelCard { name, dimensions: u32, supports_multimodal: bool }

# ── 新增（agent_scope_rig 内部，非持久化，不进任何存档/序列化边界）──
RigChatModel         # 实现 ChatModel，持有 Arc<dyn RigChatBackend> + 公共配置
RigChatModelConfig   # api_key/base_url/model_name/stream/parameters/max_retries/retry_delay/context_size
RigParameters        # max_tokens/temperature/top_p/top_k/seed/stop/thinking_budget（对齐 DashScope 公共子集）
RigProviderKind      # OpenAi | Anthropic | DeepSeek（决定构造与能力位）
RigProviderCapabilities { supports_thinking, thinking_tool_choice_incompatible, supports_embedding }
RigChatBackend       # 对象安全桥接 trait（async_trait，返回具体类型）
RigEmbeddingBackend  # 对象安全 embedding 桥接 trait
RigEmbeddingModel    # 实现 agent_scope_embedding::EmbeddingModel（OpenAI）
```

## 核心声明：公共数据模型零变更

本特性的所有改动**不触及公共数据模型**——`agent_scope_model` / `agent_scope_message` / `agent_scope_embedding` 的类型、serde 布局、trait 契约全部保持不变：

| 模型 | 现状 | 本特性 |
|------|------|--------|
| `ChatModel` trait | `Arc<dyn ChatModel>`，agent 引擎消费 | **零变更**——`RigChatModel` 只是新实现者 |
| `ChatResponse` / `ContentBlock` / `Msg` / `ToolChoice` | serde 布局对齐 Python | **零变更**——映射层在 `agent_scope_rig` 内部产出这些既有类型 |
| `ModelError` / `ModelErrorKind` | 六分类 + 重试语义 | **零变更**——rig 错误在 `agent_scope_rig` 内映射到现有分类 |
| `EmbeddingModel` trait / `EmbeddingResponse` / `EmbeddingModelCard` | Feature 011 交付 | **零变更**——`RigEmbeddingModel` 实现既有 trait |

依据：FR-005（保留 trait）、FR-004（可观察行为保持）、宪法第十二条（稳定数据协议）。会话存档/兼容矩阵无需迁移。

## 新增数据模型（agent_scope_rig 内部）

### RigChatModelConfig（构造配置）

字段对齐现有 `DashScopeChatModel` 公共字段（模型.rs 结构），保证"相当的开销"迁移（FR-003）：

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | `String` | — | 从 `RigChatModel::openai(api_key, model)` 传入 |
| `base_url` | `String` | provider 官方默认 | `with_base_url()` 可覆盖（mock/代理端点） |
| `model_name` | `String` | — | `ChatModel::model_name()` 返回值 |
| `stream` | `bool` | `true` | `with_stream()` 覆盖；`ChatModel::stream_enabled()` |
| `parameters` | `RigParameters` | 全 None | 生成参数；`with_parameters()` 或逐字段 setter |
| `max_retries` | `u32` | `3` | 继承 `ChatModel::call` 重试 |
| `retry_delay` | `f64` | `1.0` | 同上 |
| `context_size` | `i64` | provider 默认（OpenAI 131072 / Anthropic 200000 / DeepSeek 131072） | 覆盖模型窗口 |
| `provider` | `RigProviderKind` | 由构造器决定 | 内部只读 |

**校验规则**：`api_key` 非空（构造时校验，空值 → `ModelError::ValidationError`，不 panic）；`base_url` 若提供须为合法 `http(s)://` URL（构造时校验）；`retry_delay` 按 `ChatModel::call` 的 clamp 语义（0.0–600.0）。

### RigProviderKind + 能力位

```rust
pub enum RigProviderKind { OpenAi, Anthropic, DeepSeek }

impl RigProviderKind {
    fn capabilities(&self) -> RigProviderCapabilities;
}
pub struct RigProviderCapabilities {
    pub supports_thinking: bool,                  // OpenAi(o 系)=true / Anthropic=true / DeepSeek(reasoner)=true
    pub thinking_tool_choice_incompatible: bool,  // 实现时按 provider 版本定值（research 验证点 5）
    pub supports_embedding: bool,                 // 仅 OpenAi=true
}
```

- 能力位在构造时固定，随 `RigChatBackend` 暴露（`fn capabilities(&self) -> RigProviderCapabilities`）。
- **Thinking 与 tool_choice 互斥守卫**（决策 9）：请求带 `ToolChoice::required` 且 `thinking_tool_choice_incompatible=true` 时，`agent_scope_rig` 将 `tool_choice` 降级为 `auto` 并发出 `tracing::info!` 事件（不静默，宪法第五条/第十四条）。

### RigChatBackend / RigEmbeddingBackend（对象安全桥接）

```rust
#[async_trait::async_trait]
pub trait RigChatBackend: Send + Sync {
    fn capabilities(&self) -> RigProviderCapabilities;
    async fn completion(&self, request: rig::completion::CompletionRequest)
        -> Result<rig::completion::CompletionResponse, rig::completion::CompletionError>;
    async fn stream(&self, request: rig::completion::CompletionRequest)
        -> Result<rig::completion::StreamingCompletionResponse, rig::completion::CompletionError>;
}

#[async_trait::async_trait]
pub trait RigEmbeddingBackend: Send + Sync {
    fn ndims(&self) -> usize;
    async fn embed_texts(&self, texts: Vec<String>)
        -> Result<Vec<Vec<f32>>, rig::providers::openai::OpenAiEmbeddingError>;  // 或统一 EmbeddingError
    async fn embed_text(&self, text: &str) -> Result<Vec<f32>, ...>;
}
```

- **不变量**：`RigChatBackend` 的输入/输出均为 rig 具体类型（`CompletionRequest`/`CompletionResponse`/`StreamingCompletionResponse`），不暴露泛型；映射层（message/tools/stream/structured/error）在 `RigChatModel` 侧消费这些具体类型。rig 类型**不越过** `agent_scope_rig` 边界（宪法第十一条）。

### RigParameters（生成参数）

| 字段 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `max_tokens` | `Option<u32>` | None | → `CompletionRequest.max_tokens` |
| `temperature` | `Option<f64>` | None | → `CompletionRequest.temperature` |
| `top_p` | `Option<f64>` | None | provider 支持的经 `additional_params` 传（rig 无 top_p 顶层字段时） |
| `top_k` | `Option<u32>` | None | 同上（DeepSeek/部分模型） |
| `seed` | `Option<u64>` | None | 同上 |
| `stop` | `Option<Vec<String>>` | None | provider 支持时经 `additional_params` 传 |
| `thinking_budget` | `Option<u32>` | None | thinking 预算（Anthropic/DeepSeek 支持时） |
| `additional_params` | `serde_json::Map` | 空 | provider 特有参数兜底通道（`CompletionRequest.additional_params`） |

- **说明**：rig `CompletionRequest` 顶层有 `temperature`/`max_tokens`/`tool_choice`/`output_schema`；`top_p`/`top_k`/`seed`/`stop` 因 provider 而异，走 `additional_params`（rig 透传给 provider 的 JSON 映射），实现时以各 provider 实际支持为准（research 验证点 1）。

## 状态转换

无新状态机。流式生命周期沿用 agent 既有模型：

```text
rig StreamedAssistantContent 流
  → stream.rs 增量转换
  → ChatResponse（is_last=false … is_last=true，usage 在流末聚合）
  → StreamAccumulator / react_loop / streaming_reactor（零变更）
```

## 序列化兼容性

- 公共类型零变更 → 既有会话存档/工具结果/事件流 100% 兼容（宪法第十二条）。
- `agent_scope_rig` 内部类型（`RigChatModelConfig`/`RigProviderCapabilities`/`RigChatBackend`）**不实现 serde**、不进入任何序列化边界——仅存在于进程内配置/桥接。
- 兼容性矩阵更新 `provider-*` 条目（DashScope 移除，登记 OpenAI/Anthropic/DeepSeek 能力覆盖与已知偏差，宪法第十八条）——见 [contracts/provider-adapter.md](contracts/provider-adapter.md)。

## 校验规则汇总

| 规则 | 来源 |
|------|------|
| 构造时 `api_key` 非空，否则 `ModelError::ValidationError`（不 panic） | FR-003 / 宪法第九条 |
| `base_url` 合法 `http(s)://`（若提供） | FR-003 |
| `retry_delay` 沿用 `ChatModel::call` clamp（0–600s） | 既有语义 |
| `ToolChoice::specific_tool(name)` 经 `validate_tool_choice` 校验工具存在 | 既有 trait 默认 |
| thinking+required 互斥降级 + tracing 事件 | 决策 9 / 宪法第五条 |
| `EmbeddingInput::DataBlock` 在非多模态模型 → `EmbeddingError::MultimodalNotSupported` | EmbeddingModel trait 契约 |

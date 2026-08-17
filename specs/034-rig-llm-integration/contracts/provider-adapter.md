# Contract: Provider Adapter（`agent_scope_rig` 公开适配契约）

**Feature**: 034-rig-llm-integration | **Date**: 2026-08-17

本契约定义 `agent_scope_rig` 对外的**公开 API** 与**可观察行为承诺**。它是用户/示例/文档迁移的依据，也是实现验收（quickstart.md）的契约源。

## 1. 公开构造入口

与现有 `agent_scope_dashscope` 的构造开销等价（FR-003）。`api_key` 与 `model_name` 必填；`base_url` 可选覆盖。

```rust
// chat
let model = agent_scope_rig::RigChatModel::openai(api_key, "gpt-4.1")
    .with_stream(true)
    .with_base_url("https://api.openai.com/v1");      // 可选，默认官方
let model = agent_scope_rig::RigChatModel::anthropic(api_key, "claude-sonnet-4-5");
let model = agent_scope_rig::RigChatModel::deepseek(api_key, "deepseek-chat");

// embedding（OpenAI 专用）
let embed = agent_scope_rig::RigEmbeddingModel::openai(api_key, "text-embedding-3-small");

// 参数（链式，可选）
.with_parameters(RigParameters { temperature: Some(0.7), ..Default::default() })
```

### 与现有构造器的等价映射

| 现有 `agent_scope_dashscope` | 迁移后 `agent_scope_rig` |
|------------------------------|---------------------------|
| `DashScopeChatModel::new(key, model)` | `RigChatModel::openai(key, model)`（示例统一 OpenAI） |
| `.with_stream(true)` | `.with_stream(true)`（同名同语义） |
| `.with_base_url(url)` | `.with_base_url(url)`（同名同语义，默认值不同） |
| `DashScopeParameters{max_tokens,temperature,top_p,top_k,seed,stop}` | `RigParameters`（同名同语义，经 `CompletionRequest`/`additional_params` 传递） |
| `DashScopeEmbeddingModel::new(key, model)` | `RigEmbeddingModel::openai(key, model)` |
| 环境变量 `DASHSCOPE_API_KEY` | `OPENAI_API_KEY`（示例统一） |

**默认值等价表**（构造后立即生效的可观察字段）：

| 字段 | DashScope 旧值 | Rig 新值 | 说明 |
|------|---------------|----------|------|
| `stream_enabled()` | `true` | `true` | 不变 |
| `max_retries` | `3` | `3` | 不变 |
| `retry_delay` | `1.0` | `1.0` | 不变 |
| `context_size` | `131072` | provider 默认（OpenAI 131072 / Anthropic 200000 / DeepSeek 131072） | 按模型窗口对齐 |
| `base_url` | `dashscope.../compatible-mode/v1` | provider 官方端点 | 归属不同服务商 |

## 2. Trait 实现契约

`RigChatModel` 实现 `agent_scope_model::ChatModel`（trait 契约见 model_trait.rs），逐方法承诺：

| `ChatModel` 成员 | `RigChatModel` 实现 |
|------------------|---------------------|
| `model_name()` | 返回构造时的 `model_name` |
| `stream_enabled()` | 返回 `config.stream` |
| `call_api(model_name, msgs, tools, tool_choice)` | 映射→rig `CompletionRequest`→`completion()` 或 `stream()`→转 `ModelCallResult`（见 [rig-mapping.md](rig-mapping.md)） |
| `max_retries()` / `retry_delay()` / `context_size()` | 返回配置值 |
| `retryable_errors()` | 返回 `[RateLimit, InternalServer, ApiConnection, ApiTimeout]`（对齐现状可重试面） |
| `generate_structured_output()` | **覆写**：优先 rig `output_schema` 原生路径，回退 tool-calling bypass（见 [rig-mapping.md](rig-mapping.md) §6） |
| `count_tokens()` / `validate_tool_choice()` / `list_models()` | 继承 trait 默认（不覆写） |

`RigEmbeddingModel` 实现 `agent_scope_embedding::EmbeddingModel`：`embed()`（Text→rig embed_text，DataBlock→`MultimodalNotSupported`）、`model_card()`（name=模型名，dimensions=rig ndims，supports_multimodal=false）。

## 3. 可观察行为承诺

迁移后**必须保持**（宪法第一条 / FR-004）的可观察行为：

1. **流式事件序列**：`reply_stream` 产出的事件顺序（Text/Thinking/ToolCall 的 Start/Delta/End、is_last、tool result、end-of-stream）与现状一致——由 [rig-mapping.md](rig-mapping.md) §5 的顺序契约保证，并由确定性测试验证。
2. **工具调用生命周期**：`ToolCallBlock`（id/name/input）→ `ToolResultBlock` 的往返在 agent 引擎内不变；`tool_call_id_map`（`tc_{idx}` → provider id）在流末正确填充。
3. **错误分类**：`ModelErrorKind` 六分类语义不变（网络/超时/限流/服务端/参数/认证），重试语义（`ChatModel::call`）不变。
4. **结构化输出**：`StructuredResponse` 结构不变；JSON repair 回退保留。
5. **Trace 协议**：`ChatResponse` 的 `response_type="chat_response"`、`finished_reason`（Completed/Interrupted）、`usage`、`metadata` 字段语义不变。

## 4. Provider 能力矩阵与已知限制（宪法第五条：显式登记，不静默）

| 能力 | OpenAI | Anthropic | DeepSeek | 备注 |
|------|--------|-----------|----------|------|
| 聊天补全 | ✅ | ✅ | ✅ | 全部 |
| 流式 | ✅ | ✅ | ✅ | rig 原生 |
| 工具调用 | ✅ | ✅ | ✅ | rig 原生 |
| 结构化输出（output_schema） | ✅ | ✅ | ✅ | 按 provider 支持度，否则回退工具 bypass |
| thinking / 推理内容 | ✅ o 系 | ✅ extended thinking | ✅ reasoner | 均映射为 `ThinkingBlock` |
| embedding | ✅ `text-embedding-3-*` | ❌ | ❌ | **仅 OpenAI**；Anthropic/DeepSeek 构造 `RigEmbeddingModel` → 编译期不存在，文档明确 |
| thinking 与 `tool_choice=required` 并发 | ✅ 无互斥 | ⚠️ 按 provider 版本（若互斥则降级 auto + tracing 事件） | ✅ 无互斥 | 能力位 `thinking_tool_choice_incompatible` 定值见 research 验证点 5 |
| DashScope 特有 `enable_search` | — | — | — | **不迁移**，已知限制（spec Assumptions） |

**已登记偏差**（DashScope 特有能力，不视为回归）：

- `enable_search`（百炼联网搜索）：新 provider 不提供等价能力，作为 provider 差异记录。
- `repetition_penalty`：非三家通用参数，不暴露顶层 API；如需经 `additional_params` 透传由用户自行处理（不承诺等价）。
- qwen 专用参数：不迁移。

## 5. 兼容性登记（宪法第十八条）

- 框架兼容等级：维持 **L2（核心行为）+ L3（公开 API 语义）**，不降级。
- `capability-matrix.json` 的 `provider-*` 条目更新：移除 DashScope 条目，新增/改写 OpenAI/Anthropic/DeepSeek 覆盖与上表已知限制。
- 迁移是 provider 替换（服务商+模型不同），**LLM 实际输出内容的自然差异**不属框架兼容性问题（注册为 provider 选择差异）。

## 6. 边界与不变量

- `rig` 类型（`CompletionRequest`/`CompletionResponse`/`Message` 等）**不越过** `agent_scope_rig` 公开边界；对外只见 `ChatModel`/`EmbeddingModel` 既有类型（宪法第十一条/第十二条）。
- `RigChatModelConfig`/`RigProviderCapabilities`/`RigChatBackend` 不实现 serde、不进入任何序列化边界。
- `#![deny(unsafe_code)]`；无新 panic 路径（构造校验走 `Result`/`ValidationError`）。
- 无新 spawn/新 channel；rig 流在 agent 既有取消/超时语义内消费。

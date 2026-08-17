# 参考:模型抽象与 rig Provider(`agent_scope_model` / `agent_scope_rig`)

> 详细 API 参考:`ChatModel` trait、`ModelCallResult`、`ChatResponse`、`StreamAccumulator`、`ToolChoice`,以及 `RigChatModel`(OpenAI / Anthropic / DeepSeek)的配置与参数。

## 1. `ChatModel` trait(抽象层)

所有聊天模型 Provider 的统一接口(`async_trait`):

| 方法 | 类别 | 说明 |
|------|------|------|
| `model_name() -> &str` | 必需 | 模型标识符(如 `"qwen3.7-plus"`) |
| `stream_enabled() -> bool` | 必需 | 是否默认流式 |
| `call_api(...) -> Result<ModelCallResult, ModelError>` | 必需 | Provider 特定 API 实现(唯一需要写网络代码的方法) |
| `max_retries() -> u32` | 可覆盖 | 默认 `3` |
| `retry_delay() -> f64` | 可覆盖 | 默认 `1.0` |
| `retryable_errors() -> &[ModelErrorKind]` | 可覆盖 | 默认空(不重试) |
| `context_size() -> i64` | 可覆盖 | 默认 `32768` |
| `call(...)` | 默认 | **调用入口**:包装 `call_api` 的重试循环 |
| `count_tokens(...)` | 默认 | bytes/4 启发式;Provider 可覆盖为精确分词 |
| `generate_structured_output(...)` | 默认 | 结构化输出(JSON mode) |

**重试语义**:`call()` 最多额外执行 `max_retries` 次,仅当错误的 `kind()` 命中 `retryable_errors()`;全部失败返回 `ModelError::RetryExhausted { attempts, last_error, provider }`。

## 2. `ModelCallResult`:流式与非流式统一返回

```rust
pub enum ModelCallResult {
    Complete(ChatResponse),
    Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>),
}
```

`stream_enabled()` 为 true 时 `call()` 返回 `Stream`;流中每个 `ChatResponse` 是增量(Delta),`is_last` 标记末尾。

## 3. `ChatResponse`

| 字段 | 说明 |
|------|------|
| `content: Vec<ContentBlock>` | 响应内容块(text/thinking/tool_call) |
| `is_last: bool` | 流式序列最后一块 |
| `id` / `created_at` | 响应 UUID 与 RFC 3339 时间戳 |
| `usage: Option<ChatUsage>` | `input_tokens`/`output_tokens`/`time`/缓存统计 |
| `finished_reason: FinishedReason` | `completed`(默认)/ `interrupted` |
| `metadata` | Provider 附加元数据 |

Provider 可用构建器方法 `append_text` / `append_thinking` / `append_tool_call` 按 `block_id` 增量合并。

## 4. 直接调用模型(绕过 Agent)

```rust
use agent_scope_rig::RigChatModel;
use agent_scope_model::{ChatModel, StreamAccumulator};
use agent_scope_message::factory::user_msg;
use futures::StreamExt;

let model = RigChatModel::openai(api_key, "qwen3.7-plus")?;

// 非流式
let messages = vec![user_msg("user", "你好").expect("valid user message")];
if let ModelCallResult::Complete(resp) = model.call(&messages, None, None).await? {
    for block in &resp.content { /* ... */ }
}

// 流式 + 累积
if let ModelCallResult::Stream(mut stream) = model.call(&messages, None, None).await? {
    let mut acc = StreamAccumulator::new();
    while let Some(chunk) = stream.next().await {
        let delta = chunk?;
        acc.append_chat_response(&delta);
    }
    let full = acc.build();
}
```

## 5. `StreamAccumulator`:O(n) 流式累积

`new()` → 对每个增量 `append_chat_response(&delta)` → 结束后 `build()` 得到合并后的完整 `ChatResponse`。内部按块类型分缓冲,避免 O(n²) 拼接。

## 6. `ToolChoice`

| 构造器 | 说明 |
|--------|------|
| `auto()` | 默认,模型自行决定 |
| `none()` | 禁止工具 |
| `required()` | 强制调用工具 |
| `specific_tool(name)` | 强制指定工具 |

`validate()` 对照可用工具名校验,非法时 `call()` 前置返回 `ValidationError`。

## 7. `RigChatModel`(Provider, rig-backed)

基于 [rig](https://github.com/0xPlaygrounds/rig) 框架的聊天模型实现,统一支持 OpenAI / Anthropic / DeepSeek。构造后可用链式方法配置:

| 构造器 | Provider | 说明 |
|--------|----------|------|
| `RigChatModel::openai(api_key, model)` | OpenAI | Chat Completions 协议 |
| `RigChatModel::anthropic(api_key, model)` | Anthropic | Messages API |
| `RigChatModel::deepseek(api_key, model)` | DeepSeek | OpenAI-compatible 端点 |

| 链式方法 | 说明 |
|----------|------|
| `with_stream(bool)` | 是否默认流式(默认 `true`) |
| `with_base_url(impl Into<String>)` | 覆盖 base_url(backend 首次使用时校验合法 URL) |
| `with_parameters(RigParameters)` | 生成参数 |
| `with_max_retries(u32)` / `with_retry_delay(f64)` | 重试配置(默认 `3` / `1.0`) |
| `with_context_size(i64)` | 上下文窗口 token 数(按 provider 定默认:OpenAI/DeepSeek 131072、Anthropic 200000) |

构造返回 `Result<Self, ModelError>`,api_key 为空 → `ValidationError`。

**可重试错误**:`ApiConnection`/`ApiTimeout`/`RateLimit`/`InternalServer` 自动重试;401/400 等立即返回。

```rust
let model = RigChatModel::openai(api_key, "qwen3.7-plus")?
    .with_base_url("https://your-proxy.example.com/v1")
    .with_stream(true)
    .with_parameters(RigParameters::new().with_temperature(0.7).with_max_tokens(2048));
```

**Provider 能力差异**(见能力矩阵 `specs/001-compatibility-baseline/capability-matrix.json`):
- **embedding 仅 OpenAI**(`RigEmbeddingModel::openai`,`text-embedding-3-*`);Anthropic/DeepSeek 不构造 embedding。
- **thinking**:OpenAI o 系、Anthropic extended thinking、DeepSeek reasoner 均映射为 `ThinkingBlock`;Anthropic 的 extended thinking 与 `tool_choice=required` **互斥**——能力位 `thinking_tool_choice_incompatible=true`,`call_api` 自动降级 `auto` 并 `tracing::info!`(不静默)。
- DashScope 特有 `enable_search` **不迁移**(已知限制)。

## 8. `RigParameters`

| 参数 | 类型 | 说明 |
|------|------|------|
| `max_tokens` | `Option<u64>` | 最大生成 token 数 |
| `temperature` | `Option<f64>` | 采样温度(0–2) |
| `top_p` / `top_k` | `Option<f64>` / `Option<u64>` | 核采样 / Top-K(经 `additional_params` 透传) |
| `seed` | `Option<u64>` | 随机种子 |
| `stop` | `Option<Vec<String>>` | 停止序列 |
| `thinking_budget` | `Option<u64>` | 思考 token 预算(经 `additional_params` 透传) |
| `additional_params` | `Option<serde_json::Value>` | 用户兜底参数,合并进请求体 |

提供构建器方法 `with_max_tokens` / `with_temperature` / `with_top_p` / `with_top_k` / `with_seed` / `with_stop` / `with_thinking_budget`。thinking 模式按 provider 模型启用(如 OpenAI `gpt-5-*` / Anthropic extended thinking / DeepSeek `deepseek-reasoner`),经 `thinking_budget` 或 provider 参数配置,流式返回 `ThinkingBlock` delta。

## 9. `RigEmbeddingModel`

```rust
use agent_scope_embedding::EmbeddingInput;
use agent_scope_rig::RigEmbeddingModel;

let emb = RigEmbeddingModel::openai(api_key, "text-embedding-3-small")?;
let resp = emb.embed(vec![EmbeddingInput::Text("你好".into())]).await?;
```

维度由 `ndims()` 探知(`model_card().dimensions`)。API key 为空时构造返回 `ValidationError`(不 panic)。**仅 OpenAI 支持 embedding**;Anthropic/DeepSeek 无对应构造入口。

## 10. 凭据配置(已核实事实)

- crate **不自行读取环境变量**——凭据由调用方显式传入(分层设计)。
- 惯例:应用入口 `dotenv::dotenv().ok()` 加载 `.env`(含 `DEFAULT_API_KEY=sk-...`),经 clap 参数或环境变量读入,再传入 `RigChatModel::openai`(或 `anthropic` / `deepseek`)。

## 11. 错误

| 错误 | 触发条件 |
|------|----------|
| `ModelError::ApiError { status, message, provider }` | Provider HTTP 错误;`kind()` 按状态码分类(401→Authentication、429→RateLimit、5xx→InternalServer) |
| `ModelError::RetryExhausted` | 可重试错误耗尽 |
| `ModelError::Cancelled` | 调用被 CancellationToken 取消 |
| `ModelError::ValidationError` | 参数校验失败(如 tool_choice 指定不存在的工具) |
| `ModelError::UnsupportedFeature { feature, provider }` | Provider 不支持该能力 |
| `ModelError::StructuredOutputError` | 结构化输出解析失败或流式下用默认实现 |

## 12. 自定义 Provider

实现 `ChatModel` 的三个必需方法(`model_name`/`stream_enabled`/`call_api`),按需覆盖重试配置,以 `Arc<dyn ChatModel>` 注入 Agent:

```rust
let model: Arc<dyn ChatModel> = Arc::new(MyProvider::new(...));
```

`call_api` 返回 `ModelError`;用 `ModelError::ApiError { status, message, provider }` 包装 HTTP 错误即可自动获得 `kind()` 分类。

> **注意**:`generate_structured_output` 默认通过 tool-calling 旁路实现,**不支持流式调用下的结构化输出**(返回 `StructuredOutputError`)。

# DashScope Provider

> 一句话定位：阿里云百炼（DashScope）的 `ChatModel`/`EmbeddingModel` 参考实现——通过 OpenAI 兼容端点接入 Qwen 系列模型，是仓库当前唯一内置的模型 Provider。

## 1. 模块概述 (Overview)

本模块对应 `agent_scope_dashscope` crate，位于 Provider 层（实现 `agent_scope_model` 的 `ChatModel` trait 与 `agent_scope_embedding` 的 `EmbeddingModel` trait）。聊天走 OpenAI 兼容端点 `/compatible-mode/v1/chat/completions`；Embedding 走 Text Embedding API。

**适用场景**：使用 Qwen 系列模型进行对话/推理（含 thinking 模式）、文本向量化（RAG 链路的 Embedding 环节）。

**前置阅读**：[模型抽象](./model.md)（trait 语义）；RAG 场景见 [RAG](./rag.md)。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `DashScopeChatModel`

公开字段结构体，构造后可直接修改字段或用链式方法配置：

| 成员 | 说明 | 默认值 |
|------|------|--------|
| `new(api_key, model_name)` | 构造：`api_key: impl Into<String>`，`model_name: impl Into<String>` | — |
| `base_url` | OpenAI 兼容端点 | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| `stream` | 是否默认流式 | `true` |
| `max_retries` / `retry_delay` | 重试次数 / 间隔秒 | `3` / `1.0` |
| `context_size` | 上下文窗口 token 数 | `131072` |
| `parameters` | 生成参数（见 2.2） | `DashScopeParameters::default()` |
| `extra_body` | 合并进每个请求体的额外字段 | 空 |
| `with_base_url(...)` / `with_stream(bool)` | 链式配置 | — |

**可重试错误**已配置为 `ApiConnection`/`ApiTimeout`/`RateLimit`/`InternalServer`——这些类别的错误由 `ChatModel::call()` 自动重试（最多 `max_retries` 次）；其余错误（如 401 认证失败、400 参数错误）立即返回。

### 2.2 `DashScopeParameters`

生成参数（`Option` 字段为 `None` 时不出现在请求体中）：

| 参数 | 类型 | 说明 |
|------|------|------|
| `max_tokens` | `Option<u32>` | 最大生成 token 数 |
| `temperature` | `Option<f64>` | 采样温度（0–2） |
| `top_p` / `top_k` | `Option<f64>` / `Option<u32>` | 核采样 / Top-K 采样 |
| `enable_search` | `bool`（默认 `false`） | 联网搜索增强（DashScope 扩展） |
| `enable_thinking` | `bool`（默认 `false`） | thinking/推理模式（流式返回 `reasoning_content`，转为 `ThinkingBlock`） |
| `thinking_budget` | `Option<u32>` | 推理 token 预算；`Some(n)` 限额，`None` 不限制（仅 thinking 开启时有意义） |
| `repetition_penalty` | `Option<f64>` | 重复惩罚，必须 > 0（`validate()` 校验） |
| `seed` | `Option<u64>` | 随机种子（[0, 2³¹-1]） |
| `stop` | `Option<Vec<String>>` | 停止序列 |

**参数约束**（`ParamError`）：`repetition_penalty` 必须为正；`enable_thinking=true` 与 `tool_choice="required"` 不兼容；`enable_search` 仅部分模型支持。

### 2.3 `DashScopeEmbeddingModel`

文本向量化实现（`agent_scope_embedding::EmbeddingModel` trait）：

- `new(api_key: String, model_card: EmbeddingModelCard)`——`model_card` 携带模型名、维度、是否支持多模态
- `with_cache(cache: Arc<dyn EmbeddingCache>)`——挂载响应缓存
- `with_base_url(...)`——自定义端点（默认 `https://dashscope.aliyuncs.com`）
- API key 为空时 `embed()` 返回 `EmbeddingError::ApiKeyMissing`（不 panic）

### 2.4 凭据配置（已核实事实）

crate **不自行读取环境变量**——凭据由调用方显式传入（分层设计）。仓库示例的惯例：

1. 仓库根 `.env` 文件含 `API_KEY=sk-...`（已被 `.gitignore` 忽略），程序入口 `dotenv::dotenv().ok();` 加载（`examples/chat.rs:388`）；
2. 多数示例经 clap `#[arg(short = 'k', long, env = "API_KEY")]` 注入 CLI 参数；
3. 例外：`examples/chat.rs` 仅支持 `-k`/`--api-key` 显式传参（无 `env` 属性，`chat.rs:40`）；
4. 示例默认模型名为 `qwen-plus`。

## 3. 快速示例 (Quick Example)

创建聊天模型的标准方式（示例共享库）：

<!-- source: examples/common.rs:L34-L36 -->
```rust
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}
```

thinking 模式变体（`enable_thinking = true` + 可选预算 + 强制流式）见 `examples/common.rs` L43 `create_model_with_thinking`；Embedding 模型的完整使用见 `examples/rag_test.rs`（L196 `run_ingest_test`）。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 配置生成参数

`DashScopeChatModel` 字段公开，直接修改 `parameters`：

```rust
let mut model = DashScopeChatModel::new(api_key, "qwen-plus");
model.parameters.temperature = Some(0.7);
model.parameters.max_tokens = Some(2048);
model.parameters.validate()?; // 可选：发送前校验参数约束
```

### 4.2 thinking（推理）模式

```rust
model.parameters.enable_thinking = true;
model.parameters.thinking_budget = Some(8192); // 或 None 不限预算
model.stream = true; // thinking 内容经流式 ThinkingBlock delta 返回
```

注意：thinking 模式与 `ToolChoice::required()` 不兼容（`ParamError::ThinkingNotCompatibleWithRequired`）。

### 4.3 自定义端点与额外请求字段

区域端点/代理/mock 测试用 `with_base_url`；`extra_body` 合并任意厂商扩展字段：

```rust
let model = DashScopeChatModel::new(api_key, "qwen-plus")
    .with_base_url("https://your-proxy.example.com/v1")
    .with_stream(false);
// model.extra_body.insert("vl_high_resolution_images".into(), json!(true));
```

### 4.4 Embedding 向量化

```rust
let card = EmbeddingModelCard { name: "text-embedding-v4".into(), dimensions: 1024, .. };
let emb = DashScopeEmbeddingModel::new(api_key, card).with_cache(cache);
let resp = emb.embed(vec![EmbeddingInput::Text("你好".into())]).await?;
```

Embedding CLI 参数惯例见 `examples/rag_test.rs` L35-L49（`--embedding-model` 默认取环境变量 `EMBEDDING_MODEL`，`--embedding-dims` 默认取 `EMBEDDING_DIMS`）。

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误 | 触发条件 |
|------|----------|
| `ModelError::ApiError { status, .. }` | DashScope 返回 HTTP 错误（401 凭据无效、429 限流、5xx 服务故障等，按状态码自动分类） |
| `ModelError::RetryExhausted` | 可重试错误重试耗尽（默认 4 次尝试后） |
| `ModelError::ValidationError` | `tool_choice` 非法等前置校验失败 |
| `EmbeddingError::ApiKeyMissing` | Embedding 调用时 API key 为空（不 panic，调用期返回） |
| `ParamError::RepetitionPenaltyMustBePositive` | `repetition_penalty <= 0` |
| `ParamError::ThinkingNotCompatibleWithRequired` | thinking 模式与强制工具调用并用 |
| `ParamError::EnableSearchNotSupported(model)` | 当前模型不支持联网搜索 |

**不支持的能力**：遵循 `ChatModel` 约定，未实现的能力显式返回 `ModelError::UnsupportedFeature { feature, provider }`，不做伪兼容。

**常见问题**：

- *API key 为空/错误*：聊天侧由服务端返回 401（`ApiError`，`kind()=Authentication`，不重试）；Embedding 侧本地前置返回 `ApiKeyMissing`。
- *凭据从哪读*：crate 不读环境变量——在应用入口用 `dotenv` 加载 `.env` 后经 clap/显式参数传入（见 2.4）。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L2**（行为等价：OpenAI 兼容端点调用语义、参数映射、错误分类与 Python 侧 DashScope 实现一致；凭据管理 LL1-L2）
- **权威来源**: `specs/001-compatibility-baseline/capability-matrix.json`
- **已知偏差**:
  - 矩阵 `status` 字段当前全部为 `NOT_ANALYZED`（未随 Feature 001-017 回填）；本页等级以矩阵 `target_level`（credential 类目）+ `specs/004-provider-architecture`、`specs/005-provider-extraction` + 代码实际状态交叉核实为准。
  - crate 不读取环境变量（Python 侧部分实现支持自动读取 `DASHSCOPE_API_KEY`）——凭据必须显式传入，属刻意的分层设计差异。
  - Embedding 错误类型为 `EmbeddingError::ApiKeyMissing`，其提示文案提及 `DASHSCOPE_API_KEY`，但实际凭据来源由调用方决定。
- **不支持的能力**: 未实现的 Provider 能力经 `ModelError::UnsupportedFeature` 显式返回（如特定模型的多模态输入校验：`supports_multimodal` 为 false 时拒绝 `DataBlock` 输入）。

## 7. 相关模块 (See Also)

- [模型抽象 / model](./model.md) — `ChatModel` trait 与错误分类
- [RAG / rag](./rag.md) — Embedding 模型的消费方
- [Agent 系统 / agent](./agent.md) — 模型的主要调用方

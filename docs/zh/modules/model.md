# 模型抽象 / Model

> 一句话定位：所有 LLM Provider 的统一接口——`ChatModel` trait 定义流式/非流式调用、自动重试、token 计数与结构化输出，`Arc<dyn ChatModel>` 是 Agent 与具体模型厂商之间的解耦层。

## 1. 模块概述 (Overview)

本模块对应 `agent_scope_model` crate，位于抽象层（依赖 `agent_scope_types`/`agent_scope_message`，不依赖任何具体 Provider）。它定义"什么是模型调用"，具体厂商（如 DashScope）在独立 crate 中实现该 trait。

**适用场景**：调用模型生成回复；实现自定义 Provider 接入 Agent；消费流式响应并累积为完整结果；使用结构化输出（JSON mode）。

**前置阅读**：[消息与基础类型](./message-types.md)（`Msg`/`ContentBlock`）；Provider 具体用法见 [DashScope](./dashscope.md)。

## 2. 核心概念与主要公开类型 (Core Concepts)

### 2.1 `ChatModel` trait

所有聊天模型 Provider 的统一接口（`async_trait`）：

| 方法 | 类别 | 说明 |
|------|------|------|
| `model_name() -> &str` | 必需 | 模型标识符（如 `"qwen-plus"`） |
| `stream_enabled() -> bool` | 必需 | 是否默认流式 |
| `call_api(...) -> Result<ModelCallResult, ModelError>` | 必需 | Provider 特定 API 实现（唯一需要写网络代码的方法） |
| `max_retries() -> u32` | 可选覆盖 | 最大重试次数，默认 `3` |
| `retry_delay() -> f64` | 可选覆盖 | 重试间隔秒数，默认 `1.0` |
| `retryable_errors() -> &[ModelErrorKind]` | 可选覆盖 | 触发重试的错误类别，默认空（不重试） |
| `context_size() -> i64` | 可选覆盖 | 上下文窗口 token 数，默认 `32768` |
| `call(...) -> Result<ModelCallResult, ModelError>` | 默认方法 | **调用入口**：包装 `call_api` 的重试循环 |
| `count_tokens(...) -> usize` | 默认方法 | bytes/4 启发式计数（每个 `DataBlock` 约 2000 tokens），Provider 可覆盖为精确分词器 |
| `generate_structured_output(...) -> Result<StructuredResponse, ModelError>` | 默认方法 | 结构化输出（见 4.4） |

**重试语义**：`call()` 最多执行 `max_retries` 次额外尝试，仅当错误的 `kind()` 命中 `retryable_errors()` 时才重试，间隔 `retry_delay` 秒；全部失败返回 `ModelError::RetryExhausted { attempts: max_retries + 1, last_error, provider }`。

### 2.2 `ModelCallResult`：流式与非流式的统一返回

```rust
pub enum ModelCallResult {
    Complete(ChatResponse),                                  // 非流式：一次性完整响应
    Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>),  // 流式：增量响应序列
}
```

`stream_enabled()` 为 true 时 `call()` 返回 `Stream` 变体；流中的每个 `ChatResponse` 是一个增量（Delta），`is_last` 标记最后一个。

### 2.3 `ChatResponse`

| 字段 | 说明 |
|------|------|
| `content: Vec<ContentBlock>` | 响应内容块（text/thinking/tool_call 等） |
| `is_last: bool` | 流式序列的最后一块 |
| `id` / `created_at` | 响应 ID（UUID）与 RFC 3339 时间戳 |
| `usage: Option<ChatUsage>` | 扩展用量统计：`input_tokens`/`output_tokens`/`time`/`cache_creation_input_tokens`/`cache_input_tokens`（比 `message::Usage` 多了耗时与缓存统计） |
| `finished_reason: FinishedReason` | `completed`（默认）/ `interrupted` |
| `metadata` | Provider 附加元数据（如工具调用 extras） |

Provider 实现可用构建器方法 `append_text`/`append_thinking`/`append_tool_call` 按 `block_id` 增量合并内容块。

### 2.4 `StreamAccumulator`：O(n) 流式累积

流式消费的标准工具：`new()` → 对每个增量调用 `append_chat_response(&delta)` → 结束后 `build()` 得到合并后的完整 `ChatResponse`。内部按块类型分缓冲（文本/思考/工具调用输入各自累积），避免 O(n²) 字符串拼接。

### 2.5 `ToolChoice` 与 `ModelCard`

- `ToolChoice`：工具选择配置——`auto()`（默认）/`none()`/`required()`/`specific_tool(name)`；`validate()` 对照可用工具名校验，非法时 `call()` 前置返回 `ValidationError`。
- `ModelCard`/`ModelStatus`：模型卡片（上下文窗口、参数 schema 等元数据），由 `ChatModel::list_models()` 从 Provider 提供的 YAML 值解析。
- `Formatter` trait：将 `Msg` 列表转换为 Provider API 消息格式（及反向），`FormatError` 为转换错误。

## 3. 快速示例 (Quick Example)

示例共享库中创建模型的标准方式——返回 `Arc<DashScopeChatModel>`，可直接注入 Agent：

<!-- source: examples/common.rs:L34-L36 -->
```rust
pub fn create_model(api_key: &str, model_name: &str) -> Arc<DashScopeChatModel> {
    Arc::new(DashScopeChatModel::new(api_key, model_name))
}
```

thinking 模式变体（启用推理内容输出并强制流式）见 `examples/common.rs` L43 `create_model_with_thinking`；Agent 侧如何消费模型流见 [事件与流式](./event-streaming.md)。

## 4. 关键用法模式 (Usage Patterns)

### 4.1 非流式调用

```rust
let result = model.call(&messages, None, None).await?;
if let ModelCallResult::Complete(resp) = result {
    for block in &resp.content { /* 读取 ContentBlock */ }
    if let Some(u) = &resp.usage {
        println!("in={} out={}", u.input_tokens, u.output_tokens);
    }
}
```

### 4.2 流式调用 + 累积

```rust
let result = model.call(&messages, None, None).await?;
if let ModelCallResult::Stream(mut stream) = result {
    let mut acc = StreamAccumulator::new();
    while let Some(chunk) = stream.next().await {
        let delta = chunk?;                 // Result<ChatResponse, ModelError>
        acc.append_chat_response(&delta);   // O(n) 累积
        // 或即时渲染 delta.content 中的增量块
    }
    let full = acc.build();                 // 合并后的完整响应
}
```

### 4.3 自定义 Provider 接入

实现 `ChatModel` 的三个必需方法（`model_name`/`stream_enabled`/`call_api`），按需覆盖重试配置，然后以 trait object 注入：

```rust
let model: Arc<dyn ChatModel> = Arc::new(MyProvider::new(...));
// Agent 构造处接受 Arc<dyn ChatModel>，与具体厂商解耦
```

`call_api` 返回 `ModelError`；用 `ModelError::ApiError { status, message, provider }` 包装 HTTP 错误即可自动获得 `kind()` 分类（401/403→`Authentication`、429→`RateLimit`、400/422→`BadRequest`、5xx→`InternalServer`），供 `retryable_errors()` 匹配。

### 4.4 结构化输出

`generate_structured_output(messages, &json_schema)` 默认通过 tool-calling 旁路实现：注入名为 `generate_structured_output` 的工具并强制 `ToolChoice::required()`，从工具调用输入解析 JSON；解析失败时以 json_repair 兜底修复。注意：**默认实现不支持流式调用下的结构化输出**（返回 `StructuredOutputError`）。

### 4.5 超时与取消

trait 层无内建超时——超时由调用方以 `tokio::time::timeout` 包裹 `call()`，或在 Provider 的 HTTP client 上配置。取消在 Agent 层通过 `CancellationToken` 实现，取消中的调用以 `ModelError::Cancelled` 结束（见 [Agent 系统](./agent.md)）。

## 5. 错误与不支持的能力 (Errors & Unsupported)

| 错误变体 | 触发条件 |
|----------|----------|
| `ModelError::ApiError { status, message, provider }` | Provider 返回 HTTP 错误；`kind()` 按状态码分类 |
| `ModelError::RetryExhausted { attempts, last_error, provider }` | 重试耗尽（`attempts` = `max_retries` + 1） |
| `ModelError::Cancelled` | 调用被取消（CancellationToken） |
| `ModelError::ValidationError { field, message }` | 参数校验失败（如 `tool_choice` 指定了不存在的工具、结构化输出 messages 为空） |
| `ModelError::SerializationError` / `FormatError` | JSON 序列化失败 / Formatter 消息格式转换失败 |
| `ModelError::StructuredOutputError { reason }` | 结构化输出解析失败或流式下使用默认实现 |
| `ModelError::UnsupportedFeature { feature, provider }` | Provider 不支持所请求能力（宪法第五条：显式拒绝而非伪兼容） |
| `ModelError::ConfigError { message }` | 配置错误（如凭据缺失） |

**不支持的能力**：由具体 Provider 决定并显式返回 `UnsupportedFeature`，不在本抽象层定义清单。

## 6. 兼容性 (Compatibility)

- **兼容等级**: **L1**（核心类型与序列化协议逐字段兼容，9 条）；**L2**（trait 调用语义/重试/计数行为等价，34 条）
- **权威来源**: `specs/001-compatibility-baseline/capability-matrix.json`
- **已知偏差**:
  - 矩阵 `status` 字段当前全部为 `NOT_ANALYZED`（未随 Feature 001-017 回填）；本页等级以矩阵 `target_level`（model 类目 L1×9/L2×34）+ `specs/003-model-api`、`specs/005-provider-extraction` + 代码实际状态交叉核实为准。
  - `count_tokens` 默认实现为 bytes/4 启发式（Python 侧依赖 tiktoken 等精确分词器）；Provider 可覆盖——属近似行为的显式偏差。
  - 结构化输出默认实现为 tool-calling 旁路且不支持流式；与 Python 侧原生 JSON mode 的路径不同。
  - `ChatUsage` 较 Python 侧扩展了 `time` 与 prompt 缓存统计字段。
- **不支持的能力**: 流式结构化输出（默认实现，返回 `StructuredOutputError`）；其余由 Provider 以 `UnsupportedFeature` 显式声明。

## 7. 相关模块 (See Also)

- [DashScope Provider / dashscope](./dashscope.md) — 本 trait 的参考实现
- [事件与流式 / event-streaming](./event-streaming.md) — 模型流到 Agent 事件的转换
- [Agent 系统 / agent](./agent.md) — `Arc<dyn ChatModel>` 的消费方
- [消息与基础类型 / message-types](./message-types.md) — `ChatResponse.content` 的块类型

# Research: Model API (Feature 003)

**Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

## 1. OpenAI API 调用方式：reqwest vs async-openai

**Decision**: 使用 `reqwest` + 手动构建 HTTP 请求

**Rationale**:
- Python 参考实现直接调用 OpenAI SDK 的 `client.chat.completions.create()`，但 Rust 的 `async-openai` crate 封装层次较高，在流式响应解析、tool_choice 回退、extra_body 透传等场景下与 Python 行为对齐有额外适配成本。
- `reqwest` 提供对 HTTP 请求/响应的完全控制，SSE 流式解析可手动实现（与 Python 的 `AsyncStream` 处理逻辑一一对应）。
- 后续其他 Provider（Anthropic、Gemini 等）也通过 HTTP API 交互，统一使用 `reqwest` 避免多 SDK 依赖管理。

**Alternatives considered**:
- `async-openai` crate：封装良好但需要绕道适配 tool_choice 回退、stream_options（如 `include_usage`）等细节。作为可选 future 优化方向保留。
- `hyper`：更底层，对基础 HTTP 功能开销更大。

## 2. 流式返回类型设计

**Decision**: 使用 `futures::stream::Stream` trait，返回 `Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>`

**Rationale**:
- `tokio_stream::StreamExt` 提供 `next()`、`map()`、`filter_map()` 等操作符，与 Python 的 `AsyncGenerator` 等价。
- `Pin<Box<dyn Stream + Send>>` 避免了在 trait 方法返回类型中引入泛型参数（泛型会让 `ChatModel` trait 不可 object-safe）。
- 取消语义：当 Stream 被 drop 时，内部的 `AbortHandle` 或 `CancellationToken` 触发取消逻辑，StreamAccumulator 的 `finished_reason` 被置为 `INTERRUPTED`。

**Alternatives considered**:
- `async fn call() -> Result<Vec<ChatResponse>, ModelError>` + 单独的 `async fn call_stream()` — 会导致 trait 膨胀，也不支持非流式/流式的统一 retry 逻辑。
- `tokio::sync::mpsc` channel — 引入额外的 channel 管理复杂度，且丢失 backpressure 语义。
- 泛型关联类型（GAT）— `type Stream<'a>: Stream<Item = ...>` — 使 trait 不可 object-safe。

## 3. 重试机制

**Decision**: 手动实现指数退避 + 可重试异常过滤，不引入 tokio-retry

**Rationale**:
- Python 实现使用简单的 `max_retries` + `retry_delay`（固定间隔），不做指数退避。为保持行为兼容，Rust 侧也采用固定间隔。
- 可重试异常列表由 `_get_retryable_exceptions()` 返回，Provider 子类覆写。Rust 侧将此映射为 `ModelError` 的特定变体（如 `ApiError` 中的 status code 匹配）。
- 取消检查点：在每次重试前的 sleep 期间，通过 `tokio::select!` + `CancellationToken` 检查取消信号。

**Alternatives considered**:
- `tokio-retry` crate：对简单的固定间隔重试过度设计，且行为必须与 Python 参考一致。
- 指数退避：与 Python 参考兼容性要求冲突（Python 端是固定间隔）。

## 4. StreamAccumulator 实现模式

**Decision**: 片段列表 + 一次性 join（O(n) 总复杂度）

**Rationale**:
- Python 的 `_StreamAccumulator` 使用 `list[str]` 暂存文本/thinking/tool_call input 片段，`build()` 时 `"".join()`。
- 这避免了 Python 原版 `ChatResponse.append_chat_response()` 中 `block.text += delta` 的 O(n²) 字符串拼接问题。
- Rust 侧直接复用此模式：`AccTextBlock.text: Vec<String>`、`AccThinkingBlock.thinking: Vec<String>`、`AccToolCallBlock.input: Vec<String>`、`AccDataBlock.source: _AccBase64Source { data: Vec<Vec<u8>> }`。

**Implementation note**:
- `_AccTextBlock`/`_AccThinkingBlock`/`_AccToolCallBlock`/`_AccDataBlock`/`_AccBase64Source` 在 Rust 中定义为 `StreamAccumulator` 内部的辅助结构体，不暴露为公开 API。
- Provider extras（如 Anthropic 的 signature）通过 `extra_fields: HashMap<String, JsonValue>` 累积——最后一个非 None 值覆盖前面的值。

## 5. ModelCard YAML 加载

**Decision**: 使用 `serde_yaml` crate

**Rationale**:
- `serde_yaml` 是 Rust 生态最成熟的 YAML 库，支持 `serde::Deserialize`。
- Python 实现中 `ModelCard.from_yaml()` 的逻辑（加载 YAML → 获取 base parameter JSON Schema → 应用 overrides → 合并）在 Rust 中用 `serde_yaml::from_reader()` + 手动合并 JSON Schema dict（`serde_json::Value`）实现。

**Implementation note**:
- `parameter_class` 在 Python 中是一个 Pydantic `BaseModel` 子类，其 `model_json_schema()` 生成 JSON Schema dict。Rust 侧对应的概念是：每个 Provider 的 `Parameters` struct 通过 `schemars::JsonSchema` derive 生成 JSON Schema，`ModelCard::from_yaml()` 接收该 schema 的 `serde_json::Value` 表示。
- YAML 文件路径的扫描逻辑：`std::fs::read_dir(yaml_dir)?.filter(|e| e.path().extension() == Some("yaml"))`。

**Alternatives considered**:
- `yaml-rust` / `yaml-rust2`：功能较弱，yaml-rust2 维护不活跃。`serde_yaml` 已足够且与 serde 生态无缝集成。

## 6. JSON Repair（结构化输出）

**Decision**: 使用 `jsonrepair` crate（Rust 生态中称为 `json_repair` 或手动实现基础修复）

**Rationale**:
- Python 的 `_json_loads_with_repair` 尝试修复格式不佳的 JSON（如末尾缺 `}`、多余逗号等），然后再用 `jsonschema.validate()` 验证。
- Rust 生态中 `jsonrepair` crate 提供类似功能（修复各种 JSON 语法错误）。
- 若 crate 功能不足，可手动实现基础修复（匹配括号、去除尾逗号），因为实际场景中主要问题是 LLM 截断导致的末尾括号不匹配。

**Status**: NEEDS VERIFICATION — 确认 `jsonrepair` crate 在 Rust 中的可用性和功能覆盖范围。

## 7. JSON Schema Flatten（$ref/$defs 内联）

**Decision**: 手动实现递归 $ref 解析

**Rationale**:
- Python 的 `_flatten_json_schema(params)` 将内嵌的 `$defs` + `$ref` 引用内联为自包含的 JSON Schema。
- 逻辑相对简单：遍历 schema 树，遇到 `$ref: "#/$defs/TypeName"` 时从 `$defs` 字典中取出对应定义递归内联。防止无限递归通过 `visited: HashSet<String>` 跟踪。
- 此函数在 OpenAI Chat 的 `_format_tools` 中调用，用于保证不同 Provider 对 schema reference 的支持程度一致。

## 8. WAV Header 注入（音频流）

**Decision**: 实现最小化 WAV header 构建函数

**Rationale**:
- OpenAI 流式音频仅支持 PCM16（24kHz, 16-bit mono），而前端需要 WAV 格式播放。
- Python 的 `_build_streaming_wav_header()` 生成一个"流式 WAV header"（data chunk size 置为最大值 0xFFFFFFFF），前端解析时忽略该值。
- Rust 侧提供等效函数：构建 44 字节 WAV header（RIFF + fmt + data chunk），data size 置 `u32::MAX`。

## 9. Cancellation 传播

**Decision**: 使用 `tokio_util::sync::CancellationToken`

**Rationale**:
- `ChatModel::call()` 需要感知外部取消信号（drop/cancel Stream）。
- `CancellationToken` 是 tokio 生态的标准取消机制，`tokio::select!` 可同时等待 API 响应和取消信号。
- 当取消被触发时，`_call_api` 内部的 inflight HTTP 请求被 abort（reqwest 的 `RequestBuilder::timeout` 或手动 abort handle），StreamAccumulator 设置 `finished_reason = INTERRUPTED` 并 yield 最终的 `is_last=true` chunk。

## 10. reqwest 流式响应解析（SSE）

**Decision**: 手动实现 SSE 行解析

**Rationale**:
- OpenAI 的流式响应使用 SSE（Server-Sent Events），即 `data: {...}\n\n` 格式。
- `reqwest::Response::bytes_stream()` 返回字节流，需要按行分割、识别 `data: ` 前缀、解析 JSON、处理 `[DONE]` 结束标记。
- `tokio_stream::StreamExt` + 自定义 `pin_project!` 辅助宏实现 SSE parser adapter。
- 此模式可复用于 Anthropic（也使用 SSE）等其他 Provider。

**Alternatives considered**:
- `eventsource-stream` crate：专用 SSE 解析器，但因 OpenAI 的流式响应格式（`data: [DONE]` 非标准 JSON）需要额外适配，手动实现更可控。

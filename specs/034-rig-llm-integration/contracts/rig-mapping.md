# Contract: Rig Mapping（`agent_scope_rig` 内部类型映射契约）

**Feature**: 034-rig-llm-integration | **Date**: 2026-08-17

本契约定义 `agent_scope_rig` 内**双向往返映射**：agent_scope 既有类型（`Msg`/`ContentBlock`/`ToolChoice`/`ModelError`）↔ rig 类型（`Message`/`AssistantContent`/`CompletionRequest`/`CompletionError`）。映射层全部在 `agent_scope_rig` 内部，rig 类型不越过 crate 边界。

## 1. 消息映射（`message.rs`）

### 1.1 agent_scope → rig（出站）

`Msg`（role + content blocks）→ rig `Message` 序列：

| `Msg` 特征 | rig `Message` |
|-----------|---------------|
| `role=User`，纯文本 blocks | `Message::User{ content: vec![UserTextContent::from(text)], images: vec![], name }` |
| `role=User`，含 `ContentBlock::Data` | `Message::User{ content, images: [data 编码], name }`（多模态走 `images`） |
| `role=User`，含 `ContentBlock::ToolResult` | **展开**为独立 `Message::ToolResult{ name, content }`（紧跟在对应 ToolCall 后） |
| `role=Assistant`，文本 | `Message::Assistant{ content: [AssistantContent::Text], thinking: vec![], images: vec![], name, tool_calls: vec![] }` |
| `role=Assistant`，含 `ContentBlock::Thinking` | `Assistant.thinking`（ThinkingBlock.text → `Text` 内容） |
| `role=Assistant`，含 `ContentBlock::ToolCall(tc)` | `Assistant.tool_calls.push(ToolCall{ id: tc.id, name: tc.name, arguments: parse(tc.input), .. })` |
| `role=System` | `Message::System{ content }` |
| `ContentBlock::Hint` | 以文本内容发送（注入给模型看的参考/指令，如 RAG 检索知识、记忆、任务提醒；注入方负责在文本内声明 untrusted 边界） |
| `ContentBlock::Unknown` / 无法映射 | `ModelError::FormatError`（不静默丢弃） |

出站约束：
- ToolResult 展开为独立 `Message::ToolResult` 后，紧随其 ToolCall 的 **顺序保持**（OpenAI/Anthropic/DeepSeek wire 均要求）。
- `tc.input`（JSON 字符串）→ `arguments: Value`（`serde_json::from_str`；解析失败 → `ModelError::FormatError`，沿用 json_repair 后仍失败则报错）。

### 1.2 rig → agent_scope（入站，非流式 `completion()`）

rig `CompletionResponse.choice`（`Vec<AssistantContent>`）→ `ChatResponse`：

| rig `AssistantContent` | agent_scope `ContentBlock` |
|------------------------|----------------------------|
| `Text(Text)` | `ContentBlock::Text(TextBlock)` |
| `Reasoning(Reasoning)` | `ContentBlock::Thinking(ThinkingBlock)`（extras 记 reasoning id） |
| `ToolCall(ToolCall)` | `ContentBlock::ToolCall(ToolCallBlock{ id: tc.id, name, input: to_string(arguments) })` |
| `Image` | `ContentBlock::Data(DataBlock)` |

入站约束：
- 空 choice → `ModelError::EmptyResponse`（既有错误面）。
- `ChatResponse.id`/`created_at` 用默认生成（`ChatResponse::default()`），不依赖 provider 返回的 message_id（可观察结构不变，宪法第十二条）。

## 2. 工具映射（`tools.rs`）

### 2.1 Tool schema（`&[JsonValue]`）→ rig `ToolDefinition`

```json
// 输入（OpenAI function-calling 格式，现状不变）
{"type":"function","function":{"name":"search","description":"...","parameters":{...}}}
// → rig
ToolDefinition { name, description, parameters: Value, strict: false }
```

- 逐项转换；`function.name`/`description`/`parameters` 透传；缺少 `function` 包裹 → `ModelError::FormatError`。
- `tools` 数组为空 / `None` → 不设置 `CompletionRequest.tools`。

### 2.2 `ToolChoice` → rig tool_choice / 参数

| `ToolChoice` | rig 输出 |
|--------------|----------|
| `mode=auto` | `CompletionRequest.tool_choice = None`（rig 默认 auto） |
| `mode=none` | 不设置 `tools`（等价禁用），或 provider 支持的 `none` 枚举 |
| `mode=required` | `CompletionRequest.tool_choice = Some(ToolChoice::Required)`；若 `thinking_tool_choice_incompatible` 则降级 `auto` + tracing 事件（见 data-model 校验规则） |
| `mode=specific_tool(name)` | `CompletionRequest.tool_choice = Some(ToolChoice::Specific(name))` |
| `tools` 子集过滤 | 转换时仅保留 `tools` 列出的工具（延续 round-4 M18 语义） |

## 3. 参数映射（`params.rs`）

| `RigParameters` | rig `CompletionRequest` |
|-----------------|-------------------------|
| `max_tokens` | 顶层 `max_tokens` |
| `temperature` | 顶层 `temperature` |
| `top_p` / `top_k` / `seed` / `stop` | `additional_params`（provider 支持时透传） |
| `thinking_budget` | `additional_params`（Anthropic/DeepSeek 支持时） |
| `additional_params`（用户兜底） | 合并进 `additional_params` |

- 生成参数全部可选（None → 不设置），`CompletionRequest` 其余字段（model/preamble/chat_history/documents/output_schema/record_telemetry_content）由 `RigChatModel::call_api` 按需填充。

## 4. 错误映射（`error.rs`）

rig `CompletionError` → `ModelError`（`ModelErrorKind` 六分类，宪法第十三条）：

| rig 错误类别（0.42.0 近似） | `ModelErrorKind` |
|----------------------------|------------------|
| HTTP 401/403（`ApiError`） | `Authentication` |
| HTTP 429 | `RateLimit` |
| HTTP 5xx | `InternalServer` |
| HTTP 4xx（非 401/403/429） | `BadRequest` |
| 连接失败（reqwest 层） | `ApiConnection` |
| 超时 | `ApiTimeout` |
| 响应解析 / 流式失败（`ResponseError`/`StreamError`） | `FormatError`（context="rig:stream"/"rig:response"） |
| 工具 / 输出 schema 错误（`ToolError`/`OutputSchemaError`） | `StructuredOutputError` / `FormatError` |
| 空响应 | `EmptyResponse` |

- **重试语义不变**：`ModelErrorKind` 落入 `retryable_errors()`（RateLimit/InternalServer/ApiConnection/ApiTimeout）时由 `ChatModel::call` 既有循环重试；rig 不介入重试。
- 错误消息中**不泄露 API key**；provider 返回的原始 message 裁剪敏感字段（宪法第九/十四条）。

## 5. 流式映射契约（`stream.rs`）——事件顺序

消费 rig `StreamingCompletionResponse`（`Stream<Item=Result<StreamedAssistantContent, CompletionError>>`），产出 `Stream<Item=Result<ChatResponse, ModelError>>`。

**顺序契约**（agent 引擎 `reply_stream` 可观察，FR-004/宪法第七条）：

| 阶段 | rig 事件（入站） | 产出 `ChatResponse` |
|------|------------------|---------------------|
| 推理 | `StreamedAssistantContent::Reasoning{reasoning, id}`（及 `ReasoningDelta`） | `ContentBlock::Thinking`（增量拼接，block_id 稳定）→ ThinkingStart/ThinkingDelta 事件 |
| 文本 | `Text(Text)`（及 delta） | `ContentBlock::Text`（增量拼接）→ TextStart/TextDelta 事件 |
| 工具 | `ToolCall{tool_call, internal_call_id}` / `ToolCallDelta` | `ContentBlock::ToolCall`（按 `internal_call_id` 增量拼接 name/arguments）→ ToolCallStart/ToolCallDelta 事件 |
| 图片 | `Image` | `ContentBlock::Data` |
| 流末 | 流结束 + `usage()` 聚合 + `finish_reason` | 末个 `ChatResponse`：`is_last=true`、`usage=Some(...)`、`finished_reason`（Completed/Interrupted 映射）、`tool_call_id_map`（`tc_{idx}` → provider id） |

**流式不变量**：
- 每个 `Text`/`Thinking`/`ToolCall` block 在整个流中保持**稳定 block_id**，增量拼接语义与现有 DashScope SSE 解析一致。
- `is_last=true` 只出现在流的最后一个 `ChatResponse`；此前全部 `is_last=false`。
- 流中 `Err(CompletionError)` → 对应 `Err(ModelError)`，流终止（不 panic）。
- `tool_call_id_map` 用于工具结果回填（`tc_{idx}` 是 agent 引擎积累键，provider id 是 rig `ToolCall.id`），在流末一次性写入（`skip` 序列化，同现状）。

## 6. 结构化输出契约（`structured.rs`）

`generate_structured_output(messages, structured_model)`：

1. **原生路径**：`flatten_json_schema_with_defs_checked(schema)` → 填入 `CompletionRequest.output_schema` → `completion()`。从响应 `AssistantContent` 中取符合 schema 的文本/JSON → `StructuredResponse{ content, usage }`。
2. **回退路径**（provider 不支持 `output_schema` 时）：沿用 `ChatModel` 默认 tool-calling bypass（注入 `generate_structured_output` 工具 + `tool_choice=required`），`json_repair` 兜底。
3. 空 messages → `ModelError::ValidationError`（同 trait 默认）。
4. 输出 `StructuredResponse`（`response_type="structured_response"`），与现状一致。

## 7. Embedding 映射（`RigEmbeddingModel`）

| `EmbeddingInput` | rig 调用 |
|------------------|----------|
| `Text(s)` | `embed_text(s)`（批：`embed_texts(vec)`） |
| `DataBlock(_)` | `EmbeddingError::MultimodalNotSupported`（model_card.supports_multimodal=false） |

- `EmbeddingResponse{ embeddings, usage }`：`embeddings.len() == inputs.len()`、每向量长度 `== model_card().dimensions`（`ndims()`），满足 trait 不变量。
- 缓存：`agent_scope_embedding::cache::EmbeddingCache`/`FileEmbeddingCache` 不受影响（作用于 trait 之上）。

## 8. 映射层单元测试矩阵（宪法第六条，确定性）

| 测试文件 | 覆盖 |
|----------|------|
| `message_mapping_tests.rs` | 1.1/1.2 往返：各 role、Thinking、ToolCall、ToolResult 展开、Hint 以文本发送、Unknown 报错 |
| `tools_mapping_tests.rs` | 2.1 schema 转换、2.2 ToolChoice 四模式 + 子集过滤 + required 降级 |
| `streaming_tests.rs` | §5 顺序契约：Reasoning→Thinking、Text 增量、ToolCall/Delta 拼接、is_last/usage/tool_call_id_map |
| `error_mapping_tests.rs` | §4 全部分类映射 |
| `structured_output_tests.rs` | §6 原生 + 回退 + JSON repair + 空消息 |
| `openai/anthropic/deepseek_tests.rs` | 构造 + mock HTTP 冒烟（请求体形状、流式解析） |

## 9. 已记录偏差（Foundational 实现 vs 契约，T004–T012 登记）

以下偏差在实现时确认，均已同步写入对应模块 doc comment，后续阶段必须沿用：

| # | 契约原文 | 实现 | 原因 |
|---|----------|------|------|
| D1 | `Message::ToolResult` 独立变体 | `Message::User{UserContent::ToolResult}` | rig 0.41.0 无独立变体，ToolResult 内嵌于 UserContent |
| D2 | `Assistant.thinking` 字段 | `AssistantContent::Reasoning` | 0.41.0 `Assistant` 无 thinking 字段，Reasoning 是其替代表达 |
| D3 | `ToolDefinition{..., strict:false}` | 无 `strict` 字段 | 0.41.0 `ToolDefinition` 无 strict（0.42 才有） |
| D4 | `ToolChoice::Specific(name)` | `Specific{function_names: Vec<String>}` | 0.41.0 Specific 接收工具名列表 |
| D5 | 空 choice → `ModelError::EmptyResponse` | `ModelError::FormatError{context:"rig:response", InvalidMessage("empty assistant response")}` | 框架无 EmptyResponse 变体；空响应视为响应格式异常 |
| D6 | T004 签名 `completion()->Result<CompletionResponse,...>` | `NormCompletion`/`RigStreamDelta` 归一化 | 0.41.0 补全响应泛型（`T`/`R`），对象安全需归一化 |
| D7 | `ProviderResponseError` 可构造测试 | 该分支无法直接单测 | `provider_response` 为 rig `pub(crate)` 模块，外部不可命名载荷类型；其分类逻辑与 `InvalidStatusCode` 共用 `classify_status`，已被状态测试覆盖 |
| D8 | 错误消息仅提取 `error.message` | 提取后对 `sk-` token 打码 | OpenAI 的 `error.message` 本身含完整 key（"Incorrect API key provided: sk-…"），必须二次打码 |

- **超时表达**：契约 §4 记"超时→ApiTimeout"，但 `ModelError.kind()` 只看 status，status:0→ApiConnection（retryable）；框架无 ApiTimeout 产生路径，超时与连接统一 `ApiError{status:0}`，注释已说明（D9 登记于 error.rs）。

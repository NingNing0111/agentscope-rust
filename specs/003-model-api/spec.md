# Feature Specification: AgentScope Model API

**Feature Branch**: `003-model-api`

**Created**: 2026-07-28

**Status**: Draft

**Input**: User description: "实现 AgentScope Model 层：ChatModel trait、ChatResponse、ChatUsage、ModelCard、StreamAccumulator、Formatter trait、OpenAI Chat 参考实现"

## Clarifications

### Session 2026-07-28

- Q: Model 层如何解决对 ToolChoice 类型的跨层依赖（Python 实现中 `ChatModelBase` 依赖 `tool.ToolChoice`）？ → A: `ToolChoice` 是一个纯结构类型（mode + tools 列表），在 model 层内直接定义，不依赖 tool crate。后续 tool 层可直接复用或按需扩展。
- Q: Formatter（将 Msg 格式化为 provider API 格式）应该放在哪一层？ → A: 放在 model 层。Formatter 是 Msg → API format 的转换层，只依赖 message crate，是 model 层的自然组成部分。具体 provider formatter（如 OpenAIChatFormatter）和 model 实现在同一个 crate 中。
- Q: 具体 Provider 实现（OpenAI、Anthropic 等）应该放在单独 crate 还是统一放在 `agent_scope_model` 中？ → A: 首个 Provider（OpenAI Chat）放在 `agent_scope_model` 中作为参考实现，证明 trait 的可用性。后续 provider 可根据需要在单 crate 内扩展或拆分为独立 crate/feature flags。
- Q: 流式响应的 Rust 类型应该如何设计？ → A: 返回 `Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>`。内部流式解析使用 StreamAccumulator 做 O(n) 增量拼接，替代 Python 实现中原 ChatResponse.append_chat_response() 的 O(n²) 字符串拼接。
- Q: Model 层是否需要定义 `Credential` 抽象？ → A: 本 Feature 不定义 Credential trait——credential 是 provider 特定的认证信息（API key、base URL 等），在具体 model 构造时直接注入。Model 层保持 credential-agnostic。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 模型调用与流式响应 (Priority: P1)

开发者使用 AgentScope 进行 LLM 调用时，需要一个统一的 `ChatModel` trait 来抽象不同 Provider 的 API 调用。调用支持同步返回和流式返回两种模式，具备自动重试（retry）和取消（cancellation）能力。流式调用产生 `ChatResponse` 增量 chunk，最终汇聚为完整回复。

**Why this priority**: 模型调用是整个 Agent 系统的核心驱动——Agent 的推理-行动循环依赖模型调用来生成思考和决策。ChatModel trait 定义了所有 Provider 的统一接口，是上层 Agent/Memory/Middleware 的基础。

**Independent Test**: 可创建 Mock 模型实现 ChatModel trait，验证调用流程（重试、取消、流式拼接），验证 ChatResponse 在各阶段的状态。

**Acceptance Scenarios**:

1. **Given** 一个实现了 `ChatModel` trait 的 mock 模型，**When** 调用 `call(messages)` 传入消息列表，**Then** 返回完整的 `ChatResponse`，其中 `is_last=True`，包含 `content`、`usage`、`finished_reason`。
2. **Given** 一个采用流式模式的模型，**When** 调用 `call(messages)` 且 `stream=True`，**Then** 返回 `Stream<Item=ChatResponse>`，chunk 的 `is_last=False`，最终 chunk 的 `is_last=True`。
3. **Given** 一次模型调用过程中发生 `asyncio.CancelledError`（Rust 侧对应 Stream 被 drop 或 cancel），**When** 取消发生，**Then** 最终响应的 `finished_reason` 为 `INTERRUPTED`。
4. **Given** 模型调用第一次失败（触发可重试异常），**When** `max_retries=3`，**Then** 自动重试最多 3 次，重试间隔 `retry_delay` 秒，全部失败后抛出最后一次错误。
5. **Given** 一种不可重试的异常（如 `ValueError`），**When** 模型调用中触发，**Then** 不重试，立即向上传播错误。
6. **Given** 一个流式响应的多 chunk，**When** 使用 StreamAccumulator 累积，**Then** 每个 block 的内容被正确拼接（文本以字符串连接，tool call input 以 JSON 片段连接，base64 数据以 decode-concat-re-encode 模式累积），最终 `build()` 产生完整 `ChatResponse`。

---

### User Story 2 - ChatResponse 增量构建 (Priority: P1)

在流式调用过程中，Provider 返回的每个 chunk 需要被转换为 `ChatResponse` 并逐步累积到最终消息中。`ChatResponse` 提供 `append_text`、`append_thinking`、`append_tool_call`、`append_data_block` 等方法，支持按 `block_id` 增量追加内容块。

**Why this priority**: ChatResponse 是流式响应的基本构建块。没有正确的增量构建逻辑，流式响应无法被转换为结构化的 ContentBlock，最终无法构建 Msg。

**Independent Test**: 可通过模拟流式 chunk 序列 → 调用 ChatResponse 增量方法 → 验证最终 content 的正确性来独立测试。也可验证 JSON 序列化往返。

**Acceptance Scenarios**:

1. **Given** 空的 `ChatResponse`，**When** 依次调用 `append_text("Hel")`、`append_text("lo")`（同一 block_id），**Then** content 包含一个 `TextBlock`，其 `text="Hello"`。
2. **Given** 正在构建的 `ChatResponse`，**When** 调用 `append_tool_call(block_id="tc1", name="search", input='{"q":')` 再调用 `append_tool_call(block_id="tc1", ... input='"test"}')`，**Then** content 包含一个 `ToolCallBlock`，其 `input='{"q":"test"}'`。
3. **Given** 正在构建的 `ChatResponse`，**When** 调用 `append_data_block` 两次传入同一个 block_id 的音频 bytes，**Then** DataBlock 内 `Base64Source.data` 为两次 bytes 拼接后再 base64 编码的结果（而非直接字符串拼接）。
4. **Given** 两个 `ChatResponse` chunk A 和 B，**When** 调用 `A.append_chat_response(B)`，**Then** A 的 content 按 block_id 匹配合并，B 中新的 block 追加到 A 尾部，A 的 usage 更新为 B 的 usage。
5. **Given** 一个完整的 `ChatResponse`（含 TextBlock、ThinkingBlock、ToolCallBlock），**When** 序列化为 JSON 再反序列化，**Then** 所有字段值一致，`type="chat_response"` 标签存在。

---

### User Story 3 - 结构化输出生成 (Priority: P2)

开发者需要让 LLM 生成符合 JSON Schema 的结构化输出。`ChatModel` trait 提供 `generate_structured_output()` 方法，底层通过工具调用（tool-calling）手段强制 LLM 生成结构化数据。Provider 若有原生结构化输出 API 可覆写默认实现。

**Why this priority**: 结构化输出是 Agent 系统的重要能力——Agent 需要以结构化方式提取信息、生成行动计划、完成表单填充等。但它建立在基础调用能力之上。

**Independent Test**: 可模拟 LLM 返回 tool call → 验证解析出的 `StructuredResponse` 符合预期 schema。

**Acceptance Scenarios**:

1. **Given** 一个 JSON Schema 描述的输出格式，**When** 调用 `generate_structured_output(messages, json_schema)`，**Then** 底层被视为一次 tool-call-instructed 调用，返回值 `StructuredResponse.content` 符合 Schema 约束。
2. **Given** Provider 的 API 不支持 forced tool_choice，**When** 调用 `generate_structured_output()` 且首个调用失败，**Then** 自动回退为 `tool_choice="auto"` 模式重试。
3. **Given** 返回的 tool call input 为格式不佳的 JSON（如末尾缺 `}`），**When** 解析结构化输出，**Then** 可进行 JSON repair 后成功解析。
4. **Given** `structured_model` 为 Pydantic BaseModel（Rust 侧对应 `serde_json::Value` 或 json schema dict），**When** 调用 `generate_structured_output`，**Then** 其 `model_json_schema()` 被提取为 JSON Schema 传给 LLM，返回结果通过 `model_validate` 校验。

---

### User Story 4 - ModelCard 与 Model 发现 (Priority: P2)

每个 Provider 拥有一组候选模型，模型信息以 YAML 文件形式存放在 Provider 源码旁。`ModelCard` 描述模型的名称、状态、输入/输出类型、上下文大小、参数 Schema 等元数据。`ChatModel::list_models()` 类方法扫描 YAML 目录返回 ModelCard 列表。

**Why this priority**: ModelCard 帮助前端和用户了解可用的模型及参数，是模型发现的基础设施。但它不直接参与模型调用流程。

**Independent Test**: 可准备测试用的 YAML 文件 → 调用 `list_models()` → 验证返回的 ModelCard 列表结构和参数 Schema 合并逻辑。

**Acceptance Scenarios**:

1. **Given** 一个 Provider 的 `_models/` 目录含 2 个 YAML 文件，**When** 调用 `ChatModel::list_models()`，**Then** 返回 2 个 `ModelCard` 实例，每个包含 name、label、status、context_size、output_size、parameter_schema。
2. **Given** YAML 中 `parameter_overrides` 指定了某参数的 `hidden: true`，**When** 加载 ModelCard，**Then** 该参数从 `parameter_schema.properties` 中移除。
3. **Given** YAML 中 `output_types` 不含 `application/x-thinking`，**When** 加载 ModelCard，**Then** `thinking_enable` 和 `thinking_budget` 参数被自动过滤。

---

### User Story 5 - Formatter 消息格式化 (Priority: P2)

在模型调用前，AgentScope 的 `Msg` 对象需要被格式化为 Provider API 要求的字典格式。`Formatter` trait 封装这个转换逻辑——包括消息分组（工具调用序列 vs 普通对话）、工具结果的多模态数据分离、不支持媒体类型的降级处理。

**Why this priority**: Formatter 是 Msg → API request 的关键桥梁。不同 Provider 的消息格式各不相同（OpenAI Chat Completions、Anthropic Messages、Gemini API 等）。统一 Formatter 接口使 Provider 可各自实现格式化逻辑。

**Independent Test**: 可构造包含各种 ContentBlock 的 Msg 列表 → 调用 Formatter → 验证输出字典结构与 Provider 文档定义一致。

**Acceptance Scenarios**:

1. **Given** 若干条 Msg 包含 tool_call 和 tool_result 交替出现，**When** 调用 `Formatter::group_messages()`，**Then** 消息被正确分组为 tool_sequence（连续的 tool_call/tool_result）和 agent_message（非工具消息），保留原始顺序。
2. **Given** 一个 ToolResultBlock 的 output 包含 TextBlock 和 DataBlock（图片 URL），**When** 调用 `format()`，**Then** 图片 DataBlock 以 `<system-reminder>` 标记嵌入文本输出，若 Provider 不支持该媒体类型则以 URL 文本形式呈现。
3. **Given** Formatter 的 `input_types` 配置为 `["text/plain", "image/*"]`，**When** 格式化含图片 URL DataBlock 的消息，**Then** 图片 DataBlock 被提升到 user content 中作为多模态输入；而音频 DataBlock 被降级为本地文件路径字符串。

---

### User Story 6 - 依赖拓扑与跨层约束 (Priority: P3)

Model 层的模块间依赖关系必须遵循宪法要求：仅依赖 message、types、utils 三个 Foundation 层 crate，不依赖 tool、agent、memory 等上层模块。Model 层使用的 `ToolChoice` 类型在 model crate 内直接定义，不引入 tool crate 依赖。

**Why this priority**: 依赖拓扑约束确保分层架构不被破坏。虽然它不产生直接功能价值，但它是架构正确性的保障。

**Independent Test**: 通过 `cargo tree` 静态分析验证 model crate 不依赖 model 层以上的 crate。

**Acceptance Scenarios**:

1. **Given** model crate 的 `Cargo.toml`，**When** 检查其依赖，**Then** 仅包含 `agent_scope_types`、`agent_scope_message`、`agent_scope_utils`，不含 `agent_scope_tool`、`agent_scope_agent` 等。
2. **Given** `ToolChoice` 定义在 model crate 内，**When** 检查其 import 来源，**Then** 不依赖 tool crate。

---

### Edge Cases

- ChatResponse.content 中可以同时存在多个相同类型的 block（如两个 TextBlock）——通过 block_id 区分追加目标，若 block_id 不匹配则应创建新 block。
- DataBlock 的流式累加仅对 `audio/*` 类型的 chunk 有明确语义（字节可拼接）。对于 `image/*`、`video/*` 等非流式媒体，新的 delta 直接替换旧 DataBlock。
- 当 ToolCallBlock 的 input 字段在流式过程中累积的最终字符串不是合法 JSON 时，序列化行为应保留原始字符串（不做运行时 JSON 解析）。
- ChatUsage 的 token 用量在多次 MODEL_CALL_END 事件（即多次 `_call_api` 调用）中累加——上限由具体 Provider 的 tokenizer 决定，model 层不做硬性限制。
- `count_tokens()` 默认实现使用 `bytes/4` 启发式估算。对于 DataBlock，每个 DataBlock 估算为 2000 tokens。Provider 实现可覆写以使用精确 tokenizer。
- 当 `generate_structured_output` 的消息列表为空时，应返回 `ValueError`（Rust 侧返回 `Err`）。
- StreamAccumulator 中，若同一个 block_id 在不同 chunk 中改变了 block type（如 text→tool_call），应丢弃之前累积的片段并发出警告。
- `chat_completion` 非流式响应中，若 Provider 返回空 choices（如 safety filter 触发），响应 content 应为空列表，usage 为 None。
- Formatter 中的多模态数据提升需要考虑 ModelCard 的 input_types——不是所有 Provider 都支持图片/音频输入。
- ModelCard 的 `parameter_schema` 需要与 Parameters 类的 JSON Schema 合并 YAML overrides——仅覆盖已有字段，不凭空新增属性。
- 流式响应中，Provider 可能发送空 content 的 chunk（如 usage-only chunk）——这些 chunk 的 metadata（usage、id）需被 StreamAccumulator 吸收但不产生可见的内容块。

## Requirements *(mandatory)*

### Functional Requirements

#### ChatResponse 与响应模块

- **FR-001**: 系统 MUST 提供 `FinishedReason` 枚举，包含 `COMPLETED` 和 `INTERRUPTED` 两种变体，序列化为蛇形字符串。不同于 `agent_scope_types` 中的 `ReplyFinishedReason`（4 变体），`FinishedReason` 专门用于单一模型响应级别。
- **FR-002**: 系统 MUST 提供 `ChatResponse` 数据结构，包含以下字段：`content`（ContentBlock 列表，支持 TextBlock/ThinkingBlock/ToolCallBlock/DataBlock）、`is_last`（bool）、`id`（自动生成或从 API 响应中提取）、`created_at`（ISO 8601）、`type`（固定为 `"chat_response"`）、`usage`（ChatUsage 或 None）、`finished_reason`（FinishedReason，默认 COMPLETED）、`metadata`（字典）。
- **FR-003**: `ChatResponse` MUST 提供 `append_text(text, block_id)` 方法——若 content 中存在同 block_id 的 TextBlock 则追加文本，否则追加新的 TextBlock。
- **FR-004**: `ChatResponse` MUST 提供 `append_thinking(thinking, block_id, extra_fields)` 方法——若 content 中存在同 block_id 的 ThinkingBlock 则追加思考内容及 provider 特定字段，否则追加新块。
- **FR-005**: `ChatResponse` MUST 提供 `append_tool_call(block_id, name, input, extra_fields)` 方法——若 content 中存在同 block_id 的 ToolCallBlock 则追加 input 字符串和 provider 特定字段，否则追加新块。
- **FR-006**: `ChatResponse` MUST 提供 `append_data_block(block_id, data, media_type, name)` 方法——对 `audio/*` 类型做 decode-concat-re-encode（base64 拼接的正确语义），对非音频类型保留块的整体替换语义。
- **FR-007**: `ChatResponse` MUST 提供 `append_chat_response(other)` 方法——按 block_id 匹配合并内容块（类型匹配则累加，否则替换），追加对方独有的新块，更新 usage。
- **FR-008**: 系统 MUST 提供 `StructuredResponse` 数据结构，包含字段：`content`（dict/JSON object）、`id`、`created_at`、`type`（固定为 `"structured_response"`）、`usage`、`metadata`、`finished_reason`。

#### ChatUsage 模块

- **FR-009**: 系统 MUST 提供 `ChatUsage` 数据结构，包含字段：`input_tokens`（i64）、`output_tokens`（i64）、`time`（f64，秒）、`cache_creation_input_tokens`（i64，默认 0）、`cache_input_tokens`（i64，默认 0）、`type`（固定为 `"chat"`）、`metadata`（可选字典）。注意：此类型不同于 `agent_scope_message` 中的 `Usage` 结构（仅含 input_tokens/output_tokens）。

#### ChatModel Trait

- **FR-010**: 系统 MUST 提供 `ChatModel` trait，定义以下方法签名：
  - `call(&self, messages: &[Msg], tools: Option<&[...]>, tool_choice: Option<&ToolChoice>, ...) -> Result<ModelCallResult, ModelError>`
  - 内部抽象方法 `_call_api(...)` 由子实现覆盖。
  - 提供 `stream` 属性（bool）控制返回模式。
- **FR-011**: `ChatModel::call()` MUST 实现自动重试逻辑：最多重试 `max_retries` 次，仅重试 `_get_retryable_exceptions()` 返回的错误类型，重试间隔 `retry_delay` 秒，全部失败后抛出最后捕获的可重试异常。
- **FR-012**: `ChatModel::call()` MUST 在流式模式下处理取消——当 Stream 被 drop/cancel 时，StreamAccumulator 将 `finished_reason` 置为 `INTERRUPTED` 并 yield 累积的 `is_last=True` chunk。
- **FR-013**: `ChatModel` trait MUST 提供 `_get_retryable_exceptions()` 方法（默认返回空集合，由 Provider 子类覆写声明可重试的异常类型）。
- **FR-014**: `ChatModel` trait MUST 提供 `count_tokens(messages, tools) -> usize` 方法——默认实现使用 `bytes/4` 启发式估算（每个 DataBlock 估算 2000 tokens），Provider 可覆写以使用精确 tokenizer。
- **FR-015**: `ChatModel` trait MUST 提供 `generate_structured_output(messages, structured_model) -> Result<StructuredResponse, ModelError>` 方法——通过底层工具调用机制强制 LLM 输出 JSON Schema 约束的结构化数据，具备独立的 retry 循环。
- **FR-016**: `ChatModel` trait MUST 提供默认的 `_call_api_with_structured_output()` 实现——构造 `generate_structured_output` 工具，注入系统提示词强制调用该工具，解析 tool call input，通过 JSON Schema 验证并返回 `StructuredResponse`。
- **FR-017**: `ChatModel` trait MUST 提供 `list_models(custom_yaml_dir) -> Vec<ModelCard>` 类方法——扫描 YAML 目录，加载每个 YAML 文件为 ModelCard，加载失败的模型仅告警不影响其他模型。
- **FR-018**: `ChatModel` trait MUST 提供 `_validate_tool_choice(tool_choice, tools)` 方法——验证 mode（auto/none/required 或具体工具名）和 tool names 是否合法，不合法时返回错误。
- **FR-019**: `ModelCallResult` MUST 为枚举类型，包裹 `ChatResponse`（非流式）或 `Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>`（流式）。

#### ToolChoice 类型

- **FR-020**: 系统 MUST 在 model crate 中定义 `ToolChoice` 数据结构（不依赖 tool crate）。包含字段：`mode`（字符串，支持 `"auto"`、`"none"`、`"required"` 或具体工具名）、`tools`（可选字符串列表，用于过滤工具 Schema 和验证 mode）。

#### Formatter Trait

- **FR-021**: 系统 MUST 提供 `Formatter` trait，定义方法：`format(&self, msgs: &[Msg]) -> Result<Vec<serde_json::Value>, FormatError>`——将 Msg 列表转换为 Provider API 格式的字典列表。
- **FR-022**: `Formatter` trait MUST 提供 `convert_tool_result_to_string(&self, output: &ToolOutput) -> (String, Vec<ContentBlock>)`——将工具结果中的多模态数据分离为文本表示和提升块（promoted blocks），不支持的多媒体类型降级为 URL 文本或本地文件路径引用。
- **FR-023**: `Formatter` trait MUST 提供 `group_messages(&self, msgs: &[Msg])` 方法——将消息按角色分组为 `tool_sequence`（连续的 tool_call/tool_result 消息）和 `agent_message` 两类，保持原始顺序。
- **FR-024**: `Formatter` trait MUST 包含 `input_types: Vec<String>` 字段，并派生 `supported_input_media_types()` 方法（排除 `text/plain` 和 `application/x-thinking` 的媒体类型模式）。

#### StreamAccumulator（流式累加器）

- **FR-025**: 系统 MUST 提供 `StreamAccumulator` 内部结构——以 O(n) 总复杂度累积流式 chunk，替代 Python 实现中 `append_chat_response` 的 O(n²) 字符串拼接。每个 block 内用片段列表（如 `Vec<String>`）暂存增量，仅在 `build()` 时拼接一次。
- **FR-026**: `StreamAccumulator` MUST 支持四类累积块：`AccTextBlock`（文本片段列表）、`AccThinkingBlock`（思考片段列表 + provider extras 累积）、`AccToolCallBlock`（JSON 参数片段列表，name 从首个非空 delta 提取）、`AccDataBlock`（audio bytes 列表在 build 时一次性 base64 编码，非音频 source 保留最新 delta 替换语义）。
- **FR-027**: `StreamAccumulator::append_chat_response(&mut self, delta: &ChatResponse)` 在 block 类型改变时（同一 block_id 从 TextBlock 变为 ToolCallBlock）MUST 丢弃旧片段并发出警告。
- **FR-028**: `StreamAccumulator::build(self) -> ChatResponse` 将每个 block 的片段列表拼接为最终内容，返回 `is_last=True` 的完整 ChatResponse。

#### ModelCard 模块

- **FR-029**: 系统 MUST 提供 `ModelCard` 数据结构，包含字段：`type`（固定为 `"chat_model"`）、`name`、`label`（前端展示名称）、`status`（`"active" | "deprecated" | "sunset"`）、`deprecated_at`（可选的 datetime）、`input_types`（默认 `["text/plain"]`）、`output_types`（默认 `["text/plain"]`）、`context_size`（i64，> 0）、`output_size`（i64，> 0）、`parameter_schema`（JSON Schema dict）、`parameters_overrides`（dict）。
- **FR-030**: `ModelCard::from_yaml(yaml_path, parameter_json_schema) -> Result<ModelCard, ...>` 方法 MUST：加载 YAML 文件 → 以 parameter JSON Schema 为基底 → 应用 `parameter_overrides`（hidden→删除，其他→合并）→ 自动过滤 thinking 参数（当 output_types 不含 `application/x-thinking`）→ 自动过滤 voice 参数（当 output_types 不含 `audio/*`）→ 从 `output_size` 设置 `max_tokens` 最大值。

#### 参考实现：OpenAI Chat 模型

- **FR-031**: 系统 MUST 提供 `OpenAIChatModel` 结构体，实现 `ChatModel` trait，支持通过 `reqwest` 或 `async-openai` crate 调用 OpenAI Chat Completions API。构造函数接受：`api_key`、`model`、`parameters`、`stream`、`max_retries`、`retry_delay`、`context_size`、`formatter`、`client_kwargs`。
- **FR-032**: `OpenAIChatModel::Parameters` MUST 包含：`max_tokens`（可选）、`thinking_enable`（默认 false）、`reasoning_effort`（可选枚举）、`temperature`（可选）、`top_p`（可选）、`parallel_tool_calls`（默认 true）、`voice`（可选）。
- **FR-033**: `OpenAIChatModel` MUST 实现流式响应解析——处理 OpenAI 的 SSE 流，提取 delta.content（文本）、delta.reasoning_content（思考）、delta.tool_calls（工具调用）、delta.audio（音频输出含 transcript），构建增量 `ChatResponse` chunk。
- **FR-034**: `OpenAIChatModel` MUST 实现非流式响应解析——从 `ChatCompletion` 对象提取 choices[0].message 的 content/reasoning/tool_calls/audio，构建完整 `ChatResponse`（`is_last=True`）。
- **FR-035**: `OpenAIChatModel` MUST 提供 `OpenAIChatFormatter`——实现 `Formatter` trait，将 Msg 列表格式化为 OpenAI Chat Completions API 格式的消息字典列表。

#### 错误处理

- **FR-036**: 系统 MUST 提供 `ModelError` 枚举，至少区分以下类别：`ApiError`（Provider API 返回错误）、`RetryExhausted`（重试次数耗尽）、`Cancelled`（调用被取消）、`ValidationError`（输入参数不合法）、`SerializationError`（序列化/反序列化失败）、`FormatError`（消息格式化失败）、`StructuredOutputError`（结构化输出生成失败，含 JSON repair 失败等子情况）。
- **FR-037**: Model 层所有 fallible 操作 MUST 返回 `Result<T, ModelError>` 而非 panic 或静默忽略错误。

#### 依赖拓扑

- **FR-038**: `agent_scope_model` crate 的依赖 MUST 仅包含：`agent_scope_types`、`agent_scope_message`、`agent_scope_utils`。MUST NOT 依赖 `agent_scope_tool`、`agent_scope_agent`、`agent_scope_memory` 等上层模块。
- **FR-039**: model crate MUST 使用 `#![deny(unsafe_code)]`。

### Key Entities

- **ChatModel（trait）**: 所有聊天模型的统一抽象。定义 `call()` 入口方法（含重试和取消逻辑）、抽象的 `_call_api()` 方法、`count_tokens()`、`generate_structured_output()`、`list_models()` 等。Provider 通过实现 `_call_api()` 接入具体 API。
- **ChatResponse**: 模型单次调用的响应。包含内容块列表、流式标志（is_last）、token 用量、完成原因。流式模式下每个 chunk 都是 ChatResponse 实例，通过 StreamAccumulator 汇聚。
- **StructuredResponse**: 结构化输出响应。content 为 JSON dict（而非 ContentBlock 列表），由 `generate_structured_output()` 生成。
- **ChatUsage**: 模型调用的 token 统计。包含输入/输出 token 数、缓存命中 token 数、调用耗时。比 `Usage`（message 层）更丰富。
- **ModelCard**: 模型元信息。描述模型名称、状态、上下文/输出尺寸、参数 Schema。从 YAML 文件加载，支持参数覆盖。
- **ToolChoice**: 工具选择配置。mode 指定是否强制调用工具（auto/none/required/特定工具名），tools 可选择性地过滤可用的工具列表。
- **Formatter（trait）**: 消息格式化抽象。将 Msg 对象转换为 Provider API 格式的字典列表。处理多模态内容提取、不支持媒体类型降级、消息分组。
- **StreamAccumulator**: 流式响应累加器。内部用片段列表（字符串列表、bytes 列表）暂存增量，在 `build()` 时一次性拼接，保证 O(n) 时间复杂度。
- **OpenAIChatModel**: 参考 Provider 实现。连接 OpenAI Chat Completions API，实现完整的流式/非流式调用、重试、格式化。
- **ModelError**: 模型层统一错误类型。区分 API 错误、重试耗尽、取消、参数校验失败等场景，提供稳定的机器可读错误码。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `ChatModel` trait 定义完整，包含 `call()`、`_call_api()`、`count_tokens()`、`generate_structured_output()`、`list_models()` 全部方法签名。
- **SC-002**: `ChatResponse` 的 JSON 序列化格式与 Python 参考实现的输出在差分比较中一致（经 timestamp/id 归一化处理）。
- **SC-003**: `ChatUsage` 的 JSON 序列化格式与 Python 参考实现一致。
- **SC-004**: `ModelCard::from_yaml()` 能正确加载 YAML 文件、合并参数 Schema、应用过滤规则，产出与 Python 实现等价的 ModelCard JSON。
- **SC-005**: `StreamAccumulator` 在流式场景下产生的最终 ChatResponse 与 Python `_StreamAccumulator` 的输出一致（字段级对比）。
- **SC-006**: `OpenAIChatModel` 的流式/非流式响应解析逻辑与 Python 实现等价——相同输入消息、相同 API 响应（mocked）下产出的 ChatResponse 一致。
- **SC-007**: `OpenAIChatFormatter::format()` 的格式化输出与 Python `OpenAIChatFormatter.format()` 在相同 Msg 输入下一致。
- **SC-008**: 所有 39 个功能需求可通过自动化测试验证。
- **SC-009**: `cargo test -p agent_scope_model` 全部通过，0 失败。
- **SC-010**: `cargo tree -p agent_scope_model` 显示依赖仅包含 Foundation 层三个 crate 及标准库/serde/tokio 等框架依赖。

## Assumptions

- Rust 侧不使用 `async-openai` crate 作为硬依赖——OpenAI API 调用通过 `reqwest` + 手动构建 HTTP 请求实现，以保持与 Python 实现中对 API 细节的精确控制一致（如 chunk 解析、tool_choice 回退等）。
- `Formater` trait 的 `format()` 方法返回 `Vec<serde_json::Value>`（而非强类型 struct），以匹配 Provider API 的灵活需求——每个 Provider 的 API 格式差异较大，强类型建模性价比低。
- `StreamAccumulator` 在 model 层内部使用，不作为公开 API 暴露。上层 Agent 通过 `ChatModel::call()` 的 Stream 接口消费 chunk，不直接操作累加器。
- Tool 的 JSON Schema 在 model 层仅以 `serde_json::Value` 形式传递（与 Python 中 `list[dict]` 对应），不做 schema 级别的类型强校验。
- `OpenAIChatModel` 的流式解析中包含 PCM16→WAV header 注入逻辑（与 Python 一致），但音频相关的 `_build_streaming_wav_header` 工具函数不在此 Feature 的主要交付范围内——作为内部辅助函数提供基本实现即可。
- Provider 特定的 SDK 依赖（如 Anthropic Python SDK、Google Generative AI SDK）在 Rust 实现中替换为 `reqwest` + 手动 HTTP 请求。
- JSON Schema 的 flatten（`$ref`/`$defs` 内联）逻辑在 Python 中由 `_flatten_json_schema` 完成，Rust 侧提供等价实现。
- `ModelCard` 的 YAML 加载在 Rust 侧使用 `serde_yaml` crate。
- 本 Feature 不涵盖 Provider 的 credential 管理（API key 加载、环境变量读取等）——在构造 `OpenAIChatModel` 时直接传入 api_key 字符串。

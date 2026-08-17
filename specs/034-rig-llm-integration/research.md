# Research: Rig LLM Provider Integration

**Feature**: [spec.md](spec.md) | **Branch**: `034-rig-llm-integration` | **Date**: 2026-08-17

## 概述

本调研解决 Feature 034 的全部技术不确定性：如何用 [rig](https://github.com/0xPlaygrounds/rig)（v0.42.0）替换 `agent_scope_dashscope`，同时保持 `ChatModel`/`EmbeddingModel` trait 公共 API 与可观察行为不变。核心难点是 rig 的 `CompletionModel` trait 用 RPITIT（返回 `impl Future`），**非对象安全**，而 agent_scope 的 `ChatModel` 以 `Arc<dyn ChatModel>` 贯穿 agent 引擎。本调研确立 boxed backend 桥接、消息/工具/流式/错误映射方案，并明确删除 dashscope 的完整影响面。

## 调研范围与方法

- **对象**：rig-core 0.42.0 源码（`/tmp/rig` 浅克隆）+ agent_scope 侧现有契约源码。
- **方法**：直接阅读 rig `CompletionModel`/`EmbeddingModel` trait、`CompletionRequest`/`Message`/`CompletionResponse`/`StreamingCompletionResponse`、openai/anthropic/deepseek provider 构造；对照 `agent_scope_model::model_trait::ChatModel`、`agent_scope_embedding::EmbeddingModel`、`agent_scope_dashscope` 现有实现，逐项确认映射可行性。
- **结论形态**：每个决策记录 Decision / Rationale / Alternatives considered。

---

## 决策

### 决策 1：crate 结构——单 provider crate `agent_scope_rig`，内含三 backend

- **Decision**: 新建 `crates/agent_scope_rig`，内含 `openai` / `anthropic` / `deepseek` 三个 backend 模块 + 共享映射层（message/tools/stream/structured/error/params）。
- **Rationale**: 三个 provider 均走 rig 统一抽象，共享约 90% 适配代码（消息↔rig `Message`、工具 schema、流式转换、错误分类、结构化输出）。单 crate 消除重复，示例只需一个依赖。rig 层差异仅在 client 构造与模型名/能力（thinking、embedding 有无）。
- **Alternatives considered**:
  - *每 provider 一 crate*（Feature 005 惯例）：自研 provider 各协议差异大时合理；rig-backed provider 差异集中在构造层，拆 crate 徒增样板与 workspace 噪音。拒绝。
  - *直接在 `agent_scope_model` 内实现*：违反宪法第十一条（core 不依赖 provider）与依赖方向。拒绝。

### 决策 2：对象安全 bridge——内部 `RigChatBackend` trait（boxed）

- **Decision**: `agent_scope_rig` 内部定义对象安全 trait：
  ```rust
  #[async_trait]
  pub trait RigChatBackend: Send + Sync {
      async fn completion(
          &self,
          request: CompletionRequest,
      ) -> Result<CompletionResponse, CompletionError>;
      async fn stream(
          &self,
          request: CompletionRequest,
      ) -> Result<StreamingCompletionResponse, CompletionError>;
  }
  ```
  三个 provider（`OpenAiBackend`/`AnthropicBackend`/`DeepSeekBackend`）实现之；`RigChatModel` 持有 `Arc<dyn RigChatBackend>` 实现 `ChatModel`。
- **Rationale**: rig `CompletionModel` 的 `completion()`/`stream()` 返回 `impl Future`（RPITIT），无法直接 `Arc<dyn CompletionModel>`。`RigChatBackend` 用 `async_trait` 返回具体类型（boxed future），既是对象安全又隐藏 provider 泛型。符合宪法第八条（动态扩展点优先 trait object，不暴露复杂泛型）；`ChatModel` 的 `Arc<dyn ChatModel>` 契约零改变，agent 引擎零感知。
- **Alternatives considered**:
  - *让 `RigChatModel` 泛型化*（`RigChatModel<M: CompletionModel>`）：迫使 agent 侧 `AgentConfig::model` 接受泛型而非 `Arc<dyn ChatModel>`，侵入 config/react_loop 全链路，破坏宪法第一/十一条。拒绝。
  - *直接 `Box<dyn CompletionModel + Send>` 并 raw pointer 转换*：需要 unsafe + 类型擦除魔法，违反宪法第九条。拒绝。

### 决策 3：公开构造入口——`RigChatModel::openai/anthropic/deepseek` + 链式配置

- **Decision**: 公开构造器与现有 `DashScopeChatModel` 同级 ergonomics：
  ```rust
  let model = RigChatModel::openai(api_key, "gpt-4.1")
      .with_stream(true)
      .with_base_url("https://api.openai.com/v1");  // 可选，默认官方
  let model = RigChatModel::anthropic(api_key, "claude-sonnet-4-5");
  let model = RigChatModel::deepseek(api_key, "deepseek-chat");
  // embedding（OpenAI 专用）
  let embed = RigEmbeddingModel::openai(api_key, "text-embedding-3-small");
  ```
- **Rationale**: `ChatModel` 构造是用户首个接触面，FR-003 要求"与当前相当的开销"。每 provider 一个类型化关联函数（返回 `Self`），内部构造对应 rig client，封装泛型细节。默认值与现状对齐：`stream=true`、`max_retries=3`、`retry_delay=1.0`、`context_size=131072`（OpenAI gpt-4.1 128k；Anthropic/DeepSeek 各自模型族默认按 provider 覆盖）。
- **Alternatives considered**:
  - *暴露 rig 的 `ClientBuilder` 风格*（`RigChatModel::new().api_key().build()`）：多一层 builder 与现状 new(key, model) 不一致，增加样板。拒绝。
  - *单一 `RigChatModel::new(provider, key, model)` 枚举分发*：provider 枚举使运行时分支，且各 provider 能力位（thinking/embedding）编码更绕。关联函数更静态、更安全。采纳关联函数方案。

### 决策 4：消息映射——`Msg`/`ContentBlock` ↔ rig `Message`

- **Decision**: `message.rs` 实现双向映射：
  - `Msg{role,content}` → rig `Message`：
    - `role=User`：`Message::User{content, images, name}`；`ContentBlock::Text(tb)` → `UserTextContent(text)`；`ContentBlock::Data(db)` → `images`（多模态），`name` 取 `Msg.name`。
    - `role=Assistant`：`Message::Assistant{content, thinking, images, name, tool_calls}`；`ContentBlock::Text`→`content`；`ContentBlock::Thinking(tb)`→`thinking`（仅发送，见决策 9）；`ContentBlock::ToolCall(tc)`→`tool_calls: Vec<ToolCall>`（含 id/name/arguments）。
    - `role=System`：`Message::System{content}`。
  - 工具结果：`ContentBlock::ToolResult(tr)`（在 `role=User` 消息内，符合 OpenAI/Anthropic 惯例）→ `Message::ToolResult{name, content}`（rig 的 `Message::ToolResult` 独立于 role）。**注意**：rig 要求 ToolResult 消息紧跟对应 ToolCall；agent_scope 的 ToolResultBlock 在 `User` 消息中，映射时展开为独立 `Message::ToolResult`，并保证顺序（映射为对话中每个 tool_result 一条）。
  - rig `Message` → `Msg`：`Message::User`→User 消息（images 转 DataBlock）；`Message::Assistant{content,thinking,tool_calls}`→Assistant 消息（Text→TextBlock、thinking→ThinkingBlock、tool_calls→ToolCallBlock，`input` 序列化 `arguments:Value` 为 JSON 字符串）；`Message::ToolResult`→User 消息 + ToolResultBlock；`Message::System`→System 消息。
  - **Hint 策略**：`ContentBlock::Hint`（框架内部指令）**不发送**到 provider，与现有 DashScopeFormatter 一致（hint 仅框架内消费）。
  - **Unknown 策略**：映射错误（无法识别的 block）→ `ModelError::FormatError`，不静默丢弃。
- **Rationale**: rig `Message` 枚举直接对应 OpenAI/Anthropic/DeepSeek 三种 wire 格式，用其作为归一化中间态，三 provider 共享同一映射。ToolResult 的独立 `Message::ToolResult` 是 rig 归一化产物，避免在 agent_scope 侧改消息模型。
- **Alternatives considered**:
  - *保留 dashscope 自研 formatter 逐 provider 格式化*：违背 FR-002（rig 取代自研 HTTP/格式化代码）。拒绝。
  - *改造 `Msg` 结构以贴近 rig*：破坏宪法第十二条（稳定数据协议）。拒绝。

### 决策 5：工具映射——OpenAI function schema ↔ rig `ToolDefinition`；`ToolChoice` 归一化

- **Decision**:
  - 工具 schema 输入是 `&[JsonValue]`（OpenAI function-calling 格式：`{"type":"function","function":{"name","description","parameters"}}`）。`tools.rs` 将其转为 rig `ToolDefinition{name, description, parameters: Value, strict}`，供 `CompletionRequest.tools`。
  - `ToolChoice{mode, tools}` 归一化为 rig 请求参数：
    - `mode=auto`→ rig 默认（不传 tool_choice）/ 显式 auto；`mode=none`→禁用工具；`mode=required`→rig `ToolChoice::Required`；`mode=specific_tool(name)`→rig `ToolChoice::Specific(name)`。
    - `tools` 子集过滤：在转换时仅保留 `tools` 列出的工具（延续 round-4 M18 修复的语义）。
    - **thinking 互斥**：见决策 9。
- **Rationale**: rig `ToolDefinition` 与 OpenAI function schema 一一对应，`parameters` 保留原始 JSON，零信息丢失；`CompletionRequest.tool_choice` 接受 rig 枚举，直接映射四模式。
- **Alternatives considered**: 用 `OpenAICompatibleProvider` 自定义格式——不适用，OpenAI/Anthropic/DeepSeek 均为 rig 原生 provider，无需兼容层。拒绝。

### 决策 6：流式映射——rig `StreamingCompletionResponse` → `Stream<ChatResponse>`

- **Decision**: `stream.rs` 消费 rig `Stream<Item=Result<StreamedAssistantContent, CompletionError>>`，逐项转换为 `ChatResponse` 增量流：
  - `StreamedAssistantContent::Text(text)` → `ChatResponse` 含 `TextBlock`（增量拼接，block_id 稳定）。
  - `StreamedAssistantContent::Reasoning{reasoning, id}` / `ReasoningDelta` → `ThinkingBlock` 增量（extras 记 reasoning id）。
  - `StreamedAssistantContent::ToolCall{tool_call, internal_call_id}` → `ToolCallBlock`（name/arguments 完整到达时建立；若分片则由 `ToolCallDelta` 增量拼接）。
  - `StreamedAssistantContent::ToolCallDelta` → 按 `internal_call_id` 拼接到对应 `ToolCallBlock`。
  - `StreamedAssistantContent::Image` → `DataBlock`。
  - **流末**：末个 chunk 置 `is_last=true`，填 `finished_reason`（由 rig `finish_reason` 映射，见决策 10 的终态映射），填 `usage`（聚合 `StreamingCompletionResponse::usage()`，流结束后可取）。
  - **tool_call_id_map**：`tc_{idx}` → provider 工具调用 id（rig `ToolCall.id`），在流末写入 `ChatResponse.tool_call_id_map`（不序列化，供内部 tool result 回填）。
  - 每个产出的 `ChatResponse` 用独立 `id`（默认 `ChatResponse::default()` 生成），`response_type="chat_response"`。
  - 错误：stream 中 `Err(CompletionError)` → `Err(ModelError)`（映射见决策 10）。
- **Rationale**: agent 引擎（`react_loop`/`streaming_reactor`/`StreamAccumulator`）消费的 `Stream<Item=Result<ChatResponse,ModelError>>` 语义不变；rig 把 token 级 delta 与聚合后的 `AssistantContent` 都给出，流式转换器在 block 增量层拼接，与现有 DashScope SSE 解析产出的 block 增量语义一致。
- **Alternatives considered**: 用 rig `StreamingCompletionResponse.choices()`（聚合的完整 choice 序列）直接产出整块 `ChatResponse`——丢失增量体验（前端逐 token 展示依赖 TextBlock 增量）。拒绝，采用 delta 拼接。

### 决策 7：结构化输出——优先 rig `output_schema`，回退 tool-calling bypass

- **Decision**: `structured.rs` 覆写 `ChatModel::generate_structured_output`：
  1. **原生路径**：把 flatten 后的 JSON schema 填入 `CompletionRequest.output_schema`（rig 原生支持），走 `completion()`；解析 `CompletionResponse` 中符合 schema 的 `AssistantContent`（文本/JSON）。provider 不支持时（运行时或编译期探知）进入回退。
  2. **回退路径**：沿用 trait 默认 tool-calling bypass（注入 `generate_structured_output` 工具 + `tool_choice=required`），JSON repair（`json_repair`）保留。
  3. 返回 `StructuredResponse{content, usage, ...}`，契约与现有实现一致。
- **Rationale**: rig 的 `output_schema` 是官方结构化输出通道（OpenAI json_schema 等），质量优于工具 bypass；回退保证 provider 差异不破坏契约（宪法第五条：不支持显式记录）。与现有 `call_api_with_structured_output` 保持 `StructuredResponse` 结构。
- **Alternatives considered**: 一律 tool-calling bypass（默认实现）——浪费 rig 原生能力且与 FR-002 精神相悖（rig 接管请求构造）。拒绝。

### 决策 8：embedding——`RigEmbeddingModel`（OpenAI）

- **Decision**: `RigEmbeddingModel` 包装 rig openai `embedding_model(model_name)`（`text-embedding-3-*`），实现 `agent_scope_embedding::EmbeddingModel`：
  - `embed(Vec<EmbeddingInput>)`：`Text(s)` → rig `embed_text(s)`；`DataBlock(_)` → `EmbeddingError::MultimodalNotSupported`（model_card.supports_multimodal=false）。
  - `model_card()`：`EmbeddingModelCard::new(model_name, ndims, false)`；`ndims` 由 rig `EmbeddingModel::ndims()` 探知。
  - 批量：rig `embed_texts` 支持批；`MAX_DOCUMENTS` 由 rig trait 提供。
  - 构造：`RigEmbeddingModel::openai(api_key, model)`。
- **Rationale**: 三家 provider 中仅 OpenAI 提供 embedding（FR-007/Assumptions）；OpenAI 官方 embedding 模型语义与现有 DashScope text-embedding 兼容（同一 OpenAI-compatible 协议）。`EmbeddingModel` trait 契约（每输入一向量、长度=dimensions、multimodal 限制）逐条满足。
- **Alternatives considered**: Anthropic 无 embedding、DeepSeek 无 embedding（原生）——维持 OpenAI 唯一 embedding provider 决策，不引入自定义扩展。记录为已知限制（宪法第五条）。

### 决策 9：thinking 适配——按 provider 能力位处理，互斥规避迁移为按 provider 适配

- **Decision**: `params.rs` 定义 provider 能力位（`supports_thinking`/`thinking_tool_choice_incompatible`），构造时确定：
  - **Anthropic**：`thinking` 走 `Assistant.thinking`（rig anthropic provider 支持 extended thinking 配置）；extended thinking 与 tool use 的互斥由 rig/provider 版本处理，`tool_choice=required` 时若 provider 拒绝则回退 `auto`（记录为 provider 差异）。
  - **OpenAI o 系列**：reasoning 内容经 rig 流入 `StreamedAssistantContent::Reasoning` → `ThinkingBlock`；OpenAI reasoning 不与 tool_choice 冲突，无互斥处理。
  - **DeepSeek reasoner**：reasoning_content 经 rig 归一化为 reasoning delta → `ThinkingBlock`；与 tool_choice 无冲突。
  - **迁移现状互斥逻辑**：现有 `DashScopeParameters::is_thinking_enabled` 的 `enable_thinking && tool_choice="required"` 互斥检查迁移为 `RigChatModel` 内部的按 provider 守卫——当 provider 声明 `thinking_tool_choice_incompatible=true` 且请求带 `required` 时，按用户文档记录的语义降级并记 tracing 事件（不静默，宪法第五/十四条）。
- **Rationale**: thinking 是能力面（Q3）一部分，三 provider 推理内容均能映射到 `ThinkingBlock`；互斥是 provider 特性而非框架特性，按能力位适配比硬编码 DashScope 更通用。
- **Alternatives considered**: 保留 `enable_thinking` 布尔开关并只在 DashScope 语义下工作——新 provider 无 DashScope，需通用化。采纳能力位方案。

### 决策 10：错误映射——rig `CompletionError` → `ModelErrorKind`

- **Decision**: `error.rs` 把 rig 错误映射到现有分类（`ModelError`/`ModelErrorKind` 不变，宪法第十三条）：
  - HTTP 状态分类（rig `ApiError` 携带 provider 响应）：401/403→`Authentication`；429→`RateLimit`；5xx→`InternalServer`；4xx→`BadRequest`。
  - 连接/传输（reqwest 层）：连接失败→`ApiConnection`；超时→`ApiTimeout`。
  - 解析/流式（rig `ResponseError`/`StreamError`）→`FormatError`（context="rig:stream" 或 "rig:response"）。
  - 工具/结构化输出（rig `ToolError`/`OutputSchemaError`）→`StructuredOutputError` 或 `FormatError`。
  - `RetryExhausted` 由 `ChatModel::call` 默认重试循环产生，rig 不介入。
- **Rationale**: 错误模型是公共契约，agent 引擎按 `ModelErrorKind` 分类决策（重试/终止/用户提示）。rig 错误多为对 HTTP/解析层的包装，映射到现有六分类即可保持语义。
- **Alternatives considered**: 透传 rig 错误类型——破坏宪法第十三条，agent 引擎与 middleware 无感知。拒绝。

### 决策 11：删除 dashscope——影响面清单与迁移顺序

- **Decision**: 删除 `crates/agent_scope_dashscope`，影响面：
  1. 根 `Cargo.toml`（workspace members + root deps + `agent_scope_dashscope` re-export）→ 移除，加入 `agent_scope_rig`。
  2. 7 示例（agent/chat/human-in-the-loop/plan-react-agent/quickstart/rag/subagent）：`Cargo.toml` 依赖 + `main.rs` 构造行换 `RigChatModel::openai`/`RigEmbeddingModel::openai`；环境变量 `DASHSCOPE_API_KEY` → `OPENAI_API_KEY`。
  3. dashscope 特有测试契约：`model_tests.rs`/`embedding_tests.rs` 中与 provider 无关的契约（错误映射、消息格式化、流式 block 顺序）迁移到 `agent_scope_rig` 测试；DashScope 专有断言删除。
  4. 文档：README、`docs/rust/zh`、`agentscope-guide` skill、相关 specs 的 `agent_scope_dashscope` 引用清理/改写为 `agent_scope_rig`；兼容性矩阵 `provider-*` 条目改写为 OpenAI/Anthropic/DeepSeek。
  5. `pi-rust`（已移出主工作树）不在本次范围。
- **Rationale**: dashscope 是叶子 provider（agent 引擎只依赖 `ChatModel` trait），删除面精确可控。先完成 `agent_scope_rig` + 示例迁移 + 文档，再删 crate（FR-001 的"删除是迁移完成的产物"）。
- **Alternatives considered**: 保留 dashscope crate 并存一段时间（兼容期）——spec 明确 MUST 删除（US1），且保留会分裂 provider 面。拒绝。

### 决策 12：OpenAI backend 协议选择——`CompletionsClient`（Chat Completions）为首选

- **Decision**: OpenAI backend 使用 rig `openai::CompletionsClient`（Chat Completions API），而非默认 `openai::Client`（Responses API）。
- **Rationale**: agent_scope 现有工具/流式格式全部是 OpenAI Chat Completions 语义（function calling schema、delta 流、`tool_choice` 枚举）；DashScope 兼容端点也是该协议。`CompletionsClient` 使 OpenAI 路径与 Anthropic/DeepSeek 的 wire 语义（均为 chat 补全风格）最一致，映射层单一。Responses API 的工具调用格式（output items）与现有 `ToolCallBlock` 生命周期差异较大，迁移成本与行为偏差风险高。
- **Alternatives considered**: `openai::Client`（Responses API）——rig 默认路径、更新，但工具调用/流式格式需额外适配且 OpenAI 仍提供 Chat Completions 兼容端点。记录为 future 备选（OpenAI 弃用 Chat Completions 时切换）。

---

## 已解决的不确定性（原 NEEDS CLARIFICATION）

| 不确定点 | 结论 |
|----------|------|
| rig `CompletionModel` 非对象安全，如何实现 `Arc<dyn ChatModel>`？ | 内部 `RigChatBackend` boxed 桥接（决策 2） |
| 新 provider 放哪、几个 crate？ | 单 `agent_scope_rig` 含三 backend（决策 1） |
| OpenAI 用 Responses 还是 Completions？ | `CompletionsClient`（决策 12） |
| thinking 与 tool_choice 互斥如何通用化？ | 按 provider 能力位守卫 + tracing 事件（决策 9） |
| `ContentBlock::Hint` 如何处理？ | 不发送，框架内部消费（决策 4） |
| Anthropic/DeepSeek 无 embedding 怎么办？ | 仅 OpenAI 提供 embedding，记录已知限制（决策 8） |
| 7 示例/文档/环境变量迁移范围？ | 决策 11 完整清单 |
| 测试如何保证确定性（不依赖真实 LLM）？ | mock HTTP server（wiremock/rig 自测桩）回放固定响应；消息/工具/流式/错误映射单元测试（决策 6/10，见 quickstart.md） |

## 依赖与最佳实践

- **rig 0.42.0**（锁定）：`rig`（default features，含 openai/anthropic/deepseek providers）。评估：开源 MIT、活跃维护、`rig-core` 抽象成熟；依赖无重复、无循环（仅 `agent_scope_rig` 引入）。license 检查纳入 CI（FR-011）。
- **mock 策略**：映射层纯函数单测（消息/工具/错误无网络）；backend 用本地 HTTP server（`wiremock` 或 rig 自带的 test doubles）回放固定 OpenAI-compatible 响应验证请求构造与流式转换（宪法第六条）。
- **`#![deny(unsafe_code)]`**（宪法第九条）；core 层零新增依赖（宪法第十一条）。
- 现有 `agent_scope_model` 的 provider 无关辅助（formatter/json_repair/schema_flat）继续复用，不清理（spec Assumptions）。

## 实现时需验证的点（进入 tasks 阶段时核实）

1. rig 0.42.0 `CompletionRequest` 字段的确切 setter 名称与 `tool_choice` 枚举路径（`CompletionRequest` builder vs 字段构造）。
2. rig `CompletionError` 完整变体清单（HTTP 分类字段位置），确保映射穷尽。
3. rig openai `CompletionsClient` 的流式/工具调用是否完整（与 Responses API 功能差集）；若 Completions 工具调用受限，决策 12 回退到 Responses API 并记录偏差。
4. rig `StreamingCompletionResponse::usage()` 的时机（流结束前/后）与 `finish_reason` 来源字段。
5. Anthropic extended thinking 与 tool use 并发的当前约束（rig/provider 版本），确定 `thinking_tool_choice_incompatible` 的实际值。
6. DeepSeek reasoner 的 thinking 内容是否经 rig 流入 `Reasoning`（而非 text）；若否，记录为 provider 差异。
7. 现有 `agent_scope_dashscope` 测试中哪些是"provider 无关契约"可迁移，哪些是 DashScope 专有需删除。

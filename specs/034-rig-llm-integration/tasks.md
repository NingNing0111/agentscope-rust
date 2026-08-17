---

description: "Task list for Feature 034: Rig LLM Provider Integration"
---

# Tasks: Rig LLM Provider Integration（用 rig 完成 LLM 接入，移除 dashscope provider）

**Input**: Design documents from `/specs/034-rig-llm-integration/`

**Prerequisites**: [plan.md](plan.md) (required)、[spec.md](spec.md) (required)、[research.md](research.md)、[data-model.md](data-model.md)、[contracts/provider-adapter.md](contracts/provider-adapter.md)、[contracts/rig-mapping.md](contracts/rig-mapping.md)、[quickstart.md](quickstart.md)

**Tests**: 宪法第六条 + spec FR-010 明确要求确定性测试 → 测试任务是必需项（mock/recorded 组件，不依赖真实 LLM）。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to（US1–US4 对应 spec.md）
- Include exact file paths in descriptions

---

## Phase 1: Setup（共享基础设施）

**Purpose**: 依赖引入与 `agent_scope_rig` crate 初始化

- [X] T001 在根 `Cargo.toml` 的 `workspace.dependencies` 添加 `rig = "0.41"`（**勘误：原任务记 0.42，crates.io 实际最新为 0.41.0**，含版本锁定注释与 license 备注），并把 `crates/agent_scope_rig` 加入 `workspace.members`（经 `cargo check -p agent_scope_rig` 验证通过）；同时把 `agent_scope_rig` 加入根 package 依赖（暂不删 `agent_scope_dashscope`，删除在 US1）
- [X] T002 创建 `crates/agent_scope_rig/Cargo.toml`（deps: `rig`、`agent_scope_model`、`agent_scope_message`、`agent_scope_embedding`、`async-trait`、`futures`、`serde_json`、`tracing`、`tokio`；dev-deps: `wiremock`、`serde`）与 `crates/agent_scope_rig/src/lib.rs`（首行 `#![deny(unsafe_code)]` + 空模块声明 `pub mod backend; pub mod error; pub mod message; pub mod openai; pub mod params; pub mod stream; pub mod structured; pub mod tools;`，各模块空文件占位，保证 `cargo check -p agent_scope_rig` 通过）
- [X] T003 [P] 依赖治理（FR-011）：核实 `cargo tree -i rig` 依赖图、license、维护/安全评估，结果记录到 `specs/034-rig-llm-integration/dependencies.md`（含实际编译树：rig-core/rig-agent 0.41.0 + reqwest 0.13.4 + rustls 0.23，rmcp 未编译）

---

## Phase 2: Foundational（阻塞性前置）

**Purpose**: 对象安全桥接 + 映射层（US2/US3/US4 共用基础），MUST 在任一 story 前完成

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T004 定义 `crates/agent_scope_rig/src/backend.rs`：`RigProviderKind`（OpenAi/Anthropic/DeepSeek）+ `RigProviderCapabilities{supports_thinking, thinking_tool_choice_incompatible, supports_embedding}` + 对象安全 trait `RigChatBackend`（async_trait：`capabilities()`、`completion(CompletionRequest)->Result<CompletionResponse,CompletionError>`、`stream(CompletionRequest)->Result<StreamingCompletionResponse,CompletionError>`）+ `RigEmbeddingBackend`（`ndims()`、`embed_texts(Vec<String>)`、`embed_text(&str)`），按 [contracts/rig-mapping.md](contracts/rig-mapping.md) 决策 2 的签名
- [X] T005 实现 `crates/agent_scope_rig/src/message.rs` 出站映射：`Msg`/`ContentBlock` → rig `Message`（User/Assistant/System、Thinking→`Assistant.thinking`、ToolCall→`assistant.tool_calls`（`arguments: Value` 解析）、ToolResult 展开为独立 `Message::ToolResult` 且紧随对应 ToolCall、Data→`images`、Hint 不发送、Unknown→`ModelError::FormatError`），对照 [contracts/rig-mapping.md](contracts/rig-mapping.md) §1.1
- [X] T006 [P] 实现 `crates/agent_scope_rig/src/message.rs` 入站映射（非流式）：rig `AssistantContent`（Text/Reasoning/ToolCall/Image）→ `ChatResponse.content`（`ContentBlock::Text/Thinking/ToolCall/Data`），空 choice→`EmptyResponse`，对照 §1.2
- [X] T007 实现 `crates/agent_scope_rig/src/tools.rs`：`&[JsonValue]` OpenAI function schema → rig `ToolDefinition{name,description,parameters,strict}`（缺 `function` 包裹→`FormatError`）；`ToolChoice`（auto/none/required/specific_tool + tools 子集过滤）→ rig `tool_choice`，对照 §2
- [X] T008 实现 `crates/agent_scope_rig/src/params.rs`：`RigParameters{max_tokens,temperature,top_p,top_k,seed,stop,thinking_budget,additional_params}` → `CompletionRequest` 顶层字段 + `additional_params`，对照 §3
- [X] T009 实现 `crates/agent_scope_rig/src/error.rs`：rig `CompletionError` → `ModelError`/`ModelErrorKind` 全分类映射（401/403→Authentication、429→RateLimit、5xx→InternalServer、4xx→BadRequest、连接→ApiConnection、超时→ApiTimeout、解析/流式→FormatError、工具/schema→StructuredOutputError、空响应→EmptyResponse），错误消息不泄露 key，对照 §4
- [X] T010 [P] 测试 `crates/agent_scope_rig/tests/message_mapping_tests.rs`：各 role 往返、Thinking、ToolCall arguments 解析、ToolResult 展开顺序、Hint 不发送、Unknown→FormatError（先 FAIL 后实现）
- [X] T011 [P] 测试 `crates/agent_scope_rig/tests/tools_mapping_tests.rs`：schema→ToolDefinition、ToolChoice 四模式 + 子集过滤（先 FAIL 后实现）
- [X] T012 [P] 测试 `crates/agent_scope_rig/tests/error_mapping_tests.rs`：§4 全部分类映射 + key 不泄露（先 FAIL 后实现）

**Checkpoint**: Foundation ready - user story implementation can now begin

---

## Phase 3: User Story 2 - rig-backed OpenAI 聊天 agent（Priority: P1）🎯 MVP

**Goal**: 用户能以 API key + 模型名（+ 可选流式开关）构造 rig-backed OpenAI 聊天模型并跑 ReAct agent（FR-002/003/007，示例改造前的新能力主体）

**Independent Test**: `cargo test -p agent_scope_rig`（openai/streaming/structured）+ `cargo check -p agent_scope_rig`；构造入口冒烟（quickstart 场景 1/2）

### Tests for User Story 2 ⚠️（确定性组件，宪法第六条；先 FAIL 后实现）

- [X] T013 [P] [US2] 测试 `crates/agent_scope_rig/tests/streaming_tests.rs`：确定性流式 chunk → `ChatResponse` 顺序断言（Reasoning→Thinking 增量、Text 增量、ToolCall/Delta 按 `internal_call_id` 拼接、`is_last` 只在末 chunk、流末 `usage`/`finished_reason`/`tool_call_id_map` 填充），对照 [contracts/rig-mapping.md](contracts/rig-mapping.md) §5
- [X] T014 [P] [US2] 测试 `crates/agent_scope_rig/tests/structured_output_tests.rs`：`generate_structured_output`（output_schema 原生 + 工具 bypass 回退 + JSON repair + 空消息→ValidationError），对照 §6
- [X] T015 [US2] 测试 `crates/agent_scope_rig/tests/openai_tests.rs`：mock HTTP（wiremock 回放固定 OpenAI-compatible 响应）验证 `OpenAiBackend` 请求体形状（model/messages/tools/tool_choice）、流式解析、错误分类（401/429/500→ModelErrorKind）

### Implementation for User Story 2

- [X] T016 [US2] 实现 `crates/agent_scope_rig/src/openai.rs`：`OpenAiBackend`（rig `openai::CompletionsClient`，Chat Completions 协议，见 research 决策 12）实现 `RigChatBackend`——`completion()`/`stream()` 经 `CompletionRequest` 调用 rig，`capabilities()`（OpenAI：thinking=o 系 true、embedding=true）
- [X] T017 [US2] 实现 `crates/agent_scope_rig/src/lib.rs`：`RigChatModel`（持有 `Arc<dyn RigChatBackend>` + `RigChatModelConfig`）实现 `ChatModel`——`model_name`/`stream_enabled`/`max_retries`/`retry_delay`/`context_size`/`retryable_errors`/`call_api`（非流式→`completion()` + 入站映射 → `ModelCallResult::Complete`；流式→`stream()` + `stream.rs` 转换 → `ModelCallResult::Stream`；构造校验 api_key 非空/base_url 合法→`ValidationError`），含 `RigChatModel::openai(api_key, model)` 构造器与 `.with_stream()`/`.with_base_url()`/`.with_parameters()` 链式方法，对照 [contracts/provider-adapter.md](contracts/provider-adapter.md) §1/§2
- [X] T018 [US2] 实现 `crates/agent_scope_rig/src/stream.rs`：rig `StreamingCompletionResponse`（`Stream<Item=Result<StreamedAssistantContent,CompletionError>>`）→ `Stream<Item=Result<ChatResponse,ModelError>>` 增量转换器（Text/Reasoning/ToolCall/ToolCallDelta/Image 分派、稳定 block_id、流末 is_last/usage/finish_reason/tool_call_id_map），对照 §5
- [X] T019 [US2] 实现 `crates/agent_scope_rig/src/structured.rs`：覆写 `ChatModel::generate_structured_output`——`flatten_json_schema_with_defs_checked` 后优先填 `CompletionRequest.output_schema` 原生路径，provider 不支持时回退工具 bypass + `json_repair`，返回 `StructuredResponse`，对照 §6

**Checkpoint**: 构造 `RigChatModel::openai` 后可独立完成一次完整回复（非流式 + 流式）+ 结构化输出

---

## Phase 4: User Story 3 - embedding / RAG via rig-backed OpenAI（Priority: P2）

**Goal**: RAG/知识库继续可用——OpenAI `text-embedding-3-*` 提供 embedding，缓存行为保留（FR-007，US3）

**Independent Test**: `cargo test -p agent_scope_rig`（embedding）+ `cargo check -p agent_scope_rig`

### Tests for User Story 3 ⚠️（先 FAIL 后实现）

- [X] T020 [P] [US3] 测试 `crates/agent_scope_rig/tests/embedding_tests.rs`：`embed(Vec<Text>)` 每输入一向量、长度=model_card().dimensions；`DataBlock`→`EmbeddingError::MultimodalNotSupported`；`model_card` 稳定；与 `agent_scope_embedding::cache::FileEmbeddingCache` 集成往返

### Implementation for User Story 3

- [X] T021 [US3] 实现 `crates/agent_scope_rig/src/lib.rs` + `src/openai.rs`：`RigEmbeddingModel`（包装 rig openai `embedding_model(model_name)`，`ndims()` 探知维度）实现 `agent_scope_embedding::EmbeddingModel`——`embed()`（`Text`→`embed_text`/`embed_texts` 批、`DataBlock`→`MultimodalNotSupported`）、`model_card()`（`EmbeddingModelCard::new(model, ndims, false)`），构造入口 `RigEmbeddingModel::openai(api_key, model)`，对照 [contracts/provider-adapter.md](contracts/provider-adapter.md) §2 与 [contracts/rig-mapping.md](contracts/rig-mapping.md) §7

**Checkpoint**: `RigEmbeddingModel::openai` 可独立完成文本嵌入，缓存层无回归

---

## Phase 5: User Story 1 - 移除 DashScope provider crate（Priority: P1）🎯

**Goal**: 删除 `agent_scope_dashscope`，7 示例迁移到 `agent_scope_rig::RigChatModel::openai`（FR-001/008）。⚠️ 本阶段删除动作依赖 US2（聊天模型）与 US3（embedding）实现就绪（research 决策 11：删除是迁移完成的产物）

**Independent Test**: `cargo build --workspace` 通过 + `grep -r agent_scope_dashscope` 零命中（code + manifests）

### Implementation for User Story 1

- [X] T022 [US1] 根 `Cargo.toml`：从 `workspace.members` 与根 package deps 移除 `agent_scope_dashscope`；`crates/agent_scope_dashscope/` 目录整体删除
- [X] T023 [P] [US1] 示例迁移（quickstart/chat/agent）：各自 `Cargo.toml` 依赖 `agent_scope_dashscope`→`agent_scope_rig`，`src/main.rs` 构造行 `DashScopeChatModel::new(key, model)`→`RigChatModel::openai(key, model)`（保留 `.with_stream()` 调用），环境变量 `DASHSCOPE_API_KEY`→`OPENAI_API_KEY`，对照 [contracts/provider-adapter.md](contracts/provider-adapter.md) §1 等价表
- [X] T024 [P] [US1] 示例迁移（human-in-the-loop/plan-react-agent/subagent）：同 T023 模式，`Cargo.toml` + `src/main.rs` 构造行 + 环境变量
- [X] T025 [P] [US1] 示例迁移（rag）：`examples/rag/Cargo.toml` + `src/main.rs`——聊天模型 `DashScopeChatModel`→`RigChatModel::openai`、`DashScopeEmbeddingModel`→`RigEmbeddingModel::openai`，环境变量换 `OPENAI_API_KEY`
- [X] T026 [US1] dashscope 测试契约处置：审查 `crates/agent_scope_dashscope` 内测试（`model.rs`/`embedding.rs` 单测、`model_tests.rs`），provider 无关契约（错误映射/消息格式化/流式 block 顺序）迁移到 `agent_scope_rig` 对应测试；DashScope 专有断言（enable_search、qwen 参数）删除
- [X] T027 [US1] 验证：`cargo build --workspace` + `cargo test --workspace`（移除依赖后无悬空引用）+ `grep -r "agent_scope_dashscope" --include="*.rs" --include="Cargo.toml"` 全仓零命中（specs 历史文档可保留注明"已移除"）

**Checkpoint**: workspace 无 `agent_scope_dashscope` 残留，7 示例全部用 `agent_scope_rig`

---

## Phase 6: User Story 4 - 可观察行为保留（Priority: P2）

**Goal**: provider 替换后事件顺序/工具生命周期/结构化输出/重试错误语义等价（FR-004/010），已知偏差登记（FR-006/007/011，SC-004）

**Independent Test**: `cargo test -p agent_scope_rig` + 确定性端到端 `rig_e2e` + 兼容矩阵 `provider-*` 条目更新

### Tests for User Story 4 ⚠️（先 FAIL 后实现）

- [X] T028 [P] [US4] 测试 `crates/agent_scope_rig/tests/rig_e2e_tests.rs`：mock HTTP 驱动 rig-backed 模型跑 ReAct 循环（`agent_scope_agent`），断言消息 delta→工具调用→工具结果→结束的事件顺序、`tool_call_id_map` 回填、`is_last`/`finished_reason`/`usage`，对照 [contracts/rig-mapping.md](contracts/rig-mapping.md) §5 顺序契约与 quickstart 场景 4
- [X] T029 [P] [US4] 测试 `crates/agent_scope_rig/tests/error_mapping_tests.rs`（补充）：重试语义——mock 返回 429/500→`ChatModel::call` 重试 3 次后 `RetryExhausted`（或按 `retryable_errors` 收敛）；401→不重试直接 `Authentication`

### Implementation for User Story 4

- [X] T030 [US4] thinking 互斥能力位落地：`RigChatModel::call_api` 中当 `capabilities().thinking_tool_choice_incompatible==true` 且请求带 `ToolChoice::required` 时降级 `auto` 并 `tracing::info!` 事件（不静默）；Anthropic extended thinking 与 tool use 并发约束按 research 验证点 5 核实后定值（写入 `backend.rs` 能力位常量 + 注释引述研究结论）
- [X] T031 [US4] 兼容性矩阵更新：`specs/001-compatibility-baseline/capability-matrix.json` ——移除 `provider-dashscope-*` 条目（实际不存在于矩阵，仅 Python 上游符号镜像 `model-dash-scope-chat-model` 等保留），新增 `provider-openai-rig-adapter`/`provider-anthropic-rig-adapter`/`provider-deepseek-rig-adapter` 条目登记能力覆盖与已知限制（仅 OpenAI embedding、enable_search 不迁移、thinking 互斥按 provider），对照 [contracts/provider-adapter.md](contracts/provider-adapter.md) §4/§5

**Checkpoint**: 确定性端到端事件顺序与迁移前基线一致；兼容矩阵无未登记 UnsupportedFeature

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: 文档/示例/全仓验收收尾

- [X] T032 [P] 文档更新：`README.md` + `docs/rust/zh/` provider 接入章节（构造示例改 `RigChatModel::openai/anthropic/deepseek`、环境变量、能力矩阵链接），全仓 `grep -r "DashScopeChatModel" README.md docs/` 清理
- [X] T033 [P] 文档更新：`agentscope-guide` skill 中 dashscope 构造示例改写；`CHANGELOG.md` 记录 Feature 034
- [X] T034 全仓验收：`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check` 全部通过；quickstart 场景 3/6 命令执行通过
- [X] T035 依赖治理复核（FR-011）：`cargo tree -i rig` 确认 rig 仅经 `agent_scope_rig` 引入、无重复/循环依赖；宪法第十七条完成定义 checklist 逐项确认（spec 已批、plan 一致、任务全完成、单测通过、无静默降级、文档更新、示例可编译、兼容矩阵已更新、clippy/fmt 通过、无未登记 UnsupportedFeature）

---

## Dependencies & Execution Order

### Phase Dependencies

```text
Phase 1 Setup ──► Phase 2 Foundational ──► US2 (Phase 3) ──► US3 (Phase 4)
                                                        │
US1 (Phase 5) ◄── 依赖 US2 + US3 实现就绪 ◄────────────┘
US4 (Phase 6) ◄── 依赖 US2/US3 产物
Phase 7 Polish ◄── 依赖全部 story
```

- **Setup（Phase 1）**: 无依赖，可立即开始
- **Foundational（Phase 2）**: 依赖 Setup；**阻塞所有 story**
- **US2（Phase 3）**: 依赖 Foundational；无其他 story 依赖
- **US3（Phase 4）**: 依赖 Foundational + US2（`RigEmbeddingModel` 复用 `openai.rs` client 构造模式）；可独立测试
- **US1（Phase 5）**: 依赖 US2 + US3 就绪（删除是迁移完成的产物）；其 manifest 清理任务（T022）可先于 T023–T027 的删除动作执行
- **US4（Phase 6）**: 依赖 US2/US3 产物（确定性 e2e 需要 `RigChatModel` 与 agent 循环）
- **Polish（Phase 7）**: 依赖全部 story

### User Story Dependencies（spec 优先级 vs 执行顺序）

| Story | spec 优先级 | 建议执行阶段 | 依赖 | 独立测试 |
|-------|------------|--------------|------|----------|
| US1 移除 dashscope | P1 | Phase 5 | US2 + US3（删除动作） | `cargo build --workspace` + grep 零命中 |
| US2 rig-backed OpenAI 聊天 | P1 | Phase 3 | Foundational | `cargo test -p agent_scope_rig`（openai/streaming/structured） |
| US3 embedding / RAG | P2 | Phase 4 | Foundational + US2 | `cargo test -p agent_scope_rig`（embedding） |
| US4 可观察行为保留 | P2 | Phase 6 | US2/US3 产物 | `rig_e2e` + 兼容矩阵条目 |

> US1 在 spec 中为 P1 且是 MVP 目标，但其"删除"动作语义上依赖替代实现（US2/US3）就绪；因此执行顺序为 US2 → US3 → US1 → US4，US1 的独立测试（workspace 无残留）是其验收。

### Within Each User Story

- 测试任务（T013–T015、T020、T028–T029）先写并确保 FAIL 后再实现；映射类单测（T010–T012）在 Foundational 阶段即先 FAIL
- 映射模块（message/tools/params/error）先于 backend 装配（T016）
- backend（T016）先于 `RigChatModel`（T017）与 stream 装配（T018）
- `RigChatModel` 装配完成后先跑非流式，再跑流式转换
- 示例迁移逐示例验证 `cargo build -p <example>`，最后统一 workspace 验证（T027）

### Parallel Opportunities

- **Setup**: T003 可并行
- **Foundational**: T006、T010、T011、T012 可并行（不同文件/纯函数）
- **US2**: T013、T014 可并行（不同测试文件）；T015 依赖 mock 环境单独进行；T016 完成后 T018、T019 可并行（不同文件）
- **US3**: T020 可并行
- **US1**: T023、T024、T025 可并行（不同示例目录）；T022 删除操作独立
- **US4**: T028、T029 可并行（不同测试文件）

### Parallel Example（示意）

```bash
# Foundational 映射测试并行：
Task: "T010 message_mapping_tests.rs" + "T011 tools_mapping_tests.rs" + "T012 error_mapping_tests.rs"

# US1 示例迁移并行（三个 worker）：
Task: "T023 quickstart/chat/agent" + "T024 human-in-the-loop/plan-react-agent/subagent" + "T025 rag"

# US4 行为测试并行：
Task: "T028 rig_e2e_tests.rs" + "T029 重试语义"
```

---

## Implementation Strategy

### MVP First（建议 = US2 + US1 同批次）

1. 完成 Phase 1 Setup + Phase 2 Foundational
2. Phase 3 US2：`RigChatModel::openai` 独立可回复（非流式 + 流式 + 结构化）——**新 provider 能力就绪**
3. Phase 4 US3：`RigEmbeddingModel::openai` 独立可嵌入
4. Phase 5 US1：7 示例迁移 + 删除 dashscope —— **迁移完成（spec 原始诉求）**
5. **STOP and VALIDATE**: `cargo build --workspace` + 全 workspace 测试 + grep 零命中

### Incremental Delivery

1. Setup + Foundational → 映射层就绪（单测通过）
2. US2 → 新聊天 provider 可独立使用 → 演示（`quickstart` 手工冒烟）
3. US3 → embedding/RAG 可用
4. US1 → 删除 dashscope，示例全量迁移
5. US4 → 确定性 e2e + 兼容矩阵登记
6. Polish → 文档/全仓验收

### Parallel Team Strategy

- 单人顺序：Phase 1→2→3→4→5→6→7
- 双人：A 做 Foundational 映射（T004–T012），B 预写 US2/US3 测试（T013/T014/T015/T020）；完成后 A 装配 backend/chat（T016–T019），B 做 US1 示例迁移（T023–T025）
- 三人：第 3 人并行 US4 测试（T028/T029）

---

## Notes

- [P] tasks = different files, no dependencies
- 每个任务必须对照 [contracts/rig-mapping.md](contracts/rig-mapping.md) 与 [contracts/provider-adapter.md](contracts/provider-adapter.md) 的具体映射契约，实现前先核实 research.md 末节 7 个验证点（rig `CompletionRequest` setter 名称、`CompletionError` 变体、`CompletionsClient` 工具调用完整度、`usage()`/`finish_reason` 时机、Anthropic thinking-tool 并发、DeepSeek reasoning 流向、dashscope 测试契约迁移范围）
- 公开数据协议（`ChatResponse`/`ContentBlock`/`Msg`/`ToolChoice`/`ModelError`/`EmbeddingResponse`）零变更（宪法第十二/十三条）
- `#![deny(unsafe_code)]`；无新 spawn/无新 channel（宪法第九/十条）
- 提交约定：按任务或逻辑组 commit；每阶段 checkpoint 独立验证

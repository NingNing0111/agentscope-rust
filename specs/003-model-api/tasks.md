# Tasks: AgentScope Model API

**Input**: Design documents from `/specs/003-model-api/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Tests are included per the specification's acceptance scenarios and the Constitution's "Test-Driven Compatibility" principle (第六条).

**Organization**: Tasks are grouped by user story to enable independent implementation and testing. US2 (ChatResponse data structures) is implemented first in Phase 2 as a foundational dependency because ChatModel trait (US1) depends on these types.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4, US5, US6)
- Include exact file paths in descriptions

## Path Conventions

- **Workspace root**: `Cargo.toml` (add `agent_scope_model` to members)
- **Crate**: `crates/agent_scope_model/`
- **OpenAI submodule**: `crates/agent_scope_model/src/openai/`
- **Integration tests**: `tests/model/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Crate initialization, workspace registration, dependency configuration

- [X] T001 Create `crates/agent_scope_model/Cargo.toml` with dependencies on `agent_scope_types`, `agent_scope_message`, `agent_scope_utils`, plus `reqwest` (0.12, `stream` feature), `tokio` (1, `full` features), `tokio-stream` (0.1), `futures` (0.3), `serde`/`serde_json`, `serde_yaml` (0.9), `schemars` (0.8), `base64` (0.22), `uuid` (1), `chrono` (0.4)
- [X] T002 [P] Add `"crates/agent_scope_model"` to workspace members in root `Cargo.toml`
- [X] T003 [P] Create `crates/agent_scope_model/src/lib.rs` skeleton with `#![deny(unsafe_code)]` and placeholder module declarations
- [X] T004 [P] Create `tests/model/` directory structure for integration tests

**Checkpoint**: `cargo build -p agent_scope_model` compiles empty crate successfully

---

## Phase 2: User Story 2 — ChatResponse 增量构建 (Priority: P1) 🔗 Foundational

**Goal**: Implement `FinishedReason`, `ChatUsage`, `ChatResponse` (with all append methods), and `StructuredResponse`. These are self-contained data structures that all other modules (US1 ChatModel trait, US3 structured output, US4 ModelCard) depend on.

**Independent Test**: Create empty ChatResponse → call append_text/append_thinking/append_tool_call/append_data_block → verify content blocks correct. Serialize/deserialize JSON round-trip.

**⚠️ CRITICAL**: This phase MUST complete before US1 begins — ChatModel trait returns ChatResponse.

### Implementation for User Story 2

- [X] T005 [P] [US2] Implement `FinishedReason` enum (`Completed`, `Interrupted`) with `#[serde(rename_all = "lowercase")]` in `crates/agent_scope_model/src/response.rs`
- [X] T006 [P] [US2] Implement `ChatUsage` struct (fields: `input_tokens`, `output_tokens`, `time`, `cache_creation_input_tokens` default 0, `cache_input_tokens` default 0, `type` = `"chat"`, `metadata` optional) with `#[serde(rename_all = "snake_case")]` in `crates/agent_scope_model/src/usage.rs`
- [X] T007 [US2] Implement `ChatResponse` struct (fields: `content`, `is_last`, `id`, `created_at`, `response_type` = `"chat_response"`, `usage`, `finished_reason` default Completed, `metadata`) with `Default` derive in `crates/agent_scope_model/src/response.rs`
- [X] T008 [US2] Implement `ChatResponse::append_text(&mut self, text: &str, block_id: Option<&str>) -> &mut Self` — match TextBlock by id or create new in `crates/agent_scope_model/src/response.rs`
- [X] T009 [US2] Implement `ChatResponse::append_thinking(&mut self, thinking: &str, block_id: Option<&str>, extra_fields: HashMap<String, JsonValue>) -> &mut Self` — match ThinkingBlock by id, merge extras via `#[serde(flatten)]` semantics in `crates/agent_scope_model/src/response.rs`
- [X] T010 [US2] Implement `ChatResponse::append_tool_call(&mut self, block_id: &str, name: &str, input: &str, extra_fields: HashMap<String, JsonValue>) -> &mut Self` — match ToolCallBlock by id, append input, merge extras in `crates/agent_scope_model/src/response.rs`
- [X] T011 [US2] Implement `ChatResponse::append_data_block(&mut self, block_id: &str, data: &[u8], media_type: &str, name: Option<&str>) -> &mut Self` — audio/* uses decode→concat→re-encode base64; non-audio replaces in `crates/agent_scope_model/src/response.rs`
- [X] T012 [US2] Implement `ChatResponse::append_chat_response(&mut self, other: &ChatResponse) -> &mut Self` — merge by block_id: TextBlock concat text, ThinkingBlock concat thinking+merge extras, ToolCallBlock concat input+merge extras, DataBlock audio concat bytes/other replace, append unmatched new blocks, update usage in `crates/agent_scope_model/src/response.rs`
- [X] T013 [US2] Implement `ChatResponse::get_text_content(&self, separator: &str) -> String` convenience method in `crates/agent_scope_model/src/response.rs`
- [X] T014 [P] [US2] Implement `StructuredResponse` struct (fields: `content: JsonValue`, `id`, `created_at`, `response_type` = `"structured_response"`, `usage`, `metadata`, `finished_reason`) in `crates/agent_scope_model/src/response.rs`
- [X] T015 [US2] Update `crates/agent_scope_model/src/lib.rs` with `pub mod response; pub mod usage;` and re-export `ChatResponse`, `StructuredResponse`, `ChatUsage`, `FinishedReason`

### Tests for User Story 2

- [X] T016 [P] [US2] Write unit tests for `ChatUsage` JSON serialization round-trip (all fields, cache tokens default to 0, `type` = `"chat"`) in `crates/agent_scope_model/src/usage.rs` (`#[cfg(test)] mod tests`)
- [X] T017 [P] [US2] Write unit tests for `FinishedReason` serialization (Completed → `"completed"`, Interrupted → `"interrupted"`) in `crates/agent_scope_model/src/response.rs`
- [X] T018 [US2] Write unit tests for `ChatResponse::append_text` — same block_id accumulates, different block_id creates new, None creates new in `crates/agent_scope_model/src/response.rs`
- [X] T019 [US2] Write unit tests for `ChatResponse::append_thinking` — accumulate thinking text, extras merge (latest non-None wins) in `crates/agent_scope_model/src/response.rs`
- [X] T020 [US2] Write unit tests for `ChatResponse::append_tool_call` — accumulate JSON input fragments, extras merge in `crates/agent_scope_model/src/response.rs`
- [X] T021 [US2] Write unit tests for `ChatResponse::append_data_block` — audio bytes decode→concat→re-encode, non-audio replace in `crates/agent_scope_model/src/response.rs`
- [X] T022 [US2] Write unit tests for `ChatResponse::append_chat_response` — merge 2 chunks with matching block_ids (text + thinking + tool_call + data), usage update, new blocks appended in `crates/agent_scope_model/src/response.rs`
- [X] T023 [US2] Write unit tests for `ChatResponse` JSON serialization round-trip — verify `type: "chat_response"`, all fields preserved, `is_last` flag in `crates/agent_scope_model/src/response.rs`
- [X] T024 [P] [US2] Write unit tests for `StructuredResponse` JSON serialization round-trip — verify `type: "structured_response"`, content as JSON object in `crates/agent_scope_model/src/response.rs`

**Checkpoint**: `cargo test -p agent_scope_model` passes all US2 tests. ChatResponse can be created, incrementally built, and serialized.

---

## Phase 3: User Story 1 — 模型调用与流式响应 (Priority: P1) 🎯 MVP

**Goal**: Implement `ModelError`, `ToolChoice`, `StreamAccumulator` (O(n) accumulator), `ChatModel` trait with `call()`, `count_tokens()`, retry/cancel logic, and `ModelCallResult` enum.

**Independent Test**: Create a mock ChatModel implementation → call via trait interface → verify retry behavior, streaming accumulation, cancel semantics.

### Implementation for User Story 1

#### ModelError & ToolChoice

- [X] T025 [P] [US1] Implement `ModelError` enum (variants: `ApiError { status, message, provider }`, `RetryExhausted { attempts, last_error, provider }`, `Cancelled`, `ValidationError { field, message }`, `SerializationError { context, source }`, `FormatError { context, source: FormatError }`, `StructuredOutputError { reason }`, `UnsupportedFeature { feature, provider }`, `ConfigError { message }`) with `std::fmt::Display` and `std::error::Error` impls in `crates/agent_scope_model/src/model_error.rs`
- [X] T026 [P] [US1] Implement `ModelErrorKind` enum (variants: `ApiConnection`, `ApiTimeout`, `RateLimit`, `InternalServer`, `BadRequest`, `Authentication`) used for retryable error matching in `crates/agent_scope_model/src/model_error.rs`
- [X] T027 [P] [US1] Implement `ToolChoice` struct (fields: `mode: String`, `tools: Option<Vec<String>>`) with validation: mode must be one of `"auto"`, `"none"`, `"required"` or a valid tool name in `crates/agent_scope_model/src/tool_choice.rs`

#### StreamAccumulator

- [X] T028 [US1] Implement `AccTextBlock` internal struct with `text: Vec<String>` fragment list, `append(block: &TextBlock)`, `build() -> TextBlock` in `crates/agent_scope_model/src/accumulator.rs`
- [X] T029 [US1] Implement `AccThinkingBlock` internal struct with `thinking: Vec<String>` + `extras: HashMap<String, JsonValue>`, `append(block: &ThinkingBlock)`, `build() -> ThinkingBlock` in `crates/agent_scope_model/src/accumulator.rs`
- [X] T030 [US1] Implement `AccToolCallBlock` internal struct with `input: Vec<String>` + `name: String` (from first non-empty), `append(block: &ToolCallBlock)`, `build() -> ToolCallBlock` in `crates/agent_scope_model/src/accumulator.rs`
- [X] T031 [US1] Implement `AccBase64Source` internal struct with `data: Vec<Vec<u8>>`, `append(source: &Base64Source)`, `build() -> Base64Source` (one-time base64 encode of concatenated bytes) in `crates/agent_scope_model/src/accumulator.rs`
- [X] T032 [US1] Implement `AccDataBlock` internal struct with `source: AccDataSource` enum (`Audio(AccBase64Source)` for audio/* streaming, `Other(DataSource)` for non-streamable replace-latest), `append(block: &DataBlock)`, `build() -> DataBlock` in `crates/agent_scope_model/src/accumulator.rs`
- [X] T033 [US1] Implement `StreamAccumulator` struct (fields: `blocks: HashMap<String, AccBlock>`, `id: Option<String>`, `usage: Option<ChatUsage>`, `finished_reason: FinishedReason`) with `append_chat_response(&mut self, delta: &ChatResponse)` — match by block_id, type change → warn+replace, and `build(self) -> ChatResponse` — join all fragments in `crates/agent_scope_model/src/accumulator.rs`

#### ChatModel Trait

- [X] T034 [US1] Implement `ModelCallResult` enum (variants: `Complete(ChatResponse)`, `Stream(Pin<Box<dyn Stream<Item = Result<ChatResponse, ModelError>> + Send>>)`) in `crates/agent_scope_model/src/model_trait.rs`
- [X] T035 [US1] Define `ChatModel` trait with required methods: `model_name()`, `stream_enabled()`, `max_retries()` (default 3), `retry_delay()` (default 1.0), `context_size()` (default 32768), `retryable_errors()` (default empty), `_call_api()` (abstract) in `crates/agent_scope_model/src/model_trait.rs`
- [X] T036 [US1] Implement `ChatModel::call()` default method — retry loop: attempt up to `max_retries + 1` times, check `retryable_errors()` for matching error categories, sleep `retry_delay` between retries, return `RetryExhausted` if all attempts fail. Streaming mode: wrap stream with StreamAccumulator → final `is_last=true` chunk with `finished_reason` in `crates/agent_scope_model/src/model_trait.rs`
- [X] T037 [US1] Implement `ChatModel::count_tokens()` default method — byte/4 heuristic: collect all text from messages (TextBlock.text, ThinkingBlock.thinking, HintBlock.hint, ToolCallBlock.input, ToolResultBlock output text), each DataBlock adds 2000, serialize tools JSON to text, total = `(total_bytes / 4).ceil()` in `crates/agent_scope_model/src/model_trait.rs`
- [X] T038 [US1] Implement `ChatModel::validate_tool_choice()` default method — validate mode in {auto, none, required} or as tool name present in tool list; validate tool_choice.tools names against available tools in `crates/agent_scope_model/src/model_trait.rs`
- [X] T039 [US1] Implement `ChatModel::list_models()` default method — scan YAML directory (default: next to concrete subclass source file), load each `.yaml` as `ModelCard`, warn on failures but continue in `crates/agent_scope_model/src/model_trait.rs`
- [X] T040 [US1] Implement `ChatModel::generate_structured_output()` default method — retry loop, delegate to `_call_api_with_structured_output()` in `crates/agent_scope_model/src/model_trait.rs`
- [X] T041 [US1] Implement default `ChatModel::_call_api_with_structured_output()` — construct `generate_structured_output` tool with schema, inject system-reminder into messages, call `_call_api` with forced tool_choice, parse ToolCallBlock.input as JSON (with repair), validate against schema, return `StructuredResponse` in `crates/agent_scope_model/src/model_trait.rs`

#### Helper Utilities

- [X] T042 [P] [US1] Implement `json_repair()` function in `crates/agent_scope_model/src/json_repair.rs` — fix truncated JSON (missing closing brackets/braces/ quotes), remove trailing commas
- [X] T043 [P] [US1] Implement `flatten_json_schema()` function in `crates/agent_scope_model/src/schema_flat.rs` — resolve `$ref: "#/$defs/TypeName"` inline from `$defs` dict, track visited types to prevent recursion
- [X] T044 [P] [US1] Implement `build_streaming_wav_header() -> Vec<u8>` in `crates/agent_scope_model/src/wav_header.rs` — construct 44-byte WAV header: RIFF header (size=0xFFFFFFFF), fmt chunk (PCM, 1ch, 24000Hz, 16-bit), data chunk header (size=0xFFFFFFFF)

#### Module Exports

- [X] T045 [US1] Update `crates/agent_scope_model/src/lib.rs` with `pub mod model_trait; pub mod model_error; pub mod tool_choice; pub mod accumulator; pub mod json_repair; pub mod schema_flat; pub mod wav_header;` and re-export `ChatModel`, `ModelCallResult`, `ModelError`, `ModelErrorKind`, `ToolChoice`, `StreamAccumulator`

### Tests for User Story 1

- [X] T046 [P] [US1] Write unit tests for `ToolChoice` validation — valid modes, invalid tool name rejected, tools list filtering in `crates/agent_scope_model/src/tool_choice.rs`
- [X] T047 [P] [US1] Write unit tests for `StreamAccumulator` — simulate text streaming (2 deltas → build → verify text joined), thinking streaming with extras merge, tool call JSON fragment accumulation, audio bytes decode→concat→re-encode in `crates/agent_scope_model/src/accumulator.rs`
- [X] T048 [P] [US1] Write unit tests for `StreamAccumulator` edge cases — block type change warning, empty usage chunk absorbed, id propagation from latest delta in `crates/agent_scope_model/src/accumulator.rs`
- [X] T049 [US1] Write unit tests for `ChatModel::count_tokens()` — verify byte/4 heuristic with known text lengths, data block adds 2000 each, tools JSON contributes to count in `crates/agent_scope_model/src/model_trait.rs`
- [X] T050 [US1] Write unit tests for `ChatModel::validate_tool_choice()` — valid: auto/none/required modes, specific tool name. Invalid: unknown tool name, unknown mode when tools list empty in `crates/agent_scope_model/src/model_trait.rs`
- [X] T051 [US1] Write unit tests for `ChatModel::call()` retry logic — mock model that fails 2 times (retryable) then succeeds on 3rd attempt; mock model that fails with non-retryable error (raises immediately); mock model that exceeds max_retries (returns RetryExhausted) in `crates/agent_scope_model/src/model_trait.rs`
- [X] T052 [US1] Write unit tests for `ChatModel::call()` streaming cancellation — mock stream, drop consumer, verify StreamAccumulator sets finished_reason=Interrupted in `crates/agent_scope_model/src/model_trait.rs`
- [X] T053 [US1] Write unit tests for `json_repair()` — fix missing `}`, missing `]`, trailing comma, truncated string in `crates/agent_scope_model/src/json_repair.rs`
- [X] T054 [P] [US1] Write unit tests for `flatten_json_schema()` — simple $ref resolution, nested $ref, circular ref prevention in `crates/agent_scope_model/src/schema_flat.rs`
- [X] T055 [P] [US1] Write unit tests for `build_streaming_wav_header()` — verify 44 bytes, "RIFF" magic, "WAVE" format, "fmt " and "data" chunk IDs in `crates/agent_scope_model/src/wav_header.rs`

**Checkpoint**: `cargo test -p agent_scope_model` passes all US1 + US2 tests. ChatModel trait is functional with mock implementations.

---

## Phase 4: User Story 4 — ModelCard 与 Model 发现 (Priority: P2)

**Goal**: Implement `ModelCard` struct and `from_yaml()` loader — YAML loading, parameter schema merge with overrides, auto-filter thinking/voice params, output_size → max_tokens max.

**Independent Test**: Create test YAML file → load via ModelCard::from_yaml() → verify parameter schema merged correctly, auto-filters applied.

**Note**: US4 is independent of US1/US3 and can run in parallel with Phase 3/5.

### Implementation for User Story 4

- [X] T056 [P] [US4] Implement `ModelStatus` enum (`Active`, `Deprecated`, `Sunset`) with `#[serde(rename_all = "lowercase")]` in `crates/agent_scope_model/src/card.rs`
- [X] T057 [US4] Implement `ModelCard` struct (fields: `card_type` = `"chat_model"`, `name`, `label`, `status: ModelStatus`, `deprecated_at: Option<DateTime<Utc>>`, `input_types` default `["text/plain"]`, `output_types` default `["text/plain"]`, `context_size: i64`, `output_size: i64`, `parameter_schema: JsonValue`, `parameters_overrides: HashMap<String, JsonValue>`) in `crates/agent_scope_model/src/card.rs`
- [X] T058 [US4] Implement `ModelCard::from_yaml(yaml_path: &Path, base_parameter_schema: &JsonValue) -> Result<ModelCard, ModelError>` — load YAML via `serde_yaml`, apply auto-filters (remove thinking_enable/thinking_budget if `application/x-thinking` not in output_types; remove voice if no `audio/*` in output_types), apply parameter_overrides (null→remove, hidden:true→remove, other→merge), set max_tokens.maximum from output_size, build final parameter_schema in `crates/agent_scope_model/src/card.rs`
- [X] T059 [US4] Update `crates/agent_scope_model/src/lib.rs` with `pub mod card;` and re-export `ModelCard`, `ModelStatus`

### Tests for User Story 4

- [X] T060 [P] [US4] Write unit tests for `ModelCard` JSON serialization round-trip — all fields preserved, `type: "chat_model"` in `crates/agent_scope_model/src/card.rs`
- [X] T061 [US4] Write unit tests for `ModelCard::from_yaml()` — basic YAML loading, parameter overrides merge (hidden removal, value override), thinking_enable auto-filter when output_types lacks `application/x-thinking`, voice auto-filter when output_types lacks `audio/*`, max_tokens maximum set from output_size in `crates/agent_scope_model/src/card.rs`
- [X] T062 [US4] Write unit tests for `ModelCard::from_yaml()` edge cases — empty overrides, multiple override keys, null override removal, unknown YAML field ignored in `crates/agent_scope_model/src/card.rs`

**Checkpoint**: `cargo test -p agent_scope_model` passes all US4 tests. ModelCard can be loaded from YAML files.

---

## Phase 5: User Story 5 — Formatter 消息格式化 (Priority: P2)

**Goal**: Implement `Formatter` trait and `FormatError` enum — Msg → API dict conversion, tool result multimodal separation, message grouping.

**Independent Test**: Create Msg list with various content → format via OpenAIChatFormatter → verify output dicts match OpenAI Chat Completions API format.

**Note**: US5 is independent of US1/US3/US4 and can run in parallel with Phases 3-4.

### Implementation for User Story 5

- [X] T063 [P] [US5] Implement `FormatError` enum (variants: `InvalidMessage(String)`, `UnsupportedMediaType { media_type, block_id }`, `Io(std::io::Error)`, `Base64Decode(base64::DecodeError)`) with `Display` and `Error` impls in `crates/agent_scope_model/src/formatter.rs`
- [X] T064 [P] [US5] Implement `MessageGroup` enum (`ToolSequence`, `AgentMessage`) in `crates/agent_scope_model/src/formatter.rs`
- [X] T065 [US5] Define `Formatter` trait with methods: `supported_input_media_types(&self) -> &[String]`, `format(&self, msgs: &[Msg]) -> Result<Vec<JsonValue>, FormatError>`, `convert_tool_result_to_string(&self, output: &ToolOutputType) -> Result<(String, Vec<ContentBlock>), FormatError>`, `group_messages(&self, msgs: &[Msg]) -> Vec<(MessageGroup, Vec<&Msg>)>` in `crates/agent_scope_model/src/formatter.rs`
- [X] T066 [US5] Implement `Formatter::supported_input_media_types()` default — derive from `input_types`, exclude `"text/plain"` and `"application/x-thinking"` in `crates/agent_scope_model/src/formatter.rs`
- [X] T067 [US5] Implement `Formatter::group_messages()` default — iterate Msg list: consecutive tool_call/tool_result messages grouped as ToolSequence, non-tool messages as AgentMessage, preserve original order in `crates/agent_scope_model/src/formatter.rs`
- [X] T068 [US5] Implement `Formatter::convert_tool_result_to_string()` default — for each block in output: TextBlock→append text, DataBlock with supported media_type→generate shortuuid ID, promote block with system-reminder; DataBlock with unsupported URL→reference URL; DataBlock with unsupported base64→save to temp file and reference path in `crates/agent_scope_model/src/formatter.rs`
- [X] T069 [US5] Update `crates/agent_scope_model/src/lib.rs` with `pub mod formatter;` and re-export `Formatter`, `FormatError`, `MessageGroup`

### Tests for User Story 5

- [X] T070 [P] [US5] Write unit tests for `Formatter::group_messages()` — pure text messages → all AgentMessage, tool_sequence messages grouped together, mixed sequence correctly split in `crates/agent_scope_model/src/formatter.rs`
- [X] T071 [P] [US5] Write unit tests for `Formatter::convert_tool_result_to_string()` — text-only output, DataBlock with supported image media type promoted, DataBlock with unsupported URL source references URL, DataBlock with unsupported base64 source saved to temp file in `crates/agent_scope_model/src/formatter.rs`

**Checkpoint**: `cargo test -p agent_scope_model` passes all US5 tests. Formatter trait is functional.

---

## Phase 6: User Story 3 — 结构化输出生成 (Priority: P2)

**Goal**: Integrate `generate_structured_output()` with JSON repair and schema validation. Write dedicated integration tests for structured output flow.

**Independent Test**: Mock model that returns a tool call with JSON → verify StructuredResponse.content matches expected schema.

**Note**: US3 depends on US1 (ChatModel trait), US2 (StructuredResponse). Can run in parallel with US4/US5.

### Implementation for User Story 3

- [X] T072 [US3] Write integration test for `ChatModel::generate_structured_output()` — mock model returns ToolCallBlock with JSON `{"name": "test", "value": 42}`, verify StructuredResponse.content parsed correctly in `crates/agent_scope_model/src/model_trait.rs`
- [X] T073 [US3] Write integration test for `ChatModel::generate_structured_output()` — schema validation failure (missing required field) returns `StructuredOutputError` in `crates/agent_scope_model/src/model_trait.rs`
- [X] T074 [US3] Write integration test for `ChatModel::generate_structured_output()` — JSON repair scenario (missing closing brace → repair succeeds) in `crates/agent_scope_model/src/model_trait.rs`
- [X] T075 [US3] Write integration test for `ChatModel::generate_structured_output()` — empty messages list returns `ValidationError` in `crates/agent_scope_model/src/model_trait.rs`

**Checkpoint**: `cargo test -p agent_scope_model` passes all US3 tests. Structured output generation works.

---

## Phase 7: User Story 1b — OpenAI 参考实现 (Priority: P1) 🎯 MVP

**Goal**: Implement `OpenAIChatModel` as the reference ChatModel provider—construct request body, parse streaming SSE response, parse non-streaming response, implement `OpenAIChatFormatter`, `OpenAIChatParameters`.

**Independent Test**: With a mock HTTP server (or recorded API responses), call OpenAIChatModel → verify ChatResponse chunks match expected content.

### Implementation for OpenAI Reference

#### Parameters & Formatter

- [X] T076 [P] [US1] Implement `ReasoningEffort` enum (`None`, `Minimal`, `Low`, `Medium`, `High`, `Xhigh`) with `#[serde(rename_all = "lowercase")]` in `crates/agent_scope_model/src/openai/parameters.rs`
- [X] T077 [P] [US1] Implement `OpenAIChatParameters` struct (fields: `max_tokens: Option<u32>`, `thinking_enable` default false, `reasoning_effort: Option<ReasoningEffort>`, `temperature: Option<f64>`, `top_p: Option<f64>`, `parallel_tool_calls` default true, `voice: Option<String>`) with `JsonSchema` derive in `crates/agent_scope_model/src/openai/parameters.rs`
- [X] T078 [US1] Implement `OpenAIChatFormatter` struct (field: `input_types: Vec<String>`) implementing `Formatter` trait — format Msg list to OpenAI Chat Completions API dicts (role, content as string or content array for multimodal, tool_calls structure) in `crates/agent_scope_model/src/openai/formatter.rs`

#### Model Implementation

- [X] T079 [US1] Implement `OpenAIChatModel` struct (fields: `api_key`, `organization: Option<String>`, `base_url` default `"https://api.openai.com/v1"`, `model_name`, `parameters`, `stream` default true, `max_retries` default 3, `retry_delay` default 1.0, `context_size` default 128000, `formatter: Box<dyn Formatter>`, `client: reqwest::Client`, `client_kwargs`, `extra_body`) with constructor in `crates/agent_scope_model/src/openai/model.rs`
- [X] T080 [US1] Implement `OpenAIChatModel::build_request_body()` — construct JSON body: model, messages (from formatter), stream, max_completion_tokens, temperature, top_p, reasoning_effort (if thinking_enable), audio config (if voice set), tools/tool_choice, stream_options={include_usage:true}, extra_body merge in `crates/agent_scope_model/src/openai/model.rs`
- [X] T081 [US1] Implement `ChatModel` trait for `OpenAIChatModel` — `retryable_errors()` returns ApiConnection, ApiTimeout, RateLimit, InternalServer; `_call_api()` makes HTTP POST to {base_url}/chat/completions, returns Stream or Complete based on `self.stream` in `crates/agent_scope_model/src/openai/model.rs`
- [X] T082 [US1] Implement `OpenAIChatModel::parse_stream_response()` — consume SSE byte stream, parse `data:` lines, extract delta fields (content→text, reasoning_content→thinking, tool_calls→tool_call, audio.data→data_block with WAV header on first chunk, audio.transcript→text), yield ChatResponse chunks, handle `[DONE]` sentinel, absorb empty-content carrier chunks in `crates/agent_scope_model/src/openai/model.rs`
- [X] T083 [US1] Implement `OpenAIChatModel::parse_completion_response()` — extract from ChatCompletion JSON: choices[0].message.reasoning_content→ThinkingBlock, message.content→TextBlock, message.tool_calls→ToolCallBlock(s), message.audio→DataBlock+transcript TextBlock, usage→ChatUsage, return ChatResponse { is_last: true } in `crates/agent_scope_model/src/openai/model.rs`
- [X] T084 [US1] Implement `OpenAIChatModel::_format_tools()` — validate tool_choice, filter tools by tool_choice.tools list, flatten JSON schemas via `flatten_json_schema()`, format tool_choice for API (literal modes→string, specific tool name→{type:function,function:{name}}) in `crates/agent_scope_model/src/openai/model.rs`
- [X] T085 [US1] Implement `OpenAIChatModel::_call_api_with_structured_output()` override — try base impl, on BadRequestError mentioning "tool_choice" → retry with `tool_choice=ToolChoice { mode: "auto" }` in `crates/agent_scope_model/src/openai/model.rs`
- [X] T086 [US1] Create `crates/agent_scope_model/src/openai/mod.rs` with `pub mod model; pub mod formatter; pub mod parameters;` and re-exports

#### Module Exports

- [X] T087 [US1] Update `crates/agent_scope_model/src/lib.rs` with `pub mod openai;` and re-export `OpenAIChatModel`, `OpenAIChatFormatter`, `OpenAIChatParameters`

### Tests for OpenAI Reference

- [X] T088 [P] [US1] Write unit tests for `OpenAIChatParameters` JSON serialization — verify field names match OpenAI API (snake_case keys in JSON), defaults (parallel_tool_calls=true) in `crates/agent_scope_model/src/openai/parameters.rs`
- [X] T089 [P] [US1] Write unit tests for `OpenAIChatFormatter::format()` — single text message produces role+content string, multimodal message produces content array, tool_calls formatted correctly, tool result output separation in `crates/agent_scope_model/src/openai/formatter.rs`
- [X] T090 [US1] Write unit tests for `OpenAIChatModel::build_request_body()` — verify model name, formatted messages, stream_options when streaming, audio config when voice set, extra_body merged, tools filtered and formatted in `crates/agent_scope_model/src/openai/model.rs`
- [X] T091 [US1] Write unit tests for `OpenAIChatModel::parse_stream_response()` — mock SSE bytes with text delta, thinking delta, tool_call delta, audio delta, [DONE] sentinel; verify correct ChatResponse chunks yielded in `crates/agent_scope_model/src/openai/model.rs`
- [X] T092 [US1] Write unit tests for `OpenAIChatModel::parse_completion_response()` — mock ChatCompletion JSON, verify ThinkingBlock+TextBlock+ToolCallBlock+DataBlock extraction, usage parsing with cache tokens in `crates/agent_scope_model/src/openai/model.rs`
- [X] T093 [US1] Write unit tests for `OpenAIChatModel::_format_tools()` — tool_choice validation, tools list filtering, literal mode formatting, specific tool name formatting, schema flatten in `crates/agent_scope_model/src/openai/model.rs`

**Checkpoint**: `cargo test -p agent_scope_model` passes all OpenAI tests. OpenAIChatModel is functional with mock HTTP responses.

---

## Phase 8: User Story 6 — 依赖拓扑与跨层约束 (Priority: P3)

**Goal**: Verify dependency topology — agent_scope_model only depends on Foundation crates, not on tool/agent/memory.

**Independent Test**: `cargo tree -p agent_scope_model` shows only Foundation-level dependencies.

### Implementation for User Story 6

- [X] T094 [P] [US6] Verify `crates/agent_scope_model/Cargo.toml` has zero agentscope internal deps beyond `agent_scope_types`, `agent_scope_message`, `agent_scope_utils` — check with `cargo tree -p agent_scope_model --no-deps`
- [X] T095 [P] [US6] Verify `ToolChoice` is defined in `crates/agent_scope_model/src/tool_choice.rs` — no import from any tool crate
- [X] T096 [P] [US6] Verify `#![deny(unsafe_code)]` in `lib.rs` and compilation succeeds

**Checkpoint**: Dependency topology verified. No circular deps. No upper-layer deps.

---

## Phase 9: Integration Tests & Cross-Cutting Concerns

**Purpose**: Cross-crate integration tests, compatibility diff tests, code quality.

### Integration Tests

- [X] T097 [P] Write integration test for ChatResponse → Msg conversion — ChatResponse.content blocks match agent_scope_message ContentBlock types in `tests/model/chat_response_integration.rs`
- [X] T098 [P] Write integration test for Formatter → Msg — format Msg objects through OpenAIChatFormatter, verify output structure matches OpenAI API spec in `tests/model/formatter_integration.rs`
- [X] T099 [P] Write integration test for StreamAccumulator → ChatResponse — full streaming simulation, verify final build matches Python reference behavior in `tests/model/accumulator_integration.rs`
- [X] T100 [P] Write integration test for ModelCard with actual test YAML files — create 2 test YAML files, load via list_models(), verify cards in `tests/model/model_card_integration.rs`
- [X] T101 Write integration test for cross-crate consistency — ChatResponse serialized from model crate correctly uses agent_scope_message ContentBlock types in `tests/model/cross_crate_tests.rs`

### Compatibility Tests (Golden Snapshot Diff)

- [X] T102 [P] Create Python golden snapshot generation script `tests/compatibility/generate_model_fixtures.py` — exports sample ChatResponse, ChatUsage, StructuredResponse, ModelCard JSON from Python reference implementation
- [X] T103 Create Rust golden snapshot diff test in `tests/compatibility/model_diff_tests.rs` — reads fixture JSON, compares Rust serialization output (timestamp/UUID normalization), reports mismatches
- [X] T104 Populate `tests/compatibility/fixtures/model/` with golden snapshot JSON files

### Code Quality

- [X] T105 [P] Run `cargo clippy -p agent_scope_model -- -D warnings` and fix all warnings
- [X] T106 [P] Run `cargo fmt -p agent_scope_model -- --check` and format all code
- [X] T107 Verify all tests in `specs/003-model-api/quickstart.md` validation scenarios compile and pass

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **US2/ChatResponse (Phase 2)**: Depends on Setup (Phase 1) — BLOCKS US1, US3
- **US1/ChatModel Trait (Phase 3)**: Depends on US2 (needs ChatResponse, ChatUsage) — BLOCKS US3, OpenAI impl
- **US4/ModelCard (Phase 4)**: Depends on Setup only — can parallel with Phase 3, 5
- **US5/Formatter (Phase 5)**: Depends on Setup only — can parallel with Phase 3, 4
- **US3/StructuredOutput (Phase 6)**: Depends on US1 (needs ChatModel trait)
- **OpenAI impl (Phase 7)**: Depends on US1 (ChatModel trait) + US5 (Formatter trait)
- **US6/Topology (Phase 8)**: Verification-only, after all crates exist
- **Polish (Phase 9)**: Depends on all previous phases

### Module Dependency Graph

```
agent_scope_types / agent_scope_message / agent_scope_utils
                       ↑
                agent_scope_model
         ┌─────────┬──────┼──────────┬─────────┐
    response.rs  usage.rs model_trait.rs  card.rs  formatter.rs
    (US2)        (US2)    (US1)          (US4)    (US5)
         │          │         │
         └──────────┴─────┬───┘
                          │
                    openai/model.rs
                    (US1b, Phase 7)
```

### User Story Dependencies

- **US2 (P1)**: Phase 2 — Foundational data structures, MUST complete first
- **US1 (P1)**: Phase 3 — Depends on US2; no dependency on US3/US4/US5
- **US4 (P2)**: Phase 4 — Depends on Setup only; independent of US1/US3/US5
- **US5 (P2)**: Phase 5 — Depends on Setup only; independent of US1/US3/US4
- **US3 (P2)**: Phase 6 — Depends on US1 (ChatModel trait)
- **US1b (P1)**: Phase 7 — Depends on US1 (ChatModel trait) + US5 (Formatter)
- **US6 (P3)**: Phase 8 — Verification only, after all modules exist

### Within Each User Story

- Data types (structs/enums) before methods
- Methods before trait implementations
- Implementation before module exports
- Module exports before tests
- All implementation tasks before story checkpoint

### Parallel Opportunities

- **Phase 1**: T002, T003, T004 all [P] — run in parallel
- **Phase 2**: T005, T006, T014 all [P] — run in parallel; T016, T017, T024 all [P] — run in parallel
- **Phase 3**: T025, T026, T027 all [P]; T042, T043, T044 all [P]; T046-T048, T054, T055 all [P]
- **Phase 4 + 5**: Run in parallel (different crate modules)
- **Phase 7**: T076, T077 all [P]; T088, T089 all [P]
- **Phase 9**: T097-T100, T102, T105, T106 all [P] — run in parallel

---

## Parallel Example: Phase 2 (US2) Implementation

```bash
# Launch all independent structs together:
Task: "Implement FinishedReason enum in crates/agent_scope_model/src/response.rs"
Task: "Implement ChatUsage struct in crates/agent_scope_model/src/usage.rs"
Task: "Implement StructuredResponse struct in crates/agent_scope_model/src/response.rs"

# After structs exist, implement ChatResponse + append methods:
Task: "Implement ChatResponse struct + append_text/append_thinking/..."
```

---

## Implementation Strategy

### MVP First (US2 + US1 Trait + Mock Model)

1. Complete Phase 1: Setup — `cargo build` passes
2. Complete Phase 2: US2 (ChatResponse + ChatUsage) — data structures
3. Complete Phase 3: US1 (ChatModel trait + StreamAccumulator) — trait with mock
4. **STOP and VALIDATE**: Mock model → call → retry → streaming → accumulate
5. MVP delivers: ChatModel trait is defined and usable with mock implementations

### Incremental Delivery

1. Setup (Phase 1) → Crate compiles ✅
2. US2/ChatResponse (Phase 2) → Data structures can be created and serialized ✅
3. US1/ChatModel Trait (Phase 3) → **MVP**: Trait functional with mock ✅
4. US4/ModelCard (Phase 4) → Model discovery from YAML ✅
5. US5/Formatter (Phase 5) → Msg → API format conversion ✅
6. US3/StructuredOutput (Phase 6) → Structured output via tool-calling ✅
7. OpenAI impl (Phase 7) → Working OpenAI provider ✅
8. US6/Topology (Phase 8) → Dependency verification ✅
9. Polish (Phase 9) → Integration tests, golden snapshots, clippy/fmt ✅

### Parallel Team Strategy

With multiple developers:
1. Team completes Phase 1 (Setup) + Phase 2 (US2) together
2. After Phase 2:
   - Developer A: US1 (ChatModel trait — Phase 3)
   - Developer B: US4 (ModelCard — Phase 4) — can start immediately
   - Developer C: US5 (Formatter — Phase 5) — can start immediately
3. After Phase 3 + Phase 5:
   - Developer A: OpenAI impl (Phase 7) — needs ChatModel trait + Formatter
   - Developer B: US3 (StructuredOutput — Phase 6) — needs ChatModel trait
   - Developer C: Golden snapshot fixtures (Phase 9 T102-T104)
4. All: Phase 9 (Integration tests, clippy, fmt)

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story phase should be independently completable and testable
- ContentBlock types (TextBlock, ThinkingBlock, etc.) are imported from `agent_scope_message` crate — no redefinition
- ChatResponse.content uses `Vec<ContentBlock>` (tagged enum from message crate), NOT individual block types
- StreamAccumulator is internal API — not exposed in `pub mod` beyond crate visibility tests
- ThinkingBlock extra fields merge via `#[serde(flatten)] extras: HashMap<String, JsonValue>` — last non-None value wins
- Base64Source data field is base64-encoded String (not raw bytes) — matching Python's serialization format
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently

# Tasks: AgentScope Foundation Layer

**Input**: Design documents from `/specs/002-foundation-layer/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Tests are included per the specification's acceptance scenarios and the Constitution's "Test-Driven Compatibility" principle (第六条). All tasks generate their own unit/integration tests.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing. Note: US4 (Types) is implemented first in Phase 2 as a blocking foundational dependency, even though it is P2 in the spec, because all other modules depend on it.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4, US5)
- Include exact file paths in descriptions

## Path Conventions

- **Workspace root**: `Cargo.toml` (workspace with `members = ["crates/*"]`)
- **Crates**: `crates/agent_scope_types/`, `crates/agent_scope_message/`, `crates/agent_scope_event/`, `crates/agent_scope_state/`, `crates/agent_scope_utils/`
- **Integration tests**: `tests/types/`, `tests/message/`, `tests/event/`, `tests/state/`, `tests/compatibility/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project workspace initialization, utility crate, and all crate skeletons

- [X] T001 Create workspace root `Cargo.toml` with `[workspace]` section, members = `["crates/*"]`, and workspace-level dependencies (serde, serde_json, uuid, chrono, schemars)
- [X] T002 [P] Create `crates/agent_scope_utils/Cargo.toml` with dependencies on uuid and chrono, and create `crates/agent_scope_utils/src/lib.rs` with `pub mod id;`
- [X] T003 [P] Implement `generate_id()` (UUID v4 hex string) and `generate_timestamp()` (ISO 8601) in `crates/agent_scope_utils/src/id.rs`
- [X] T004 [P] Create `crates/agent_scope_types/Cargo.toml` with no agentscope internal deps, only serde/serde_json/uuid/chrono, and create `crates/agent_scope_types/src/lib.rs` skeleton
- [X] T005 [P] Create `crates/agent_scope_message/Cargo.toml` with dependency on `agent_scope_types` (path = `../agent_scope_types`), and create `crates/agent_scope_message/src/lib.rs` skeleton
- [X] T006 [P] Create `crates/agent_scope_event/Cargo.toml` with dependencies on `agent_scope_message` and `agent_scope_types`, and create `crates/agent_scope_event/src/lib.rs` skeleton
- [X] T007 [P] Create `crates/agent_scope_state/Cargo.toml` with dependencies on `agent_scope_message` and `agent_scope_types`, and create `crates/agent_scope_state/src/lib.rs` skeleton

**Checkpoint**: `cargo build` from workspace root compiles all empty crates successfully

---

## Phase 2: User Story 4 — 核心类型定义与错误模型 (Priority: P2) 🔗 Foundational

**Goal**: Implement the `agent_scope_types` crate — the zero-dependency types module that all other Foundation crates depend on. Includes ReplyFinishedReason, ErrorType, ErrorInfo, Embedding, JsonValue, and Hook type constants.

**Independent Test**: Verify each enum serializes to correct JSON snake_case strings; verify ErrorInfo JSON structure matches Python reference; verify hook constant strings match Python Literal values.

**⚠️ CRITICAL**: This phase MUST complete before US1, US2, US3 can begin — all other crates depend on `agent_scope_types`.

### Implementation for User Story 4

- [X] T008 [P] [US4] Implement `ReplyFinishedReason` enum (4 variants: Completed, Interrupted, ExceedMaxIters, Error) with `#[serde(rename_all = "snake_case")]` in `crates/agent_scope_types/src/reply.rs`
- [X] T009 [P] [US4] Implement `ErrorType` enum (8 variants: Authentication, Permission, RateLimit, InvalidRequest, Upstream, Connection, Internal, Unknown) with `#[serde(rename_all = "snake_case")]` in `crates/agent_scope_types/src/error.rs`
- [X] T010 [US4] Implement `ErrorInfo` struct with `error_type: ErrorType` (default Unknown) and `message: String` in `crates/agent_scope_types/src/error.rs`
- [X] T01- [x] T011 [P] [US4] Define `Embedding` type alias (`pub type Embedding = Vec<f64>`) in `crates/agent_scope_types/src/lib.rs`
- [X] T01- [x] T012 [P] [US4] Define `JsonValue` type alias (`pub type JsonValue = serde_json::Value`) and implement `JSONSerializableObject` as serde_json::Value in `crates/agent_scope_types/src/json.rs`
- [X] T01- [x] T013 [P] [US4] Define agent hook constants (`PRE_REPLY`, `POST_REPLY`, `PRE_PRINT`, `POST_PRINT`, `PRE_OBSERVE`, `POST_OBSERVE`) in `crates/agent_scope_types/src/hook.rs`
- [X] T01- [x] T014 [US4] Define ReAct agent hook constants (agent hooks + `PRE_REASONING`, `POST_REASONING`, `PRE_ACTING`, `POST_ACTING`) in `crates/agent_scope_types/src/hook.rs`
- [X] T01- [x] T015 [US4] Update `crates/agent_scope_types/src/lib.rs` with public module exports: `pub mod reply; pub mod error; pub mod json; pub mod hook;` and re-export key types

### Tests for User Story 4

- [X] T01- [x] T016 [P] [US4] Write unit tests for `ReplyFinishedReason` JSON serialization (verify each variant → snake_case string, all 4 variants) in `crates/agent_scope_types/src/reply.rs` (`#[cfg(test)] mod tests`)
- [X] T01- [x] T017 [P] [US4] Write unit tests for `ErrorType` serialization (verify each variant → snake_case string) and `ErrorInfo` JSON round-trip in `crates/agent_scope_types/src/error.rs`
- [X] T01- [x] T018 [P] [US4] Write unit tests for hook constants (verify all 6 agent hook string values, all 10 ReAct hook values) in `crates/agent_scope_types/src/hook.rs`

**Checkpoint**: `cargo test -p agent_scope_types` passes all tests. Types module is complete and ready for downstream consumption.

---

## Phase 3: User Story 1 — 构建和传递消息 (Priority: P1) 🎯 MVP

**Goal**: Implement the `agent_scope_message` crate — Msg, ContentBlock (6 types), factory functions, role validation, and content manipulation methods. This is the core data carrier for all agent communication.

**Independent Test**: Create messages of each role, add various ContentBlock types, verify JSON serialization round-trip, verify role-based validation blocks illegal content types.

### Implementation for User Story 1

#### ContentBlock Sub-Types

- [X] T01- [x] T019 [P] [US1] Implement `TextBlock` struct (fields: `text`, `id`, `created_at`, `finished_at`) with `new()` constructor using auto-generated id/timestamp in `crates/agent_scope_message/src/block.rs`
- [X] T020 [P] [US1] Implement `ThinkingBlock` struct (fields: `thinking`, `id`, `created_at`, `finished_at`) with `#[serde(flatten)] extras: HashMap<String, JsonValue>` for provider-specific field passthrough in `crates/agent_scope_message/src/block.rs`
- [X] T021 [P] [US1] Implement `HintContent` enum (untagged: `Text(String)`, `Blocks(Vec<HintBlockItem>)`) and `HintBlock` struct (fields: `hint`, `source`, `id`, `created_at`, `finished_at`) in `crates/agent_scope_message/src/block.rs`
- [X] T022 [P] [US1] Implement `Base64Source` struct (tag `"base64"`, fields: `data`, `media_type`) in `crates/agent_scope_message/src/source.rs`
- [X] T023 [P] [US1] Implement `URLSource` struct (tag `"url"`, fields: `url`, `media_type`) in `crates/agent_scope_message/src/source.rs`
- [X] T024 [US1] Implement `DataSource` tagged enum (`Base64(Base64Source)`, `Url(URLSource)`) and `DataBlock` struct (fields: `source`, `name`, `id`, `created_at`, `finished_at`) in `crates/agent_scope_message/src/block.rs`
- [X] T025 [P] [US1] Implement `ToolCallState` enum (5 variants: Pending, Asking, Allowed, Submitted, Finished) with `#[serde(rename_all = "lowercase")]` in `crates/agent_scope_message/src/state.rs`
- [X] T026 [US1] Implement `ToolCallBlock` struct (fields: `id`, `name`, `input`, `state`, `suggested_rules`, `created_at`, `finished_at`) with tag `"tool_call"` in `crates/agent_scope_message/src/block.rs`
- [X] T027 [P] [US1] Implement `ToolResultState` enum (5 variants: Success, Error, Interrupted, Denied, Running) with `#[serde(rename_all = "lowercase")]` in `crates/agent_scope_message/src/state.rs`
- [X] T028 [US1] Implement `ToolOutput` enum (untagged: `Text(String)`, `Blocks(Vec<ToolResultBlockItem>)`) and `ToolResultBlock` struct (fields: `id`, `name`, `output`, `state`, `metadata`, `created_at`, `finished_at`) with tag `"tool_result"` in `crates/agent_scope_message/src/block.rs`

#### ContentBlock Enum & BlockType

- [X] T029 [US1] Implement `ContentBlock` tagged enum (`#[serde(tag = "type")]`, 6 variants: Text, Thinking, Hint, Data, ToolCall, ToolResult) with `#[serde(other)]` catch-all variant for forward compatibility in `crates/agent_scope_message/src/block.rs`
- [X] T030 [P] [US1] Implement `BlockType` enum (Text, Thinking, Hint, Data, ToolCall, ToolResult) with `block_type()` method on `ContentBlock` in `crates/agent_scope_message/src/block.rs`

#### Usage, Role, and Msg

- [X] T031 [P] [US1] Implement `Usage` struct (fields: `input_tokens: i64`, `output_tokens: i64`) in `crates/agent_scope_message/src/msg.rs`
- [X] T032 [P] [US1] Implement `Role` enum (User, Assistant, System) with `#[serde(rename_all = "lowercase")]` and implement `ValidationError` enum in `crates/agent_scope_message/src/msg.rs`
- [X] T033 [US1] Implement `Msg` struct with all 11 fields (name, content, role, id, metadata, created_at, usage, finished_at, finished_reason, structured_output, error) and `Msg::new()` constructor with role-content validation returning `Result<Self, ValidationError>` in `crates/agent_scope_message/src/msg.rs`
- [X] T034 [US1] Implement `Msg::get_content_blocks(block_type: Option<BlockType>) -> Vec<&ContentBlock>` method in `crates/agent_scope_message/src/msg.rs`
- [X] T035 [US1] Implement `Msg::get_text_content(separator: &str) -> Option<String>` method in `crates/agent_scope_message/src/msg.rs`
- [X] T036 [US1] Implement `Msg::has_content_blocks(block_type: Option<BlockType>) -> bool` method in `crates/agent_scope_message/src/msg.rs`

#### Factory Functions

- [X] T037 [US1] Implement `UserMsg(name, content) -> Result<Msg, ValidationError>` factory — creates Msg with Role::User, validates only text/data blocks allowed, sets finished_at = created_at by default in `crates/agent_scope_message/src/factory.rs`
- [X] T038 [US1] Implement `AssistantMsg(name, content) -> Msg` factory — creates Msg with Role::Assistant, allows all block types, finished_at defaults to None in `crates/agent_scope_message/src/factory.rs`
- [X] T039 [US1] Implement `SystemMsg(name, content) -> Result<Msg, ValidationError>` factory — creates Msg with Role::System, validates only text blocks allowed, sets finished_at = created_at by default in `crates/agent_scope_message/src/factory.rs`

#### PermissionRule Placeholder

- [X] T040 [P] [US1] Implement `PermissionRule` placeholder struct with `#[serde(flatten)] extras: HashMap<String, JsonValue>` in `crates/agent_scope_message/src/block.rs`

#### Module Exports

- [X] T041 [US1] Update `crates/agent_scope_message/src/lib.rs` with public module exports: `pub mod block; pub mod msg; pub mod state; pub mod source; pub mod factory;` and re-export key types (Msg, ContentBlock, all block structs, Role, factory fns)

### Tests for User Story 1

- [X] T042 [P] [US1] Write unit tests for `TextBlock`, `ThinkingBlock`, `HintBlock` creation and JSON round-trip serialization in `crates/agent_scope_message/src/block.rs`
- [X] T043 [P] [US1] Write unit tests for `DataBlock`, `Base64Source`, `URLSource` creation and JSON round-trip serialization in `crates/agent_scope_message/src/block.rs`
- [X] T044 [P] [US1] Write unit tests for `ToolCallBlock` (with all 5 ToolCallState values) and `ToolResultBlock` (with all 5 ToolResultState values) JSON round-trip serialization in `crates/agent_scope_message/src/block.rs`
- [X] T045 [US1] Write unit tests for `ContentBlock` tagged enum serialization (verify `"type"` tag present, all 6 variants) in `crates/agent_scope_message/src/block.rs`
- [X] T046 [US1] Write unit tests for `Msg::new()` role validation — user role rejects tool_call/hint/thinking blocks, system role rejects data blocks, assistant role accepts all — in `crates/agent_scope_message/src/msg.rs`
- [X] T047 [US1] Write unit tests for `Msg::get_content_blocks()` filtering (single type, multiple types in list, None returns all) in `crates/agent_scope_message/src/msg.rs`
- [X] T048 [US1] Write unit tests for `Msg::get_text_content()` with different separators and `Msg::has_content_blocks()` in `crates/agent_scope_message/src/msg.rs`
- [X] T049 [US1] Write unit tests for factory functions — `UserMsg` (valid + invalid content rejected), `AssistantMsg` (all content accepted), `SystemMsg` (valid + invalid content rejected) in `crates/agent_scope_message/src/factory.rs`

**Checkpoint**: `cargo test -p agent_scope_message` passes all 8 test modules. Messages can be created, validated, filtered, and serialized.

---

## Phase 4: User Story 2 — 流式事件驱动消息构建 (Priority: P1)

**Goal**: Implement the `agent_scope_event` crate — 27 event types, EventBase, AgentEvent tagged union, and integrate `Msg::append_event()` for incremental message construction from streaming events.

**Independent Test**: Simulate streaming event sequences → apply via `append_event()` → verify final Msg state. Verify all 27 EventType enum values and tagged serialization.

### Implementation for User Story 2

#### EventType Enum & EventBase

- [X] T050 [P] [US2] Implement `EventType` enum (28 variants: REPLY_START to CUSTOM) with `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` in `crates/agent_scope_event/src/event_type.rs`
- [X] T051 [P] [US2] Implement `EventBase` struct (fields: `id`, `created_at`, `metadata`) with auto-generating `new()` constructor in `crates/agent_scope_event/src/base.rs`

#### Reply Events

- [X] T052 [US2] Implement `ReplyStartEvent` (fields: `session_id`, `reply_id`, `name`, `role`) and `ReplyEndEvent` (fields: `session_id`, `reply_id`, `finished_reason`, `error`) in `crates/agent_scope_event/src/reply_events.rs`

#### Model Call Events

- [X] T053 [US2] Implement `ModelCallStartEvent` (fields: `reply_id`, `model_name`) and `ModelCallEndEvent` (fields: `reply_id`, `input_tokens`, `output_tokens`, `finished_reason`) in `crates/agent_scope_event/src/model_events.rs`

#### Content Block Streaming Events

- [X] T054 [P] [US2] Implement `TextBlockStartEvent`, `TextBlockDeltaEvent` (field: `delta`), `TextBlockEndEvent` in `crates/agent_scope_event/src/block_events.rs`
- [X] T055 [P] [US2] Implement `DataBlockStartEvent` (field: `media_type`), `DataBlockDeltaEvent` (fields: `data`, `media_type`), `DataBlockEndEvent` in `crates/agent_scope_event/src/block_events.rs`
- [X] T056 [P] [US2] Implement `ThinkingBlockStartEvent`, `ThinkingBlockDeltaEvent` (field: `delta`), `ThinkingBlockEndEvent` in `crates/agent_scope_event/src/block_events.rs`
- [X] T057 [P] [US2] Implement `HintBlockEvent` (fields: `reply_id`, `block_id`, `source`, `hint`) — one-shot non-streaming event in `crates/agent_scope_event/src/block_events.rs`

#### Tool Call & Tool Result Events

- [X] T058 [US2] Implement `ToolCallStartEvent` (fields: `reply_id`, `tool_call_id`, `tool_call_name`), `ToolCallDeltaEvent` (field: `delta`), `ToolCallEndEvent` in `crates/agent_scope_event/src/tool_events.rs`
- [X] T059 [US2] Implement `ToolResultStartEvent` (fields: `reply_id`, `tool_call_id`, `tool_call_name`), `ToolResultTextDeltaEvent` (field: `delta`), `ToolResultDataDeltaEvent` (fields: `block_id`, `media_type`, `data`, `url`), `ToolResultEndEvent` (fields: `state`, `metadata`) in `crates/agent_scope_event/src/tool_events.rs`

#### Control & Interaction Events

- [X] T060 [US2] Implement `ExceedMaxItersEvent` (fields: `reply_id`, `name`) and `UserInterruptEvent` (field: `reply_id`) in `crates/agent_scope_event/src/control_events.rs`
- [X] T061 [US2] Implement `RequireUserConfirmEvent` (field: `tool_calls: Vec<ToolCallBlock>`), `ConfirmResult` struct (fields: `confirmed`, `tool_call`, `rules`), and `UserConfirmResultEvent` (field: `confirm_results: Vec<ConfirmResult>`) in `crates/agent_scope_event/src/control_events.rs`
- [X] T062 [US2] Implement `RequireExternalExecutionEvent` (field: `tool_calls: Vec<ToolCallBlock>`) and `ExternalExecutionResultEvent` (field: `execution_results: Vec<ToolResultBlock>`) in `crates/agent_scope_event/src/control_events.rs`
- [X] T063 [US2] Implement `CustomEvent` (fields: `name`, `value: HashMap<String, JsonValue>`) in `crates/agent_scope_event/src/custom.rs`

#### AgentEvent Tagged Union

- [X] T064 [US2] Implement `AgentEvent` tagged enum (`#[serde(tag = "type")]`, 27 variants using EventType string values as tags via `#[serde(rename = "...")]`) in `crates/agent_scope_event/src/lib.rs`

#### AppendEvent Integration (in Message crate)

- [X] T065 [US2] Implement `AppendEventError` enum (variants: `ReplyIdMismatch`, `BlockNotFound`, `UnknownEventType`) in `crates/agent_scope_message/src/msg.rs`
- [X] T066 [US2] Implement `Msg::append_event(&mut self, event: &AgentEvent) -> Result<(), AppendEventError>` method — handle all 27 event types: text/thinking/data block streaming (start→delta→end), tool call lifecycle, tool result lifecycle, reply start/end, model call start/end, hint block, user confirm/interrupt, external execution, custom, exceed_max_iters in `crates/agent_scope_message/src/msg.rs`

#### Module Exports

- [X] T067 [US2] Update `crates/agent_scope_event/src/lib.rs` with public module exports: `pub mod base; pub mod event_type; pub mod reply_events; pub mod model_events; pub mod block_events; pub mod tool_events; pub mod control_events; pub mod custom;` and re-export AgentEvent + all event structs

### Tests for User Story 2

- [X] T068 [P] [US2] Write unit tests for `EventType` enum serialization (verify each variant → SCREAMING_SNAKE_CASE string, all 28 variants) in `crates/agent_scope_event/src/event_type.rs`
- [X] T069 [P] [US2] Write unit tests for all event struct JSON serialization round-trip — verify `"type"` tag and all fields per struct — in `crates/agent_scope_event/src/` (per-module `#[cfg(test)]` blocks)
- [X] T070 [P] [US2] Write unit tests for `AgentEvent` tagged union serialization — verify correct tag injection and deserialization for all 27 variants in `crates/agent_scope_event/src/lib.rs`
- [X] T071 [US2] Write unit tests for `Msg::append_event()` — simulate full text streaming sequence (REPLY_START → TEXT_BLOCK_START → 2× DELTA → TEXT_BLOCK_END → MODEL_CALL_END → REPLY_END) and verify final TextBlock content in `crates/agent_scope_message/src/msg.rs`
- [X] T072 [US2] Write unit tests for `Msg::append_event()` tool call lifecycle — simulate TOOL_CALL_START → DELTA → END → TOOL_RESULT_START → TEXT_DELTA → END, verify ToolCallBlock state transitions (PENDING → FINISHED) in `crates/agent_scope_message/src/msg.rs`
- [X] T073 [US2] Write unit tests for `Msg::append_event()` data block streaming with base64 decode-concat-re-encode logic in `crates/agent_scope_message/src/msg.rs`
- [X] T074 [US2] Write unit tests for `Msg::append_event()` edge cases — reply_id mismatch (should skip with warning), USER_INTERRUPT (sets finished_reason=INTERRUPTED), unknown event type (graceful skip), multiple MODEL_CALL_END events (token accumulation) in `crates/agent_scope_message/src/msg.rs`

**Checkpoint**: `cargo test -p agent_scope_event -p agent_scope_message` passes all tests. Streaming event sequences correctly build Msg content.

---

## Phase 5: User Story 3 — 智能体状态管理与持久化 (Priority: P2)

**Goal**: Implement the `agent_scope_state` crate — AgentState, ReplyContext, ToolContext, TaskContext, Task, and legacy format migration. Enables session state persistence and cross-session restoration.

**Independent Test**: Create AgentState, populate context with Msg objects, serialize/deserialize round-trip, verify all nested structures intact, verify legacy format auto-migration.

### Implementation for User Story 3

#### ReplyContext

- [X] T075 [P] [US3] Implement `ReplyContext` struct (fields: `reply_id`, `cur_iter`, `structured_schema`, `structured_output`) with defaults in `crates/agent_scope_state/src/agent_state.rs`

#### ReadCacheEntry & ToolContext

- [X] T076 [P] [US3] Implement `ReadCacheEntry` struct (fields: `lines`, `updated_at`, `bytes`, `file_path`) in `crates/agent_scope_state/src/tool_context.rs`
- [X] T077 [US3] Implement `ToolContext` struct (fields: `max_cache_files`, `max_cache_bytes`, `read_file_cache`, `activated_groups`) with `get_cache()`, `cache_file()` (LRU eviction), `clean_file_cache()` methods in `crates/agent_scope_state/src/tool_context.rs`

#### Task & TaskContext

- [X] T078 [P] [US3] Implement `TaskState` enum (Pending, InProgress, Completed) with `#[serde(rename_all = "snake_case")]` and `Task` struct (fields: `subject`, `description`, `metadata`, `created_at`, `state`, `id`, `owner`, `blocks`, `blocked_by`) in `crates/agent_scope_state/src/task.rs`
- [X] T079 [US3] Implement `TaskContext` struct (field: `tasks: Vec<Task>`) with methods: `add_task()`, `get_task()`, `update_task_state()`, `tasks_by_state()`, `tasks_by_owner()` in `crates/agent_scope_state/src/task.rs`

#### Permission Placeholders

- [X] T080 [P] [US3] Define `PermissionContext` type alias (`pub type PermissionContext = HashMap<String, serde_json::Value>`) in `crates/agent_scope_state/src/permission.rs`
- [X] T081 [P] [US3] Redefine `PermissionRule` placeholder (or re-export from message crate) in `crates/agent_scope_state/src/permission.rs`

#### AgentState

- [X] T082 [US3] Implement `AgentState` struct (fields: `session_id`, `summary`, `context`, `max_context_messages`, `reply_context`, `permission_context`, `tool_context`, `tasks_context`, `middle_context`) with `new()` and `with_session_id()` constructors in `crates/agent_scope_state/src/agent_state.rs`
- [X] T083 [US3] Implement `AgentState::append_context(name, blocks) -> Result<(), AppendContextError>` — appends blocks to tail assistant message if name+reply_id match, otherwise creates new Msg; rejects if `max_context_messages` reached in `crates/agent_scope_state/src/agent_state.rs`
- [X] T084 [US3] Implement `AgentState::has_awaiting_tool_calls(name) -> bool` and `get_awaiting_tool_calls(name) -> Vec<&ToolCallBlock>` — checks tail assistant message for ASKING state ToolCall or SUBMITTED without tool result in `crates/agent_scope_state/src/agent_state.rs`
- [X] T085 [US3] Implement `AgentState::set_max_context_messages()` and `AgentState::context_length()` methods in `crates/agent_scope_state/src/agent_state.rs`
- [X] T086 [US3] Implement `AgentState::from_legacy_json()` — custom deserializer that migrates top-level `reply_id`/`cur_iter` into `reply_context` nested structure in `crates/agent_scope_state/src/agent_state.rs`

#### SummaryContent Type

- [X] T087 [US3] Implement `SummaryContent` enum (untagged: `Text(String)`, `Blocks(Vec<ContentBlock>)`) used by `AgentState.summary` field in `crates/agent_scope_state/src/agent_state.rs`

#### Module Exports

- [X] T088 [US3] Update `crates/agent_scope_state/src/lib.rs` with public module exports: `pub mod agent_state; pub mod tool_context; pub mod task; pub mod permission;` and re-export AgentState, ReplyContext, ToolContext, Task, TaskContext, TaskState

### Tests for User Story 3

- [X] T089 [P] [US3] Write unit tests for `ReplyContext` creation and JSON round-trip serialization in `crates/agent_scope_state/src/agent_state.rs`
- [X] T090 [P] [US3] Write unit tests for `ToolContext` — verify LRU cache eviction logic, `get_cache()` stale detection, `clean_file_cache()` reservation in `crates/agent_scope_state/src/tool_context.rs`
- [X] T091 [P] [US3] Write unit tests for `Task` and `TaskContext` — task creation, state transitions (pending→in_progress→completed), dependency query (blocks/blocked_by) in `crates/agent_scope_state/src/task.rs`
- [X] T092 [US3] Write unit tests for `AgentState::append_context()` — append to matching tail message (same name+reply_id), create new message (different name), reject on ContextFull when max_context_messages reached in `crates/agent_scope_state/src/agent_state.rs`
- [X] T093 [US3] Write unit tests for `AgentState::has_awaiting_tool_calls()` — ASKING state returns true, SUBMITTED without result returns true, FINISHED returns false, no tool calls returns false in `crates/agent_scope_state/src/agent_state.rs`
- [X] T094 [US3] Write unit tests for `AgentState` JSON serialization round-trip — create full state with context (multiple Msg), reply_context, tool_context with cache entries, tasks — serialize, deserialize, verify all nested structures intact in `crates/agent_scope_state/src/agent_state.rs`
- [X] T095 [US3] Write unit tests for legacy format migration — provide JSON with top-level `reply_id`/`cur_iter`, verify auto-migration to `reply_context` nested structure in `crates/agent_scope_state/src/agent_state.rs`

**Checkpoint**: `cargo test -p agent_scope_state` passes all 7 test groups. State can be created, populated, serialized, and legacy-migrated.

---

## Phase 6: User Story 5 — Foundation 层的零内部依赖拓扑 (Priority: P3)

**Goal**: Verify and enforce the correct dependency topology — types depends on nothing, message only depends on types, event depends on message+types, state depends on message+types. No circular dependencies, no dependency on upper-layer modules (model/tool/agent).

**Independent Test**: Static dependency analysis via `cargo tree` confirms the topological constraints.

### Implementation for User Story 5

- [X] T096 [P] [US5] Configure `crates/agent_scope_types/Cargo.toml` to have zero agentscope internal dependencies — verify with `cargo tree -p agent_scope_types --no-deps`
- [X] T097 [P] [US5] Configure `crates/agent_scope_message/Cargo.toml` to depend only on `agent_scope_types` and `agent_scope_utils` — verify with `cargo tree -p agent_scope_message`
- [X] T098 [P] [US5] Configure `crates/agent_scope_event/Cargo.toml` and `crates/agent_scope_state/Cargo.toml` to depend only on `agent_scope_message` + `agent_scope_types` + `agent_scope_utils` — verify with `cargo tree` for both crates, confirm no dependency on model/tool/agent crates (not yet created)
- [X] T099 [US5] Add `#![deny(unsafe_code)]` to `lib.rs` of all 5 crates (types, message, event, state, utils) and verify compilation succeeds

**Checkpoint**: `cargo tree --no-deps` shows correct dependency hierarchy. No circular deps. No upper-layer deps.

---

## Phase 7: Integration Tests & Cross-Cutting Concerns

**Purpose**: Cross-crate integration tests, serialization round-trip validation, and code quality checks.

### Integration Tests

- [X] T100 [P] Write integration tests for `ReplyFinishedReason` and `ErrorType/ErrorInfo` serialization round-trip in `tests/types/reply_tests.rs` and `tests/types/error_tests.rs`
- [X] T101 [P] Write integration tests for hook type constants in `tests/types/hook_tests.rs`
- [X] T102 [P] Write integration tests for `Msg` creation, ContentBlock operations, and factory functions in `tests/message/msg_tests.rs`
- [X] T103 [P] Write integration tests for `ContentBlock` tagged serialization (all 6 variants) in `tests/message/block_tests.rs`
- [X] T104 [P] Write integration tests for `append_event` text streaming in `tests/message/append_event_tests.rs`
- [X] T105 [P] Write integration tests for `EventType` full enumeration (28 variants) in `tests/event/event_type_tests.rs`
- [X] T106 [P] Write integration tests for event struct serialization (sample 10 key event types) in `tests/event/event_serde_tests.rs`
- [X] T107 [P] Write integration tests for `AgentState` creation and manipulation in `tests/state/agent_state_tests.rs`
- [X] T108 [P] Write integration tests for `Task` and `TaskContext` in `tests/state/task_tests.rs`
- [X] T109 [P] Write integration tests for legacy format migration in `tests/state/migration_tests.rs`
- [X] T110 Write integration tests for Foundation-layer cross-crate serialization consistency — verify Msg serialized from message crate correctly deserializes in state crate, verify Event serialized from event crate correctly applies to Msg in message crate in `tests/message/cross_crate_tests.rs`

### Compatibility Tests (Golden Snapshot Diff)

- [X] T111 [P] Create Python golden snapshot generation script `tests/compatibility/generate_fixtures.py` that exports sample Msg/Event/State JSON for each type
- [X] T112 Create Rust golden snapshot diff test framework in `tests/compatibility/diff_tests.rs` — reads fixture JSON, compares Rust serialization output (with timestamp/UUID normalization), reports mismatches
- [X] T113 Populate `tests/compatibility/fixtures/` with golden snapshot JSON files from Python reference implementation for Msg, all ContentBlock types, all Event types, AgentState, and Task

### Code Quality

 - [X] T114 [P] Run `cargo clippy -- -D warnings` on workspace and fix all warnings
 - [X] T115 [P] Run `cargo fmt -- --check` on workspace and format all code
- [X] T116 Verify all tests in `tests/quickstart.md` validation scenarios compile and pass

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **US4/Types (Phase 2)**: Depends on Setup (Phase 1) — BLOCKS US1, US2, US3
- **US1/Message (Phase 3)**: Depends on US4/Types — BLOCKS US2 (event needs message types)
- **US2/Event (Phase 4)**: Depends on US1/Message (AgentEvent variants reference ToolCallBlock, etc.)
- **US3/State (Phase 5)**: Depends on US1/Message (AgentState holds Vec<Msg>, ReplyContext, etc.)
- **US5/Topology (Phase 6)**: Can be verified after Phase 1 Setup independently, fully verified after Phases 2-5
- **Polish (Phase 7)**: Depends on all previous phases

### Module Dependency Graph

```
agent_scope_utils  ← (no agentscope deps)
agent_scope_types  ← (no agentscope deps)
     ↑
agent_scope_message  ← (types + utils)
     ↑
agent_scope_event  ← (message + types + utils)
agent_scope_state  ← (message + types + utils)
```

### User Story Dependencies

- **US4 (P2)**: Phase 2 — Foundational, MUST complete first (types are dependency for everything)
- **US1 (P1)**: Phase 3 — Depends on US4 only; no dependency on US2/US3
- **US2 (P1)**: Phase 4 — Depends on US1+US4 (needs Msg, ContentBlock, ToolCallBlock from message crate)
- **US3 (P2)**: Phase 5 — Depends on US1+US4 (needs Msg, ContentBlock from message crate); independent of US2
- **US5 (P3)**: Phase 6 — Depends on all crates existing; verification-only

### Within Each User Story

- Data types (structs/enums) before aggregates (tagged union, Msg)
- Aggregates before methods (get_content_blocks, append_event)
- Implementation before module exports
- Module exports before tests
- All implementation tasks before story checkpoint

### Parallel Opportunities

- **Phase 1**: T002, T003, T004, T005, T006, T007 all run in parallel (different crate directories)
- **Phase 2**: T008, T009, T011, T012, T013 all [P] — run in parallel; T010 depends on T009
- **Phase 3**: T019-T028 (ContentBlock sub-types) all [P] — run in parallel; T029 (ContentBlock enum) depends on all sub-types
- **Phase 4**: T054-T057 (block events) all [P]; T058-T059 (tool events) sequential within group
- **Phase 5**: T075, T076, T078, T080, T081 all [P] — run in parallel
- **Phase 7**: All integration test tasks T100-T111 [P] — run in parallel

---

## Parallel Example: User Story 1 ContentBlock Implementation

```bash
# Launch all ContentBlock sub-types together (T019-T028):
Task: "Implement TextBlock struct in crates/agent_scope_message/src/block.rs"
Task: "Implement ThinkingBlock struct in crates/agent_scope_message/src/block.rs"
Task: "Implement HintContent enum and HintBlock struct in crates/agent_scope_message/src/block.rs"
Task: "Implement Base64Source struct in crates/agent_scope_message/src/source.rs"
Task: "Implement URLSource struct in crates/agent_scope_message/src/source.rs"
Task: "Implement DataSource enum and DataBlock struct in crates/agent_scope_message/src/block.rs"
Task: "Implement ToolCallState enum in crates/agent_scope_message/src/state.rs"
Task: "Implement ToolCallBlock struct in crates/agent_scope_message/src/block.rs"
Task: "Implement ToolResultState enum in crates/agent_scope_message/src/state.rs"
Task: "Implement ToolResultBlock struct in crates/agent_scope_message/src/block.rs"

# After all sub-types complete, then:
Task: "Implement ContentBlock tagged enum (T029)"
Task: "Implement BlockType enum (T030)"
```

---

## Implementation Strategy

### MVP First (Types + Message Only)

1. Complete Phase 1: Setup — all crates compile empty
2. Complete Phase 2: US4 (Types) — ErrorType, ErrorInfo, ReplyFinishedReason, hooks
3. Complete Phase 3: US1 (Message) — Msg, ContentBlock, factory functions
4. **STOP and VALIDATE**: Create messages, serialize/deserialize, verify role validation
5. MVP delivers: Message model — the fundamental data carrier. All higher layers can now build on Msg.

### Incremental Delivery

1. Setup (Phase 1) → Workspace compiles ✅
2. Types (Phase 2 / US4) → Error model, reply reasons, hooks ✅
3. Message (Phase 3 / US1) → **MVP**: Messages can be created and serialized ✅
4. Event (Phase 4 / US2) → Streaming event system, append_event integration ✅
5. State (Phase 5 / US3) → Session state persistence, task management ✅
6. Topology (Phase 6 / US5) → Verified dependency hierarchy ✅
7. Polish (Phase 7) → Integration tests, golden snapshots, clippy/fmt ✅

### Parallel Team Strategy

With multiple developers:
1. Team completes Phase 1 (Setup) + Phase 2 (Types) together
2. After Phase 2:
   - Developer A: US1 (Message crate — Phase 3)
   - Developer B: Starts on US3 (State crate — Phase 5) once US1 ContentBlock types are stable
   - Developer C: Prepares golden snapshot fixtures (T111-T113) in parallel
3. After US1:
   - Developer A: US2 (Event crate — Phase 4) — needs Msg from US1
   - Developer B: Continues US3 (State crate — Phase 5)
4. All: Phase 7 (Integration tests, clippy, fmt)

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story phase should be independently completable and testable
- ContentBlock `type` tag uses serde `#[serde(tag = "type")]` internally-tagged enum per research.md decision #1
- ThinkingBlock uses `#[serde(flatten)]` + `HashMap<String, JsonValue>` for provider extras per research.md decision #2
- ToolCallBlock.input stored as raw JSON `String` — not parsed at Foundation layer per research.md decision #5
- DataBlock base64 streaming uses decode-concat-re-encode pattern per research.md decision #6
- Golden snapshot diff tests normalize timestamps and UUIDs before comparison per research.md decision #10
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently

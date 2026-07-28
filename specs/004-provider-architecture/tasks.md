# Tasks: Provider Architecture & DashScope Integration

**Input**: Design documents from `/specs/004-provider-architecture/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Tests are REQUIRED per spec.md FR-021 ("每个 Provider crate MUST 包含 mock HTTP 测试") and SC-003 (≥10 DashScope tests). Mock HTTP tests (`wiremock`) are integral to all Provider stories.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Create Provider crate scaffolds and prepare workspace

- [ ] T001 Create `crates/agent_scope_openai/` crate scaffold: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/formatter.rs`, `src/parameters.rs`, `tests/` directory, and copy `_models/` from `agent_scope_model/src/openai/_models/`
- [ ] T002 [P] Create `crates/agent_scope_dashscope/` crate scaffold: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/formatter.rs`, `src/parameters.rs`, `tests/` directory
- [ ] T003 [P] Add `wiremock` 0.6 to workspace-level `[dev-dependencies]` or ensure each Provider crate individually declares it in `[dev-dependencies]`

---

## Phase 2: Foundational — Core Dependency Cleanup (Blocking Prerequisites)

**Purpose**: Remove `reqwest`, `tokio-stream`, `tokio-util`, `serde_yaml` from `agent_scope_model`. This MUST complete before any Provider crate work, because Provider crates import from the cleaned `agent_scope_model`.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete. Core must be pure first.

- [ ] T004 Remove `pub mod openai;` and all OpenAI re-exports from `crates/agent_scope_model/src/lib.rs`
- [ ] T005 Remove `openai/` submodule directory entirely from `crates/agent_scope_model/src/openai/`
- [ ] T006 Refactor `ModelCard::from_yaml()` → `ModelCard::from_raw(raw_data: &HashMap, base_schema: &JsonValue)` in `crates/agent_scope_model/src/card.rs`
- [ ] T007 Remove `reqwest`, `tokio-stream`, `tokio-util`, `serde_yaml` from `crates/agent_scope_model/Cargo.toml` dependencies; verify `futures` stays (needed for `Pin<Box<dyn Stream>>` in `model_trait.rs`)
- [ ] T008 Move `tests/formatter_integration.rs` from `crates/agent_scope_model/tests/` to `crates/agent_scope_openai/tests/`
- [ ] T009 Build and test `cargo build -p agent_scope_model` and `cargo test -p agent_scope_model` — confirm all remaining core tests pass and `cargo tree -p agent_scope_model` shows zero `reqwest` dependency

**Checkpoint**: Foundation ready — `agent_scope_model` is pure (no HTTP deps, no Provider code). User story implementation can now begin.

---

## Phase 3: User Story 1 — Provider Crate 拆分与独立部署 (Priority: P1) 🎯 MVP

**Goal**: Extract OpenAI implementation to independent `agent_scope_openai` crate, keeping all existing behavior and tests.

**Independent Test**: `cargo build -p agent_scope_openai` compiles independently; `cargo test -p agent_scope_openai` passes all 10 original tests; `cargo tree -p agent_scope_model` shows zero Provider crates in dependency tree.

### Implementation for User Story 1

- [ ] T010 [US1] Migrate `openai/model.rs` to `crates/agent_scope_openai/src/model.rs`: rewrite `use crate::*` → `use agent_scope_model::*` imports, add `reqwest::Client` dep
- [ ] T011 [P] [US1] Migrate `openai/formatter.rs` to `crates/agent_scope_openai/src/formatter.rs`: rewrite `use crate::*` → `use agent_scope_model::*` imports
- [ ] T012 [P] [US1] Migrate `openai/parameters.rs` to `crates/agent_scope_openai/src/parameters.rs`: rewrite `use crate::*` → `use agent_scope_model::*` imports
- [ ] T013 [US1] Wire up `crates/agent_scope_openai/src/lib.rs`: declare modules (`model`, `formatter`, `parameters`), re-export `OpenAIChatModel`, `OpenAIChatFormatter`, `OpenAIChatParameters`, `ReasoningEffort`
- [ ] T014 [US1] Migrate formatter integration tests from `crates/agent_scope_openai/tests/formatter_integration.rs` (moved in T008): update imports to use `agent_scope_openai::*`
- [ ] T015 [US1] Add mock HTTP tests for OpenAI non-streaming call in `crates/agent_scope_openai/tests/model_tests.rs` using `wiremock`
- [ ] T016 [US1] Run `cargo build -p agent_scope_openai && cargo test -p agent_scope_openai` — confirm all 10+ tests pass
- [ ] T017 [US1] Run `cargo tree -p agent_scope_model --no-dedupe | grep -q reqwest` — confirm FAIL (no reqwest in core)

**Checkpoint**: OpenAI Provider fully extracted, core crate pure, all tests passing.

---

## Phase 4: User Story 2 — DashScope Provider 实现 (Priority: P1)

**Goal**: Implement `DashScopeChatModel` in `agent_scope_dashscope` crate, supporting text chat, streaming, tool calling, and structured output via DashScope OpenAI-compatible endpoint.

**Independent Test**: `cargo test -p agent_scope_dashscope` passes ≥10 mock HTTP tests covering non-streaming, streaming, tool calls, structured output, and error handling.

### Implementation for User Story 2

- [ ] T018 [P] [US2] Implement `DashScopeParameters` in `crates/agent_scope_dashscope/src/parameters.rs`: fields per data-model (max_tokens, temperature, top_p, top_k, enable_search, enable_thinking, thinking_budget, repetition_penalty, seed, stop) with `schemars::JsonSchema` derive, serde `skip_serializing_if = "Option::is_none"`, and `#[serde(other)]` for forward compatibility
- [ ] T019 [P] [US2] Implement `DashScopeFormatter` in `crates/agent_scope_dashscope/src/formatter.rs`: implement `Formatter` trait, format Msg → DashScope API JSON (single TextBlock → string content, multimodal → content array, ToolCall → assistant role, ToolResult → tool role)
- [ ] T020 [US2] Implement `DashScopeChatModel` struct + `ChatModel` trait in `crates/agent_scope_dashscope/src/model.rs`: fields (api_key, base_url, model_name, parameters, stream, max_retries, retry_delay, context_size, formatter, client, extra_body), constructor `new()` and builder methods
- [ ] T021 [US2] Implement `ChatModel::call_api()` in `crates/agent_scope_dashscope/src/model.rs`: POST to `{base_url}/chat/completions`, build request body from messages + parameters + tools, handle non-streaming response → `ModelCallResult::Complete(ChatResponse)`
- [ ] T022 [US2] Implement streaming in `ChatModel::call_api()`: SSE byte stream parsing (`data:` lines, `[DONE]` marker), yield chunks via `tokio::sync::mpsc` or `futures::Stream`, handle empty `choices: []` for usage-only final chunks
- [ ] T023 [US2] Implement tool calling support in `crates/agent_scope_dashscope/src/model.rs`: format tool definitions → DashScope JSON, parse tool_calls in response → `ToolCallBlock` in ChatResponse, handle `tool_choice` parameter (auto/none/required, with `required` constraint check)
- [ ] T024 [US2] Implement `generate_structured_output()` in `crates/agent_scope_dashscope/src/model.rs`: inject `generate_structured_output` tool with JSON Schema, parse tool call result → `StructuredResponse`
- [ ] T025 [US2] Implement error handling: error response parser (compatible with both nested `{"error": ...}` and flat `{"code": ..., "message": ...}` formats), HTTP status → `ModelErrorKind` mapping per dashscope-api.md contract, implement `retryable_errors()` (429/500/502/503/timeout)
- [ ] T026 [US2] Implement `count_tokens()` in `crates/agent_scope_dashscope/src/model.rs`: attempt DashScope tokenizer API first, fallback to bytes/2 heuristic for Chinese text
- [ ] T027 [US2] Wire up `crates/agent_scope_dashscope/src/lib.rs`: declare modules, re-export `DashScopeChatModel`, `DashScopeFormatter`, `DashScopeParameters`
- [ ] T028 [US2] Write mock HTTP test: non-streaming text chat in `crates/agent_scope_dashscope/tests/model_tests.rs` — verify ChatResponse with text content
- [ ] T029 [P] [US2] Write mock HTTP test: streaming SSE response in `crates/agent_scope_dashscope/tests/model_tests.rs` — verify stream chunks accumulate to correct ChatResponse via `StreamAccumulator`
- [ ] T030 [P] [US2] Write mock HTTP test: tool calling (function call request → tool call response parse) in `crates/agent_scope_dashscope/tests/model_tests.rs`
- [ ] T031 [P] [US2] Write mock HTTP test: structured output via tool-calling mechanism in `crates/agent_scope_dashscope/tests/model_tests.rs`
- [ ] T032 [P] [US2] Write mock HTTP test: error responses (401 auth error, 429 rate limit, 500 server error) in `crates/agent_scope_dashscope/tests/model_tests.rs`
- [ ] T033 [US2] Write formatter tests in `crates/agent_scope_dashscope/tests/formatter_tests.rs`: text message formatting, multimodal formatting, tool call/tool result formatting
- [ ] T034 [P] [US2] Write parameter serde round-trip tests in `crates/agent_scope_dashscope/tests/parameters_tests.rs`: serialization/deserialization with various parameter combinations, verify `enable_search`, `enable_thinking` serialization
- [ ] T035 [US2] Run `cargo test -p agent_scope_dashscope` — confirm ≥10 tests pass, all covered scenarios work

**Checkpoint**: DashScope Provider fully functional with mock-tested coverage of all scenarios (non-streaming, streaming, tool calls, structured output, errors).

---

## Phase 5: User Story 3 — Provider 通用测试基础设施 (Priority: P2)

**Goal**: Create reusable mock HTTP test helpers and optionally extract common testing patterns.

**Independent Test**: `cargo test -p agent_scope_test_utils` (if extracted crate) passes, OR both OpenAI and DashScope crates use shared test pattern macros/functions.

**Note**: Per research.md decision, FR-020 is downgraded from MUST to SHOULD. If only 2 Provider crates exist, the simplest approach is a shared `tests/common/mod.rs` pattern or duplicate with consistency. A separate `agent_scope_test_utils` crate is deferred until a 3rd Provider exists.

### Implementation for User Story 3

- [ ] T036 [US3] Extract common wiremock helpers (mock server setup, SSE response builder, JSON response builder) into `crates/agent_scope_model/tests/common/` as a reusable module, re-export for other crate tests via `[dev-dependencies]` path or doc comment reference
- [ ] T037 [US3] Add SSE stream builder helper: create `fn build_sse_stream(chunks: &[&str]) -> String` that joins chunks with SSE framing (`data: ...\n\n`)
- [ ] T038 [US3] Refactor `agent_scope_openai/tests/model_tests.rs` to use shared helpers from T036-T037
- [ ] T039 [P] [US3] Refactor `agent_scope_dashscope/tests/model_tests.rs` to use shared helpers from T036-T037
- [ ] T040 [US3] Verify `cargo test --workspace` passes with all crates using shared test infrastructure

**Checkpoint**: Test infrastructure shared, duplication eliminated, all Provider tests still passing.

---

## Phase 6: User Story 4 — Provider 注册与发现机制 (Priority: P3)

**Goal**: Implement runtime Provider selection from configuration (provider name → `Box<dyn ChatModel>`).

**Independent Test**: Pass config `{"provider": "dashscope", "model_name": "qwen-plus"}` and receive a working `Box<dyn ChatModel>`; pass unknown provider name and receive `ConfigError`.

### Implementation for User Story 4

- [ ] T041 [US4] Define `ProviderConfig` struct in `crates/agent_scope_model/src/provider_config.rs`: fields `provider` (String), `api_key` (String), `model_name` (String), `base_url` (Option<String>), `parameters` (serde_json::Value for provider-specific params)
- [ ] T042 [US4] Define `ProviderRegistry` trait in `crates/agent_scope_model/src/provider_config.rs`: `fn create(&self, config: &ProviderConfig) -> Result<Arc<dyn ChatModel>, ModelError>`
- [ ] T043 [US4] Implement `OpenAIProviderRegistry` in `crates/agent_scope_openai/src/registry.rs`: parse `ProviderConfig` → construct `OpenAIChatModel`
- [ ] T044 [US4] Implement `DashScopeProviderRegistry` in `crates/agent_scope_dashscope/src/registry.rs`: parse `ProviderConfig` → construct `DashScopeChatModel`
- [ ] T045 [US4] Implement `DefaultProviderRegistry` in `crates/agent_scope_model/src/provider_config.rs`: map of provider name → `Box<dyn ProviderRegistry>`, `register()` and `create()` methods, return `ConfigError` for unknown provider names (not panic)
- [ ] T046 [US4] Write integration test: construct `DashScopeChatModel` from config in `crates/agent_scope_model/tests/provider_registry_tests.rs`
- [ ] T047 [US4] Write integration test: unknown provider name returns error in `crates/agent_scope_model/tests/provider_registry_tests.rs`
- [ ] T048 [US4] Run `cargo test --workspace` — confirm registry tests pass

**Checkpoint**: Provider registry functional, configuration-driven Provider instantiation works.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final validation, lints, and documentation.

- [ ] T049 [P] Run `cargo clippy --workspace -- -D warnings` and fix all warnings across all crates
- [ ] T050 [P] Run `cargo fmt --all -- --check` and format all crates
- [ ] T051 Verify dependency topology: `cargo tree -p agent_scope_model --no-dedupe` shows no `reqwest`/`openai`/`dashscope`
- [ ] T052 Verify dependency topology: `cargo tree -p agent_scope_dashscope --no-dedupe` shows no `agent_scope_tool`/`agent_scope_agent`
- [ ] T053 Update `agent_scope_model/Cargo.toml` description: remove "OpenAI reference implementation" from description field
- [ ] T054 Run full quickstart.md validation checklist:
  - `cargo build -p agent_scope_model` passes
  - `cargo test -p agent_scope_model` — all core tests pass
  - `cargo build -p agent_scope_openai` passes
  - `cargo test -p agent_scope_openai` — all tests pass
  - `cargo build -p agent_scope_dashscope` passes
  - `cargo test -p agent_scope_dashscope` — ≥10 tests pass
  - `cargo tree -p agent_scope_model` has zero Provider deps
  - `cargo tree -p agent_scope_dashscope` has only Foundation deps
- [ ] T055 [P] Update project-level documentation: add Provider crate overview to `README.md` or workspace docs

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (crate scaffolds must exist for T008) — **BLOCKS all user stories**
- **User Story 1 (Phase 3)**: Depends on Phase 2 completion — OpenAI code can only be wired after core is clean
- **User Story 2 (Phase 4)**: Depends on Phase 2 completion — DashScope needs clean `agent_scope_model` to import. Can start in parallel with US1
- **User Story 3 (Phase 5)**: Depends on US1 + US2 completion — needs both Provider crates to extract common patterns
- **User Story 4 (Phase 6)**: Depends on US1 + US2 completion — needs both Provider crates to implement registries. Can start in parallel with US3
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

```
Phase 1 (Setup)
    │
    ▼
Phase 2 (Foundational — Core Cleanup) ◄── BLOCKS ALL STORIES
    │
    ├─► Phase 3 (US1: OpenAI Extraction) ─────────────────────┐
    │                                                          │
    └─► Phase 4 (US2: DashScope Provider) ────────────────────┤
                                                               │
                               ┌───────────────────────────────┘
                               ▼
                    Phase 5 (US3: Test Infra) ──┬── Phase 6 (US4: Registry)
                                                │
                                                ▼
                                        Phase 7 (Polish)
```

- **US1 and US2 are parallel** after Phase 2 — they have zero overlap (different crates)
- **US3 depends on US1 + US2** — needs both to extract common patterns
- **US4 depends on US1 + US2** — needs both to implement registries
- **US3 and US4 are parallel** — different files, no shared dependencies

### Within Each User Story

- Parameters before model (model uses parameter types)
- Formatter before model (model uses formatter)
- Core implementation before tests
- Tests after the feature they test compiles

---

## Parallel Opportunities

### Phase 1 (Setup)
```
Parallel: T001 (OpenAI scaffold) + T002 (DashScope scaffold)
```

### Phase 3 (US1: OpenAI Extraction)
```
Parallel: T011 (formatter.rs) + T012 (parameters.rs) — different files, can run together
```

### Phase 4 (US2: DashScope Provider)
```
Parallel batch 1: T018 (parameters) + T019 (formatter) — different files
Then: T020 (model struct — depends on T018, T019 types)
Then parallel batch 2: T029 (streaming test) + T030 (tool call test) + T031 (structured output test) + T032 (error test) + T034 (parameter test)
```

### Phase 5 (US3: Test Infrastructure) — parallel with Phase 6 (US4: Registry)
```
Whole phases can run in parallel: Phase 5 || Phase 6
```

### Phase 6 (US4: Registry)
```
Parallel: T043 (OpenAI registry) + T044 (DashScope registry) + T046 (integration test) + T047 (error test)
```

### Phase 7 (Polish)
```
Parallel: T049 (clippy) + T050 (fmt) + T055 (docs)
```

---

## Parallel Example: User Story 2

```bash
# Batch 1 — Launch models together (different files, no deps):
Task: "Implement DashScopeParameters in crates/agent_scope_dashscope/src/parameters.rs"
Task: "Implement DashScopeFormatter in crates/agent_scope_dashscope/src/formatter.rs"

# After T018 + T019 complete:
Task: "Implement DashScopeChatModel struct + ChatModel trait in crates/agent_scope_dashscope/src/model.rs"

# Batch 2 — Launch tests together (different test cases, all mock):
Task: "Write mock HTTP test: streaming SSE response in tests/model_tests.rs"
Task: "Write mock HTTP test: tool calling in tests/model_tests.rs"
Task: "Write mock HTTP test: structured output in tests/model_tests.rs"
Task: "Write mock HTTP test: error responses in tests/model_tests.rs"
Task: "Write parameter serde round-trip tests in tests/parameters_tests.rs"
```

---

## Implementation Strategy

### MVP First (US1: OpenAI Extraction Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (Core Dependency Cleanup)
3. Complete Phase 3: User Story 1 (OpenAI Extraction)
4. **STOP and VALIDATE**: `cargo test -p agent_scope_openai` passes, `cargo tree -p agent_scope_model` has no `reqwest`
5. Deploy/demo — core crate is now Provider-agnostic

### Incremental Delivery

1. Setup + Foundational → Foundation ready, all crates scaffolded
2. Add US1 (OpenAI) → Test independently → Core is pure, OpenAI works (MVP!)
3. Add US2 (DashScope) → Test independently → Second Provider functional
4. Add US3 (Test Infra) → Test independently → Test helpers shared
5. Add US4 (Registry) → Test independently → Config-driven Provider selection
6. Polish → Production-ready quality

### Single Developer Strategy

Recommended order: Phase 1 → Phase 2 → Phase 3 (US1) → Phase 4 (US2) → Phase 5 (US3) → Phase 6 (US4) → Phase 7

### Parallel Team Strategy

With 2+ developers:
1. All complete Phase 1 + Phase 2 together
2. After Phase 2:
   - Developer A: Phase 3 (US1: OpenAI Extraction)
   - Developer B: Phase 4 (US2: DashScope Provider)
3. After US1 + US2 complete:
   - Developer A: Phase 5 (US3: Test Infra)
   - Developer B: Phase 6 (US4: Registry)
4. All: Phase 7 (Polish)

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- All Provider tests use `wiremock` mock HTTP (no real LLM, no network)
- `#![deny(unsafe_code)]` applies to all new crates
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- SC-001 through SC-006 from spec.md are the final acceptance criteria

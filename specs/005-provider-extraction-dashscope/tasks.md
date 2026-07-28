# Tasks: Provider 剥离与 DashScope 优先实现

**Feature**: 005-provider-extraction-dashscope
**Branch**: `005-provider-extraction-dashscope`
**Date**: 2026-07-29
**Input**: Design documents from `/specs/005-provider-extraction-dashscope/`

**Current State** (verified 2026-07-29):
- ✅ `openai/` deleted (model.rs, formatter.rs, parameters.rs, mod.rs, _models/)
- ✅ `lib.rs` cleaned — no `pub mod openai`, no OpenAI re-exports
- ✅ `Cargo.toml` deps cleaned — no `reqwest`, `tokio-stream`, `tokio-util`, `thiserror`
- ✅ `formatter_integration.rs` removed
- ❌ **BLOCKER**: `card.rs:83` still calls `serde_yaml::from_str()` but `serde_yaml` is not in Cargo.toml → compile error
- ✅ Description updated to "provider-independent abstractions"

**Tests**: REQUIRED per spec.md FR-019 & SC-002 (≥10 DashScope mock tests).

---

## Phase 1: Foundational — Core Crate Compile Fix 🔴 BLOCKER

**Purpose**: Fix the single remaining compile blocker. Everything else in US1 is already done.

**⚠️ CRITICAL**: DashScope crate cannot import `agent_scope_model` until it compiles.

- [ ] T001 Refactor `ModelCard::from_yaml()` at `crates/agent_scope_model/src/card.rs:74-85` — change signature from `from_yaml(path: &Path, base_schema: &JsonValue)` to `from_raw(yaml_str: &str, base_schema: &JsonValue)`. Remove `std::path::Path` import and `std::fs::read_to_string` call. Replace `serde_yaml::from_str(&raw)` at line 83 with deserialization that works without serde_yaml. **Option A**: Keep YAML parsing internal but add `serde_yaml` back to Cargo.toml. **Option B** (per research.md): Make caller responsible for providing a `RawModelCardYaml` value, removing YAML parsing from core entirely.
- [ ] T002 Verify `cargo build -p agent_scope_model` succeeds
- [ ] T003 Verify `cargo test -p agent_scope_model` — both remaining test files pass: `chat_response_integration.rs`, `cross_crate_tests.rs`
- [ ] T004 Verify `cargo tree -p agent_scope_model --no-dedupe | grep -qiE 'reqwest|openai|dashscope|serde_yaml'` returns nothing (exit 1 = PASS)

**Checkpoint**: Core crate compiles cleanly — the single blocker is resolved.

---

## Phase 2: User Story 1 — 核心脱耦验证 ✅ (Priority: P1) 🎯 MVP

**Goal**: Confirm all US1 work is complete. (All implementation work was done prior; T001 was the only remaining fix.)

**Independent Test**: `cargo build -p agent_scope_model && cargo test -p agent_scope_model`.

- [ ] T005 [US1] Confirm `crates/agent_scope_model/src/openai/` does not exist
- [ ] T006 [US1] Confirm `crates/agent_scope_model/src/lib.rs` has zero OpenAI mentions
- [ ] T007 [US1] Confirm `crates/agent_scope_model/Cargo.toml` has zero `reqwest`/`tokio-stream`/`tokio-util`/`serde_yaml`/`thiserror` mentions
- [ ] T008 [US1] Confirm `crates/agent_scope_model/tests/formatter_integration.rs` does not exist

**Checkpoint**: US1 fully verified. Core crate is pure.

---

## Phase 3: User Story 2 — DashScope Provider 实现 (Priority: P1)

**Goal**: Create `agent_scope_dashscope` crate implementing `ChatModel` trait.

**Independent Test**: `cargo test -p agent_scope_dashscope` ≥10 mock tests pass.

### 3.1 Crate Scaffold

- [ ] T009 [US2] Create `crates/agent_scope_dashscope/` directory structure: `src/` (lib.rs, model.rs, formatter.rs, parameters.rs), `tests/`
- [ ] T010 [US2] Create `crates/agent_scope_dashscope/Cargo.toml` with deps: `agent_scope_model`, `agent_scope_message`, `agent_scope_types` (path), `reqwest` 0.12 (stream+json), `tokio` 1.x (full), `tokio-stream` 0.1, `futures` 0.3, `serde`/`serde_json`, `serde_yaml` 0.9 (for model card YAML, Provider-side), `base64`, `schemars`, `uuid`, `chrono`. Dev: `wiremock` 0.6, `tokio` (macros)
- [ ] T011 [US2] Create `crates/agent_scope_dashscope/src/lib.rs` — `#![deny(unsafe_code)]`, `pub mod model/formatter/parameters`, re-export `DashScopeChatModel`, `DashScopeFormatter`, `DashScopeParameters`

### 3.2 Parameters & Formatter (parallel)

- [ ] T012 [P] [US2] Implement `DashScopeParameters` in `crates/agent_scope_dashscope/src/parameters.rs` — fields: `max_tokens`(Option<u32>), `temperature`(Option<f64>), `top_p`(Option<f64>), `top_k`(Option<u32>), `enable_search`(bool, default false), `enable_thinking`(bool, default false), `thinking_budget`(Option<u32>), `repetition_penalty`(Option<f64>), `seed`(Option<u64>), `stop`(Option<Vec<String>>). Derive: Debug, Clone, Serialize, Deserialize, JsonSchema. Serde attrs: `skip_serializing_if="Option::is_none"`, `#[serde(other)]` on enum if any. Impl Default. Add `validate()` → errors if `repetition_penalty <= 0` or `enable_thinking && tool_choice=="required"`
- [ ] T013 [P] [US2] Implement `DashScopeFormatter` in `crates/agent_scope_dashscope/src/formatter.rs` — struct: `input_types: Vec<String>`, impl `Formatter` trait: `supported_input_media_types()`, `format(msgs) -> Result<Vec<JsonValue>, FormatError>`: single TextBlock → `{"role":"...","content":"<text>"}`, multimodal → content array `[{"type":"text","text":"..."},{"type":"image_url","image_url":{"url":"data:..."}}]`, ToolCall → `{"role":"assistant","tool_calls":[...]}`, ToolResult → `{"role":"tool","tool_call_id":"...","content":"..."}`, `convert_tool_result_to_string()`

### 3.3 ChatModel Implementation

- [ ] T014 [US2] Define `DashScopeChatModel` struct in `crates/agent_scope_dashscope/src/model.rs` — fields: `api_key: String`, `base_url: String` (default `https://dashscope.aliyuncs.com/compatible-mode/v1`), `model_name: String`, `parameters: DashScopeParameters` (default), `stream: bool` (default true), `max_retries: u32` (default 3), `retry_delay: f64` (default 1.0), `context_size: i64` (default 131072), `formatter: Box<dyn Formatter>`, `client: reqwest::Client`, `extra_body: HashMap<String, JsonValue>`. Impl `new(api_key, model_name) -> Self`, builder: `with_base_url()`, `with_parameters()`, `with_stream()`
- [ ] T015 [US2] Implement `ChatModel` trait boilerplate in `crates/agent_scope_dashscope/src/model.rs`: `model_name()`, `stream_enabled()`, `context_size()`, `max_retries()`, `retry_delay()`, `formatter()` accessor
- [ ] T016 [US2] Implement `call_api()` non-streaming path in `crates/agent_scope_dashscope/src/model.rs`: build request JSON (model, messages via formatter, stream=false, parameters merged, tools, tool_choice), POST to `{base_url}/chat/completions` with `Authorization: Bearer {api_key}`, parse JSON response → `ChatResponse` (choices, message, usage, finish_reason), return `ModelCallResult::Complete`
- [ ] T017 [US2] Implement `call_api()` streaming path in `crates/agent_scope_dashscope/src/model.rs`: when `stream=true`, add `stream_options: {"include_usage": true}` to body, read `response.bytes_stream()`, split by `\n`, parse `data: {json}` lines, emit chunks. Handle: `data: [DONE]` → close stream, `choices: []` + `usage` → capture usage without panic, delta aggregation
- [ ] T018 [US2] Implement tool formatting in `crates/agent_scope_dashscope/src/model.rs`: `format_tools(tools: &[Tool]) -> Vec<JsonValue>` → `[{"type":"function","function":{"name":"...","description":"...","parameters":{...}}}]`. Parse tool_calls in response → `ToolCallBlock` (id, function name, arguments). Handle `tool_choice`: `"auto"`/`"none"`/`{"type":"function","function":{"name":"X"}}`
- [ ] T019 [US2] Implement `retryable_errors()` in `crates/agent_scope_dashscope/src/model.rs` → return `[RateLimit, InternalServer, ApiTimeout]`
- [ ] T020 [US2] Implement structured output in `crates/agent_scope_dashscope/src/model.rs`: `generate_structured_output()` → inject `generate_structured_output` tool with schema, force tool_choice to that tool, parse tool call arguments → `StructuredResponse`
- [ ] T021 [US2] Implement error response parsing in `crates/agent_scope_dashscope/src/model.rs`: parse error from HTTP response body: try `{"error":{"message":"...","code":"..."}}` first, fallback to `{"code":"...","message":"..."}`. Map HTTP status + error code to `ModelError` variants per contracts/dashscope-api.md
- [ ] T022 [US2] Implement `count_tokens()` in `crates/agent_scope_dashscope/src/model.rs` — default fallback: byte_len/2 heuristic for Chinese text

### 3.4 Tests (all parallel, different test functions)

- [ ] T023 [P] [US2] Non-streaming mock test in `crates/agent_scope_dashscope/tests/model_tests.rs` — wiremock POST /chat/completions → 200 with ChatResponse JSON, verify text content
- [ ] T024 [P] [US2] Streaming SSE mock test in `crates/agent_scope_dashscope/tests/model_tests.rs` — raw SSE body with multiple `data:` chunks + `[DONE]`, verify stream chunks via StreamAccumulator
- [ ] T025 [P] [US2] Empty choices mock test in `crates/agent_scope_dashscope/tests/model_tests.rs` — SSE with `"choices":[],"usage":{...}` chunk, verify no panic
- [ ] T026 [P] [US2] Tool calling mock test in `crates/agent_scope_dashscope/tests/model_tests.rs` — response with tool_calls in message, verify parsed as ToolCallBlock
- [ ] T027 [P] [US2] Structured output mock test in `crates/agent_scope_dashscope/tests/model_tests.rs` — tool call with JSON schema arguments, verify StructuredResponse
- [ ] T028 [P] [US2] Error: 401 auth mock test in `crates/agent_scope_dashscope/tests/model_tests.rs`
- [ ] T029 [P] [US2] Error: 429 rate limit mock test in `crates/agent_scope_dashscope/tests/model_tests.rs`
- [ ] T030 [P] [US2] Error: flat format mock test in `crates/agent_scope_dashscope/tests/model_tests.rs` — `{"code":"...","message":"..."}`
- [ ] T031 [P] [US2] Formatter tests in `crates/agent_scope_dashscope/tests/formatter_tests.rs` — text, multimodal, tool call, tool result
- [ ] T032 [P] [US2] Parameters serde tests in `crates/agent_scope_dashscope/tests/parameters_tests.rs` — round-trip, skip_serializing_if, validation

**Checkpoint**: `cargo test -p agent_scope_dashscope` ≥10 tests pass.

---

## Phase 4: User Story 3 — 测试基础设施 (Priority: P2)

**Goal**: Extract reusable mock helpers for future Provider crates.

- [ ] T033 [US3] Create `fn build_sse_stream(chunks: &[&str]) -> String` helper — wraps chunks in `data: ...\n\n` + appends `data: [DONE]\n\n` — place in `crates/agent_scope_model/tests/common/mod.rs`
- [ ] T034 [P] [US3] Create `fn json_response(status: u16, body: JsonValue) -> ResponseTemplate` helper in same file
- [ ] T035 [US3] Refactor DashScope tests to use shared helpers from T033-T034
- [ ] T036 [US3] Verify `cargo test -p agent_scope_dashscope` still passes

---

## Phase 5: Polish & Validation

- [ ] T037 [P] `cargo clippy --workspace -- -D warnings` — fix all
- [ ] T038 [P] `cargo fmt --all -- --check` — format all
- [ ] T039 Verify: `cargo tree -p agent_scope_model --no-dedupe | grep -qiE 'reqwest|openai|dashscope'` → FAIL (= PASS, no matches)
- [ ] T040 Verify: `cargo tree -p agent_scope_dashscope --no-dedupe | grep -qiE 'agent_scope_tool|agent_scope_agent'` → FAIL (= PASS)
- [ ] T041 Run quickstart.md full checklist: build/test/clippy/fmt for all crates, dependency topologies, Arc<dyn ChatModel> construction

---

## Dependencies & Execution Order

```
Phase 1 (T001-T004): Fix card.rs → core compiles
    │
    ├─► Phase 2 (T005-T008): US1 verification → MVP!
    │
    └─► Phase 3 (T009-T032): US2 DashScope
            │
            └─► Phase 4 (T033-T036): US3 Test Infra
                    │
                    └─► Phase 5 (T037-T041): Polish
```

- **Phase 1 is the BLOCKER** — single file fix (`card.rs`), everything else depends on it
- **Phase 2 (US1)** is verification-only — can overlap with Phase 3 setup
- **Phase 4 (US3)** depends on Phase 3 (needs DashScope tests to refactor)

## Within Phase 3 (US2)

```
T009-T011 (scaffold) → T012 || T013 (params + formatter parallel)
    → T014-T022 (model implementation, sequential due to shared file)
    → T023-T032 (10 tests, all parallel)
```

## Parallel Opportunities

- Phase 3: T012 ∥ T013 (different files); T023-T032 all parallel (different test functions)
- Phase 4: T033 ∥ T034 (different functions in same file, can be written together)
- Phase 5: T037 ∥ T038 (different tools)

---

## Implementation Strategy

### MVP First

1. T001 → fix card.rs (the **ONLY** blocker)
2. T002-T004 → verify core compiles
3. T005-T008 → confirm US1 clean → **MVP!**

### Incremental

1. Phase 1: Fix blocker → core compiles
2. Phase 2: Verify US1 → MVP
3. Phase 3: DashScope Provider → 2nd deliverable
4. Phase 4: Test infra → 3rd deliverable
5. Phase 5: Polish → production ready

---

## Task Summary

| Phase | Story | Tasks | Notes |
|-------|-------|-------|-------|
| Phase 1 | — | T001-T004 (4) | 🔴 BLOCKER: card.rs fix |
| Phase 2 | US1 | T005-T008 (4) | Verification only (work already done) |
| Phase 3 | US2 | T009-T032 (24) | DashScope implementation + 10 tests |
| Phase 4 | US3 | T033-T036 (4) | Shared test helpers |
| Phase 5 | — | T037-T041 (5) | Polish & validation |
| **Total** | | **41** | |

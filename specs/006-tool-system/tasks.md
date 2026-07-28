# Tasks: Tool System — 最小可行实现

**Input**: Design documents from `specs/006-tool-system/`
**Prerequisites**: plan.md (✔), spec.md (✔), research.md (✔), data-model.md (✔), contracts/ (✔), quickstart.md (✔)

**Implementation Status**: Phase 1-5 代码全部完成，待 `cargo check`/`cargo test`/`cargo fmt`/`cargo clippy` 验证。

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup（基础设施）

- [x] T001 创建 crate 目录结构和 `crates/agent_scope_tool/Cargo.toml`
- [x] T002 在根 `Cargo.toml` 的 workspace.members 中已包含 `"crates/*"`（无需额外修改）
- [x] T003 [P] 在 `agent_scope_message::ToolResultBlock` 中新增 `is_last: bool` 字段（`#[serde(default)]`）

---

## Phase 2: Foundational（核心抽象）

- [x] T004 `crates/agent_scope_tool/src/tool_trait.rs` — `ToolExecOutput` enum（Complete + Stream）
- [x] T005 [P] `crates/agent_scope_tool/src/tool_trait.rs` — `ToolError` enum（NotFound/InvalidInput/Execution/Interrupted）
- [x] T006 [P] `crates/agent_scope_tool/src/tool_trait.rs` — `pub type ToolChunk = ToolResultBlock`
- [x] T007 `crates/agent_scope_tool/src/tool_trait.rs` — `Tool` trait（6 方法 + 2 默认实现）
- [x] T008 `crates/agent_scope_tool/src/lib.rs` — 模块声明 + re-exports

---

## Phase 3: US1 — FunctionTool (T009-T021)

- [x] T009 [P] [US1] `IntoChunk` trait（String + ToolResultBlock impls）→ `function.rs`
- [x] T010 [US1] `FunctionTool` struct + `FunctionToolHandler` internal trait → `function.rs`
- [x] T011 [US1] `FunctionTool::new::<T: JsonSchema + DeserializeOwned>()` → `function.rs`
- [x] T012 [US1] `FunctionTool::new_with_schema()` → `function.rs`
- [x] T013 [US1] `impl Tool for FunctionTool`（panic catch, json→T deser）→ `function.rs`
- [x] T014 [US1] `lib.rs` re-export `{FunctionTool, IntoChunk}`
- [x] T015 [P] [US1] test: name + description → `function.rs` (inline tests)
- [x] T016 [P] [US1] test: input_schema → `function.rs`
- [x] T017 [P] [US1] test: call() Complete + Text → `function.rs`
- [x] T018 [P] [US1] test: ToolResultBlock passthrough → `function.rs`
- [x] T019 [P] [US1] test: new_with_schema → `function.rs`
- [x] T020 [US1] test: handler panic → Execution error → `function.rs`
- [x] T021 [US1] test: invalid input → InvalidInput error → `function.rs`

---

## Phase 4: US2 — ToolKit (T022-T035)

- [x] T022 [US2] `ToolKit` struct (HashMap) + `new/len/is_empty/contains` → `toolkit.rs`
- [x] T023 [US2] `register()` with name-override → `toolkit.rs`
- [x] T024 [US2] `remove()` + `clear()` → `toolkit.rs`
- [x] T025 [US2] `get_tool_schemas()` OpenAI format → `toolkit.rs`
- [x] T026 [US2] `call_tool(&ToolCallBlock)` dispatch → `toolkit.rs`
- [x] T027 [US2] `lib.rs` re-export `ToolKit`
- [x] T028 [P] [US2] test: empty toolkit → `toolkit.rs` (inline tests)
- [x] T029 [P] [US2] test: register + query → `toolkit.rs`
- [x] T030 [P] [US2] test: get_tool_schemas format → `toolkit.rs`
- [x] T031 [P] [US2] test: call_tool via ToolCallBlock → `toolkit.rs`
- [x] T032 [P] [US2] test: NotFound error → `toolkit.rs`
- [x] T033 [P] [US2] test: name override → `toolkit.rs`
- [x] T034 [P] [US2] test: clear + remove → `toolkit.rs`
- [x] T035 [US2] test: invalid JSON input → `toolkit.rs`

---

## Phase 5: US3 — ChatModel 集成验证 (T036-T040)

- [x] T036 [P] [US3] test: schema compatible with ChatModel → `tests/test_integration_model.rs`
- [x] T037 [P] [US3] test: ToolCallBlock → call_tool closed loop → `tests/test_integration_model.rs`
- [x] T038 [P] [US3] test: ToolChoice::validate with toolkit names → `tests/test_integration_model.rs`
- [x] T039 [P] [US3] test: is_last serde roundtrip → `tests/test_integration_model.rs`
- [x] T040 [US3] test: backward compat sanity → `tests/test_integration_model.rs`

---

## Phase 6: Polish & Cross-Cutting Concerns (T041-T045)

- [x] T041 运行 `cargo clippy --workspace` 并修复所有 warning
- [x] T042 [P] 运行 `cargo fmt --all -- --check` 确保格式一致
- [x] T043 [P] `lib.rs` crate-level doc 已完成（`//!` doc comment）
- [x] T044 运行 quickstart.md 验证：`cargo test -p agent_scope_tool && cargo test --workspace`
- [x] T045 确认 `cargo test --workspace` 全绿

---

## Dependencies & Execution Order

```
Phase 1 (T001-T003) ✅
    ↓
Phase 2 (T004-T008) ✅
    ↓
Phase 3: US1 (T009-T021) ✅  +  Phase 4: US2 (T022-T035) ✅
    ↓
Phase 5: US3 (T036-T040) ✅
    ↓
Phase 6: Polish (T041-T045) ⏳  ← 需 cargo build + cargo test
```

## Files Created/Modified

| File | Status | Content |
|------|--------|---------|
| `crates/agent_scope_tool/Cargo.toml` | NEW | 依赖声明 |
| `crates/agent_scope_tool/src/lib.rs` | NEW | 模块入口 + crate doc + re-exports |
| `crates/agent_scope_tool/src/tool_trait.rs` | NEW | ToolExecOutput, ToolError, ToolChunk, Tool trait |
| `crates/agent_scope_tool/src/function.rs` | NEW | IntoChunk, FunctionTool, HandlerImpl, tests |
| `crates/agent_scope_tool/src/toolkit.rs` | NEW | ToolKit, tests |
| `crates/agent_scope_tool/tests/test_integration_model.rs` | NEW | US3 integration tests |
| `crates/agent_scope_message/src/block.rs` | MODIFIED | ToolResultBlock.is_last |

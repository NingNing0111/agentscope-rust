# Tasks: Skill Tool Integration

**Input**: Design documents from `/specs/013-skill-tool-integration/`

**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Included — feature SC-003 requires ≥85% test coverage. Tests are organized per user story phase.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Crate**: `crates/agent_scope_tool/`
- **Source**: `crates/agent_scope_tool/src/`
- **Tests**: `crates/agent_scope_tool/tests/`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependency configuration and module registration — unlocks all subsequent phases

- [x] T001 Add `agent_scope_workspace` dependency to `crates/agent_scope_tool/Cargo.toml` (for `Skill`, `SkillEntry`, `SkillsIndex` types)
- [x] T002 [P] Add `sha2` dependency to `crates/agent_scope_tool/Cargo.toml` (for content hash dedup)
- [x] T003 [P] Register new public modules `skill_loader` and `skill_viewer` in `crates/agent_scope_tool/src/lib.rs` and re-export key types (`SkillLoader`, `LocalSkillLoader`, `SkillOrLoader`, `SkillViewer`, `ListSkillsCallback`, `DEFAULT_SKILL_INSTRUCTION`)

---

## Phase 2: Foundational — Core Types & Implementations (Blocking Prerequisites)

**Purpose**: Core abstractions that ALL user stories depend on — MUST complete before US1-US3

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 [P] Define `SkillLoader` trait with `async fn list_skills(&self) -> Vec<Skill>` in `crates/agent_scope_tool/src/skill_loader.rs` (re-use `agent_scope_workspace::Skill`; `#[async_trait::async_trait]`, `Send + Sync` bounds)
- [x] T005 [P] Define `SkillOrLoader` enum with variants `Skill(Skill)`, `Loader(Box<dyn SkillLoader>)`, `Dir(String)` in `crates/agent_scope_tool/src/skill_loader.rs`
- [x] T006 Export and/or duplicate `parse_skill_md()` helper from `agent_scope_workspace::skill` into `crates/agent_scope_tool/src/skill_loader.rs` as `pub(crate)` (parse YAML frontmatter: name, description, body; handle missing `---` delimiters)
- [x] T007 Implement `LocalSkillLoader` struct (`directory: String`, `scan_subdir: bool`, `_cache: HashMap<String, Skill>`) with `new()` constructor in `crates/agent_scope_tool/src/skill_loader.rs`
- [x] T008 Implement `list_skills()` for `LocalSkillLoader`: scan directory for `SKILL.md` files, parse frontmatter via `parse_skill_md()`, skip invalid (log `tracing::warn!`), cache by `updated_at` mtime, return deduplicated list; handle: directory-not-found → empty vec, missing name/description → skip+warn
- [x] T009 [P] Add `ToolError::SkillNotFound { skill_name: String }` variant to `ToolError` enum in `crates/agent_scope_tool/src/tool_trait.rs` with `#[error("skill '{skill_name}' not found")]` derive

**Checkpoint**: Foundation ready — `SkillLoader` trait, `LocalSkillLoader` impl, and `SkillOrLoader` are compilable

---

## Phase 3: User Story 1 — Agent 通过 SkillViewer 工具获取 Skill 内容 (Priority: P1) 🎯 MVP

**Goal**: Agent can call `Skill` tool with a skill name and receive markdown content

**Independent Test**: Create a SkillViewer with a mock callback returning a known Skill, call with valid and invalid names, verify ToolExecOutput content

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T010 [P] [US1] Test SkillViewer returns markdown for known skill in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — setup: mock callback returning `HashMap<"test", Skill { markdown: "# Hello" }>`, call with `{"skill": "test"}`, assert `ToolExecOutput::Complete` with text `"# Hello"` and `state: Success`
- [x] T011 [P] [US1] Test SkillViewer returns error ToolChunk for unknown skill in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — mock callback returning empty map, call with `{"skill": "unknown"}`, assert `state: Error` with text containing `"SkillNotFoundError"`
- [x] T012 [P] [US1] Test SkillViewer callback exception is caught in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — mock callback that panics, assert returned `ToolChunk` has `state: Error` (not propagated)

### Implementation for User Story 1

- [x] T013 [US1] Define `ListSkillsCallback` type alias (`Box<dyn Fn(&[String]) -> HashMap<String, Skill> + Send + Sync>`) in `crates/agent_scope_tool/src/skill_viewer.rs`
- [x] T014 [US1] Implement `SkillViewer` struct (field: `_get_skills_method: ListSkillsCallback`) with `new()` constructor in `crates/agent_scope_tool/src/skill_viewer.rs`
- [x] T015 [US1] Implement `Tool` trait for `SkillViewer`: `name()` → `"Skill"`, `description()` → Python-aligned text, `input_schema()` → `{"type":"object","properties":{"skill":{"type":"string"}},"required":["skill"]}`, `is_read_only()` → `true`, `is_concurrency_safe()` → `true`
- [x] T016 [US1] Implement `Tool::call()` for `SkillViewer`: extract `"skill"` string from input JSON; invoke `_get_skills_method` with empty activated groups; look up name in map; on found return `Ok(Complete(TextBlock { text: skill.markdown, state: Success }))`; on not-found return `Ok(Complete(TextBlock { text: "SkillNotFoundError: Skill '<name>' not found.", state: Error }))`; catch callback panics via `std::panic::catch_unwind`
- [x] T017 [US1] Add `tracing::info!` on successful skill view and `tracing::warn!` on not-found in `SkillViewer::call()`

**Checkpoint**: US1 independently testable — mock callback + SkillViewer → markdown output; 3 tests pass

---

## Phase 4: User Story 2 — 开发者注册 Skill 到 ToolKit/ToolGroup (Priority: P2)

**Goal**: Developer can register skill dirs/objects/loaders into ToolKit, and skills are discoverable via `list_skills()`

**Independent Test**: Create ToolKit, call `add_skill_dir()` with a temp dir containing SKILL.md, call `list_skills()` and verify returned Skill object

### Tests for User Story 2

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T018 [P] [US2] Test `ToolKit::add_skill_dir()` registers a skill from directory in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — create temp dir with SKILL.md, add to fresh ToolKit, call `list_skills()`, assert len=1 and name matches
- [x] T019 [P] [US2] Test `ToolKit::add_skill_dir()` error on missing SKILL.md in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — add empty temp dir, assert returns `Err` or silently skips
- [x] T020 [P] [US2] Test adding two skills from different dirs in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — add dir A (skill-a) and dir B (skill-b), assert `list_skills()` returns 2 skills
- [x] T021 [P] [US2] Test ToolKit registers `SkillViewer` automatically in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — create new ToolKit, call `get_tool_schemas()`, assert contains tool with name `"Skill"`
- [x] T022 [P] [US2] Test duplicate skill name deduplication in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — add two skills with same name, assert `list_skills()` returns 1, first wins

### Implementation for User Story 2

- [x] T023 [US2] Add `skills_or_loaders: Vec<SkillOrLoader>` field to any `ToolGroup`-equivalent struct in `crates/agent_scope_tool/src/toolkit.rs` (if ToolGroup doesn't exist yet, add a minimal `ToolGroup` struct with `name`, `description`, `tools`, `skills_or_loaders` fields; or integrate directly into `ToolKit` with a default "basic" group concept)
- [x] T024 [US2] Implement `async fn list_skills(&self) -> Vec<Skill>` method on ToolKit: iterate all tool groups' `skills_or_loaders`, expand `Skill` directly, call `list_skills()` on `Loader`, wrap `Dir` as `LocalSkillLoader { scan_subdir: true }` and call `list_skills()`; dedup by name (first-seen wins, log `tracing::warn!` on duplicate)
- [x] T025 [US2] Implement `fn add_skill_dir(&mut self, path: &str)` on ToolKit in `crates/agent_scope_tool/src/toolkit.rs` — delegate to `add_skill_loader()` with `SkillOrLoader::Dir(path.into())`
- [x] T026 [US2] Implement `fn add_skill(&mut self, skill: Skill)` on ToolKit in `crates/agent_scope_tool/src/toolkit.rs` — delegate to default group's `skills_or_loaders.push(SkillOrLoader::Skill(skill))`
- [x] T027 [US2] Implement `fn add_skill_loader(&mut self, loader: Box<dyn SkillLoader>)` on ToolKit in `crates/agent_scope_tool/src/toolkit.rs` — delegate to default group's `skills_or_loaders.push(SkillOrLoader::Loader(loader))`
- [x] T028 [US2] Auto-register `SkillViewer` on `ToolKit::new()`: create a `SkillViewer` with callback `|groups| { self.list_skills_as_map(groups) }`, register it in the default tool group in `crates/agent_scope_tool/src/toolkit.rs`
- [x] T029 [US2] Implement private helper `fn list_skills_as_map(&self, activated_groups: &[String]) -> HashMap<String, Skill>` on ToolKit — collects from groups matching `activated_groups` (or all groups if empty), dedups by name

**Checkpoint**: US2 independently testable — ToolKit with skill registration, list_skills, and auto-registered SkillViewer; 5 tests pass

---

## Phase 5: User Story 4 — 独立 SkillLoader 扫描和缓存 (Priority: P2)

**Goal**: LocalSkillLoader correctly handles edge cases: caching, concurrency, missing directories, partial failures

**Independent Test**: Create LocalSkillLoader pointing to a directory with multiple subdirs containing SKILL.md, verify all are loaded, modify one file and re-scan to verify cache update

### Tests for User Story 4

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T030 [P] [US4] Test LocalSkillLoader `scan_subdir=true` loads skills from subdirectories in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — create 2 subdirs each with SKILL.md, assert 2 skills returned
- [x] T031 [P] [US4] Test LocalSkillLoader cache — second scan returns cached results (same `updated_at`) in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — scan once, modify one file, scan again, verify only modified file re-read
- [x] T032 [P] [US4] Test LocalSkillLoader `scan_subdir=false` only checks root in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — put SKILL.md in root, create subdir with another SKILL.md, assert only root skill returned
- [x] T033 [P] [US4] Test LocalSkillLoader missing `name` field in frontmatter is skipped with warning in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — SKILL.md with `description` but no `name`, assert empty result
- [x] T034 [P] [US4] Test LocalSkillLoader directory not exists returns empty list in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — point to nonexistent dir, assert empty vec (no error)
- [x] T035 [P] [US4] Test LocalSkillLoader malformed frontmatter is gracefully skipped in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — SKILL.md with invalid YAML (no `---` end delimiter), assert not in results
- [x] T036 [P] [US4] Test LocalSkillLoader empty markdown body is accepted in `crates/agent_scope_tool/tests/skill_loader_tests.rs` — SKILL.md with frontmatter but no body, assert returned with empty markdown

### Implementation for User Story 4

- [x] T037 [US4] Refine `LocalSkillLoader::list_skills()` for `scan_subdir=true`: use `std::fs::read_dir` or `walkdir` (if added as dep) to recursively find all dirs containing `SKILL.md` in `crates/agent_scope_tool/src/skill_loader.rs`
- [x] T038 [US4] Implement mtime-based cache in `LocalSkillLoader`: on each scan, check `std::fs::metadata(path)?.modified()?` against cached `updated_at`; only re-read if mtime changed; evict entries for deleted dirs
- [x] T039 [US4] Add `tracing::warn!` for: missing name/description, frontmatter parse failure, directory not found, individual file read error (continue loading others)

**Checkpoint**: US4 independently testable — 7 edge-case tests pass; LocalSkillLoader handles all failure modes gracefully

---

## Phase 6: User Story 3 — Agent System Prompt 中包含可用 Skill 列表 (Priority: P3)

**Goal**: System prompt includes `<agent-skills>` XML block listing available skills, guiding Agent to use `Skill` tool

**Independent Test**: Register 2 skills, call `get_skill_instructions()`, verify output contains XML with correct skill names/descriptions

### Tests for User Story 3

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T040 [P] [US3] Test `get_skill_instructions()` with registered skills produces `<agent-skills>` XML in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — register 2 skills, call method, assert output contains `<agent-skills>`, `<skill>`, `<name>`, `<description>`, `<dir>` tags for both
- [x] T041 [P] [US3] Test `get_skill_instructions()` empty when no skills registered in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — fresh ToolKit, assert returns empty string `""`
- [x] T042 [P] [US3] Test `get_skill_instructions()` with custom template in `crates/agent_scope_tool/tests/skill_viewer_tests.rs` — provide custom template string, assert custom text appears in output

### Implementation for User Story 3

- [x] T043 [US3] Define `DEFAULT_SKILL_INSTRUCTION` constant in `crates/agent_scope_tool/src/skill_viewer.rs` — XML template with `<agent-skills>` wrapper, explanation text "Skills are NOT tools", placeholders `{skill_viewer}` and `{skills_list}`
- [x] T044 [US3] Implement `fn get_skill_instructions(&self, template: Option<&str>) -> String` on ToolKit in `crates/agent_scope_tool/src/toolkit.rs` — if no skills registered return `""`; render `{skill_viewer}` → `"Skill"`; render `{skills_list}` by iterating `list_skills()` and formatting each as `<skill><name>...</name><description>...</description><dir>...</dir></skill>`
- [x] T045 [US3] Wire `get_skill_instructions()` return value into an accessible public API — ensure it can be called externally to inject into Agent system prompt

**Checkpoint**: US3 independently testable — prompt generation works with 0, 1, and N skills; custom templates supported; 3 tests pass

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Final quality assurance, integration validation, and documentation

- [x] T046 [P] Run `cargo test --workspace` — all existing tests must still pass (no regression)
- [x] T047 [P] Run `cargo clippy -p agent_scope_tool -- -D warnings` — 0 warnings
- [x] T048 [P] Run `cargo fmt --check --all` — clean formatting
- [x] T049 [P] Verify `SkillViewer` tool appears in `ToolKit::get_tool_schemas()` output alongside existing tools
- [x] T050 Review and update `crates/agent_scope_tool/src/lib.rs` module-level docs to describe new skill capabilities
- [x] T051 Validate quickstart.md scenarios 1-5 manually with generated test binaries
- [x] T052 [P] Add `#![deny(unsafe_code)]` to `skill_loader.rs` and `skill_viewer.rs` if not already covered by crate-level attribute

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (T001-T003) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Phase 2 (T004-T009) — requires `SkillLoader` trait and `Skill` type available
- **User Story 2 (Phase 4)**: Depends on Phase 3 (SkillViewer must exist for auto-registration in ToolKit)
- **User Story 4 (Phase 5)**: Depends on Phase 2 (LocalSkillLoader already defined); tests run after Phase 4 integration
- **User Story 3 (Phase 6)**: Depends on Phase 4 (needs `list_skills()` from ToolKit)
- **Polish (Phase 7)**: Depends on all user stories complete

### User Story Dependencies

```
Phase 1 (Setup)
    │
    ▼
Phase 2 (Foundational)
    │
    ├────► Phase 3 (US1: SkillViewer) ────► Phase 4 (US2: Toolkit) ───► Phase 6 (US3: Prompt)
    │                                            │
    └────► Phase 5 (US4: Loader tests) ◄─────────┘  (US4 tests validate Loader with ToolKit integration)
```

- **US1 (P1)**: Independent after Phase 2 — pure Tool implementation
- **US2 (P2)**: Depends on US1 (SkillViewer struct) — ToolKit wraps SkillViewer
- **US4 (P2)**: Depends on Phase 2 — tests validate LocalSkillLoader edge cases
- **US3 (P3)**: Depends on US2 (needs ToolKit::list_skills)

### Within Each Phase

- Tests MUST be written and FAIL before implementation
- Models/types before methods
- Core implementation before integration
- Tracing/logging after implementation

### Parallel Opportunities

| Phase | Parallel Tasks |
|-------|---------------|
| Phase 1 | T002 ∥ T003 (after T001) |
| Phase 2 | T004 ∥ T005 (after T001+T002+T003); T006 ∥ T009 (different files) |
| Phase 3 | T010 ∥ T011 ∥ T012 (all tests, different test functions) |
| Phase 4 | T018 ∥ T019 ∥ T020 ∥ T021 ∥ T022 (all tests) |
| Phase 5 | T030 ∥ T031 ∥ T032 ∥ T033 ∥ T034 ∥ T035 ∥ T036 (all tests, different test functions) |
| Phase 6 | T040 ∥ T041 ∥ T042 (all tests) |
| Phase 7 | T046 ∥ T047 ∥ T048 ∥ T049 ∥ T052 (independent validations) |

---

## Parallel Examples

### Phase 2 — Foundational (after Setup)

```bash
# These can run in parallel (different declarations):
Task: "Define SkillLoader trait in skill_loader.rs" (T004)
Task: "Define SkillOrLoader enum in skill_loader.rs" (T005)
Task: "Add ToolError::SkillNotFound to tool_trait.rs" (T009)
```

### Phase 3 — US1 MVP

```bash
# Launch all tests together (after T013-T017 implementation):
Task: "Test SkillViewer returns markdown" (T010)
Task: "Test SkillViewer not-found error" (T011)
Task: "Test SkillViewer callback panic caught" (T012)
```

### Phase 4 — US2 + Phase 5 — US4

```bash
# US4 tests can run in parallel with US2 implementation (different test files):
Task: "US2: Add skills_or_loaders to ToolKit" (T023)
# Meanwhile, US4 tests are being written:
Task: "US4: Test scan_subdir=true" (T030)
Task: "US4: Test cache behavior" (T031)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (3 tasks)
2. Complete Phase 2: Foundational (6 tasks) — **CRITICAL: blocks all stories**
3. Complete Phase 3: User Story 1 (8 tasks — 3 tests + 5 impl)
4. **STOP and VALIDATE**: Run `cargo test -p agent_scope_tool -- skill_viewer`, verify 3 US1 tests pass
5. Demonstrate: SkillViewer Tool registered in ToolKit, Agent can call it

**MVP task count**: 17 tasks (Phases 1-3)

### Incremental Delivery

1. Phases 1-2 → Foundation ready (9 tasks)
2. + Phase 3 (US1) → SkillViewer works → **MVP!** (17 tasks)
3. + Phase 4 (US2) → ToolKit skill registration works (12 tasks, 33 total)
4. + Phase 5 (US4) → Loader edge cases handled (10 tasks, 43 total)
5. + Phase 6 (US3) → System prompt injection works (6 tasks, 49 total)
6. + Phase 7 → Polish & validation (7 tasks, **52 total**)

### File Change Summary

| File | Action | Tasks |
|------|--------|-------|
| `Cargo.toml` | MODIFY | T001, T002 |
| `src/lib.rs` | MODIFY | T003, T050 |
| `src/skill_loader.rs` | **NEW** | T004, T005, T006, T007, T008, T037, T038, T039 |
| `src/skill_viewer.rs` | **NEW** | T013, T014, T015, T016, T017, T043 |
| `src/toolkit.rs` | MODIFY | T023, T024, T025, T026, T027, T028, T029, T044, T045 |
| `src/tool_trait.rs` | MODIFY | T009 |
| `tests/skill_viewer_tests.rs` | **NEW** | T010, T011, T012, T018, T019, T020, T021, T022, T040, T041, T042 |
| `tests/skill_loader_tests.rs` | **NEW** | T030, T031, T032, T033, T034, T035, T036 |

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story is independently completable and testable
- Verify tests fail before implementing
- Commit after each phase checkpoint
- Stop at any checkpoint to validate story independently
- `tracing` macros used throughout (no println!)
- `#![deny(unsafe_code)]` already active at crate level; all new code must be safe Rust
- `parse_skill_md()` from `agent_scope_workspace::skill` is `pub(crate)` — either re-export as `pub` in workspace or duplicate the implementation in tool crate

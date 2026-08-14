# Implementation Plan: Dependency Simplification

**Branch**: `028-dependency-simplification` | **Date**: 2026-08-11 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/028-dependency-simplification/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Optimize AgentScope Rust by identifying custom basic implementations that can be replaced or consolidated with mature third-party crates while preserving externally observable behavior. The technical approach is inventory-first: classify at least 10 candidates, adopt only low-risk commodity replacements with explicit dependency evaluations and compatibility evidence, and reject/defer changes that touch AgentScope protocol, serialization, provider behavior, persisted state, or security boundaries.

The recommended first implementation batch is:

1. shared YAML/frontmatter parsing for `SKILL.md`,
2. memory markdown frontmatter parsing/serialization behind compatibility tests,
3. pi-rust file discovery simplification using glob/traversal crates,
4. remaining typed error enums migrated to `thiserror` where display/source behavior can remain stable.

## Technical Context

**Language/Version**: Rust 2024 edition workspace

**Primary Dependencies**: Existing workspace dependencies include `serde`, `serde_json`, `uuid`, `chrono`, `chrono-tz`, `base64`, `schemars`, and `clap`. Candidate dependency families for implementation tasks include YAML/frontmatter parsers (`serde_yaml`, `gray_matter`, or `yaml-front-matter`), file discovery crates (`globset`, `walkdir`, or `ignore`), error derivation (`thiserror`), filename/slug helpers (`sanitize-filename`, `slug`, `slugify`, or `deunicode`), and lightweight cache helpers (`indexmap` or `lru`). Exact crate choices require dependency evaluation before implementation.

**Storage**: File-based markdown memory files, JSON session state files, workspace/offload paths, and example session files. Persistent file formats and existing path mappings must remain compatible unless a later spec approves migration.

**Testing**: `cargo test` across workspace and targeted crate tests; `cargo clippy`; `cargo fmt --check`; golden/round-trip tests for parsing and persistence; example/tool behavior tests for pi-rust where applicable.

**Target Platform**: Cross-platform Rust library and CLI/example workspace, with filesystem behavior that must remain safe on Unix-like and Windows path semantics where applicable.

**Project Type**: Rust workspace containing library crates (`crates/*`) and a CLI/demo example (`examples/pi-rust`).

**Performance Goals**: Simplifications must not regress bounded file scanning, streaming/event behavior, or persistence safety. File discovery replacements should maintain or improve large-tree behavior while preserving current scan/result caps.

**Constraints**: Compatibility with the locked AgentScope Python behavior remains higher priority than code reduction. No public API semantics, serialized data shape, event ordering, cancellation behavior, error classification, example behavior, or security boundary may change silently. New dependencies must be license-compatible with Apache-2.0 distribution, maintained, security-reviewed, and aligned with crate dependency direction.

**Scale/Scope**: Planning inventory covers at least 10 candidates across `crates/*` and `examples/pi-rust`; implementation should select a small first batch of at least 3 approved low-risk simplifications or document why candidates are unsafe/unsuitable.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Research Gate

- **Python baseline and compatibility first**: PASS. The plan treats compatibility semantics as non-negotiable and rejects replacing AgentScope event/message protocols.
- **Upstream version lock and no pseudo-compatibility**: PASS. Dependency adoption requires behavior evidence and forbids silent fallback or unsupported behavior disguised as success.
- **Spec before implementation**: PASS. This feature is defined in `spec.md`; implementation tasks must be generated later from planning artifacts.
- **Test-driven compatibility**: PASS. Adopted candidates require golden, round-trip, unit, integration, or example evidence mapped to public behavior.
- **Security Rust first / unsafe prohibited by default**: PASS. Dependency evaluations must review unsafe exposure and security posture; sandbox/path containment replacement is rejected.
- **Layering and dependency direction**: PASS. Dependency evaluations must verify no core/provider/infra coupling or crate cycle is introduced.
- **Stable data and error protocols**: PASS. Public data protocols are rejected as replacement targets; error migrations must preserve typed enum variants and display/source semantics.
- **Performance does not override correctness**: PASS. File discovery improvements must preserve caps and safety behavior rather than merely improve speed.
- **Small-step delivery and clear Definition of Done**: PASS. Plan recommends a small first batch and explicit validation before completion.

No constitution violations require complexity justification.

### Post-Design Gate

- **Research resolved technical unknowns**: PASS. `research.md` classifies candidates and records alternatives.
- **Design artifacts define review records**: PASS. `data-model.md` defines candidate, dependency evaluation, and behavior evidence records.
- **Contracts protect dependency governance**: PASS. `contracts/dependency-evaluation-contract.md` defines the required dependency review shape.
- **Quickstart maps validation commands and expected outcomes**: PASS. `quickstart.md` documents planning and implementation validation scenarios with RTK-prefixed commands.
- **No unresolved `NEEDS CLARIFICATION` markers**: PASS. The plan and generated artifacts contain no unresolved clarification markers.

## Project Structure

### Documentation (this feature)

```text
specs/028-dependency-simplification/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── dependency-evaluation-contract.md
├── checklists/
│   └── requirements.md
└── tasks.md              # Phase 2 output from /speckit-tasks; not created by /speckit-plan
```

### Source Code (repository root)

```text
Cargo.toml                         # Workspace dependency governance
crates/
├── agent_scope_agent/             # Runtime injection and typed agent errors
├── agent_scope_dashscope/         # Provider streaming/SSE behavior; defer broad replacement
├── agent_scope_embedding/         # Cache key/path component sanitization candidate
├── agent_scope_event/             # Event protocol; reject replacement
├── agent_scope_mcp/               # MCP adapter layer; defer deeper model replacement
├── agent_scope_memory/            # Memory markdown frontmatter candidate
├── agent_scope_message/           # Message/content protocol; reject replacement
├── agent_scope_model/             # Model errors, retry loop, JSON repair/schema flatten candidates
├── agent_scope_rag/               # RAG/tool-name sanitization and chunking candidates
├── agent_scope_sandbox/           # Path containment security boundary; reject replacement
├── agent_scope_state/             # Session store and ToolContext cache candidates
├── agent_scope_tool/              # Skill frontmatter and JSON repair candidates
└── agent_scope_workspace/         # Skill frontmatter, workspace tools, containment-facing behavior

examples/
└── pi-rust/
    └── src/                       # Glob/Grep/ListDir and session/path sanitization candidates
```

**Structure Decision**: This is a Rust workspace optimization feature. Planning artifacts live under `specs/028-dependency-simplification/`; implementation tasks will touch selected crates/examples only after dependency evaluations and behavior evidence are attached. No new runtime service, API endpoint, or storage backend is introduced by planning.

## Complexity Tracking

No constitution violations are introduced by this plan. Complexity is deliberately constrained by rejecting or deferring replacements in protocol, provider-streaming, persisted-state, and sandbox security areas unless a later spec provides a stronger compatibility and migration design.

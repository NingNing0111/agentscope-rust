# Research: Pi Coding Agent (Rust)

**Feature**: 023-pi-coding-agent
**Created**: 2026-08-02
**Phase**: 0 — Outline & Research

## Decision 1: Default runtime architecture

**Decision**: Build pi-rust as a standalone CLI example under `examples/pi-rust/`, backed by agentscope-rust framework crates. The CLI owns argument parsing, REPL, rendering, and session persistence; framework crates own Agent loop, model calls, tools, memory, workspace, RAG, and events.

**Rationale**:

- The user explicitly requested `examples/pi-rust` as the Rust implementation location.
- `examples/pi-rust` is already a workspace member and currently only contains a placeholder `main.rs`.
- This keeps pi-rust independent from `pi-ts` while avoiding duplicate implementations of AgentScope primitives already provided by this repository.
- Example-level code can evolve quickly without destabilizing public framework APIs.

**Alternatives considered**:

- Create a new workspace crate under `crates/`: rejected because this is a runnable example/application, not a reusable framework module.
- Port pi-ts package structure directly: rejected because it would mechanically copy TypeScript architecture and violate Rust-native design goals.
- Extend `examples/agent-demo`: rejected because pi-rust has a different user-facing goal and should remain a separate example.

## Decision 2: Agent orchestration model

**Decision**: Use `ReActAgent` as the default orchestration model for normal turns. Planner and SubAgent modes are not part of the initial pi-rust MVP.

**Rationale**:

- The core feature is a coding assistant that can decide when to read, edit, write, or run commands.
- ReAct fits this need directly: reason, call tool, observe result, continue or answer.
- Planner/SubAgent support increases implementation and validation surface and is already demonstrated in `examples/agent-demo`; pi-rust should first deliver a focused coding-agent baseline.

**Alternatives considered**:

- Planner-first execution: rejected for MVP because coding turns are often short and tool-driven; forcing planning for every turn adds latency and complexity.
- Multi-agent team mode: deferred because spec assumes single-user, single-agent CLI for initial version.
- Pure chat mode: rejected because file and shell tools are core requirements.

## Decision 3: Tool set and permissions

**Decision**: Expose four first-class coding tools: `Read`, `Write`, `Edit`, and `Bash`. Use a permission context that allows low-risk reads by default and requires confirmation for potentially destructive Bash or file overwrite operations.

**Rationale**:

- These four tools cover the essential coding-agent loop: inspect, modify, create, verify.
- The spec explicitly requires file operations and shell command execution.
- Confirmation for risky operations is user-visible behavior and prevents accidental destructive changes.
- Tool names intentionally mirror common coding-agent vocabulary so model prompts can stay direct.

**Alternatives considered**:

- Reuse only `LocalWorkspace` tools: rejected because pi-rust needs a coding-agent-specific UX and contract with familiar tool names.
- Add many specialized tools immediately (`Grep`, `Glob`, `TodoWrite`, etc.): deferred to keep MVP bounded.
- Allow all Bash commands without confirmation: rejected for safety.

## Decision 4: Session persistence

**Decision**: Persist sessions as JSON files under `<workdir>/sessions/`, with one file per session containing metadata and serialized conversation turns.

**Rationale**:

- File storage is simple, portable, inspectable, and sufficient for a local CLI example.
- It does not require additional infrastructure.
- JSON contracts are easy to test with round-trip serialization.
- This aligns with current FileMemory-style storage patterns in the project.

**Alternatives considered**:

- SQLite: rejected for MVP due to extra dependency and migration overhead.
- In-memory only: rejected because session recovery is a P2 requirement.
- Reuse memory store for full sessions: rejected because long-term memories and full transcript persistence have different retention and retrieval semantics.

## Decision 5: Context management

**Decision**: Keep recent conversation turns in Agent context and persist full turns to session storage. Long-term stable facts go through MemoryMiddleware. Context compaction is deferred to the framework layer and triggered when context approaches configured limits.

**Rationale**:

- Recent-turn context is needed for natural follow-ups.
- Full persisted turns enable recovery.
- MemoryMiddleware should store stable facts, not every transcript line.
- Compaction is a framework capability and should not be reimplemented in pi-rust.

**Alternatives considered**:

- Store all conversation text in Memory: rejected because memory search would be polluted by transient turns.
- No compaction: rejected because long sessions can exceed model context windows.
- Eager summarize every turn: rejected because it may lose exact edit/tool details needed for coding tasks.

## Decision 6: Provider support

**Decision**: Use DashScope as the default provider and allow model name/API key configuration through CLI args and environment variables. Additional providers are contractually allowed but deferred unless supported by existing framework crates.

**Rationale**:

- Existing examples and project history already use DashScope as the real provider path.
- The feature can be tested with existing `agent_scope_dashscope` integration.
- The spec says default DashScope is acceptable for initial version.
- Avoids creating new provider abstractions inside example code.

**Alternatives considered**:

- Implement OpenAI-compatible provider in pi-rust: rejected because provider abstractions belong in framework crates.
- Hard-code model/API key: rejected because users need runtime configuration.
- Mock-only provider: rejected because the CLI must work as a real coding assistant.

## Decision 7: Validation strategy

**Decision**: Use deterministic tests for tools/session serialization and mock-model integration tests for ReAct tool flow. Use real DashScope only for manual quickstart validation.

**Rationale**:

- Real LLM outputs are nondeterministic and should not be the sole correctness gate.
- Tool and session behavior can be validated deterministically.
- Mock-model flows can assert expected tool call sequences and final messages.
- Manual real-provider quickstart proves the runnable example works end-to-end.

**Alternatives considered**:

- CI with real provider API: rejected because it depends on credentials, network, and model behavior.
- Only compile checks: rejected because core behaviors would remain unverified.
- Snapshot real LLM output: rejected due to provider drift and flakiness.

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Compatibility baseline: AgentScope Python v2.0.5 (commit `27b6a0d2a2afedf53462c9a2add33932d54b2d20`)

## [0.1.0] — 2026-08-02

### Added

#### Foundation Layer
- **Compatibility Baseline** (Feature 001): Python golden-snapshot test infrastructure, capability matrix, trace schema, JSON fixtures
- **Message & ContentBlock Model** (Feature 002): `Msg`, `ContentBlock`, `TextBlock`, `ThinkingBlock`, `ToolCallBlock`, `ToolResultBlock`, factory functions, serialization round-trip tests
- **Event System** (Feature 002): 27 event types covering reply lifecycle, model calls, streaming blocks, tool execution, user interaction, and control events
- **Type Definitions** (Feature 002): `ErrorInfo`, `ErrorType`, `ReplyFinishedReason`, `JsonValue`, `Embedding` type alias

#### Model & Provider
- **Model API** (Feature 003): `ChatModel` trait, `ChatResponse`, `StreamAccumulator`, `Formatter`, `ModelCard`, `ToolChoice`, `ChatUsage`, `ModelError`, structured output support
- **Provider Architecture** (Feature 004): Pluggable provider design with `DashScopeFormatter` separation
- **DashScope Provider** (Feature 005): `DashScopeChatModel`, `DashScopeEmbeddingModel`, Qwen/Model Studio models via OpenAI-compatible API, streaming SSE support

#### Tool System
- **Tool System** (Feature 006): `Tool` trait, `FunctionTool` adapter (auto schema generation via `schemars`), `ToolKit` registry with OpenAI-compatible schema output
- **Skill Tool Integration** (Feature 013): `SkillLoader`, `SkillViewer`, `LocalSkillLoader`, skill-to-tool conversion pipeline

#### Agent System
- **Agent System** (Feature 007): `Agent` trait, `ReActAgent` implementation, `Middleware` trait (8 hook points), event emission, permission checking, interruption handling
- **Streaming Infrastructure** (Feature 008): `reply_stream()`, `AgentEvent` stream, `StreamingReactor`, tool-call streaming events, thinking block streaming
- **End Event Content** (Feature 014): Accumulated content in streaming end events for easier rendering

#### Memory & State
- **Memory System** (Feature 009): `Memory` trait, `FileMemory` (Markdown + frontmatter), `MEMORY.md` index, `MemoryMiddleware`, search and relevant-memory retrieval
- **Session Management** (Feature 010): `Session`, `SessionStore`, `InMemorySessionStore`, `AgentState`, `ReplyContext`, context trimming (`TokenCounter`, `TrimStrategy`)
- **TurboVec RAG** (Feature 016): `TurboVecStore`, vector-based knowledge retrieval with `turbovec` backend
- **TurboVec Long-term Memory** (Feature 022): `TurbovecMemory`, `MemoryVectorIndex`, persistent vector-based long-term memory with rebuild reports

#### RAG System
- **RAG System** (Feature 011): `Parser` (text/Markdown), `Chunker` (fixed-size, paragraph-aware), `VectorStore` trait, `KnowledgeBase`, `RAGMiddleware`, `TurbovecMemoryAdapter`

#### Workspace & Sandbox
- **Workspace Management** (Feature 012): `WorkspaceBase` trait, `LocalWorkspace`, file I/O tools, MCP client config (`McpClientConfig`, `McpTransportConfig`), skill management, context offloading
- **Sandbox** (Feature 017): `SandboxSession` trait, `LocalSandboxSession` reference implementation, path traversal prevention, command execution with timeouts, explicit capability reporting

#### Agent Extensions
- **Planner + ReActAgent** (Feature 021): `Planner`, `Plan`, `PlanStep`, `PlanningTrace`, plan-based agent orchestration with streaming plan events
- **SubAgent** (Feature 020): `SubAgent`, delegation patterns, subagent lifecycle management
- **Agent Task Planning** (Feature 024): built-in task planning tools (`TaskCreate`/`TaskList`/`TaskGet`/`TaskUpdate`) + unfinished-task reminder injection, replacing the standalone Planner
- **Agent State Persistence** (Feature 025): session store (`JsonFileSessionStore`), agent-state save/load round-trip, auto-persist after reply
- **Runtime State Injection** (Feature 026): unified `_inject_runtime_state` pipeline — time / unfinished-task / context-length dimensions in a single `HintBlock`, `InjectionConfig` full configuration (timezone, time format, template, source, task tool names, extra fields, hint event emission), `HintBlockEvent` emission, IANA timezone support via `chrono-tz`

#### Documentation & Examples
- **Usage Docs** (Feature 018): Chinese and English module documentation, getting-started guide
- **Agent Demo** (Feature 019): `agent_demo` example with real DashScope ReActAgent, streaming events, tools, skills, memory, workspace, RAG, and permission denial
- **Integration API Tests** (Feature 015): Cross-crate integration examples

### Compatibility
- All crates follow AgentScope Python v2.0.5 as behavioral reference
- Compatibility levels: L1 (protocol), L2 (core behavior), L3 (API semantics), L4 (example migration)
- Golden snapshot fixtures and diff-test infrastructure in `tests/compatibility/`
- Upstream Python reference repo pinned in `agentscope/` directory

### Engineering
- **Zero `unsafe` code**: All 14 crates guard with `#![deny(unsafe_code)]`
- **722 tests**: Unit tests, integration tests, doc-tests, round-trip serialization tests
- **Structured tracing**: `tracing` spans/events for all critical paths
- **Structured concurrency**: `CancellationToken`, bounded channels, explicit task ownership
- **Rust 2024 edition**: All crates on edition 2024

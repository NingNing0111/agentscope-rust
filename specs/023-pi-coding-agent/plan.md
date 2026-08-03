# Implementation Plan: Pi Coding Agent (Rust)

**Branch**: `023-pi-coding-agent` | **Date**: 2026-08-02 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/023-pi-coding-agent/spec.md`

## Summary

在 `examples/pi-rust/` 下构建一个功能等价的 Rust 编码 Agent CLI 工具。它基于 agentscope-rust 框架已有的 ReActAgent、MemoryMiddleware、LocalWorkspace、RAGMiddleware 等基础设施，提供交互式编码助手能力——读取/编辑文件、执行 shell 命令、流式对话、会话持久化。pi-ts（TypeScript 版本）仅作为功能参考，不依赖其任何代码。

## Technical Context

**Language/Version**: Rust edition 2024 (workspace), MSRV: 1.85+

**Primary Dependencies**: agent_scope_agent (ReActAgent, Agent), agent_scope_dashscope (LLM provider), agent_scope_tool (FunctionTool, ToolKit), agent_scope_message (Msg construction), agent_scope_memory (FileMemory, MemoryMiddleware), agent_scope_workspace (LocalWorkspace, Skill), agent_scope_rag (RAGMiddleware, KnowledgeBase, TurbovecVectorStore), agent_scope_embedding (EmbeddingModelCard), agent_scope_event (AgentEvent streaming), clap (CLI args), tokio (async runtime), serde/serde_json (serialization)

**Storage**: File-based — FileMemory stores memories under `<workdir>/Memory/`, LocalWorkspace manages files under `<workdir>/workspace/`, session data serialized as JSON under `<workdir>/sessions/`

**Testing**: `cargo test` — unit tests for tool implementations, integration tests for agent loop with mock models

**Target Platform**: macOS (primary dev), Linux (CI + production), Windows (best-effort)

**Project Type**: CLI application (workspace member under `examples/pi-rust/`)

**Performance Goals**: CLI startup <1s (excluding LLM API), streaming response first-byte <3s (dependent on LLM provider), session persistence save <500ms

**Constraints**: Single-agent REPL mode initially (multi-agent out of scope for this feature), DashScope as default provider, must not import or reference pi-ts code

**Scale/Scope**: Single user per CLI instance, ~100 messages per session before compaction, 1-2 concurrent tool calls per turn

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| **I. 兼容性优先** | N/A | 本项目是对 pi-ts 的功能等价实现，不是 AgentScope Python 的兼容实现。pi-ts 作为功能参考，不要求行为兼容性。 |
| **II. 锁定上游版本** | N/A | pi-ts 没有锁定版本要求。 |
| **III. Python AgentScope 是行为基准** | N/A | 同上。 |
| **IV. 先定义契约，再实现代码** | ✅ PASS | 本 spec 已定义用户场景、需求、成功标准。plan 将进一步产出 data-model 和 contracts。 |
| **V. 不允许伪兼容** | ✅ PASS | 所有工具都有明确定义的输入/输出/错误行为。不支持的功能（如多 Agent）明确标记为 out-of-scope，不提供占位实现。 |
| **VI. 测试驱动兼容性** | ✅ PASS | 工具测试用 mock 模型，集成测试覆盖核心流程。 |
| **VII. Trace 是核心验收产物** | N/A | 非 AgentScope Python 兼容项目。但 Agent 事件流将被验证。 |
| **VIII. Rust 原生设计** | ✅ PASS | 使用 trait object（`Arc<dyn Agent>`, `Arc<dyn Memory>`），enum 表达状态，Result<T,E> 处理错误。不机械复制 TypeScript 继承体系。 |
| **IX. 安全 Rust 优先** | ✅ PASS | 无 unsafe 代码需求。所有 crate 默认 `#[deny(unsafe_code)]`。 |
| **X. 结构化并发** | ✅ PASS | 使用 tokio async runtime，ReActAgent 内部已实现结构化并发。 |
| **XI. 分层与依赖方向** | ✅ PASS | pi-rust 依赖 agentscope-rust 框架 crate，不反向依赖。框架 crate 间无循环依赖。 |
| **XII. 稳定的数据协议** | ✅ PASS | 工具参数和结果使用 serde 序列化，含 `#[serde(default)]` 处理未知字段。 |
| **XIII. 稳定错误模型** | ✅ PASS | 使用 `anyhow::Result` 或自定义 `PiError` 枚举区分错误类型。不依赖字符串匹配。 |
| **XIV. 可观测性** | ✅ PASS | 使用 `tracing` + `env_logger` 记录关键 span（tool invocation, API call, session save/load）。API Key 通过 `#[serde(skip_serializing)]` 排除。 |
| **XV. 性能不能牺牲正确性** | ✅ PASS | CLI 启动路径无性能捷径；工具调用顺序由 ReActAgent 保证。 |
| **XVI. 小步交付** | ✅ PASS | 单 feature 聚焦 pi-rust CLI 的 5 个核心用户故事。 |
| **XVII. 完成的定义** | ✅ PASS | 将遵循 Done Definition checklist。 |
| **XVIII. 兼容性分级** | N/A | 不适用。 |
| **XIX. 变更治理** | ✅ PASS | spec → plan → tasks → implement 流程遵从宪法治理。 |

**Gate Result**: PASS — 无违规项需要 Justification。

## Project Structure

### Documentation (this feature)

```text
specs/023-pi-coding-agent/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── tool-contracts.md    # Tool input/output schemas
│   └── cli-contract.md      # CLI arguments and REPL commands
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
examples/pi-rust/
├── Cargo.toml           # Dependencies on agent_scope_* crates
└── src/
    ├── main.rs          # CLI entry point, argument parsing, REPL loop
    ├── config.rs         # RuntimeConfig from CLI args
    ├── agent.rs          # Agent builder, system prompt, middleware assembly
    ├── tools.rs          # Read/Write/Edit/Bash tool implementations
    ├── session.rs        # Session persistence (save/load/list)
    ├── render.rs         # Streaming event renderer (text + tool call status)
    └── repl.rs           # REPL command handlers (/help, /model, /tools, /exit)
```

**Structure Decision**: 单项目结构（`examples/pi-rust/`）。每个核心关注点一个模块文件，避免过度拆分（总共 <2000 行 Rust）。不创建子 crate——pi-rust 是示例应用，复用框架 crate 的能力。

## Complexity Tracking

> 无 Constitution Check 违规项需要 Justification。

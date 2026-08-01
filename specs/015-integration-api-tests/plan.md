# Implementation Plan: Integration API Tests (Examples)

**Branch**: `015-integration-api-tests` | **Date**: 2026-07-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/015-integration-api-tests/spec.md`

## Summary

创建 4 个独立的 example binary，使用 DashScope 真实 API 测试已实现模块的端到端集成：
Memory（记忆系统）、Session（会话持久化）、RAG（检索增强生成）、Streaming Tool-Call 事件生命周期。

每个 example 都通过 `--api-key` 或 `API_KEY` 环境变量接收 DashScope API key，通过 `--model` 选择模型（默认 `qwen-plus`），产生明确的 pass/fail 输出。

## Technical Context

**Language/Version**: Rust 2024 edition (stable)

**Primary Dependencies**: agent_scope_agent, agent_scope_dashscope, agent_scope_memory, agent_scope_state, agent_scope_rag, agent_scope_embedding, agent_scope_message, agent_scope_tool, clap, tokio, futures, schemars, serde, uuid, chrono

**Storage**: File-based (FileMemory for memory tests), In-memory (InMemorySessionStore for session tests), In-memory VectorStore (RAG tests)

**Testing**: Manual execution via `cargo run --example <name> -- --api-key sk-xxx`

**Target Platform**: macOS/Linux, requires network access to dashscope.aliyuncs.com

**Project Type**: CLI examples（集成测试用）

**Performance Goals**: 每个测试场景在 60s 内完成（网络正常时）

**Constraints**: 需要有效的 DashScope API key；依赖 `qwen-plus` 模型支持 tool calling；依赖 DashScope embedding API 可用

**Scale/Scope**: 4 个 example binary，覆盖 3 个以上不同模块

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | 条款 | 状态 | 说明 |
|---|------|------|------|
| 1 | 兼容性优先 | ✅ 通过 | Examples 是 leaf 节点，不影响核心兼容性 |
| 4 | 先定义契约 | ✅ 通过 | spec.md 已创建并通过质量检查 |
| 6 | 测试驱动兼容性 | ✅ 通过 | 这些 examples 本身即为集成测试；使用真实 API 但每个场景有确定性断言（如"回答包含 408"） |
| 8 | Rust 原生设计 | ✅ 通过 | 使用 async/await、Arc<dyn Trait>、Result 等 Rust 原生模式 |
| 9 | 安全 Rust 优先 | ✅ 通过 | 所有 examples 使用 `#![deny(unsafe_code)]` |
| 11 | 分层与依赖方向 | ✅ 通过 | Examples 作为 CLI 二进制，合法依赖所有 crate |
| 16 | 小步交付 | ✅ 通过 | 每个 example 是独立的可交付单元 |

**结论**: 19/19 条款全部通过或豁免，无违规。

## Project Structure

### Documentation (this feature)

```text
specs/015-integration-api-tests/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (CLI contracts)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
examples/
├── chat.rs              # 现有：交互式流式对话
├── verify_agent.rs      # 现有：Agent 验证测试套件
├── common.rs            # 现有：共享构建函数
├── memory_test.rs       # 新增：Memory 集成测试
├── session_test.rs      # 新增：Session 持久化集成测试
├── rag_test.rs          # 新增：RAG Pipeline 集成测试
└── streaming_tool_test.rs  # 新增：流式工具调用事件生命周期测试
```

**Structure Decision**: 所有 example 放在 `examples/` 目录，与现有 `chat.rs`、`verify_agent.rs` 保持一致。复用 `common.rs` 中已有的 `build_agent`、`create_model`、`create_calculator_tool` 等 helper。

## Complexity Tracking

> 无违规，无复杂度追踪项。

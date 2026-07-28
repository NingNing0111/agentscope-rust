# Implementation Plan: Tool System — 最小可行实现

**Branch**: `006-tool-system` | **Date**: 2026-07-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/006-tool-system/spec.md`

## Summary

实现 `agent_scope_tool` crate，包含 Tool trait（核心抽象）、FunctionTool（通过 schemars 自动推导 schema 的 async handler 适配器）、ToolKit（注册中心 + schema 导出 + 调用分发）。对齐上游 AgentScope Python 的 Tool 设计，但不机械复制 Python 的继承体系。ToolChunk 复用现有 agent_scope_message 中的 `ToolResultBlock`，仅新增 `is_last` 字段。完全在单元测试中验证，不依赖真实 LLM。

## Technical Context

**Language/Version**: Rust edition 2024 (toolchain: stable)

**Primary Dependencies**: 
- `serde` + `serde_json` (序列化/反序列化)
- `schemars` 0.8 (JSON Schema 自动推导)
- `tokio` + `async-trait` (异步运行时与 trait 抽象)
- `futures` (Stream trait)
- `agent_scope_message` (ToolCallBlock, ToolResultBlock, ToolOutput, ContentBlock)
- `agent_scope_model` (ToolChoice 用于 US3 验证)

**Storage**: N/A — ToolKit 使用内存 `HashMap<String, Box<dyn Tool>>` 存储

**Testing**: `cargo test` — 所有测试为纯单元测试，使用 mock/固定 Tool，无需网络或 LLM

**Target Platform**: Linux/macOS server (纯 Rust 库，无平台特定依赖)

**Project Type**: 单 Rust 库 crate (`agent_scope_tool`)，加入 workspace

**Performance Goals**: N/A — Tool 调用本身开销远小于 LLM API 调用

**Constraints**:
- 零 unsafe 代码
- `cargo clippy` 和 `cargo fmt` 全通过
- 所有测试可在无网络环境下运行
- 公开 API 通过 trait object (`Arc<dyn Tool>`) 暴露，不引入复杂泛型

**Scale/Scope**: 最小可行实现 — Tool trait、FunctionTool、ToolKit。不包含 ToolGroup、ToolMiddleware、MCP、Skill、Permission。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Article 1: Compatibility First

| Check | Status |
|-------|--------|
| Tool trait 设计对齐 Python ToolBase | ✅ Tool trait 的 `name()`、`description()`、`input_schema()`、`call()` 对齐 Python |
| ToolKit.get_tool_schemas() 格式对齐 Python | ✅ 输出 OpenAI function schema 格式，与 Python `Toolkit.get_tool_schemas()` 一致 |
| 外部可观察行为对齐 | ✅ ToolKit schema 输出格式、call_tool 分发逻辑与 Python 一致 |
| 内部实现允许差异 | ✅ Rust 侧用 `trait` + trait object（非 Python 继承体系），符合 Constitution Art.8 |

### Article 2: Locked Upstream Version

| Check | Status |
|-------|--------|
| 上游版本锁定 | ✅ agent_scope Python v1.0.0 是兼容基线 |
| 不作为模糊目标 | ✅ 本 Feature 以 AgentScope Python `ToolBase`/`Toolkit` 的具体行为为准 |

### Article 3: Python AgentScope is Behavioral Baseline

| Check | Status |
|-------|--------|
| 行为基准基于实际运行结果 | ✅ ToolKit schema 格式和 Tool 调用流程经过 Python 参考代码验证 |
| 非仅凭文档实现 | ✅ 设计决策已在对齐 Python ToolBase 源码后确认（见 spec-design-decision 过程） |

### Article 4: Contract Before Implementation

| Check | Status |
|-------|--------|
| Spec 已批准 | ✅ spec.md 已完成并包含完整 FR、US、Edge Cases |
| Plan 文档生成 | ✅ 本文档 |
| 公开 API 先定义再实现 | ✅ Tool trait、FunctionTool、ToolKit 接口均在 spec 中明确 |

### Article 5: No Fake Compatibility

| Check | Status |
|-------|--------|
| 暂未实现的功能明确标记 | ✅ ToolGroup、ToolMiddleware、Permission、MCP 在 spec Assumptions 中声明为后续 Feature |
| 不返回虚假成功 | ✅ FR 明确定义：未知 Tool → `ToolError::NotFound`，非法 input → `ToolError::InvalidInput` |

### Article 6: Test-Driven Compatibility

| Check | Status |
|-------|--------|
| 单元测试覆盖 | ✅ 每个 FR 有对应测试 |
| 无 LLM 依赖 | ✅ 所有测试用 mock Tool/固定 handler |
| 序列化往返测试 | ✅ ToolKit schema 输出会做 JSON 往返测试 |

### Article 8: Rust-Native Design

| Check | Status |
|-------|--------|
| 不机械复制 Python 继承体系 | ✅ 使用 `trait Tool` + trait object 模式 |
| 避免过度泛型 | ✅ 公开 API 使用 `Box<dyn Tool>` / `Arc<dyn Tool>` |
| 不依赖反射/Any | ✅ 类型安全：通过 `T: JsonSchema` 约束自动推导 schema |

### Article 9: Safe Rust First

| Check | Status |
|-------|--------|
| 无 unsafe | ✅ 整个 crate 零 unsafe 代码 |
| 无无理由 unwrap | ✅ 所有错误通过 Result 传播 |
| 仅有合理 panic 捕获 | ✅ `FunctionTool::call()` 使用 `std::panic::catch_unwind` 捕获 handler panic |

### Article 11: Layering & Dependency Direction

| Check | Status |
|-------|--------|
| Tool abstraction 层不依赖具体 provider | ✅ agent_scope_tool 仅依赖 agent_scope_message + agent_scope_model（核心抽象），不依赖任何 provider crate |
| 无循环依赖 | ✅ agent_scope_tool → agent_scope_message + agent_scope_model（单向） |

### Article 12: Stable Data Protocol

| Check | Status |
|-------|--------|
| 新增字段向后兼容 | ✅ `ToolResultBlock.is_last` 使用 `#[serde(default)]`，默认 `false` |
| 序列化版本兼容 | ✅ 旧 JSON（无 is_last）反序列化不报错 |

### Article 13: Stable Error Model

| Check | Status |
|-------|--------|
| 类型化错误 | ✅ `ToolError` enum 包含 NotFound, InvalidInput, Execution, Interrupted |
| 遵循错误分类表 | ✅ ToolError 对应宪法中的 ToolError 类别 |

### Article 16: Small-Step Delivery

| Check | Status |
|-------|--------|
| 独立能力模块 | ✅ Feature 005（Dashboard/Provider 拆分）已完成后，Tool System 独立交付 |
| 可独立完成 spec → impl → test → 验收 | ✅ Tool System 仅依赖已完成的消息和模型基础设施 |

### Article 17: Definition of Done

| Check | Status |
|-------|--------|
| Checklist 将随实现完成逐项确认 | ⏳ 实现阶段执行 |

### Gate Summary

**所有适用的宪法条款检查通过，无违规项。** 无违规项需要记录在 Complexity Tracking 表中。

## Project Structure

### Documentation (this feature)

```text
specs/006-tool-system/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/
├── agent_scope_tool/       # NEW — Tool System crate
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # 模块入口, pub mod 声明
│       ├── tool_trait.rs   # Tool trait, ToolOutput, ToolError, ToolChunk
│       ├── function.rs     # FunctionTool + IntoChunk trait
│       └── toolkit.rs      # ToolKit 注册中心
├── agent_scope_message/    # MODIFIED — ToolResultBlock 新增 is_last
│   └── src/block.rs
├── agent_scope_model/      # (already depends on message, no changes)
├── ...
```

**Structure Decision**: 单 Rust crate `agent_scope_tool` 加入现有 workspace。遵循现有 crates 的目录布局约定。`agent_scope_message` 最小修改（仅 `ToolResultBlock` 新增字段）。

## Complexity Tracking

> **无违规项，无需记录。**

## Post-Phase-1 Constitution Re-Check

After completing Phase 1 design (data-model.md, contracts/, quickstart.md), re-evaluate all gates:

| Article | Re-Check |
|---------|----------|
| Art.1 (Compatibility First) | ✅ Tool trait, ToolKit schema format, call dispatch all align with Python AgentScope |
| Art.4 (Contract Before Implementation) | ✅ contracts/tool-api-contract.md defines all public interfaces |
| Art.5 (No Fake Compatibility) | ✅ Unsupported features (ToolGroup, Middleware, MCP) explicitly listed as non-goals |
| Art.8 (Rust-Native Design) | ✅ trait + trait object pattern, no mechanical copy of Python inheritance |
| Art.9 (Safe Rust First) | ✅ Zero unsafe, all errors typed |
| Art.11 (Layering) | ✅ Dependency direction: tool → message + model (one-way, no cycles) |
| Art.12 (Stable Data Protocol) | ✅ ToolResultBlock.is_last uses serde(default) for backward compat |
| Art.13 (Stable Error Model) | ✅ ToolError enum with typed variants, implements Display + Error |

**All gates remain passed. No new violations introduced by design decisions.**

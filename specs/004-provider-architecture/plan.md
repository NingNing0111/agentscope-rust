# Implementation Plan: Provider Architecture & DashScope Integration

**Branch**: `004-provider-architecture` | **Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/004-provider-architecture/spec.md`

## Summary

将 `agent_scope_model` 核心 crate 中的 OpenAI Provider 实现（`src/openai/`, ~1040 行）提取为独立 crate `agent_scope_openai`，从核心 crate 移除 `reqwest` 等 HTTP 依赖。在此基础上，优先实现阿里云百炼 DashScope Provider（`agent_scope_dashscope` crate），通过 DashScope 的 OpenAI 兼容模式（`/compatible-mode/v1`）接入通义千问系列模型。

## Technical Context

**Language/Version**: Rust 2024 edition, MSRV 1.85+

**Primary Dependencies**: `agent_scope_model`（ChatModel trait）、`reqwest` 0.12（stream + json features）、`serde`/`serde_json`、`tokio` 1.x、`tokio-stream` 0.1、`futures` 0.3、`base64` 0.22、`serde_json::Value`、`wiremock` 0.6（dev-dependency for mock HTTP tests）

**Storage**: N/A（无持久化存储，Provider crate 仅做 HTTP 调用和响应解析）

**Testing**: `cargo test`（所有 crate 下）、mock HTTP server（`wiremock` 0.6 as `[dev-dependencies]`）、录制回放（US3/P2，不阻塞 P1 交付）

**Target Platform**: Linux/macOS server-side, x86_64 + aarch64

**Project Type**: library（Rust crate 库，供下游 agent 应用依赖）

**Performance Goals**: Provider 层不应引入可感知延迟——HTTP 调用延迟主导（<5ms 框架开销）

**Constraints**: 所有 Provider crate 测试 MUST 在无网络环境可运行（全部 mock）；Provider crate MUST NOT 依赖 `agent_scope_tool`、`agent_scope_agent`，只能依赖 `agent_scope_model` + Foundation crates

**Dependency Cleanup (agent_scope_model)**: 移除 `reqwest`（仅 openai/ 使用）、`tokio-stream`（0 actual refs）、`tokio-util`（0 actual refs）。`serde_yaml` 需重构——`ModelCard::from_yaml()` 改为 `from_raw()`，YAML 解析移入 Provider 层。`futures` 保留（`model_trait.rs` 中 `Pin<Box<dyn Stream>>` 依赖）

**Scale/Scope**: 2 个 Provider crate（openai + dashscope），每个 ~800-1200 行源码 + 测试

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 要求 | 状态 |
|------|------|------|
| **第一条** 兼容性优先 | 拆分后的 OpenAI crate 行为 MUST 与拆分前一致 | ✅ 同代码迁移 |
| **第四条** 先定义契约 | ChatModel trait、Formatter trait 已在 Feature 003 定义 | ✅ 已有契约 |
| **第五条** 不允许伪兼容 | DashScope 不支持的参数 MUST 返回 `UnsupportedFeature` | ✅ FR 明确 |
| **第八条** Rust 原生设计 | Provider 用 struct + trait impl，不机械翻译 Python | ✅ |
| **第九条** 安全 Rust | `#![deny(unsafe_code)]` 延续到所有 Provider crate | ✅ |
| **第十一条** 分层与依赖 | Provider crate 不依赖 agent/tool/memory | ✅ 仅依赖 model |
| **第十三条** 稳定错误模型 | DashScope 错误 → `ModelError` 变体 | ✅ |
| **第十六条** 小步交付 | 拆分为独立 crate，逐个交付 | ✅ |

**Gate Result (Initial Check)**: ALL PASS — 无违规，无需 Complexity Tracking 条目。

### Post-Design Re‑evaluation (Phase 1 完成后)

Re‑checked against research findings and data model design decisions:

| 条款 | 设计决策 | 状态 |
|------|---------|------|
| **第一条** 兼容性优先 | 抽取后 `agent_scope_openai` 使用完全相同的代码和测试 | ✅ |
| **第四条** 先定义契约 | ChatModel/Formatter trait 契约已在 contracts/ 中明确定义 | ✅ |
| **第五条** 不允许伪兼容 | DashScope `tool_choice: "required"` 不支持的模型返回 `UnsupportedFeature`，`enable_search` 同理 | ✅ |
| **第六条** 测试驱动兼容性 | 所有 Provider 测试使用 `wiremock` mock HTTP（无真实 LLM 依赖） | ✅ |
| **第八条** Rust 原生设计 | Provider 使用 struct + trait impl，`Arc<dyn ChatModel>` 作为动态分发 | ✅ |
| **第九条** 安全 Rust | 所有 crate 使用 `#![deny(unsafe_code)]` | ✅ |
| **第十一条** 分层与依赖 | 依赖拓扑已验证：`agent_scope_model` → 无 reqwest ↔ `agent_scope_openai/dashscope` → `agent_scope_model`（无循环、无反向） | ✅ |
| **第十二条** 稳定数据协议 | DashScopeParameters 使用 `#[serde(skip_serializing_if = "Option::is_none")]`，支持未知字段 | ✅ |
| **第十三条** 稳定错误模型 | DashScope 错误 → `ModelError` 对应变体（含 HTTP status → `ModelErrorKind` 映射） | ✅ |
| **第十六条** 小步交付 | OpenAI 提取（Step 1）+ DashScope 实现（Step 2）+ 验证（Step 3） | ✅ |

**Re‑evaluation Result**: ALL PASS — 设计决策与宪法完全一致，无违规项。

## Project Structure

### Documentation (this feature)

```text
specs/004-provider-architecture/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
# crates/ layout (workspace members via "crates/*" glob)
crates/
├── agent_scope_model/          # Core: ChatModel trait (NO reqwest)
│   ├── src/
│   │   ├── lib.rs              # 移除 pub mod openai 和 OpenAI re-exports
│   │   ├── accumulator.rs
│   │   ├── card.rs
│   │   ├── formatter.rs
│   │   ├── json_repair.rs
│   │   ├── model_error.rs
│   │   ├── model_trait.rs
│   │   ├── response.rs
│   │   ├── schema_flat.rs
│   │   ├── tool_choice.rs
│   │   ├── usage.rs
│   │   └── wav_header.rs       # 保留（core 使用）
│   ├── Cargo.toml              # 移除 reqwest/tokio-stream/tokio-util/futures/serde_yaml
│   └── tests/                  # 核心集成测试
│
├── agent_scope_openai/         # NEW: OpenAI Provider
│   ├── src/
│   │   ├── lib.rs              # pub mod 声明 + re-exports
│   │   ├── model.rs            # 从 agent_scope_model::openai::model.rs 迁移
│   │   ├── formatter.rs        # 从 agent_scope_model::openai::formatter.rs 迁移
│   │   ├── parameters.rs       # 从 agent_scope_model::openai::parameters.rs 迁移
│   │   └── _models/            # OpenAI model card YAML 文件
│   ├── Cargo.toml              # 依赖 agent_scope_model + reqwest + tokio-stream
│   └── tests/                  # OpenAI-specific 集成测试
│
└── agent_scope_dashscope/      # NEW: DashScope Provider
    ├── src/
    │   ├── lib.rs
    │   ├── model.rs            # DashScopeChatModel: ChatModel trait 实现
    │   ├── formatter.rs        # DashScopeFormatter: Formatter trait 实现
    │   ├── parameters.rs       # DashScopeParameters: 含 enable_search 等
    │   └── _models/            # 百炼模型 model card YAML
    ├── Cargo.toml              # 依赖 agent_scope_model + reqwest + tokio-stream
    └── tests/
```

**Structure Decision**: 采用 Cargo workspace 多 crate 结构，所有 Provider crate 放在 `crates/` 下，通过 `crates/*` glob 自动注册。每个 Provider crate 遵循相同的内部结构（model.rs / formatter.rs / parameters.rs / _models/），降低用户认知成本。

## Complexity Tracking

无违规项，无需记录。

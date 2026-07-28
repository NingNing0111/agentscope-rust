# Implementation Plan: Provider 剥离与 DashScope 优先实现

**Branch**: `005-provider-extraction-dashscope` | **Date**: 2026-07-29 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/005-provider-extraction-dashscope/spec.md`

## Summary

将 `agent_scope_model` 核心 crate 中内嵌的 OpenAI Provider 实现（`src/openai/`，约 1040 行）直接移除（不创建独立 crate），从核心 crate 剥离 `reqwest`、`tokio-stream`、`tokio-util`、`serde_yaml` 等 HTTP/实现依赖。在此基础上，创建 `agent_scope_dashscope` crate 作为第一个独立 Provider crate，通过 DashScope OpenAI 兼容端点（`/compatible-mode/v1`）接入通义千问系列模型。

## Technical Context

**Language/Version**: Rust 2024 edition, MSRV 1.85+

**Primary Dependencies**: `agent_scope_model`（ChatModel trait）、`reqwest` 0.12（stream + json features）、`serde`/`serde_json`、`tokio` 1.x、`tokio-stream` 0.1、`futures` 0.3、`base64` 0.22、`wiremock` 0.6（dev-dependency for mock HTTP tests）

**Storage**: N/A（无持久化存储，Provider crate 仅做 HTTP 调用和响应解析）

**Testing**: `cargo test`（所有 crate 下）、mock HTTP server（`wiremock` 0.6 as `[dev-dependencies]`）

**Target Platform**: Linux/macOS server-side, x86_64 + aarch64

**Project Type**: library（Rust crate 库，供下游 agent 应用依赖）

**Performance Goals**: Provider 层不应引入可感知延迟——HTTP 调用延迟主导（<5ms 框架开销）

**Constraints**: 所有 Provider crate 测试 MUST 在无网络环境可运行（全部 mock）；Provider crate MUST NOT 依赖 `agent_scope_tool`、`agent_scope_agent`，只能依赖 `agent_scope_model` + Foundation crates

**Dependency Cleanup (agent_scope_model)**:

| 依赖 | 使用位置 | 处理 |
|------|---------|------|
| `reqwest` | 仅 `openai/model.rs` | 随 openai/ 移除 |
| `tokio-stream` | 无实际引用（仅在 Cargo.toml 声明） | 直接移除 |
| `tokio-util` | 无实际引用（仅在 Cargo.toml 声明） | 直接移除 |
| `serde_yaml` | 仅 `card.rs:83` (`from_yaml`) | 重构 `from_yaml()` → `from_raw()` |
| `futures` | `model_trait.rs:7` (trait 定义) + `openai/model.rs` | **保留**（核心 trait 需要 `Pin<Box<dyn Stream>>`） |
| `thiserror` | `model_error.rs` 使用手动 impl | 移除（改用手动 `Display + Error`） |

**Scale/Scope**: 1 个核心 crate 清理 + 1 个新 Provider crate（`agent_scope_dashscope`），DashScope crate ~800-1200 行源码 + 测试

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 要求 | 状态 |
|------|------|------|
| **第一条** 兼容性优先 | 核心 crate 移除 OpenAI 后，剩余接口行为不变 | ✅ 核心 trait 不涉及具体 Provider |
| **第四条** 先定义契约 | ChatModel trait、Formatter trait 已在 Feature 003 定义 | ✅ 已有契约 |
| **第五条** 不允许伪兼容 | DashScope 不支持的参数 MUST 返回 `UnsupportedFeature` | ✅ FR 明确 |
| **第八条** Rust 原生设计 | Provider 用 struct + trait impl | ✅ |
| **第九条** 安全 Rust | `#![deny(unsafe_code)]` 延续到 `agent_scope_dashscope` | ✅ |
| **第十一条** 分层与依赖 | Provider crate 不依赖 agent/tool/memory，仅依赖 model | ✅ |
| **第十三条** 稳定错误模型 | DashScope 错误 → `ModelError` 变体 | ✅ |
| **第十六条** 小步交付 | 拆分为 crate 清理 + Provider 实现，逐个交付 | ✅ |

**Gate Result (Initial Check)**: ALL PASS — 无违规。

### Post-Design Re‑evaluation (Phase 1 完成后)

Re‑checked against research findings and data model design decisions:

| 条款 | 设计决策 | 状态 |
|------|---------|------|
| **第一条** 兼容性优先 | 核心 API 保持不变，仅移除 openai/ 模块和 re-export | ✅ |
| **第四条** 先定义契约 | ChatModel/Formatter trait 契约已在 contracts/ 中明确定义 | ✅ |
| **第五条** 不允许伪兼容 | DashScope `tool_choice: "required"` 不支持的模型返回 `UnsupportedFeature`，`enable_search` 同理 | ✅ |
| **第六条** 测试驱动兼容性 | 所有 Provider 测试使用 `wiremock` mock HTTP（无真实 LLM 依赖） | ✅ |
| **第八条** Rust 原生设计 | Provider 使用 struct + trait impl，`Arc<dyn ChatModel>` 作为动态分发 | ✅ |
| **第九条** 安全 Rust | `agent_scope_dashscope` 使用 `#![deny(unsafe_code)]` | ✅ |
| **第十一条** 分层与依赖 | 依赖拓扑验证：`agent_scope_model`（无 reqwest） ← `agent_scope_dashscope`（有 reqwest），无循环 | ✅ |
| **第十二条** 稳定数据协议 | DashScopeParameters 使用 `#[serde(skip_serializing_if = "Option::is_none")]`，支持未知字段 | ✅ |
| **第十三条** 稳定错误模型 | DashScope 错误映射到 `ModelError`，兼容两种错误响应格式 | ✅ |
| **第十六条** 小步交付 | 核心清理（Step 1）+ DashScope 实现（Step 2）+ 验证（Step 3） | ✅ |

**Re‑evaluation Result**: ALL PASS — 设计决策与宪法完全一致，无违规项。

## Project Structure

### Documentation (this feature)

```text
specs/005-provider-extraction-dashscope/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
# Before (current state)
crates/
├── agent_scope_model/
│   ├── src/
│   │   ├── lib.rs              # pub mod openai + re-exports ← TO REMOVE
│   │   ├── accumulator.rs
│   │   ├── card.rs             # serde_yaml in from_yaml() ← TO REFACTOR
│   │   ├── formatter.rs
│   │   ├── json_repair.rs
│   │   ├── model_error.rs
│   │   ├── model_trait.rs
│   │   ├── openai/             # ← TO DELETE (model.rs, formatter.rs, parameters.rs, _models/)
│   │   ├── response.rs
│   │   ├── schema_flat.rs
│   │   ├── tool_choice.rs
│   │   ├── usage.rs
│   │   └── wav_header.rs
│   ├── Cargo.toml              # reqwest, tokio-stream, tokio-util, serde_yaml ← TO REMOVE
│   └── tests/
│       ├── chat_response_integration.rs   # KEEP
│       ├── cross_crate_tests.rs           # KEEP
│       └── formatter_integration.rs       # TO REMOVE (references OpenAIChatFormatter)

# After
crates/
├── agent_scope_model/          # Core: ChatModel trait (NO reqwest, NO Provider code)
│   ├── src/
│   │   ├── lib.rs              # NO pub mod openai, NO OpenAI re-exports
│   │   ├── accumulator.rs
│   │   ├── card.rs             # from_yaml() → from_raw(HashMap, JsonValue)
│   │   ├── formatter.rs
│   │   ├── json_repair.rs
│   │   ├── model_error.rs
│   │   ├── model_trait.rs
│   │   ├── response.rs
│   │   ├── schema_flat.rs
│   │   ├── tool_choice.rs
│   │   ├── usage.rs
│   │   └── wav_header.rs
│   ├── Cargo.toml              # ONLY: serde, serde_json, uuid, chrono, base64, schemars, futures
│   └── tests/
│       ├── chat_response_integration.rs
│       └── cross_crate_tests.rs
│
└── agent_scope_dashscope/      # NEW: DashScope Provider
    ├── src/
    │   ├── lib.rs
    │   ├── model.rs            # DashScopeChatModel: ChatModel trait 实现
    │   ├── formatter.rs        # DashScopeFormatter: Formatter trait 实现
    │   └── parameters.rs       # DashScopeParameters: 含 enable_search 等
    ├── Cargo.toml              # 依赖 agent_scope_model + reqwest + tokio-stream
    └── tests/
        ├── model_tests.rs      # Mock HTTP tests
        ├── formatter_tests.rs  # Formatter output validation
        └── parameters_tests.rs # Parameters serde round-trip
```

**Structure Decision**: 采用 Cargo workspace 多 crate 结构，`agent_scope_dashscope` 放在 `crates/` 下通过 `crates/*` glob 自动注册。与 Feature 004 不同，不创建 `agent_scope_openai` crate——OpenAI 代码直接移除（可由 Git 历史恢复）。DashScope crate 遵循 Feature 003/004 确认的标准 Provider crate 布局。

## Complexity Tracking

无违规项，无需记录。

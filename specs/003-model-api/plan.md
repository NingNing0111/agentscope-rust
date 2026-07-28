# Implementation Plan: Model API (Feature 003)

**Branch**: `003-model-api` | **Date**: 2026-07-28 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-model-api/spec.md`

## Summary

实现 AgentScope Rust 的 Model 层 — `ChatModel` trait 定义了所有 LLM Provider 的统一接口：`call()`（带重试和取消）、`count_tokens()`、`generate_structured_output()`、`list_models()`。交付 `ChatResponse`（流式增量构建 + StreamAccumulator O(n) 累加）、`ChatUsage`（扩展 token 统计）、`Formatter` trait（Msg → API 格式转换）、`ModelCard`（YAML 模型发现）。首个参考 Provider 实现：`OpenAIChatModel`（通过 `reqwest` 调用 OpenAI Chat Completions API）。

## Technical Context

**Language/Version**: Rust 2024 edition (1.85+)
**Primary Dependencies**: `reqwest` (HTTP), `tokio` (async runtime), `futures` / `tokio-stream` (Stream trait), `serde` / `serde_json`, `serde_yaml`, `schemars` (JSON Schema generation), `base64`
**Storage**: YAML files (`_models/*.yaml`) on disk — no database
**Testing**: `cargo test` with per-crate inline tests + `tests/` directory integration tests
**Target Platform**: Linux/macOS server (async runtime)
**Project Type**: library (Rust crate: `agent_scope_model`)
**Performance Goals**: O(n) StreamAccumulator accumulation (vs Python's O(n²) string concat); no worse than Python reference for HTTP round-trips
**Constraints**: No dependency on tool/agent/memory crates; `#![deny(unsafe_code)]`
**Scale/Scope**: 1 trait, 8 data structures, 1 reference Provider implementation, ~10 source files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Article | Requirement | Status |
|---------|------------|--------|
| **第一条：兼容性优先** | ChatResponse/ChatUsage/ModelCard JSON 序列化与 Python 一致 | ✅ P0 差分测试计划 |
| **第二条：锁定上游版本** | Feature 001 已锁定上游版本 | ✅ 基础已就绪 |
| **第三条：Python 是行为基准** | 重试行为、stream 处理、cancel 语义对齐 | ✅ StreamAccumulator、cancel 传播对齐 |
| **第四条：先定义契约** | Spec 中定义了 trait 签名、数据结构、生命周期 | ✅ 39 FRs |
| **第五条：不允许伪兼容** | 未实现 Provider 返回 UnsupportedFeature | ✅ ModelError 覆盖 |
| **第六条：测试驱动兼容性** | 单元测试 + 差分测试 + Mock Model | ✅ Mock 模型计划 |
| **第七条：Trace 是核心验收** | StreamAccumulator 最终 output 与 Python 等价 | ✅ 差分测试 |
| **第八条：Rust 原生设计** | trait object (`Arc<dyn ChatModel>`) + Stream trait | ✅ 符合 |
| **第九条：安全 Rust 优先** | `#![deny(unsafe_code)]` | ✅ 已在 FR-039 中要求 |
| **第十一条：分层与依赖方向** | model 仅依赖 types/message/utils | ✅ FR-038 |
| **第十二条：稳定数据协议** | ContentBlock unknown variant 处理 | ✅ 继承自 message crate |
| **第十三条：稳定错误模型** | ModelError 区分 6 类错误 | ✅ FR-036/037 |
| **第十六条：小步交付** | 独立 feature，可独立测试 | ✅ |
| **第十七条：完成的定义** | 列入 checklist | ✅ |

**GATE STATUS**: ✅ **PASS** — No violations.

### Re-check After Phase 1 Design

All design decisions align with Constitution:
- `ToolChoice` 在 model crate 内定义 → 避免跨层依赖（第十一条）
- `Formatter` trait 在 model crate 内 → 仅依赖 message（第十一条）
- Stream 类型使用 `Pin<Box<dyn Stream>>` → Rust 原生 trait object 模式（第八条）
- 无 unsafe 代码使用 → 符合第九条

## Project Structure

### Documentation (this feature)

```text
specs/003-model-api/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── chat-model-trait.md
│   ├── chat-response-api.md
│   ├── formatter-trait.md
│   └── openai-model-api.md
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_model/
├── Cargo.toml
└── src/
    ├── lib.rs                    # crate root: pub mod re-exports
    ├── response.rs               # ChatResponse, StructuredResponse, append methods
    ├── usage.rs                  # ChatUsage
    ├── model_trait.rs            # ChatModel trait definition
    ├── model_error.rs            # ModelError enum
    ├── tool_choice.rs            # ToolChoice struct
    ├── accumulator.rs            # StreamAccumulator (_AccTextBlock etc.)
    ├── card.rs                   # ModelCard + from_yaml()
    ├── formatter.rs              # Formatter trait base
    ├── json_repair.rs            # JSON repair helper
    ├── schema_flat.rs            # JSON Schema $ref/$defs flatten
    ├── wav_header.rs             # Streaming WAV header builder
    └── openai/
        ├── mod.rs                # OpenAIChatModel, re-exports
        ├── model.rs              # OpenAIChatModel struct + impl ChatModel
        ├── formatter.rs          # OpenAIChatFormatter
        ├── parameters.rs         # OpenAIChatParameters
        └── _models/              # YAML model cards
            ├── gpt-4.1.yaml
            └── ...

tests/
├── model/
│   ├── chat_response_tests.rs    # ChatResponse unit tests
│   ├── chat_usage_tests.rs       # ChatUsage tests
│   ├── stream_accumulator_tests.rs
│   ├── model_card_tests.rs       # ModelCard loading tests
│   └── formatter_tests.rs        # Formatter tests
└── compatibility/
    ├── fixtures/                 # Python golden snapshot JSON
    └── model_diff_tests.rs       # Diff test framework
```

**Structure Decision**: Single crate `agent_scope_model` 遵循已有 workspace 结构（`crates/*`）。OpenAI 相关代码放在 `openai/` 子目录中，未来可扩展 `anthropic/`、`gemini/` 等。集成测试在 workspace 级的 `tests/model/` 中。

## Complexity Tracking

> No violations — table intentionally left empty.

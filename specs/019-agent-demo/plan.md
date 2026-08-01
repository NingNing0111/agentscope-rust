# Implementation Plan: Complete Agent Demo

**Branch**: `019-agent-demo` | **Date**: 2026-08-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/019-agent-demo/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

在 `examples/agent-demo` 下新增一个完整、可验证的 AgentScope Rust Agent showcase。该 demo 提供确定性验证路径与可选 live-model 路径，用单一连贯场景展示 Agent 交互、结构化消息、事件/流式进度、工具调用、session 连续性、memory/context recall、middleware/trace、RAG/workspace/sandbox 等主要能力。实现方式优先复用现有 examples 的 helper 与已实现 crate API：默认 deterministic 模式使用 scripted/mock 行为生成稳定 trace；live 模式在明确提供 `API_KEY` 等配置后使用 DashScope provider，且所有输出默认脱敏。

## Technical Context

**Language/Version**: Rust 2024 edition（跟随 workspace）；Markdown（README 与 demo 文档）

**Primary Dependencies**: 现有 workspace crate：`agent_scope_agent`、`agent_scope_message`、`agent_scope_event`、`agent_scope_tool`、`agent_scope_memory`、`agent_scope_state`、`agent_scope_rag`、`agent_scope_embedding`、`agent_scope_workspace`、`agent_scope_sandbox`、`agent_scope_dashscope`；现有外部依赖：`tokio`、`futures`、`clap`、`serde`、`serde_json`、`schemars`、`dotenv`

**Storage**: 默认使用临时 demo workspace / session / memory 文件目录（位于 `target/agent-demo/` 或运行时临时目录，避免污染用户工作区）；可选输出结构化 demo trace JSON/Markdown 摘要

**Testing**: `cargo build --examples`；`cargo run --example agent_demo -- --mode deterministic`；必要时 `cargo test --examples` 或新增 example-level deterministic validation；`cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings`

**Target Platform**: 本地开发机器上的 Rust CLI 示例；主要面向 macOS/Linux，Windows 路径行为需避免硬编码；sandbox/platform-specific 能力通过显式 capability status 处理

**Project Type**: Rust library workspace + CLI examples；本特性新增 canonical example package/directory，不新增生产 crate

**Performance Goals**: 确定性主路径在已编译后应快速完成；满足 spec SC-001：prepared machine 下按说明 10 分钟内完成 deterministic primary scenario

**Constraints**:
- `examples/agent-demo` 是用户指定且必须交付的位置
- 根 `Cargo.toml` 当前 `autoexamples = false`，因此新增 example 入口必须显式登记为 `[[example]]`
- deterministic 模式不得依赖真实 LLM 自然语言输出或网络
- live 模式必须与 deterministic validation 明确分离，缺少 `API_KEY` 时给出可操作提示而非 opaque failure
- 默认终端输出、trace、生成文件不得泄露 API key、token、原始凭据或不必要敏感对话内容
- 对无法默认安全演示的能力（例如平台沙箱隔离、外部 provider、长耗时 RAG 数据导入）必须给出 opt-in path 或稳定的 unsupported/skip 说明，禁止伪成功

**Scale/Scope**:
- 新增 `examples/agent-demo/` 目录，包含 CLI 入口、scenario、deterministic runner、live runner、tools、trace rendering、README/coverage checklist 等
- 展示至少 8 项主要 framework capability：Agent loop、messages/content blocks、events/streaming、tool invocation、session continuity、memory/context recall、middleware/trace、RAG/context enrichment、workspace artifact、sandbox/permission handling、typed errors/cancellation 中的可适用项
- 现有 examples (`chat.rs`、`verify_agent.rs`、`memory_test.rs`、`session_test.rs`、`rag_test.rs`、`streaming_tool_test.rs`、`common.rs`) 作为 API 与展示风格参考

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 适用性 | 评估 | 状态 |
|------|--------|------|------|
| 第一条 兼容性优先 | 适用 | Demo 展示的是已实现公共能力的外部可观察行为；trace/coverage checklist 映射到实际场景步骤，避免只展示最终文本 | ✅ |
| 第二条 锁定上游版本 | 适用 | Demo 文档需引用当前 compatibility baseline；不改变上游兼容目标 | ✅ |
| 第三条 Python 是行为基准 | 部分适用 | 本特性为 Rust 示例，不新增核心行为；若声明与 Python 示例迁移等价，需引用既有兼容性记录而非推测 | ✅ |
| 第四条 先契约后实现 | 适用 | 本 plan 生成 CLI/trace/coverage contracts 后再进入 tasks/implementation | ✅ |
| 第五条 不允许伪兼容 | 适用 | 不可用能力必须输出 explicit skip/unsupported 状态；live provider 缺配置不得假装成功 | ✅ |
| 第六条 测试驱动兼容性 | 适用 | deterministic 模式与 mock/scripted 行为为主验证路径；真实 LLM 仅作为可选辅助 | ✅ |
| 第七条 Trace 是核心验收产物 | 适用 | Demo trace 为一等交付物，覆盖 events、tool calls、memory/session/workspace 状态、errors/final output | ✅ |
| 第八条 Rust 原生设计 | 适用 | Example 使用 Rust CLI、trait object/Arc、Result、typed errors，不照搬 Python 动态风格 | ✅ |
| 第九条 安全 Rust 优先 | 适用 | 示例不得引入 unsafe；错误处理避免无理由 unwrap/expect；secret 默认脱敏 | ✅ |
| 第十条 结构化并发 | 适用 | 流式/取消/后台步骤需有 owner、timeout、cancellation path；不引入 orphan tasks | ✅ |
| 第十一条 分层与依赖方向 | 适用 | Example 位于根包 examples，可依赖 public crates；不反向污染核心 crate | ✅ |
| 第十二条 稳定数据协议 | 适用 | Trace JSON/summary schema 明确、可脱敏、未知扩展留出 metadata 字段 | ✅ |
| 第十三条 稳定错误模型 | 适用 | CLI contract 需区分 config/tool/model/unsupported/cancelled 等错误类别，用户输出不依赖字符串匹配 | ✅ |
| 第十四条 可观测性 | 适用 | Demo 必须展示 observable trace；默认不输出 secrets 或完整敏感对话 | ✅ |
| 第十五条 性能不能牺牲正确性 | 适用 | 优先 deterministic correctness；不为简化输出改变事件顺序或隐藏错误 | ✅ |
| 第十六条 小步交付 | 适用 | 本特性只交付 canonical Agent demo，不扩展 Multi-agent/Distributed runtime 等未完成能力 | ✅ |
| 第十七条 完成的定义 | 适用 | 示例可编译运行、docs/coverage 完整、fmt/clippy/tests 通过后方可完成 | ✅ |
| 第十八条 兼容性分级 | 适用 | README/coverage checklist 标注哪些能力为 demonstrated、optional、unsupported/skip，不宣称笼统完全兼容 | ✅ |
| 第十九条 变更治理 | 不适用 | 无宪法违反 | N/A |

**Gate 结果**: 19/19 通过（1 项 N/A 为无宪法违反治理需求），无违反，无需 Complexity Tracking 豁免。

## Project Structure

### Documentation (this feature)

```text
specs/019-agent-demo/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── cli-contract.md
│   ├── trace-schema.md
│   └── coverage-checklist.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml                         # 新增 [[example]] agent_demo，path 指向 examples/agent-demo/main.rs
examples/
├── agent-demo/                    # 本特性新增 canonical complete Agent demo
│   ├── README.md                  # setup/run/coverage/expected output/limitations
│   ├── main.rs                    # clap CLI + mode dispatch
│   ├── scenario.rs                # coherent walkthrough definition and capability steps
│   ├── deterministic.rs           # offline deterministic validation path
│   ├── live.rs                    # optional DashScope-backed live path
│   ├── tools.rs                   # safe demo tools and failure injection
│   ├── trace.rs                   # sanitized trace model + renderers
│   ├── middleware.rs              # observation/enrichment/policy demo middleware
│   └── fixtures/                  # deterministic inputs / expected summaries if needed
└── common.rs                      # 可复用现有 helper；若 API 不适合，agent-demo 保持局部 helper
```

**Structure Decision**: 采用目录型 example（`examples/agent-demo/main.rs`）而不是单文件 `examples/agent_demo.rs`，因为该 demo 需要 README、fixtures、trace/schema、deterministic/live 双路径与多能力场景拆分。由于根包设置了 `autoexamples = false`，实现阶段必须在 `Cargo.toml` 显式添加 `[[example]] name = "agent_demo" path = "examples/agent-demo/main.rs"`。不新增生产 crate，避免扩大依赖面；所有能力通过已发布 public crate API 集成。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

无违反项，本表留空。

## Phase 0 Research Summary

见 [research.md](./research.md)。关键决策：目录型 example + 显式 `[[example]]`；deterministic 为默认验证路径，live DashScope 为 opt-in；trace schema 默认脱敏；unsupported/optional capability 以稳定状态呈现；不新增生产 crate。

## Phase 1 Design Summary

见 [data-model.md](./data-model.md)、[contracts/cli-contract.md](./contracts/cli-contract.md)、[contracts/trace-schema.md](./contracts/trace-schema.md)、[contracts/coverage-checklist.md](./contracts/coverage-checklist.md) 与 [quickstart.md](./quickstart.md)。Post-design Constitution Check 仍为 19/19 通过，无新增违反。
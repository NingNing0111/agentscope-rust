# Implementation Plan: Rig LLM Provider Integration（用 rig 完成 LLM 接入，移除 dashscope provider）

**Branch**: `034-rig-llm-integration` | **Date**: 2026-08-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/034-rig-llm-integration/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

用第三方 LLM 框架 [rig](https://github.com/0xPlaygrounds/rig)（v0.42.0）替换自研的 `agent_scope_dashscope` provider crate。保留 `agent_scope_model::ChatModel` / `agent_scope_embedding::EmbeddingModel` trait 作为公共 API（Q1=A），rig 作为 provider 实现层；新 provider 支持 **Anthropic / OpenAI / DeepSeek** 三家（Q2=C 自定义，均为 rig 原生支持）；示例中原 DashScope 统一改用 **OpenAI**；能力面完整保留（Q3=A：流式 / 工具调用 / 结构化输出 / thinking / embedding）。

核心设计决策（详见 [research.md](research.md)）：

1. **新 crate `agent_scope_rig`**：实现 `ChatModel`/`EmbeddingModel`，内含 `openai`/`anthropic`/`deepseek` 三个 backend。rig 的 `CompletionModel` 是 RPITIT 非对象安全（`completion()`/`stream()` 返回 `impl Future`），而 `ChatModel` 是 `Arc<dyn ChatModel>`（宪法第八条倾向 trait object）——故内部定义对象安全的 `RigChatBackend` trait（`completion(request) -> CompletionResponse` / `stream(request) -> StreamingCompletionResponse`，返回值均为具体类型可 box），三个 provider 各实现之，`RigChatModel` 持有 `Arc<dyn RigChatBackend>`。
2. **示例构造入口**：`RigChatModel::openai(api_key, model)` / `anthropic(...)` / `deepseek(...)`，链式 `.with_stream(true)` / `.with_base_url(...)`，与现有 `DashScopeChatModel::new(...).with_stream(true)` 同级 ergonomics。RAG 用 `RigEmbeddingModel`（OpenAI `text-embedding-3-*`）。
3. **映射层**：`Msg`/`ContentBlock` ↔ rig `Message`（Thinking→`Assistant.thinking`、ToolCall→`assistant.tool_calls`、ToolResult→`Message::ToolResult`、Data 多模态→`User.images`，Hint 为框架内部不发送）；OpenAI function schema JSON ↔ rig `ToolDefinition`；rig `Stream<StreamedAssistantContent>` → `Stream<ChatResponse>`（Text delta→TextBlock 增量、ToolCall/Delta→ToolCallBlock、Reasoning/Delta→ThinkingBlock、流末 is_last + tool_call_id_map 重写）；rig `CompletionError` → `ModelError`。
4. **删除 dashscope**：删除 `crates/agent_scope_dashscope`，根 manifest（`Cargo.toml:52`）+ 7 个示例（agent/chat/human-in-the-loop/plan-react-agent/quickstart/rag/subagent 的 `Cargo.toml:14` + main.rs）迁移到 `agent_scope_rig`；环境变量 `DASHSCOPE_API_KEY` → `OPENAI_API_KEY`；文档（README / `docs/rust/zh` / `agentscope-guide` skill / CHANGELOG）同步。
5. **能力保留**：thinking 与 tool_choice 互斥规避迁移为按 provider 适配；`generate_structured_output` 优先用 rig `output_schema` 原生结构化输出，否则回退 tool-calling bypass；embedding 缓存（`EmbeddingCache`/`FileEmbeddingCache`）保留，仅 OpenAI 提供 embedding。

## Technical Context

**Language/Version**: Rust（workspace edition 2024，stable toolchain）；rig 0.42.0（2026-08-16 发布，纳入依赖锁定基线）

**Primary Dependencies**:
- 新增：`rig = "0.42"`（含 default features；依赖 `rig-core` 的 openai/anthropic/deepseek providers）
- 新增：`agent_scope_rig`（path crate，依赖 `agent_scope_model`/`agent_scope_message`/`agent_scope_embedding`/`rig`）
- 删除：`agent_scope_dashscope`
- 保持：`agent_scope_model`/`agent_scope_embedding`/`agent_scope_message` 零第三方 HTTP 依赖（宪法第十一条：core 层不依赖 provider）

**Storage**: N/A——不涉及持久化；embedding 缓存复用现有 `FileEmbeddingCache`

**Testing**: `cargo test`（workspace）+ 确定性测试策略（宪法第六条）：mock/recorded rig 响应回放（rig `test-utils` 或自定义 wiremock 录制 OpenAI-compatible 响应）；消息映射往返测试；流式事件顺序断言；错误映射测试；7 示例编译验证；`plan-react-agent`/`chat` 手工（真实 OpenAI key 可选）

**Target Platform**: 跨平台库（Linux / macOS / Windows）；rig 走 HTTPS（reqwest）

**Project Type**: library（多 crate Cargo workspace）——新增 provider crate `agent_scope_rig`

**Performance Goals**: 无独立性能目标——LLM 网络 I/O 是瓶颈（宪法第十五条）；rig 依赖带来的编译时间成本需在 CI 关注，但无运行时性能回归

**Constraints**: `#![deny(unsafe_code)]`（宪法第九条）；core 层（model/embedding/message）MUST 不依赖 rig（宪法第十一条）；无新后台任务/无未绑定 spawn（宪法第十条）；公共数据协议（ChatResponse/ContentBlock/Msg/ToolChoice/ModelError）零变更（宪法第十二/十三条）；不存在未登记的 `UnsupportedFeature`（宪法第五条）；示例 + 文档同步（宪法第十七条）

**Scale/Scope**: 主改 1 个新 crate（`agent_scope_rig`：backend + 映射 + 流式转换）；删 1 个 crate（`agent_scope_dashscope`）；改 7 个示例（main.rs 构造 + Cargo.toml）；改根 `Cargo.toml`；迁移测试（dashscope 的 `model_tests.rs`/`embedding_tests.rs` 契约迁移到 rig crate）；更新文档（README / docs/rust/zh / agentscope-guide / CHANGELOG / 兼容性矩阵 `provider-*` 条目）

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 符合性 | 说明 |
|------|--------|------|
| 第一条 兼容性优先 | ✅ | `ChatModel`/`EmbeddingModel` trait 及公共数据协议（ChatResponse/ContentBlock/Msg/ToolChoice/ModelError）零变更；agent 引擎（react_loop/streaming_reactor/middleware/token_counter）零感知。流式事件顺序、工具调用生命周期、结构化输出在映射层保持。**实际 LLM 输出的自然差异（不同服务商/模型）属 provider 选择，不属框架兼容性** |
| 第二条 锁定上游版本 | ✅ | rig 版本锁定 0.42.0，纳入依赖基线记录；DashScope 兼容基线移除（该 provider 已删除，其行为不再作为兼容目标） |
| 第三条 Python 是行为基准 | ✅ | 框架行为以确定性（mock/recorded）测试验证，不依赖真实 LLM 自然语言；provider 变更不改变框架对外可观察结构 |
| 第四条 先契约后实现 | ✅ | spec(034) → research → data-model → contracts（`contracts/provider-adapter.md`、`contracts/rig-mapping.md`）先行 |
| 第五条 不允许伪兼容 | ✅ | 无 stub/no-op/静默忽略；provider 不支持的能力（如 Anthropic/DeepSeek 无 embedding）显式记录为已知限制；`UnsupportedFeature` 不静默 |
| 第六条 测试驱动兼容性 | ✅ | 消息映射往返、流式顺序、错误映射、结构化输出测试均用确定性组件；示例仅编译验证（真实 key 为可选冒烟） |
| 第七条 Trace 是核心验收产物 | ✅ | 流式 chunk→事件协议（TextBlock/ThinkingBlock/ToolCallBlock 的 Start/Delta/End）在映射层保持，`react_loop`/`streaming_reactor` 不改 |
| 第八条 Rust 原生设计 | ✅ | 内部 `RigChatBackend`/`RigEmbeddingBackend` 用 trait object（`Arc<dyn ...>`），符合宪法"动态扩展点优先 trait object"；公开构造入口保持简洁（`RigChatModel::openai(...)`），不暴露复杂泛型 |
| 第九条 安全 Rust 优先 | ✅ | 无 unsafe；无新 panic 路径（映射层错误全部走 `Result`） |
| 第十条 结构化并发 | ✅ | 零新 spawn/零新 channel；rig 流在 agent 既有 `tokio::select!`（cancel_token 竞态）内消费，取消/超时语义不变 |
| 第十一条 分层与依赖方向 | ✅ | rig 依赖仅进入 `agent_scope_rig`；`agent_scope_model`/`agent_scope_message`/`agent_scope_embedding`/`agent_scope_agent` 均不依赖 rig；无循环依赖 |
| 第十二条 稳定数据协议 | ✅ | 公开 serde 结构零变更（Msg/ContentBlock/ChatResponse/ToolChoice 等）；rig 类型仅存在于 `agent_scope_rig` 内部映射层 |
| 第十三条 稳定错误模型 | ✅ | `ModelError`/`ModelErrorKind` 不变；rig `CompletionError` 在 `agent_scope_rig` 内映射到现有分类（Authentication/RateLimit/InternalServer/BadRequest/ApiConnection/ApiTimeout） |
| 第十四条 可观测性 | ✅ | tracing span/事件不变；映射层不输出 key/敏感内容 |
| 第十五条 性能不牺牲正确性 | ✅ | 无性能优化诉求；rig 引入的编译成本是编译期成本，运行时行为正确性优先 |
| 第十六条 小步交付 | ✅ | 单一能力（provider 接入层替换），前置（003 model API / 004-005 provider 架构 / 006 tool / 007 agent / 011 RAG）均已交付 |
| 第十七条 完成的定义 | ✅ | quickstart.md 定义完整验收（build/test/clippy/fmt/示例编译/文档/兼容矩阵更新）；无未登记 UnsupportedFeature |
| 第十八条 兼容性分级 | ✅ | 目标等级维持 **L2**（核心行为）+ **L3**（公开 API 语义）；provider 差异（服务商/模型能力）在兼容矩阵 `provider-*` 条目登记，不降级框架兼容等级 |
| 第十九条 变更治理 | ✅ | 三个关键决策（Q1 保留 trait / Q2 三服务商 / Q3 完整能力）均由用户在 `/speckit-specify` 阶段人工批准；provider 特有差异按流程记录（原因/替代/风险/批准） |

**Gate 结果（Phase 0 前）**: 通过——无未论证的宪法违规；关键架构选择（保留 trait + boxed backend + 单 provider crate）符合第一/八/十一条。
**Gate 结果（Phase 1 后复审）**: 通过——research/data-model/contracts/quickstart 已生成，未引入新违规。复审要点：
- 决策 12（OpenAI 用 `CompletionsClient`）把 OpenAI 与 Anthropic/DeepSeek 拉回同一 wire 语义，映射层单一，符合宪法第三条（行为基准）与第十一条（依赖方向）。
- 公开数据协议零变更（data-model.md 核心声明）保持宪法第十二/十三条；rig 类型不越过 `agent_scope_rig` 边界。
- thinking 互斥按 provider 能力位守卫（`thinking_tool_choice_incompatible`），降级非静默（tracing 事件），符合宪法第五条/第十四条。
- 已知偏差全部显式登记（provider-adapter.md §4：仅 OpenAI embedding、enable_search 不迁移等），无未登记 UnsupportedFeature（宪法第五条/第十八条）。

## Project Structure

### Documentation (this feature)

```text
specs/034-rig-llm-integration/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
# 新增 provider crate
crates/agent_scope_rig/
├── Cargo.toml                # deps: rig, agent_scope_model, agent_scope_message, agent_scope_embedding, async-trait, futures
├── src/
│   ├── lib.rs                # RigChatModel, RigEmbeddingModel, 各 provider 构造入口, re-exports
│   ├── backend.rs            # RigChatBackend / RigEmbeddingBackend（对象安全 trait）
│   ├── openai.rs             # OpenAI backend（rig openai provider）
│   ├── anthropic.rs          # Anthropic backend（rig anthropic provider）
│   ├── deepseek.rs           # DeepSeek backend（rig deepseek provider）
│   ├── message.rs            # Msg/ContentBlock ↔ rig Message 映射
│   ├── tools.rs              # JSON tool schema ↔ rig ToolDefinition；ToolChoice 映射
│   ├── stream.rs             # rig StreamingCompletionResponse → Stream<ChatResponse>
│   ├── structured.rs         # generate_structured_output（output_schema / tool-calling bypass）
│   ├── error.rs              # rig CompletionError/EmbeddingError → ModelError/EmbeddingError
│   └── params.rs             # ChatParameters ↔ rig 请求参数（temperature/max_tokens/stop 等）
└── tests/
    ├── message_mapping_tests.rs   # Msg↔rig Message 往返
    ├── tools_mapping_tests.rs     # JSON tool schema ↔ ToolDefinition
    ├── streaming_tests.rs         # 流式 chunk→ChatResponse 顺序断言
    ├── error_mapping_tests.rs     # rig 错误 → ModelErrorKind 分类
    ├── openai_tests.rs            # OpenAI backend 构造 + mock 冒烟
    ├── anthropic_tests.rs
    ├── deepseek_tests.rs
    └── structured_output_tests.rs # output_schema + tool-calling 回退

# 删除
crates/agent_scope_dashscope/     # 整个 crate 删除

# 示例迁移（构造行 + Cargo.toml 依赖换 agent_scope_rig）
examples/agent/src/main.rs            # DashScopeChatModel → RigChatModel::openai
examples/chat/src/main.rs
examples/human-in-the-loop/src/main.rs
examples/plan-react-agent/src/main.rs
examples/quickstart/src/main.rs
examples/subagent/src/main.rs
examples/rag/src/main.rs              # chat + embedding 均换

# 根 workspace
Cargo.toml                     # 移除 agent_scope_dashscope，加入 agent_scope_rig
```

**Structure Decision**: 单 provider crate `agent_scope_rig`（内含三 backend）而非每 provider 一个 crate。理由：rig 本身是统一抽象，三个 provider 共享约 90% 适配代码（消息/工具/流式/结构化输出映射）；一个 crate 避免重复，示例只需一个依赖。与 Feature 005 的"每 provider 一 crate"惯例不同，但那是自研 provider（各 provider 协议差异大）；rig-backed provider 差异仅在 client 构造与模型名，合并更合理（见 research.md 决策 1）。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| 无 | — | — |

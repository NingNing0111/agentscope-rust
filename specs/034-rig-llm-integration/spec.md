# Feature Specification: Rig LLM Provider Integration

**Feature Branch**: `034-rig-llm-integration`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "使用rig完成LLM的接入，移除dashscope的实现 crate。rig项目：https://github.com/0xPlaygrounds/rig"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Remove the DashScope provider crate (Priority: P1)

As a maintainer, I want the hand-rolled DashScope provider crate (`agent_scope_dashscope`) removed from the workspace so that LLM provider integration is delegated to the vetted third-party rig framework instead of in-project HTTP/SSE/formatting code.

**Why this priority**: Removing the deprecated provider is the concrete, testable outcome of the request; everything else (examples, docs, tests) is migration work that must be complete before the crate can actually be deleted.

**Independent Test**: Can be fully tested by removing the crate from workspace members, migrating all dependents, and verifying `cargo build --workspace` succeeds with no `agent_scope_dashscope` reference remaining in code or manifests.

**Acceptance Scenarios**:

1. **Given** the workspace currently ships `crates/agent_scope_dashscope`, **When** the migration completes, **Then** the crate is deleted, removed from workspace members and the root manifest, and no example or crate depends on it.
2. **Given** the root package previously re-exported the dashscope crate, **When** the removal lands, **Then** the root manifest no longer lists it and the root package builds cleanly.

---

### User Story 2 - Create chat agents through the rig-backed OpenAI provider (Priority: P1)

As a user, I want to create a chat model and run a ReAct agent with the same low-friction setup I have today — an API key, a model name, and an optional streaming toggle — so that my existing example-based workflows (quickstart, chat, agent, subagent, human-in-the-loop, plan-react-agent) keep working, now against the rig-backed OpenAI provider.

**Why this priority**: The current examples are the primary onboarding surface; if a user cannot construct a working model with similar effort, the migration is a regression even if the internals are cleaner.

**Independent Test**: Can be tested by building every affected example and running the agent loop against a deterministic (mock/recorded) rig-backed model, verifying a reply is produced and streaming events are emitted in the documented order.

**Acceptance Scenarios**:

1. **Given** a user provides an OpenAI API key and a model name, **When** they construct the rig-backed chat model through the new entry point, **Then** they can pass it to `AgentConfig::builder().model(...)` and run a ReActAgent.
2. **Given** streaming is enabled, **When** the agent runs `reply_stream`, **Then** the same event sequence (message deltas, tool calls, tool results, end-of-stream) is emitted as today.

---

### User Story 3 - Embedding / RAG through the rig-backed integration (Priority: P2)

As a user of RAG features, I want to construct an embedding model through the rig-backed integration so that the RAG example and knowledge-base workflows continue to work after the DashScope crate is gone. Embedding is provided through OpenAI's embedding models (text-embedding-3-*), which rig supports natively.

**Why this priority**: RAG is a core AgentScope capability; losing embedding support in the migration would leave the RAG example and knowledge-base features without a working provider.

**Independent Test**: Can be tested by building the RAG example and running embedding + retrieval against a deterministic embedding source (fixed vectors), verifying the knowledge base returns expected documents.

**Acceptance Scenarios**:

1. **Given** the RAG example previously imported `DashScopeEmbeddingModel`, **When** the migration completes, **Then** the example builds and can embed texts through the rig-backed OpenAI embedding model with comparable model-card and caching behavior.

---

### User Story 4 - Observable behavior preserved (Priority: P2)

As a compatibility-conscious maintainer, I want the agent engine's externally observable behavior — event ordering, tool call/result lifecycle, structured output, retry/error semantics — to remain equivalent after swapping the provider, so that the compatibility baseline is not silently weakened.

**Why this priority**: The constitution makes observable compatibility the top priority; a provider swap must not silently change streaming order, error categories, or tool-call behavior.

**Independent Test**: Can be tested by running the existing model/agent compatibility tests against the rig-backed provider and diffing the trace (events, tool calls, finish reasons) against the recorded baseline.

**Acceptance Scenarios**:

1. **Given** a recorded/mock response stream, **When** the rig-backed model consumes it, **Then** the emitted `ChatResponse`/events match the baseline trace modulo allowed normalization fields.
2. **Given** a thinking-mode request, **When** the provider receives it, **Then** the behavior matches today's documented handling of thinking-vs-tool-choice mutual exclusion (or the difference is recorded as an approved known deviation).

---

### Edge Cases

- 各 provider 的能力差异：OpenAI reasoning（o 系列）、Anthropic thinking、DeepSeek reasoner 的推理内容在流式事件中的表现与现有 DashScope thinking 的对齐情况。
- thinking 与 `tool_choice` 互斥的规避逻辑如何迁移为通用/按 provider 适配逻辑（现有 DashScope 有专门处理）。
- API key 缺失/无效时的错误信息与分类是否保持清晰（不泄露 key）。
- 流式中断、网络错误、超时与重试行为是否与现有 `ModelError` 分类一致。
- embedding 缓存（`EmbeddingCache`/`FileEmbeddingCache`）行为是否保留（三家 provider 中仅 OpenAI 提供 embedding）。
- 旧的 dashscope crate 删除后，引用它的文档、specs 历史与 `agentscope-guide` skill 的清理范围。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 工作区 MUST 移除并删除 `agent_scope_dashscope` 实现 crate，包括 workspace members、根 manifest 及所有示例 manifest 中的引用。
- **FR-002**: 系统 MUST 通过第三方 LLM 框架（rig）完成模型提供商接入，取代自研 provider crate 中的 HTTP 请求、SSE 流解析、请求/响应构造与格式转换代码。
- **FR-003**: 用户 MUST 能以与当前相当的开销（API key + 模型名 + 可选的流式开关）创建聊天模型，并将其用于 agent（`AgentConfig::model`）。
- **FR-004**: 新接入 MUST 保持现有流式事件协议、工具调用/结果生命周期、结构化输出、错误分类与重试语义的外部可观察行为；任何偏差 MUST 作为已知偏差记录。
- **FR-005**: 模型抽象层 MUST 保留现有 `ChatModel`/`EmbeddingModel` trait 作为公共 API（与 Python 参考实现兼容）；rig 作为 provider 实现层，通过新建的 rig-backed provider 实现这些 trait。
- **FR-006**: 新 provider 层 MUST 支持 Anthropic、OpenAI、DeepSeek 三家服务商（均为 rig 原生支持）；项目示例中原使用 DashScope 的地方 MUST 统一改用 OpenAI provider。
- **FR-007**: 新接入 MUST 完整保留现有能力面：流式响应、工具调用、结构化输出、thinking（在 provider 支持时）、embedding；不支持的 provider 特有能力 MUST 记录为已知限制而非静默降级。
- **FR-008**: 现有 7 个示例（agent、chat、human-in-the-loop、plan-react-agent、quickstart、rag、subagent）MUST 迁移到新接入方式并可编译运行。
- **FR-009**: 用户文档（README、`docs/rust/zh`、`agentscope-guide` skill、相关 specs 中的过时引用）MUST 同步更新为新的 provider 接入方式。
- **FR-010**: 兼容性验证 MUST 使用确定性组件（mock/recorded model、固定工具、固定 clock），MUST NOT 仅依赖真实 LLM 的自然语言输出来判定兼容性。
- **FR-011**: 新引入的 rig 依赖 MUST 满足依赖治理要求：锁定版本、license/维护/安全评估、无重复依赖、不引入循环依赖。

### Key Entities *(include if feature involves data)*

- **Provider 配置**：接入 LLM 所需的 api_key、base_url、模型名、生成参数（temperature/top_p 等）与能力开关（streaming、thinking 等）。迁移后由 rig 的配置机制承载，但对外暴露的构造入口与默认值需与现状保持等价。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 工作区 `cargo build --workspace`、`cargo test`、`cargo clippy`、`cargo fmt --check` 全部通过，且代码与 manifests 中无 `agent_scope_dashscope` 残留引用。
- **SC-002**: 全部 7 个受影响示例可编译（示例统一使用 OpenAI provider）；使用确定性（mock/recorded）模型运行的 agent 测试通过，流式事件顺序、工具调用、结构化输出与迁移前基线一致（或偏差已登记）。
- **SC-003**: 文档更新后，用户按新文档仅凭 API key 与模型名即可在 5 分钟内跑通 quickstart 示例。
- **SC-004**: 兼容性矩阵/文档记录 OpenAI/Anthropic/DeepSeek 三个 provider 的能力覆盖与已知偏差，不存在未登记的 `UnsupportedFeature`。

## Assumptions

- 迁移后主服务商为 OpenAI（示例统一改用 OpenAI provider）；Anthropic 与 DeepSeek 作为 rig 原生支持的 provider 一并接入。
- rig 版本锁定为当时最新稳定版（调研时为 0.42.0），并纳入上游版本锁定的依赖基线记录。
- 模型抽象层保留现有 `ChatModel`/`EmbeddingModel` trait 作为公共 API，rig 作为 provider 实现层。
- 能力覆盖以完整保留为原则：流式、工具调用、结构化输出、thinking、embedding 均须可用；三家 provider 中仅 OpenAI 提供 embedding。
- `pi-rust` 已移出主工作树，不在本次替换范围内。
- DashScope 特有能力（如 enable_search、qwen 专用参数）不迁移到新 provider，作为 provider 差异记录为已知限制。
- 本次范围不含 `agent_scope_model`/`agent_scope_embedding` 中 provider 无关辅助模块（formatter、json_repair、schema_flat 等）的清理；如需简化留待后续 feature。

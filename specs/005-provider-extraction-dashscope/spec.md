# Feature Specification: Provider 剥离与 DashScope 优先实现

**Feature Branch**: `005-provider-extraction-dashscope`

**Created**: 2026-07-29

**Status**: Draft

**Input**: User description: "具体的模型服务商暂时不要实现，后续要实现具体的应该另外起一个crate。并且优先实现dashscope,已实现的openai移除掉。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 核心 Crate 脱耦：移除嵌入式 Provider 实现 (Priority: P1)

作为一个框架维护者，我需要将当前嵌入在 `agent_scope_model` 核心 crate 中的 OpenAI Provider 实现（`openai/` 子模块）彻底移除，使核心 crate 不包含任何具体模型服务商的代码和 HTTP 依赖。这样做之后，核心 crate 只定义 `ChatModel` trait 等抽象接口，不再依赖 `reqwest` 等 HTTP 库。

**Why this priority**: 当前 `agent_scope_model/src/openai/` 包含约 1040 行具体 Provider 代码（model.rs、formatter.rs、parameters.rs），且 `Cargo.toml` 依赖了 `reqwest`、`tokio-stream`、`serde_yaml` 等具体实现库。这违反了分层架构原则——核心抽象层不应依赖具体厂商实现。此问题是后续所有 Provider 工作的前置条件。

**Independent Test**: `cargo build -p agent_scope_model` 编译通过，`cargo tree -p agent_scope_model` 确认依赖树中不包含 `reqwest`、`openai` 相关符号，且所有核心层原有测试（56 个）保持通过。

**Acceptance Scenarios**:

1. **Given** `agent_scope_model` 当前内嵌 `openai/` 子模块（model.rs、formatter.rs、parameters.rs）并依赖 `reqwest`，**When** 将 `openai/` 子模块从 crate 中删除，**Then** `cargo build -p agent_scope_model` 仍然成功，核心层独立编译通过
2. **Given** 移除 `openai/` 子模块后，**When** 检查 `Cargo.toml`，**Then** `agent_scope_model` 不再依赖 `reqwest`、`tokio-stream`、`tokio-util`（如果仅 OpenAI 使用）、`serde_yaml`（如果 YAML 解析被上移至 Provider 层）
3. **Given** 移除了 OpenAI Provider 的核心 crate，**When** 运行 `cargo test -p agent_scope_model`，**Then** 所有不依赖 OpenAI 类型的测试（如 `chat_response_integration.rs`、`cross_crate_tests.rs`）仍然通过

---

### User Story 2 - DashScope Provider 优先实现 (Priority: P1)

作为一个 AgentScope Rust 用户，我需要通过 DashScope（阿里云百炼平台）调用通义千问（Qwen）系列大模型，包括文本对话、流式输出和工具调用功能。DashScope 作为独立 crate（`agent_scope_dashscope`）实现 `ChatModel` trait，可独立编译、测试、版本发布。

**Why this priority**: 阿里云百炼是国内最主要的大模型 API 平台之一，对国内用户网络可达性和中文支持优于 OpenAI。用户明确要求"优先实现 dashscope"。同时，DashScope 作为 Provider crate 拆分的第一个实际案例，验证了 Provider crate 架构的可行性。

**Independent Test**: 创建 `agent_scope_dashscope` crate，实现 `ChatModel` trait。通过 mock HTTP 服务器验证：非流式调用返回 `ChatResponse`、流式调用返回 `Stream`、工具调用正确格式化。无需真实 API Key 即可运行全部测试。

**Acceptance Scenarios**:

1. **Given** DashScope API Key 和 `agent_scope_dashscope` crate，**When** 调用 `DashScopeChatModel::call()` 传入文本消息，**Then** 返回包含模型回复的 `ChatResponse`
2. **Given** 开启流式模式，**When** 调用 `call()` 方法，**Then** 返回 `ModelCallResult::Stream`，流式 chunk 可通过 `StreamAccumulator` 累积为完整 `ChatResponse`
3. **Given** 包含工具定义的调用请求，**When** 通过 DashScope Provider 发送，**Then** 工具定义正确转换为 DashScope API 格式，工具调用结果正确解析为 `ToolCallBlock`
4. **Given** JSON Schema 描述的结构化输出需求，**When** 调用 `generate_structured_output()`，**Then** DashScope Provider 通过工具调用机制返回符合 schema 的结构化数据

---

### User Story 3 - Provider 通用测试基础设施 (Priority: P2)

作为一个后续将要开发更多 Provider 的开发者，我需要一套可复用的 mock HTTP 测试模式，使为新 Provider 编写测试时无需从零搭建 mock 环境。

**Why this priority**: 每个 Provider 都需要 mock HTTP 测试、SSE 流解析测试、错误处理测试。将通用测试工具沉淀为共享模块可降低新增 Provider 的开发成本。此项为 P2，不阻塞 P1 交付。

**Independent Test**: 能够用相同的 mock 辅助函数/宏为不同 Provider 编写测试，验证 StreamAccumulator 行为、SSE 解析、错误映射。

**Acceptance Scenarios**:

1. **Given** 共享的测试辅助模块，**When** 编写 DashScope Provider 的流式响应测试，**Then** 可使用通用 SSE mock 工具模拟 API 响应
2. **Given** 通用的错误响应 mock 工具，**When** 测试各种 HTTP 错误码（401/429/500），**Then** 错误被正确转换为 `ModelError` 枚举变体

---

### Edge Cases

- 当 Provider crate 的 `ChatModel` trait 版本与核心 crate 不一致时，编译期如何检测到大版本不兼容？
- DashScope API 的流式 SSE 响应中，仅含 `usage` 的最终 chunk 可能 `choices` 为空数组——解析器需正确处理而非 panic
- DashScope 部分模型不支持 `tool_choice: "required"`——传入时应返回 `UnsupportedFeature` 而非静默降级
- DashScope 的错误响应格式可能为 OpenAI 兼容嵌套格式 `{"error": {"message": "..."}}` 或百炼自身扁平格式 `{"code": "...", "message": "..."}`——解析器需兼容两种
- `enable_thinking` 与 `tool_choice: "required"` 互斥——同时传入时需明确拒绝

---

## Requirements *(mandatory)*

### Functional Requirements

#### Part A: 架构脱耦

- **FR-001**: `agent_scope_model` crate MUST NOT 包含任何具体 Provider 的实现代码（即 MUST NOT 存在 `openai/` 子模块或类似模块）
- **FR-002**: `agent_scope_model` crate MUST NOT 直接依赖任何 HTTP 客户端库（如 `reqwest`），除非该依赖被核心抽象（如 Stream trait）所需
- **FR-003**: 每个具体 Provider MUST 实现在独立的 crate 中，crate 命名规范为 `agent_scope_<provider>`（小写、下划线分隔）
- **FR-004**: Provider crate MUST 通过实现 `ChatModel` trait 接入框架，MUST NOT 修改核心 crate 的任何代码
- **FR-005**: 已嵌入的 OpenAI `openai/` 子模块 MUST 从 `agent_scope_model/src/` 中移除，其代码 MUST NOT 残留于核心 crate
- **FR-006**: 移除 `openai/` 子模块后，`agent_scope_model` 的 `lib.rs` MUST 移除 `pub mod openai` 声明及相关 re-export（`OpenAIChatModel`、`OpenAIChatFormatter`、`OpenAIChatParameters`、`ReasoningEffort`）
- **FR-007**: 核心 crate 中仅被 OpenAI 子模块使用的依赖（`serde_yaml`、`tokio-stream`、`tokio-util`，如果无其他使用者）MUST 从 `agent_scope_model/Cargo.toml` 中移除

#### Part B: DashScope 实现

- **FR-008**: `agent_scope_dashscope` crate MUST 实现 `ChatModel` trait 的全部必需方法
- **FR-009**: DashScope Provider MUST 支持通过 `Authorization: Bearer <api_key>` 请求头认证
- **FR-010**: DashScope Provider MUST 支持配置 `base_url`，默认值指向 DashScope OpenAI 兼容端点
- **FR-011**: DashScope Provider MUST 支持非流式文本对话，返回 `ModelCallResult::Complete(ChatResponse)`
- **FR-012**: DashScope Provider MUST 支持流式对话（SSE），返回 `ModelCallResult::Stream`
- **FR-013**: DashScope Provider MUST 支持工具调用（Function Calling），包括工具定义传递和结果解析
- **FR-014**: DashScope Provider MUST 支持结构化输出，通过工具调用机制实现
- **FR-015**: DashScope Provider MUST 实现 `retryable_errors()`，将 HTTP 429/500/502/503 及网络超时归类为可重试
- **FR-016**: DashScope Provider MUST 正确处理 DashScope 错误响应，兼容 OpenAI 嵌套格式和百炼扁平格式
- **FR-017**: DashScope Provider MUST 正确处理 SSE 流中空 `choices` 数组的 chunk（仅含 usage 信息）

#### Part C: 测试基础设施

- **FR-018**: 项目 SHOULD 提供可复用的 mock HTTP 测试辅助工具，供所有 Provider crate 在测试中使用
- **FR-019**: 每个 Provider crate MUST 包含 mock HTTP 测试，验证请求格式和响应解析正确性
- **FR-020**: 所有 Provider crate 的 `cargo test` MUST 可在无网络环境下运行（全部使用 mock HTTP）

### Key Entities

- **Provider Crate**: 独立的 Rust crate（如 `agent_scope_dashscope`），通过工作空间（workspace）的 `crates/*` glob 自动注册。包含：`model.rs`（`ChatModel` trait 实现）、`formatter.rs`（`Formatter` trait 实现）、`parameters.rs`（模型参数定义）、`tests/`（mock HTTP 测试）。对外暴露一个 `XxxChatModel` struct 和相关配置类型。

- **DashScopeChatModel**: `ChatModel` trait 的具体实现。封装 DashScope HTTP API 的认证、请求构建、流式 SSE 解析、错误处理。核心字段包含 `api_key`、`base_url`、`model_name`、`parameters`、`stream`。

- **DashScopeParameters**: 模型参数的配置 struct。包含 `max_tokens`、`temperature`、`top_p`、`top_k`、`enable_search`（百炼特有：联网搜索）、`enable_thinking`（思考模式）、`repetition_penalty` 等参数。

- **DashScopeFormatter**: 将 AgentScope `Msg` 转换为 DashScope API 请求 JSON 的 `Formatter` trait 实现。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `agent_scope_model` 在移除 `openai/` 子模块和 `reqwest` 等 HTTP 依赖后，编译通过且所有核心测试（56 个，不含 OpenAI-specific 测试）通过
- **SC-002**: `agent_scope_dashscope` crate 独立编译通过，所有 mock HTTP 测试通过，测试数不少于 10 个
- **SC-003**: DashScope Provider 的流式响应通过 `StreamAccumulator` 可正确累积为完整的 `ChatResponse`
- **SC-004**: 新增 Provider 只需实现 `ChatModel` trait 和 `Formatter` trait，无需修改 `agent_scope_model` 核心 crate
- **SC-005**: 所有 Provider crate 的 `cargo test` 可在无网络环境下运行
- **SC-006**: `cargo clippy --workspace` 和 `cargo fmt --all -- --check` 在所有 crate 上无警告

## Assumptions

- DashScope API 的 `/compatible-mode/v1` 端点与 OpenAI Chat Completions API 格式基本对齐（messages、model、stream、tools、tool_choice 字段语义一致），差异在于百炼特有参数（`enable_search`、`repetition_penalty`、`enable_thinking`）和错误响应格式
- DashScope 不原生支持 `structured_output` 响应格式（`response_format: {"type": "json_schema", ...}`），结构化输出将通过通用 tool-calling 机制实现
- 各 Provider crate 之间通过 Cargo workspace 管理，共同依赖（`reqwest`、`serde_json`）由 Cargo 的依赖解析自然处理，不产生版本冲突
- 用户了解 DashScope 的网络可达性取决于其网络环境（公网访问或阿里云 VPC 内网访问），框架不负责网络配置
- 被移除的 OpenAI 代码暂存于 Git 历史中（通过 `git rm` 或直接删除），若后续需要可作为独立 crate 恢复。当前阶段不创建 `agent_scope_openai` crate，仅清理核心 crate
- 现有的 `tests/formatter_integration.rs` 如果直接引用 OpenAI 类型，将随 `openai/` 子模块的删除而移除

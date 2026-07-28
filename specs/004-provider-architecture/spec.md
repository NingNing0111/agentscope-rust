# Feature Specification: Provider Architecture & DashScope Integration

**Feature Branch**: `004-provider-architecture`

**Created**: 2026-07-28

**Status**: Draft

**Input**: User description: "具体的模型服务商暂时不要实现，后续要实现具体的应该另外起一个crate。并且优先实现dashscope"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Provider Crate 拆分与独立部署 (Priority: P1)

作为一个框架维护者，我需要将具体的模型服务商（Provider）实现从 `agent_scope_model` 核心 crate 中分离到独立的 crate 中，使得每个 Provider 可以独立编译、测试、发布和版本管理，不污染核心抽象层的依赖树。

**Why this priority**: 当前 `agent_scope_model` 内嵌了 `openai/` 子模块，导致核心 crate 依赖了 `reqwest`、HTTP 相关等具体实现细节。这违反了宪章第十一条（分层与依赖方向）——核心抽象 MUST NOT 依赖具体厂商实现。拆分后核心 crate 保持纯净，Provider crate 各自管理自身依赖。

**Independent Test**: 将 OpenAI 子模块从 `agent_scope_model` 中移出，创建独立的 `agent_scope_openai` crate（实现 `ChatModel` trait），验证 `agent_scope_model` 的依赖树不再包含 `reqwest`，且集成测试通过。

**Acceptance Scenarios**:

1. **Given** `agent_scope_model` 当前内嵌 `openai/` 子模块和 `reqwest` 依赖，**When** 将 OpenAI 实现提取为独立 crate `agent_scope_openai`，**Then** `agent_scope_model` 的 `Cargo.toml` 不再依赖 `reqwest`，`cargo build -p agent_scope_model` 依然成功
2. **Given** 已拆分的 `agent_scope_openai` crate，**When** 用户在其 `Cargo.toml` 中添加 `agent_scope_openai` 依赖，**Then** 可以通过 `use agent_scope_openai::OpenAIChatModel` 正常使用 OpenAI Provider
3. **Given** Provider 拆分后的架构，**When** 新增一个 Provider（如 DashScope），**Then** 只需创建新 crate 并实现 `ChatModel` trait，无需修改核心 crate

---

### User Story 2 - DashScope Provider 实现 (Priority: P1)

作为一个 AgentScope Rust 用户，我需要通过统一的 `ChatModel` trait 接口调用阿里云百炼（DashScope）平台的大模型，包括普通文本对话、流式输出、工具调用和结构化输出功能，使得在中国大陆网络环境下无需 VPN 即可使用大模型服务。

**Why this priority**: 阿里云百炼是国内最主要的大模型 API 平台之一，支持通义千问（Qwen）系列模型。对国内用户而言，DashScope 的网络可达性和中文支持优于 OpenAI。优先实现 DashScope 符合项目的实际用户需求。

**Independent Test**: 创建 `agent_scope_dashscope` crate，实现 `ChatModel` trait，通过 mock HTTP 服务器验证：非流式调用返回 `ModelCallResult::Complete(ChatResponse)`，流式调用返回 `ModelCallResult::Stream(...)`，工具调用正确格式化，结构化输出正确注入 schema。

**Acceptance Scenarios**:

1. **Given** DashScope API Key 和 `agent_scope_dashscope` crate，**When** 调用 `DashScopeChatModel::call()` 传入文本消息，**Then** 返回包含模型回复的 `ChatResponse`，格式与 OpenAI Provider 行为一致
2. **Given** 开启流式模式的 DashScope Provider，**When** 调用 `call()` 方法，**Then** 返回 `ModelCallResult::Stream`，流式 chunk 通过 `StreamAccumulator` 可正确累积
3. **Given** 包含工具定义的调用请求，**When** 通过 DashScope Provider 发送，**Then** 工具定义正确转换为 DashScope API 格式，工具调用结果正确解析为 `ToolCallBlock`
4. **Given** JSON Schema 描述的结构化输出需求，**When** 调用 `generate_structured_output()`，**Then** DashScope Provider 通过工具调用机制返回符合 schema 的结构化数据

---

### User Story 3 - Provider 通用测试基础设施 (Priority: P2)

作为一个 Provider 开发者，我需要一套可复用的测试工具（如 mock HTTP server、录制/回放机制），使得为新 Provider 编写兼容性测试时无需重复搭建测试环境。

**Why this priority**: 每个 Provider 都需要做 mock HTTP 测试、流式 SSE 解析测试、错误处理测试。将通用测试工具抽取为共享模块可显著降低新增 Provider 的开发成本。但此项不阻塞 P1 交付，可在 DashScope 完成后提取。

**Independent Test**: 创建一个测试辅助 crate（`agent_scope_test_utils` 或类似），提供 `MockHttpServer`、`RecordedResponse` 等工具，验证 DashScope 和 OpenAI 的测试代码都可以复用这些工具。

**Acceptance Scenarios**:

1. **Given** 测试辅助 crate，**When** 编写 DashScope Provider 的流式响应测试，**Then** 可使用通用 SSE mock 工具模拟 API 响应，无需手动构造 HTTP 字节流
2. **Given** 录制-回放工具，**When** 录制一次真实的 DashScope API 调用，**Then** 后续离线测试可回放该录制内容，验证解析逻辑不变

---

### User Story 4 - Provider 注册与发现机制 (Priority: P3)

作为一个应用开发者，我需要一种机制在运行时根据配置选择使用哪个 Provider（如通过配置文件或环境变量指定），而不是在代码中硬编码 Provider 类型。

**Why this priority**: 这是易用性优化。用户可以手动 `use agent_scope_dashscope::DashScopeChatModel` 并用 `Box::new(...)` 构造。但提供工厂/注册机制可以简化配置驱动的 Provider 切换。此项为 P3，基础 trait 和 Provider crate 拆分是更前置的需求。

**Independent Test**: 通过配置文件指定 `provider: "dashscope"`，运行时自动构造对应的 `Box<dyn ChatModel>` 实例。

**Acceptance Scenarios**:

1. **Given** 配置 `{"model": {"provider": "dashscope", "model_name": "qwen-plus"}}`，**When** 运行时加载此配置，**Then** 自动创建 `DashScopeChatModel` 实例
2. **Given** 未知的 provider 名称，**When** 加载配置，**Then** 返回明确的 `ConfigError` 而非 panic

---

### Edge Cases

- 当 Provider crate 的版本与核心 `agent_scope_model` 版本不兼容时，编译期如何检测？
- 当两个 Provider crate 依赖同一个 HTTP 库的不同大版本时，如何避免依赖冲突？
- DashScope API 的流式响应格式（SSE）与 OpenAI 是否完全兼容？若不兼容，差异在何处？
- DashScope 的 tool_choice 参数是否支持 `"required"` 模式？（当前已知百炼部分模型仅支持 `"auto"` 和 `"none"`）
- DashScope 的 token 计数 API 是独立端点，与 OpenAI 的模型内返回不同——如何在 `count_tokens()` 默认实现中处理？

---

## Requirements *(mandatory)*

### Functional Requirements

#### Part A: 架构拆分

- **FR-001**: `agent_scope_model` crate MUST NOT 直接依赖任何 HTTP 客户端库（如 `reqwest`）
- **FR-002**: `agent_scope_model` crate MUST NOT 包含任何具体 Provider 的实现代码（如 `openai/` 子模块）
- **FR-003**: 每个具体 Provider MUST 实现在独立的 crate 中（如 `agent_scope_openai`、`agent_scope_dashscope`），通过实现 `ChatModel` trait 接入框架
- **FR-004**: Provider crate 的命名规范 MUST 为 `agent_scope_<provider>`（小写、下划线分隔）
- **FR-005**: 已实现的 OpenAI Provider MUST 从 `agent_scope_model` 中提取到 `agent_scope_openai` crate，保持所有现有测试通过
- **FR-006**: 提取后的 `agent_scope_openai` crate MUST 保持与提取前完全相同的公开 API 和行为

#### Part B: DashScope 实现

- **FR-007**: `agent_scope_dashscope` crate MUST 实现 `ChatModel` trait 的全部必需方法
- **FR-008**: DashScope Provider MUST 支持通过 API Key 认证，使用 `Authorization: Bearer <api_key>` 请求头
- **FR-009**: DashScope Provider MUST 支持配置 `base_url`（默认 `https://dashscope.aliyuncs.com/compatible-mode/v1`）
- **FR-010**: DashScope Provider MUST 支持文本对话（非流式），返回 `ModelCallResult::Complete(ChatResponse)`
- **FR-011**: DashScope Provider MUST 支持流式对话（SSE），返回 `ModelCallResult::Stream`
- **FR-012**: DashScope Provider MUST 支持 `stream_options: {"include_usage": true}` 以获取流式 token 统计
- **FR-013**: DashScope Provider MUST 支持工具调用（Function Calling），包括工具定义传递和工具调用结果解析
- **FR-014**: DashScope Provider 的 `generate_structured_output()` MUST 通过工具调用机制（注入 `generate_structured_output` tool）实现
- **FR-015**: DashScope Provider MUST 实现 `retryable_errors()`，将 HTTP 429/500/502/503 以及网络超时归类为可重试
- **FR-016**: DashScope Provider MUST 实现 `count_tokens()` 的精确版本（通过 DashScope tokenizer API 端点，如可用），否则 fallback 到 byte/4 启发式算法
- **FR-017**: DashScope Provider MUST 正确格式化消息为 DashScope API 兼容的 JSON 格式（与 OpenAI Chat Completions API 兼容模式对齐）
- **FR-018**: DashScope Provider MUST 正确解析 DashScope 的流式 SSE 响应，包括 `data:` 行解析、`[DONE]` 结束标记处理
- **FR-019**: DashScope Provider MUST 正确处理 DashScope 的错误响应格式，将其转换为 `ModelError` 的对应变体

#### Part C: 测试基础设施

- **FR-020**: 项目 SHOULD 提供可复用的 mock HTTP 测试工具，供所有 Provider crate 使用
- **FR-021**: 每个 Provider crate MUST 包含 mock HTTP 测试，验证请求格式和响应解析的正确性

### Key Entities

- **Provider Crate**: 独立的 Rust crate，实现 `ChatModel` trait，包含 Formatter、Parameters、请求构建和响应解析逻辑。对外仅暴露一个 `XxxChatModel` struct 和相关配置类型。
- **DashScopeChatModel**: `ChatModel` trait 的具体实现，封装 DashScope HTTP API 的认证、请求构建、流式解析、错误处理。
- **DashScopeParameters**: DashScope 模型参数的配置 struct，包含 `temperature`、`max_tokens`、`top_p`、`enable_search`（百炼特有：联网搜索增强）、`repetition_penalty` 等。
- **DashScopeFormatter**: 将 AgentScope `Msg` 转换为 DashScope API 格式的 Formatter 实现。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `agent_scope_model` crate 在移除 `openai/` 子模块和 `reqwest` 依赖后，编译通过且所有 56 个核心测试通过
- **SC-002**: `agent_scope_openai` crate 独立编译通过，所有原有 OpenAI 测试（10 个）保持通过
- **SC-003**: `agent_scope_dashscope` crate 通过 mock HTTP 测试覆盖：非流式调用、流式调用、工具调用、结构化输出、错误处理——至少 10 个独立测试
- **SC-004**: DashScope Provider 的流式响应通过 `StreamAccumulator` 可正确累积为完整的 `ChatResponse`，与 OpenAI Provider 的行为一致（相同 API 格式）
- **SC-005**: 新增 Provider 只需实现 `ChatModel` trait 和 `Formatter` trait，无需修改 `agent_scope_model` 核心 crate
- **SC-006**: 所有 Provider crate 的 `cargo test` 可在无网络环境下运行（全部使用 mock HTTP）

## Assumptions

- DashScope API 的 `/compatible-mode/v1` 端点与 OpenAI Chat Completions API 格式基本对齐（messages、model、stream、tools、tool_choice 字段语义一致），差异在于特定参数（如 `enable_search`、`repetition_penalty`）和错误响应格式
- DashScope 不原生支持 `structured_output` 响应格式（`response_format: {"type": "json_schema", ...}`），因此结构化输出通过通用 tool-calling 机制实现（与 OpenAI 在不原生支持 structured output 的模型上策略一致）
- Provider crate 之间的共同依赖（`reqwest`、`tokio`、`serde_json` 基础类型）不会导致版本冲突，Cargo 的依赖解析可自然处理
- `agent_scope_test_utils`（如果创建）作为 `[dev-dependencies]` 被各 Provider crate 引用，不影响生产依赖树
- 用户了解 DashScope 的网络可达性取决于其网络环境（公网访问或阿里云 VPC 内网访问），框架不负责网络配置
- 第一个 Provider 拆分（OpenAI）完成后，其拆分模式（crate 结构、`Cargo.toml` 模板、测试方式）将作为后续 Provider 的参考模板

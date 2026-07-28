# Feature Specification: Tool System — 最小可行实现

**Feature Branch**: `006-tool-system`

**Created**: 2026-07-29

**Status**: Draft

**Input**: User description: "实现 Tool System，对齐上游 AgentScope Python 的 Tool 设计，声明式注册 + FunctionTool 适配器 + 最小可行性"

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Tool trait 定义与 FunctionTool 适配器 (Priority: P1)

作为一个 Rust 开发者，我需要将一个普通 Rust 函数包装为实现了 `Tool` trait 的对象，自动提取函数参数的 JSON Schema（via `schemars`），并在 Tool 执行时自动将返回值转换为 `ToolChunk`。

**Why this priority**: `Tool` trait 是整个 Tool System 的核心抽象，`FunctionTool` 是最常用的适配器。没有它们，后续 `ToolKit` 和其他工具类型都无从谈起。

**Independent Test**: 定义一个带 `#[derive(JsonSchema)]` 的输入结构体，用 `FunctionTool::new()` 包装一个 async handler，验证 `tool.name()`、`tool.description()`、`tool.input_schema()` 返回预期值，且 `tool.call(json_input)` 返回 `ToolOutput::Complete(chunk)`。

**Acceptance Scenarios**:

1. **Given** 一个 `#[derive(JsonSchema)]` 的输入结构体和 async handler 函数，**When** 通过 `FunctionTool::new("my_tool", "描述", handler)` 创建，**Then** `input_schema()` 返回与 `schemars` 自动生成的 schema 一致的 JSON
2. **Given** 一个 `FunctionTool` 实例，**When** 调用 `tool.call(valid_json_input)`，**Then** 返回 `Ok(ToolOutput::Complete(chunk))`，chunk 的 output 包含 handler 返回的文本内容
3. **Given** handler 返回 `String` 类型，**When** `call()` 执行完成，**Then** `chunk.output` 为 `ToolOutput::Text("content")`，`chunk.state` 为 `ToolResultState::Success`
4. **Given** handler 返回 `ToolChunk` 类型，**When** `call()` 执行完成，**Then** chunk 直接透传

---

### User Story 2 — ToolKit 注册与管理 (Priority: P1)

作为一个智能体开发者，我需要将多个 `Tool` 实例注册到一个 `ToolKit` 中，通过它导出 OpenAI 兼容的 function schema 列表（喂给 `ChatModel`），并在收到模型的 `ToolCallBlock` 后执行对应 Tool。

**Why this priority**: `ToolKit` 是连接 Tool 和 ChatModel 的桥梁——Tool 的注册、发现、schema 导出、调用分发都通过它完成。无此组件则 Tool 无法在 Agent 流程中使用。

**Independent Test**: 创建 `ToolKit`，注册 2 个 `FunctionTool`，调用 `get_tool_schemas()` 验证输出为正确的 OpenAI function schema 格式，通过 `call_tool(tool_call_block)` 执行指定 Tool 并验证结果。

**Acceptance Scenarios**:

1. **Given** `ToolKit` 空实例，**When** 注册 2 个 Tool，**Then** `get_tool_schemas()` 返回 2 个 schema 条目
2. **Given** 已注册的 Toolkit，**When** 调用 `get_tool_schemas()`，**Then** 输出格式为 `[{"type": "function", "function": {"name": "...", "description": "...", "parameters": {...}}}, ...]`
3. **Given** 已注册的 Toolkit 和 `ToolCallBlock { name: "search", input: "{\"q\":\"test\"}" }`，**When** 调用 `call_tool(tool_call)`，**Then** 返回 `Ok(ToolOutput::Complete(chunk))`
4. **Given** Toolkit 中不存在名为 `unknown_tool` 的 Tool，**When** 调用 `call_tool(unknown)`，**Then** 返回 `Err(ToolError::NotFound { name: "unknown_tool" })`
5. **Given** 注册时 Tool 名重复，**When** 调用 `register(new_tool)`，**Then** 新 Tool 覆盖旧 Tool（与 Python 行为一致）

---

### User Story 3 — Tool → ChatModel 集成验证 (Priority: P2)

作为一个框架用户，我需要验证 Tool System 可以与现有的 `ChatModel` trait 无缝协作——ToolKit 导出的 schema 可以直接作为 `ChatModel::call()` 的 `tools` 参数使用，模型返回的 `ToolCallBlock` 可以被 Toolkit 正确执行。

**Why this priority**: 这是端到端验证——确保 Tool System 的接口设计与 Model API 对齐。不通过 mock ChatModel 验证这一点，未来 Agent Layer 集成时会有隐藏的不匹配问题。

**Independent Test**: 用 `agent_scope_model` 中的 mock `ChatModel`（test 中已有的 `TestModel`），传入 Toolkit 生成的 schemas，验证 tools 参数格式兼容性，以及 ToolCallBlock → Toolkit::call_tool 的闭环。

**Acceptance Scenarios**:

1. **Given** Toolkit 的 `get_tool_schemas()` 输出，**When** 作为 `ChatModel::call(msgs, Some(&schemas), Some(&tool_choice))` 的 tools 参数，**Then** `DashScopeChatModel::build_request_body()` 正确序列化（现有测试已覆盖 tool 序列化，仅需验证 schema 格式匹配）
2. **Given** Mock ChatModel 返回的 `ToolCallBlock`，**When** `ToolKit::call_tool(&block)` 执行，**Then** 返回对应 Tool 的执行结果

---

### Edge Cases

- handler 执行 panicked 时如何处理？→ `call()` 应 catch panic（via `std::panic::catch_unwind` 或 `AssertUnwindSafe`），转为 `ToolError::Execution`
- `input: JsonValue` 无法反序列化为目标类型时，返回 `ToolError::InvalidInput`
- `Stream` 类型的 `ToolOutput`：ToolKit 不自动累积，由调用方（Agent）负责消费 stream
- 空的 `ToolKit`（零 Tool 注册）：`get_tool_schemas()` 返回空数组 `[]`
- `ToolChunk.is_last` 的语义：`Complete` 模式下始终为 `true`；`Stream` 模式下每个 chunk 由 Tool 实现自行设置

---

## Requirements *(mandatory)*

### Functional Requirements

#### Part A: Tool Trait

- **FR-001**: `agent_scope_tool` crate MUST 定义 `Tool` trait，包含 `name()`、`description()`、`input_schema()`、`is_concurrency_safe()`、`is_read_only()`、`call()` 方法
- **FR-002**: `input_schema()` MUST 返回 `serde_json::Value`，格式为 JSON Schema `{"type": "object", "properties": {...}, "required": [...]}`
- **FR-003**: `is_concurrency_safe()` 和 `is_read_only()` MUST 有默认实现（分别返回 `true` 和 `false`）
- **FR-004**: `ToolOutput` enum MUST 包含 `Complete(ToolChunk)` 和 `Stream(Pin<Box<dyn Stream<Item = Result<ToolChunk, ToolError>> + Send>>)` 两个变体，与 `ModelCallResult` 风格一致
- **FR-005**: `ToolError` enum MUST 包含 `NotFound`、`InvalidInput`、`Execution`、`Interrupted` 变体，实现 `Display` 和 `Error`

#### Part B: FunctionTool

- **FR-006**: `FunctionTool::new::<T: JsonSchema>(name, description, handler)` MUST 自动从泛型参数 `T` 提取 JSON Schema 作为 `input_schema()`
- **FR-007**: `FunctionTool::new_with_schema(name, description, schema, handler)` MUST 支持手动传入 schema（逃生舱）
- **FR-008**: `FunctionTool` 的 handler 返回值若为 `String`，MUST 自动转换为 `ToolChunk`（`ToolOutput::Text(s)`, `state: Success`, `is_last: true`）
- **FR-009**: `FunctionTool` 的 handler 返回值若为 `ToolChunk`，MUST 直接透传
- **FR-010**: handler 内部 panic 时 MUST 被捕获并转为 `ToolError::Execution`，而非传播 panic

#### Part C: ToolKit

- **FR-011**: `ToolKit::register(tool: impl Tool + 'static)` MUST 将 Tool 以 name 为 key 存储到内部 `HashMap`
- **FR-012**: `ToolKit::register()` 时若 name 重复 MUST 覆盖旧 Tool（与 Python 行为一致）
- **FR-013**: `ToolKit::get_tool_schemas()` MUST 返回 OpenAI 格式的 function schema 列表
- **FR-014**: `ToolKit::call_tool(tool_call: &ToolCallBlock)` MUST 根据 `tool_call.name` 查找 Tool，反序列化 `tool_call.input` 并调用 `tool.call()`
- **FR-015**: `call_tool()` 中若 Tool 不存在 MUST 返回 `ToolError::NotFound`
- **FR-016**: `call_tool()` 中若 input 反序列化失败 MUST 返回 `ToolError::InvalidInput`
- **FR-017**: `ToolKit::clear()` MUST 清空所有注册的 Tool

#### Part D: ToolResultBlock 扩展

- **FR-018**: `agent_scope_message` 中的 `ToolResultBlock` MUST 新增 `is_last: bool` 字段（`#[serde(default)]`，默认 `false` 以保持向后兼容）

### Key Entities

- **`Tool` trait**: 核心抽象，定义 Tool 的元数据（name、description、input_schema）和执行逻辑（call）。设计对齐上游 Python `ToolBase`。

- **`ToolOutput`**: 执行结果的枚举，`Complete`（一次性结果）和 `Stream`（流式增量）。与 `ModelCallResult` 同构。

- **`ToolChunk`**: `ToolResultBlock` 的类型别名。流式场景中多个 chunk 可累积为完整结果。`is_last` 标记流结束。

- **`ToolError`**: Tool 系统专用错误枚举，覆盖未找到、输入非法、执行失败、中断四种情况。

- **`FunctionTool`**: 将普通 async 函数适配为 `Tool` trait 实现。通过 `T: JsonSchema` 自动推导 schema，通过 `IntoChunk` trait 自动转换返回值。

- **`ToolKit`**: Tool 注册中心。管理 `name → Box<dyn Tool>` 映射，提供 schema 导出和调用分发。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `agent_scope_tool` crate 编译通过，无 warning
- **SC-002**: `Tool` trait 可用 `FunctionTool::new()` 创建，`input_schema()` 正确生成
- **SC-003**: `ToolKit::get_tool_schemas()` 输出与 Python `Toolkit.get_tool_schemas()` 格式一致
- **SC-004**: `ToolKit::call_tool()` 可通过 mock ToolCallBlock 执行已注册 Tool
- **SC-005**: 所有 test 可在无网络环境下运行（纯单元测试）
- **SC-006**: `cargo clippy --workspace` 和 `cargo fmt --all -- --check` 全通过

## Assumptions

- 第一阶段不实现 `ToolGroup`（分组管理）、`ToolMiddleware`（洋葱中间件）、`MCP`、`Skill` 集成——后续 Feature 扩展
- 第一阶段不实现 `Permission` 集成——`check_permissions()` 等方法留待 Permission 系统完成后添加
- `ToolChunk` 复用 `ToolResultBlock`，`is_last` 默认 `false`，`Complete` 模式下显式设为 `true`
- Tool 的 Stream 模式与 `ChatModel` 的 `StreamAccumulator` 可以协作（后续 Agent Layer 实现时验证）
- `agent_scope_tool` 依赖 `agent_scope_message`（ToolResultBlock）、`agent_scope_model`（ToolChoice 验证）、`schemars`（schema 生成）

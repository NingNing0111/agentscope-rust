# Feature Specification: AgentScope Foundation Layer

**Feature Branch**: `002-foundation-layer`

**Created**: 2026-07-28

**Status**: Draft

**Input**: User description: "实现 AgentScope Foundation 层：Message、Event、State、Types 核心数据协议"

## Clarifications

### Session 2026-07-28

- Q: Foundation 层如何解决对 Permission 类型（PermissionContext、PermissionRule，属于上层 permission 模块）的跨层依赖？ → A: Option A — 在 Foundation 层定义最小化占位类型（仅字段结构，无行为），后续由 permission 特性替换或扩展。
- Q: Foundation 层数据结构的验证失败应采用何种错误处理模式？ → A: Option A — 显式 `Result<T, ValidationError>` 返回，构造失败时返回错误由调用者决定处理，不 panic。
- Q: Foundation 层是否应定义 AgentState 的 context 规模管理策略？ → A: Option A — 在 Foundation 层定义可配置的 `max_context_length` 上限，超出时自动拒绝追加。

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 构建和传递消息 (Priority: P1)

开发者使用 AgentScope 构建智能体对话时，需要创建、修改和传递消息（Msg）。消息是智能体间通信的基本单元，必须支持 user、assistant、system 三种角色，以及文本、思考、工具调用、工具结果、二进制数据等多种内容块类型。

**Why this priority**: 消息是 AgentScope 中最基础的数据载体，所有上层能力（Agent 推理循环、Model 调用、Tool 执行、Memory 存储）都依赖于消息的创建和操作。没有正确的消息结构，整个框架无法运行。

**Independent Test**: 可通过创建不同角色的消息、添加各种内容块、调用消息方法（如 `get_content_blocks`、`get_text_content`、`has_content_blocks`）并验证 JSON 序列化/反序列化结果来独立测试。

**Acceptance Scenarios**:

1. **Given** 一个开发者，**When** 使用 `UserMsg` 工厂函数创建一条包含文本内容的消息，**Then** 消息具有 `role="user"`、正确的 name、content 包含 TextBlock、自动生成的 id 和 created_at 时间戳。
2. **Given** 一条 assistant 消息包含 text、tool_call、thinking 多个内容块，**When** 调用 `get_content_blocks("text")` 过滤，**Then** 仅返回 TextBlock 列表，不影响其他块。
3. **Given** 一条消息的 content 为 `[TextBlock(text="Hello")]`，**When** 调用 `get_text_content()`，**Then** 返回拼接后的字符串 `"Hello"`。
4. **Given** user 角色的消息，**When** 试图添加 tool_call 类型的 ContentBlock，**Then** 验证失败并抛出错误（user 消息只允许 text 和 data 块）。
5. **Given** system 角色的消息，**When** 试图添加 data 类型的 ContentBlock，**Then** 验证失败并抛出错误（system 消息只允许 text 块）。
6. **Given** 一条消息，**When** 序列化为 JSON 再反序列化，**Then** 所有字段值保持一致，包括 ContentBlock 的判别类型字段。

---

### User Story 2 - 流式事件驱动消息构建 (Priority: P1)

在智能体推理-行动循环中，Model 调用产生流式事件（Event），这些事件被逐步应用到 Msg 上以增量构建最终回复。事件系统需要覆盖完整的生命周期：回复开始/结束、模型调用开始/结束、各类内容块的流式增量（text/thinking/data/tool_call/tool_result）、用户确认/中断、外部执行等。

**Why this priority**: 事件系统是流式响应的核心机制，也是 Agent 状态变更的唯一信号通道。没有事件系统，Agent 无法实时反馈推理进度，也无法实现中断、确认等交互控制流。

**Independent Test**: 可通过模拟事件序列→应用 `append_event`→验证最终消息状态来独立测试，也可验证 `EventType` 枚举覆盖所有 27 种事件类型，并验证每种事件类的字段结构。

**Acceptance Scenarios**:

1. **Given** 一条空白的 assistant 消息，**When** 依次应用 `REPLY_START`、`TEXT_BLOCK_START`、`TEXT_BLOCK_DELTA("Hel")`、`TEXT_BLOCK_DELTA("lo")`、`TEXT_BLOCK_END`、`MODEL_CALL_END`、`REPLY_END` 事件序列，**Then** 消息 content 包含一个文本为 `"Hello"` 的 TextBlock，`finished_reason` 为 `COMPLETED`，usage 记录了 token 消耗。
2. **Given** 一条正在流式构建的消息，**When** 收到 `USER_INTERRUPT` 事件，**Then** 消息的 `finished_reason` 变为 `INTERRUPTED`。
3. **Given** 一个 `reply_id` 不匹配的事件，**When** 调用 `append_event`，**Then** 事件被跳过且记录警告日志。
4. **Given** 一个 `ToolResultEndEvent`，**When** 应用到消息上，**Then** 对应的 `ToolCallBlock` 的状态变为 `FINISHED`。
5. **Given** 流式 DataBlock 事件（多个 `DATA_BLOCK_DELTA`），**When** 依次应用，**Then** base64 编码的数据被正确拼接（而非简单字符串连接）。

---

### User Story 3 - 智能体状态管理与持久化 (Priority: P2)

Agent 的状态需要被保存、加载和跨 Session 恢复。AgentState 管理会话上下文（对话历史、回复状态、工具上下文、任务上下文、中间件上下文、权限上下文），确保 Agent 可以在跨请求的过程中保持一致的内部状态。

**Why this priority**: 状态管理是多轮对话和 Agent 持久化的基础。虽然优先级略低于消息和事件（状态本身依赖消息结构），但它对于生产环境中的会话管理和故障恢复至关重要。

**Independent Test**: 可通过创建 AgentState、填充 context（含多条 Msg）、设置 reply_context 和 tool_context，序列化到 JSON/字典再反序列化还原，验证所有嵌套结构完整性和向后兼容迁移逻辑来独立测试。

**Acceptance Scenarios**:

1. **Given** 一个新创建的 AgentState，**When** 调用 `append_context` 添加内容块，**Then** 如果 context 尾部存在同 name 同 reply_id 的 assistant 消息，则将块追加到该消息；否则创建新的 assistant 消息。
2. **Given** AgentState 的 context 中包含带有 `ASKING` 状态 ToolCallBlock 的消息，**When** 调用 `has_awaiting_tool_calls(name)`，**Then** 返回 `True`。
3. **Given** 旧格式的 AgentState 数据（顶层 `reply_id`/`cur_iter` 字段），**When** 反序列化加载，**Then** 自动迁移到 `reply_context` 嵌套结构中。
4. **Given** 一个 Task 对象，**When** 设置 state 为 "in_progress" 并指定 owner，**Then** 所有字段在序列化-反序列化后保持一致。
5. **Given** AgentState 的 tasks_context，**When** 添加多个 Task 并设置阻塞关系（blocks/blocked_by），**Then** 依赖关系可被正确查询。

---

### User Story 4 - 核心类型定义与错误模型 (Priority: P2)

框架需要统一的类型定义：回复结束原因（ReplyFinishedReason）、错误分类（ErrorType/ErrorInfo）、JSON 序列化类型别名（JSONPrimitive/JSONSerializableObject）、Hook 类型（AgentHookTypes/ReActAgentHookTypes）、Embedding 类型等。这些类型被 message、event、state 等多个模块共享引用。

**Why this priority**: 类型定义是跨模块的共享契约。它们没有自己的依赖，所以可以尽早实现，但单独来看不直接产生用户可见价值——它们的价值体现在被消息、事件等模块引用时。

**Independent Test**: 可通过验证每个枚举值的完整性和 JSON 序列化格式、验证 ErrorInfo 的字段结构、验证 JSONSerializableObject 类型别名的递归正确性来独立测试。

**Acceptance Scenarios**:

1. **Given** `ReplyFinishedReason` 枚举，**When** 检查其成员，**Then** 包含 `COMPLETED`、`INTERRUPTED`、`EXCEED_MAX_ITERS`、`ERROR` 四种值，每个值序列化为对应的蛇形字符串。
2. **Given** `ErrorType` 枚举，**When** 检查其成员，**Then** 包含 9 种错误分类：`AUTHENTICATION`、`PERMISSION`、`RATE_LIMIT`、`INVALID_REQUEST`、`UPSTREAM`、`CONNECTION`、`INTERNAL`、`UNKNOWN`，每种对应明确的 HTTP 状态码语义。
3. **Given** 一个 `ErrorInfo(type=ErrorType.RATE_LIMIT, message="Too many requests")`，**When** 序列化为 JSON，**Then** 输出 `{"type": "rate_limit", "message": "Too many requests"}`。
4. **Given** `AgentHookTypes` 类型别名，**When** 检查，**Then** 包含 6 个 hook 点：`pre_reply`、`post_reply`、`pre_print`、`post_print`、`pre_observe`、`post_observe`。

---

### User Story 5 - Foundation 层的零内部依赖拓扑 (Priority: P3)

Foundation 层（Message、Event、State、Types）作为整个 AgentScope 框架的最底层，必须形成零内部依赖的基础协议层。这意味着本层中的任何模块都不应依赖上层的 Model、Tool、Agent 等模块，从而保证后续实现可以按拓扑顺序进行。

**Why this priority**: 依赖拓扑的正确性影响整个项目的实现顺序。虽然它在功能层面不产生直接用户价值，但它是架构正确性的基础保障。按宪法要求，必须先实现基础协议才能构建上层能力。

**Independent Test**: 可通过静态分析依赖图验证——types 模块无任何 agentscope 内部依赖；message 仅依赖 types；event 仅依赖 message 和 types；state 仅依赖 message 和 types；四者之间不依赖 model/tool/agent 等任何上层模块。

**Acceptance Scenarios**:

1. **Given** types 模块的所有公开符号，**When** 分析其 import 来源，**Then** 不依赖 agentscope 中除 `_utils` 以外的任何其他模块。
2. **Given** message 模块的所有公开符号，**When** 分析其 import 来源，**Then** 仅依赖 types 和 `_utils` 模块。
3. **Given** event 模块的所有公开符号，**When** 分析其 import 来源，**Then** 仅依赖 message、types 和 `_utils` 模块。
4. **Given** state 模块的所有公开符号，**When** 分析其 import 来源，**Then** 仅依赖 message、types 和 `_utils` 模块。

---

### Edge Cases

- 当 ContentBlock 的 type 字面值与实际数据类不匹配时（如 type="text" 但实际是 ToolCallBlock 结构），序列化/反序列化应如何处理？
- DataBlock 的 source 字段同时支持 Base64Source 和 URLSource——当两者同时提供时应有明确的优先级或报错行为。
- ToolCallBlock 的 input 字段是原始 JSON 字符串——当该字符串不是合法 JSON 时，序列化行为应如何？
- Usage（token 统计）在多次 MODEL_CALL_END 事件中累加——若累加溢出应如何处理？
- AgentState 的 context 字段通过 `max_context_messages` 上限进行规模控制——当消息数量达到上限时，`append_context` 拒绝追加并返回错误。上层 Agent/Memory 模块应在达到上限前主动触发摘要压缩和截断。
- ReplyContext 的 structured_schema 在序列化时需将 Pydantic model 类转为 JSON Schema dict——若 schema 为空应如何处理？
- EventBase 的 metadata 字段为 `dict[str, Any]`——是否允许嵌套的非 JSON 可序列化对象？

## Requirements *(mandatory)*

### Functional Requirements

#### Message 模块

- **FR-001**: 系统 MUST 提供 `Msg` 数据结构，包含以下必填字段：`name`（字符串）、`content`（ContentBlock 列表）、`role`（"user"|"assistant"|"system"）、`id`（自动生成的唯一标识符）。
- **FR-002**: `Msg` MUST 包含以下可选元数据字段：`metadata`（字典）、`created_at`（ISO 8601 时间戳字符串）、`usage`（Usage 对象或 None）、`finished_at`（字符串或 None）、`finished_reason`（ReplyFinishedReason 或 None）、`structured_output`（字典或 None）、`error`（ErrorInfo 或 None）。
- **FR-003**: 系统 MUST 支持以下六种 ContentBlock 子类型：`TextBlock`、`ThinkingBlock`、`HintBlock`、`DataBlock`、`ToolCallBlock`、`ToolResultBlock`，每种包含判别字段 `type`（字面值）。
- **FR-004**: `TextBlock` MUST 包含 `type="text"`、`text`（字符串）、`id`、`created_at`、`finished_at` 字段。
- **FR-005**: `ThinkingBlock` MUST 包含 `type="thinking"`、`thinking`（字符串）、`id`、`created_at`、`finished_at` 字段，并允许额外 provider 特定字段透传。
- **FR-006**: `HintBlock` MUST 包含 `type="hint"`、`hint`（字符串或 TextBlock/DataBlock 列表）、`source`（字符串或 None）、`id`、`created_at`、`finished_at` 字段。
- **FR-007**: `DataBlock` MUST 包含 `type="data"`、`source`（Base64Source 或 URLSource 的联合类型）、`id`、`name`（可选）、`created_at`、`finished_at` 字段。
- **FR-008**: `Base64Source` MUST 包含 `type="base64"`、`data`（base64 编码字符串）、`media_type`（MIME 类型字符串）字段。
- **FR-009**: `URLSource` MUST 包含 `type="url"`、`url`（符合 RFC 3986 的 URI 字符串）、`media_type`（MIME 类型字符串）字段，序列化时 url 字段输出为字符串。
- **FR-010**: `ToolCallBlock` MUST 包含 `type="tool_call"`、`id`、`name`（工具名称）、`input`（原始 JSON 参数字符串）、`state`（ToolCallState 枚举）、`suggested_rules`、`created_at`、`finished_at` 字段。
- **FR-011**: `ToolCallState` MUST 为字符串枚举，包含 `PENDING`、`ASKING`、`ALLOWED`、`SUBMITTED`、`FINISHED` 五种状态，状态转换路径为：PENDING→ASKING→ALLOWED→SUBMITTED→FINISHED（各阶段均可直接终止为 FINISHED）。
- **FR-012**: `ToolResultBlock` MUST 包含 `type="tool_result"`、`id`、`name`（工具名称）、`output`（字符串或 TextBlock/DataBlock 列表）、`state`（ToolResultState 枚举）、`metadata`（字典）、`created_at`、`finished_at` 字段。
- **FR-013**: `ToolResultState` MUST 为字符串枚举，包含 `SUCCESS`、`ERROR`、`INTERRUPTED`、`DENIED`、`RUNNING` 五种状态。
- **FR-014**: `Msg` MUST 在构造时根据 role 验证 content blocks 的合法性——user 角色只允许 text 和 data 块，system 角色只允许 text 块，assistant 角色无限制。验证失败时 MUST 通过 `Result<T, ValidationError>` 返回错误，由调用者决定如何处理，不 panic。
- **FR-015**: `Msg` MUST 提供 `get_content_blocks(block_type)` 方法，支持按类型过滤内容块，接受单个类型字符串、类型列表或 None（返回全部）。
- **FR-016**: `Msg` MUST 提供 `get_text_content(separator)` 方法，将所有 TextBlock 的文本用指定分隔符拼接返回。
- **FR-017**: `Msg` MUST 提供 `has_content_blocks(block_type)` 方法，检查是否存在指定类型的内容块。
- **FR-018**: `Msg` MUST 提供 `append_event(event)` 方法，根据事件类型逐步构建消息内容——支持文本增量追加、工具调用状态变更、数据块拼接、token 用量累加、回复终止标记等全部事件类型的处理。
- **FR-019**: 系统 MUST 提供 `UserMsg()`、`AssistantMsg()`、`SystemMsg()` 三个工厂函数，分别创建 role 为 "user"、"assistant"、"system" 的 Msg 实例，自动处理默认值（created_at、finished_at、id）。
- **FR-020**: `Usage` MUST 包含 `input_tokens`（整数）和 `output_tokens`（整数）字段。

#### Event 模块

- **FR-021**: 系统 MUST 提供 `EventType` 字符串枚举，包含以下 27 种事件类型：`REPLY_START`、`REPLY_END`、`MODEL_CALL_START`、`MODEL_CALL_END`、`TEXT_BLOCK_START`、`TEXT_BLOCK_DELTA`、`TEXT_BLOCK_END`、`DATA_BLOCK_START`、`DATA_BLOCK_DELTA`、`DATA_BLOCK_END`、`THINKING_BLOCK_START`、`THINKING_BLOCK_DELTA`、`THINKING_BLOCK_END`、`HINT_BLOCK`、`TOOL_CALL_START`、`TOOL_CALL_DELTA`、`TOOL_CALL_END`、`TOOL_RESULT_START`、`TOOL_RESULT_TEXT_DELTA`、`TOOL_RESULT_DATA_DELTA`、`TOOL_RESULT_END`、`EXCEED_MAX_ITERS`、`REQUIRE_USER_CONFIRM`、`REQUIRE_EXTERNAL_EXECUTION`、`USER_CONFIRM_RESULT`、`USER_INTERRUPT`、`EXTERNAL_EXECUTION_RESULT`、`CUSTOM`。
- **FR-022**: 系统 MUST 提供 `EventBase` 基类，包含 `id`（自动生成）、`created_at`（ISO 8601 时间戳）、`metadata`（字典）字段，并设置 `use_enum_values=True` 以确保枚举值序列化为字符串。
- **FR-023**: 系统 MUST 为每种 EventType 提供对应的事件类，每种事件类继承 EventBase 并包含 `type` 判别字面量和特定字段。
- **FR-024**: `ReplyStartEvent` MUST 包含 `session_id`、`reply_id`、`name`、`role` 字段。
- **FR-025**: `ReplyEndEvent` MUST 包含 `session_id`、`reply_id`、`finished_reason`（ReplyFinishedReason）、`error`（ErrorInfo 或 None）字段。
- **FR-026**: `ModelCallStartEvent` MUST 包含 `reply_id`、`model_name` 字段。
- **FR-027**: `ModelCallEndEvent` MUST 包含 `reply_id`、`input_tokens`、`output_tokens`、`finished_reason`（FinishedReason）字段。
- **FR-028**: 文本/思考/数据块事件 MUST 遵循 Start-Delta-End 三阶段生命周期：Start 事件创建块并分配 block_id，Delta 事件追加增量内容，End 事件标记完成时间。
- **FR-029**: `HintBlockEvent` MUST 为一次性事件（非流式），包含 `reply_id`、`block_id`、`source`、`hint` 字段，完整内容一次性到达。
- **FR-030**: 工具调用/结果事件 MUST 遵循完整生命周期：`TOOL_CALL_START`→`TOOL_CALL_DELTA`（可多个）→`TOOL_CALL_END`→`TOOL_RESULT_START`→`TOOL_RESULT_TEXT_DELTA/DATA_DELTA`（可多个）→`TOOL_RESULT_END`。
- **FR-031**: `RequireUserConfirmEvent` MUST 包含 `reply_id`、`tool_calls`（ToolCallBlock 列表）字段。
- **FR-032**: `UserConfirmResultEvent` MUST 包含 `reply_id`、`confirm_results`（ConfirmResult 列表）字段。
- **FR-033**: `ConfirmResult` MUST 包含 `confirmed`（布尔值）、`tool_call`（ToolCallBlock）、`rules`（PermissionRule 列表或 None）字段。
- **FR-034**: `UserInterruptEvent` MUST 包含 `reply_id` 字段，仅对 parked（等待外部输入的）reply 有效。
- **FR-035**: `RequireExternalExecutionEvent` MUST 包含 `reply_id`、`tool_calls`（ToolCallBlock 列表）字段。
- **FR-036**: `ExternalExecutionResultEvent` MUST 包含 `reply_id`、`execution_results`（ToolResultBlock 列表）字段。
- **FR-037**: `CustomEvent` MUST 包含 `name`（字符串）、`value`（字典）字段，用于不特定于框架的服务层通知。
- **FR-038**: `ExceedMaxItersEvent` MUST 包含 `reply_id`、`name` 字段。
- **FR-039**: `AgentEvent` MUST 为所有事件类的联合类型别名，覆盖全部 27 种事件类。

#### State 模块

- **FR-040**: 系统 MUST 提供 `AgentState` 数据结构，包含：`session_id`、`summary`（字符串或 TextBlock/DataBlock 列表）、`context`（Msg 列表）、`max_context_messages`（可配置整数上限，默认无限制）、`reply_context`（ReplyContext）、`permission_context`、`tool_context`（ToolContext）、`tasks_context`（TaskContext）、`middle_context`（字典）字段。当 context 中的消息数量达到 `max_context_messages` 上限时，`append_context` MUST 返回错误而非静默丢弃。
- **FR-041**: `AgentState` MUST 提供 `append_context(name, blocks)` 方法，用于在 context 尾部追加内容块——若尾部 assistant 消息的 name 和 reply_id 匹配则追加到现有消息，否则创建新消息。
- **FR-042**: `AgentState` MUST 提供 `has_awaiting_tool_calls(name)` 和 `get_awaiting_tool_calls(name)` 方法，用于查询指定 agent 的尾部 assistant 消息中是否存在等待用户确认（ASKING）或外部执行（SUBMITTED 且无结果）的 ToolCall。
- **FR-043**: `AgentState` MUST 支持从旧存储格式（顶层 `reply_id`/`cur_iter` 字段）自动迁移到新的 `reply_context` 嵌套结构。
- **FR-044**: `ReplyContext` MUST 包含 `reply_id`、`cur_iter`（当前迭代次数）、`structured_schema`（Pydantic model 类或 JSON schema dict 或 None）、`structured_output`（字典或 None）字段。
- **FR-045**: `ToolContext` MUST 包含 `max_cache_files`（默认 100）、`max_cache_bytes`（默认 25000）、`read_file_cache`（ReadCacheEntry 列表）、`activated_groups`（字符串列表）字段。
- **FR-046**: `ToolContext` MUST 提供 LRU 缓存管理的方法：`get_cache(file_path)`（检查时效性并返回缓存）、`cache_file(file_path, lines)`（按 LRU 策略缓存并驱逐旧条目）、`clean_file_cache(reserved_file_paths)`（清理不在保留列表中的缓存）。
- **FR-047**: `TaskContext` MUST 包含 `tasks`（Task 列表）字段。
- **FR-048**: `Task` MUST 包含 `subject`、`description`、`metadata`（字典）、`created_at`、`state`（"pending"|"in_progress"|"completed"）、`id`、`owner`（字符串或 None）、`blocks`（被阻塞任务 ID 列表）、`blocked_by`（阻塞任务 ID 列表）字段。

#### Types 模块

- **FR-049**: 系统 MUST 提供 `ReplyFinishedReason` 字符串枚举，包含 `COMPLETED`、`INTERRUPTED`、`EXCEED_MAX_ITERS`、`ERROR` 四种值。
- **FR-050**: 系统 MUST 提供 `ErrorType` 字符串枚举，包含 `AUTHENTICATION`（401）、`PERMISSION`（403）、`RATE_LIMIT`（429）、`INVALID_REQUEST`（400/422）、`UPSTREAM`（5xx）、`CONNECTION`（网络错误）、`INTERNAL`（框架内部错误）、`UNKNOWN`（兜底）九种分类。
- **FR-051**: `ErrorInfo` MUST 包含 `type`（ErrorType，默认 UNKNOWN）和 `message`（字符串）字段。
- **FR-052**: 系统 MUST 提供 `JSONPrimitive` 类型别名（`str | int | float | bool | None`）和递归的 `JSONSerializableObject` 类型别名。
- **FR-053**: 系统 MUST 提供 `Embedding` 类型别名（`List[float]`）。
- **FR-054**: 系统 MUST 提供 `AgentHookTypes` 类型别名，包含 6 个 hook 点字面量。
- **FR-055**: 系统 MUST 提供 `ReActAgentHookTypes` 类型别名，在 AgentHookTypes 基础上增加 4 个 ReAct 专用 hook 点。

#### 跨模块约束

- **FR-056**: Foundation 层的模块间依赖关系 MUST 遵循：types（无 agentscope 内部依赖）→ message（依赖 types）→ event（依赖 message、types）→ state（依赖 message、types）。禁止 event 依赖 state，禁止 state 依赖 event。
- **FR-057**: 所有 Foundation 层数据结构的 JSON 序列化格式 MUST 与 AgentScope Python 参考实现（锁定版本）的输出一致。字段名称、枚举值的大小写形式、嵌套结构的层级必须完全匹配。
- **FR-058**: 所有公开数据结构的字段级行为（默认值、验证规则、序列化策略）MUST 与 Python 参考实现保持一致，除非宪法明确允许的偏差（如 Rust 语言强制的行为差异）。

### Key Entities

- **Msg（消息）**: AgentScope 中智能体间通信的基本单元。包含 name（发送者）、content（ContentBlock 列表）、role（user/assistant/system）、唯一 id、时间戳、token 用量、完成原因、错误信息和结构化输出。消息通过工厂函数创建，通过 `append_event` 方法由流式事件逐步构建。
- **ContentBlock（内容块）**: 消息内容的原子单元。六种类型：TextBlock（纯文本）、ThinkingBlock（模型推理过程）、HintBlock（提示/指令）、DataBlock（二进制数据，如 base64 或 URL 引用）、ToolCallBlock（工具调用请求及状态机）、ToolResultBlock（工具执行结果）。
- **EventType / AgentEvent（事件系统）**: 27 种事件类型的枚举及对应的事件数据类。覆盖回复生命周期、模型调用生命周期、六种内容块的流式构建、工具调用/结果的全流程、用户交互（确认/中断）和外部执行。事件通过 `append_event` 应用到 Msg 上。
- **AgentState（智能体状态）**: Agent 的完整运行时状态，包含会话上下文（对话历史）、回复状态、工具缓存、任务列表、权限上下文和中间件上下文。支持序列化持久化和跨 Session 恢复。
- **Task（任务）**: Agent 待执行任务的描述单元，包含主题、描述、状态流转（pending→in_progress→completed）、负责人、依赖关系（blocks/blocked_by）。
- **ReplyFinishedReason / ErrorType / ErrorInfo**: 回复终止原因和错误分类体系。ReplyFinishedReason 描述回复为何结束（正常完成/中断/超迭代/错误），ErrorType 对错误进行 HTTP 语义对齐的分类，ErrorInfo 将两者组合为结构化错误描述。
- **JSONPrimitive / JSONSerializableObject**: JSON 兼容类型的递归类型定义，用于约束框架中所有可序列化数据的类型边界。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 所有 20 个 Message 模块功能需求（FR-001～FR-020）可通过自动化测试验证，包括消息创建、内容块操作、事件应用和角色约束。
- **SC-002**: 所有 19 个 Event 模块功能需求（FR-021～FR-039）可通过自动化测试验证，覆盖全部 27 种事件类型的字段结构和序列化格式。
- **SC-003**: 所有 9 个 State 模块功能需求（FR-040～FR-048）可通过自动化测试验证，包括状态迁移、上下文追加和旧格式兼容。
- **SC-004**: 所有 7 个 Types 模块功能需求（FR-049～FR-055）可通过自动化测试验证。
- **SC-005**: Message 模块的 Msg 和所有 ContentBlock 子类型的 JSON 序列化输出与 Python 参考实现的输出在差分比较中完全一致（经归一化规则处理后）。
- **SC-006**: Event 模块的所有事件类 JSON 序列化输出与 Python 参考实现的输出在差分比较中一致。
- **SC-007**: Foundation 层所有数据结构在 Rust 实现中的字段名称（转为 snake_case 后的 JSON key）、枚举值字符串、默认值与 Python 参考实现匹配。
- **SC-008**: Foundation 层模块间不存在违反 FR-056 的依赖关系——即不形成循环依赖，不依赖 model/tool/agent 等上层模块。
- **SC-009**: Msg 的 `append_event` 方法正确响应全部 27 种事件类型，对于未知事件类型不 panic 且优雅降级。
- **SC-010**: ToolCallBlock 的状态机在接收到对应事件时正确执行状态转换，不产生非法状态或丢失转换。

## Assumptions

- AgentScope Python 参考实现的上游版本已在 Feature 001（compatibility-baseline）中锁定，本 Feature 以此锁定的版本为兼容目标。
- ContentBlock 的序列化依赖于类型的判别字段 `type`（字面值），Rust 实现可使用 serde 的 tag 机制或等效方式处理。
- `_utils._common` 中的 `_generate_id`（生成 UUID hex 字符串）和 `_generate_timestamp`（生成 ISO 8601 时间戳）作为内部工具函数，不属于 Foundation 层的公开 API，但被 Foundation 层的数据结构所使用。
- ToolCallBlock.input 字段存储原始 JSON 字符串（非结构化对象），不做运行时 JSON 解析或验证。
- DataBlock 在 Rust 实现中仍使用字符串存储 base64 数据，不做二进制优化——以保证 JSON 序列化兼容性。
- PermissionContext 和 PermissionRule 属于 permission 模块，虽然被 state 模块引用，但其具体实现在 permission 模块中定义。本 Feature 仅在 Foundation 层中定义最小化占位类型（`PermissionContext` 为 dict 别名或最小化 struct，`PermissionRule` 为最小化 struct，仅保留字段结构无权限检查逻辑），后续由 permission 模块的特性替换或扩展为完整实现。
- `ThinkingBlock` 的 `extra="allow"` 配置在 Rust 实现中需要等效机制来支持 provider 特定的透传字段。
- Foundation 层的 Rust 实现将使用 serde 进行 JSON 序列化/反序列化，并使用适当的数据建模库（如 pydantic 在 Python 中的角色）。
- 本 Feature 不涵盖 AgentState 的实际持久化机制（文件系统、数据库）——仅定义数据结构本身。持久化逻辑属于存储模块的范围。

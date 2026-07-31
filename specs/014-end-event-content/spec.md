# Feature Specification: End Event Content

**Feature Branch**: `014-end-event-content`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description: "扩展事件协议，让 EndEvent 携带完整内容，例如 TextBlockEndEvent { text: Option<String> }、ThinkingBlockEndEvent { thinking: Option<String> }、ToolCallEndEvent { input: Option<String> }、ToolResultEndEvent { output: Option<String> }，从 start 开始到 end 过程中流式收集的结果。同时要考虑非流式场景。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 消费者在 EndEvent 读取完整内容 (Priority: P1)

作为事件消费者，我需要在每个内容块结束事件中直接读取该块从开始到结束期间累积的完整内容，以便无需自行监听和拼接所有增量事件即可获得最终文本、思考内容、工具参数或工具结果。

**Why this priority**: 这是本功能的核心价值。当前 EndEvent 只表示生命周期结束，消费者必须自行维护累积状态，容易遗漏增量、处理顺序错误或在不同 block 类型上实现不一致。

**Independent Test**: 构造包含文本、思考、工具调用和工具结果的流式事件序列，验证每个 EndEvent 携带的完整内容与该 block 生命周期内所有增量拼接后的结果一致。

**Acceptance Scenarios**:

1. **Given** 一个文本块依次产生 Start、多个 TextDelta、End，**When** 消费者收到 TextBlockEndEvent，**Then** 事件的 `text` 字段包含从该文本块开始到结束期间累积的完整文本。
2. **Given** 一个 thinking 块依次产生 Start、多个 ThinkingDelta、End，**When** 消费者收到 ThinkingBlockEndEvent，**Then** 事件的 `thinking` 字段包含该 thinking 块的完整思考内容。
3. **Given** 一个工具调用块依次产生 Start、多个 ToolCallDelta、End，**When** 消费者收到 ToolCallEndEvent，**Then** 事件的 `input` 字段包含完整工具输入内容。
4. **Given** 一个工具结果块依次产生 Start、多个 ToolResultDelta、End，**When** 消费者收到 ToolResultEndEvent，**Then** 事件的 `output` 字段包含完整工具输出内容。

---

### User Story 2 - 非流式响应也发布完整 EndEvent 内容 (Priority: P1)

作为非流式调用的事件消费者，我需要在非流式模型响应、工具调用结果或一次性内容块完成时，也能从 EndEvent 中读取完整内容，以便流式和非流式场景拥有一致的事件消费体验。

**Why this priority**: 用户明确要求考虑非流式场景。如果非流式 EndEvent 不携带内容，消费者仍需为两种执行模式维护不同逻辑，功能价值不完整。

**Independent Test**: 使用非流式模型响应和一次性工具结果生成事件，验证对应 EndEvent 的可选内容字段填充为完整内容，而不是无意义的空值。

**Acceptance Scenarios**:

1. **Given** 非流式模型响应中包含完整文本内容，**When** 系统发布 TextBlockEndEvent，**Then** `text` 字段包含该完整文本内容。
2. **Given** 非流式模型响应中包含完整 thinking 内容，**When** 系统发布 ThinkingBlockEndEvent，**Then** `thinking` 字段包含该完整 thinking 内容。
3. **Given** 非流式流程产生工具调用输入或工具执行输出，**When** 系统发布对应 ToolCallEndEvent 或 ToolResultEndEvent，**Then** `input` 或 `output` 字段包含完整内容。

---

### User Story 3 - 旧消费者可继续只依赖 EndEvent 生命周期语义 (Priority: P2)

作为已有事件消费者，我需要在 EndEvent 增加内容字段后仍能把 EndEvent 当作“块已结束”的生命周期信号使用，以便现有依赖事件顺序、block 标识和完成时机的逻辑不被破坏。

**Why this priority**: 事件协议变更会影响所有订阅方。新增内容字段必须保持生命周期语义和事件顺序稳定，避免引入兼容性回归。

**Independent Test**: 对比变更前后相同输入的事件类型顺序、block 标识关联和结束事件数量，验证除新增可选内容字段外，生命周期语义保持不变。

**Acceptance Scenarios**:

1. **Given** 消费者只检查 EndEvent 的类型、顺序和 block 标识，**When** 系统升级为携带内容的 EndEvent，**Then** 消费者仍观察到相同的结束事件时机和数量。
2. **Given** 某个 block 没有产生任何内容增量，**When** 该 block 结束，**Then** EndEvent 仍被发布，内容字段为空值或空内容，并明确表示该字段未产生可累积内容。
3. **Given** 内容累积过程中出现错误或取消，**When** 系统发布最终事件，**Then** 已发布的 EndEvent 不得宣称包含未完成的完整内容；错误或取消语义必须保持可观察。

---

### User Story 4 - Trace 与调试工具可直接展示完整块内容 (Priority: P3)

作为调试和 trace 工具使用者，我希望事件 trace 中的 EndEvent 能展示每个块的最终内容摘要或完整内容，以便更容易定位流式拼接、工具调用参数和工具结果显示问题。

**Why this priority**: 这是对可观测性的增强。它不改变核心执行能力，但能显著降低排查事件协议和流式处理问题的成本。

**Independent Test**: 记录一次包含多种 block 类型的 trace，验证 trace 中每个 EndEvent 的内容字段与最终可见内容一致，并可用于重建用户看到的完整块输出。

**Acceptance Scenarios**:

1. **Given** 一次完整 agent 执行产生结构化 trace，**When** 查看 EndEvent 记录，**Then** trace 中能看到对应 block 的最终内容字段。
2. **Given** trace 需要重建完整输出，**When** 只读取 EndEvent 的内容字段，**Then** 可以得到与读取全部增量事件一致的块级最终内容。

---

### Edge Cases

- 当某个 block 在 Start 后没有产生任何增量内容即结束时，EndEvent 必须仍然发布，内容字段必须清楚区分“无内容”和“字段不可用”。
- 当增量内容为空字符串、空 JSON 片段或空工具输出时，EndEvent 必须保留该空内容语义，而不是误判为字段缺失。
- 当同一响应中交错出现多个 block 时，每个 EndEvent 只能携带对应 block 自身累积的内容，不能混入其他 block。
- 当工具调用输入由多个不完整片段组成时，ToolCallEndEvent 必须携带按原始事件顺序拼接后的完整输入。
- 当工具结果输出包含结构化文本、JSON 字符串或多段输出时，ToolResultEndEvent 必须携带消费者可观察到的完整输出表示。
- 当执行被取消或中途失败时，系统必须保持错误/取消事件语义，不得用带内容的 EndEvent 掩盖未完成状态。
- 当旧数据或外部 producer 生成不含新增字段的 EndEvent 时，消费者必须能够将其视为内容未知，而不是协议错误。

## Requirements *(mandatory)*

### Functional Requirements

#### EndEvent 内容字段

- **FR-001**: 系统 MUST 扩展文本块结束事件，使其可携带从对应文本块开始到结束期间累积的完整文本内容。
- **FR-002**: 系统 MUST 扩展 thinking 块结束事件，使其可携带从对应 thinking 块开始到结束期间累积的完整思考内容。
- **FR-003**: 系统 MUST 扩展工具调用结束事件，使其可携带从对应工具调用块开始到结束期间累积的完整工具输入内容。
- **FR-004**: 系统 MUST 扩展工具结果结束事件，使其可携带从对应工具结果块开始到结束期间累积的完整工具输出内容。
- **FR-005**: 新增内容字段 MUST 是可选字段，以支持旧事件、无内容块、取消/错误路径和 producer 无法提供完整内容的场景。
- **FR-006**: EndEvent 的原有生命周期含义 MUST 保持不变：事件仍表示对应 block 生命周期结束，不得变更结束事件类型、数量或相对顺序。

#### 内容累积语义

- **FR-007**: 系统 MUST 从 block Start 到对应 End 的生命周期范围内累积内容，只将属于该 block 的增量纳入最终内容字段。
- **FR-008**: 系统 MUST 按增量事件的可观察发布顺序累积内容，最终 EndEvent 内容必须与消费者自行按顺序拼接增量得到的结果一致。
- **FR-009**: 系统 MUST 在同一响应存在多个 block 或 block 交错时保持独立累积状态，防止内容跨 block 串扰。
- **FR-010**: 系统 MUST 对空内容进行语义保留：确实产生的空字符串或空输出应可与“内容未知/未提供”区分。
- **FR-011**: 系统 MUST 在错误或取消导致 block 未完整结束时保持现有错误/取消语义；若发布 EndEvent，其内容字段只能表示已确认完整的内容或显式未知，不得误导消费者。

#### 流式与非流式一致性

- **FR-012**: 在流式场景中，系统 MUST 根据从 Start 到 End 之间发布的增量事件填充对应 EndEvent 内容字段。
- **FR-013**: 在非流式场景中，系统 MUST 根据一次性响应或一次性工具结果中的完整内容填充对应 EndEvent 内容字段，使消费者无需区分流式与非流式模式。
- **FR-014**: 对于同一业务输出，流式和非流式场景的 EndEvent 最终内容字段 MUST 在语义上等价。
- **FR-015**: 系统 MUST 保留增量事件本身；新增 EndEvent 内容不得替代、删除或改变现有 DeltaEvent 的发布。

#### 协议兼容与可观测性

- **FR-016**: 事件序列化协议 MUST 允许缺失新增内容字段，并将其解释为内容未知或未提供。
- **FR-017**: 事件序列化协议 MUST 在内容字段存在时保留原始可观察内容，不得进行会改变语义的裁剪、重排或格式转换。
- **FR-018**: Trace 记录 MUST 能捕获新增 EndEvent 内容字段，使调试工具可以直接展示 block 的最终内容。
- **FR-019**: 文档和兼容性说明 MUST 明确 EndEvent 内容字段是完整块内容的便利快照，而不是替代 DeltaEvent 的唯一来源。
- **FR-020**: 系统 MUST 提供覆盖文本、thinking、工具调用输入和工具结果输出的验收测试，包含流式和非流式两类场景。

### Key Entities *(include if feature involves data)*

- **TextBlockEndEvent**: 文本块生命周期结束事件。新增可选完整文本内容，用于表示该文本块最终可观察文本。
- **ThinkingBlockEndEvent**: 思考块生命周期结束事件。新增可选完整 thinking 内容，用于表示该思考块最终可观察思考文本。
- **ToolCallEndEvent**: 工具调用块生命周期结束事件。新增可选完整输入内容，用于表示该工具调用最终参数或输入文本。
- **ToolResultEndEvent**: 工具结果块生命周期结束事件。新增可选完整输出内容，用于表示该工具执行结果最终输出。
- **Block Content Accumulator**: 每个 block 生命周期内的内容累积状态。负责将 Start 与 End 之间的增量内容合成为 EndEvent 的完整内容字段。
- **Event Consumer**: 订阅 agent/model/tool 事件的调用方、trace 记录器或 UI。可选择读取 EndEvent 内容字段，也可继续读取增量事件自行拼接。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 对文本、thinking、工具调用输入、工具结果输出四类 block，100% 的正常完成 EndEvent 在可获得内容时携带与增量拼接结果一致的完整内容。
- **SC-002**: 覆盖流式与非流式场景的测试均通过，且至少包含每类 EndEvent 的正常内容、空内容和缺失内容三类用例。
- **SC-003**: 相同输入下，新增字段前后事件类型顺序、结束事件数量和 block 关联标识保持 100% 一致。
- **SC-004**: 旧序列化数据中缺失新增内容字段时，事件消费者能够成功读取并将内容解释为未知，兼容性测试通过率达到 100%。
- **SC-005**: Trace 工具可通过 EndEvent 内容字段重建块级最终输出，重建结果与读取全部增量事件的结果一致率达到 100%。
- **SC-006**: 新增内容累积逻辑在包含至少 10 个交错 block 的测试中不发生跨 block 内容串扰、顺序错乱或内容丢失。

## Assumptions

- 本 feature 只扩展已有 EndEvent 的数据协议和内容填充语义，不引入新的 block 生命周期事件类型。
- 新增内容字段采用可选语义，以兼容旧事件、无内容事件和外部 producer 无法提供完整内容的情况。
- “完整内容”指对应 block 从 Start 到 End 的可观察内容累积结果；不包含其他 block 的内容，也不包含未发布给消费者的内部状态。
- 流式场景中的完整内容以已发布增量事件的顺序为准；非流式场景中的完整内容以一次性响应或工具结果中的可观察内容为准。
- 错误、取消和 timeout 的既有事件语义优先于便利内容字段；不得为了填充 EndEvent 内容而改变错误传播或取消行为。
- 该变更目标兼容等级为 L2（核心行为兼容）和 L1（协议兼容），并需要更新事件协议兼容性说明。

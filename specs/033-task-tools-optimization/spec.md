# Feature Specification: 任务工具输出质量优化（Task Tools Output Optimization）

**Feature Branch**: `033-task-tools-optimization`

**Created**: 2026-08-17

**Status**: Draft

**Input**: User description: "优化下 agent_scope_agent 中的 Task 功能。现在 examples/plan-react-agent 示例输出：多个 TaskCreate 结果拼接成一串、TaskGet 描述后直接粘连模型推理、TaskUpdate 只报 'Update task (id=1) status.' 不报告实际变更值……功能很差。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 多工具调用的结果清晰可读 (Priority: P1)

开发者运行 `plan-react-agent` 或其他流式示例，Agent 在同一轮连续调用多个任务工具时，每个工具的结果在终端单独成行显示，不再拼接成一团无法辨认的长串。工具结果与随后的模型推理文本之间以换行分隔，边界清晰。

**Why this priority**: 用户反馈"很差"最直接的体现就是输出拼接——同一轮 3 次 TaskCreate 的输出 `Task (id=1) created successfully: ...Task (id=2) created successfully: ...Task (id=3) ...` 粘连成一串；TaskGet 的 `Description: ...` 后直接接上模型推理。修复它立即可见，是本次优化的核心价值。

**Independent Test**: 运行 `plan-react-agent`（或等效的流式集成测试），构造一轮包含多个任务工具调用的回复，检查每个工具结果在展示层独立成行、无拼接；工具结果与后续模型文本之间有换行分隔。

**Acceptance Scenarios**:

1. **Given** 同一轮回复中 Agent 连续调用多次 TaskCreate，**When** 工具结果以流式事件输出，**Then** 每个"创建成功"结果独立成行显示，互不粘连
2. **Given** 工具结果文本之后紧跟着模型的下一条推理文本，**When** 流式输出，**Then** 两段文本以换行分隔，不粘连在同一行
3. **Given** 任意非任务工具（如 Bash、Grep 等）的结果文本未以换行结尾，**When** 流式输出，**Then** 展示层同样自动换行终止，所有工具输出一致可读

---

### User Story 2 - 更新结果报告实际变更 (Priority: P1)

Agent 调用 TaskUpdate 同时更新任务状态与依赖关系时，工具结果明确报告"哪个字段被改成了什么值"。Agent 无需猜测即可核实状态流转与依赖建立是否生效，避免后续规划混乱。

**Why this priority**: 当前 `Update task (id=1) status.` 只列字段名不报值，且掩盖了同一次调用中同时应用的 `add_blocked_by: ['4']` 依赖变更。模型无法确认依赖是否建立，是示例后续规划陷入混乱（创建无谓的"向用户索取 README 内容"任务）的根源之一。

**Independent Test**: 使用任务工具直接调用 TaskUpdate（单元/集成测试），分别验证：仅更新状态、同时更新状态与依赖、无任何实际变更、删除任务四种场景的输出均准确反映实际发生的变化。

**Acceptance Scenarios**:

1. **Given** Agent 调用 TaskUpdate 将任务 3 状态改为 in_progress，**When** 结果返回，**Then** 输出明确说明 status 变为 `in_progress`
2. **Given** Agent 在一次调用中同时更新状态并追加 `add_blocked_by` 依赖，**When** 结果返回，**Then** 输出同时包含两项变更及其具体值（新状态、新增的依赖 id 列表）
3. **Given** 调用未产生任何实际变更（如引用不存在的依赖 id、空更新），**When** 结果返回，**Then** 输出明确的"无变更"提示，与现有行为一致
4. **Given** Agent 将任务状态更新为 deleted，**When** 结果返回，**Then** 保持明确的删除确认输出
5. **Given** 更新后任务状态为 completed，**When** 结果返回，**Then** 保留现有的"任务完成，请 TaskList 查看下一步"引导提示

---

### User Story 3 - 任务详情保持紧凑 (Priority: P2)

Agent 用 TaskGet 查询包含超长描述的任务时，结果紧凑可读，不把完整长描述原样倾入工具结果与后续上下文。描述超过阈值时输出前缀摘要与省略提示，并告知完整长度。

**Why this priority**: 任务描述可能是模型自写的大段规划文本。原样倾倒导致工具结果体积大、展示上与后续文本粘连、上下文 token 成本高。截断后模型仍能拿到关键信息与完整长度提示，可据此决定是否继续。

**Independent Test**: 直接调用 TaskGet，分别以超长描述、正常长度描述、空描述的任务验证输出行为符合截断/完整/边界规则。

**Acceptance Scenarios**:

1. **Given** 任务描述超过阈值（默认 200 字符），**When** TaskGet 返回，**Then** 输出描述前缀 + 省略提示（含完整长度），而非完整文本
2. **Given** 任务描述未超过阈值，**When** TaskGet 返回，**Then** 输出完整描述
3. **Given** 任务描述为空，**When** TaskGet 返回，**Then** 描述行正常显示为空，不报错
4. **Given** 任务不存在，**When** TaskGet 返回，**Then** 保持现有"Task not found"错误结果

---

### User Story 4 - 示例渲染同步改进 (Priority: P2)

`plan-react-agent` 示例的流式渲染自身也改进：事件之间有清晰换行分隔，工具的输入与结果输出可分辨，展示层不再放大拼接问题。

**Why this priority**: 示例是库的演示窗口，当前渲染用无换行的 `print!` 直接放大拼接问题。改进渲染后示例输出本身即成为"可读"的示范。

**Independent Test**: 运行 `plan-react-agent` 示例，目视检查一轮多工具调用与最终答复的终端输出分段清晰。

**Acceptance Scenarios**:

1. **Given** 示例流式输出工具结果与文本增量事件，**When** 事件到达渲染层，**Then** 每个工具结果与文本段以换行正确分隔
2. **Given** 一轮包含多个工具调用，**When** 展示工具调用，**Then** 每个调用的输入与其结果在视觉上可对应，不互相混淆

---

### Edge Cases

- 单轮仅 1 次工具调用：结果尾随换行不影响阅读，也不产生多余空行
- 任务描述长度恰好等于截断阈值边界：定义明确的包含/排除规则（达到阈值即截断，或未超过才完整）
- 描述截断位置落在换行符上：不产生奇怪的半行截断
- 空描述任务：TaskGet 正常显示空描述行
- 旧 Python 逐字对齐测试（`task_tools_tests.rs` 等断言精确文本）全部迁移到新输出格式，无遗漏
- 通用换行终止引入后，其它工具的事件流断言若受影响需一并排查迁移
- 流式工具输出中途被取消/中断：不产生换行终止后的残留状态

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: 任务工具（TaskCreate / TaskList / TaskGet / TaskUpdate）的成功文本输出必须以换行符终止，确保同一轮多个工具结果在流式展示层独立成行、互不粘连
- **FR-002**: 流式事件层对任意"完整结束"的工具结果文本统一确保以换行终止（若文本本身未以换行结尾则补齐），使所有工具（含非任务工具）的连续结果在展示层一致可读；该补齐只作用于展示/事件流，不得改变工具状态语义
- **FR-003**: TaskUpdate 的输出必须报告实际应用的字段值——状态类字段报告新值（如 `status → in_progress`），依赖类字段报告新增的 id 列表（如 `add_blocked_by → ['4']`），而非仅字段名；一次调用多项变更时全部列出
- **FR-004**: TaskGet 的输出必须对超过阈值（默认 200 字符）的 description 进行截断，输出前缀、省略提示与完整长度；未超过阈值或描述为空时输出完整/空描述行
- **FR-005**: 任务工具的对外 API 表面不变——工具名称（TaskCreate/TaskList/TaskGet/TaskUpdate）、输入 JSON Schema、状态流转语义、依赖语义、错误语义与 `AgentState.tasks_context` 数据模型保持 Feature 024 的现状，仅输出文本与展示层变化
- **FR-006**: 任务工具的文本对齐契约（`contracts/task-tools.md`）必须更新为"任务工具输出为 Rust 优化版、不再逐字对齐 Python 参考"，并记录每个工具的文本差异摘要
- **FR-007**: 兼容性文档必须新增一条任务工具输出文本相对 Python 参考的偏差记录，说明偏差原因（输出可读性与模型可用性优化）与影响范围，符合工程宪法第一条的偏差记录要求
- **FR-008**: 依赖旧精确文本的测试（`task_tools_tests.rs` 等）必须迁移到新输出格式，并新增针对尾随换行、变更值报告、描述截断提示的断言
- **FR-009**: `plan-react-agent` 示例的流式渲染必须改进，使工具结果与文本增量事件以换行正确分隔、工具输入与结果可对应

### Key Entities *(include if feature involves data)*

- **Task（任务）**: 数据模型不变——顺序标识、标题、描述、状态、负责人、双向阻塞关系、元数据与创建时间，生命周期语义保持 Feature 024 现状
- **TaskContext（任务上下文）**: 数据容器不变，随 Agent 状态持久化
- **Task Tools（任务工具）**: 对外表面不变（名称/Schema/语义），变化集中在成功输出文本的内容与格式
- **流式展示层**: 承载工具结果事件（文本增量）的展示路径，本特性为其引入统一的换行终止规则

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 运行 `plan-react-agent`（或等效流式测试），构造一轮含多个任务工具调用的回复，终端/事件流中连续工具结果 100% 独立成行、零拼接
- **SC-002**: TaskUpdate 在更新状态或依赖关系时，输出 100% 包含实际变更值；无实际变更与删除场景保持明确提示
- **SC-003**: TaskGet 对超过阈值的描述 100% 截断并附完整长度提示，未超过阈值时 100% 输出完整描述
- **SC-004**: 全部既有测试迁移到新输出格式后通过，且新增测试覆盖尾随换行、变更值报告、描述截断三类场景
- **SC-005**: 兼容性文档新增任务工具输出文本偏差记录 1 条，契约文档同步更新，无遗漏
- **SC-006**: 任务工具名称、Schema、状态/依赖/错误语义与数据模型零变更（对外 API 向后兼容 Feature 024）

## Assumptions

- 描述截断阈值取 200 字符，以常量形式实现并文档化；达到或超过阈值即截断，未超过输出完整
- 换行终止的通用修复对所有工具统一生效（不只任务工具），属于展示层改进，不改工具状态语义
- TaskCreate / TaskList 的文本内容本身可读性可接受，本次优化重点是换行终止与格式修正，不重写其文案
- 任务提醒（task_reminder）注入内容不在本特性优化范围内，保持现状
- 输出文本相对 Python 参考的偏差为"已批准的有意偏差"，在兼容矩阵记录后不要求恢复逐字对齐
- 本特性为纯输出质量优化，不改变任务数据模型与 Agent 行为语义，Feature 024 的验收场景保持满足

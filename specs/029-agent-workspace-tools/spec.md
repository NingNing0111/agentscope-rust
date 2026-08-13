# Feature Specification: Agent Workspace Built-in Tools

**Feature Branch**: `[029-agent-workspace-tools]`

**Created**: 2026-08-12

**Status**: Draft

**Input**: User description: "agent_scope_agent启用workspace后，要内置一些工具。包括 Bash、Edit、Write、Grep、Glob、PowerShell、ResetTools、SkillViewer/Skill 等工具，并定义名称、描述要点和主要参数。"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 使用 workspace 后自动获得基础文件与命令工具 (Priority: P1)

作为使用 agent workspace 的开发者，我希望 agent 在绑定 workspace 后自动拥有一组基础工具，以便在同一个受控工作区内执行命令、搜索文件、查看技能并修改文件，而不需要调用方为每个常见能力手动注册工具。

**Why this priority**: 这是 feature 的核心价值；没有默认工具集，workspace 对 agent 的实际执行能力有限，调用方需要重复配置常见工具。

**Independent Test**: 可以通过创建一个启用 workspace 的 agent，并检查其可用工具列表是否包含所有必需内置工具且名称、描述和参数契约完整来独立验证。

**Acceptance Scenarios**:

1. **Given** agent 已启用 workspace，**When** 调用方读取可用工具列表，**Then** 工具列表包含 Bash、Edit、Write、Grep、Glob、ResetTools、Skill，其中支持 Windows shell 的环境还包含 PowerShell。
2. **Given** agent 未启用 workspace，**When** 调用方读取可用工具列表，**Then** workspace 相关内置工具不会被默认注入，避免未授权的文件或命令能力暴露。
3. **Given** agent 已启用 workspace，**When** 工具被展示给模型或调用方，**Then** 每个工具都提供清晰描述，说明用途、优先使用限制和关键安全约束。

---

### User Story 2 - 安全、可审计地修改 workspace 文件 (Priority: P2)

作为开发者，我希望 agent 通过 Read 后再 Edit/Write 的约束修改文件，以便减少误改、覆盖未知文件内容和不可追踪变更的风险。

**Why this priority**: 文件写入能力是高影响操作，必须具备明确的安全边界和可验证行为，否则会破坏用户 workspace 的可信度。

**Independent Test**: 可以通过让 agent 在读取和未读取文件两种状态下尝试 Edit/Write，并验证只有满足前置条件的修改被接受来独立验证。

**Acceptance Scenarios**:

1. **Given** 某现有文件尚未被当前工具会话读取，**When** agent 请求使用 Edit 修改该文件，**Then** 请求被拒绝并给出需要先读取文件的可操作提示。
2. **Given** 某现有文件已被当前工具会话读取，且 Edit 的 old_string 在文件中唯一出现，**When** agent 请求替换该字符串，**Then** 文件内容被精确替换且结果可追踪。
3. **Given** 某现有文件中 old_string 出现多次且 replace_all 未启用，**When** agent 请求 Edit，**Then** 请求被拒绝并说明匹配不唯一。
4. **Given** 某新文件路径位于 workspace 允许范围内，**When** agent 请求 Write，**Then** 文件被创建并记录为一次 workspace 写入操作。

---

### User Story 3 - 高效搜索与技能查看 (Priority: P3)

作为开发者，我希望 agent 优先使用专用搜索和技能查看工具，而不是把所有操作都塞进 shell 命令，以便获得更稳定、更结构化、更少噪声的结果。

**Why this priority**: 专用工具能够降低模型误用 shell 的概率，并提升搜索、文件发现和技能匹配的可解释性。

**Independent Test**: 可以通过向 agent 提供文件搜索、内容搜索和技能查看任务，并验证其可使用对应工具完成任务来独立验证。

**Acceptance Scenarios**:

1. **Given** workspace 中存在匹配模式的文件，**When** agent 使用 Glob 查找文件，**Then** 返回限定在 workspace 范围内的匹配文件列表。
2. **Given** workspace 中存在匹配内容，**When** agent 使用 Grep 搜索内容，**Then** 返回符合输出模式、上下文行数和数量限制的搜索结果。
3. **Given** workspace 中存在可用技能，**When** agent 使用 Skill 查看精确技能名称，**Then** 返回该技能内容或明确的未找到错误。
4. **Given** agent 需要恢复默认工具可见性或切换工具组，**When** agent 使用 ResetTools，**Then** 工具组状态按用户授权的规则恢复或更新。

### Edge Cases

- 启用 workspace 但 workspace 路径无效、不可访问或超出允许边界时，内置工具不得执行文件或命令操作，并应返回明确错误。
- Bash 或 PowerShell 命令超过允许执行时长时，操作应超时终止并返回可诊断结果。
- 搜索结果过多时，Grep 和 Glob 应提供可控的结果数量限制，避免输出淹没主要上下文。
- Edit 的匹配字符串为空、缺失、过大或不唯一时，应拒绝执行并说明原因。
- Write 写入已存在文件但该文件尚未被读取时，应拒绝覆盖并提示先读取。
- Skill 查询名称不存在、大小写不匹配或存在多个近似名称时，应返回明确反馈，避免误加载错误技能。
- 在非 Windows 环境中请求 PowerShell 时，应清晰说明该工具不可用或未启用。
- ResetTools 不得授予超出当前 workspace 权限边界的工具能力。

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST inject a default built-in tool set when an agent is explicitly configured with workspace access.
- **FR-002**: System MUST NOT inject workspace file or command tools into agents that do not have workspace access.
- **FR-003**: System MUST expose a `Bash` tool for command execution with a persistent working directory scoped to the active workspace.
- **FR-004**: `Bash` MUST require `command` and support optional `description` and `timeout`, with a default timeout of 120,000 ms and a maximum timeout of 600,000 ms.
- **FR-005**: `Bash` descriptions MUST encourage use of dedicated tools for file reading, searching, and structured edits instead of generic shell commands when a dedicated tool exists.
- **FR-006**: System MUST expose an `Edit` tool for exact string replacement in files inside the workspace.
- **FR-007**: `Edit` MUST require `file_path`, `old_string`, and `new_string`, and support `replace_all` defaulting to false.
- **FR-008**: `Edit` MUST require the target file to have been read in the current tool session before modification.
- **FR-009**: `Edit` MUST reject non-unique `old_string` matches unless `replace_all` is explicitly true.
- **FR-010**: System MUST expose a `Write` tool for creating new files or overwriting files inside the workspace.
- **FR-011**: `Write` MUST require `file_path` and `content`.
- **FR-012**: `Write` MUST require an existing target file to have been read in the current tool session before overwrite.
- **FR-013**: System MUST expose a `Grep` tool for content search with pattern matching, file filtering, output mode selection, context line controls, case-insensitive matching, multiline matching, and result limits.
- **FR-014**: `Grep` MUST support at least `pattern`, optional file selection controls, output modes for matching content, matching files, and counts, context controls, case sensitivity control, multiline control, and maximum result limits.
- **FR-015**: System MUST expose a `Glob` tool for file discovery using glob-style patterns within the workspace.
- **FR-016**: `Glob` MUST support a required file pattern and an optional search root scoped to the active workspace.
- **FR-017**: System SHOULD expose a `PowerShell` command tool when the active environment supports Windows shell execution.
- **FR-018**: `PowerShell` MUST follow the same workspace scoping, timeout, description, and auditability expectations as `Bash`.
- **FR-019**: System MUST expose a `ResetTools` meta-tool that restores or activates tool group state without expanding permissions beyond the agent's current authorization.
- **FR-020**: System MUST expose a `Skill` tool for viewing skill content by exact skill name.
- **FR-021**: `Skill` MUST require a `skill` parameter containing the exact skill name.
- **FR-022**: All built-in tools MUST provide stable names, human-readable descriptions, and machine-readable input contracts.
- **FR-023**: All built-in tools MUST enforce workspace boundaries for file paths, search roots, and command working directories.
- **FR-024**: All rejected tool invocations MUST return actionable errors that distinguish validation failure, permission denial, unsupported capability, timeout, and execution failure.
- **FR-025**: Tool availability and tool invocation outcomes MUST be observable in agent traces, including tool name, parameter summary, success or error category, and ordering relative to agent events.
- **FR-026**: Built-in tools MUST preserve compatibility expectations with existing workspace, tool, event, and trace behavior.

### Key Entities *(include if feature involves data)*

- **Workspace-enabled Agent**: An agent configured with explicit access to a bounded workspace and eligible for default built-in tools.
- **Built-in Tool Definition**: A stable tool contract containing name, description, availability rules, and input parameters.
- **Tool Invocation**: A single request to run a built-in tool, including arguments, workspace context, result, error category, and trace metadata.
- **Read State**: Per-session record of which files have been read and are therefore eligible for guarded Edit or overwrite Write operations.
- **Tool Group State**: The current activation state of tool groups that may be reset or changed by ResetTools within authorization boundaries.
- **Skill Catalog Entry**: A named skill that can be viewed through the Skill tool for agent capability matching.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of workspace-enabled agents expose the required built-in tool definitions with documented names, descriptions, and input contracts.
- **SC-002**: 0% of agents without workspace access expose default file mutation or command execution tools.
- **SC-003**: 100% of guarded file mutation attempts enforce the read-before-modify rule for existing files.
- **SC-004**: 100% of rejected tool invocations return a typed, actionable error category without silently succeeding.
- **SC-005**: A developer can complete a representative workflow—search files, inspect matches, edit a file, create a file, and view a skill—using only the built-in tools in under 5 minutes.
- **SC-006**: All tool invocations in a representative agent run appear in trace output in the same order as they occurred.
- **SC-007**: Search tools can limit large result sets so a query with more than 1,000 possible matches returns a bounded, comprehensible response.
- **SC-008**: Command tools terminate timed-out commands within the configured maximum timeout window and report timeout status reliably.

## Assumptions

- The feature applies only after an agent is explicitly configured with workspace access.
- Workspace path validation, boundary enforcement, and existing Read capability are already available or will be treated as dependencies for this feature.
- `PowerShell` is environment-dependent and may be unavailable on non-Windows environments without making the rest of the feature incomplete.
- `Grep` and `Glob` are intended as dedicated search/discovery tools so agents can avoid using shell commands for routine search tasks.
- The names `Bash`, `Edit`, `Write`, `Grep`, `Glob`, and `Skill` are stable public-facing tool names for this feature.
- `ResetTools` is a meta-tool for tool group state only; it does not create new permissions or bypass workspace authorization.

# Feature Specification: Workspace Management（工作空间管理）

**Feature Branch**: `012-workspace-management`

**Created**: 2026-07-31

**Status**: Draft

**Input**: User description: "实现Workspace模块，Agent工作区间管理，参考python版本的agentscope"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 创建和管理本地工作空间 (Priority: P1) 🎯 MVP

作为开发者，我希望为每个 Agent 分配一个隔离的本地工作目录，Agent 在该目录中执行文件操作（读写、搜索、执行命令），而不会影响到其他 Agent 或宿主机文件系统。

**Why this priority**: 本地工作空间是 Agent 执行任务的基础设施——没有工作区间隔离，Agent 无法安全地创建文件、运行命令或管理项目。这是整个 Workspace 模块的核心价值。

**Independent Test**: 创建一个 `LocalWorkspace` 实例并指定 `workdir` 路径，验证目录结构自动创建（`data/`、`skills/`、`sessions/`），可通过 `list_tools()` 获取绑定到该工作空间的工具列表，并在工作空间内执行文件读写操作。

**Acceptance Scenarios**:

1. **Given** 一个不存在的工作目录路径, **When** 初始化 `LocalWorkspace` 并调用 `initialize()`, **Then** 自动创建 `{workdir}/data/`、`{workdir}/skills/`、`{workdir}/sessions/` 以及 `{workdir}/.mcp` 文件
2. **Given** 一个已初始化的 `LocalWorkspace`, **When** 调用 `list_tools()`, **Then** 返回一组内置工具（Bash/Edit/Glob/Grep/Read/Write），且这些工具的工作根目录绑定到该 workspace 的 `workdir`
3. **Given** 一个已初始化的 `LocalWorkspace`, **When** 在工作空间内写入一个文件然后读取, **Then** 文件实际存储在 `workdir` 下，且读写操作限定在该目录范围内
4. **Given** 一个已初始化的 `LocalWorkspace`, **When** 使用 `async with` 语法（或 Rust 等价模式）管理生命周期, **Then** 进入时自动调用 `initialize()`，退出时自动调用 `close()`

---

### User Story 2 - 工作空间内的资源管理 (Priority: P2)

作为开发者，我希望在工作空间内动态管理 MCP 服务器和 Skill（可复用技能模块），包括注册、检索和移除操作，使 Agent 可以在运行时获取所需的外部工具和领域知识。

**Why this priority**: 工作空间不仅仅是文件目录，它还是 Agent 能力的注册中心。MCP 连接扩展了 Agent 的能力边界，Skill 管理支持跨任务的代码和知识复用。

**Independent Test**: 在已初始化的 `LocalWorkspace` 中执行 `add_mcp()` 添加一个 MCP 客户端配置，通过 `list_mcps()` 验证已注册，再通过 `remove_mcp()` 移除并验证列表为空。对 Skill 执行相同的增删查操作。

**Acceptance Scenarios**:

1. **Given** 一个已初始化的 `LocalWorkspace`, **When** 调用 `add_mcp(mcp_client)` 注册一个 MCP 客户端, **Then** MCP 客户端信息被持久化到 `{workdir}/.mcp`，且 `list_mcps()` 可返回该客户端
2. **Given** 工作空间中已有 MCP 客户端, **When** 调用 `remove_mcp(name)` 按名称移除, **Then** 该客户端从内存和 `.mcp` 文件中同时删除
3. **Given** 本地存在一个包含 `SKILL.md` 的 skill 目录, **When** 调用 `add_skill(skill_path)` 将其添加到工作空间, **Then** skill 被复制到 `{workdir}/skills/` 下，且 `list_skills()` 返回该 skill
4. **Given** 工作空间中已有 skill, **When** 调用 `remove_skill(name)` 按名称移除, **Then** skill 目录被删除且不再出现在 `list_skills()` 中

---

### User Story 3 - 大内容上下文卸载 (Priority: P3)

作为开发者，当 Agent 的对话上下文或工具执行结果包含大量数据（超出 LLM 上下文窗口限制），我希望将这些内容卸载（offload）到工作空间的持久化存储中，用文件引用替代原始数据，使 Agent 能够"记住"更长的历史而不超出 token 限制。

**Why this priority**: 上下文卸载是解决 LLM 上下文窗口有限问题的关键基础设施。虽然重要，但相比工作空间基本文件操作和资源管理，它属于进阶功能。

**Independent Test**: 在已初始化的 `LocalWorkspace` 中创建一组包含 `Base64Source` 大数据的 `Msg` 对象，调用 `offload_context(session_id, msgs)`，验证 base64 数据被提取到 `data/` 目录并改写为 `file://` URL 引用，且返回了正确的 offload 文件路径。

**Acceptance Scenarios**:

1. **Given** 一组包含 base64 编码图片的 `Msg` 消息, **When** 调用 `offload_context(session_id, msgs)`, **Then** base64 数据被解码存储到 `{workdir}/data/{hash}.{ext}`，消息中的 `DataBlock` source 被替换为 `file://` URL
2. **Given** 一个 `ToolResultBlock` 包含文本和数据块, **When** 调用 `offload_tool_result(session_id, tool_result)`, **Then** 结果被持久化到 `{workdir}/sessions/{session_id}/tool_result-{id}.txt`
3. **Given** 同一 session 多次 `offload_context`, **When** 写入多次, **Then** 每次都追加到 `{workdir}/sessions/{session_id}/context.jsonl`，不覆盖已有内容
4. **Given** 同一 base64 数据块被 offload 两次, **When** 第二次 offload, **Then** 基于 SHA-256 哈希检测到文件已存在，跳过写入直接复用已有文件

---

### User Story 4 - 工作空间生命周期与重置 (Priority: P4)

作为开发者，我希望完整管理工作空间的生命周期（初始化、关闭、重置），确保在 Agent 任务完成后资源被正确释放，且在需要时可以清空工作空间回到干净状态。

**Why this priority**: 生命周期管理保证资源不泄漏，重置功能对于测试和重复使用的场景非常关键。这是一个保障性功能，优先级排在核心功能之后。

**Independent Test**: 在同一 `LocalWorkspace` 中依次调用 `initialize()` → 执行一些操作（创建文件、添加 MCP）→ `reset()` → 验证 `data/`、`skills/`、`sessions/` 目录被清空，`.mcp` 被清空。

**Acceptance Scenarios**:

1. **Given** 一个已初始化并添加了文件/MCP/skills 的工作空间, **When** 调用 `reset()`, **Then** `data/`、`skills/`、`sessions/` 目录内容被清空，`.mcp` 被清空（但不删除目录结构），`default_mcps` 和 `skill_paths` 不被重新播种
2. **Given** 一个已连接有状态 MCP 的工作空间, **When** 调用 `close()`, **Then** 所有有状态 MCP 连接被断开，`is_alive` 变为 `false`
3. **Given** 一个已关闭的工作空间, **When** 再次调用 `initialize()`, **Then** 从 `.mcp` 文件恢复之前持久化的 MCP 配置（幂等重启）

---

### User Story 5 - 工作空间管理器（多租户） (Priority: P5)

作为平台开发者，当系统中存在多个 Agent 或用户时，我需要一个 `WorkspaceManager` 来统一管理多个工作空间实例的生命周期，支持按用户/会话隔离，以及基于 TTL 的惰性淘汰。

**Why this priority**: 多租户管理是生产化部署的需求，在单 Agent 开发场景中不必须。但它是 AgentScope 面向多用户服务的关键组件。

**Independent Test**: 创建 `WorkspaceManager` 实例，指定 workspace 构造函数和 TTL，通过 `get()` 获取/创建工作空间，然后验证同一 key 返回同一实例，超时后自动清理。

**Acceptance Scenarios**:

1. **Given** 一个 `WorkspaceManager`, **When** 调用 `get(key)` 获取不存在的 key, **Then** 创建新 workspace 并返回
2. **Given** 已通过 `get(key)` 创建 workspace, **When** 再次调用 `get(key)`, **Then** 返回同一个已存在的 workspace 实例
3. **Given** workspace 超过 TTL 未被访问, **When** 下一次清理周期触发, **Then** 该 workspace 被自动关闭并回收

---

### Edge Cases

- 工作空间 `workdir` 指定的路径不存在时，`initialize()` 应自动创建整个目录树
- 当 `workdir` 指向已存在的非空目录时，应保留已有内容（不覆盖用户数据），仅确保 `data/`、`skills/`、`sessions/` 子目录存在
- `.mcp` 文件损坏（非有效 JSON）时，应使用 `default_mcps` 重新播种，不崩溃
- `remove_mcp(name)` 传入不存在的名称时，应记录警告日志并静默返回（不抛错）
- `remove_skill(name)` 传入不存在的名称时，应记录警告日志并静默返回（不抛错）
- `add_skill(path)` 传入缺少 `SKILL.md` 的目录时，应返回明确错误
- `add_skill` 遇到重复 skill（基于 SKILL.md 的 SHA-256 哈希），应静默跳过
- `get_backend()` 在 workspace 未初始化时调用应返回错误（`RuntimeError` 等价）
- 工作空间已 alive 时重复调用 `initialize()` 应为幂等操作（no-op）
- `offload_tool_result` 的 id 发生冲突时，应自动添加数字后缀 `(1)`、`(2)` 避免覆盖
- 工作空间路径必须解析为绝对路径（无论用户传入相对还是绝对路径）
- 当工作空间不可持久化（如 docker 无 bind-mount），`.mcp` 写入操作应为 no-op

## Requirements *(mandatory)*

### Functional Requirements

#### 核心抽象

- **FR-001**: 系统 MUST 提供 `WorkspaceBase` trait/抽象，定义 workspace 的统一接口：`initialize()`、`close()`、`reset()`、`get_instructions()`、`list_tools()`、`list_mcps()`、`list_skills()`、`add_mcp()`、`remove_mcp()`、`add_skill()`、`remove_skill()`、`offload_context()`、`offload_tool_result()`、`get_backend()`
- **FR-002**: `WorkspaceBase` MUST 包含 `workspace_id: String`（唯一标识符）、`workdir: String`（Agent 可见的工作根目录）、`is_alive: bool`（存活状态标志）
- **FR-003**: `WorkspaceBase` MUST 实现 RAII 风格的生命周期管理（drop 时自动 close），并提供显式的 async `initialize()` / `close()` 方法

#### 目录结构

- **FR-004**: 工作空间的目录布局 MUST 遵循固定结构：`{workdir}/data/`（卸载的多模态数据）、`{workdir}/skills/`（技能子目录）、`{workdir}/sessions/`（会话上下文和工具结果）、`{workdir}/.mcp`（持久化的 MCP 配置 JSON）
- **FR-005**: `workdir` 路径 MUST 在初始化时解析为绝对路径，并在 `initialize()` 时自动创建所需目录树
- **FR-006**: `data/`、`skills/`、`sessions/` 子目录路径 MUST 通过 derived/计算属性获得，而非可变的存储字段

#### 工具提供

- **FR-007**: `list_tools()` MUST 返回一组绑定到当前工作空间 Backend 的内置工具：Bash(绑定 workdir)、Edit、Glob、Grep、Read、Write
- **FR-008**: 工具 MUST 通过 workspace 的 Backend 执行所有文件系统和进程 I/O 操作，确保操作限定在工作空间范围内

#### MCP 管理

- **FR-009**: 系统 MUST 支持通过 `add_mcp(mcp_client)` 动态注册 MCP 客户端，注册即持久化到 `.mcp` 文件
- **FR-010**: 系统 MUST 支持通过 `remove_mcp(name)` 按名称移除已注册的 MCP 客户端，同时从内存和持久化文件中删除
- **FR-011**: `list_mcps()` MUST 返回当前所有已注册的 MCP 客户端
- **FR-012**: `initialize()` 时 MUST 从 `.mcp` 文件恢复之前持久化的 MCP 配置；若文件不存在则使用 `default_mcps` 初始化并持久化
- **FR-013**: `.mcp` 文件损坏时 MUST 使用 `default_mcps` 重新播种，不引发致命错误

#### Skill 管理

- **FR-014**: 系统 MUST 支持通过 `add_skill(skill_path)` 将本地 skill 目录复制到 `{workdir}/skills/` 下
- **FR-015**: Skill 目录 MUST 包含有效的 `SKILL.md` 文件（包含 `name` 和 `description` frontmatter），否则拒绝添加
- **FR-016**: 系统 MUST 基于 SKILL.md 内容的 SHA-256 哈希进行去重——相同哈希的 skill 静默跳过
- **FR-017**: 系统 MUST 支持通过 `remove_skill(name)` 按名称删除 skill 目录
- **FR-018**: `list_skills()` MUST 扫描 `skills/` 目录，解析所有 `SKILL.md` 并返回 `Skill` 对象列表（含 name、description、dir、markdown、updated_at）
- **FR-019**: 当 skill 的 agent-facing 名称冲突时，MUST 自动添加数字后缀（如 "my-skill" → "my-skill (1)"）
- **FR-020**: Skill 管理操作 MUST 在并发锁保护下执行

#### 上下文卸载

- **FR-021**: `offload_context(session_id, msgs)` MUST 将消息批量追加写入 `{workdir}/sessions/{session_id}/context.jsonl`
- **FR-022**: offload_context 中的 base64 `DataBlock` 数据 MUST 被提取到 `data/` 目录（文件名为 `{sha256}.{ext}`）并替换为 `file://` URL 引用
- **FR-023**: 同一 base64 数据块的二次 offload MUST 通过哈希检测跳过重复写入
- **FR-024**: `offload_tool_result(session_id, tool_result)` MUST 将工具结果写入 `{workdir}/sessions/{session_id}/tool_result-{id}.txt`
- **FR-025**: offload_tool_result 在文件名冲突时 MUST 自动添加数字后缀

#### 重置与清理

- **FR-026**: `reset()` MUST 清空 workspace 内容：关闭并移除所有 MCP，删除 `skills/`、`sessions/`、`data/` 内容，清空 `.mcp`
- **FR-027**: `reset()` MUST NOT 重新播种 `default_mcps` 和 `skill_paths`——重置到空状态而非初始状态
- **FR-028**: `close()` MUST 关闭所有有状态（stateful）MCP 连接，将 `is_alive` 设为 `false`

#### 多租户管理

- **FR-029**: 系统 SHOULD 提供 `WorkspaceManager` 用于多租户场景下的 workspace 生命周期管理
- **FR-030**: `WorkspaceManager.get(key)` SHOULD 按 key 返回/创建 workspace，同一 key 始终返回同一实例
- **FR-031**: `WorkspaceManager` SHOULD 支持基于 TTL 的惰性淘汰，超时未访问的 workspace 自动关闭回收

#### 工作空间指令

- **FR-032**: `get_instructions()` MUST 返回 workspace 特定的系统提示片段，引导 Agent 在工作空间内正确组织文件和项目
- **FR-033**: 默认指令 MUST 包含：项目子目录命名规范（日期前缀）、README.md 要求、版本控制建议、临时文件管理、Python 环境隔离指南

#### 安全与隔离

- **FR-034**: 系统 MUST 防止路径遍历攻击——skill 复制操作必须验证目标路径在工作空间范围内
- **FR-035**: 所有 Backend 文件操作 MUST 通过 `get_backend()` 获取当前后端，不得保留过期的 Backend 引用
- **FR-036**: `workdir` 路径 MUST 解析为规范化的绝对路径，防止符号链接逃逸

### Key Entities *(include if feature involves data)*

- **Workspace（工作空间）**: 一个隔离的执行环境，包含唯一的 `workspace_id`、Agent 可见的 `workdir` 根路径、标准子目录布局（data/skills/sessions）、已注册的 MCP 客户端列表、已安装的技能列表、`is_alive` 状态标志。通过 Backend 提供文件系统和进程执行能力。

- **Backend（执行后端）**: 抽象文件系统和进程 I/O 接口。LocalBackend 直接在本地文件系统上操作，DockerBackend 在容器内操作，E2BBackend 在云沙箱内操作。为工具和 workspace 操作提供统一的 `exec_shell`、`read_file`、`write_file`、`list_dir`、`is_dir`、`file_exists`、`delete_path`、`join_path`、`basename`、`dirname` 等方法。

- **MCP Client（MCP 客户端）**: Model Context Protocol 客户端实例，通过 name 标识，包含连接配置（服务器 URL、传输方式等）。可以是 stateful（持久长连接，如 stdio/SSE）或 stateless（每次调用独立连接，如 HTTP）。注册在 workspace 中，持久化在 `.mcp` 文件。

- **Skill（技能）**: 可复用的领域知识模块，以目录形式组织，包含一个 `SKILL.md` 文件（YAML frontmatter 含 name/description）和相关的代码/文档文件。通过名称标识，基于内容 SHA-256 哈希进行去重。

- **Session Offload（会话卸载数据）**: 按 session_id 分区存储的卸载数据：`context.jsonl`（对话上下文，JSONL 格式一行一条消息）和 `tool_result-{id}.txt`（工具执行结果文本文件）。用于超出 LLM 上下文窗口限制的历史数据持久化。

- **WorkspaceManager（工作空间管理器）**: 多租户场景下的 workspace 注册中心，维护 `HashMap<Key, Workspace>` 映射。支持工厂函数动态创建 workspace、基于 TTL 的惰性淘汰策略、按隔离策略（共享 vs 隔离）分配 workspace。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 开发者可以在 5 行代码以内创建并初始化一个 `LocalWorkspace` 实例
- **SC-002**: `LocalWorkspace` 初始化时间不超过 100ms（不含 MCP 连接时间）
- **SC-003**: 上下文卸载 (`offload_context`) 处理 100 条消息（含 10 个 base64 数据块）的时间不超过 2 秒
- **SC-004**: 所有 workspace 操作在未初始化状态下调用时返回可理解的错误，不会 panic/crash
- **SC-005**: `reset()` 完成后 workspace 处于干净状态，`list_skills()` 和 `list_mcps()` 返回空列表，`data/`、`skills/`、`sessions/` 目录为空
- **SC-006**: 与 Python AgentScope 的 `LocalWorkspace` 保持外部行为兼容——相同的输入产生等价的目录结构、MCP 配置文件和 offload 输出
- **SC-007**: 100% 的公开 API 有对应的测试覆盖（单元测试 + 集成测试）

## Assumptions

- 假设立即实现的是 `LocalWorkspace`（基于本地文件系统），Docker/E2B/K8s 等沙箱后端留待后续 Feature 实现
- `WorkspaceManager` 在本次 Feature 中实现基本版本（内存映射 + TTL 淘汰），不包含分布式共享等高级特性
- 假设现有的 `agent_scope_tool` crate 中的 `BackendBase` trait 和内置工具（Bash/Edit/Glob/Grep/Read/Write）已经可用，Workspace 直接使用它们
- 假设 `agent_scope_memory` 中的 `MCPClient`、`Skill` 类型已可用，或可定义适配层
- 假设 `agent_scope_message` 中的 `Msg`、`DataBlock`、`Base64Source`、`URLSource`、`ToolResultBlock`、`TextBlock` 类型已可用
- Offload 协议中的文件引用使用标准的 `file://` URI 格式
- Workspace 的指令模板基于 Python 版本的 `DEFAULT_WORKSPACE_INSTRUCTIONS` 翻译和适配
- 新的 crate 命名为 `agent_scope_workspace`，放在 `crates/agent_scope_workspace/`
- 所有路径操作使用 `/` 作为分隔符（Rust 的 `std::path::Path` 会自动处理平台差异）

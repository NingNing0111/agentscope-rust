# Feature Specification: MCP SDK Integration

**Feature Branch**: `027-mcp-sdk-integration`

**Created**: 2026-08-07

**Status**: Draft

**Input**: User description: "引入官方MCP SDK，重构 agent_scope_workspace 里面的mcp模块：https://github.com/modelcontextprotocol/rust-sdk"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 连接外部 MCP 服务器并发现其工具 (Priority: P1)

作为 Agent 开发者，我希望工作空间里注册的 MCP 客户端能够真正连接到外部 MCP 服务器（stdio 子进程或远程 HTTP 端点），列出该服务器提供的工具清单，让 Agent 的能力边界扩展到工作空间之外。

**Why this priority**: 当前 workspace 的 MCP 能力只有"配置存储"——`add_mcp()`/`list_mcps()` 只是把 JSON 配置写进 `.mcp` 文件，从未建立真实连接。没有连接和工具发现，MCP 配置就没有任何运行时价值。这是本次重构的核心价值。

**Independent Test**: 注册一个指向已知 MCP 测试服务器（stdio 或进程内 worker）的客户端配置，建立连接，调用工具列表接口，断言返回的工具数量大于零且每个工具都有名称和输入模式。

**Acceptance Scenarios**:

1. **Given** 工作空间中已注册一个 stdio 传输的 MCP 客户端配置, **When** 请求建立连接, **Then** 握手成功，返回的服务器工具列表包含预期的工具名称
2. **Given** 工作空间中已注册一个 HTTP 传输（streamable-http）的 MCP 客户端配置, **When** 请求建立连接, **Then** 连接成功并返回服务器工具列表
3. **Given** MCP 服务器不可达或握手失败, **When** 请求建立连接, **Then** 返回类型化错误，且错误信息不泄露任何认证秘密

---

### User Story 2 - 调用远端 MCP 工具 (Priority: P1)

作为 Agent 开发者，我希望 Agent 能像调用本地工具一样调用远端 MCP 工具，传入 JSON 参数并获得结构化结果，从而把外部服务的能力直接接入 Agent 的推理-行动循环。

**Why this priority**: 工具调用是 MCP 的实际价值所在。仅有配置和工具发现而无法调用，Agent 仍无法使用外部能力。此场景把远端工具接入现有 `Tool` 抽象，使 Agent 循环无需感知工具在本地还是远端。

**Independent Test**: 通过统一工具接口调用一个已连接 MCP 服务器的已知工具，传入合法参数，断言返回结构化结果；再传入非法参数，断言返回类型化错误。

**Acceptance Scenarios**:

1. **Given** 已连接一个提供计算工具的 MCP 服务器, **When** Agent 通过统一工具接口调用该工具并传入合法 JSON 参数, **Then** 返回结构化工具结果，结果内容与服务器实际输出一致
2. **Given** 工具输入参数不符合服务器要求, **When** 调用该工具, **Then** 返回类型化错误，指明工具名与失败原因
3. **Given** 会话关闭或工作空间重置, **When** 有状态 MCP 连接存在, **Then** 所有有状态连接被断开，资源被释放

---

### User Story 3 - MCP 配置演进：从 SSE 迁移到 streamable-http (Priority: P2)

作为工作空间管理员，我希望既有的 SSE 传输配置在升级后仍能工作——通过配置映射将旧格式自动映射到新的传输能力，避免升级后既有 `.mcp` 文件失效。

**Why this priority**: 官方 SDK 明确以 streamable-http 替代旧版 HTTP+SSE 传输。若直接丢弃 SSE，既有用户升级会破坏存量配置；提供显式的配置迁移路径降低升级摩擦。

**Independent Test**: 加载一个含 SSE 传输配置的 `.mcp` 文件，验证配置被成功解析并映射到新的传输配置，且映射过程记录提示信息。

**Acceptance Scenarios**:

1. **Given** 存量 `.mcp` 文件中存在 SSE 传输配置, **When** 加载该文件, **Then** 配置被解析并显式映射到新的传输类型，映射过程返回提示而非静默失败
2. **Given** 配置中含敏感请求头, **When** 持久化或列出配置, **Then** 敏感头值始终被脱敏，绝不写入 `.mcp` 文件或返回给调用方

---

### User Story 4 - 进程内 MCP 测试与开发体验 (Priority: P3)

作为测试工程师，我希望无需启动真实外部服务器即可验证 MCP 集成逻辑，通过进程内 worker 传输运行完整的功能测试，使 CI 稳定、快速且不依赖网络。

**Why this priority**: 真实服务器依赖进程和网络，使测试不稳定且慢。进程内传输让核心集成逻辑在 CI 中可确定性验证，同时保留真实传输的集成测试作为补充。

**Independent Test**: 在 `#[tokio::test]` 中用进程内 worker 传输起一个测试 MCP 服务器，执行连接→列出工具→调用工具的完整流程，断言全部通过。

**Acceptance Scenarios**:

1. **Given** CI 环境无外部网络, **When** 运行 MCP 集成测试, **Then** 核心流程测试全部通过（使用进程内传输，不依赖外部服务）
2. **Given** 测试需要验证错误处理, **When** 注入握手失败、工具调用失败等场景, **Then** 返回类型化错误，测试可断言错误类别

---

### Edge Cases

- 配置的 MCP 服务器进程不存在（stdio 子进程启动失败）时，返回类型化错误，不 panic
- 远端服务器在工具调用过程中断开连接，返回带重试/连接状态信息的错误
- 同一 MCP 客户端被重复注册时，明确拒绝并返回已存在错误（保持现有 `add_mcp` 语义）
- `.mcp` 文件包含未知字段或未来版本的配置格式时，宽容解析（未知字段忽略），不整体失败
- 工具发现结果包含非 JSON 序列化的扩展字段时，宽容处理，不丢失已知字段
- 无状态（stateless）HTTP 工具调用在有状态连接建立前被发起时，按配置按需建立连接或返回清晰错误
- 敏感请求头（authorization、x-api-key 等）出现在任意传输类型时，持久化和读取路径均脱敏

## Requirements *(mandatory)*

### Functional Requirements

#### MCP 配置层（配置演进）

- **FR-001**: 系统 MUST 保留 `McpTransportConfig` 与 `McpClientConfig` 作为公开配置类型，维护 `add_mcp()`、`remove_mcp()`、`list_mcps()` 的现有工作空间接口语义
- **FR-002**: 系统 MUST 支持 stdio、streamable-http 两种传输配置；对旧版 SSE 配置 MUST 提供显式迁移路径（解析成功 + 返回提示），不得静默丢弃
- **FR-003**: 敏感请求头（authorization、proxy-authorization、x-api-key、x-auth-token、cookie、set-cookie）MUST 在持久化 `.mcp` 文件与 `list_mcps()` 返回值中始终脱敏
- **FR-004**: 配置解析 MUST 采用宽容策略：`.mcp` 文件中的未知字段被忽略，不导致整体反序列化失败
- **FR-005**: `.mcp` 文件损坏（非合法配置）时 MUST 回退到 `default_mcps` 播种并记录警告，不崩溃

#### 连接与工具发现层

- **FR-006**: 系统 MUST 能基于已注册配置建立到 MCP 服务器的真实连接（stdio 子进程或 streamable-http），并完成协议握手
- **FR-007**: 系统 MUST 能从已连接服务器获取工具清单，每个工具包含名称、描述和输入 JSON Schema
- **FR-008**: 系统 MUST 支持按名称调用远端 MCP 工具，传入 JSON 参数，返回结构化结果
- **FR-009**: 连接、握手、工具调用等所有失败 MUST 返回类型化错误，区分连接失败、协议错误、工具调用失败和取消，且错误信息不泄露认证秘密
- **FR-010**: 会话关闭、工作空间 `close()`/`reset()` 时，MUST 断开所有有状态 MCP 连接并释放子进程等资源
- **FR-011**: 系统 MUST 提供无状态 HTTP 调用的按需连接语义：工具调用前未建立持久连接时，可按配置建立一次性连接或返回清晰错误

#### 与 Tool 系统的集成

- **FR-012**: 远端 MCP 工具 MUST 通过统一的工具接口暴露给 Agent 循环——远程工具与本地工具使用相同调用契约，Agent 无需区分来源
- **FR-013**: 远端工具的输入 Schema 与调用结果 MUST 通过现有工具抽象适配，工具名冲突时 MUST 有明确的命名/去重策略
- **FR-014**: 工具调用 MUST 遵循现有 Tool 抽象的错误与取消契约，取消请求正确传播到远端调用

#### 测试基础设施

- **FR-015**: 系统 MUST 提供进程内 MCP worker 传输（或等价机制）用于确定性测试，使核心集成逻辑在无网络环境中可验证
- **FR-016**: 每个公开 MCP 能力 MUST 有对应测试：连接、工具发现、工具调用、错误处理、配置迁移、敏感信息脱敏

### Key Entities *(include if feature involves data)*

- **MCP 客户端配置（McpClientConfig）**: 持久化于 `.mcp` 文件的注册信息：唯一名称、传输配置（类型 + 端点/命令）、有状态标志。升级后作为连接的输入描述，而非仅存储条目。

- **MCP 服务器（MCP Server）**: 外部能力提供者。通过 stdio 子进程或 streamable-http 端点访问，暴露工具清单与可调用工具。连接生命周期由工作空间持有。

- **远端工具（Remote Tool）**: 由 MCP 服务器提供、经适配后暴露为统一工具接口的工具。承载服务器返回的名称、描述、输入 JSON Schema 与调用转发逻辑。

- **统一工具契约（Tool Contract）**: 本地与远端工具共享的调用接口——输入 JSON Schema、执行调用、返回结构化结果、类型化错误与取消语义。

- **MCP 会话（MCP Session）**: 一次成功的协议握手建立的工作单元。有状态会话在 `close()`/`reset()` 时被显式关闭并释放资源。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 开发者使用 3 个以内 API 调用即可完成"注册配置 → 连接 → 获取工具清单"的流程
- **SC-002**: 从已连接服务器发现并调用工具的成功率，在合法输入下为 100%（进程内测试），真实传输下以集成测试覆盖
- **SC-003**: 既有 `.mcp` 文件（含 SSE 配置）升级后无需手工编辑即可加载，迁移过程有明确提示
- **SC-004**: 任何 MCP 连接、握手或调用失败的场景都不会导致进程 panic，且返回可机器判别的错误类别
- **SC-005**: 敏感请求头在 `.mcp` 文件与 `list_mcps()` 返回值中的出现次数为 0（回归测试断言）
- **SC-006**: 全部 MCP 集成测试在无网络 CI 环境通过（进程内传输），且公开 API 测试覆盖率达到 100%
- **SC-007**: 与 Python AgentScope 的 `MCPClient`/`MCPTool` 适配行为保持外部一致——相同配置产生等价的工具清单与调用结果（与 `specs/001` 兼容性矩阵对齐）

## Assumptions

- 采用官方 Rust MCP SDK（`rmcp`，modelcontextprotocol/rust-sdk）作为唯一的 MCP 协议实现，不自研协议层
- 官方 SDK 以 streamable-http 作为 HTTP 传输标准，旧版 HTTP+SSE（SSE 传输）通过显式映射保留兼容路径，映射行为记录为已知偏差
- MCP 客户端能力聚焦于**工具（tools）**协议：列表、调用；资源（resources）与提示词（prompts）协议留待后续 Feature
- stdio 传输要求运行环境可启动子进程；`nix` 进程组回收逻辑（现有 Backend 能力）复用于子进程生命周期管理
- 远端工具通过适配层接入现有 `agent_scope_tool::Tool` 抽象，复用其统一 Schema、错误与取消契约
- `.mcp` 文件格式在本次升级中保持向后兼容：新增字段可选，旧字段含义不变
- 进程内测试传输（`transport-worker`）用于 CI 确定性验证；真实 stdio/HTTP 传输的集成测试标记为可选（需环境支持）
- 本次重构不改变 `WorkspaceBase` 公开方法签名；能力增强通过实现层与新增类型完成
- 多租户并发访问同一 MCP 连接的互斥策略沿用现有 workspace 的锁机制（`_mcp_lock`）

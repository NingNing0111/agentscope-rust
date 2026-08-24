# Feature Specification: Microsandbox Sandbox Backend（基于 microsandbox 的强隔离沙箱后端）

**Feature Branch**: `035-microsandbox-sandbox-backend`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description: "新增一个基于microsandbox的沙箱实现，这个作为一个feture，"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 创建并运行 microsandbox 会话 (Priority: P1) 🎯 MVP

作为开发者，我希望通过 Rust API 创建一个基于 microsandbox microVM 的沙箱会话，在该会话内执行命令、读写文件并获取结构化执行结果，从而让不可信或半可信代码运行在真实硬件隔离边界内，而不是 local-process 参考后端内。

**Why this priority**: 这是用户请求的核心价值。没有可创建、可执行、可清理的 microsandbox 会话，后续 Workspace、Tool、Agent 集成都没有真实强隔离后端可用。

**Independent Test**: 在启用 `microsandbox` feature 且本机 runtime 可用时，创建 `MicrosandboxSession`，初始化后执行 `python -c "print('hello')"`，验证退出码、stdout/stderr、执行历史和 cleanup 行为；runtime 不可用时返回稳定错误且不回退到 local-process。

**Acceptance Scenarios**:

1. **Given** 一个配置了 image、workdir 和默认策略的 `MicrosandboxSession`, **When** 用户调用 `initialize()`, **Then** 系统创建 microsandbox microVM 并进入 `Ready` 状态
2. **Given** microsandbox 会话处于 `Ready`, **When** 用户执行一条返回文本的命令, **Then** 系统返回 `ExecutionResult`，包含退出状态、stdout/stderr 摘要、开始/结束时间和耗时
3. **Given** 命令返回非零退出码, **When** microsandbox SDK 成功完成 exec, **Then** 系统将其记录为 `ExecutionStatus::Exited { code }`，而不是 sandbox 系统错误
4. **Given** microsandbox runtime、平台或 SDK 初始化不可用, **When** 用户创建或初始化会话, **Then** 系统返回稳定错误，且 MUST NOT 静默回退到 `LocalSandboxSession`

---

### User Story 2 - 复用现有 SandboxSession 契约 (Priority: P1)

作为库维护者，我希望 `MicrosandboxSession` 实现现有 `SandboxSession` trait，并复用 `SandboxPolicy`、`ExecutionRequest`、`ExecutionResult`、`ExecutionRecord`、`SandboxError`、`CapabilityReport` 等公共模型，从而避免为 microsandbox 新建一套不兼容 API。

**Why this priority**: 现有 Feature 017 已定义 sandbox 核心抽象。新后端应增强实现能力，而不是分裂调用方 API 或破坏 local backend。

**Independent Test**: 编写 contract helper，以 `Box<dyn SandboxSession>` 形式分别驱动 `LocalSandboxSession` 和 feature-gated `MicrosandboxSession`，验证生命周期、命令执行、文件 API、history、capability report 的结构一致性。

**Acceptance Scenarios**:

1. **Given** 调用方持有 `Box<dyn SandboxSession>`, **When** 其中实际对象是 `MicrosandboxSession`, **Then** 调用方可以使用同一 trait 方法完成初始化、执行、文件操作、查询历史和关闭
2. **Given** `SandboxPolicy` 含有 timeout/output/network/memory/cpu/mount 配置, **When** microsandbox 后端能稳定支持该配置, **Then** 系统将其映射到 microsandbox SDK
3. **Given** `SandboxPolicy` 请求了 microsandbox SDK 或当前平台不能稳定支持的能力, **When** 用户初始化或执行相关操作, **Then** 系统返回 `UnsupportedFeature` 或等价稳定错误，并在 `CapabilityReport` 中登记
4. **Given** local-process 后端仍存在, **When** 新 feature 合入, **Then** `LocalSandboxSession` 既有行为、能力报告和 unsupported feature 语义不回归

---

### User Story 3 - 将 Workspace/Tool/Agent 绑定到 microsandbox 后端 (Priority: P2)

作为 Agent 使用者，我希望现有 Workspace built-in tools（Bash、Read、Write、Edit、Grep、Glob、ResetTools）可以通过同一个 microsandbox 会话运行，使 Agent 的命令和文件操作都发生在 microVM 隔离边界内。

**Why this priority**: 单独的 sandbox API 只解决底层执行问题；接入 Workspace 后，Agent 才能在真实任务中自然使用 microsandbox，而无需修改每个工具。

**Independent Test**: 构造包裹 `MicrosandboxSession` 的 `SandboxWorkspaceBackend`，通过 `WorkspaceBackend` contract 执行 Bash/Read/Write/List/Delete，再用 `BuiltInToolContext` 驱动 BashTool/ReadTool/WriteTool 的基本流程。

**Acceptance Scenarios**:

1. **Given** `SandboxWorkspaceBackend` 持有 `MicrosandboxSession`, **When** Workspace 工具调用 `exec_shell`, **Then** 命令在 microVM 中运行并返回标准化 `ExecOutput`
2. **Given** Agent 配置了 microsandbox-backed Workspace, **When** `react_agent` 自动注入 workspace tools, **Then** Bash/Read/Write/Edit/Grep/Glob 共享同一个 microsandbox 文件系统边界
3. **Given** 现有调用方使用 `SandboxWorkspaceBackend::new(LocalSandboxSession)`, **When** 新实现合入, **Then** 该 API 保持可用且 local tests 继续通过
4. **Given** microsandbox 初始化失败, **When** 上层请求 Workspace backend, **Then** 系统返回显式错误，不创建 local fallback backend

---

### User Story 4 - 资源、网络、挂载和审计能力可追踪 (Priority: P2)

作为平台开发者，我希望 microsandbox 后端能把资源限制、网络策略、挂载策略和执行审计记录映射到可验证的能力声明，使不同信任等级的任务可以选择合适的隔离强度，并能追踪不支持能力。

**Why this priority**: microsandbox 的主要价值是强隔离与可控资源边界；但为了不伪兼容，所有能力必须能被测试、记录和稳定失败。

**Independent Test**: 使用默认 CI 中不依赖 runtime 的单元测试验证 policy mapping、capability report、unsupported feature 和 error mapping；使用 ignored real-runtime tests 验证 no-net、readonly mount、timeout、output ref 和 cleanup。

**Acceptance Scenarios**:

1. **Given** 策略配置 `NetworkPolicy::Disabled`, **When** microsandbox 后端初始化, **Then** 系统使用 microsandbox no-net 或等价机制禁用网络
2. **Given** 策略配置 host allowlist 但 SDK 不能精确支持, **When** 用户初始化会话, **Then** 系统返回 `UnsupportedFeature`，不得放宽为 unrestricted 网络
3. **Given** 策略配置 `memory_limit_bytes`, **When** microsandbox 后端支持内存限制, **Then** 系统按 MiB 映射到 SDK `.memory(...)` 并保留边界检查
4. **Given** 策略配置 `cpu_limit.cpu_shares` 或 `process_limit` 而 SDK 语义不能稳定等价, **When** 用户请求该策略, **Then** 系统显式拒绝或登记为 unsupported，不得伪映射
5. **Given** 某次命令输出超过 `max_output_bytes`, **When** 用户读取执行结果或 history, **Then** inline 输出被截断，完整输出引用带 sha256 和字节数

---

### Edge Cases

- microsandbox runtime 未安装、版本不兼容、平台不支持 KVM/Apple Silicon、镜像拉取失败或 SDK create 失败时，必须返回稳定错误，不得静默回退到 local-process
- Cloud backend 必须由用户显式选择；不得从 `MSB_API_KEY` 或 `MSB_API_URL` 推断 Cloud
- sandbox stdout/stderr/log/file 内容均是不可信数据，绝不能作为系统、用户或开发者指令执行
- 网络默认采用最小权限；不可信代码默认 no-net 或严格 allowlist
- 不得将 `~/.ssh`、`~/.aws`、`~/.config`、token 目录、credential 文件或真实 secret 暴露给 sandbox
- 命令返回非零退出码必须作为执行结果返回，不应误判为 SDK/系统故障
- `ExecutionRequest` 为空命令、cwd 越界、timeout 超出 `max_timeout`、env key 非法、mount 源不存在时应返回可理解的参数错误
- 路径规范化必须处理 `..`、重复分隔符、绝对路径和 mount 权限，microVM 隔离之外仍要保留 Rust 层授权检查
- 只读 mount 写入必须稳定拒绝；若 SDK 无法强制只读语义，则该 mount 配置必须 UnsupportedFeature
- `close()`、`cleanup()` 必须幂等；初始化失败后应清理已创建但未交付的资源
- `keep_on_close` / persistent sandbox 行为必须由调用方显式配置，不得意外保留临时资源
- 默认 CI 环境不应要求真实 microsandbox runtime；真实 e2e tests 必须 feature-gated 且 `#[ignore]`

## Requirements *(mandatory)*

### Functional Requirements

#### microsandbox 会话生命周期

- **FR-001**: 系统 MUST 在 `agent_scope_sandbox` 中新增 feature-gated microsandbox 后端，而不是创建与现有 sandbox 抽象平行的独立体系
- **FR-002**: 系统 MUST 提供 `MicrosandboxConfig`，至少包含 session_id、image、workdir、policy、mounts、env、replace_existing 和 keep/persist 相关配置
- **FR-003**: 系统 MUST 提供 `MicrosandboxSession`，并实现现有 `SandboxSession` trait
- **FR-004**: `MicrosandboxSession::initialize()` MUST 创建 microsandbox microVM，并在成功后进入 `SandboxState::Ready`
- **FR-005**: 初始化、关闭和清理操作 MUST 幂等或返回稳定生命周期错误；初始化失败 MUST 清理部分资源
- **FR-006**: microsandbox runtime/platform/SDK 不可用时 MUST 返回稳定错误，MUST NOT 自动使用 `LocalSandboxSession` 兜底

#### 命令执行与输出

- **FR-007**: 系统 MUST 将 `ExecutionRequest.command/cwd/env/timeout/stdin` 映射到 microsandbox exec/exec_with 或等价 SDK API
- **FR-008**: 系统 MUST 区分命令非零退出码与 sandbox 系统错误；非零退出码 MUST 表示为 `ExecutionStatus::Exited { code }`
- **FR-009**: 系统 MUST 支持 timeout，并在超时时返回 `ExecutionStatus::TimedOut` 或稳定 timeout 错误，同时保留可诊断记录
- **FR-010**: 系统 MUST 继续支持 stdout/stderr inline 截断、完整输出引用、sha256、字节数和 execution history
- **FR-011**: 系统 MUST 使用 `redacted_command_summary` 或等价机制记录命令摘要，不在 history/log/error 中泄露敏感 env 明文

#### 文件、路径与挂载

- **FR-012**: 系统 MUST 通过 microsandbox fs API 或等价机制实现 `read_file/write_file/delete_path/is_dir/stat_mtime/list_dir`
- **FR-013**: 系统 MUST 将所有路径操作限制在 sandbox 授权 workdir 或显式 mount 内，并拒绝路径遍历与未授权绝对路径
- **FR-014**: 系统 MUST 支持只读和可写 mount 的可观察语义；无法稳定实现时 MUST 返回 `UnsupportedFeature`
- **FR-015**: 系统 MUST 不自动挂载宿主敏感路径或注入 host credential；secret 注入若不在本 feature 范围内，必须明确 unsupported

#### Workspace / Tool / Agent 集成

- **FR-016**: `SandboxWorkspaceBackend` MUST 被泛化为可持有任意 `SandboxSession` trait object 或等价抽象，而非硬编码 `LocalSandboxSession`
- **FR-017**: 系统 MUST 保留 `SandboxWorkspaceBackend::new(LocalSandboxSession)` 的现有调用兼容性
- **FR-018**: 系统 SHOULD 提供 `SandboxWorkspaceBackend::from_session` 或 `from_boxed_session`，用于接入 `MicrosandboxSession`
- **FR-019**: Bash/Read/Write/Edit/Grep/Glob/ResetTools 工具 SHOULD 无需逐个修改即可通过 `WorkspaceBackend` 使用 microsandbox 后端
- **FR-020**: Agent 通过 `AgentConfig::workspace(...)` 使用 microsandbox-backed Workspace 时，workspace tool 注入行为 MUST 与 local backend 保持结构兼容

#### 策略、能力和错误模型

- **FR-021**: 系统 MUST 新增 microsandbox 专用 policy validation/mapping helper，避免改变 `LocalSandboxSession` 的 unsupported feature 语义
- **FR-022**: `NetworkPolicy::Disabled` MUST 映射为 no-net 或等价禁网；无法精确支持的 network allowlist MUST 显式 unsupported，不得放宽
- **FR-023**: `memory_limit_bytes` SHOULD 映射到 SDK memory 限制；单位转换和越界 MUST 有测试覆盖
- **FR-024**: `cpu_limit.cpu_shares` 和 `process_limit` 若无法与 SDK 能力稳定等价，MUST 返回 `UnsupportedFeature` 或登记为 unsupported
- **FR-025**: 系统 MUST 提供 `CapabilityReport::microsandbox()` 或等价能力报告，列出 supported/unsupported/known deviations
- **FR-026**: 所有 SDK/runtime/platform 错误 MUST 映射到稳定 `SandboxError` 类别；错误消息不得泄露 secret

#### 文档、测试与 feature gate

- **FR-027**: `microsandbox` crate dependency MUST 通过 Cargo feature gate 引入，默认 CI 不应要求 runtime 可用
- **FR-028**: 系统 MUST 提供不依赖真实 runtime 的默认测试，覆盖 config/policy/capability/error/workspace adapter regression
- **FR-029**: 系统 MUST 提供 feature-gated + ignored 的真实 microsandbox integration tests，并在 quickstart 中记录运行方式
- **FR-030**: docs/examples MUST 明确 local-process 后端与 microsandbox microVM 后端的隔离等级差异、安全约束和 runtime 要求

### Key Entities *(include if feature involves data)*

- **Microsandbox Config（microsandbox 配置）**: 描述创建 microVM sandbox 所需的 image、workdir、策略、挂载、环境变量、替换/持久化行为和启动/停止超时。
- **Microsandbox Session（microsandbox 会话）**: `SandboxSession` 的强隔离实现，持有 microsandbox SDK handle、生命周期状态、策略、执行历史和输出引用。
- **Sandbox Policy Mapping（策略映射）**: 将 `SandboxPolicy` 中的 timeout、output、network、memory、cpu、process 和 mount 语义映射到 microsandbox SDK 或明确 unsupported 的规则集合。
- **Sandbox Workspace Backend（沙箱工作空间后端）**: 将任意 `SandboxSession` 适配为 `WorkspaceBackend` 的桥接层，使 Workspace tools 在同一沙箱边界内执行。
- **Capability Report（能力报告）**: microsandbox 后端对强隔离、网络、资源限制、挂载、审计和已知偏差的稳定声明。
- **Execution Record（执行记录）**: 单次命令执行的审计条目，包含 redacted command summary、状态、耗时、失败类别、stdout/stderr 摘要和 full output refs。

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 启用 `microsandbox` 后，开发者可以在 15 行以内创建 `MicrosandboxSession`、初始化、执行命令并获得结构化 `ExecutionResult`
- **SC-002**: 默认测试套件不依赖 microsandbox runtime，且覆盖 config validation、policy mapping、capability report、error mapping、workspace adapter local regression
- **SC-003**: 真实 runtime 可用时，ignored integration tests 覆盖成功执行、非零退出、timeout、文件读写、输出截断、network disabled、readonly mount 和 cleanup 幂等
- **SC-004**: `SandboxWorkspaceBackend::new(LocalSandboxSession)` 既有调用不破坏；新增 `from_session/from_boxed_session` 可包裹 `MicrosandboxSession`
- **SC-005**: Bash/Read/Write/Edit/Grep/Glob 至少一个端到端测试证明可通过 microsandbox-backed Workspace 共享同一 microVM 文件系统
- **SC-006**: runtime/platform/SDK 不可用、unsupported policy、非法路径、只读写入等失败路径 100% 返回稳定错误，不存在 local-process silent fallback
- **SC-007**: `CapabilityReport::local_process()` 或现有 local 报告保持不变；microsandbox report 明确强隔离能力与所有 known deviations
- **SC-008**: `rtk cargo fmt --check`、`rtk cargo check --workspace --all-targets`、`rtk cargo test --workspace`、`rtk cargo clippy --workspace --all-targets -- -D warnings` 通过；真实 runtime gate 单独可选

## Assumptions

- microsandbox Rust SDK 版本锁定为 `0.6.10`，实际 API 以本仓库 microsandbox skill references 和实现期编译结果为准
- local-process sandbox 作为显式选择的 reference backend 保留，不升级为强隔离，也不作为 microsandbox 失败 fallback
- Cloud backend、secret injection、snapshot、registry auth、port publishing、long-running interactive TTY 不作为本 feature 默认范围；如后续支持需单独设计安全边界
- 默认网络策略采用安全优先原则：不可信代码默认 no-net 或严格 allowlist
- 真实 microsandbox runtime 可能只在 Linux KVM 或 macOS Apple Silicon 可用，因此默认 CI 仅运行 deterministic/unit tests
- Workspace、Tool、Agent 注入链路已由前序 features 提供，本 feature 优先通过泛化 `SandboxWorkspaceBackend` 接入，不改每个工具的 public API

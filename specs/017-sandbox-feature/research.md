# Research: Sandbox Feature

**Feature**: 017-sandbox-feature | **Date**: 2026-08-01

## 1. 沙箱后端边界

### Decision
新增 `agent_scope_sandbox` crate，定义 `SandboxSession`、`SandboxPolicy`、`SandboxMount`、`SandboxBackend` trait 与本地进程型 MVP 后端；Workspace 通过一个 `SandboxWorkspaceBackend` 适配到现有 `WorkspaceBackend` trait。

### Rationale
- Sandbox 是宪法第十六条路线图中的独立能力，不能把隔离策略塞进 `agent_scope_workspace` 的本地后端。
- 现有 `WorkspaceBackend` 已经是文件系统 + 进程 I/O 抽象，沙箱后端可复用其契约并为 Bash/Read/Write/Edit/Grep 等工具提供一致隔离边界。
- 独立 crate 保持依赖方向：Workspace 可以可选依赖 Sandbox，core/model/tool 不依赖具体隔离实现。

### Alternatives Considered
1. **直接扩展 `LocalBackend`**: 拒绝 — 会把“本地执行”伪装成“隔离执行”，违反不允许伪兼容。
2. **只实现 Docker 后端**: 拒绝 — 增加环境前置依赖，不适合作为 MVP。
3. **独立 Sandbox trait + Workspace 适配器**: 采纳 — API 边界清晰，可继续接 Docker/OpenSandbox/E2B。

## 2. MVP 隔离模型

### Decision
MVP 采用单机本地沙箱：每个会话创建独立临时根目录，所有文件操作经过 canonicalize + scope check，命令执行的 `cwd` 被限制在沙箱工作目录内；不支持的强隔离能力（CPU/内存/进程数/网络硬隔离）通过能力报告和 `UnsupportedFeature` 风格错误显式暴露。

### Rationale
- Rust 标准库和 tokio 可稳定实现进程执行、超时、输出限制、临时目录清理和路径逃逸防护。
- CPU/内存/network namespace 等能力强依赖平台和容器运行时；若 MVP 静默宣称支持会违反宪法第五条。
- 本地沙箱先提供 L2 核心行为兼容，后续可用相同 trait 接入更强隔离后端。

### Alternatives Considered
1. **把本地沙箱称为完全安全隔离**: 拒绝 — 这是伪安全声明。
2. **要求 Docker/容器作为唯一后端**: 拒绝 — 宿主环境不一定可用，且会扩大实现范围。
3. **本地受控执行 + 显式能力报告**: 采纳 — 保持可交付和诚实能力边界。

## 3. 命令执行与超时

### Decision
使用 `tokio::process::Command` 执行 argv 形式命令，配合 `tokio::time::timeout`、显式 child kill 与 wait；非零退出码作为 `ExecutionStatus::Exited { code }` 返回，不当作系统错误。

### Rationale
- argv 形式避免默认 shell 注入风险；需要 shell 时由上层显式传入 `sh -c`。
- 超时必须终止子进程并返回诊断信息，不能只返回 future timeout 后遗留进程。
- 非零退出是命令自身的可观察结果，应与 spawn 失败、权限错误、沙箱不可用区分。

### Alternatives Considered
1. **直接复用 `LocalBackend::exec_shell`**: 拒绝 — 当前实现忽略 timeout，且没有沙箱生命周期/审计信息。
2. **默认 shell 字符串执行**: 拒绝 — 契约不够明确，安全边界弱。
3. **argv + timeout + kill/wait**: 采纳 — 可测试、可诊断。

## 4. 输出大小限制

### Decision
执行结果内联返回 stdout/stderr 摘要，分别受 `max_stdout_bytes`、`max_stderr_bytes` 或统一 `max_output_bytes` 控制；完整输出写入沙箱内审计输出文件，并通过 `OutputRef` 引用。

### Rationale
- 防止大输出在内存和上下文中无限增长。
- 审计记录仍需可复现完整输出，不能简单丢弃。
- 与 Workspace offload 的“摘要 + 引用”模式一致。

### Alternatives Considered
1. **完整输出全部内联**: 拒绝 — 大输出会导致内存和 token 风险。
2. **超限即报错并丢弃输出**: 拒绝 — 诊断信息不足。
3. **内联摘要 + 完整输出引用**: 采纳。

## 5. 路径与挂载安全

### Decision
所有路径访问先解析为沙箱内路径，再通过 canonicalize（存在路径）或 canonicalize parent + final component（待创建路径）进行边界校验；挂载以 `SandboxMount { host_path, sandbox_path, access, persist }` 表示，只读写入返回 `PermissionDenied`。

### Rationale
- 仅字符串前缀无法防止 `..`、符号链接和路径别名逃逸。
- 待创建文件不能直接 canonicalize，需要校验已存在父目录。
- 只读/可写挂载是 Workspace 集成和安全策略的核心可观察语义。

### Alternatives Considered
1. **只用 `Path::normalize` 字符串处理**: 拒绝 — 不解析符号链接。
2. **引入 `cap-std`**: 暂不采纳 — 可作为后续强化，MVP 用标准库即可覆盖当前需求。
3. **canonicalize + scope/mount policy check**: 采纳。

## 6. Workspace 集成

### Decision
提供 `SandboxWorkspaceBackend` 实现 `agent_scope_workspace::WorkspaceBackend`；它持有 `Arc<dyn SandboxSession>` 或会话句柄，将 `exec_shell/read_file/write_file/list_dir/delete_path/file_exists` 映射到沙箱会话操作。

### Rationale
- 不改变 Workspace 工具的外部结构化结果，符合 FR-015。
- Sandbox 与 Workspace 的生命周期可以通过 backend close/reset 协调。
- 未来 Agent/Tool 只需选择 Workspace 后端，无需知道具体沙箱实现。

### Alternatives Considered
1. **新增一套 Sandbox 专用工具**: 拒绝 — 会造成 Bash/Read/Write 行为分裂。
2. **修改所有 Workspace 工具签名**: 拒绝 — 破坏已有 Feature 012 契约。
3. **WorkspaceBackend 适配器**: 采纳。

## 7. 审计与能力报告

### Decision
每个会话维护按 sequence 递增的 `ExecutionRecord`，记录命令摘要、状态、耗时、错误类别、输出引用和资源限制命中情况；`CapabilityReport` 声明当前后端支持/不支持能力和目标兼容等级。

### Rationale
- 宪法第七条要求 trace 是核心验收产物，沙箱执行历史属于可观察副作用。
- 能力报告避免用户误以为本地 MVP 具备容器级隔离。
- 执行历史为兼容性矩阵、调试和回归测试提供稳定数据。

### Alternatives Considered
1. **只返回最近一次执行结果**: 拒绝 — 无法审计和复现。
2. **把审计放到 tracing 日志**: 拒绝 — 日志不是稳定 API。
3. **结构化 ExecutionRecord + CapabilityReport**: 采纳。

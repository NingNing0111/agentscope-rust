# Implementation Plan: Sandbox Feature（代码执行沙箱）

**Branch**: `017-sandbox-feature` | **Date**: 2026-08-01 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/017-sandbox-feature/spec.md`

## Summary

实现 AgentScope Rust 的 Sandbox Feature：提供独立沙箱会话、受控命令执行、文件系统隔离、资源/网络策略声明、执行历史审计，并通过 `SandboxWorkspaceBackend` 接入现有 Workspace 抽象，使 Bash/Read/Write/Edit/Grep 等工作空间工具共享同一个沙箱边界。

技术方案采用独立 `agent_scope_sandbox` crate 定义核心沙箱类型与 trait，MVP 提供单机本地受控执行后端：每个会话拥有独立临时根目录，所有路径访问经过 canonicalize + scope/mount policy 校验，命令执行通过 `tokio::process::Command` + 显式 timeout/kill/wait 管理。不支持的强隔离能力（如 CPU/内存/网络硬隔离）必须通过能力报告和稳定错误显式暴露，不允许静默降级为普通本地执行。

## Technical Context

**Language/Version**: Rust 2024 edition（workspace `Cargo.toml` 使用 `edition = "2024"`）

**Primary Dependencies**:
- `tokio` — async runtime、process、time、fs、sync primitives
- `serde` / `serde_json` — 沙箱策略、执行记录、能力报告序列化
- `uuid` — `session_id` / `execution_id` 生成
- `chrono` — `created_at`、`started_at`、`finished_at` 等时间戳
- `sha2` — 完整输出引用的 SHA-256 摘要
- `tempfile`（dev/test 或生产可选）— 临时沙箱根目录与测试 fixture
- `agent_scope_workspace` — `WorkspaceBackend` trait 与 Workspace 集成
- `agent_scope_tool` / `agent_scope_agent` — 后续 Tool/Agent 端到端集成验证参考

**Storage**: 本地临时目录（每个 session 独立 root/workdir）+ 沙箱内审计输出文件 + 内存执行历史；后续可扩展为 Docker/OpenSandbox/E2B 持久后端

**Testing**: `cargo test`（unit + integration tests），`cargo check --workspace`，`cargo clippy --workspace --all-targets -- -D warnings`，`cargo fmt --check`

**Target Platform**: Linux/macOS 优先；Windows 路径语义通过 `std::path` 尽量兼容，但命令与网络/资源限制能力需在 `CapabilityReport` 中逐项声明

**Project Type**: Rust library workspace；新增 `crates/agent_scope_sandbox`，并在 `agent_scope_workspace` 中增加可选/适配集成

**Performance Goals**:
- 创建并初始化本地沙箱会话 < 100ms
- 执行简单命令（不含命令自身耗时）框架开销 < 50ms
- 输出摘要内存占用不超过配置的 `max_output_bytes` + bounded overhead
- 20 个并发会话基础命令/文件操作无可观察状态泄漏

**Constraints**:
- `#![deny(unsafe_code)]`；不得新增 unsafe
- 不允许伪兼容：不可用隔离能力必须返回 `UnsupportedFeature` / `SandboxUnavailable` 或出现在 `CapabilityReport.unsupported`
- 所有后台子进程必须有 owner、timeout/cancellation、kill/wait 和错误传播路径
- 所有路径访问必须防止 `..`、符号链接和路径别名逃逸
- 日志、审计记录和错误信息不得泄露敏感环境变量明文

**Scale/Scope**: 1 个新增 crate（`agent_scope_sandbox`）+ Workspace 适配；预计 1500–2500 LOC 生产代码 + 1000–1800 LOC 测试；覆盖 4 个 user stories、25 个 FR、8 个 SC

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Design Check (Phase 0)

| # | 宪法条款 | 评估 | 备注 |
|---|----------|------|------|
| 1 | 兼容性优先 | ✅ PASS | 以 Python AgentScope sandbox/workspace 可观察行为为目标，无法兼容项登记为偏差 |
| 2 | 锁定上游版本 | ✅ PASS | 继承项目既有兼容基线；本 feature 不变更上游版本 |
| 3 | Python 行为基准 | ✅ PASS | contracts/ 与 quickstart 要求行为/trace/历史可比较 |
| 4 | 先定义契约 | ✅ PASS | spec.md 已定义 US/FR/SC；本 plan 生成 contracts |
| 5 | 不允许伪兼容 | ✅ PASS | 明确禁止静默降级到非隔离本地执行 |
| 6 | 测试驱动兼容性 | ✅ PASS | quickstart 覆盖 timeout、路径逃逸、输出限制、并发隔离 |
| 7 | Trace 是核心验收产物 | ✅ PASS | `ExecutionRecord` 与 output refs 是核心审计产物 |
| 8 | Rust 原生设计 | ✅ PASS | trait + struct/enum + `Result<T,E>` + `Arc<dyn T>` |
| 9 | 安全 Rust 优先 | ✅ PASS | 无 unsafe；错误返回替代 panic/unwrap |
| 10 | 结构化并发 | ✅ PASS | 子进程必须有 owner、timeout、kill/wait、cleanup |
| 11 | 分层与依赖方向 | ✅ PASS | 新增 sandbox crate；通过 trait 接入 workspace，不污染 core/provider |
| 12 | 稳定数据协议 | ✅ PASS | policy/result/history/report 均设计为可序列化稳定结构 |
| 13 | 稳定错误模型 | ✅ PASS | `SandboxError` 类型化，含稳定类别 |
| 14 | 可观测性 | ✅ PASS | 执行历史 + tracing，不记录敏感 env 明文 |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 输出限制和 cleanup 不改变事件/错误语义 |
| 16 | 小步交付 | ✅ PASS | 聚焦 Sandbox，不包含 Multi-agent/Distributed runtime |
| 17 | 完成的定义 | ✅ PASS | quickstart 定义 test/check/clippy/fmt gate |
| 18 | 兼容性分级 | ✅ PASS | 目标至少 L2；L3/L4 差异登记 |
| 19 | 变更治理 | ✅ PASS | 当前设计无宪法违反 |

**Gate Result**: ✅ ALL PASS — 无违反，可进入 Phase 0

### Post-Design Check (Phase 1)

| 条款 | 设计决策 | 状态 |
|------|----------|------|
| §I/§III | `SandboxSession`、`ExecutionResult`、`ExecutionRecord` 捕获可观察执行行为 | ✅ |
| §V | 本地 MVP 不声称支持无法强制的 CPU/内存/网络硬隔离，统一通过 `CapabilityReport` / `UnsupportedFeature` 暴露 | ✅ |
| §VII | `ExecutionRecord.sequence`、状态、耗时、输出引用构成稳定 trace | ✅ |
| §VIII | `SandboxSession` trait、`SandboxPolicy`/`ExecutionStatus` enum、`SandboxWorkspaceBackend` adapter | ✅ |
| §IX | 计划新增 crate 使用 `#![deny(unsafe_code)]` 与 typed errors | ✅ |
| §X | 命令执行必须 timeout + kill/wait；close/reset 处理运行中命令 | ✅ |
| §XI | Sandbox 独立 crate，Workspace 通过 adapter 依赖，不引入 core/provider 反向依赖 | ✅ |
| §XII/§XIII | data-model.md 定义稳定序列化实体与错误类别 | ✅ |
| §XIV | quickstart.md 定义路径逃逸、输出限制、并发隔离、workspace regression 验证 | ✅ |
| §XVI | 不实现 Multi-agent 与 Distributed runtime | ✅ |
| §XVIII | 目标 L2；不能达到 L3/L4 的平台偏差写入能力报告/兼容性矩阵 | ✅ |

**Post-Design Gate Result**: ✅ ALL PASS — 设计无违反宪法

## Project Structure

### Documentation (this feature)

```text
specs/017-sandbox-feature/
├── plan.md                         # This file
├── research.md                     # Phase 0 output
├── data-model.md                   # Phase 1 output
├── quickstart.md                   # Phase 1 output
├── contracts/                      # Phase 1 output
│   ├── sandbox-session.md          # SandboxSession public API contract
│   └── sandbox-workspace-backend.md # WorkspaceBackend adapter contract
└── tasks.md                        # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_sandbox/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # crate attrs, module declarations, re-exports
│   ├── error.rs                # SandboxError typed error model
│   ├── policy.rs               # SandboxPolicy, NetworkPolicy, resource limits
│   ├── mount.rs                # SandboxMount, MountAccess, MountOwner
│   ├── session.rs              # SandboxSession trait + lifecycle types
│   ├── execution.rs            # ExecutionRequest/Result/Status/Record/OutputRef
│   ├── capability.rs           # CapabilityReport and SandboxCapability
│   ├── local.rs                # LocalSandboxSession MVP implementation
│   ├── path.rs                 # path canonicalization and scope checks
│   └── workspace_backend.rs    # SandboxWorkspaceBackend adapter
└── tests/
    ├── session_tests.rs        # US1 lifecycle + command execution
    ├── file_isolation_tests.rs # path traversal, symlink, mount policy
    ├── policy_tests.rs         # timeout/output/network/resource unsupported
    ├── audit_tests.rs          # execution history and output refs
    └── concurrency_tests.rs    # multi-session isolation

crates/agent_scope_workspace/
├── Cargo.toml                  # optional/path dependency or dev dependency on sandbox as needed
└── tests/
    └── sandbox_backend_tests.rs # WorkspaceBackend adapter integration tests

specs/001-compatibility-baseline/
└── capability-matrix.json      # register Sandbox module level/deviations during implementation
```

**Structure Decision**: 新增独立 `agent_scope_sandbox` crate，并以 `SandboxWorkspaceBackend` 适配现有 Workspace。这样既保持 Sandbox 作为第 13 步路线图能力的独立性，也避免破坏 Feature 012 的 Workspace 契约。

## Complexity Tracking

> 无违反宪法的情况，不需要填写此表。

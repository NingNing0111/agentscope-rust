# Implementation Plan: Microsandbox Sandbox Backend（基于 microsandbox 的强隔离沙箱后端）

**Branch**: `035-microsandbox-sandbox-backend` | **Date**: 2026-08-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/035-microsandbox-sandbox-backend/spec.md`

## Summary

新增基于 `microsandbox` Rust SDK 的 microVM 强隔离沙箱后端，作为现有 `agent_scope_sandbox` crate 的 feature-gated backend。当前 Feature 017 已提供 `LocalSandboxSession`，但它是 local-process reference backend，仅提供临时目录、路径防护、timeout、输出截断和审计历史，并明确不支持真实 network/cpu/memory/process/hard filesystem isolation。本 feature 在不破坏 local backend 的前提下新增 `MicrosandboxSession`，实现现有 `SandboxSession` trait，并通过泛化 `SandboxWorkspaceBackend` 让 Workspace built-in tools 和 Agent 可以选择 microsandbox microVM 边界。

关键约束：不可用能力必须显式失败或进入能力报告，不允许静默降级到 local-process；sandbox 输出一律视为不可信数据；默认最小权限，网络默认禁用或受限；不暴露宿主凭据，不在命令、日志、测试或文档中写入真实 secret；Cloud backend 必须由用户显式选择，不能从 `MSB_API_KEY` 或 `MSB_API_URL` 推断。

## Technical Context

**Language/Version**: Rust 2024 edition（workspace `Cargo.toml` 使用 `edition = "2024"`）

**Primary Dependencies**:
- `microsandbox = "0.6.10"` — Rust SDK，用于创建/执行/管理 microVM sandbox（feature-gated）
- `tokio` — async runtime、fs、sync、time
- `async-trait` — `SandboxSession` trait async methods
- `serde` / `serde_json` — policy、result、history、capability report 序列化
- `chrono` — created/started/finished/closed timestamps
- `sha2` — full output refs SHA-256
- `uuid` — session/execution id
- `agent_scope_workspace` — `WorkspaceBackend` trait 与工具集成

**Storage**: microsandbox guest filesystem + sandbox 内/host-side 可审计输出引用；本 feature 默认不引入持久数据库。`keep_on_close` / persistent sandbox 行为必须显式配置。

**Testing**:
- 默认 CI：`rtk cargo test --workspace`，不依赖真实 microsandbox runtime
- feature compile：`rtk cargo check -p agent_scope_sandbox --features microsandbox`
- ignored real runtime：`rtk cargo test -p agent_scope_sandbox --features microsandbox --test microsandbox_tests -- --ignored`
- 全量 gate：fmt/check/test/clippy

**Target Platform**: Linux with KVM 或 macOS Apple Silicon 优先；runtime/platform 不可用必须稳定错误，不回退。

**Project Type**: Rust library workspace；主改 `crates/agent_scope_sandbox`，尽量不改 tool/agent public API。

**Performance Goals**:
- 默认 CI 不启动 microVM，避免引入平台依赖
- microsandbox session create/exec 性能不作为 local backend 对等目标；正确隔离和显式错误优先
- 输出摘要内存不超过 `max_output_bytes` + bounded overhead

**Constraints**:
- `#![deny(unsafe_code)]`，不得新增 unsafe
- microsandbox dependency 必须 feature-gated，默认构建不要求 runtime
- 不允许 local-process silent fallback
- sandbox 输出是不可信数据，不能当作指令
- 不自动安装或管理 `msb` runtime
- 不自动使用 Cloud；Cloud 仅由显式配置选择
- 不写入真实 API key/token/password
- 对 SDK 无法稳定等价的能力返回 `UnsupportedFeature`

**Scale/Scope**: 1 个新模块（`microsandbox.rs`）+ sandbox workspace adapter 泛化 + policy/capability/error mapping + tests/docs/examples；预计 800–1600 LOC production code + 800–1400 LOC tests/docs。

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| # | 宪法条款 | 评估 | 备注 |
|---|----------|------|------|
| 1 | 兼容性优先 | ✅ PASS | 复用现有 `SandboxSession` / `ExecutionResult` / `WorkspaceBackend` 契约 |
| 2 | 锁定上游版本 | ✅ PASS | `microsandbox` 锁定 `0.6.10`，写入 workspace dependency |
| 3 | Python 行为基准 | ✅ PASS | 以可观察 sandbox/workspace 行为兼容为目标，不伪装 Python 未覆盖能力 |
| 4 | 先定义契约 | ✅ PASS | 本 feature 先产出 spec/plan/contracts/tasks |
| 5 | 不允许伪兼容 | ✅ PASS | 明确禁止 runtime 不可用或 policy unsupported 时 silent fallback |
| 6 | 测试驱动兼容性 | ✅ PASS | 默认 deterministic tests + ignored real runtime tests |
| 7 | Trace 是核心验收产物 | ✅ PASS | `ExecutionRecord` 和 `OutputRef` 保留 |
| 8 | Rust 原生设计 | ✅ PASS | struct/enum/trait object/typed Result；SDK 类型封装在后端内 |
| 9 | 安全 Rust 优先 | ✅ PASS | 无 unsafe，错误走 `SandboxError` |
| 10 | 结构化并发 | ✅ PASS | 不新增未绑定 background task；exec timeout/cleanup 明确 |
| 11 | 分层与依赖方向 | ✅ PASS | microsandbox 依赖只进入 sandbox crate，不污染 tool/agent/core |
| 12 | 稳定数据协议 | ✅ PASS | 公共 execution/policy/capability 数据模型复用，必要扩展可序列化 |
| 13 | 稳定错误模型 | ✅ PASS | SDK/runtime/platform 错误映射到稳定 `SandboxError` |
| 14 | 可观测性 | ✅ PASS | history/capability/tracing，不泄露 secret |
| 15 | 性能不能牺牲正确性 | ✅ PASS | 强隔离和显式失败优先，不为性能绕过策略 |
| 16 | 小步交付 | ✅ PASS | 聚焦 microsandbox backend，不含 cloud/secret/snapshot/interactive TTY |
| 17 | 完成的定义 | ✅ PASS | quickstart/tasks 定义 fmt/check/test/clippy 和 real runtime gate |
| 18 | 兼容性分级 | ✅ PASS | local 维持既有 L2；microsandbox 能力单独报告 |
| 19 | 变更治理 | ✅ PASS | 新 feature 独立规格与已知偏差记录 |

**Gate Result**: ✅ ALL PASS — 可进入 Phase 0/1 设计与实现。

## Project Structure

### Documentation (this feature)

```text
specs/035-microsandbox-sandbox-backend/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── microsandbox-session.md
│   ├── sandbox-backend-selection.md
│   └── workspace-backend.md
└── tasks.md
```

### Source Code (repository root)

```text
Cargo.toml                                      # workspace dependency microsandbox = "0.6.10"

crates/agent_scope_sandbox/
├── Cargo.toml                                  # optional microsandbox dependency + feature flag
├── src/
│   ├── lib.rs                                  # feature-gated module/re-export
│   ├── microsandbox.rs                         # new MicrosandboxConfig / MicrosandboxSession
│   ├── capability.rs                           # CapabilityReport::microsandbox()
│   ├── error.rs                                # stable SDK/runtime/platform error mapping if needed
│   ├── policy.rs                               # microsandbox-specific validation/mapping helpers
│   └── workspace_backend.rs                    # Arc<Mutex<Box<dyn SandboxSession>>> adapter
└── tests/
    ├── microsandbox_config_tests.rs            # default CI, no runtime
    ├── microsandbox_policy_tests.rs            # mapping/unsupported tests
    ├── microsandbox_workspace_backend_tests.rs # local regression + trait object adapter
    └── microsandbox_tests.rs                   # feature-gated ignored real runtime tests

examples/sandbox/                               # optional microsandbox example branch
```

**Structure Decision**: 不新增独立 crate。`agent_scope_sandbox` 已是 sandbox 抽象边界，microsandbox 是它的一个后端实现。Tool/Agent 通过 `WorkspaceBackend` 解耦，优先泛化 `SandboxWorkspaceBackend` 而不是修改每个工具。

## Complexity Tracking

> 无违反宪法的情况，不需要填写例外。

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| 无 | — | — |

## Phase 0 Research

详见 [research.md](research.md)。核心结论：

1. SDK 入口使用 `microsandbox::Sandbox::builder(name)`，image/memory/cpus/env/volume/network/replace/create 按实际编译 API 校准。
2. `NetworkPolicy::Disabled` 可映射到 no-net 或 SDK 等价网络禁用；host allowlist 若不能精确映射必须 unsupported。
3. `memory_limit_bytes` 可按 MiB 映射；`cpu_shares` 不等于 vCPU 数，默认不伪映射；`process_limit` 默认 unsupported。
4. Default CI 不运行真实 microVM；真实 tests `#[ignore]`。
5. Cloud/secret/snapshot/port publishing 不纳入 MVP，避免扩大安全边界。

## Phase 1 Design

详见 [data-model.md](data-model.md) 与 [contracts](contracts/)。关键 API：

```rust
#[derive(Debug, Clone)]
pub struct MicrosandboxConfig {
    pub session_id: Option<String>,
    pub image: String,
    pub workdir: String,
    pub policy: SandboxPolicy,
    pub mounts: Vec<SandboxMount>,
    pub env: std::collections::HashMap<String, String>,
    pub replace_existing: bool,
    pub persist: bool,
    pub startup_timeout: std::time::Duration,
    pub stop_timeout: std::time::Duration,
}
```

```rust
pub struct MicrosandboxSession { /* SDK handle hidden */ }

#[async_trait::async_trait]
impl SandboxSession for MicrosandboxSession { /* ... */ }
```

```rust
pub struct SandboxWorkspaceBackend {
    session: Arc<Mutex<Box<dyn SandboxSession>>>,
    instructions: String,
}

impl SandboxWorkspaceBackend {
    pub fn new(session: LocalSandboxSession) -> Self;
    pub fn from_session<S>(session: S) -> Self
    where
        S: SandboxSession + 'static;
    pub fn from_boxed_session(session: Box<dyn SandboxSession>) -> Self;
}
```

## Implementation Strategy

1. 先提交 spec-kit 工件和 `.specify/feature.json` 指向 Feature 035。
2. 添加 `microsandbox` workspace dependency 与 `agent_scope_sandbox` feature flag，但先不启用默认 feature。
3. 编写默认 CI 可跑的 config/policy/capability/error mapping tests。
4. 泛化 `SandboxWorkspaceBackend`，保持 local backend tests 通过。
5. 实现 `MicrosandboxConfig` 与 `MicrosandboxSession` 生命周期/exec/history。
6. 实现 fs API、mount/network/resource policy mapping。
7. 增加 ignored real runtime tests、examples/docs。
8. 运行默认全量 gate；runtime 可用时补跑 real gate。

## Risk & Trade-offs

- **runtime 可用性**: microVM 依赖平台能力，默认 CI 不能要求 runtime。
- **SDK API 演进**: 用后端模块封装 SDK 类型，减少 public API 受影响范围。
- **资源语义不等价**: `cpu_shares` 与 vCPU 数不同，不伪映射；需要显式 unsupported 或新增语义明确字段。
- **网络安全**: allowlist 不能精确实现时宁可拒绝，不放宽为 unrestricted。
- **secret 安全**: MVP 不提供真实 secret injection；后续必须单独设计。
- **workspace 泛化回归**: 必须保留 `SandboxWorkspaceBackend::new(LocalSandboxSession)`。

## Completion Definition

- Feature 035 spec/plan/research/data-model/contracts/quickstart/tasks 完整落地。
- `LocalSandboxSession` 行为和 capability report 不回归。
- `MicrosandboxSession` feature-gated 实现 `SandboxSession`。
- `SandboxWorkspaceBackend` 可持有任意 sandbox session。
- 默认 tests 不依赖 runtime；真实 runtime tests 有明确 ignored 运行方式。
- 所有 unsupported/unavailable 能力稳定、显式报告。
- `rtk cargo fmt --check`、`rtk cargo check --workspace --all-targets`、`rtk cargo test --workspace`、`rtk cargo clippy --workspace --all-targets -- -D warnings` 通过。

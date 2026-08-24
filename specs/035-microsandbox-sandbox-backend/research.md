# Research: Microsandbox Sandbox Backend（基于 microsandbox 的强隔离沙箱后端）

**Feature**: `035-microsandbox-sandbox-backend`
**Date**: 2026-08-24
**Spec**: [spec.md](spec.md)
**Plan**: [plan.md](plan.md)

## Research Goals

本研究文档为 Feature 035 的 Phase 0 输出，目标是在实现前明确 microsandbox 后端的 SDK/CLI 能力边界、与现有 `agent_scope_sandbox` 抽象的映射方式，以及不可伪兼容的安全约束。

本 feature 的核心原则：

- `microsandbox` 是新的强隔离 backend，不是新的一套 sandbox API。
- `LocalSandboxSession` 继续作为 local-process reference backend，不声称强隔离。
- `MicrosandboxSession` 必须实现现有 `SandboxSession` trait。
- runtime/platform/SDK 不可用必须稳定失败，不得 silent fallback 到 local backend。
- sandbox 输出、日志和文件一律是不可信数据。
- 默认最小权限；网络、mount、secret、Cloud 都必须显式配置。

## Decision 1: 后端放在 `agent_scope_sandbox` crate 内

**Decision**: 在 `crates/agent_scope_sandbox` 内新增 feature-gated `microsandbox` 模块，而不是新增平行 crate 或平行 API。

**Rationale**:

- 现有 Feature 017 已定义 `SandboxSession`、`SandboxPolicy`、`ExecutionRequest`、`ExecutionResult`、`CapabilityReport`、`SandboxError` 等公共模型。
- microsandbox 是 sandbox 后端实现，不应让调用方学习另一套生命周期、执行结果或错误模型。
- Workspace/Tool/Agent 已经通过 `WorkspaceBackend` 与 sandbox adapter 解耦，最小改动路径是泛化 `SandboxWorkspaceBackend`。

**Alternatives considered**:

1. 新增 `agent_scope_microsandbox` crate：会引入额外 crate 边界，但当前没有明显复用收益，且 public API 容易分裂。
2. 直接改 `LocalSandboxSession`：会混淆 local-process 与 microVM 隔离等级，违反“不伪兼容”。

**Result**: 采用 `crates/agent_scope_sandbox/src/microsandbox.rs`，通过 Cargo feature `microsandbox` 暴露。

## Decision 2: 依赖和 feature gate

**Decision**: 根 workspace 添加 `microsandbox = "0.6.10"`；`agent_scope_sandbox` 以 optional dependency 引入，并由 `microsandbox` feature 启用。

推荐 manifest：

```toml
# Cargo.toml
[workspace.dependencies]
microsandbox = "0.6.10"
```

```toml
# crates/agent_scope_sandbox/Cargo.toml
[features]
default = []
microsandbox = ["dep:microsandbox"]

[dependencies]
microsandbox = { workspace = true, optional = true }
```

**Rationale**:

- 默认 CI 不应要求真实 runtime 或 SDK backend 可用。
- 未启用 feature 时，不应编译 microsandbox SDK 依赖。
- 上游版本锁定符合项目依赖治理要求。

**Risk**:

- 若 `microsandbox` 0.6.10 实际 API 与参考文档不一致，实现阶段必须以编译结果校准，不能把未经验证的 API 直接暴露到 public API。

## Decision 3: SDK 入口与生命周期映射

**Decision**: `MicrosandboxSession::initialize()` 使用 microsandbox Rust SDK 创建 persistent 或 named sandbox；SDK handle 隐藏在后端内部。

预期 SDK/CLI 能力参考：

- CLI 支持 `msb run`、`msb create`、`msb exec`、`msb stop`、`msb remove`。
- Rust SDK 计划通过 builder 创建 sandbox，设置 image、name、workdir、memory、cpu、env、volume、network、replace 等选项。
- 实际 API 名称在实现时以 `microsandbox = "0.6.10"` 编译结果为准。

**Lifecycle mapping**:

| `SandboxSession` | microsandbox 行为 |
|------------------|------------------|
| `Created` | Rust 对象已构造，microVM 未创建或未启动 |
| `initialize()` | create/start sandbox，成功后 `Ready` |
| `execute()` | 在已 Ready sandbox 内 exec 命令 |
| `close()` | stop sandbox；按 persist/keep 配置决定是否 remove |
| `cleanup()` | 幂等 remove 临时资源；失败返回稳定错误 |
| `Failed` | 初始化或系统级操作失败后的稳定状态 |

**Rationale**:

- 保持 trait 生命周期不变。
- SDK handle 不暴露，可隔离上游 API 演进。
- close/cleanup 幂等，方便上层错误恢复。

## Decision 4: runtime/platform 不可用错误

**Decision**: runtime 未安装、平台不支持、KVM/Apple Silicon 不可用、SDK create 失败、image pull 失败时，返回稳定 `SandboxError` 类别；不得创建 `LocalSandboxSession` 兜底。

若现有 `SandboxError` 无专用变体，可优先映射到已有稳定类型；不足时新增类似：

```rust
SandboxError::SandboxUnavailable { reason: String }
```

或：

```rust
SandboxError::BackendError { backend: "microsandbox", message: String }
```

**Rationale**:

- Feature 017 已明确禁止 silent fallback。
- 强隔离不可用时继续本地执行会造成安全误导。
- 错误消息必须可诊断但不得泄露 secret。

**Tests**:

- 单元测试覆盖 SDK/runtime/platform 错误 mapping helper。
- ignored integration test 可在 runtime 不可用环境验证稳定错误类别。

## Decision 5: NetworkPolicy 映射

**Decision**: 网络策略安全优先，能精确映射才启用；不能精确映射则 `UnsupportedFeature`。

| `NetworkPolicy` | microsandbox mapping | 处理 |
|-----------------|----------------------|------|
| `Disabled` | `--no-net` 或 SDK 等价 no-net | MUST 支持或初始化失败 |
| `LoopbackOnly` | loopback allow / deny external | 若 SDK 无精确 loopback-only，则 unsupported |
| `Allowlist { hosts }` | `--net-default deny` + host allow rules | 若 SDK 无精确 host allowlist，则 unsupported |
| `Unrestricted` | default/public/unrestricted egress | 仅显式配置时允许 |

**Rationale**:

- allowlist 不能放宽为 unrestricted。
- 不可信代码默认 no-net 或严格 allowlist。
- Cloud/API key 不得隐式改变 backend 或网络策略。

**Tests**:

- `Disabled` 生成 no-net config。
- host allowlist unsupported 情况稳定报错。
- `Unrestricted` 需要显式策略，不作为隐式 fallback。

## Decision 6: 资源限制映射

**Decision**: 仅映射语义稳定等价的资源限制。

| `SandboxPolicy` 字段 | microsandbox mapping | 处理 |
|----------------------|----------------------|------|
| `default_timeout` / `max_timeout` | Rust 层执行 timeout + SDK max-duration（如可用） | MUST 保留 |
| `max_output_bytes` | Rust 层 inline output 截断 | MUST 保留 |
| `memory_limit_bytes` | SDK memory MiB/GiB | SHOULD 支持，需单位和边界测试 |
| `cpu_limit.cpu_shares` | vCPU 数不等价 | 默认 unsupported，不伪映射 |
| `process_limit` | 若 SDK 无直接限制 | unsupported |

**Rationale**:

- `cpu_shares` 是相对权重，不等于 vCPU 数；把它当 `--cpus` 会伪兼容。
- `process_limit` 若不能由 microVM/runtime 精确强制，就不能声明支持。
- timeout/output limit 可在 Rust 层稳定实现。

## Decision 7: Mount 和路径授权

**Decision**: microVM 隔离之外仍保留 Rust 层路径规范化与授权检查。mount 必须显式声明，宿主敏感路径默认拒绝或要求调用方显式承担。

Mapping rules:

- `SandboxMount` 映射到 microsandbox volume/bind mount。
- 只读 mount 必须使用 SDK/CLI 的只读 mount 语义；若 SDK 无法强制只读，则该 mount 配置 unsupported。
- 写操作仅允许 sandbox workdir 或 writable mount。
- `..`、宿主绝对路径、越界路径必须在 Rust 层拒绝。
- 不自动挂载 `~/.ssh`、`~/.aws`、`~/.config`、token/credential 目录。

**Rationale**:

- microVM 是 containment boundary，但 host mount 是主动扩大边界，必须最小权限。
- Rust 层防护可提供稳定错误和兼容的路径语义。

## Decision 8: 文件 API 实现方式

**Decision**: `read_file/write_file/delete_path/is_dir/stat_mtime/list_dir` 通过 microsandbox SDK fs API、`msb copy` 等等价机制或 guest exec 实现，但 public 行为必须保持 `SandboxSession` contract。

Priority:

1. SDK 原生 fs API（若可用）。
2. SDK copy/exec helper。
3. 受控 guest command fallback（例如 `stat`/`find`/`rm`），仅当 image 基础工具可依赖或明确记录限制。

**Rationale**:

- trait 要求完整文件 API。
- 若基础 image 不保证 `stat/find`，则不应把 command fallback 当无条件能力。

**Known risk**:

- 不同 image 的 shell/coreutils 可用性不同。若 SDK 无原生 fs API，quickstart 应选择具备基本工具的 image，并在 capability report 登记偏差。

## Decision 9: 输出摘要和审计历史

**Decision**: `MicrosandboxSession` 复用 local backend 的输出语义：stdout/stderr inline 截断、完整输出引用、sha256、bytes、history。

Output handling:

- `inline` 长度不超过 `policy.max_output_bytes`。
- `truncated = true` 时保留 full output ref。
- full output ref 保存到可审计位置；路径不得暴露 secret。
- `ExecutionRecord` 使用 redacted command summary。
- 非零退出码是 `ExecutionStatus::Exited { code }`，不是 SDK/system error。

**Rationale**:

- trace 是项目核心验收产物。
- 调用方不应因 backend 切换而改变 result parsing。

## Decision 10: WorkspaceBackend 泛化

**Decision**: `SandboxWorkspaceBackend` 从持有 `LocalSandboxSession` 改为持有 `Box<dyn SandboxSession>`，并保留 local 兼容构造入口。

Recommended API:

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

**Rationale**:

- Bash/Read/Write/Edit/Grep/Glob/ResetTools 都经 `WorkspaceBackend`，无需逐个改工具。
- `Arc<Mutex<Box<dyn SandboxSession>>>` 与现有 async mutable trait 方法兼容。
- 保留 `new(LocalSandboxSession)` 避免破坏现有示例和测试。

## Decision 11: Cloud/backend selection

**Decision**: Cloud backend 不属于默认 MVP；即使环境中存在 `MSB_API_KEY` 或 `MSB_API_URL`，也不得推断 Cloud。Cloud 必须由显式配置选择。

**Rationale**:

- `MSB_API_KEY` 只配置 Cloud credential，不代表用户允许使用 Cloud。
- Cloud 执行是外部服务边界，必须由用户显式授权。
- 默认 local microVM 已满足本 feature 核心目标。

**Future work**:

- 单独 feature 设计 `MicrosandboxBackendKind::{Local, Cloud}`、profile、credential redaction、audit logs 和数据驻留约束。

## Decision 12: Secrets 不纳入 MVP

**Decision**: 本 feature 不提供真实 secret injection。`env` 仅用于非 secret 环境变量；真实凭据不得写入示例、测试、日志或文档。

**Rationale**:

- microsandbox 支持 scoped secret placeholder substitution，但安全边界复杂。
- 当前目标是 sandbox backend；secret injection 应单独设计 host allowlist、redaction 和审计。

**Documentation requirement**:

- quickstart 必须说明：不要通过 `env` 传 API key/token/password；如将来需要 secret，请等待单独能力。

## Decision 13: Testing strategy

**Decision**: 默认测试完全 deterministic，不依赖真实 microsandbox runtime；真实 runtime tests feature-gated + `#[ignore]`。

Default tests:

- config validation
- policy mapping
- capability report
- error mapping
- workspace adapter local regression
- trait object adapter contract

Ignored runtime tests:

- create/exec success
- nonzero exit mapping
- timeout
- file roundtrip
- output truncation/ref
- network disabled
- readonly mount rejection
- close/cleanup idempotency

**Rationale**:

- CI runner 可能没有 KVM/Apple Silicon/runtime/image cache。
- 编译级 feature check 能保证 SDK integration 至少能 build。
- 真实 runtime 验收仍可由维护者手动执行。

## Decision 14: Implementation order

**Decision**: 先完成 spec-kit 工件，再做可默认测试的基础变更，最后接真实 SDK。

Order:

1. 完整写入 spec/plan/research/data-model/contracts/quickstart/tasks。
2. 更新 `.specify/feature.json`。
3. 添加 Cargo dependency + feature gate。
4. 先写 config/policy/capability/error mapping tests。
5. 泛化 `SandboxWorkspaceBackend`，验证 local regression。
6. 实现 `MicrosandboxConfig`/`MicrosandboxSession` 生命周期与 exec。
7. 实现 fs/mount/network/resource mapping。
8. 添加 ignored runtime tests、docs、examples。
9. 运行 fmt/check/test/clippy gate。

## Open Questions for Implementation

1. `microsandbox = "0.6.10"` Rust SDK 的具体 builder/type/method 名称需以编译为准。
2. SDK 是否提供原生 fs API；若没有，文件操作 fallback 能否稳定跨 image。
3. SDK 的 network allowlist 和 loopback-only 语义是否能精确表达现有 `NetworkPolicy`。
4. SDK 是否支持 read-only bind mount 的稳定强制语义。
5. SDK exec 是否支持 stdin/cwd/env/timeout；不支持部分需要 Rust 层包装或 unsupported。
6. SDK 返回 exit code/stdout/stderr 的类型和错误分类需要在 error mapping 中固定。

## Final Research Conclusion

本 feature 可安全实现为 `agent_scope_sandbox` 的 feature-gated microsandbox backend。架构关键点不是逐个改工具，而是让 `MicrosandboxSession` 实现既有 `SandboxSession`，并泛化 `SandboxWorkspaceBackend`。所有不能精确映射的安全/资源/network/mount 语义必须显式 `UnsupportedFeature` 或稳定 unavailable error；不可将 local-process 当 fallback。默认 CI 只验证 deterministic mapping 与 local regression，真实 microVM 行为用 ignored integration tests 单独验收。

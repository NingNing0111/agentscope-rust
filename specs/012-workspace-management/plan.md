# Implementation Plan: Workspace Management

**Branch**: `012-workspace-management` | **Date**: 2026-07-31 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/012-workspace-management/spec.md`

## Summary

为 AgentScope Rust 实现 Workspace 模块，提供 Agent 隔离的本地工作区间管理。核心交付物：`WorkspaceBackend` trait（扩展的文件系统+进程 I/O 抽象）、`WorkspaceBase` trait（工作空间统一接口）、`LocalWorkspace`（本地文件系统实现）、Skill/MCP 资源管理、上下文 offload 机制，以及 `WorkspaceManager`（多租户管理）。

基于 Python AgentScope `workspace/` 模块（`_base.py` 749行 + `_local_workspace.py` 867行）反向工程，采用 Rust 原生设计（trait + enum + `Arc<dyn T>`）。

## Technical Context

**Language/Version**: Rust 2024 edition (workspace Cargo.toml `edition = "2024"`)

**Primary Dependencies**: 
- `agent_scope_message` — Msg, ContentBlock, DataBlock, Base64Source, URLSource, ToolResultBlock, TextBlock, Role
- `agent_scope_memory` — Backend trait (需扩展), MemoryError (参考)
- `agent_scope_tool` — Tool trait, ToolExecOutput, ToolError (参考)
- `tokio` — async runtime, file I/O, sync primitives
- `serde` / `serde_json` — 序列化（.mcp 持久化, SKILL.md frontmatter）
- `sha2` — SHA-256（skill 去重, data block offload）
- `uuid` — workspace_id 生成

**Storage**: 本地文件系统（`{workdir}/data/`, `{workdir}/skills/`, `{workdir}/sessions/`, `{workdir}/.mcp` JSON）

**Testing**: `cargo test` (单元测试 + 集成测试, 使用 tempfile)

**Target Platform**: Linux, macOS (Windows 支持通过 `std::path::Path` 自动平台差异处理)

**Project Type**: Library crate (`agent_scope_workspace`)

**Performance Goals**: 初始化 < 100ms, offload 100条消息+10个base64数据块 < 2s

**Constraints**: 
- `#![deny(unsafe_code)]` per Constitution §IX
- `#![deny(clippy::unwrap_used)]` per Constitution §IX
- 所有公开 API 使用 `#[allow(unused)]` 仅在实现阶段，发布前 MUST 移除

**Scale/Scope**: 单 crate, 估计 ~1500-2000 LOC 生产代码 + ~800 LOC 测试

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Design Check (Phase 0)

| 条款 | 要求 | 本 Feature 符合性 |
|------|------|-------------------|
| §I 兼容性优先 | 公开 API 兼容 Python AgentScope | ✅ LocalWorkspace 的目录结构、.mcp 格式、offload 输出与 Python 等价 |
| §II 锁定上游版本 | 绑定明确的 Python 版本 | ✅ 基于 agentscope-rust/agentscope/ 根目录下的 Python 参考代码 |
| §III Python 是行为基准 | 相同输入产生相同可观察结果 | ✅ SC-006 要求兼容性测试 |
| §IV 差分测试 | Rust vs Python 黄金快照 | ✅ contracts/ 定义测试契约 |
| §V 错误分类 | 机器可读错误码 | ✅ WorkspaceError enum 含分类变体 |
| §VI 可观测性 | tracing 支持 | ✅ 使用 `tracing` crate 记录关键操作 |
| §VII 跨平台 | Linux/macOS/Windows | ✅ `std::path::Path` 自动处理，但 `exec_shell` 仅支持 Unix（Windows 用 PowerShell） |
| §VIII Rust 原生设计 | trait enum Arc Result | ✅ WorkspaceBase trait, LocalWorkspace struct, Arc<dyn WorkspaceBackend> |
| §IX 安全 Rust | `#![deny(unsafe_code)]` | ✅ 无 unsafe 需求 |
| §X 结构化并发 | 任务有明确所有者 | ✅ LocalWorkspace 不启动后台任务 |
| §XI 零停机 | N/A（library crate） | N/A |
| §XII 向后兼容 | semver | ✅ 首个版本 0.1.0 |
| §XIII 错误即值 | Result<T,E> | ✅ 所有 fallible 操作返回 Result |
| §XIV 测试策略 | 单元+集成+兼容性 | ✅ 参考 test-infrastructure-patterns [[test-infrastructure-patterns]] |
| §XV 代码质量 | clippy + fmt | ✅ CI 检查 |
| §XVI 小步交付 | 独立可交付模块 | ✅ 本次仅 LocalWorkspace |
| §XVII 完成定义 | spec/plan/tasks/tests/docs | ✅ |
| §XVIII 兼容性分级 | L1-L4 等级 | 目标: L2 (核心行为兼容) |
| §XIX 变更治理 | 违反宪法需审批 | N/A — 无违反 |

**Gate Result**: ✅ ALL PASS — 无违反，可进入 Phase 0

### Post-Design Check (Phase 1)

| 条款 | 设计决策 | 状态 |
|------|---------|------|
| §I | `LocalWorkspace` 目录结构 / `.mcp` 格式 / offload 输出与 Python 等价 | ✅ |
| §II | 锁定 `agentscope/` 根目录下的 Python 参考代码 | ✅ |
| §III | contracts/ 定义了兼容性契约，quickstart.md 定义了验证场景 | ✅ |
| §IV | SC-006 要求兼容性测试，后续 tasks 中实现差分测试 | ✅ |
| §V | `WorkspaceError` enum 含 10 个分类变体，实现 Display+Error | ✅ |
| §VI | 关键操作使用 `tracing` 宏（info/warn/error） | ✅ |
| §VII | `std::path::Path` 自动处理路径分隔符；Windows 需 Bash→PowerShell 替换 | ✅ |
| §VIII | trait + enum + Arc + Result 原生设计，符合宪法第八条 trait object 模式 | ✅ |
| §IX | `#![deny(unsafe_code)]` + `#![deny(clippy::unwrap_used)]` | ✅ |
| §X | `LocalWorkspace` 不启动后台任务（WorkspaceManager 的清理任务可选） | ✅ |
| §XIII | 所有 fallible 操作返回 `Result<T, WorkspaceError>` | ✅ |
| §XIV | 按 test-infrastructure-patterns 组织 tests/ 目录 | ✅ |
| §XVI | 本次仅 LocalWorkspace，沙箱后端留待后续 | ✅ |
| §XVIII | 目标 L2（核心行为兼容） | ✅ |

**Post-Design Gate Result**: ✅ ALL PASS — 设计无违反宪法

## Project Structure

### Documentation (this feature)

```text
specs/012-workspace-management/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── workspace-backend.md
│   ├── workspace-base.md
│   └── local-workspace.md
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/agent_scope_workspace/
├── Cargo.toml
├── src/
│   ├── lib.rs               # 模块声明 + re-exports + deny 属性
│   ├── error.rs             # WorkspaceError enum
│   ├── backend.rs           # WorkspaceBackend trait + LocalBackend impl
│   ├── base.rs              # WorkspaceBase trait (async_trait)
│   ├── local_workspace.rs   # LocalWorkspace struct impl
│   ├── skill.rs             # Skill struct + SkillManager
│   ├── mcp_registry.rs      # MCP 注册表（持久化到 .mcp）
│   ├── offload.rs           # offload_context / offload_tool_result
│   ├── manager.rs           # WorkspaceManager（多租户）
│   └── instructions.rs      # get_instructions() 模板
└── tests/
    ├── local_workspace_tests.rs    # US1 测试
    ├── resource_tests.rs           # US2 测试 (MCP + Skill)
    ├── offload_tests.rs            # US3 测试
    ├── lifecycle_tests.rs          # US4 测试
    └── manager_tests.rs            # US5 测试
```

**Structure Decision**: 单 crate 结构 — workspace 模块是独立的能力单元，不依赖其他内部 crate 以外的外部服务。所有子模块放在 `src/` 下按职责拆分。

## Complexity Tracking

> 无违反宪法的情况，不需要填写此表。

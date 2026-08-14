# Implementation Plan: Agent Workspace Built-in Tools

**Branch**: `029-agent-workspace-tools` | **Date**: 2026-08-12 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/029-agent-workspace-tools/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

当 agent 被显式配置 workspace 访问后，系统 MUST 自动向该 agent 的 `ToolKit` 注入一组内置工具：`Bash`、`Read`、`Edit`、`Write`、`Grep`、`Glob`、`ResetTools`、`Skill`，以及环境支持时可选注入 `PowerShell`。这些工具以可执行的 `Tool` 实现形式存在于 `agent_scope_tool` crate（持有 workspace backend 句柄），由 agent 构造路径在显式 workspace 配置存在时合并入 `ToolKit`；未配置 workspace 的 agent 不暴露文件/命令工具。核心安全约束包括：workspace 边界强制（路径含 `..`/符号链接逃逸拒绝）、Read 后 Edit/Write 的读-改守卫、Edit `old_string` 非唯一拒绝、命令超时与输出截断、搜索结果有界、`ResetTools` 仅按授权范围切换工具组激活状态。设计以 vendored Python 参考实现（`agentscope/src/agentscope/tool/_builtin/`，上游 commit `9d1026fa`）的公开契约（工具名、参数 schema、描述、错误语义）为兼容基准。

## Technical Context

**Language/Version**: Rust (workspace edition, 2021), tokio async runtime

**Primary Dependencies**:
- `agent_scope_tool` — `Tool` trait, `ToolKit` registry, `FunctionTool` adapter（新增内置工具实现）
- `agent_scope_workspace` — `WorkspaceBase::get_backend()`, `WorkspaceBackend`（`exec_shell`/`read_file`/`write_file`/`list_dir`/`file_exists`）, `Skill`
- `agent_scope_state` — `AgentState.tool_context.activated_groups`（ResetTools 激活状态落点）
- `agent_scope_message` — `ToolCallBlock`, `ToolResultBlock`, `ToolOutput`, `ToolResultState`
- vendored Python 参考实现 `agentscope/src/agentscope/tool/_builtin/`（契约基准）

**Storage**: N/A（内置工具通过 workspace backend 操作文件系统，无独立存储）

**Testing**: `cargo test`（单元 + 集成）、`cargo clippy`、`cargo fmt`；Golden snapshot + diff test 对齐 Python 参考实现（宪法 Art.6）

**Target Platform**: 跨平台（macOS/Linux/Windows）；`PowerShell` 仅 Windows 环境启用

**Project Type**: 库（多 crate workspace：`agent_scope_tool` / `agent_scope_workspace` / `agent_scope_agent` / `agent_scope_state`）

**Performance Goals**: 搜索工具结果有界（Grep/Glob 结果上限、单文件字节上限、扫描条目上限）；命令输出截断；避免将超大文件整体读入内存

**Constraints**:
- `agent_scope_workspace` MUST NOT 依赖 `agent_scope_tool`（宪法 Art.11 依赖方向）
- `agent_scope_agent` 当前不依赖 `agent_scope_workspace`，注入需通过配置层打通
- `Tool` 实现 MUST 零 `unsafe`（宪法 Art.9）
- 工具名/参数/描述/错误语义 MUST 对齐 Python `_builtin/` 参考实现（宪法 Art.1/Art.3）
- 内置工具不包含 ToolGroup/ToolMiddleware/MCP/Skill/Permission 的完整机制（本 feature 仅实现工具集 + 激活状态）

**Scale/Scope**: 9 个内置工具（Bash/Read/Edit/Write/Grep/Glob/PowerShell/ResetTools/Skill）；workspace 边界内文件操作；单 agent 会话级 read-state

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 检查项 | 状态 |
|------|--------|------|
| Art.1 兼容性优先 | 工具名、参数 schema、描述、错误文本对齐 Python `_builtin/` 参考实现 | ✅ |
| Art.2 锁定上游版本 | 兼容基准绑定 vendored Python 源码（上游 `9d1026fa`） | ✅ |
| Art.3 Python 是行为基准 | 契约取自 vendored `_builtin/_bash.py` 等实际源码，非文档推测 | ✅ |
| Art.4 先契约后实现 | 本 plan 输出 contracts/ 契约文件，先于实现 | ✅ |
| Art.5 不允许伪兼容 | PowerShell 非 Windows 平台返回 typed `UnsupportedCapability`，不静默降级 | ✅ |
| Art.6 测试驱动兼容 | Golden snapshot + diff test + 固定 Tool/Clock 组件（read-before-edit、超时、搜索界限） | ✅ |
| Art.7 Trace 核心验收 | 工具调用通过现有 ReAct tool-call 事件路径，含参数概要/错误类别/顺序 | ✅ |
| Art.8 Rust 原生设计 | 用 struct/enum/trait 表达工具契约与错误类别，不机械复制 Python 继承 | ✅ |
| Art.9 安全 Rust 优先 | 所有 `Tool` 实现零 `unsafe`；路径校验不 `unwrap` 用户输入 | ✅ |
| Art.10 结构化并发 | 命令执行经 `WorkspaceBackend::exec_shell`（有界输出、kill_on_drop、超时） | ✅ |
| Art.11 分层与依赖方向 | `agent_scope_workspace` 不依赖 `agent_scope_tool`；工具适配器在 tool crate | ✅ |
| Art.12 稳定数据协议 | 工具 input_schema 用 JSON Schema，`ToolError` typed enum | ✅ |
| Art.13 稳定错误模型 | `ToolError` 加 `UnsupportedCapability`；错误分类可机器可读 | ✅ |
| Art.14 可观测性 | 工具调用有 tracing span（名称/参数概要），不改变执行顺序 | ✅ |
| Art.15 性能不牺牲正确性 | 搜索界限/超时不改变事件顺序或吞错 | ✅ |
| Art.16 小步交付 | 单一 feature 仅内置工具集，不扩展完整 ToolGroup/MCP 机制 | ✅ |
| Art.17 完成定义 | plan 产物 + 测试 + clippy/fmt + 契约文档齐备 | ✅ |
| Art.18 兼容性分级 | 目标 L2（核心行为兼容） | ✅ |
| Art.19 变更治理 | 无宪法违反豁免 | ✅ |

**Gate Summary**: 全部条款通过，无违规。

## Project Structure

### Documentation (this feature)

```text
specs/029-agent-workspace-tools/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── bash.md          # Bash / PowerShell command contract
│   ├── read.md          # Read contract
│   ├── edit.md          # Edit contract
│   ├── write.md         # Write contract
│   ├── grep.md          # Grep contract
│   ├── glob.md          # Glob contract
│   ├── reset-tools.md   # ResetTools meta-tool contract
│   ├── skill.md         # Skill contract
│   └── workspace-tool-session.md  # Read-state / session contract
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/agent_scope_tool/            # 内置工具实现（Tool trait 适配器）
├── src/
│   ├── lib.rs                      # re-export 新工具模块
│   ├── tool_trait.rs               # Tool / ToolError（加 UnsupportedCapability variant）
│   ├── toolkit.rs                  # ToolGroup 激活状态接入 get_tool_schemas
│   └── builtin/                    # NEW: 内置工具实现目录
│       ├── mod.rs
│       ├── bash.rs                 # Bash 工具
│       ├── powershell.rs           # PowerShell 工具（Windows）
│       ├── read.rs                 # Read 工具（记录 read-state）
│       ├── edit.rs                 # Edit 工具（read-before-modify 守卫）
│       ├── write.rs                # Write 工具（read-before-overwrite 守卫）
│       ├── grep.rs                 # Grep 工具（内容搜索）
│       ├── glob.rs                 # Glob 工具（文件发现）
│       ├── reset_tools.rs          # ResetTools 元工具（激活状态切换）
│       └── session.rs              # WorkspaceToolSession（共享 read-state + 激活组）
└── tests/
    ├── builtin_bash_tests.rs
    ├── builtin_edit_write_tests.rs
    ├── builtin_search_tests.rs
    ├── builtin_reset_tools_tests.rs
    └── session_tests.rs

crates/agent_scope_workspace/       # 保持轻量：get_backend() 已提供后端句柄
└── src/
    ├── base.rs                     # WorkspaceBase::list_tools() 元数据（保持）
    └── backend.rs                  # WorkspaceBackend（exec_shell/read_file/...）

crates/agent_scope_agent/           # 注入连接点（不新增对 workspace 的依赖）
├── src/
│   ├── config.rs                   # AgentConfig 新增 workspace_tools 装配钩子
│   └── react_agent.rs              # 构造期合并 workspace 内置工具（仿 task tools 模式）

crates/agent_scope_state/           # 激活状态持久化
└── src/
    └── agent_state.rs              # ToolContext.activated_groups（已存在，供 ResetTools 读写）

examples/pi-rust/                   # 示例：workspace 启用时自动获得内置工具（验证 SC-005）
└── src/
    └── tools.rs                    # 可复用或验证现有工具实现
```

**Structure Decision**: 内置工具实现放 `agent_scope_tool` crate（它已依赖 `agent_scope_workspace`，可访问 `WorkspaceBase::get_backend()` 拿后端句柄），保持 `agent_scope_workspace` 不反向依赖 tool crate（宪法 Art.11）。`agent_scope_agent` 通过配置层装配（不新增 workspace 依赖），复用 Feature 024 任务工具的构造期注册模式。激活状态落点用 `agent_scope_state::ToolContext.activated_groups`（已存在，随会话持久化）。

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

无宪法违规，Complexity Tracking 不需要填写。以下是本 feature 的关键复杂度取舍，供 tasks 阶段参考：

| 设计点 | 采用方案 | 简化替代被否决的原因 |
|--------|---------|----------------------|
| 内置工具实现位置 | `agent_scope_tool` crate 内新增 `builtin/` 模块 | 放 `agent_scope_workspace` 会要求 workspace 依赖 tool crate，违反 Art.11 依赖方向 |
| 激活状态落点 | 复用 `AgentState.tool_context.activated_groups`（state crate） | 新建 session 级字段会与 state crate 既有激活语义重复，且丢失会话持久化 |
| `get_tool_schemas` 过滤 | ToolKit 内部持有激活组集合，同步过滤 | 让 tool crate 依赖 agent crate 会反转分层，不可行 |
| Read-state 守卫 | `WorkspaceToolSession`（工具 crate 内共享） | 放 `WorkspaceBase` 会把 agent 会话策略污染通用 workspace 抽象 |

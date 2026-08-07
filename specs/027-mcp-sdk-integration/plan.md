# Implementation Plan: MCP SDK Integration

**Branch**: `027-mcp-sdk-integration` | **Date**: 2026-08-07 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/027-mcp-sdk-integration/spec.md`

## Summary

引入官方 Rust MCP SDK（`rmcp` v3.1.1，modelcontextprotocol/rust-sdk）重构 `agent_scope_workspace` 的 `mcp.rs` 模块。

**现状痛点**：`mcp.rs`（216 行）仅有 `McpTransportConfig`/`McpClientConfig`/`McpRegistry` 三个配置管理类型——`.mcp` 文件的序列化/反序列化/脱敏——但从未与任何 MCP 服务器建立真实连接，更没有工具发现或调用能力。

**目标**：在保留现有配置管理与脱敏语义的前提下，叠加以下能力：
1. **`McpClient`**：包装 `rmcp::RunningService<RoleClient>` 管理连接生命周期，提供 `connect()`/`disconnect()`/`list_tools()`/`call_tool()`
2. **`McpTool`**：适配器实现 `agent_scope_tool::Tool` trait，将远端 MCP 工具接入既有 Agent 工具调用循环
3. **SSE 兼容过渡**：旧 `McpTransportConfig::Sse` 变体保留（`#[serde(alias)]` 自动解析），运行时提示映射到 `StreamableHttpClientTransport`
4. **进程内测试**：基于 SDK 内置 `WorkerTransport`，CI 无需外部进程或网络

**API 策略**：`WorkspaceBase` trait 新增 `connect_mcp()`/`disconnect_mcp()`/`get_mcp_tools()` 三个方法。配置注册（`add_mcp`/`remove_mcp`/`list_mcps`）与连接生命周期分离——注册是持久化配置，连接是按需建立的运行时操作。

核心设计决策见 [research.md](research.md)。

## Technical Context

**Language/Version**: Rust（workspace edition 2024，见根 `Cargo.toml`；MSRV 1.83+）

**Primary Dependencies**:
- `rmcp` v3.1.1（crates.io，license MIT），features: `client`, `transport-child-process`, `transport-streamable-http-client-reqwest`, `transport-worker`
- 内部 crate：`agent_scope_workspace`（修改目标）、`agent_scope_tool`（Tool trait）

**Storage**: `.mcp` JSON 文件（现有 `McpRegistry`，无格式变更）

**Testing**: `cargo test`（workspace）；`WorkerTransport` 进程内集成测试；序列化兼容性测试（SSE 配置 alias）；敏感头脱敏回归测试

**Target Platform**: 跨平台库（Linux / macOS / Windows）；stdio 传输仅在有子进程能力的平台可用

**Project Type**: library（多 crate Cargo workspace），仅修改 `agent_scope_workspace`

**Performance Goals**: 无独立性能目标——MCP 连接/工具调用为低频操作（I/O bound）

**Constraints**: `#![deny(unsafe_code)]`；库代码禁 unwrap/expect/panic；类型化错误；敏感头脱敏保持；无循环依赖

**Scale/Scope**: 仅修改 1 个 crate：重构 1 个文件（`mcp.rs`）、新增 2 个文件（`mcp_client.rs`/`mcp_tool.rs`）、小幅改动 3 个文件（`base.rs`/`error.rs`/`local_workspace.rs`/`lib.rs`）、新增 1 个测试文件

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 条款 | 符合性 | 说明 |
|------|--------|------|
| 第一条 兼容性优先 | ✅ | `McpTransportConfig::Sse` 变体保留，`#[serde(alias)]` 向后兼容存量 `.mcp`；`WorkspaceBase` 原有方法签名不变 |
| 第三条 Python 是行为基准 | ✅ | Python `MCPClient`（stdio/stdin 子进程 + HTTP streamable-http）与 `MCPTool` 适配器行为对比，见 contracts/ |
| 第四条 先契约后实现 | ✅ | spec → research → data-model → contracts 完整链路 |
| 第五条 不允许伪兼容 | ✅ | SSE→streamable-http 显式提示（`tracing::info!`），不静默丢弃 |
| 第六条 测试驱动兼容性 | ✅ | `WorkerTransport` 进程内测试，Mock Tool handler 控制行为，不依赖真实 LLM |
| 第八条 Rust 原生设计 | ✅ | `Arc<Mutex<Option<RunningService>>>` 管理连接；`McpTool` 实现 `Tool` trait |
| 第九条 安全 Rust 优先 | ✅ | 无 unsafe |
| 第十条 结构化并发 | ✅ | SDK 内部管理后台任务，`McpClient` 持有 `RunningService` 所有权，空 drop 时 cancel |
| 第十一条 分层与依赖方向 | ✅ | `rmcp` 仅由 `agent_scope_workspace` 消费，不引入新 crate 间边 |
| 第十二条 稳定数据协议 | ✅ | 配置字段零删除，仅内部新增 optional 字段用于运行时连接状态 |
| 第十三条 稳定错误模型 | ✅ | 新增 `McpConnectionError`/`McpCallError`/`McpNotConnected`，映射自 `rmcp::ServiceError` |
| 第十八条 兼容性分级 | ✅ | 目标等级 L2（核心行为兼容）+ L3（公开 API 语义兼容）|

**Gate 结果（Phase 0 前）**: 通过，无违规需论证。

**Gate 结果（Phase 1 后复审）**: 通过。设计产物（research/data-model/contracts/quickstart）与宪法条款一致：
- `McpTool` 通过 `Tool` trait 注入，符合第八条（`Arc<dyn Tool>` trait object 模式）
- `McpClient` 持有 `RunningService`，close 时显式 cancel，符合第十条（结构化并发）
- 错误模型新增三个变体映射 `rmcp::ServiceError`，符合第十三条（类型化错误）
- SSE 兼容路径通过 `#[serde(alias)]` + `info!` 提示，符合第五条（不伪兼容）
- `WorkerTransport` 测试策略符合第六条（可重复测试组件）

## Project Structure

### Documentation (this feature)

```text
specs/027-mcp-sdk-integration/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
crates/agent_scope_workspace/
├── Cargo.toml           # +rmcp 依赖; +dev-dependencies 含 tokio full features
├── src/
│   ├── lib.rs           # +pub mod mcp_client; +pub mod mcp_tool; re-export McpClient/McpTool
│   ├── mcp.rs           # 保留 McpTransportConfig(含 SSE alias) / McpClientConfig / McpRegistry
│   ├── mcp_client.rs    # [NEW] McpClient — 包装 RunningService 生命周期管理
│   ├── mcp_tool.rs      # [NEW] McpTool — 适配器: rmcp::Tool → agent_scope_tool::Tool
│   ├── base.rs          # WorkspaceBase: +connect_mcp/+disconnect_mcp/+get_mcp_tools
│   ├── error.rs         # +McpConnectionError/+McpCallError/+McpNotConnected
│   └── local_workspace.rs # 实现连接生命周期方法
└── tests/
    ├── mcp_integration_tests.rs  # [NEW] WorkerTransport 进程内集成测试
    ├── resource_tests.rs         # 已有敏感头脱敏测试保留
    └── lifecycle_tests.rs        # 更新: close/reset 断开连接验证
```

**Structure Decision**: 在 `agent_scope_workspace` 现有模块基础上叠加 `mcp_client.rs` + `mcp_tool.rs`。`mcp.rs` 保持配置类型职责不变。不新增 crate。这种叠加式的结构选择基于：`rmcp` 仅由 workspace 消费、不改动其他 crate 的依赖边界、配置存储与运行时连接是不同关注面。

## Complexity Tracking

> 无宪法违反，无需填此表。

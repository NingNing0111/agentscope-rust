# Tasks: MCP SDK Integration

**Input**: Design documents from `/specs/027-mcp-sdk-integration/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: spec 明确要求每个公开 MCP 能力有对应测试（FR-016），且 US4 专门要求进程内 WorkerTransport 测试。测试任务已包含在各 Phase 中。

**Organization**: 任务按用户故事分组，支持独立实现与测试。US1+US2 同为 P1 且存在强依赖，合并为一个 Phase。

## 架构修正说明（相对 plan.md）

> **循环依赖破环（用户已确认）**：plan.md 将 `McpTool` 放在 `agent_scope_workspace` 中实现 `agent_scope_tool::Tool`，但 `agent_scope_tool → agent_scope_workspace` 依赖已存在（`skill_loader.rs:7` 等引用 `Skill`），会形成 crate 循环依赖，Cargo 无法编译（宪法第十一条）。
>
> **决策**：新增 **`crates/agent_scope_mcp`** crate，承载 `McpClient`（连接生命周期）与 `McpTool`（Tool 适配器）。依赖方向单向无环：
>
> ```
> agent_scope_mcp → agent_scope_workspace  (McpClientConfig, WorkspaceError, McpConnectionHandle)
> agent_scope_mcp → agent_scope_tool       (Tool trait, ToolError, ToolExecOutput)
> agent_scope_mcp → rmcp                    (协议实现)
> ```
>
> **API 策略**：`WorkspaceBase` 主 trait 若新增返回 `Arc<dyn Tool>` 的方法会引入 `workspace → tool` 边（再次成环）。因此在 `agent_scope_mcp` 定义 **`McpExt` 扩展 trait**（`connect_mcp`/`disconnect_mcp`/`get_mcp_tools`），对 `LocalWorkspace` 实现——反而更符合宪法第一条"不改变 `WorkspaceBase` 公开方法签名"。`WorkspaceBase` 仅新增一个轻量 `McpConnectionHandle` trait 与 `McpConnectionsHost` 访问器，供 `McpExt` 与 `close()/reset()` 释放连接。
>
> **已核实**：`McpTransportConfig` 的 `#[serde(rename="sse")]`/`#[serde(rename="streamable_http")]` 已使存量 `.mcp` 的 `"type":"sse"` 可解析，SSE 解析兼容基础已就绪，无需新增 alias。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 可并行执行（不同文件，无依赖）
- **[Story]**: 归属的用户故事（US12=US1+US2, US3, US4）
- 描述中包含精确文件路径

## Path Conventions

- 新 crate: `crates/agent_scope_mcp/`（源码 `src/`，测试 `tests/`）
- 修改 crate: `crates/agent_scope_workspace/`

---

## Phase 1: Setup（共享基础设施）

**Purpose**: 创建 `agent_scope_mcp` crate、扩展错误类型、定义连接句柄 trait、确认传输序列化兼容——所有用户故事的前置条件

- [X] T001 [P] 创建 `crates/agent_scope_mcp/Cargo.toml`：`[dependencies]` 含 `agent_scope_workspace`、`agent_scope_tool`（均为 path 依赖）、`rmcp = { version = "3.1.1", default-features = false, features = ["client", "transport-child-process", "transport-streamable-http-client-reqwest", "transport-worker"] }`、`tokio = { version = "1", features = ["full"] }`、`serde_json.workspace`、`tracing = "0.1"`、`async-trait = "0.1"`；`[dev-dependencies]` 含 `tempfile = "3"`；创建 `crates/agent_scope_mcp/src/lib.rs` 骨架（`#![deny(unsafe_code)]`、`#![deny(clippy::unwrap_used)]`、声明 `pub mod mcp_client; pub mod mcp_tool;`）
- [X] T002 [P] 在 `crates/agent_scope_workspace/src/error.rs` 的 `WorkspaceError` 枚举新增三个变体：`McpConnectionError { name: String, reason: String }`、`McpCallError { mcp_name: String, tool_name: String, reason: String }`、`McpNotConnected { name: String }`，并同步更新 `Display` impl 与 `std::error::Error` impl（reason 字段文档注明绝不包含认证秘密）
- [X] T003 [P] 在 `crates/agent_scope_workspace/src/mcp.rs` 确认 `McpTransportConfig` 序列化兼容：验证 `#[serde(tag = "type")]` + `#[serde(rename = "sse")]`/`#[serde(rename = "streamable_http")]` 已使存量 `"type":"sse"` 与 `"type":"streamable_http"` 可反序列化；添加一个 `#[cfg(test)]` 往返测试断言 `"type":"sse"` JSON 解析为 `McpTransportConfig::Sse`
- [X] T004 [P] 在 `crates/agent_scope_workspace/src/base.rs` 新增两个轻量 trait：`McpConnectionHandle: Send + Sync`（含 `fn name(&self) -> &str`、`async fn disconnect(&self) -> Result<(), WorkspaceError>`、`fn as_any(&self) -> &dyn std::any::Any`）与 `McpConnectionsHost`（含 `fn mcp_connections(&self) -> &Arc<tokio::sync::Mutex<HashMap<String, Arc<dyn McpConnectionHandle>>>>`），两者均不引用 `agent_scope_tool`（避免引入 `workspace→tool` 边）
- [X] T005 在 `crates/agent_scope_workspace/src/lib.rs` 的 `pub use` 行新增 `McpConnectionHandle`、`McpConnectionsHost` 的 re-export（依赖 T004 的 trait 定义）

**Checkpoint**: 基础设施就绪——`agent_scope_mcp` crate 可编译，错误类型已扩展，连接句柄 trait 已定义，SSE 解析兼容已确认

---

## Phase 2: User Story 1+2 — 连接、发现与调用 MCP 工具 (Priority: P1) 🎯 MVP

**Goal**: 通过 `agent_scope_mcp` crate 实现 MCP 客户端连接生命周期与远端工具适配，使 Agent 能通过统一 `Tool` trait 调用远端 MCP 工具。

**Independent Test**: 在进程内 WorkerTransport 测试中：注册配置 → `connect_mcp()` → 断言工具列表非空且工具名符合 `{mcp_name}/{tool_name}` 格式 → 通过 `McpTool::call()` 调用工具并验证返回结果。

### 实现 for User Story 1+2

- [X] T006 [P] [US12] 创建 `crates/agent_scope_mcp/src/mcp_client.rs`：定义 `McpClient` 结构体（字段：`name: String`、`config: McpClientConfig`、`service: Arc<Mutex<Option<rmcp::RunningService<RoleClient, ()>>>>`、`tools_cache: Mutex<Vec<rmcp::model::Tool>>`），结构体实现 `Debug`（手动，跳过 `service` 字段）
- [X] T007 [US12] 在 `crates/agent_scope_mcp/src/mcp_client.rs` 实现 `McpClient::new(config: McpClientConfig) -> Self`，`name` 取自 `config.name`
- [X] T008 [US12] 在 `crates/agent_scope_mcp/src/mcp_client.rs` 实现 `McpClient::connect(&self) -> Result<(), WorkspaceError>`：按 `McpTransportConfig` 变体创建 transport（`Stdio` → `TokioChildProcess::new`；`StreamableHttp` → `StreamableHttpClientTransport::from_uri`；`Sse` → 运行时映射到 `StreamableHttpClientTransport::from_uri` 并发出 `tracing::info!("MCP SSE config '{}' mapped to streamable-http transport", name)`），调用 `().serve(transport).await` 建立连接，`list_all_tools().await` 填充 `tools_cache`；映射 `rmcp::ClientInitializeError` → `WorkspaceError::McpConnectionError`
- [X] T009 [US12] 在 `crates/agent_scope_mcp/src/mcp_client.rs` 实现 `McpClient::disconnect(&self) -> Result<(), WorkspaceError>`：`take()` 出 `service`，调用 `RunningService::cancel().await`，清空 `tools_cache`；未连接时静默成功
- [X] T010 [US12] 在 `crates/agent_scope_mcp/src/mcp_client.rs` 实现 `McpClient::list_tools(&self) -> Result<Vec<rmcp::model::Tool>, WorkspaceError>`：返回 `tools_cache` 的克隆，未连接（`service` 为 `None`）时返回 `WorkspaceError::McpNotConnected`
- [X] T011 [US12] 在 `crates/agent_scope_mcp/src/mcp_client.rs` 实现 `McpClient::call_tool(&self, tool_name: &str, arguments: serde_json::Map<String, Value>) -> Result<rmcp::model::CallToolResult, WorkspaceError>`：通过 `peer.call_tool(CallToolRequestParams::new(tool_name).with_arguments(arguments))` 转发；映射 `rmcp::ServiceError::McpError`/`Timeout` → `McpCallError`、`TransportSend`/`TransportClosed` → `McpConnectionError`；未连接时返回 `McpNotConnected`
- [X] T012 [US12] 在 `crates/agent_scope_mcp/src/mcp_client.rs` 实现 `McpConnectionHandle` trait（`name()`、`disconnect()` 委托、`as_any()` 返回 `&dyn Any`、`into_any()` 恢复 `Arc` downcast）
- [X] T013 [P] [US12] 创建 `crates/agent_scope_mcp/src/mcp_tool.rs`：定义 `McpTool` 结构体（字段：`tool_name`、`display_name`、`description`、`input_schema`、`client: Arc<McpClient>`、`read_only`），实现 `Debug`
- [X] T014 [US12] 在 `crates/agent_scope_mcp/src/mcp_tool.rs` 实现 `McpTool::new(mcp_name: String, rmcp_tool: rmcp::model::Tool, client: Arc<McpClient>) -> Self`：从 `rmcp::model::Tool` 提取 name/description/input_schema/annotations.read_only_hint
- [X] T015 [US12] 在 `crates/agent_scope_mcp/src/mcp_tool.rs` 为 `McpTool` 实现 `agent_scope_tool::Tool` trait：`name()` 返回 `"{mcp_name}/{tool_name}"`（自动去重）；`description()` 返回 `"[remote MCP: {mcp_name}] {desc}"`；`input_schema()` 返回远端 JSON Schema 的克隆；`is_concurrency_safe()` 返回 `true`；`is_read_only()` 返回 `read_only`
- [X] T016 [US12] 在 `crates/agent_scope_mcp/src/mcp_tool.rs` 实现 `Tool::call(&self, input: JsonValue) -> Result<ToolExecOutput, ToolError>`：将 input 转为 `serde_json::Map` → 调用 `client.call_tool()` → 将 `CallToolResult` 映射为 `ToolExecOutput::Complete(ToolResultBlock { id, name: self.name().to_string(), output: ToolOutput::Text(拼接的 content text), state: is_error ? Error : Success, is_last: true })`；映射 `McpNotConnected`/`McpCallError`/`McpConnectionError` → `ToolError::Execution`
- [X] T017 [US12] 在 `crates/agent_scope_mcp/src/lib.rs` 定义 `McpExt` trait（`#[async_trait::async_trait]`）：`async fn connect_mcp(&mut self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>`、`async fn disconnect_mcp(&mut self, name: &str) -> Result<(), WorkspaceError>`、`async fn get_mcp_tools(&self, name: &str) -> Result<Vec<Arc<dyn Tool>>, WorkspaceError>`；re-export `McpClient`/`McpTool`/`McpExt`（依赖 T006、T013 的类型存在）
- [X] T018 [US12] 在 `crates/agent_scope_mcp/src/lib.rs` 为 `LocalWorkspace` 实现 `McpExt::connect_mcp`：`self.list_mcps()` 找到对应 `McpClientConfig`（不存在 → `McpNotFound`）→ 检查 `mcp_connections()` 是否已含该 name（含 → `McpAlreadyExists`）→ 创建 `McpClient::new` + `connect()` → 为每个 `rmcp::Tool` 构造 `McpTool` 收集 `Vec<Arc<dyn Tool>>` → 将 `Arc<McpClient>` 存入 `mcp_connections()` → 返回工具列表
- [X] T019 [US12] 在 `crates/agent_scope_mcp/src/lib.rs` 实现 `McpExt::disconnect_mcp`（从 `mcp_connections()` 移除并调用 `disconnect()`，不存在 → `McpNotConnected`）与 `McpExt::get_mcp_tools`（通过 `into_any()` downcast 到 `Arc<McpClient>` 复用 `tools_cache` 构造 `McpTool` 列表，未连接 → `McpNotConnected`）
- [X] T020 [P] [US12] 在 `crates/agent_scope_workspace/src/local_workspace.rs` 的 `LocalWorkspace` 结构体新增字段 `_mcp_connections: Arc<tokio::sync::Mutex<HashMap<String, Arc<dyn McpConnectionHandle>>>>`，在 `new()` 中初始化，并实现 `McpConnectionsHost` trait 的 `mcp_connections()` 访问器（返回 `&Arc<Mutex<...>>`）
- [X] T021 [US12] 在 `crates/agent_scope_workspace/src/local_workspace.rs` 更新 `close()`：在现有清理逻辑（`_mcps.clear()`、`is_alive = false`）中新增——遍历 `_mcp_connections` 逐个调用 `disconnect()` 并清空 map（满足 FR-010）
- [X] T022 [US12] 在 `crates/agent_scope_workspace/src/local_workspace.rs` 更新 `reset()`：在现有清理逻辑（删除 `.mcp`、目录重建）中新增——断开所有 `_mcp_connections` 并清空（满足 FR-010）

### 测试 for User Story 1+2

- [X] T023 [P] [US12] 创建 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs`：实现 `create_test_mcp_server_with_add_tool()` fixture——使用 `rmcp::WorkerTransport` 在进程内建立 client↔server 通道，server 暴露一个 `"add"` 工具（JSON Schema 含 `a: i64, b: i64`，handler 返回 `a + b`），返回已连接的 `Arc<McpClient>` 与工具名
- [X] T024 [US12] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_connect_and_list_tools`：连接成功后 `list_tools()` 返回包含 `"add"` 的工具列表
- [X] T025 [US12] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_call_tool_success`：`call_tool("add", {"a": 1, "b": 2})` 返回 `3`
- [X] T026 [US12] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_call_tool_error`：传入非法参数（缺字段/类型错误），断言返回类型化错误（`McpCallError` 或 `ToolError::Execution`）
- [X] T027 [US12] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_mcp_tool_name_prefix`：mcp 名为 `"search"`、工具名为 `"query"` 时，`McpTool::name()` 返回 `"search/query"`，`description()` 返回 `"[remote MCP: search] ..."`
- [X] T028 [US12] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_disconnect_releases_connection`：`disconnect()` 后 `call_tool()` 返回 `McpNotConnected`
- [X] T029 [US12] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_not_connected_returns_error`：未连接时 `call_tool()`/`list_tools()` 返回 `McpNotConnected`

**Checkpoint**: US1+US2 完成——MCP 连接、工具发现、工具调用全部可用，通过 `McpExt` 扩展 trait 与统一 `Tool` 接口暴露给 Agent

---

## Phase 3: User Story 3 — SSE 配置向后兼容 (Priority: P2)

**Goal**: 存量 `.mcp` 文件中的 SSE 传输配置升级后不失效——解析成功并映射到 streamable-http 传输，敏感头脱敏逻辑完整回归，`.mcp` 损坏时安全回退。

**Independent Test**: 加载一个包含 `"type": "sse"` 配置的 `.mcp` 文件 → 验证解析成功 → 验证 `connect_mcp()` 发出映射提示日志 → 验证敏感头不被持久化。

### 实现 for User Story 3

- [X] T030 [US3] 在 `crates/agent_scope_workspace/src/local_workspace.rs` 的 `initialize()` 中处理 `.mcp` 加载失败：当 `McpRegistry::load` 返回 `CorruptMcpFile` 时，回退到 `default_mcps` 播种并发出 `tracing::warn!`，不崩溃（满足 FR-005）
- [X] T031 [US3] 在 `crates/agent_scope_workspace/src/mcp.rs` 确认宽容解析：验证 `McpClientConfig`/`McpTransportConfig` 反序列化时未知字段被忽略（不 `deny_unknown_fields`），添加一个 `#[cfg(test)]` 测试断言含未知字段的 `.mcp` JSON 仍可解析且已知字段不丢失（满足 FR-004）；同时确认 T008 中 SSE→streamable-http 映射路径的 `info!` 提示在 `connect_mcp()` 时实际输出

### 测试 for User Story 3

- [X] T032 [P] [US3] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_sse_config_parsed_and_mapped`：构造 `"type": "sse"` 的 JSON 配置 → 反序列化为 `McpTransportConfig::Sse` → 断言成功，且运行时映射路径发出 `info!` 日志
- [X] T033 [P] [US3] 在 `crates/agent_scope_workspace/tests/resource_tests.rs` 确认已有敏感头脱敏测试（`test_mcp_sse_sensitive_headers_not_persisted`、`test_mcp_streamablehttp_sensitive_headers_not_persisted`、`test_mcp_list_mcps_scrubs_headers`）在本次变更后仍然通过，断言 `.mcp` 文件与 `list_mcps()` 返回值中敏感头值均为 `[REDACTED]`（满足 FR-003/SC-005）

**Checkpoint**: US3 完成——存量配置无缝升级，敏感信息脱敏回归通过，损坏 `.mcp` 安全回退

---

## Phase 4: User Story 4 — 进程内测试与开发体验 (Priority: P3)

**Goal**: CI 环境中无需外部进程或网络即可运行 MCP 集成测试，覆盖连接失败、并发调用、close/reset 资源释放等异常场景。

**Independent Test**: `cargo test -p agent_scope_mcp` 在无网络 CI 环境全部通过。

### 实现 for User Story 4

- [X] T034 [US4] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_connection_error_typed`：使用无效 URL（或构造 transport 失败）建立连接，验证返回 `McpConnectionError` 而非 panic（满足 FR-009/SC-004）
- [X] T035 [US4] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_concurrent_tool_calls`：多个 `McpTool` 实例并发调用同一 `McpClient`，验证无死锁、无数据竞态、结果一致
- [X] T036 [US4] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_close_disconnects_all_mcps`：通过 `LocalWorkspace` + `McpExt` 连接后调用 `ws.close()`，断言 `_mcp_connections` 清空、`get_mcp_tools()` 返回 `McpNotConnected`
- [X] T037 [US4] 在 `crates/agent_scope_mcp/tests/mcp_integration_tests.rs` 编写 `test_reset_clears_mcps`：`ws.reset()` 后 `list_mcps()` 返回空，MCP 连接全部断开
- [X] T038 [US4] 在 `crates/agent_scope_workspace/tests/lifecycle_tests.rs` 新增测试：验证 `close()` + `reset()` 场景下 MCP 连接正确释放（与 T036/T037 互补，从 workspace 侧断言）

**Checkpoint**: US4 完成——CI 测试覆盖完整，所有错误路径验证通过

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: 质量门——lint、format、文档、快速验证、示例对齐

- [X] T039 [P] 更新 `crates/agent_scope_mcp/src/lib.rs` 模块级文档：在 `//!` 注释中添加 `McpExt` 使用示例（`ws.connect_mcp("search")` → 获取 `Vec<Arc<dyn Tool>>` → 交给 Agent 调用）
- [X] T040 [P] 执行 `cargo clippy -p agent_scope_mcp -p agent_scope_workspace -- -D warnings` 并修复所有 lint（确认无新增 `unsafe`、无新增 `unwrap`/`expect`）
- [X] T041 [P] 执行 `cargo fmt --check -p agent_scope_mcp -p agent_scope_workspace` 确保格式通过
- [X] T042 按 `specs/027-mcp-sdk-integration/quickstart.md` 场景执行完整验证流程：`cargo test -p agent_scope_mcp -p agent_scope_workspace && cargo clippy -p agent_scope_mcp -p agent_scope_workspace -- -D warnings && cargo fmt --check -p agent_scope_mcp -p agent_scope_workspace`；确认真实传输测试（stdio/HTTP）若存在则以 `#[ignore]` 标记不阻塞 CI
- [X] T043 运行 `cargo test` workspace 级全量测试确认无回归；在根 `Cargo.toml` 的 `[dependencies]` 中添加 `agent_scope_mcp = { path = "crates/agent_scope_mcp" }`（供后续示例使用）

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: 无依赖——可立即开始
- **Phase 2 (US1+US2 MVP)**: 依赖 Phase 1 完成——**阻塞所有后续用户故事**
- **Phase 3 (US3)**: 依赖 Phase 2 完成（需要 `McpClient` 的 SSE 映射逻辑已就绪）
- **Phase 4 (US4)**: 依赖 Phase 2 完成（需要 `McpClient`/`McpTool`/`McpExt` 已可用）
- **Phase 5 (Polish)**: 依赖所有用户故事完成

### User Story Dependencies

- **US1+US2 (P1)**: 可从 Phase 1 完成后开始——无其他故事依赖
- **US3 (P2)**: 可从 Phase 2 完成后开始——依赖 T008 的 SSE 映射逻辑与 T030/T031
- **US4 (P3)**: 可从 Phase 2 完成后开始——依赖 `McpClient`/`McpTool`/`McpExt` 存在

### Within Each Phase

- T002/T003/T004 可并行（不同文件：error.rs / mcp.rs / base.rs），T001 与三者不同文件可并行
- T005 依赖 T004（lib.rs re-export）
- T006/T013/T020 可并行（mcp_client.rs / mcp_tool.rs / local_workspace.rs 三个不同文件）
- T017 依赖 T006 + T013；T018/T019 依赖 T017 + T020
- T023 fixture 必须先于 T024-T029 完成（同一测试文件，顺序依赖）
- T032/T033 可并行；T030/T031 可并行
- Phase 3 (US3) 与 Phase 4 (US4) 可并行执行（都只依赖 Phase 2）
- Polish 阶段 T039/T040/T041 全部可并行；T042/T043 顺序执行

### Parallel Opportunities

```text
# Phase 2 内部并行（跨文件）:
T006 (McpClient struct) + T013 (McpTool struct) + T020 (local_workspace 字段) → 并行
  → T007-T012 (McpClient 方法) 同文件顺序依赖
  → T014-T016 (McpTool 方法) 依赖 T013
  → T017 (McpExt trait) 依赖 T006+T013
  → T018/T019 (McpExt impl) 依赖 T017+T020

# 跨 Phase 并行:
Phase 3 (US3) 和 Phase 4 (US4) 可并行执行（都只依赖 Phase 2）

# Polish 阶段:
T039 + T040 + T041 → 全部可并行
```

---

## Implementation Strategy

### MVP First (US1+US2 Only)

1. 完成 Phase 1: Setup（T001-T005）
2. 完成 Phase 2: US1+US2（T006-T029）
3. **STOP and VALIDATE**: 运行 `cargo test -p agent_scope_mcp`，确认连接/工具发现/工具调用通过
4. MVP 已就绪：Agent 可通过 `McpExt` + 统一 `Tool` 接口调用远端 MCP 工具

### Incremental Delivery

1. Phase 1 + Phase 2 → MVP（核心价值交付）
2. Phase 3 → SSE 向后兼容保障（存量用户无忧升级）
3. Phase 4 → CI 测试完善（质量保障）
4. Phase 5 → Lint/Fmt/Docs/示例 → 可发布

### Task Summary

| Phase | 任务数 | 说明 |
|-------|--------|------|
| Phase 1: Setup | 5 | 新 crate、错误类型、连接句柄、序列化兼容 |
| Phase 2: US1+US2 | 24 | McpClient + McpTool + McpExt + 测试 |
| Phase 3: US3 | 4 | SSE 兼容 + 损坏回退 + 脱敏回归 |
| Phase 4: US4 | 5 | CI 测试矩阵完善 |
| Phase 5: Polish | 5 | Lint/Fmt/Docs/验证/示例 |
| **Total** | **43** | |

## Notes

- [P] 任务 = 不同文件或无交叉依赖 → 可并行
- [US12] = User Story 1+2 合并（同为 P1 且强耦合）；[US3] / [US4] = 各自独立的用户故事
- **架构关键**：`agent_scope_mcp` 是新 crate，单向依赖 `workspace` + `tool`，无循环依赖（宪法第十一条）；`McpExt` 是扩展 trait，`WorkspaceBase` 公开签名保持不变（宪法第一条）
- `McpClient` 内部用 `Arc<Mutex<Option<RunningService>>>` 管理连接，`_mcp_connections` 由 `tokio::sync::Mutex` 保护；`McpTool` 通过 `Arc<McpClient>` 共享客户端实例，多工具共享同一连接
- SSE 兼容：`#[serde(rename="sse")]` 已保证存量 `.mcp` 解析；运行时 `connect()` 映射到 streamable-http 并 `info!` 提示（FR-002）
- 敏感头脱敏逻辑在 `mcp.rs` 的 `scrubbed()` 方法中保持不变，新增代码不绕过该路径（FR-003）
- 库代码禁用 `unwrap`/`expect`/`panic`（宪法第九条）；`#![deny(unsafe_code)]` 已声明

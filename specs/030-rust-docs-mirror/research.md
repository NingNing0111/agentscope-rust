# Phase 0 研究：Feature 030 docs/rust 镜像

**日期**: 2026-08-13
**来源**: 14 crate 公开 API 扫描（Explore 子代理，65 次工具调用）、docs/python 页面采样、兼容性矩阵（287 条目）、CHANGELOG、README、pi-rust 示例

## 1. 未知项解析

### R1: 各能力模块在 Rust 中的实现状态（决定每页状态标注）

**Decision**: 以子代理对 14 crate 的公开 API 扫描为准，结合兼容性矩阵与 CHANGELOG 交叉确认。核心结论：

| 能力域 | 状态 | 证据 |
|--------|------|------|
| Agent 系统（reply/reply_stream/observe/interrupt/human-in-the-loop/状态恢复） | ✅ 已实现 | `Agent` trait、`ReActAgent`、`PermissionResult::RequireConfirm`→`RequireUserConfirmEvent`、`ReActAgent::interrupt()`、`JsonFileSessionStore`(Feature 025) |
| Agent 结构化输出 | ⚠️ 部分支持 | 模型层 `generate_structured_output` 已实现；react 循环未直接接线（`reply_context.structured_schema` 存在但无 active 调用点） |
| Model（LLM） | ✅ 已实现 | `ChatModel` trait + `DashScopeChatModel`（唯一 provider，OpenAI 兼容） |
| Embedding | ✅ 已实现 | `EmbeddingModel` trait + `DashScopeEmbeddingModel` |
| TTS | ❌ 未实现 | 全仓无 TTS 类型；仅 `wav_header.rs`/`append_data_block`（播放能力非生成） |
| Permission 系统 | ✅ 已实现（较完整） | `PermissionEngine`/`PermissionMode`(5)/`PermissionRule`(allow/deny/ask)/`PermissionDecision`；比 Python 细粒度 admin policy 为部分覆盖 |
| Context 压缩 | ✅ 已实现 | `ContextConfig` + `compress_context`（`react_loop.rs:165`；当前为移除最旧消息+占位摘要，模型摘要策略 deferred） |
| Context 卸载 | ✅ 已实现 | `WorkspaceBase::offload_context/offload_tool_result`（`workspace/src/offload.rs`） |
| Environment awareness（运行时状态注入） | ✅ 已实现 | `InjectionConfig` + `maybe_inject_runtime_state`（Feature 026，HintBlock + HintBlockEvent） |
| Middleware | ✅ 已实现 | `Middleware` trait 9 钩子（FIFO，全默认 no-op），`MemoryMiddleware`/`RAGMiddleware` |
| 长期记忆 | ✅ 已实现 | `Memory`/`FileMemory`(MD+frontmatter+MEMORY.md)/`TurbovecMemory`(向量索引) |
| RAG | ✅ 已实现 | `KnowledgeBase`/`TurbovecVectorStore`/`RAGMiddleware`(Static+Agentic) |
| 消息与事件 | ✅ 已实现 | `Msg`/`ContentBlock`(7 块含 Unknown)、`AgentEvent`(33 事件)、流式 delta |
| Plan | ✅ 已实现（形态不同） | Feature 021 Planner 已移除→内置任务规划工具 `TaskCreateTool`/`TaskListTool`/`TaskGetTool`/`TaskUpdateTool` + 未完成任务注入（Feature 024） |
| Console | ❌ 未实现（库层） | 无 console crate；仅示例 pi-rust 有 ratatui TUI |
| Tool 系统 | ✅ 已实现 | `Tool` trait/`ToolKit`/`ToolGroup`/`FunctionTool`；内置 Bash/Read/Write/Edit/Grep/Glob/PowerShell(Win)/ResetTools/Skill；**ListDir 在 crate 内无**（pi-rust 用 FunctionTool 自实现） |
| Tool/MCP | ✅ 已实现（客户端） | `McpClient`/`McpTool`/`McpExt`（rmcp v3.1.1） |
| Tool/python-tool | ⚠️ 部分支持 | 等价能力为 `FunctionTool`（Rust 函数，非 Python 执行器） |
| Skill | ✅ 已实现 | `SkillViewer`/`LocalSkillLoader`/`SkillTool`/`SkillManager` |
| Workspace | ✅ 已实现（本地） | `WorkspaceBase`/`LocalWorkspace`/`WorkspaceBackend`(Local/Contained)；无 Docker/E2B 后端 |
| Workspace 资源管理 | ⚠️ 部分支持 | `WorkspaceManager`(多租户+TTL)；无独立资源配额模型 |
| Workspace MCP gateway | ⚠️ 部分支持 | 仅客户端接入（`McpExt::connect_mcp`）；无服务端 gateway |
| Sandbox | ✅ 已实现（本地隔离） | `SandboxSession`/`LocalSandboxSession`/`SandboxPolicy`/`SandboxPathResolver`；**非 Docker**，cpu/memory 限制本地后端不可强制 |
| Deploy: agent-service | ❌ 未实现 | 无 HTTP server（无 axum/actix 依赖） |
| Deploy: agent-team | ⚠️ 部分支持 | `SubAgent`/`SubAgentRegistry`/`delegate_*`/`MultiAgentConversation`（库级）；无完整 team 框架 |
| Deploy: channel(飞书/Discord/custom/routing) | ❌ 未实现 | 无 channel 代码 |
| Deploy: hub(mcp-hub/skill-hub) | ❌ 未实现 | 仅本地 `SkillManager`/`McpRegistry` |
| Deploy: sharing | ❌ 未实现 | — |
| Deploy: workspace-manager | ⚠️ 部分支持（本地） | `WorkspaceManager`(本地多租户+TTL)，非部署形态 |
| Deploy: rag | ⚠️ 部分支持（库级） | `RAGMiddleware` 已实现；无 RAG HTTP 服务 |

**Rationale**: 文档状态标注必须反映真实实现，禁止伪兼容（宪法 §5）。子代理扫描了全部 crate 的 pub API 与证据路径，可信度高。

**Alternatives considered**: 仅依赖兼容性矩阵（287 条多 NOT_ANALYZED，不反映真实实现）；仅依赖 CHANGELOG（无逐页粒度）。最终采用子代理代码扫描 + 矩阵/CHANGELOG 交叉验证。

### R2: docs/python 的页面格式（决定 docs/rust 页面怎么写）

**Decision**: 沿用 docs/python 的 Mintlify `.mdx` 语法：YAML frontmatter（`title`/`description`）、`<CardGroup>`/`<Card>`、`<Note>`/`<Tip>`、`<AccordionGroup>`/`<Accordion>`、`<Tree>`/`<Tree.Folder>`/`<Tree.File>`、`<Steps>`/`<Step>`、`<Frame>`、`<Badge>`、mermaid 图（```mermaid）、表格、版本化站内链接 `/versions/<ver>/zh/<path>`。

**Rationale**: 一比一镜像要求导航与视觉形态一致，用户可对照切换。

**Alternatives considered**: 改写为纯 Markdown（破坏一致性，否）。

### R3: 版本差（docs/python 为 2.0.7dev，Rust 兼容基线为 2.0.5）

**Decision**: 镜像源锁定 docs/python（2.0.7dev 路径），Rust 兼容基线锁定 v2.0.5（CHANGELOG: commit `27b6a0d2`）。文档索引页显式声明版本差：2.0.7dev 新增而 Rust 未实现的能力一律标注「计划中」。站内链接版本号用 Rust 侧版本（当前 `0.1.0`，与 Cargo.toml `version` 一致）。

**Rationale**: 宪法 §2 要求锁定上游版本。docs/python 是用户对标 Python 能力清单的镜像源，但行为基准是 2.0.5；如实声明避免误导。

**Alternatives considered**: 以 2.0.7dev 为兼容目标（无锁定基线，违宪 §2，否）。

### R4: 示例 crate 依赖与 CI 约束

**Decision**: 每个示例为 workspace 成员 crate（登记根 `Cargo.toml` `[workspace] members`），复用 workspace 已有依赖（tokio/serde/schemars/async-trait/futures/dotenv/clap 等），不引入新重依赖。由 CI `cargo check --workspace --all-targets` 与 `cargo clippy -D warnings` 自动编译校验。示例代码 `#![deny(unsafe_code)]`、typed errors、凭据缺失给明确提示（非 panic）。

**Rationale**: 宪法 §6（示例可编译）、§9（安全 Rust）、§13（typed errors）、§17（示例代码可编译运行）。复用 pi-rust 模式已验证可行。

**Alternatives considered**: 文档内嵌代码片段（无法编译校验，违 FR-005/006，否）；示例不注册 workspace（CI 不校验，否）。

### R5: ListDir 工具归属

**Decision**: `agent_scope_tool` crate 内置工具不含 `ListDir`（Bash/Read/Write/Edit/Grep/Glob/PowerShell/ResetTools/Skill）；pi-rust 用 `FunctionTool` 自实现 `ListDir`。tool/python-tool 文档与 examples/tool 示例如实反映：搜索浏览用 Grep/Glob，目录列举可用 `FunctionTool` 自定义或 workspace 文件操作。

**Rationale**: 子代理明确发现 crate 内无 ListDirTool。文档不得宣称存在（违宪 §5）。

**Alternatives considered**: 在文档中虚构 ListDir 内置工具（伪兼容，否）。

## 2. 技术选型最佳实践

### P1: MCP 示例形态
**Decision**: examples/mcp 复用 `crates/agent_scope_mcp/examples/mcp_excalidraw_debug.rs` 的 stdio 形态（`McpClient::connect` + `list_tools` + `call_tool`），连接一个真实 stdio MCP server 演示工具调用。

### P2: RAG 示例形态
**Decision**: examples/rag 复用 pi-rust 已落地的组合：`DashScopeEmbeddingModel` + `TurbovecVectorStore` + `KnowledgeBase` + `RAGMiddleware`，演示 Static 与 Agentic 两种模式。

### P3: 结构化输出标注
**Decision**: model/agent 文档如实标注「模型层 `generate_structured_output` 已实现，Agent 循环未直接接线」，避免宣称 Agent 级结构化输出可用。

### P4: feature 开关
**Decision**: 全 workspace 无 `[features]` 开关，文档一律不写「启用 xx feature」。能力可用性以文档状态标注为准。

## 3. 关键设计结论

- **镜像结构**: `docs/rust/zh/` 目录树与 `docs/python/zh` 逐文件一致（50 页），en 侧 `openapi.json` 为登记例外。
- **状态标注**: 三档（已实现/部分支持/计划中），每页顶部 `<Note>` 状态块，与 mirror-map、兼容性矩阵三方一致。
- **示例绑定**: 10 个示例 crate ↔ 文档页，编译锚点 + 运行命令。
- **版本声明**: 索引页声明「Rust 兼容基线 v2.0.5；镜像源 docs/python 2.0.7dev；2.0.7dev 新增能力以计划中标注」。

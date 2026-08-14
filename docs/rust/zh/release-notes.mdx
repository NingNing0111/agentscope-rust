---
title: "发布说明"
description: "AgentScope Rust 的版本历史、新特性与变更，按模块分组"
---

<Note>
**Rust 实现状态**: 已实现。本文档基于仓库根 `CHANGELOG.md` 整理，标注每个能力在 Rust 中的实现状态。
</Note>

## v0.1.0

*发布于 2026-08-02。*

兼容基线：AgentScope Python v2.0.5（commit `27b6a0d2a2afedf53462c9a2add33932d54b2d20`）。

### 基础层（Foundation）

- **兼容性基准**：Python golden-snapshot 测试基础设施、能力矩阵、trace schema、JSON fixtures。
- **消息与内容块模型**：`Msg`、`ContentBlock`（Text / Thinking / ToolCall / ToolResult / Data / Hint / Unknown）、工厂函数、序列化往返测试。
- **事件系统**：33 种 `AgentEvent`，覆盖回复生命周期、模型调用、流式块、工具执行、用户交互与控制事件。
- **类型定义**：`ErrorInfo`、`ErrorType`、`ReplyFinishedReason`、`JsonValue`、`Embedding`。

### 模型与 Provider

- **模型 API**：`ChatModel` trait、`ChatResponse`、`StreamAccumulator`、`Formatter`、`ModelCard`、`ToolChoice`、结构化输出。
- **Provider 架构**：可插拔 Provider 设计，`DashScopeFormatter` 分离。
- **DashScope Provider**：`DashScopeChatModel`、`DashScopeEmbeddingModel`，Qwen/Model Studio 模型，OpenAI 兼容端点，流式 SSE。

### 工具系统

- **工具系统**：`Tool` trait、`FunctionTool`（schemars 自动生成 schema）、`ToolKit` 注册表（OpenAI 兼容 schema 输出）。
- **内置工具**：Bash / Read / Write / Edit / Grep / Glob / PowerShell（Windows 注入）/ ResetTools / Skill（Feature 029 随 workspace 绑定自动注入）。
- **技能集成**：`SkillLoader`、`SkillViewer`、`LocalSkillLoader`，技能转工具流水线。

### Agent 系统

- **Agent 系统**：`Agent` trait、`ReActAgent` 实现、`Middleware`（9 个钩子点）、权限检查、中断处理。
- **流式基础设施**：`reply_stream()`、`AgentEvent` 流、流式工具调用与思考块事件。
- **事件驱动 HITL**：暂停-确认-恢复状态机（对齐 Python），`reply_stream_event` 接受三类事件输入（`UserConfirmResultEvent` / `ExternalExecutionResultEvent` / `UserInterruptEvent`），按 tool_call_id 精确匹配、支持多工具并发确认与 rules 采纳。
- **端事件内容**：流式结束事件携带累计内容，便于渲染。
- **子 Agent**：`SubAgent`、委托模式、子 Agent 生命周期管理。
- **任务规划**：内置任务规划工具 `TaskCreate`/`TaskList`/`TaskGet`/`TaskUpdate`（Feature 024，替代独立的 Planner）。

### 记忆与状态

- **记忆系统**：`Memory` trait、`FileMemory`（Markdown + frontmatter + `MEMORY.md` 索引）、`MemoryMiddleware`。
- **会话管理**：`Session`、`SessionStore`、`AgentState`、上下文裁剪。
- **TurboVec RAG**：`TurboVecStore` 向量检索。
- **TurboVec 长期记忆**：`TurbovecMemory`、`MemoryVectorIndex` 持久化向量记忆。
- **RAG 系统**：`Parser`、`Chunker`、`VectorStore` trait、`KnowledgeBase`、`RAGMiddleware`。
- **状态持久化**：`JsonFileSessionStore`、agent 状态保存/加载往返、回复后自动持久化。
- **运行时状态注入**：`InjectionConfig` 统一注入时间/未完成任务/上下文长度（Feature 026，`HintBlock` + `HintBlockEvent`）。

### 工作空间与沙箱

- **工作空间**：`WorkspaceBase` trait、`LocalWorkspace`、文件 I/O 工具、MCP 客户端配置、技能管理、上下文卸载。
- **沙箱**：`SandboxSession` trait、`LocalSandboxSession` 本地隔离实现、路径越界防护、命令执行超时、能力报告。
- **MCP 集成**：`McpClient` / `McpTool` / `McpExt`（Feature 027，基于官方 Rust SDK `rmcp`）。

### 尚未实现（计划中）

以下能力在 AgentScope Rust v0.1.0 中尚未实现，见对应文档页的「计划中」标注：TTS、库级 Console、Agent Service（HTTP 后端）、Channel（飞书/Discord）、Hub、共享（Sharing）。

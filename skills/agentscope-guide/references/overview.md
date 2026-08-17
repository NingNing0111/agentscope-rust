# 参考:项目概览与 crate 地图

> 本文档是 AgentScope Rust 项目结构的详细参考,回答"这个项目里有哪些 crate、各自负责什么、依赖关系如何"。

## 1. 仓库形态

AgentScope Rust 是一个 **Rust workspace**,采用多 crate 组织:

```text
agentscope-rust/
├── crates/          # 14 个功能 crate(agent_scope_*)
├── examples/        # 可运行示例(13 个按能力拆分的示例 crate)
├── docs/            # 文档站点(zh / en)
├── specs/           # Feature spec / plan / tasks
├── src/             # 根 package(仅 re-export,不提供 facade 库)
└── Cargo.toml       # workspace 定义
```

**关键事实**:workspace 根 package `agentscope` 不承载实际库代码,只做少量 re-export(`src/lib.rs` 目前 re-export `agent_scope_agent` 的 Planner 相关类型)。**使用时不要依赖根 package,直接依赖具体的 `agent_scope_*` crate**。

## 2. Crate 一览

| Crate | 分层 | 职责 | 关键公开类型 |
|-------|------|------|-------------|
| `agent_scope_types` | 基础 | 跨 crate 共享类型 | `ErrorType`、`ErrorInfo`、`ReplyFinishedReason`、`JsonValue` |
| `agent_scope_message` | 基础 | 消息与内容块数据协议 | `Msg`、`ContentBlock`、`TextBlock`、`ThinkingBlock`、`ToolCallBlock`、`ToolResultBlock`、工厂函数 |
| `agent_scope_event` | 基础 | Agent 事件协议 | `AgentEvent`(33 变体)、`EventType` |
| `agent_scope_model` | 抽象 | 模型抽象层 | `ChatModel` trait、`ChatResponse`、`ModelCallResult`、`StreamAccumulator`、`ModelCard`、`ToolChoice` |
| `agent_scope_rig` | Provider | rig-backed 模型实现 | `RigChatModel`、`RigEmbeddingModel`、`RigParameters`(OpenAI / Anthropic / DeepSeek) |
| `agent_scope_embedding` | 抽象 | Embedding 模型抽象 | `EmbeddingModel` trait、`EmbeddingModelCard`、`EmbeddingInput` |
| `agent_scope_tool` | 能力 | 工具系统 + Skill 工具化 | `Tool` trait、`FunctionTool`、`ToolKit`、`SkillLoader`、`SkillViewer` |
| `agent_scope_agent` | 编排 | Agent 编排层 | `Agent` trait、`ReActAgent`、`AgentConfig`、`ReActConfig`、`Middleware`、`PermissionContext`、`Planner`、`SubAgent`、`MemoryMiddleware` |
| `agent_scope_state` | 编排 | 会话与状态管理 | `Session`、`SessionImpl`、`SessionStore`、`AgentState`、`TokenCounter`、`TrimStrategy` |
| `agent_scope_memory` | 能力 | 长期记忆 | `Memory` trait、`FileMemory`、`TurbovecMemory`、`MemoryConfig`、`MemoryEntry`、`Backend` |
| `agent_scope_rag` | 能力 | RAG 管线 | `Parser`、`Chunker`、`VectorStore`、`KnowledgeBase`、`RAGMiddleware`、`TurbovecVectorStore` |
| `agent_scope_workspace` | 能力 | 工作空间 | `LocalWorkspace`、`WorkspaceBase`、`WorkspaceManager`、`SkillManager`、`McpRegistry` |
| `agent_scope_mcp` | 能力 | MCP 服务器集成 | `McpClient`、`McpTool`、`McpExt`(依赖 `rmcp` 官方 SDK) |
| `agent_scope_sandbox` | 能力 | 沙箱执行 | `SandboxSession`、`LocalSandboxSession`、`SandboxPolicy`、`CapabilityReport` |
| `agent_scope_utils` | 工具 | 通用工具 | 内部辅助 |

## 3. 依赖关系(DAG)

```text
基础层:  types ← message ← event
             ↑          ↑
抽象层:      ├── model ←┴── (依赖 types/message)
             └── embedding
能力层:  tool(依赖 message)     memory(独立)
         rag(依赖 embedding/model/tool)  
         workspace(依赖 tool)   sandbox(独立)
         mcp(依赖 workspace/tool)   state(依赖 message)
编排层:  agent(依赖 model/message/tool/event/memory/state)
Provider: rig(依赖 model/embedding)
```

要点:
- `agent_scope_agent` 是**唯一依赖了大部分模块的编排层**,也是绝大多数应用直接面对的核心 crate。
- 基础 crate(`types`/`message`/`event`)不依赖任何 AgentScope 其他 crate,可单独使用。
- `agent_scope_rig` 是当前**唯一内置 Provider**;接入新厂商需在独立 crate 实现 `ChatModel` / `EmbeddingModel` trait(或复用 rig 新增 backend)。

## 4. 分层职责

1. **基础层**:定义"数据长什么样"——消息协议、内容块、事件枚举、错误分类。
2. **抽象层**:定义"能力接口"——`ChatModel`、`EmbeddingModel` 这类 trait,不绑定厂商。
3. **能力层**:实现"具体能力"——工具系统、记忆、RAG、工作空间、MCP 集成、沙箱。
4. **编排层**:把能力组合成 Agent——`ReActAgent` 的 reasoning→acting 循环、middleware 挂载、权限检查。
5. **Provider 层**:厂商适配——rig-backed 的 HTTP 调用与消息格式转换(OpenAI / Anthropic / DeepSeek)。

## 5. 何时用哪个 crate

| 需求 | 依赖的 crate |
|------|-------------|
| 只是构造/解析消息 | `agent_scope_message`(+ `agent_scope_types`) |
| 直接调用某个模型 | `agent_scope_model` + `agent_scope_rig` |
| 构建一个能对话 + 用工具的 Agent | `agent_scope_agent` + `agent_scope_rig` + `agent_scope_tool` + `agent_scope_message` |
| 给 Agent 加长期记忆 | 上述 + `agent_scope_memory`(经 `agent_scope_agent::MemoryMiddleware`) |
| 给 Agent 加文档知识库 | 上述 + `agent_scope_rag` + `agent_scope_embedding` |
| Agent 需要操作文件/Shell | 上述 + `agent_scope_workspace`(或 `agent_scope_sandbox`) |
| Agent 需要调用外部 MCP 服务器(Excalidraw、搜索等) | 上述 + `agent_scope_mcp`(`McpExt::connect_mcp`) |
| 管理多会话上下文 | `agent_scope_state` |
| 多步骤任务规划 / 子 Agent 委派 | `agent_scope_agent` 的 `Planner` / `SubAgent` |

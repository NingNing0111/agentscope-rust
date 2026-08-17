# 镜像映射清单（Mirror Map）— docs/rust ↔ docs/python

> 本文档是「docs/rust 与 docs/python 一比一对齐」的权威依据（spec FR-011）。
> 任一列更新时 MUST 同步更新对应文档页面的状态块、兼容性矩阵与本表，三者保持一致。

## 版本声明

| 项 | 值 |
|----|----|
| 镜像源 | `docs/python`（Mintlify，版本路径 `2.0.7dev`） |
| Rust 兼容基线 | AgentScope Python `v2.0.5`（commit `27b6a0d2a2afedf53462c9a2add33932d54b2d20`，见根 `CHANGELOG.md`） |
| 文档语言 | 中文（`docs/rust/zh/`）；`en/` 留待未来对称补充 |
| 站内路由 | 文档内容使用站点根路由；部署前缀由 VitePress `base` 注入 |
| 生成日期 | 2026-08-13 |
| 最后更新 | 2026-08-13 |

> **版本差说明**：`docs/python` 为 2.0.7dev 文档，Rust 兼容基线锁定 v2.0.5。2.0.7dev 新增而 v2.0.5/Rust 未实现的能力，在对应页面一律标注「计划中」，不构成伪兼容（宪法 §5）。

## 例外登记

| 例外 | 原因 |
|------|------|
| `docs/python/en/deploy/openapi.json` 不镜像 | Python 后端 OpenAPI 生成物；Rust 当前无 agent-service 后端 |
| `building-blocks/agent/subagent.md` 为 Rust 特有页面 | 库级多智能体委托为 Rust 侧独有能力，Python 文档无对应 building-block（Python 侧能力在 `deploy/agent-team` 描述） |

## 表

**列含义**：`状态` = `已实现`/`部分支持`/`计划中`（三档唯一合法取值）；`兼容等级` = `L1`-`L4`（宪法 §18，未实现页面留空）；`引用示例` = `examples/<name>` 或 `—`；`备注` = 偏差/版本差/边界。

| Python 页面（docs/python/zh/...） | Rust 页面（docs/rust/zh/...） | 状态 | 兼容等级 | 引用示例 | 备注 |
|---|---|---|---|---|---|
| `index.mdx` | `index.md` | 已实现 | — | — | 版本声明 + 能力总览 |
| `quickstart.mdx` | `quickstart.md` | 已实现 | — | `quickstart` | 快速上手 |
| `release-notes.mdx` | `release-notes.md` | 已实现 | — | — | 基于根 CHANGELOG.md |
| `building-blocks/agent/overview.mdx` | `building-blocks/agent/overview.md` | 已实现 | L2 | `agent` | Agent trait / ReActAgent 主循环 |
| `building-blocks/agent/configure-agent.mdx` | `building-blocks/agent/configure-agent.md` | 已实现 | L3 | `agent` | 构造期配置 AgentConfig::builder |
| `building-blocks/agent/run-agent.mdx` | `building-blocks/agent/run-agent.md` | 已实现 | L2 | `chat` | reply/reply_stream/reply_stream_event/observe |
| `building-blocks/agent/human-in-the-loop.mdx` | `building-blocks/agent/human-in-the-loop.md` | 已实现 | L2 | `human-in-the-loop` | 暂停-确认-恢复 + reply_stream_event（Confirm/ExternalResult/Interrupt 三类事件输入） |
| `building-blocks/agent/interrupt-agent.mdx` | `building-blocks/agent/interrupt-agent.md` | 已实现 | L2 | `agent` | interrupt() 置位 + 事件注入 UserInterruptEvent（INTERRUPTED 结束） |
| — | `building-blocks/agent/subagent.md` | 已实现 | L3 | `subagent` | 库级多智能体委托（SubAgent/Registry/delegate_*/MultiAgentConversation）；Rust 特有页面 |
| `building-blocks/console.mdx` | `building-blocks/console.md` | 计划中 | — | — | Rust 无库级 console；pi-rust ratatui TUI 可参考 |
| `building-blocks/context/overview.mdx` | `building-blocks/context/overview.md` | 部分支持 | L2 | — | 注入/压缩/卸载已实现；模型摘要策略 deferred |
| `building-blocks/context/compress-context.mdx` | `building-blocks/context/compress-context.md` | 已实现 | L2 | — | ContextConfig 移除最旧+占位摘要 |
| `building-blocks/context/environment-awareness.mdx` | `building-blocks/context/environment-awareness.md` | 已实现 | L2 | — | InjectionConfig + maybe_inject_runtime_state (Feature 026) |
| `building-blocks/context/offload-context.mdx` | `building-blocks/context/offload-context.md` | 已实现 | L2 | — | offload_context/offload_tool_result |
| `building-blocks/long-term-memory.mdx` | `building-blocks/long-term-memory.md` | 已实现 | L2 | `memory` | FileMemory + TurbovecMemory 双后端 |
| `building-blocks/message-and-event.mdx` | `building-blocks/message-and-event.md` | 已实现 | L1 | `chat` | Msg/ContentBlock(7 块)/AgentEvent(33 事件) |
| `building-blocks/middleware.mdx` | `building-blocks/middleware.md` | 已实现 | L3 | `agent` | Middleware trait 9 钩子 |
| `building-blocks/model/overview.mdx` | `building-blocks/model/overview.md` | 已实现 | L1 | — | 仅 DashScope provider；无 TTS/Realtime |
| `building-blocks/model/llm.mdx` | `building-blocks/model/llm.md` | 已实现 | L2 | — | ChatModel trait + DashScopeChatModel |
| `building-blocks/model/embedding.mdx` | `building-blocks/model/embedding.md` | 已实现 | L2 | — | EmbeddingModel + DashScopeEmbeddingModel |
| `building-blocks/model/tts.mdx` | `building-blocks/model/tts.md` | 计划中 | — | — | Rust 无 TTS；仅音频数据块能力 |
| `building-blocks/permission-system/overview.mdx` | `building-blocks/permission-system/overview.md` | 已实现 | L2 | `agent` | PermissionEngine/5 模式/allow-deny-ask |
| `building-blocks/permission-system/permission-mode.mdx` | `building-blocks/permission-system/permission-mode.md` | 已实现 | L2 | `agent` | PermissionMode 5 档 |
| `building-blocks/permission-system/permission-rule.mdx` | `building-blocks/permission-system/permission-rule.md` | 已实现 | L2 | `agent` | PermissionRule allow/deny/ask 通配符 |
| `building-blocks/permission-system/tool-check.mdx` | `building-blocks/permission-system/tool-check.md` | 部分支持 | L2 | `agent` | 比 Python 细粒度 admin policy 部分覆盖 |
| `building-blocks/plan.mdx` | `building-blocks/plan.md` | 已实现 | L3 | `agent` | 任务规划工具 (Feature 024) 替代 Planner |
| `building-blocks/rag.mdx` | `building-blocks/rag.md` | 已实现 | L2 | `rag` | KnowledgeBase/RAGMiddleware(Static+Agentic) |
| `building-blocks/tool/overview.mdx` | `building-blocks/tool/overview.md` | 已实现 | L2 | `tool` | Tool/ToolKit/ToolGroup |
| `building-blocks/tool/python-tool.mdx` | `building-blocks/tool/python-tool.md` | 部分支持 | L3 | `tool` | 等价 FunctionTool；非 Python 执行器 |
| `building-blocks/tool/mcp.mdx` | `building-blocks/tool/mcp.md` | 已实现 | L2 | `mcp` | McpClient/McpTool/McpExt 客户端接入 |
| `building-blocks/tool/skill.mdx` | `building-blocks/tool/skill.md` | 已实现 | L2 | `skill` | SkillViewer/LocalSkillLoader/SkillTool |
| `building-blocks/tool/manage-tools.mdx` | `building-blocks/tool/manage-tools.md` | 已实现 | L2 | `tool` | ToolKit 注册/分组/ResetTools |
| `building-blocks/workspace/overview.mdx` | `building-blocks/workspace/overview.md` | 已实现 | L2 | `workspace` / `sandbox` | WorkspaceBase/LocalWorkspace + 沙箱小节 |
| `building-blocks/workspace/manage-resources.mdx` | `building-blocks/workspace/manage-resources.md` | 部分支持 | L2 | `workspace` | 无独立资源配额模型 |
| `building-blocks/workspace/mcp-gateway.mdx` | `building-blocks/workspace/mcp-gateway.md` | 部分支持 | L2 | `mcp` | 仅客户端接入；无服务端 gateway |
| `building-blocks/workspace/run-workspace.mdx` | `building-blocks/workspace/run-workspace.md` | 已实现 | L2 | `workspace` | WorkspaceBackend + 内置工具注入 (Feature 029) |
| `deploy/agent-service.mdx` | `deploy/agent-service.md` | 计划中 | — | — | Rust 无 HTTP server（无 axum/actix） |
| `deploy/agent-team.mdx` | `deploy/agent-team.md` | 部分支持 | L3 | — | 库级 SubAgent/delegate_*；无 service 级 team 框架 |
| `deploy/channel/custom.mdx` | `deploy/channel/custom.md` | 计划中 | — | — | 无 channel 代码 |
| `deploy/channel/discord.mdx` | `deploy/channel/discord.md` | 计划中 | — | — | 无 channel 代码 |
| `deploy/channel/feishu.mdx` | `deploy/channel/feishu.md` | 计划中 | — | — | 无 channel 代码 |
| `deploy/channel/overview.mdx` | `deploy/channel/overview.md` | 计划中 | — | — | 无 channel 代码 |
| `deploy/channel/routing.mdx` | `deploy/channel/routing.md` | 计划中 | — | — | 无 channel 代码 |
| `deploy/hub/mcp-hub.mdx` | `deploy/hub/mcp-hub.md` | 计划中 | — | — | 本地 McpRegistry 为替代 |
| `deploy/hub/overview.mdx` | `deploy/hub/overview.md` | 计划中 | — | — | 无 hub 服务 |
| `deploy/hub/skill-hub.mdx` | `deploy/hub/skill-hub.md` | 计划中 | — | — | 本地 SkillManager 为替代 |
| `deploy/rag.mdx` | `deploy/rag.md` | 部分支持 | L2 | — | 库级 RAGMiddleware；无 RAG HTTP 服务 |
| `deploy/sharing.mdx` | `deploy/sharing.md` | 计划中 | — | — | 无 sharing 能力 |
| `deploy/workspace-manager.mdx` | `deploy/workspace-manager.md` | 部分支持 | L2 | — | 本地 WorkspaceManager（多租户+TTL）；非部署形态 |
| `others/change-log.mdx` | `others/change-log.md` | 已实现 | — | — | Python 2.0 vs 1.0 摘译 + Rust 状态 |
| `others/faq.mdx` | `others/faq.md` | 已实现 | — | — | 面向 Rust 版 FAQ |

> 共 51 条。状态三档与各页面顶部状态块、兼容性矩阵保持一致（spec FR-011）。
